use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use toml::{Table, Value};

const BEVY_VERSION: &str = "0.19.1";

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
    let expected_fields = if features.is_empty() {
        &["version", "default-features"][..]
    } else {
        &["version", "default-features", "features"][..]
    };
    assert_keys(table, expected_fields, label);
    assert_eq!(
        table.get("version").and_then(Value::as_str),
        Some(BEVY_VERSION)
    );
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
}

fn assert_official_bevy(manifest: &Table) {
    visit_dependency_tables(manifest, |entries| {
        for (name, value) in entries {
            let Some(dependency) = value.as_table() else {
                continue;
            };
            if is_bevy_family(name, dependency) {
                assert_eq!(
                    dependency.get("version").and_then(Value::as_str),
                    Some(BEVY_VERSION),
                    "dependency `{name}` must use official Bevy {BEVY_VERSION}"
                );
                for field in ["git", "rev", "branch", "path", "registry"] {
                    assert!(
                        !dependency.contains_key(field),
                        "dependency `{name}` must use the default crates.io registry without `{field}`"
                    );
                }
            }
        }
    });
}

fn visit_dependency_tables(manifest: &Table, mut visit: impl FnMut(&Table)) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(entries) = manifest.get(section).and_then(Value::as_table) {
            visit(entries);
        }
    }
    if let Some(selectors) = manifest.get("target").and_then(Value::as_table) {
        for selector in selectors.values().filter_map(Value::as_table) {
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(entries) = selector.get(section).and_then(Value::as_table) {
                    visit(entries);
                }
            }
        }
    }
}

fn is_bevy_family(name: &str, dependency: &Table) -> bool {
    let package = dependency
        .get("package")
        .and_then(Value::as_str)
        .unwrap_or(name);
    package.starts_with("bevy") && !package.starts_with("bevy_ui_router")
}

#[test]
fn ecs_manifest_rejects_target_specific_bevy_branch() {
    let manifest = parse_manifest(
        r#"
[dependencies]
[dev-dependencies]

[target.'cfg(unix)'.dev-dependencies]
bevy_internal = { git = "https://github.com/bevyengine/bevy.git", branch = "target-moving-branch", version = "0.19.1", default-features = false }
"#,
    );

    let result = std::panic::catch_unwind(|| assert_official_bevy(&manifest));
    assert!(
        result.is_err(),
        "target-specific Bevy branches must be rejected"
    );
}

#[test]
fn ecs_manifest_rejects_target_alias_for_bevy_package() {
    let manifest = parse_manifest(
        r#"
[target.'cfg(unix)'.build-dependencies]
render = { package = "bevy_render", git = "https://github.com/bevyengine/bevy.git", branch = "moving", version = "0.19.1" }
"#,
    );
    let result = std::panic::catch_unwind(|| assert_official_bevy(&manifest));
    assert!(
        result.is_err(),
        "target Bevy package aliases must be rejected"
    );
}

#[test]
fn ecs_manifest_ignores_non_dependency_metadata() {
    let manifest = parse_manifest(
        r#"
[package.metadata.bevy_fake]
branch = "not-a-dependency"
"#,
    );
    let result = std::panic::catch_unwind(|| assert_official_bevy(&manifest));
    assert!(result.is_ok(), "package metadata is not a dependency table");
}

#[test]
fn ecs_manifest_rejects_target_alias_with_wrong_bevy_version() {
    let manifest = parse_manifest(
        r#"
[target.'cfg(unix)'.build-dependencies]
render = { package = "bevy_render", version = "0.19.0" }
"#,
    );
    let result = std::panic::catch_unwind(|| assert_official_bevy(&manifest));
    assert!(
        result.is_err(),
        "target Bevy package aliases must use official Bevy {BEVY_VERSION}"
    );
}

#[test]
fn ecs_manifest_locks_the_dependency_boundary() {
    let manifest = parse_manifest(
        &fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap(),
    );
    let features = table(&manifest, "features")
        .iter()
        .map(|(name, value)| (name.clone(), strings(value)))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        features,
        BTreeMap::from([
            ("default".to_owned(), BTreeSet::new()),
            (
                "test-support".to_owned(),
                BTreeSet::from(["bevy_ui_router_core/test-support".to_owned()]),
            )
        ])
    );

    let dependencies = table(&manifest, "dependencies");
    assert_keys(
        dependencies,
        &[
            "bevy_app",
            "bevy_camera",
            "bevy_color",
            "bevy_ecs",
            "bevy_input_focus",
            "bevy_picking",
            "bevy_ui",
            "bevy_ui_router_core",
            "bevy_ui_router_view",
        ],
        "[dependencies]",
    );
    for name in [
        "bevy_app",
        "bevy_camera",
        "bevy_color",
        "bevy_ecs",
        "bevy_input_focus",
        "bevy_picking",
    ] {
        assert_bevy(dependency(dependencies, name), &[], name);
    }
    assert_bevy(
        dependency(dependencies, "bevy_ui"),
        &["bevy_picking"],
        "bevy_ui",
    );
    for (name, path) in [
        ("bevy_ui_router_core", "../bevy_ui_router_core"),
        ("bevy_ui_router_view", "../bevy_ui_router_view"),
    ] {
        let entry = dependency(dependencies, name);
        assert_keys(entry, &["path"], name);
        assert_eq!(entry.get("path").and_then(Value::as_str), Some(path));
    }

    let dev = table(&manifest, "dev-dependencies");
    assert_keys(
        dev,
        &["bevy_ui_router_core", "bevy_internal", "syn", "toml"],
        "[dev-dependencies]",
    );
    let core = dependency(dev, "bevy_ui_router_core");
    assert_keys(core, &["path", "features"], "bevy_ui_router_core");
    assert_eq!(
        core.get("path").and_then(Value::as_str),
        Some("../bevy_ui_router_core")
    );
    assert_eq!(
        core.get("features").map(strings).unwrap(),
        BTreeSet::from(["test-support".to_owned()])
    );
    assert_bevy(
        dependency(dev, "bevy_internal"),
        &["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"],
        "bevy_internal",
    );
    let syn = dependency(dev, "syn");
    assert_keys(syn, &["version", "features"], "syn");
    assert_eq!(syn.get("version").and_then(Value::as_str), Some("2"));
    assert_eq!(
        syn.get("features").map(strings).unwrap(),
        BTreeSet::from(["full".to_owned()])
    );
    assert_eq!(dev.get("toml").and_then(Value::as_str), Some("1.1.2"));
    assert_official_bevy(&manifest);
}
