use crate::error::RouteError;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteExplicitKey(String);

impl RouteExplicitKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RouteError> {
        let value = value.into();
        validate_non_empty(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ErrorBoundaryKey(String);

impl ErrorBoundaryKey {
    pub fn named(name: impl Into<String>) -> Result<Self, RouteError> {
        let name = name.into();
        validate_non_empty(&name)?;
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RouteMeta {
    diagnostic_label: Option<String>,
}

impl RouteMeta {
    pub fn new() -> Self {
        Self {
            diagnostic_label: None,
        }
    }

    pub fn with_diagnostic_label(mut self, label: impl Into<String>) -> Result<Self, RouteError> {
        let label = label.into();
        validate_non_empty(&label)?;
        self.diagnostic_label = Some(label);
        Ok(self)
    }

    pub fn diagnostic_label(&self) -> Option<&str> {
        self.diagnostic_label.as_deref()
    }
}

fn validate_non_empty(value: &str) -> Result<(), RouteError> {
    if value.is_empty() {
        return Err(RouteError::invalid_route_declaration(
            "declaration value must not be empty",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn explicit_key_and_error_boundary_are_declaration_data() {
        let explicit = RouteExplicitKey::new("user-shell").unwrap();
        let boundary = ErrorBoundaryKey::named("root-boundary").unwrap();

        assert_eq!(explicit.as_str(), "user-shell");
        assert_eq!(boundary.as_str(), "root-boundary");
    }

    #[test]
    fn route_meta_exposes_only_generic_diagnostic_label() {
        let meta = RouteMeta::new()
            .with_diagnostic_label("settings route")
            .unwrap();

        assert_eq!(meta.diagnostic_label(), Some("settings route"));
    }

    #[test]
    fn declaration_keys_reject_empty_values() {
        for error in [
            RouteExplicitKey::new("").unwrap_err(),
            ErrorBoundaryKey::named("").unwrap_err(),
            RouteMeta::new().with_diagnostic_label("").unwrap_err(),
        ] {
            assert_eq!(error.kind(), RouteErrorKind::InvalidRouteDeclaration);
            assert_eq!(
                error.source_category(),
                RouteErrorSourceCategory::RouterCore
            );
        }
    }
}
