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
    "exact_owned_cleanup_cycle_quarantines_exact_set_and_releases_focus",
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryAuthorityRow {
    test_function: String,
    state: String,
}

fn parse_history_authority_rows(markdown: &str) -> Vec<HistoryAuthorityRow> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let heading = "### 10.3 History authority regression additions";
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (*line == heading).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        headings.len(),
        1,
        "History authority additions heading must occur exactly once"
    );
    let mut rows = lines[headings[0] + 1..]
        .iter()
        .skip_while(|line| line.trim().is_empty());
    assert_eq!(
        rows.next().map(|line| line.trim()),
        Some("| Test function | Owning slice | State |"),
        "History authority additions must use the exact header"
    );
    assert_eq!(
        rows.next().map(|line| line.trim()),
        Some("| --- | --- | --- |"),
        "History authority additions must use the exact separator"
    );
    let rows = rows
        .take_while(|line| line.trim_start().starts_with('|'))
        .map(|row| {
            let cells = row
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().trim_matches(char::from(96)).to_owned())
                .collect::<Vec<_>>();
            assert_eq!(
                cells.len(),
                3,
                "History authority additions rows must have exactly three columns"
            );
            assert!(
                !cells[0].is_empty(),
                "History authority test function must not be empty"
            );
            assert!(
                matches!(
                    cells[1].as_str(),
                    "Replace" | "Back execution" | "Back failure"
                ),
                "unknown History authority owning slice: {}",
                cells[1]
            );
            assert!(
                matches!(cells[2].as_str(), "planned" | "active"),
                "unknown History authority state: {}",
                cells[2]
            );
            HistoryAuthorityRow {
                test_function: cells[0].clone(),
                state: cells[2].clone(),
            }
        })
        .collect::<Vec<_>>();
    assert!(
        !rows.is_empty(),
        "History authority additions table must not be empty"
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.test_function.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        rows.len(),
        "History authority additions must not have duplicate test functions"
    );

    let active = rows
        .iter()
        .filter(|row| row.state == "active")
        .collect::<Vec<_>>();
    if !active.is_empty() {
        let final_mvp =
            fs::read_to_string(workspace_root().join("docs/mvp-acceptance-journeys.md"))
                .expect("MVP journeys must be readable");
        let final_names = parse_final_mvp_catalogue(&final_mvp);
        let root_profile =
            fs::read_to_string(workspace_root().join("docs/root-push-contract-profile.md"))
                .expect("Root profile must be readable");
        let nested_profile =
            fs::read_to_string(workspace_root().join("docs/nested-push-contract-profile.md"))
                .expect("Nested profile must be readable");
        let existing =
            parse_one_column_table(&root_profile, "### 3.2 Root Authority Regression Catalogue")
                .into_iter()
                .chain(parse_one_column_table(
                    &nested_profile,
                    "### 7.1 Nested Authority Regression Additions",
                ))
                .collect::<BTreeSet<_>>();
        for row in active {
            assert!(
                final_names.contains(&row.test_function),
                "active History authority row {} is absent from the final MVP vocabulary",
                row.test_function
            );
            assert!(
                !existing.contains(&row.test_function),
                "active History authority row {} overlaps Root/Nested",
                row.test_function
            );
        }
    }
    rows
}

#[derive(Debug)]
struct HistoryRequestBoundaryRow {
    case: String,
    cleanup_scope: String,
    continuation: String,
    requested_terminal: String,
}

fn parse_history_request_boundary_rows(markdown: &str) -> Vec<HistoryRequestBoundaryRow> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let heading = "### 9.1 History request and cleanup boundary";
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (*line == heading).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        headings.len(),
        1,
        "History request boundary heading must occur exactly once"
    );

    let section_end = lines[headings[0] + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| headings[0] + 1 + offset);
    let section = &lines[headings[0] + 1..section_end];
    let headers = section
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (line.trim() == "| Case | Cleanup scope | Continuation | requested / terminal |")
                .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        headers.len(),
        1,
        "History request boundary must contain exactly one canonical table header"
    );
    let table_start = headers[0];
    assert_eq!(
        section.get(table_start + 1).map(|line| line.trim()),
        Some("| --- | --- | --- | --- |"),
        "History request boundary must use the exact four-column separator"
    );

    let rows = section[table_start + 2..]
        .iter()
        .take_while(|line| line.trim_start().starts_with('|'))
        .map(|row| {
            let cells = row
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(
                cells.len(),
                4,
                "History request boundary rows must have exactly four columns"
            );
            assert!(
                !cells[0].is_empty(),
                "History request boundary Case is empty"
            );
            HistoryRequestBoundaryRow {
                case: cells[0].clone(),
                cleanup_scope: cells[1].clone(),
                continuation: cells[2].clone(),
                requested_terminal: cells[3].clone(),
            }
        })
        .collect::<Vec<_>>();
    assert!(
        !rows.is_empty(),
        "History request boundary table must not be empty"
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.case.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        rows.len(),
        "History request boundary Case values must be unique"
    );
    rows
}

fn markdown_plain(value: &str) -> String {
    value.replace('`', "")
}

fn unique_history_request_boundary_row<'a>(
    rows: &'a [HistoryRequestBoundaryRow],
    expected_case: &str,
) -> &'a HistoryRequestBoundaryRow {
    let matches = rows
        .iter()
        .filter(|row| markdown_plain(&row.case) == expected_case)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "History request boundary must contain exactly one `{expected_case}` row"
    );
    matches[0]
}

#[test]
fn history_no_target_matrix_rows_are_canonical() {
    let matrix = fs::read_to_string(workspace_root().join("docs/authority-transition-matrix.md"))
        .expect("authority transition matrix must be readable");
    let rows = parse_history_request_boundary_rows(&matrix);

    let resolved = unique_history_request_boundary_row(&rows, "resolved no-target back");
    assert_eq!(
        markdown_plain(&resolved.cleanup_scope),
        "ProvenEmptyProjection 后的 canonical zero manifest/ledger",
        "resolved no-target cleanup scope must remain canonical"
    );
    assert_eq!(
        markdown_plain(&resolved.continuation),
        "rejected-back，独立于 zero count/disposition",
        "resolved no-target continuation must remain canonical"
    );
    assert_eq!(
        markdown_plain(&resolved.requested_terminal),
        "queued 为 None；正常同一 schedule 内不发布 running None，terminal 为 None + BackTargetUnavailable",
        "resolved no-target public state must remain canonical"
    );

    let takeover = unique_history_request_boundary_row(
        &rows,
        "proven-empty 前的 pre-commit authority takeover",
    );
    assert_eq!(
        markdown_plain(&takeover.cleanup_scope),
        "existing scoped authority settlement，不建立 normal no-target ledger",
        "pre-commit authority takeover cleanup scope must remain canonical"
    );
    assert_eq!(
        markdown_plain(&takeover.continuation),
        "authority failure",
        "pre-commit authority takeover continuation must remain canonical"
    );
    assert_eq!(
        markdown_plain(&takeover.requested_terminal),
        "只可公开 authority-specific Running(Recovering) + None；不得发布 normal CleanupPending、HistoryIntentRejected 或 rejected-back continuation，settlement 后 Failed(HostLost/AuthorityLost)",
        "pre-commit authority takeover public state must remain canonical"
    );

    let retry = unique_history_request_boundary_row(
        &rows,
        "typed retryable no-target preparation shortage",
    );
    assert_eq!(
        markdown_plain(&retry.cleanup_scope),
        "no new ledger；原 candidate、reservation 与 typed evidence 完整返还",
        "retryable no-target shortage cleanup scope must remain canonical"
    );
    assert_eq!(
        markdown_plain(&retry.continuation),
        "unchanged",
        "retryable no-target shortage continuation must remain canonical"
    );
    assert_eq!(
        markdown_plain(&retry.requested_terminal),
        "完全 internal 且 public zero-write；不得发布 running phase/trace，只可由 evidence 绑定的 named producer 在其 trigger 推进后重试",
        "retryable no-target shortage public state must remain canonical"
    );
}

#[test]
fn history_authority_parser_requires_a_line_exact_heading() {
    let source = "prefix ### 10.3 History authority regression additions\n\n| Test function | Owning slice | State |\n| --- | --- | --- |\n| active_cycle_enters_authority_settlement | Back failure | planned |\n";
    assert!(
        std::panic::catch_unwind(|| parse_history_authority_rows(source)).is_err(),
        "a substring heading must not authorize History authority parsing"
    );
}

#[test]
fn history_authority_parser_ignores_later_tables_in_the_same_section() {
    let source = "### 10.3 History authority regression additions\n\n| Test function | Owning slice | State |\n| --- | --- | --- |\n| active_cycle_enters_authority_settlement | Back failure | planned |\n\nOther content\n\n| Test function | Owning slice | State |\n| --- | --- | --- |\n| another_authority_case | Back failure | planned |\n";
    let rows = parse_history_authority_rows(source);
    assert_eq!(rows.len(), 1);
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
fn catalogue_matches_root_nested_and_active_history_profiles_and_final_mvp_vocabulary() {
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
    let history_profile = fs::read_to_string(
        workspace_root().join("docs/superpowers/specs/2026-07-20-history-design.md"),
    )
    .expect("History profile must be readable");
    let history_names = parse_history_authority_rows(&history_profile)
        .into_iter()
        .filter_map(|row| (row.state == "active").then_some(row.test_function))
        .collect::<Vec<_>>();
    let mut expected = root_names.clone();
    expected.extend(nested_names.clone());
    expected.extend(history_names.clone());
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
        AUTHORITY_REGRESSION_CATALOGUE[20..23],
        nested_names,
        "the Rust Nested suffix must preserve the profile table order"
    );
    assert_eq!(
        AUTHORITY_REGRESSION_CATALOGUE[23..],
        history_names,
        "the Rust active History suffix must preserve the profile table order"
    );
    assert_eq!(
        AUTHORITY_REGRESSION_CATALOGUE, expected,
        "Rust catalogue must preserve the Root, Nested, and active History profile union order"
    );
    let root_set = root_names.into_iter().collect::<BTreeSet<_>>();
    let nested_set = nested_names.into_iter().collect::<BTreeSet<_>>();
    let history_set = history_names.into_iter().collect::<BTreeSet<_>>();
    let overlap = root_set.intersection(&nested_set).collect::<Vec<_>>();
    assert!(
        overlap.is_empty(),
        "Root and Nested catalogue tables must not overlap: {overlap:?}"
    );
    let root_history_overlap = root_set.intersection(&history_set).collect::<Vec<_>>();
    assert!(
        root_history_overlap.is_empty(),
        "Root and active History catalogue tables must not overlap: {root_history_overlap:?}"
    );
    let nested_history_overlap = nested_set.intersection(&history_set).collect::<Vec<_>>();
    assert!(
        nested_history_overlap.is_empty(),
        "Nested and active History catalogue tables must not overlap: {nested_history_overlap:?}"
    );
    let expected_set = expected.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        expected_set.len(),
        23,
        "active History cumulative catalogue must be 23"
    );
    let catalogue = catalogue_set();
    assert_eq!(catalogue.len(), 23, "Rust catalogue must stay locked at 23");
    assert_eq!(
        catalogue, expected_set,
        "Rust catalogue must match the Root, Nested, and active History profile union"
    );
    let final_names = parse_final_mvp_catalogue(&final_mvp);
    let unknown = catalogue
        .difference(&final_names)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        unknown.is_empty(),
        "active profile union contains non-final-MVP names: {unknown:?}"
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
