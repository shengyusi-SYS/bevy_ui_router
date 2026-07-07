use bevy_ui_router_core::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};

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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};

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
}
