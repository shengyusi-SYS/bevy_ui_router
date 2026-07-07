#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteErrorKind {
    InvalidRoutePattern,
    InvalidRouteDeclaration,
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

    pub fn invalid_route_pattern(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::InvalidRoutePattern,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub(crate) fn invalid_route_declaration(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::InvalidRouteDeclaration,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub fn invalid_route_tree(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::InvalidRouteTree,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub fn not_found(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::NotFound,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub fn match_conflict(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::MatchConflict,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
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

    #[test]
    fn match_errors_are_router_core_errors_with_redacted_messages() {
        let not_found = RouteError::not_found("no route matched requested location");
        let conflict = RouteError::match_conflict("ambiguous route match");

        assert_eq!(not_found.kind(), RouteErrorKind::NotFound);
        assert_eq!(conflict.kind(), RouteErrorKind::MatchConflict);
        assert_eq!(
            not_found.source_category(),
            RouteErrorSourceCategory::RouterCore
        );
        assert_eq!(
            conflict.source_category(),
            RouteErrorSourceCategory::RouterCore
        );
        assert!(!not_found.redacted_message().contains("/"));
    }
}
