use crate::error::RouteError;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSlotKey(String);

impl RouteSlotKey {
    pub fn default() -> Self {
        Self(String::new())
    }

    pub fn named(name: impl Into<String>) -> Result<Self, RouteError> {
        let name = name.into();
        validate_key(&name)?;
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_default(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for RouteSlotKey {
    fn default() -> Self {
        Self(String::new())
    }
}

fn validate_key(value: &str) -> Result<(), RouteError> {
    if value.is_empty() {
        return Err(RouteError::invalid_route_declaration(
            "declaration key must not be empty",
        ));
    }

    if !value
        .chars()
        .all(|ch| ch == '_' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric())
    {
        return Err(RouteError::invalid_route_declaration(
            "declaration key must be ascii alphanumeric, underscore, dash, or dot",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn slot_key_distinguishes_default_and_named_slots() {
        let default_slot = RouteSlotKey::default();
        let details = RouteSlotKey::named("details").unwrap();

        assert!(default_slot.is_default());
        assert_eq!(default_slot.as_str(), "");
        assert_eq!(details.as_str(), "details");
        assert!(!details.is_default());
    }

    #[test]
    fn slot_key_rejects_empty_or_unstable_names() {
        for name in ["", "two words", "with/slash"] {
            let error = RouteSlotKey::named(name).unwrap_err();
            assert_eq!(error.kind(), RouteErrorKind::InvalidRouteDeclaration);
            assert_eq!(
                error.source_category(),
                RouteErrorSourceCategory::RouterCore
            );
        }
    }
}
