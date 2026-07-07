use std::sync::atomic::{AtomicU64, Ordering};

use crate::branch::RouteParams;
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::meta::RouteExplicitKey;
use crate::slot::RouteSlotKey;

#[allow(dead_code)]
fn push_typed_field(raw: &mut String, field: &str, value: &str) {
    raw.push('|');
    raw.push_str(field);
    raw.push(':');
    raw.push_str(&value.len().to_string());
    raw.push(':');
    raw.push_str(value);
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteParamsFingerprint(String);

impl RouteParamsFingerprint {
    #[allow(dead_code)]
    pub(crate) fn from_route_params(route: RouteId, params: &RouteParams) -> Self {
        let route_raw = route.raw().to_string();
        let mut raw = format!("route:{}:{route_raw}", route_raw.len());
        for (name, value) in params.iter() {
            raw.push_str("|param:");
            raw.push_str(&name.len().to_string());
            raw.push(':');
            raw.push_str(name);
            raw.push('=');
            raw.push_str(&value.len().to_string());
            raw.push(':');
            raw.push_str(value);
        }
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSearchFingerprint(String);

impl RouteSearchFingerprint {
    #[allow(dead_code)]
    pub(crate) fn from_location(location: &RouteLocation) -> Self {
        match location.search() {
            Some(search) => Self(format!("search:some:{}:{search}", search.len())),
            None => Self("search:none".to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSegmentReuseKey(String);

impl RouteSegmentReuseKey {
    #[allow(dead_code)]
    pub(crate) fn from_parts(
        route: RouteId,
        slot: &RouteSlotKey,
        explicit_key: Option<&RouteExplicitKey>,
        params: &RouteParamsFingerprint,
    ) -> Self {
        let route_raw = route.raw().to_string();
        let mut raw = "segment".to_owned();
        push_typed_field(&mut raw, "route", &route_raw);
        push_typed_field(&mut raw, "slot", slot.as_str());
        match explicit_key {
            Some(key) => push_typed_field(&mut raw, "key", key.as_str()),
            None => push_typed_field(&mut raw, "key", ""),
        }
        push_typed_field(&mut raw, "params", params.as_str());
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteBranchFingerprint(String);

impl RouteBranchFingerprint {
    #[allow(dead_code)]
    pub(crate) fn from_parts<'a>(
        segments: impl IntoIterator<Item = &'a RouteSegmentReuseKey>,
        search: &RouteSearchFingerprint,
    ) -> Self {
        let mut raw = "branch".to_owned();
        for segment in segments {
            push_typed_field(&mut raw, "segment", segment.as_str());
        }
        push_typed_field(&mut raw, "search", search.as_str());
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[allow(dead_code)]
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RouteBranchProvenance(String);

impl RouteBranchProvenance {
    #[allow(dead_code)]
    pub(crate) fn next_tree_source() -> Self {
        static NEXT_TREE_SOURCE: AtomicU64 = AtomicU64::new(1);
        let raw = NEXT_TREE_SOURCE.fetch_add(1, Ordering::Relaxed);
        Self(format!("tree:{raw}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::RouteParams;
    use crate::ids::RouteId;
    use crate::location::RouteLocation;
    use crate::meta::RouteExplicitKey;
    use crate::slot::RouteSlotKey;

    #[test]
    fn params_fingerprint_uses_route_id_and_sorted_local_params() {
        let params = RouteParams::from_pairs([
            ("project".to_owned(), "alpha".to_owned()),
            ("document".to_owned(), "intro".to_owned()),
        ]);
        let fingerprint = RouteParamsFingerprint::from_route_params(RouteId::new(7), &params);

        assert_eq!(
            fingerprint.as_str(),
            "route:1:7|param:8:document=5:intro|param:7:project=5:alpha"
        );
    }

    #[test]
    fn params_fingerprint_is_structural_when_values_contain_separators() {
        let params = RouteParams::from_pairs([
            ("a".to_owned(), "1;2".to_owned()),
            ("a_eq".to_owned(), "x=y".to_owned()),
        ]);
        let fingerprint = RouteParamsFingerprint::from_route_params(RouteId::new(3), &params);

        assert_eq!(
            fingerprint.as_str(),
            "route:1:3|param:1:a=3:1;2|param:4:a_eq=3:x=y"
        );
    }

    #[test]
    fn raw_search_fingerprint_preserves_absence_empty_order_and_encoding() {
        let none = RouteSearchFingerprint::from_location(&RouteLocation::new("/items"));
        let empty =
            RouteSearchFingerprint::from_location(&RouteLocation::new("/items").with_search(""));
        let leading_question = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("?a=1"),
        );
        let no_question =
            RouteSearchFingerprint::from_location(&RouteLocation::new("/items").with_search("a=1"));
        let ordered = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=1&b=2"),
        );
        let reversed = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("b=2&a=1"),
        );
        let encoded = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=%7E"),
        );
        let decoded =
            RouteSearchFingerprint::from_location(&RouteLocation::new("/items").with_search("x=~"));
        let lower_percent = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=%2f"),
        );
        let upper_percent = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=%2F"),
        );
        let repeated_a = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=1&a=2"),
        );
        let repeated_b = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=2&a=1"),
        );
        let empty_value =
            RouteSearchFingerprint::from_location(&RouteLocation::new("/items").with_search("a="));
        let missing_value =
            RouteSearchFingerprint::from_location(&RouteLocation::new("/items").with_search("a"));
        let with_fragment = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items")
                .with_search("a=1")
                .with_fragment("ignored"),
        );

        assert_eq!(none.as_str(), "search:none");
        assert_eq!(empty.as_str(), "search:some:0:");
        assert_ne!(leading_question, no_question);
        assert_ne!(ordered, reversed);
        assert_ne!(encoded, decoded);
        assert_ne!(lower_percent, upper_percent);
        assert_ne!(repeated_a, repeated_b);
        assert_ne!(empty_value, missing_value);
        assert_eq!(no_question, with_fragment);
    }

    #[test]
    fn segment_reuse_key_and_branch_fingerprint_use_typed_structural_encoding() {
        let params = RouteParams::from_pairs([("id".to_owned(), "4|2".to_owned())]);
        let params_fingerprint =
            RouteParamsFingerprint::from_route_params(RouteId::new(9), &params);
        let slot = RouteSlotKey::named("details").unwrap();
        let explicit_key = RouteExplicitKey::new("a|b=1").unwrap();
        let segment = RouteSegmentReuseKey::from_parts(
            RouteId::new(9),
            &slot,
            Some(&explicit_key),
            &params_fingerprint,
        );
        let search = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items/42").with_search("tab=a|b"),
        );
        let branch = RouteBranchFingerprint::from_parts([&segment], &search);

        assert_eq!(
            segment.as_str(),
            "segment|route:1:9|slot:7:details|key:5:a|b=1|params:26:route:1:9|param:2:id=3:4|2"
        );
        assert_eq!(
            branch.as_str(),
            "branch|segment:81:segment|route:1:9|slot:7:details|key:5:a|b=1|params:26:route:1:9|param:2:id=3:4|2|search:21:search:some:7:tab=a|b"
        );
    }
}
