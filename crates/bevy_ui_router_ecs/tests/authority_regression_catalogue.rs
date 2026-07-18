use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use syn::{Attribute, Item, ItemFn, ItemMod, Meta};

pub const AUTHORITY_REGRESSION_CATALOGUE: &[&str] = &[
    "cleanup_observer_reentry_supersedes_attempt",
    "cleanup_receipt_replay_is_rejected",
    "commit_gate_switch_is_same_frame_atomic",
    "exact_owned_cleanup_cycle_is_world_zero_and_retains_authority",
    "foreign_component_quarantine_avoids_hooks",
    "foreign_descendant_quarantines_owned_ancestor",
    "host_children_change_only_top_slot_membership",
    "host_grant_observer_drift_rejects_after_mutation",
    "host_ui_topology_requires_all_node_ancestry",
    "pending_cycle_rejects_without_touching_active",
    "persistent_admission_observer_latches_retry",
    "plan_relationship_errors_are_world_zero",
    "post_commit_cleanup_drift_supersedes_attempt",
    "post_commit_cleanup_waits_for_receipt",
    "protected_host_is_never_despawned",
    "protected_parent_quarantine_preserves_relationship",
    "solid_rect_required_components_match_locked_bevy",
    "solid_rect_runs_real_layout_and_picking_pipeline",
    "stale_report_identity_is_zero_write",
    "structural_outputs_have_locked_manifest_and_topology",
    "mount_grant_observer_drift_rejects_after_mutation",
    "mount_token_replay_is_zero_write",
    "slot_lifecycle_has_no_orphans",
];

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    package_root()
        .parent()
        .and_then(Path::parent)
        .expect("ECS package must be nested under workspace/crates")
        .to_owned()
}

fn parse_rust(path: impl AsRef<Path>) -> syn::File {
    let path = path.as_ref();
    syn::parse_file(&fs::read_to_string(path).expect("Rust source must be readable"))
        .unwrap_or_else(|error| panic!("{} must parse: {error}", path.display()))
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn is_exact_cfg_test(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    matches!(&attr.meta, Meta::List(list) if list.tokens.to_string() == "test")
}

fn catalogue_set() -> BTreeSet<String> {
    assert!(
        !AUTHORITY_REGRESSION_CATALOGUE.is_empty(),
        "authority regression catalogue must not be empty"
    );
    let names = AUTHORITY_REGRESSION_CATALOGUE
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names.len(),
        AUTHORITY_REGRESSION_CATALOGUE.len(),
        "authority regression catalogue must not contain duplicates"
    );
    names
}

fn parse_one_column_table(markdown: &str, heading: &str) -> Vec<String> {
    let section = markdown
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing catalogue heading `{heading}`"))
        .1
        .split("\n## ")
        .next()
        .expect("catalogue section must have content");
    let mut rows = section.lines().filter(|line| line.trim().starts_with('|'));
    assert_eq!(
        rows.next().map(str::trim),
        Some("| Test function |"),
        "catalogue must use the exact one-column header"
    );
    assert_eq!(
        rows.next().map(str::trim),
        Some("| --- |"),
        "catalogue must use the exact one-column separator"
    );
    let mut ordered = Vec::new();
    for row in rows {
        let cells = row
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(
            cells.len(),
            1,
            "catalogue rows must have exactly one column"
        );
        let cell = cells[0];
        assert!(
            cell.starts_with('`') && cell.ends_with('`') && cell.len() > 2,
            "catalogue entries must be nonempty code identifiers"
        );
        ordered.push(cell.trim_matches('`').to_owned());
    }
    assert!(!ordered.is_empty(), "catalogue table must not be empty");
    let names = ordered.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(names.len(), ordered.len(), "catalogue table has duplicates");
    ordered
}

fn parse_final_mvp_catalogue(markdown: &str) -> BTreeSet<String> {
    let marker = "MVP gate必须精确等于以下完整集合：";
    let tail = markdown
        .split_once(marker)
        .expect("MVP catalogue marker must exist")
        .1;
    let block = tail
        .split_once("```text")
        .expect("MVP catalogue code block must start")
        .1
        .split_once("```")
        .expect("MVP catalogue code block must end")
        .0;
    let ordered = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!ordered.is_empty(), "final MVP catalogue must not be empty");
    let names = ordered.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        names.len(),
        ordered.len(),
        "final MVP catalogue has duplicates"
    );
    names
}

fn direct_tests(file: &syn::File) -> Vec<&ItemFn> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if has_attr(&function.attrs, "test") => Some(function),
            _ => None,
        })
        .collect()
}

#[test]
fn catalogue_matches_root_and_nested_profiles_and_final_mvp_vocabulary() {
    let root_profile =
        fs::read_to_string(workspace_root().join("docs/root-push-contract-profile.md"))
            .expect("Root profile must be readable");
    let final_mvp = fs::read_to_string(workspace_root().join("docs/mvp-acceptance-journeys.md"))
        .expect("MVP journeys must be readable");
    let root_names =
        parse_one_column_table(&root_profile, "### 3.2 Root Authority Regression Catalogue");
    let nested_profile =
        fs::read_to_string(workspace_root().join("docs/nested-push-contract-profile.md"))
            .expect("Nested profile must be readable");
    let nested_names = parse_one_column_table(
        &nested_profile,
        "### 7.1 Nested Authority Regression Additions",
    );
    assert_eq!(
        root_names.len(),
        20,
        "Root catalogue must stay locked at 20"
    );
    assert_eq!(nested_names.len(), 3, "Nested must add exactly three names");
    assert_eq!(
        AUTHORITY_REGRESSION_CATALOGUE[..20],
        root_names,
        "the Rust Root prefix must preserve the profile table order"
    );
    assert_eq!(
        AUTHORITY_REGRESSION_CATALOGUE[20..],
        nested_names,
        "the Rust Nested suffix must preserve the profile table order"
    );
    let root_set = root_names.into_iter().collect::<BTreeSet<_>>();
    let nested_set = nested_names.into_iter().collect::<BTreeSet<_>>();
    let overlap = root_set.intersection(&nested_set).collect::<Vec<_>>();
    assert!(
        overlap.is_empty(),
        "Root and Nested catalogue tables must not overlap: {overlap:?}"
    );
    let mut expected = root_set;
    expected.extend(nested_set);
    assert_eq!(expected.len(), 23, "Nested cumulative catalogue must be 23");
    let catalogue = catalogue_set();
    assert_eq!(catalogue.len(), 23, "Rust catalogue must stay locked at 23");
    assert_eq!(
        catalogue, expected,
        "Rust catalogue must match the Root and Nested profile union"
    );
    let final_names = parse_final_mvp_catalogue(&final_mvp);
    let unknown = catalogue
        .difference(&final_names)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        unknown.is_empty(),
        "Nested catalogue contains non-final-MVP names: {unknown:?}"
    );
}

#[test]
fn crate_root_registers_exact_cfg_test_authority_module() {
    let root = parse_rust(package_root().join("src/lib.rs"));
    let modules = root
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Mod(module) if module.ident == "authority_regression_tests" => Some(module),
            _ => None,
        })
        .collect::<Vec<&ItemMod>>();
    assert_eq!(modules.len(), 1, "crate root must declare the module once");
    let module = modules[0];
    assert!(
        module.content.is_none(),
        "authority module must use the fixed file"
    );
    assert_eq!(
        module
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"))
            .count(),
        1,
        "authority module must have exactly one cfg"
    );
    assert!(
        module.attrs.iter().any(|attr| is_exact_cfg_test(attr)),
        "authority module must be declared with exact #[cfg(test)]"
    );
    assert!(
        !module
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("cfg_attr")),
        "authority module must not have conditional registration beyond cfg(test)"
    );
}

#[test]
fn authority_source_tests_equal_the_nonempty_catalogue() {
    let source = parse_rust(package_root().join("src/authority_regression_tests.rs"));
    let tests = direct_tests(&source);
    assert!(
        !tests.is_empty(),
        "authority source must contain direct tests"
    );
    for test in &tests {
        assert_eq!(
            test.attrs
                .iter()
                .filter(|attr| attr.path().is_ident("test"))
                .count(),
            1,
            "{} must have one direct #[test]",
            test.sig.ident
        );
        assert!(
            !has_attr(&test.attrs, "ignore"),
            "{} must not be ignored",
            test.sig.ident
        );
        assert!(
            !test
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr")),
            "{} must not have extra cfg filtering",
            test.sig.ident
        );
    }
    let ordered = tests
        .iter()
        .map(|test| test.sig.ident.to_string())
        .collect::<Vec<_>>();
    let actual = ordered.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        actual.len(),
        ordered.len(),
        "authority source has duplicate tests"
    );
    assert_eq!(
        actual,
        catalogue_set(),
        "authority source and catalogue differ"
    );
}
