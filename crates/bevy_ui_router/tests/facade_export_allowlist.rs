#[path = "support/facade_contract_cases.rs"]
mod facade_contract_cases;

use facade_contract_cases::{
    assert_consumer_case_files, assert_devtools_adds_no_public_symbols, assert_facade_exports,
    assert_golden_covers_symbols, assert_owner_source_matches_golden, facade_dir, markdown_table,
    parse_golden, positive_consumer_cases, prepare_consumer_cases, public_facade_source_violations,
    read_workspace, root_symbols, run_positive_consumer,
};

const BEVY_GIT: &str = "https://github.com/shengyusi-SYS/bevy.git";
const BEVY_REV: &str = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7";
const BEVY_VERSION: &str = "0.19.0";
const DEPENDENCY_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

fn assert_table_keys(table: &toml::Table, expected: &[&str], label: &str) {
    use std::collections::BTreeSet;

    let actual = table.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{label} keys must match exactly");
}

fn string_set(value: Option<&toml::Value>) -> std::collections::BTreeSet<&str> {
    value
        .map(|value| {
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect()
        })
        .unwrap_or_default()
}

fn visit_dependency_tables(manifest: &toml::Table, mut visit: impl FnMut(&str, &toml::Table)) {
    for section in DEPENDENCY_SECTIONS {
        if let Some(entries) = manifest.get(section) {
            visit(
                section,
                entries
                    .as_table()
                    .unwrap_or_else(|| panic!("[{section}] must be a table")),
            );
        }
    }
    if let Some(targets) = manifest.get("target") {
        for (selector, selector_value) in targets.as_table().expect("[target] must be a table") {
            let selector_table = selector_value
                .as_table()
                .unwrap_or_else(|| panic!("[target.{selector}] must be a table"));
            for section in DEPENDENCY_SECTIONS {
                if let Some(entries) = selector_table.get(section) {
                    let label = format!("target.{selector}.{section}");
                    visit(
                        &label,
                        entries
                            .as_table()
                            .unwrap_or_else(|| panic!("[{label}] must be a table")),
                    );
                }
            }
        }
    }
}

fn dependency_package_name<'a>(name: &'a str, value: &'a toml::Value) -> &'a str {
    value
        .as_table()
        .and_then(|dependency| dependency.get("package"))
        .and_then(toml::Value::as_str)
        .unwrap_or(name)
}

fn is_bevy_workspace_dependency(name: &str, value: &toml::Value) -> bool {
    let package = dependency_package_name(name, value);
    (package == "bevy" || (package.starts_with("bevy_") && !package.starts_with("bevy_ui_router")))
        || value
            .as_table()
            .and_then(|dependency| dependency.get("git"))
            .and_then(toml::Value::as_str)
            == Some(BEVY_GIT)
}

fn assert_bevy_dependency(location: &str, name: &str, package: &str, dependency: &toml::Table) {
    assert_eq!(
        dependency.get("git").and_then(toml::Value::as_str),
        Some(BEVY_GIT),
        "Bevy dependency `{name}` in [{location}] must use the approved git source"
    );
    assert_eq!(
        dependency.get("rev").and_then(toml::Value::as_str),
        Some(BEVY_REV),
        "Bevy dependency `{name}` in [{location}] must pin the approved revision"
    );
    assert_eq!(
        dependency.get("version").and_then(toml::Value::as_str),
        Some(BEVY_VERSION),
        "Bevy dependency `{name}` in [{location}] must pin the approved version"
    );
    assert_eq!(
        dependency
            .get("default-features")
            .and_then(toml::Value::as_bool),
        Some(false),
        "Bevy dependency `{name}` in [{location}] must disable default features"
    );
    assert_eq!(
        string_set(dependency.get("features")),
        std::collections::BTreeSet::new(),
        "Bevy dependency `{name}` in [{location}] must not enable features"
    );
    assert!(
        !dependency.contains_key("branch"),
        "Bevy dependency `{name}` in [{location}] must not declare branch"
    );
    assert!(
        matches!(package, "bevy_app" | "bevy_ecs" | "bevy_ui"),
        "unexpected Bevy workspace package `{package}` in [{location}]"
    );
    assert_eq!(
        location, "dev-dependencies",
        "Bevy package `{package}` must appear only in [dev-dependencies]"
    );
    assert_eq!(
        name, package,
        "Bevy package `{package}` must use its canonical dependency key"
    );
}

fn assert_facade_dependency_contract(manifest: &toml::Value, owner: &toml::Value) {
    let manifest = manifest.as_table().expect("manifest must be a table");

    visit_dependency_tables(manifest, |location, entries| {
        for (name, value) in entries {
            if !is_bevy_workspace_dependency(name, value) {
                continue;
            }
            let package = dependency_package_name(name, value);
            let dependency = value.as_table().unwrap_or_else(|| {
                panic!("Bevy dependency `{name}` in [{location}] must use table form")
            });
            assert_bevy_dependency(location, name, package, dependency);
        }
    });

    let dependencies = manifest["dependencies"].as_table().unwrap();
    assert_table_keys(
        dependencies,
        &[
            "bevy_ui_router_bsn",
            "bevy_ui_router_core",
            "bevy_ui_router_ecs",
            "bevy_ui_router_platform",
            "bevy_ui_router_view",
        ],
        "[dependencies]",
    );
    for (name, path, optional, default_features) in [
        ("bevy_ui_router_core", "../bevy_ui_router_core", None, None),
        (
            "bevy_ui_router_view",
            "../bevy_ui_router_view",
            Some(true),
            Some(false),
        ),
        (
            "bevy_ui_router_ecs",
            "../bevy_ui_router_ecs",
            Some(true),
            None,
        ),
        (
            "bevy_ui_router_bsn",
            "../bevy_ui_router_bsn",
            Some(true),
            None,
        ),
        (
            "bevy_ui_router_platform",
            "../bevy_ui_router_platform",
            Some(true),
            None,
        ),
    ] {
        let dependency = dependencies[name].as_table().unwrap();
        let mut expected_fields = vec!["path"];
        if optional.is_some() {
            expected_fields.push("optional");
        }
        if default_features.is_some() {
            expected_fields.push("default-features");
        }
        assert_table_keys(dependency, &expected_fields, name);
        assert_eq!(dependency["path"].as_str(), Some(path));
        if let Some(optional) = optional {
            assert_eq!(dependency["optional"].as_bool(), Some(optional));
        }
        if let Some(default_features) = default_features {
            assert_eq!(
                dependency["default-features"].as_bool(),
                Some(default_features)
            );
        }
    }

    let dev_dependencies = manifest["dev-dependencies"].as_table().unwrap();
    assert_table_keys(
        dev_dependencies,
        &["bevy_app", "bevy_ecs", "bevy_ui", "syn", "toml"],
        "[dev-dependencies]",
    );

    for dependency in ["bevy_app", "bevy_ecs", "bevy_ui"] {
        let facade_dependency = dev_dependencies[dependency].as_table().unwrap();
        assert_table_keys(
            facade_dependency,
            &["git", "rev", "version", "default-features"],
            dependency,
        );
        let owner_dependency = &owner["dependencies"][dependency];
        for field in ["git", "rev", "version", "default-features"] {
            assert_eq!(
                facade_dependency.get(field),
                owner_dependency.get(field),
                "facade dev-dependency `{dependency}` field `{field}` drifted"
            );
        }
    }
    let syn = dev_dependencies["syn"].as_table().unwrap();
    assert_table_keys(syn, &["version", "features"], "syn");
    assert_eq!(syn["version"].as_str(), Some("2"));
    assert_eq!(
        string_set(syn.get("features")),
        std::collections::BTreeSet::from(["full", "visit"])
    );
    assert_eq!(dev_dependencies["toml"].as_str(), Some("1.1.2"));

    if let Some(build_dependencies) = manifest.get("build-dependencies") {
        assert!(
            build_dependencies.as_table().unwrap().is_empty(),
            "[build-dependencies] must remain empty"
        );
    }
    if let Some(targets) = manifest.get("target") {
        for (selector, selector_value) in targets.as_table().unwrap() {
            let selector_table = selector_value.as_table().unwrap();
            for section in DEPENDENCY_SECTIONS {
                if let Some(entries) = selector_table.get(section) {
                    assert!(
                        entries.as_table().unwrap().is_empty(),
                        "[target.{selector}.{section}] must remain empty"
                    );
                }
            }
        }
    }
}

fn manifest_contract_fixtures() -> (toml::Value, toml::Value) {
    let facade =
        toml::from_str(&std::fs::read_to_string(facade_dir().join("Cargo.toml")).unwrap()).unwrap();
    let owner = toml::from_str(&read_workspace("crates/bevy_ui_router_ecs/Cargo.toml")).unwrap();
    (facade, owner)
}

fn assert_hostile_manifest_rejected(manifest: &toml::Value, owner: &toml::Value, case: &str) {
    let result = std::panic::catch_unwind(|| assert_facade_dependency_contract(manifest, owner));
    assert!(result.is_err(), "hostile manifest mutation passed: {case}");
}

#[test]
fn facade_manifest_rejects_aliased_bevy_packages() {
    let (mut manifest, owner) = manifest_contract_fixtures();
    let mut alias = manifest["dev-dependencies"]["bevy_app"]
        .as_table()
        .unwrap()
        .clone();
    alias.insert("package".to_owned(), "bevy_app".into());
    manifest["dev-dependencies"]
        .as_table_mut()
        .unwrap()
        .insert("application".to_owned(), alias.into());

    assert_hostile_manifest_rejected(&manifest, &owner, "aliased Bevy package");
}

#[test]
fn facade_manifest_rejects_dependencies_in_every_target_specific_table() {
    for (selector, section) in [
        ("cfg(windows)", "dependencies"),
        ("cfg(unix)", "dev-dependencies"),
        ("cfg(target_os = \"none\")", "build-dependencies"),
    ] {
        let (mut manifest, owner) = manifest_contract_fixtures();
        let mut dependency = manifest["dev-dependencies"]["bevy_ui"]
            .as_table()
            .unwrap()
            .clone();
        dependency.insert("package".to_owned(), "bevy_ui".into());
        let mut entries = toml::Table::new();
        entries.insert("ui_alias".to_owned(), dependency.into());
        let mut selector_table = toml::Table::new();
        selector_table.insert(section.to_owned(), entries.into());
        let targets = manifest
            .as_table_mut()
            .unwrap()
            .entry("target")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap();
        targets.insert(selector.to_owned(), selector_table.into());

        assert_hostile_manifest_rejected(
            &manifest,
            &owner,
            &format!("[{selector}.{section}] dependency"),
        );
    }
}

#[test]
fn facade_manifest_rejects_bevy_source_revision_and_feature_drift() {
    let hostile_fields = [
        (
            "git",
            toml::Value::String("https://example.invalid/bevy.git".into()),
        ),
        ("rev", toml::Value::String("different".into())),
        ("version", toml::Value::String("0.20.0".into())),
        ("default-features", toml::Value::Boolean(true)),
        (
            "features",
            toml::Value::Array(vec![toml::Value::String("unexpected".into())]),
        ),
    ];
    for (field, value) in hostile_fields {
        let (mut manifest, owner) = manifest_contract_fixtures();
        manifest["dev-dependencies"]["bevy_app"]
            .as_table_mut()
            .unwrap()
            .insert(field.to_owned(), value);
        assert_hostile_manifest_rejected(&manifest, &owner, &format!("Bevy `{field}` drift"));
    }
}

#[test]
fn facade_manifest_rejects_unexpected_normal_dev_and_build_dependencies() {
    for section in DEPENDENCY_SECTIONS {
        let (mut manifest, owner) = manifest_contract_fixtures();
        manifest
            .as_table_mut()
            .unwrap()
            .entry(section)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("unexpected".to_owned(), "1".into());
        assert_hostile_manifest_rejected(
            &manifest,
            &owner,
            &format!("unexpected [{section}] entry"),
        );
    }
}

#[test]
fn facade_manifest_is_closed_world() {
    use std::collections::BTreeMap;

    let manifest_text = std::fs::read_to_string(facade_dir().join("Cargo.toml")).unwrap();
    let manifest: toml::Value = toml::from_str(&manifest_text).unwrap();
    assert_eq!(manifest["package"]["autotests"].as_bool(), Some(false));

    let features = manifest["features"].as_table().unwrap();
    let actual_features = features
        .iter()
        .map(|(name, edges)| {
            (
                name.as_str(),
                edges
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|edge| edge.as_str().unwrap())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected_features = BTreeMap::from([
        ("bsn", vec!["ecs", "dep:bevy_ui_router_bsn"]),
        ("default", vec!["view"]),
        ("devtools", vec![]),
        ("ecs", vec!["view", "dep:bevy_ui_router_ecs"]),
        (
            "ecs-test-support",
            vec!["ecs", "bevy_ui_router_ecs/test-support"],
        ),
        ("platform", vec!["dep:bevy_ui_router_platform"]),
        ("test-support", vec!["ecs-test-support"]),
        ("view", vec!["dep:bevy_ui_router_view"]),
    ]);
    assert_eq!(actual_features, expected_features);
    assert!(!features.contains_key("view-test-support"));

    let expected_tests = BTreeMap::from([
        (
            "bsn_facade_features",
            ("tests/bsn_facade_features.rs", vec!["bsn"]),
        ),
        (
            "default_facade_features",
            ("tests/default_facade_features.rs", vec![]),
        ),
        (
            "ecs_facade_features",
            ("tests/ecs_facade_features.rs", vec!["ecs"]),
        ),
        (
            "facade_authority_compile_fail",
            ("tests/facade_authority_compile_fail.rs", vec![]),
        ),
        (
            "facade_export_allowlist",
            ("tests/facade_export_allowlist.rs", vec![]),
        ),
        (
            "public_navigation_authority_audit",
            (
                "tests/public_navigation_authority_audit.rs",
                vec!["test-support"],
            ),
        ),
        (
            "public_navigation_input_isolation",
            (
                "tests/public_navigation_input_isolation.rs",
                vec!["test-support"],
            ),
        ),
        (
            "public_navigation_journeys",
            ("tests/public_navigation_journeys.rs", vec!["ecs"]),
        ),
        (
            "test_support_features",
            ("tests/test_support_features.rs", vec!["test-support"]),
        ),
    ]);
    let actual_tests = manifest["test"]
        .as_array()
        .unwrap()
        .iter()
        .map(|target| {
            let table = target.as_table().unwrap();
            let name = table["name"].as_str().unwrap();
            let path = table["path"].as_str().unwrap();
            let required = table
                .get("required-features")
                .map(|features| {
                    features
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|feature| feature.as_str().unwrap())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (name, (path, required))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_tests, expected_tests);

    let owner: toml::Value =
        toml::from_str(&read_workspace("crates/bevy_ui_router_ecs/Cargo.toml")).unwrap();
    assert_facade_dependency_contract(&manifest, &owner);
}

#[test]
fn root_profile_tables_and_curated_facade_source_are_closed_world() {
    let allowlist = read_workspace("docs/facade-export-allowlist.md");
    let profile = read_workspace("docs/root-push-contract-profile.md");
    let modules = markdown_table(
        &allowlist,
        "## 2. Allowed Modules",
        &["Facade path", "Required feature", "Contract"],
    );
    let symbols = markdown_table(
        &allowlist,
        "## 3. Allowed Type/Value/Trait Symbols",
        &["Symbol", "Module path", "Prelude", "Required feature"],
    );
    let exclusions = markdown_table(
        &profile,
        "### 3.1 Machine-Readable Exclusions",
        &["Kind", "Canonical path or normalized item", "First slice"],
    );
    assert_eq!(modules.len(), 7);
    assert!(symbols.len() > 100);
    assert_eq!(exclusions.len(), 17);

    let source = std::fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
    assert_eq!(
        public_facade_source_violations(&source),
        Vec::<String>::new()
    );
    assert_devtools_adds_no_public_symbols(&source);

    let symbols = root_symbols(&symbols, &exclusions);
    let golden = parse_golden(
        &read_workspace("docs/facade-associated-api-golden.txt"),
        &exclusions,
    );
    assert_golden_covers_symbols(&symbols, &golden);
    assert_owner_source_matches_golden(&symbols, &golden);
    assert_facade_exports(&source, &symbols);
}

#[test]
fn isolated_positive_consumers_cover_the_root_feature_matrix() {
    let allowlist = read_workspace("docs/facade-export-allowlist.md");
    let profile = read_workspace("docs/root-push-contract-profile.md");
    let modules = markdown_table(
        &allowlist,
        "## 2. Allowed Modules",
        &["Facade path", "Required feature", "Contract"],
    );
    let symbols = markdown_table(
        &allowlist,
        "## 3. Allowed Type/Value/Trait Symbols",
        &["Symbol", "Module path", "Prelude", "Required feature"],
    );
    let exclusions = markdown_table(
        &profile,
        "### 3.1 Machine-Readable Exclusions",
        &["Kind", "Canonical path or normalized item", "First slice"],
    );
    let symbols = root_symbols(&symbols, &exclusions);
    let golden = parse_golden(
        &read_workspace("docs/facade-associated-api-golden.txt"),
        &exclusions,
    );
    let cases = positive_consumer_cases(&modules, &symbols, &golden);
    assert_eq!(cases.len(), 8);
    prepare_consumer_cases("pass");
    for case in &cases {
        let tree = run_positive_consumer(case);
        if case.feature_set == "default-enabled" {
            for package in [
                "bevy_ui_router ",
                "bevy_ui_router_core ",
                "bevy_ui_router_view ",
            ] {
                assert!(tree.contains(package), "default graph misses {package}");
            }
            for line in tree.lines() {
                let package = line
                    .trim_start_matches(|character: char| {
                        character.is_whitespace() || matches!(character, '│' | '├' | '└' | '─')
                    })
                    .split_whitespace()
                    .next()
                    .unwrap_or_default();
                assert!(
                    !(package.starts_with("bevy_") && !package.starts_with("bevy_ui_router"))
                        && !package.starts_with("bevy-"),
                    "default graph unexpectedly contains Bevy-family runtime package `{package}`"
                );
            }
        }
    }
    assert_consumer_case_files("pass", &cases);
}
