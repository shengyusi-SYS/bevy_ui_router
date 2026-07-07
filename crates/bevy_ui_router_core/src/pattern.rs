use std::collections::BTreeSet;

use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutePatternKind {
    Path,
    Index,
    Pathless,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutePathSegment {
    Static(String),
    Param(String),
    Wildcard(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutePattern {
    kind: RoutePatternKind,
    original: String,
    segments: Vec<RoutePathSegment>,
}

impl RoutePattern {
    pub fn path(pattern: impl Into<String>) -> Result<Self, RouteError> {
        let original = pattern.into();
        let normalized = original.trim_start_matches('/');

        if normalized.is_empty() {
            return Ok(Self {
                kind: RoutePatternKind::Path,
                original,
                segments: Vec::new(),
            });
        }

        let mut seen_names = BTreeSet::new();
        let parts: Vec<&str> = normalized.split('/').collect();
        let mut segments = Vec::with_capacity(parts.len());

        for (index, part) in parts.iter().copied().enumerate() {
            if part.is_empty() {
                return Err(invalid_pattern("route path contains an empty segment"));
            }

            if let Some(name) = part.strip_prefix(':') {
                validate_capture_name(name, &mut seen_names)?;
                segments.push(RoutePathSegment::Param(name.to_owned()));
                continue;
            }

            if let Some(name) = part.strip_prefix('*') {
                validate_capture_name(name, &mut seen_names)?;
                if index + 1 != parts.len() {
                    return Err(invalid_pattern("wildcard segment must be last"));
                }
                segments.push(RoutePathSegment::Wildcard(name.to_owned()));
                continue;
            }

            segments.push(RoutePathSegment::Static(part.to_owned()));
        }

        Ok(Self {
            kind: RoutePatternKind::Path,
            original,
            segments,
        })
    }

    pub fn index() -> Self {
        Self {
            kind: RoutePatternKind::Index,
            original: String::new(),
            segments: Vec::new(),
        }
    }

    pub fn pathless() -> Self {
        Self {
            kind: RoutePatternKind::Pathless,
            original: String::new(),
            segments: Vec::new(),
        }
    }

    pub const fn kind(&self) -> RoutePatternKind {
        self.kind
    }

    pub fn segments(&self) -> &[RoutePathSegment] {
        &self.segments
    }

    pub fn original(&self) -> &str {
        &self.original
    }
}

fn validate_capture_name(name: &str, seen_names: &mut BTreeSet<String>) -> Result<(), RouteError> {
    if name.is_empty() {
        return Err(invalid_pattern("capture name must not be empty"));
    }

    if !name
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return Err(invalid_pattern(
            "capture name must be ascii alphanumeric or underscore",
        ));
    }

    if !seen_names.insert(name.to_owned()) {
        return Err(invalid_pattern(
            "capture name must be unique inside one pattern",
        ));
    }

    Ok(())
}

fn invalid_pattern(message: &'static str) -> RouteError {
    RouteError::new(
        RouteErrorKind::InvalidRoutePattern,
        RouteErrorSourceCategory::RouterCore,
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_pattern_normalizes_leading_slash_and_segments() {
        let pattern = RoutePattern::path("projects/:project_id/files/*rest").unwrap();
        let with_slash = RoutePattern::path("/projects/:project_id/files/*rest").unwrap();
        let root = RoutePattern::path("/").unwrap();

        assert_eq!(pattern.kind(), RoutePatternKind::Path);
        assert_eq!(pattern.original(), "projects/:project_id/files/*rest");
        assert_eq!(with_slash.segments(), pattern.segments());
        assert!(root.segments().is_empty());
        assert_eq!(
            pattern.segments(),
            &[
                RoutePathSegment::Static("projects".to_owned()),
                RoutePathSegment::Param("project_id".to_owned()),
                RoutePathSegment::Static("files".to_owned()),
                RoutePathSegment::Wildcard("rest".to_owned()),
            ]
        );
    }

    #[test]
    fn index_and_pathless_patterns_consume_no_segments() {
        assert_eq!(RoutePattern::index().kind(), RoutePatternKind::Index);
        assert_eq!(RoutePattern::pathless().kind(), RoutePatternKind::Pathless);
        assert!(RoutePattern::index().segments().is_empty());
        assert!(RoutePattern::pathless().segments().is_empty());
    }
}
