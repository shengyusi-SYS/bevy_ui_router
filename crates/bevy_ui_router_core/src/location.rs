#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RouteLocation {
    path: String,
    search: Option<String>,
    fragment: Option<String>,
}

impl RouteLocation {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: normalize_path(path.into()),
            search: None,
            fragment: None,
        }
    }

    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
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
}
