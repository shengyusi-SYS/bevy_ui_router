use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::contract::ViewRendererIssuer;
use crate::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBuildOutcome, ViewBuildRequest, ViewDisposalHint,
    ViewFailure, ViewOutputId, ViewPartialOutput, ViewRenderPlan, ViewRequestId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeViewRendererMode {
    Success,
    BuildFailure,
    ApplyFailure,
    BuildStaleResult,
    ApplyStaleResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeViewRenderer {
    issuer: ViewRendererIssuer,
    mode: FakeViewRendererMode,
    next_request: u64,
    next_output: u64,
    pending_plans: BTreeSet<ViewRequestId>,
    partial_outputs: Vec<ViewPartialOutput>,
}

impl FakeViewRenderer {
    pub fn new(mode: FakeViewRendererMode) -> Self {
        Self {
            issuer: next_renderer_issuer(),
            mode,
            next_request: 1,
            next_output: 1,
            pending_plans: BTreeSet::new(),
            partial_outputs: Vec::new(),
        }
    }

    pub fn request(&mut self, descriptor: RouteViewDescriptor) -> ViewBuildRequest {
        let request_id = ViewRequestId::from_raw(self.next_request);
        self.next_request += 1;
        ViewBuildRequest::new(self.issuer, request_id, descriptor)
    }

    pub fn build(&mut self, request: &ViewBuildRequest) -> ViewBuildOutcome {
        if request.issuer() != self.issuer {
            return ViewBuildOutcome::Stale(ViewFailure::stale_result(
                request.request_id(),
                "request was not issued by this fake renderer",
            ));
        }

        match self.mode {
            FakeViewRendererMode::Success
            | FakeViewRendererMode::ApplyFailure
            | FakeViewRendererMode::ApplyStaleResult => {
                self.pending_plans.insert(request.request_id());
                ViewBuildOutcome::Built(ViewRenderPlan::new(request))
            }
            FakeViewRendererMode::BuildFailure => ViewBuildOutcome::Failed(
                ViewFailure::build_failed(request.request_id(), "fake renderer build failed"),
            ),
            FakeViewRendererMode::BuildStaleResult => ViewBuildOutcome::Stale(
                ViewFailure::stale_result(request.request_id(), "stale renderer build result"),
            ),
        }
    }

    pub fn apply(&mut self, plan: ViewRenderPlan) -> ViewApplyOutcome {
        let request_id = plan.request_id();

        if plan.issuer() != self.issuer || !self.pending_plans.remove(&request_id) {
            return ViewApplyOutcome::Stale(ViewFailure::stale_result(
                request_id,
                "render plan was not issued by this fake renderer",
            ));
        }

        match self.mode {
            FakeViewRendererMode::Success => {
                let output = self.next_partial_output(&plan);
                self.partial_outputs.push(output.clone());
                ViewApplyOutcome::Applied(output)
            }
            FakeViewRendererMode::ApplyFailure => {
                let output = self.next_partial_output(&plan);
                ViewApplyOutcome::Failed(ViewFailure::apply_failed(
                    request_id,
                    "fake renderer apply failed",
                    Some(output),
                ))
            }
            FakeViewRendererMode::ApplyStaleResult => ViewApplyOutcome::Stale(
                ViewFailure::stale_result(request_id, "stale renderer apply result"),
            ),
            FakeViewRendererMode::BuildFailure | FakeViewRendererMode::BuildStaleResult => {
                ViewApplyOutcome::Stale(ViewFailure::stale_result(
                    request_id,
                    "apply called without successful build",
                ))
            }
        }
    }

    pub fn partial_outputs(&self) -> &[ViewPartialOutput] {
        &self.partial_outputs
    }

    fn next_partial_output(&mut self, plan: &ViewRenderPlan) -> ViewPartialOutput {
        let output_id = ViewOutputId::from_raw(self.next_output);
        self.next_output += 1;
        ViewPartialOutput::new(
            plan.request_id(),
            output_id,
            plan.descriptor().binding().clone(),
            Some(ViewDisposalHint::new(format!(
                "fake-output-{}",
                output_id.raw()
            ))),
        )
    }
}

fn next_renderer_issuer() -> ViewRendererIssuer {
    static NEXT_RENDERER: AtomicU64 = AtomicU64::new(1);

    ViewRendererIssuer::from_raw(NEXT_RENDERER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViewBindingKey;
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn fake_renderer_allocates_request_ids_deterministically() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let first = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("first")));
        let second = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("second")));

        assert_eq!(first.request_id().raw(), 1);
        assert_eq!(second.request_id().raw(), 2);
    }

    #[test]
    fn fake_renderer_build_success_does_not_create_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };

        assert_eq!(plan.request_id(), request.request_id());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_rejects_request_from_another_renderer_without_partial_output() {
        let mut source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let ViewBuildOutcome::Stale(failure) = renderer.build(&request) else {
            panic!("foreign request should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_apply_success_creates_and_records_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };

        let ViewApplyOutcome::Applied(output) = renderer.apply(plan) else {
            panic!("success mode should apply");
        };

        assert_eq!(output.request_id(), request.request_id());
        assert_eq!(output.output_id().raw(), 1);
        assert_eq!(output.binding().as_str(), "leaf");
        assert_eq!(output.disposal_hint().unwrap().as_str(), "fake-output-1");
        assert_eq!(renderer.partial_outputs(), &[output]);
    }

    #[test]
    fn fake_renderer_rejects_already_consumed_plan_without_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };
        let consumed = plan.clone();
        let ViewApplyOutcome::Applied(_output) = renderer.apply(plan) else {
            panic!("success mode should apply");
        };

        let ViewApplyOutcome::Stale(failure) = renderer.apply(consumed) else {
            panic!("consumed plan should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert_eq!(renderer.partial_outputs().len(), 1);
    }

    #[test]
    fn fake_renderer_rejects_plan_from_another_renderer_without_partial_output() {
        let mut source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = source.build(&request) else {
            panic!("source renderer should build");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("foreign plan should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_build_failure_without_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildFailure);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Failed(failure) = renderer.build(&request) else {
            panic!("build failure mode should fail build");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewBuildFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_build_failure_mode_rejects_apply_without_partial_output() {
        let mut plan_source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = plan_source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = plan_source.build(&request) else {
            panic!("success mode should provide a forced plan");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildFailure);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("build failure mode should not apply a forced plan");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_apply_failure_with_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::ApplyFailure);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("apply failure mode should build before failing apply");
        };

        let ViewApplyOutcome::Failed(failure) = renderer.apply(plan) else {
            panic!("apply failure mode should fail apply");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert_eq!(
            failure.partial_output().map(ViewPartialOutput::output_id),
            Some(ViewOutputId::from_raw(1))
        );
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_stale_build_result_without_visible_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildStaleResult);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Stale(failure) = renderer.build(&request) else {
            panic!("stale mode should report stale build result");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_build_stale_mode_rejects_apply_without_partial_output() {
        let mut plan_source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = plan_source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = plan_source.build(&request) else {
            panic!("success mode should provide a forced plan");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildStaleResult);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("build stale mode should not apply a forced plan");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_stale_apply_result_without_visible_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::ApplyStaleResult);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("apply stale mode should build before reporting stale apply");
        };

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("apply stale mode should report stale apply result");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }
}
