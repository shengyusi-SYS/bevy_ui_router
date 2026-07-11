use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use toml::{Table, Value};

const BEVY_GIT: &str = "https://github.com/shengyusi-SYS/bevy.git";
const BEVY_REV: &str = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7";
const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

fn parse_manifest(input: &str) -> Table {
    toml::from_str(input).expect("manifest should parse as a TOML table")
}

fn table<'a>(manifest: &'a Table, name: &str) -> &'a Table {
    manifest.get(name).and_then(Value::as_table).unwrap()
}

fn dependency<'a>(section: &'a Table, name: &str) -> &'a Table {
    section.get(name).and_then(Value::as_table).unwrap()
}

fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

fn assert_keys(table: &Table, expected: &[&str], label: &str) {
    let actual = table.keys().cloned().collect::<BTreeSet<_>>();
    let expected = expected.iter().map(|key| (*key).to_owned()).collect();
    assert_eq!(actual, expected, "{label} keys must match exactly");
}

fn assert_bevy(table: &Table, features: &[&str], label: &str) {
    let fields = if features.is_empty() {
        &["git", "rev", "version", "default-features"][..]
    } else {
        &["git", "rev", "version", "default-features", "features"][..]
    };
    assert_keys(table, fields, label);
    assert_eq!(table.get("git").and_then(Value::as_str), Some(BEVY_GIT));
    assert_eq!(table.get("rev").and_then(Value::as_str), Some(BEVY_REV));
    assert_eq!(table.get("version").and_then(Value::as_str), Some("0.19.0"));
    assert_eq!(
        table.get("default-features").and_then(Value::as_bool),
        Some(false)
    );
    let expected = features
        .iter()
        .map(|feature| (*feature).to_owned())
        .collect();
    assert_eq!(
        table.get("features").map(strings).unwrap_or_default(),
        expected
    );
    assert!(
        !table.contains_key("branch"),
        "{label} must not declare branch"
    );
}

fn facade_safety_violations(manifest: &Table) -> Vec<String> {
    let mut violations = Vec::new();
    for name in ["bevy_app", "bevy_ecs", "bevy_ui"] {
        let locations = SECTIONS
            .iter()
            .filter(|section| {
                manifest
                    .get(**section)
                    .and_then(Value::as_table)
                    .is_some_and(|table| table.contains_key(name))
            })
            .copied()
            .collect::<Vec<_>>();
        if locations != ["dev-dependencies"] {
            violations.push(format!("{name} must appear only in [dev-dependencies]"));
        }
    }
    collect_bevy_branch_violations(manifest, &mut violations);
    violations
}

fn collect_bevy_branch_violations(table: &Table, violations: &mut Vec<String>) {
    for (name, value) in table {
        let Some(nested) = value.as_table() else {
            continue;
        };
        if name.starts_with("bevy") && nested.contains_key("branch") {
            violations.push(format!("{name} must not declare branch"));
        }
        collect_bevy_branch_violations(nested, violations);
    }
}

#[test]
fn manifest_contract_rejects_bevy_dependency_outside_dev_dependencies() {
    let manifest = parse_manifest(
        r#"
[dependencies]
[dev-dependencies]
bevy_ecs = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
bevy_ui = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
[build-dependencies]
bevy_app = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
"#,
    );
    assert_eq!(
        facade_safety_violations(&manifest),
        vec!["bevy_app must appear only in [dev-dependencies]"]
    );
}

#[test]
fn manifest_contract_rejects_any_movable_bevy_branch() {
    let manifest = parse_manifest(
        r#"
[dependencies]
[dev-dependencies]
bevy_app = { git = "https://github.com/shengyusi-SYS/bevy.git", branch = "arbitrary-moving-branch", version = "0.19.0", default-features = false }
bevy_ecs = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
bevy_ui = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
[build-dependencies]
"#,
    );
    assert_eq!(
        facade_safety_violations(&manifest),
        vec!["bevy_app must not declare branch"]
    );
}

#[test]
fn manifest_contract_rejects_target_specific_bevy_branch() {
    let manifest = parse_manifest(
        r#"
[dependencies]
[dev-dependencies]
bevy_app = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
bevy_ecs = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }
bevy_ui = { git = "https://github.com/shengyusi-SYS/bevy.git", rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7", version = "0.19.0", default-features = false }

[target.'cfg(unix)'.build-dependencies]
bevy_render = { git = "https://github.com/shengyusi-SYS/bevy.git", branch = "target-moving-branch", version = "0.19.0", default-features = false }
"#,
    );
    assert_eq!(
        facade_safety_violations(&manifest),
        vec!["bevy_render must not declare branch"]
    );
}

#[test]
fn facade_manifest_keeps_the_exact_dependency_contract() {
    let manifest = parse_manifest(
        &fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap(),
    );
    let actual_features = table(&manifest, "features")
        .iter()
        .map(|(name, value)| (name.clone(), strings(value)))
        .collect::<BTreeMap<_, _>>();
    let expected_features = BTreeMap::from([
        ("default".to_owned(), BTreeSet::from(["view".to_owned()])),
        (
            "view".to_owned(),
            BTreeSet::from(["dep:bevy_ui_router_view".to_owned()]),
        ),
        (
            "ecs".to_owned(),
            BTreeSet::from(["view".to_owned(), "dep:bevy_ui_router_ecs".to_owned()]),
        ),
        (
            "ecs-test-support".to_owned(),
            BTreeSet::from([
                "ecs".to_owned(),
                "bevy_ui_router_ecs/test-support".to_owned(),
            ]),
        ),
        (
            "test-support".to_owned(),
            BTreeSet::from(["ecs-test-support".to_owned()]),
        ),
        (
            "bsn".to_owned(),
            BTreeSet::from(["ecs".to_owned(), "dep:bevy_ui_router_bsn".to_owned()]),
        ),
        (
            "platform".to_owned(),
            BTreeSet::from(["dep:bevy_ui_router_platform".to_owned()]),
        ),
        ("devtools".to_owned(), BTreeSet::new()),
    ]);
    assert_eq!(actual_features, expected_features);
    let dependencies = table(&manifest, "dependencies");
    assert_keys(
        dependencies,
        &[
            "bevy_ui_router_core",
            "bevy_ui_router_view",
            "bevy_ui_router_ecs",
            "bevy_ui_router_bsn",
            "bevy_ui_router_platform",
        ],
        "[dependencies]",
    );
    let expected_paths = [
        ("bevy_ui_router_core", "../bevy_ui_router_core"),
        ("bevy_ui_router_view", "../bevy_ui_router_view"),
        ("bevy_ui_router_ecs", "../bevy_ui_router_ecs"),
        ("bevy_ui_router_bsn", "../bevy_ui_router_bsn"),
        ("bevy_ui_router_platform", "../bevy_ui_router_platform"),
    ];
    for (name, path) in expected_paths {
        let entry = dependency(dependencies, name);
        let expected_fields = match name {
            "bevy_ui_router_core" => &["path"][..],
            "bevy_ui_router_view" => &["path", "optional", "default-features"][..],
            _ => &["path", "optional"][..],
        };
        assert_keys(entry, expected_fields, name);
        assert_eq!(entry.get("path").and_then(Value::as_str), Some(path));
        if name != "bevy_ui_router_core" {
            assert_eq!(entry.get("optional").and_then(Value::as_bool), Some(true));
        }
        if name == "bevy_ui_router_view" {
            assert_eq!(
                entry.get("default-features").and_then(Value::as_bool),
                Some(false)
            );
        }
    }
    let dev = table(&manifest, "dev-dependencies");
    assert_keys(
        dev,
        &["bevy_app", "bevy_ecs", "bevy_ui", "syn", "toml"],
        "[dev-dependencies]",
    );
    for name in ["bevy_app", "bevy_ecs", "bevy_ui"] {
        assert_bevy(dependency(dev, name), &[], name);
    }
    let syn = dependency(dev, "syn");
    assert_keys(syn, &["version", "features"], "syn");
    assert_eq!(syn.get("version").and_then(Value::as_str), Some("2"));
    assert_eq!(
        syn.get("features").map(strings).unwrap(),
        BTreeSet::from(["full".to_owned()])
    );
    assert_eq!(dev.get("toml").and_then(Value::as_str), Some("1.1.2"));
    assert!(facade_safety_violations(&manifest).is_empty());
}

#[test]
fn facade_prelude_exposes_core_matcher_contracts() {
    use bevy_ui_router::prelude::{
        MatchedBranch, RouteMatcher, RoutePattern, RoutePatternKind, RouteTree,
    };
    let pattern = RoutePattern::path("/").unwrap();
    assert_eq!(pattern.kind(), RoutePatternKind::Path);
    let _matcher = RouteMatcher::new(RouteTree::builder().root(pattern).unwrap().build().unwrap());
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
