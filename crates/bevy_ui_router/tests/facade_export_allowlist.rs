#[path = "support/facade_contract_cases.rs"]
mod facade_contract_cases;

use facade_contract_cases::{
    assert_consumer_case_files, assert_devtools_adds_no_public_symbols, assert_facade_exports,
    assert_golden_covers_symbols, assert_owner_source_matches_golden, facade_dir, markdown_table,
    parse_golden, positive_consumer_cases, prepare_consumer_cases, public_facade_source_violations,
    read_workspace, root_symbols, run_positive_consumer,
};

#[test]
fn facade_manifest_is_closed_world() {
    use std::collections::{BTreeMap, BTreeSet};

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

    let dependencies = manifest["dependencies"].as_table().unwrap();
    assert_eq!(
        dependencies
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "bevy_ui_router_bsn",
            "bevy_ui_router_core",
            "bevy_ui_router_ecs",
            "bevy_ui_router_platform",
            "bevy_ui_router_view",
        ])
    );

    let owner: toml::Value =
        toml::from_str(&read_workspace("crates/bevy_ui_router_ecs/Cargo.toml")).unwrap();
    for dependency in ["bevy_app", "bevy_ecs", "bevy_ui"] {
        let facade_dependency = &manifest["dev-dependencies"][dependency];
        let owner_dependency = &owner["dependencies"][dependency];
        for field in ["git", "rev", "version", "default-features"] {
            assert_eq!(
                facade_dependency.get(field),
                owner_dependency.get(field),
                "facade dev-dependency `{dependency}` field `{field}` drifted"
            );
        }
    }
    assert!(manifest["dev-dependencies"].get("syn").is_some());
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
