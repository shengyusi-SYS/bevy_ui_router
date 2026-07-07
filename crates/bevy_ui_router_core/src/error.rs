#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteErrorKind {
    InvalidRoutePattern,
    InvalidRouteTree,
    NotFound,
    MatchConflict,
    GuardRejected,
    ViewBuildFailed,
    ViewApplyFailed,
    StaleResult,
    AdapterFailed,
    HostOrSlotUnavailable,
    FinalizeFailed,
    CleanupFailed,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteErrorSourceCategory {
    RouterCore,
    RendererContract,
    HostSlotIntegration,
    InputLocationIntegration,
    AppExtension,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteError {
    kind: RouteErrorKind,
    source_category: RouteErrorSourceCategory,
    redacted_message: String,
}

impl RouteError {
    pub fn new(
        kind: RouteErrorKind,
        source_category: RouteErrorSourceCategory,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source_category,
            redacted_message: redacted_message.into(),
        }
    }

    pub const fn kind(&self) -> RouteErrorKind {
        self.kind
    }

    pub const fn source_category(&self) -> RouteErrorSourceCategory {
        self.source_category
    }

    pub fn redacted_message(&self) -> &str {
        &self.redacted_message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_keeps_adapter_failures_router_neutral() {
        let error = RouteError::new(
            RouteErrorKind::AdapterFailed,
            RouteErrorSourceCategory::RendererContract,
            "renderer reported a redacted failure",
        );

        assert_eq!(error.kind(), RouteErrorKind::AdapterFailed);
        assert_eq!(
            error.source_category(),
            RouteErrorSourceCategory::RendererContract
        );
    }

    #[test]
    fn finalization_error_does_not_name_specific_integration_crate() {
        assert_eq!(
            format!("{:?}", RouteErrorKind::FinalizeFailed),
            "FinalizeFailed"
        );
    }
}
