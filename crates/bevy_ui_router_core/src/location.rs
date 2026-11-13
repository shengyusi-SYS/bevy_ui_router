use std::{fmt, sync::Arc};

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct RouteLocation {
    parts: Arc<LocationParts>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct LocationParts {
    path: String,
    search: Option<String>,
    fragment: Option<String>,
}

impl fmt::Debug for RouteLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteLocation")
            .field("path", &self.parts.path)
            .field("search", &self.parts.search)
            .field("fragment", &self.parts.fragment)
            .finish()
    }
}

impl RouteLocation {
    /// Creates a location from a path. Search and fragment are supplied separately.
    ///
    /// Use [`Self::from_address`] to split an internal address containing `?` or `#`.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            parts: Arc::new(LocationParts {
                path: normalize_path(path.into()),
                search: None,
                fragment: None,
            }),
        }
    }

    /// Splits an internal address into its path, search, and fragment.
    ///
    /// The first `#` starts the fragment, and the first `?` before it starts the
    /// search. The delimiters are removed; missing and explicitly empty values
    /// remain distinct. The path uses [`Self::new`]'s slash normalization.
    ///
    /// This does not decode percent escapes, resolve `.` or `..`, or interpret
    /// URL schemes, authorities, or a current-location base. For example,
    /// `"?tab=info"` addresses the root path, not the current page.
    ///
    /// ```
    /// use bevy_ui_router_core::prelude::RouteLocation;
    ///
    /// let location = RouteLocation::from_address("/items/42?tab=info#details");
    /// assert_eq!(location.path(), "/items/42");
    /// assert_eq!(location.search(), Some("tab=info"));
    /// assert_eq!(location.fragment(), Some("details"));
    /// ```
    pub fn from_address(address: impl AsRef<str>) -> Self {
        let address = address.as_ref();
        let (before_fragment, fragment) = address
            .split_once('#')
            .map_or((address, None), |(path, fragment)| (path, Some(fragment)));
        let (path, search) = before_fragment
            .split_once('?')
            .map_or((before_fragment, None), |(path, search)| {
                (path, Some(search))
            });
        Self {
            parts: Arc::new(LocationParts {
                path: normalize_path(path.to_owned()),
                search: search.map(str::to_owned),
                fragment: fragment.map(str::to_owned),
            }),
        }
    }

    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        let search = search.into();
        Arc::make_mut(&mut self.parts).search = Some(search);
        self
    }

    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        let fragment = fragment.into();
        Arc::make_mut(&mut self.parts).fragment = Some(fragment);
        self
    }

    pub fn path(&self) -> &str {
        &self.parts.path
    }

    pub fn search(&self) -> Option<&str> {
        self.parts.search.as_deref()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.parts.fragment.as_deref()
    }
}

impl From<&str> for RouteLocation {
    fn from(address: &str) -> Self {
        Self::from_address(address)
    }
}

impl From<String> for RouteLocation {
    fn from(address: String) -> Self {
        Self::from_address(address)
    }
}

fn normalize_path(path: String) -> String {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", segments.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_empty_and_relative_path_to_raw_path_boundary() {
        assert_eq!(RouteLocation::new("").path(), "/");
        assert_eq!(RouteLocation::new("settings").path(), "/settings");
    }

    #[test]
    fn canonicalizes_slash_only_path_differences() {
        let canonical = RouteLocation::new("/items/1");

        for equivalent in ["/items//1/", "items/1"] {
            assert_eq!(RouteLocation::new(equivalent).path(), canonical.path());
        }

        for root in ["", "/", "///"] {
            assert_eq!(RouteLocation::new(root).path(), "/");
        }
    }

    #[test]
    fn keeps_raw_search_and_fragment_separate() {
        let location = RouteLocation::new("/items")
            .with_search("?a=1&a=2")
            .with_fragment("#details");

        assert_eq!(location.path(), "/items");
        assert_eq!(location.search(), Some("?a=1&a=2"));
        assert_eq!(location.fragment(), Some("#details"));
    }

    #[test]
    fn clones_keep_value_traits_and_independent_edits() {
        use std::{collections::hash_map::DefaultHasher, hash::Hasher};

        let original = RouteLocation::from_address("/items?a=1#details");
        let equivalent = RouteLocation::new("/items")
            .with_search("a=1")
            .with_fragment("details");
        assert_eq!(original, equivalent);
        let hash = |location: &RouteLocation| {
            let mut state = DefaultHasher::new();
            std::hash::Hash::hash(location, &mut state);
            state.finish()
        };
        assert_eq!(hash(&original), hash(&equivalent));
        assert_eq!(
            format!("{original:?}"),
            r#"RouteLocation { path: "/items", search: Some("a=1"), fragment: Some("details") }"#
        );

        let edited = original.clone().with_search("").with_fragment("next");
        assert_eq!(original.search(), Some("a=1"));
        assert_eq!(original.fragment(), Some("details"));
        assert_eq!(edited.path(), original.path());
        assert_eq!(edited.search(), Some(""));
        assert_eq!(edited.fragment(), Some("next"));
        assert_ne!(original, edited);
    }

    #[test]
    fn address_splits_delimiters_without_decoding_or_reordering_values() {
        let location =
            RouteLocation::from_address("items//%2F/../42/?tab=info&tab=details#part?query#nested");
        assert_eq!(location.path(), "/items/%2F/../42");
        assert_eq!(location.search(), Some("tab=info&tab=details"));
        assert_eq!(location.fragment(), Some("part?query#nested"));
        assert_eq!(
            RouteLocation::from("/items?q=1"),
            RouteLocation::from_address("/items?q=1")
        );
        assert_eq!(
            RouteLocation::from(String::from("/items?q=1")),
            RouteLocation::from_address("/items?q=1")
        );

        // Existing structured construction continues treating its path as a path.
        let path_only = RouteLocation::new("/items?q=1#part");
        assert_eq!(path_only.path(), "/items?q=1#part");
        assert_eq!(path_only.search(), None);
        assert_eq!(path_only.fragment(), None);
    }

    #[test]
    fn address_preserves_missing_and_empty_components_and_has_no_relative_base() {
        for (address, search, fragment) in [
            ("/", None, None),
            ("/?", Some(""), None),
            ("/#", None, Some("")),
            ("/?#", Some(""), Some("")),
            ("?tab=info", Some("tab=info"), None),
            ("#?tab=info", None, Some("?tab=info")),
        ] {
            let location = RouteLocation::from_address(address);
            assert_eq!(location.path(), "/");
            assert_eq!(location.search(), search, "{address}");
            assert_eq!(location.fragment(), fragment, "{address}");
        }
    }
}
