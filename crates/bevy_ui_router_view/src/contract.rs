use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ui_router_core::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
use bevy_ui_router_core::transaction::FrameApplyRequest;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ViewBindingKey(String);

impl ViewBindingKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewRequestId(u64);

impl ViewRequestId {
    #[allow(dead_code)]
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewOutputId(u64);

impl ViewOutputId {
    #[allow(dead_code)]
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ViewRendererIssuer(u64);

impl ViewRendererIssuer {
    #[allow(dead_code)]
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteViewDescriptor {
    binding: ViewBindingKey,
}

impl RouteViewDescriptor {
    pub fn new(binding: ViewBindingKey) -> Self {
        Self { binding }
    }

    pub fn binding(&self) -> &ViewBindingKey {
        &self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewBuildRequest {
    issuer: ViewRendererIssuer,
    request_id: ViewRequestId,
    descriptor: RouteViewDescriptor,
}

impl ViewBuildRequest {
    #[allow(dead_code)]
    pub(crate) fn new(
        issuer: ViewRendererIssuer,
        request_id: ViewRequestId,
        descriptor: RouteViewDescriptor,
    ) -> Self {
        Self {
            issuer,
            request_id,
            descriptor,
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn issuer(&self) -> ViewRendererIssuer {
        self.issuer
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn descriptor(&self) -> &RouteViewDescriptor {
        &self.descriptor
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ViewReportSession {
    frame_request: FrameApplyRequest,
    issuer: ViewRendererIssuer,
    request_id: ViewRequestId,
    descriptor: RouteViewDescriptor,
    completed: bool,
}

impl ViewReportSession {
    pub fn from_frame_request(
        request: &FrameApplyRequest,
        descriptor: RouteViewDescriptor,
    ) -> Self {
        Self {
            frame_request: request.clone(),
            issuer: next_view_renderer_issuer(),
            request_id: next_view_request_id(),
            descriptor,
            completed: false,
        }
    }

    pub fn frame_request(&self) -> &FrameApplyRequest {
        &self.frame_request
    }

    pub fn build_request(&self) -> ViewBuildRequest {
        ViewBuildRequest::new(self.issuer, self.request_id, self.descriptor.clone())
    }

    pub fn build_success(&mut self) -> ViewBuildOutcome {
        self.ensure_not_completed();
        ViewBuildOutcome::Built(ViewRenderPlan::new(&self.build_request()))
    }

    pub fn build_failed(&mut self, redacted_message: impl Into<String>) -> ViewBuildOutcome {
        self.complete();
        ViewBuildOutcome::Failed(ViewFailure::build_failed(self.request_id, redacted_message))
    }

    pub fn applied(&mut self, disposal_hint: Option<impl Into<String>>) -> ViewReportedOutput {
        self.complete();
        ViewReportedOutput {
            outcome: ViewApplyOutcome::Applied(self.partial_output(disposal_hint)),
        }
    }

    pub fn apply_failed(
        &mut self,
        redacted_message: impl Into<String>,
        disposal_hint: Option<impl Into<String>>,
    ) -> ViewApplyOutcome {
        self.complete();
        ViewApplyOutcome::Failed(ViewFailure::apply_failed(
            self.request_id,
            redacted_message,
            Some(self.partial_output(disposal_hint)),
        ))
    }

    pub fn stale(&mut self, redacted_message: impl Into<String>) -> ViewApplyOutcome {
        self.complete();
        ViewApplyOutcome::Stale(ViewFailure::stale_result(self.request_id, redacted_message))
    }

    fn partial_output(&self, disposal_hint: Option<impl Into<String>>) -> ViewPartialOutput {
        ViewPartialOutput::new(
            self.request_id,
            next_view_output_id(),
            self.descriptor.binding().clone(),
            disposal_hint.map(ViewDisposalHint::new),
        )
    }

    fn ensure_not_completed(&self) {
        assert!(
            !self.completed,
            "view report sessions cannot emit multiple terminal outcomes"
        );
    }

    fn complete(&mut self) {
        self.ensure_not_completed();
        self.completed = true;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRenderPlan {
    issuer: ViewRendererIssuer,
    request_id: ViewRequestId,
    descriptor: RouteViewDescriptor,
}

impl ViewRenderPlan {
    #[allow(dead_code)]
    pub(crate) fn new(request: &ViewBuildRequest) -> Self {
        Self {
            issuer: request.issuer(),
            request_id: request.request_id(),
            descriptor: request.descriptor().clone(),
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn issuer(&self) -> ViewRendererIssuer {
        self.issuer
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn descriptor(&self) -> &RouteViewDescriptor {
        &self.descriptor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewDisposalHint(String);

impl ViewDisposalHint {
    #[allow(dead_code)]
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartialOutput {
    request_id: ViewRequestId,
    output_id: ViewOutputId,
    binding: ViewBindingKey,
    disposal_hint: Option<ViewDisposalHint>,
}

impl ViewPartialOutput {
    #[allow(dead_code)]
    pub(crate) fn new(
        request_id: ViewRequestId,
        output_id: ViewOutputId,
        binding: ViewBindingKey,
        disposal_hint: Option<ViewDisposalHint>,
    ) -> Self {
        Self {
            request_id,
            output_id,
            binding,
            disposal_hint,
        }
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub const fn output_id(&self) -> ViewOutputId {
        self.output_id
    }

    pub fn binding(&self) -> &ViewBindingKey {
        &self.binding
    }

    pub fn disposal_hint(&self) -> Option<&ViewDisposalHint> {
        self.disposal_hint.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFailure {
    request_id: ViewRequestId,
    error: RouteError,
    partial_output: Option<ViewPartialOutput>,
}

impl ViewFailure {
    #[allow(dead_code)]
    pub(crate) fn build_failed(
        request_id: ViewRequestId,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            request_id,
            error: RouteError::new(
                RouteErrorKind::ViewBuildFailed,
                RouteErrorSourceCategory::RendererContract,
                redacted_message,
            ),
            partial_output: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn apply_failed(
        request_id: ViewRequestId,
        redacted_message: impl Into<String>,
        partial_output: Option<ViewPartialOutput>,
    ) -> Self {
        Self {
            request_id,
            error: RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                redacted_message,
            ),
            partial_output,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn stale_result(
        request_id: ViewRequestId,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            request_id,
            error: RouteError::new(
                RouteErrorKind::StaleResult,
                RouteErrorSourceCategory::RendererContract,
                redacted_message,
            ),
            partial_output: None,
        }
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn error(&self) -> &RouteError {
        &self.error
    }

    pub fn partial_output(&self) -> Option<&ViewPartialOutput> {
        self.partial_output.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewBuildOutcome {
    Built(ViewRenderPlan),
    Failed(ViewFailure),
    Stale(ViewFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewApplyOutcome {
    Applied(ViewPartialOutput),
    Failed(ViewFailure),
    Stale(ViewFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewReportedOutput {
    outcome: ViewApplyOutcome,
}

impl ViewReportedOutput {
    pub fn outcome(&self) -> &ViewApplyOutcome {
        &self.outcome
    }
}

fn next_view_renderer_issuer() -> ViewRendererIssuer {
    static NEXT_ISSUER: AtomicU64 = AtomicU64::new(1);
    ViewRendererIssuer::from_raw(NEXT_ISSUER.fetch_add(1, Ordering::Relaxed))
}

fn next_view_request_id() -> ViewRequestId {
    static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
    ViewRequestId::from_raw(NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed))
}

fn next_view_output_id() -> ViewOutputId {
    static NEXT_OUTPUT_ID: AtomicU64 = AtomicU64::new(1);
    ViewOutputId::from_raw(NEXT_OUTPUT_ID.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};
    use bevy_ui_router_core::location::RouteLocation;
    use bevy_ui_router_core::matcher::RouteMatcher;
    use bevy_ui_router_core::pattern::RoutePattern;
    use bevy_ui_router_core::route_tree::RouteTree;
    use bevy_ui_router_core::runtime::RouterRuntime;

    #[test]
    fn descriptor_and_build_request_preserve_binding_and_request_id() {
        let request_id = ViewRequestId::from_raw(7);
        let issuer = ViewRendererIssuer::from_raw(1);
        let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("layout"));
        let request = ViewBuildRequest::new(issuer, request_id, descriptor.clone());

        assert_eq!(request.issuer(), issuer);
        assert_eq!(request.request_id(), request_id);
        assert_eq!(request.descriptor(), &descriptor);
        assert_eq!(request.descriptor().binding().as_str(), "layout");
    }

    #[test]
    fn build_plan_is_not_partial_output() {
        let request = ViewBuildRequest::new(
            ViewRendererIssuer::from_raw(1),
            ViewRequestId::from_raw(9),
            RouteViewDescriptor::new(ViewBindingKey::new("leaf")),
        );
        let plan = ViewRenderPlan::new(&request);
        let ViewRenderPlan {
            issuer,
            request_id,
            descriptor,
        } = &plan;

        assert_eq!(*issuer, request.issuer());
        assert_eq!(*request_id, request.request_id());
        assert_eq!(descriptor, request.descriptor());
        assert_eq!(plan.issuer(), request.issuer());
        assert_eq!(plan.request_id(), request.request_id());
        assert_eq!(plan.descriptor(), request.descriptor());
    }

    #[test]
    fn apply_failure_carries_apply_created_partial_output_and_cleanup_hint() {
        let request_id = ViewRequestId::from_raw(11);
        let output = ViewPartialOutput::new(
            request_id,
            ViewOutputId::from_raw(3),
            ViewBindingKey::new("leaf"),
            Some(ViewDisposalHint::new("fake-output-3")),
        );
        let failure = ViewFailure::apply_failed(request_id, "apply failed", Some(output.clone()));

        assert_eq!(failure.request_id(), request_id);
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert_eq!(failure.partial_output(), Some(&output));
        assert_eq!(
            failure
                .partial_output()
                .unwrap()
                .disposal_hint()
                .unwrap()
                .as_str(),
            "fake-output-3"
        );
    }

    #[test]
    fn build_and_stale_failures_do_not_invent_partial_output() {
        let request_id = ViewRequestId::from_raw(13);
        let build = ViewFailure::build_failed(request_id, "build failed");
        let stale = ViewFailure::stale_result(request_id, "stale renderer result");

        assert_eq!(build.error().kind(), RouteErrorKind::ViewBuildFailed);
        assert_eq!(
            build.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(build.partial_output().is_none());
        assert_eq!(stale.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            stale.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(stale.partial_output().is_none());
    }

    mod report_session {
        use super::*;
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::sync::{Mutex, OnceLock};

        #[test]
        fn build_request_and_build_success_preserve_descriptor_without_public_request_constructor()
        {
            let request = issued_requests("/items/2").remove(1);
            let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
            let mut session = ViewReportSession::from_frame_request(&request, descriptor.clone());

            let build_request = session.build_request();
            assert_eq!(session.frame_request(), &request);
            assert_eq!(build_request.descriptor(), &descriptor);

            let ViewBuildOutcome::Built(plan) = session.build_success() else {
                panic!("build success should produce a render plan");
            };

            assert_eq!(plan.request_id(), build_request.request_id());
            assert_eq!(plan.descriptor(), build_request.descriptor());
        }

        #[test]
        fn distinct_frame_requests_produce_distinct_session_provenance() {
            let mut requests = issued_requests("/items/2");
            let first_request = requests.remove(0);
            let second_request = requests.remove(0);
            let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
            let first = ViewReportSession::from_frame_request(&first_request, descriptor.clone());
            let second = ViewReportSession::from_frame_request(&second_request, descriptor);

            assert_ne!(first.frame_request(), second.frame_request());
            assert_ne!(
                first.build_request().request_id(),
                second.build_request().request_id()
            );
        }

        #[test]
        fn applied_reports_partial_output_and_private_disposal_hint() {
            let request = issued_requests("/items/2").remove(0);
            let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
            let mut session = ViewReportSession::from_frame_request(&request, descriptor);
            let request_id = session.build_request().request_id();

            let reported = session.applied(Some("renderer-local"));
            let output = match reported.outcome() {
                ViewApplyOutcome::Applied(output) => output,
                _ => panic!("applied should create a partial output"),
            };

            assert_eq!(output.request_id(), request_id);
            assert_eq!(output.binding().as_str(), "leaf");
            assert_eq!(
                output.disposal_hint().map(ViewDisposalHint::as_str),
                Some("renderer-local")
            );
        }

        #[test]
        fn build_and_apply_failures_preserve_phase_3a_error_taxonomy() {
            let request = issued_requests("/items/2").remove(2);
            let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
            let mut build_failure =
                ViewReportSession::from_frame_request(&request, descriptor.clone());

            let ViewBuildOutcome::Failed(build_failure) =
                build_failure.build_failed("build failed")
            else {
                panic!("build failure should stay in the build phase");
            };
            assert_eq!(
                build_failure.error().kind(),
                RouteErrorKind::ViewBuildFailed
            );
            assert_eq!(
                build_failure.error().source_category(),
                RouteErrorSourceCategory::RendererContract
            );
            assert!(build_failure.partial_output().is_none());

            let mut apply_failure = ViewReportSession::from_frame_request(&request, descriptor);
            let ViewApplyOutcome::Failed(apply_failure) =
                apply_failure.apply_failed("apply failed", Some("cleanup-leaf"))
            else {
                panic!("apply failure should stay in the apply phase");
            };
            assert_eq!(
                apply_failure.error().kind(),
                RouteErrorKind::ViewApplyFailed
            );
            assert_eq!(
                apply_failure.error().source_category(),
                RouteErrorSourceCategory::RendererContract
            );
            assert_eq!(
                apply_failure
                    .partial_output()
                    .and_then(ViewPartialOutput::disposal_hint)
                    .map(ViewDisposalHint::as_str),
                Some("cleanup-leaf")
            );
        }

        #[test]
        fn stale_reports_renderer_local_stale_without_partial_output() {
            let request = issued_requests("/items/2").remove(0);
            let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
            let mut session = ViewReportSession::from_frame_request(&request, descriptor);

            let ViewApplyOutcome::Stale(stale) = session.stale("renderer stale") else {
                panic!("stale should remain a stale apply outcome");
            };

            assert_eq!(stale.error().kind(), RouteErrorKind::StaleResult);
            assert_eq!(
                stale.error().source_category(),
                RouteErrorSourceCategory::RendererContract
            );
            assert!(stale.partial_output().is_none());
        }

        #[test]
        fn terminal_reports_cannot_be_emitted_twice() {
            let request = issued_requests("/items/2").remove(1);

            assert_terminal_panics_after_followup(
                ViewReportSession::from_frame_request(
                    &request,
                    RouteViewDescriptor::new(ViewBindingKey::new("build-failed")),
                ),
                |session| {
                    let _ = session.build_failed("build failed");
                },
            );
            assert_terminal_panics_after_followup(
                ViewReportSession::from_frame_request(
                    &request,
                    RouteViewDescriptor::new(ViewBindingKey::new("applied")),
                ),
                |session| {
                    let _ = session.applied(None::<String>);
                },
            );
            assert_terminal_panics_after_followup(
                ViewReportSession::from_frame_request(
                    &request,
                    RouteViewDescriptor::new(ViewBindingKey::new("apply-failed")),
                ),
                |session| {
                    let _ = session.apply_failed("apply failed", None::<String>);
                },
            );
            assert_terminal_panics_after_followup(
                ViewReportSession::from_frame_request(
                    &request,
                    RouteViewDescriptor::new(ViewBindingKey::new("stale")),
                ),
                |session| {
                    let _ = session.stale("stale");
                },
            );
        }

        fn issued_requests(path: &str) -> Vec<bevy_ui_router_core::transaction::FrameApplyRequest> {
            let mut builder = RouteTree::builder()
                .root(RoutePattern::path("/").unwrap())
                .unwrap();
            let root = builder.root_id().unwrap();
            let items = builder
                .route(root, RoutePattern::path("items").unwrap())
                .unwrap();
            builder
                .route(items, RoutePattern::path(":id").unwrap())
                .unwrap();
            let target = RouteMatcher::new(builder.build().unwrap())
                .match_location(&RouteLocation::new(path))
                .unwrap();
            let mut runtime = RouterRuntime::new();

            runtime
                .begin_first_navigation(&target)
                .unwrap()
                .requests()
                .to_vec()
        }

        fn assert_terminal_panics_after_followup(
            mut session: ViewReportSession,
            complete: impl FnOnce(&mut ViewReportSession),
        ) {
            complete(&mut session);

            let result = catch_panics_silently(|| {
                let _ = session.stale("second report");
            });

            assert!(
                result.is_err(),
                "terminal sessions should reject follow-up reports"
            );
        }

        fn catch_panics_silently(
            f: impl FnOnce(),
        ) -> Result<(), Box<dyn std::any::Any + Send + 'static>> {
            static PANIC_HOOK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let _guard = PANIC_HOOK_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap();
            let hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let result = catch_unwind(AssertUnwindSafe(f));
            std::panic::set_hook(hook);
            result
        }
    }
}
