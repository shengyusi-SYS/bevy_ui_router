#![allow(dead_code)]

use bevy_ui_router_core::prelude::RouteViewKey;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ViewDriverKey(String);

impl ViewDriverKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ViewDriverKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ViewDriverKeyError { _private: () });
        }

        Ok(Self(value))
    }
}

#[derive(Debug)]
pub struct ViewDriverKeyError {
    _private: (),
}

impl std::fmt::Display for ViewDriverKeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("view driver key must not be empty")
    }
}

impl std::error::Error for ViewDriverKeyError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteViewDescriptor {
    driver: ViewDriverKey,
}

impl RouteViewDescriptor {
    pub fn new(driver: ViewDriverKey) -> Self {
        Self { driver }
    }

    pub(crate) fn driver(&self) -> &ViewDriverKey {
        &self.driver
    }
}

pub struct RouteViewRegistry {
    entries: Vec<(RouteViewKey, RouteViewDescriptor)>,
}

impl RouteViewRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn insert(
        &mut self,
        view: RouteViewKey,
        descriptor: RouteViewDescriptor,
    ) -> Result<(), RouteViewRegistryError> {
        if self.get(&view).is_some() {
            return Err(RouteViewRegistryError { _private: () });
        }

        self.entries.push((view, descriptor));
        Ok(())
    }

    pub(crate) fn get(&self, view: &RouteViewKey) -> Option<&RouteViewDescriptor> {
        self.entries
            .iter()
            .find_map(|(stored, descriptor)| (stored == view).then_some(descriptor))
    }
}

#[derive(Debug)]
pub struct RouteViewRegistryError {
    _private: (),
}

impl std::fmt::Display for RouteViewRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("route view key is already registered")
    }
}

impl std::error::Error for RouteViewRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::prelude::RouteViewKey;

    #[test]
    fn driver_key_rejects_an_empty_value_and_has_the_frozen_traits() {
        fn assert_key_traits<T: Clone + std::fmt::Debug + Eq + PartialEq + std::hash::Hash>() {}
        fn assert_error_traits<
            T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
        >() {
        }

        assert!(ViewDriverKey::new("").is_err());
        assert_key_traits::<ViewDriverKey>();
        assert_error_traits::<ViewDriverKeyError>();
    }

    #[test]
    fn registry_preserves_insertion_order_for_distinct_views() {
        let first = RouteViewKey::new("first").unwrap();
        let second = RouteViewKey::new("second").unwrap();
        let driver = ViewDriverKey::new("renderer").unwrap();
        let mut registry = RouteViewRegistry::new();

        registry
            .insert(first.clone(), RouteViewDescriptor::new(driver.clone()))
            .unwrap();
        registry
            .insert(second.clone(), RouteViewDescriptor::new(driver))
            .unwrap();

        let stored_views = registry
            .entries
            .iter()
            .map(|(view, _descriptor)| view)
            .collect::<Vec<_>>();
        assert_eq!(stored_views, vec![&first, &second]);
    }

    #[test]
    fn duplicate_view_is_rejected_without_replacing_the_first_descriptor() {
        fn assert_error_traits<
            T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
        >() {
        }

        let view = RouteViewKey::new("leaf").unwrap();
        let first_driver = ViewDriverKey::new("first").unwrap();
        let replacement_driver = ViewDriverKey::new("replacement").unwrap();
        let mut registry = RouteViewRegistry::new();

        registry
            .insert(view.clone(), RouteViewDescriptor::new(first_driver.clone()))
            .unwrap();
        let error = registry
            .insert(view.clone(), RouteViewDescriptor::new(replacement_driver))
            .unwrap_err();

        assert_error_traits::<RouteViewRegistryError>();
        assert_eq!(
            registry.get(&view).map(RouteViewDescriptor::driver),
            Some(&first_driver)
        );
        assert_eq!(error.to_string(), "route view key is already registered");
    }

    #[test]
    fn one_driver_can_back_multiple_distinct_views() {
        let first = RouteViewKey::new("first").unwrap();
        let second = RouteViewKey::new("second").unwrap();
        let driver = ViewDriverKey::new("shared").unwrap();
        let mut registry = RouteViewRegistry::new();

        registry
            .insert(first.clone(), RouteViewDescriptor::new(driver.clone()))
            .unwrap();
        registry
            .insert(second.clone(), RouteViewDescriptor::new(driver.clone()))
            .unwrap();

        assert_eq!(
            registry.get(&first).map(RouteViewDescriptor::driver),
            Some(&driver)
        );
        assert_eq!(
            registry.get(&second).map(RouteViewDescriptor::driver),
            Some(&driver)
        );
    }
}
