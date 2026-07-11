#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RouteViewKey(String);

impl RouteViewKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RouteViewKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RouteViewKeyError { _private: () });
        }

        Ok(Self(value))
    }
}

#[derive(Debug)]
pub struct RouteViewKeyError {
    _private: (),
}

impl std::fmt::Display for RouteViewKeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("route view key must not be empty")
    }
}

impl std::error::Error for RouteViewKeyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_key_rejects_an_empty_value() {
        assert!(RouteViewKey::new("").is_err());
    }

    #[test]
    fn view_key_and_error_have_the_required_traits() {
        fn assert_key_traits<T: Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash>() {}
        fn assert_error_traits<
            T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
        >() {
        }

        assert_key_traits::<RouteViewKey>();
        assert_error_traits::<RouteViewKeyError>();
    }
}
