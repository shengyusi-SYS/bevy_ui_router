use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const BEVY_GIT: &str = "git = \"https://github.com/shengyusi-SYS/bevy.git\"";
const BEVY_REV: &str = "rev = \"61f78f75684e6b0bc28a13a5933f8f55ca653fa7\"";

fn section_name(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']')
}

fn dependency_section_kind(section: &str) -> Option<&str> {
    let section = if let Some(target_tail) = section.strip_prefix("target.") {
        let (_, remainder) = target_tail.split_once('.')?;
        remainder
    } else {
        section
    };

    ["dependencies", "dev-dependencies", "build-dependencies"]
        .into_iter()
        .find(|kind| *kind == section || section.starts_with(&format!("{kind}.")))
}

fn dependency_subtable_alias(section: &str) -> Option<&str> {
    let kind = dependency_section_kind(section)?;
    section
        .strip_prefix("target.")
        .and_then(|tail| tail.split_once('.').map(|(_, remainder)| remainder))
        .unwrap_or(section)
        .strip_prefix(kind)?
        .strip_prefix('.')
}

fn dependency_name_is_forbidden(name: &str) -> bool {
    name == "bsn" || name.starts_with("bevy") && !name.starts_with("bevy_ui_router")
}

fn dependency_entry(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

fn manifest_contains_dependency_config(manifest: &str, dependency: &str, fragment: &str) -> bool {
    manifest.lines().any(|line| {
        let line = line.trim();
        line.starts_with(&format!("{dependency} = {{")) && line.contains(fragment)
    })
}

fn quoted_string(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(&value[..end])
}

fn package_rename(key: &str, value: &str) -> Option<String> {
    let value = value.split('#').next().unwrap_or(value).trim();

    if key == "package" {
        return quoted_string(value).map(str::to_owned);
    }

    let inline_table = value.strip_prefix('{')?.trim_end_matches('}').trim();
    for entry in inline_table.split(',') {
        let Some((field, field_value)) = entry.split_once('=') else {
            continue;
        };

        if field.trim() == "package" {
            return quoted_string(field_value).map(str::to_owned);
        }
    }

    None
}

fn feature_entries(manifest: &str) -> BTreeMap<String, String> {
    let mut current_section = None;
    let mut features = BTreeMap::new();

    for raw_line in manifest.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);
            continue;
        }

        if current_section == Some("features") {
            let (name, value) = line
                .split_once('=')
                .expect("feature entries should contain '='");
            features.insert(name.trim().to_owned(), value.trim().to_owned());
        }
    }

    features
}

fn dependency_violations(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut violations = Vec::new();

    for (line_number, raw_line) in manifest.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);

            if let Some(alias) = dependency_subtable_alias(section) {
                if dependency_name_is_forbidden(alias) {
                    violations.push(format!(
                        "line {}: forbidden dependency subtable `{}`",
                        line_number + 1,
                        section
                    ));
                }
            }
            continue;
        }

        if !current_section.is_some_and(|section| section == "dependencies") {
            continue;
        }

        let Some((key, value)) = dependency_entry(line) else {
            continue;
        };

        if dependency_subtable_alias(current_section.expect("checked above")).is_none()
            && dependency_name_is_forbidden(key)
        {
            violations.push(format!(
                "line {}: forbidden dependency key `{}`",
                line_number + 1,
                key
            ));
        }

        if let Some(package) = package_rename(key, value) {
            if package == "bsn" {
                violations.push(format!(
                    "line {}: forbidden package rename to `bsn`",
                    line_number + 1
                ));
            } else if package.starts_with("bevy") && !package.starts_with("bevy_ui_router") {
                violations.push(format!(
                    "line {}: forbidden package rename to Bevy-family crate",
                    line_number + 1
                ));
            }
        }
    }

    violations
}

#[test]
fn facade_default_keeps_bevy_out_of_the_default_path() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("facade manifest should be readable");

    let expected_features = BTreeMap::from([
        (
            "bsn".to_owned(),
            "[\"ecs\", \"dep:bevy_ui_router_bsn\"]".to_owned(),
        ),
        ("default".to_owned(), "[\"view\"]".to_owned()),
        ("devtools".to_owned(), "[]".to_owned()),
        (
            "ecs".to_owned(),
            "[\"view\", \"dep:bevy_ui_router_ecs\"]".to_owned(),
        ),
        (
            "ecs-test-support".to_owned(),
            "[\"ecs\", \"bevy_ui_router_ecs/test-support\"]".to_owned(),
        ),
        (
            "platform".to_owned(),
            "[\"dep:bevy_ui_router_platform\"]".to_owned(),
        ),
        (
            "view".to_owned(),
            "[\"dep:bevy_ui_router_view\"]".to_owned(),
        ),
        (
            "test-support".to_owned(),
            "[\"ecs-test-support\"]".to_owned(),
        ),
    ]);

    assert_eq!(feature_entries(&manifest), expected_features);

    let violations = dependency_violations(&manifest);
    assert!(
        violations.is_empty(),
        "found forbidden dependency declarations:\n{}",
        violations.join("\n")
    );

    for dependency in ["bevy_app", "bevy_ecs", "bevy_ui"] {
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, BEVY_GIT),
            "{dependency} should use the approved Bevy git source"
        );
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, BEVY_REV),
            "{dependency} should pin the approved Bevy revision"
        );
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, "version = \"0.19.0\""),
            "{dependency} should pin the approved Bevy version"
        );
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, "default-features = false"),
            "{dependency} should disable default features"
        );
        assert!(
            !manifest_contains_dependency_config(
                &manifest,
                dependency,
                "branch = \"sys/bevy-0.19-router\""
            ),
            "{dependency} must not use a movable branch"
        );
    }

    assert!(
        manifest_contains_dependency_config(&manifest, "syn", "version = \"2\"")
            && manifest_contains_dependency_config(&manifest, "syn", "features = [\"full\"]"),
        "facade dev-dependencies should include syn with its full feature"
    );
}

#[test]
fn dependency_subtables_are_checked_for_forbidden_crates() {
    let manifest = r#"
[features]
default = []

[dependencies]
render_inline = { package = "bevy_render", version = "1" }
bsn_inline = { package = "bsn", version = "1" }
core_inline = { package = "bevy_ui_router_core", path = "../core" }

[dependencies.bsn]
version = "1"

[dependencies.render]
package   =   "bevy_render"

[target.'cfg(target_os = "linux")'.dev-dependencies.bevy_input]
version = "1"

[target.'cfg(target_os = "windows")'.build-dependencies.integration]
package   =   "bsn"

[target.'cfg(target_os = "macos")'.dependencies.bevy_ui_router_view_alias]
package = "bevy_ui_router_view"

[package.metadata.dependencies]
package = "bevy_window"

[target.'cfg(target_os = "android")'.package.metadata.dependencies]
package = "bsn"
"#;

    assert_eq!(
        dependency_violations(manifest),
        vec![
            "line 6: forbidden package rename to Bevy-family crate",
            "line 7: forbidden package rename to `bsn`",
            "line 10: forbidden dependency subtable `dependencies.bsn`",
            "line 16: forbidden dependency subtable `target.'cfg(target_os = \"linux\")'.dev-dependencies.bevy_input`",
        ]
    );
}

#[test]
fn facade_prelude_exposes_core_matcher_contracts() {
    use bevy_ui_router::prelude::{
        MatchedBranch, RouteMatcher, RoutePattern, RoutePatternKind, RouteTree,
    };

    let pattern = RoutePattern::path("/").unwrap();
    assert_eq!(pattern.kind(), RoutePatternKind::Path);

    let tree = RouteTree::builder().root(pattern).unwrap().build().unwrap();
    let _matcher = RouteMatcher::new(tree);

    fn assert_types<T>() {}
    assert_types::<MatchedBranch>();
}

#[test]
fn facade_prelude_exposes_branch_planning_contracts() {
    use bevy_ui_router::prelude::{
        BranchDiff, ErrorBoundaryKey, MatchedBranch, MatchedRoute, RouteExplicitKey, RouteId,
        RouteLocation, RouteMeta, RouteNodeOptions, RouteParams, RouteSlotKey,
    };

    let _slot = RouteSlotKey::default();
    let _key = RouteExplicitKey::new("stable").unwrap();
    let _boundary = ErrorBoundaryKey::named("root").unwrap();
    let _location = RouteLocation::new("/items");
    let _meta = RouteMeta::new();
    let _options = RouteNodeOptions::new();

    fn assert_types<T>() {}
    assert_types::<BranchDiff>();
    assert_types::<MatchedBranch>();
    assert_types::<MatchedRoute>();
    assert_types::<RouteId>();
    assert_types::<RouteLocation>();
    assert_types::<RouteParams>();
}

#[test]
fn facade_prelude_exposes_phase_2_observation_contracts() {
    use bevy_ui_router::prelude::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, FrameCleanupPlan,
        FrameReconciliationPlan, NavigationTransactionSnapshot, NavigationTransactionState,
        RouteFrameBranch, RouteFrameIdentity, RouteFrameRecord, RouteFrameScope, RouteFrameStatus,
        StaleApplyReason, StaleApplyResult, TransactionCommitError, TransactionCommitErrorKind,
        TransactionFailure,
    };

    fn assert_public<T>() {}
    assert_public::<FrameApplyOutcome>();
    assert_public::<FrameApplyRequest>();
    assert_public::<FrameApplyToken>();
    assert_public::<FrameCleanupPlan>();
    assert_public::<FrameReconciliationPlan>();
    assert_public::<NavigationTransactionSnapshot>();
    assert_public::<NavigationTransactionState>();
    assert_public::<RouteFrameBranch>();
    assert_public::<RouteFrameIdentity>();
    assert_public::<RouteFrameRecord>();
    assert_public::<RouteFrameScope>();
    assert_public::<StaleApplyReason>();
    assert_public::<StaleApplyResult>();
    assert_public::<TransactionCommitError>();
    assert_public::<TransactionCommitErrorKind>();
    assert_public::<TransactionFailure>();
    assert_eq!(RouteFrameStatus::Active, RouteFrameStatus::Active);
}

#[cfg(feature = "view")]
#[test]
fn facade_default_prelude_exposes_phase_3a_view_binding_contracts() {
    use bevy_ui_router::prelude::{RouteViewDescriptor, ViewBindingKey};

    fn assert_public<T>() {}
    assert_public::<RouteViewDescriptor>();
    assert_public::<ViewBindingKey>();

    let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
    assert_eq!(descriptor.binding().as_str(), "leaf");
}

#[cfg(feature = "view")]
#[test]
fn facade_view_module_exposes_phase_3a_protocol_observations_by_explicit_path() {
    use bevy_ui_router::view::{
        ViewApplyOutcome, ViewBuildOutcome, ViewBuildRequest, ViewDisposalHint, ViewFailure,
        ViewOutputId, ViewPartialOutput, ViewRenderPlan, ViewRequestId,
    };

    fn assert_public<T>() {}
    assert_public::<ViewApplyOutcome>();
    assert_public::<ViewBuildOutcome>();
    assert_public::<ViewBuildRequest>();
    assert_public::<ViewDisposalHint>();
    assert_public::<ViewFailure>();
    assert_public::<ViewOutputId>();
    assert_public::<ViewPartialOutput>();
    assert_public::<ViewRenderPlan>();
    assert_public::<ViewRequestId>();
}
