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

fn enum_variant_names(source: &str, enum_name: &str) -> Vec<String> {
    syn::parse_file(source)
        .expect("embedded Rust source must parse")
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Enum(item) if item.ident == enum_name => Some(
                item.variants
                    .into_iter()
                    .map(|variant| variant.ident.to_string())
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing enum `{enum_name}`"))
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

fn braced_body(source: &str) -> &str {
    let open = source
        .find('{')
        .expect("static audit marker must be followed by a braced body");
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("static audit body must close")
}

fn body_after_marker<'a>(source: &'a str, marker: &str) -> &'a str {
    let tail = source
        .split_once(marker)
        .unwrap_or_else(|| panic!("missing static audit marker: {marker}"))
        .1;
    braced_body(tail)
}

fn match_arm_body_after_marker<'a>(source: &'a str, marker: &str) -> &'a str {
    let tail = source
        .split_once(marker)
        .unwrap_or_else(|| panic!("missing static match arm: {marker}"))
        .1;
    let arm = tail
        .split_once("=>")
        .unwrap_or_else(|| panic!("static match arm has no arrow: {marker}"))
        .1;
    braced_body(arm)
}

fn callback_after_receiver<'a>(source: &'a str, receiver: &str) -> &'a str {
    let receiver_tail = source
        .split_once(receiver)
        .expect("the helper must call the trusted Core receiver")
        .1;
    let closure_tail = receiver_tail
        .split_once("move ||")
        .expect("the trusted Core receiver must receive one move closure")
        .1
        .trim_start();
    if closure_tail.starts_with('{') {
        return braced_body(closure_tail);
    }

    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    for (offset, character) in closure_tail.char_indices() {
        match character {
            '(' => parentheses += 1,
            ')' if parentheses > 0 => parentheses -= 1,
            ')' if brackets == 0 => return closure_tail[..offset].trim(),
            '[' => brackets += 1,
            ']' => brackets -= 1,
            ',' if parentheses == 0 && brackets == 0 => {
                return closure_tail[..offset].trim();
            }
            _ => {}
        }
    }
    panic!("the trusted Core receiver callback must close")
}

fn closure_body_after_receiver<'a>(
    source: &'a str,
    receiver: &str,
    closure_marker: &str,
) -> &'a str {
    let receiver_tail = source
        .split_once(receiver)
        .unwrap_or_else(|| panic!("missing audited receiver: {receiver}"))
        .1;
    let closure_tail = receiver_tail
        .split_once(closure_marker)
        .unwrap_or_else(|| panic!("audited receiver must receive {closure_marker}"))
        .1
        .trim_start();
    if closure_tail.starts_with('{') {
        return braced_body(closure_tail);
    }

    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    for (offset, character) in closure_tail.char_indices() {
        match character {
            '(' => parentheses += 1,
            ')' if parentheses > 0 => parentheses -= 1,
            ')' if brackets == 0 => return closure_tail[..offset].trim(),
            '[' => brackets += 1,
            ']' => brackets -= 1,
            ',' if parentheses == 0 && brackets == 0 => {
                return closure_tail[..offset].trim();
            }
            _ => {}
        }
    }
    panic!("audited receiver callback must close: {receiver}")
}

fn assert_stale_reservation_drops_before_evidence_seal(helper: &str, arm_marker: &str) {
    let arm = match_arm_body_after_marker(helper, arm_marker);
    assert_eq!(
        arm.match_indices("drop(reservation);").count(),
        1,
        "{arm_marker} must drop the stale bound reservation exactly once"
    );
    assert_eq!(
        arm.match_indices("prepared_retirement.into_evidence_after_exact_receiver()")
            .count(),
        1,
        "{arm_marker} must seal the prepared retirement evidence exactly once"
    );
    let drop_index = arm
        .find("drop(reservation);")
        .expect("the stale reservation drop was counted");
    let seal_index = arm
        .find("prepared_retirement.into_evidence_after_exact_receiver()")
        .expect("the prepared evidence conversion was counted");
    assert!(
        drop_index < seal_index,
        "{arm_marker} must drop the stale owner before sealing retirement evidence"
    );
}

fn parenthesized_call_arguments<'a>(source: &'a str, callee: &str) -> Vec<&'a str> {
    let marker = format!("{callee}(");
    let mut calls = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = source[cursor..].find(&marker) {
        let open = cursor + relative_start + marker.len() - 1;
        let mut depth = 1usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut close = None;
        for (relative_offset, character) in source[open + 1..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else {
                    match character {
                        '\\' => escaped = true,
                        '"' => in_string = false,
                        _ => {}
                    }
                }
                continue;
            }
            match character {
                '"' => in_string = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + 1 + relative_offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close.unwrap_or_else(|| panic!("static call must close: {callee}"));
        calls.push(&source[open + 1..close]);
        cursor = close + 1;
    }
    calls
}

fn compact_static_source(source: &str) -> String {
    let characters = source.chars().collect::<Vec<_>>();
    let mut compact = String::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0usize;
    while index < characters.len() {
        let character = characters[index];
        if in_string {
            compact.push(character);
            if escaped {
                escaped = false;
            } else {
                match character {
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
            }
            index += 1;
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                compact.push(character);
            }
            ',' => {
                let mut next = index + 1;
                while next < characters.len() && characters[next].is_whitespace() {
                    next += 1;
                }
                if next >= characters.len() || characters[next] != ')' {
                    compact.push(character);
                }
            }
            character if character.is_whitespace() => {}
            _ => compact.push(character),
        }
        index += 1;
    }
    compact
}

#[test]
fn back_target_callback_tail_is_infallible_and_query_free() {
    let navigation = include_str!("../src/navigation.rs");
    let cleanup = include_str!("../src/cleanup.rs");
    let helper_marker = "pub(crate) fn resume_bound_back_target(";
    let receiver = ".resume_back_target_with_retirement(";

    assert_eq!(
        navigation.match_indices(receiver).count(),
        1,
        "the Core target receiver must have exactly one ECS production callsite"
    );
    let helper = body_after_marker(navigation, helper_marker);
    assert_eq!(
        helper.match_indices(receiver).count(),
        1,
        "the unique Core target receiver callsite must stay inside the exact ECS helper"
    );

    let callback = callback_after_receiver(helper, receiver);
    let normalized_callback = callback.split_whitespace().collect::<String>();
    assert_eq!(
        callback.match_indices("guard.commit()").count(),
        1,
        "the callback must consume the lifetime-bound guard exactly once"
    );
    assert!(
        matches!(
            normalized_callback.as_str(),
            "guard.commit()" | "guard.commit();"
        ),
        "the callback may contain only the single infallible guard commit; got {callback:?}"
    );
    for forbidden in [
        "bound",
        "prepared",
        "world",
        ".query",
        ".resource",
        ".entry(",
        ".get(",
        ".get_mut(",
        "[",
        "Vec::",
        "Box::",
        "String::",
        "vec!",
        ".collect",
        ".reserve",
        ".try_reserve",
        ".sort",
        ".validate",
        ".expect",
        ".unwrap",
        "panic!",
        "unreachable!",
        "unsafe",
        ".insert",
        ".remove",
        "despawn",
        "entity_mut",
        "commands",
        ".trigger",
        ".observe",
        "add_child",
        "replace_children",
        "set_parent",
    ] {
        assert!(
            !callback.contains(forbidden),
            "the trusted target retirement callback contains forbidden capture or work: {forbidden}"
        );
    }

    assert_stale_reservation_drops_before_evidence_seal(
        helper,
        "BackTargetCoreResumeOutcome::Resumed(start)",
    );
    assert_stale_reservation_drops_before_evidence_seal(
        helper,
        "BackTargetCoreResumeOutcome::Permanent(owner)",
    );

    let commit_marker = "pub(crate) fn commit(self)";
    assert_eq!(
        cleanup.match_indices(commit_marker).count(),
        1,
        "the exact capacity guard must expose one audited commit implementation"
    );
    let commit = body_after_marker(cleanup, commit_marker);
    assert!(
        commit.contains("match self.transition"),
        "guard commit must only dispatch its already-prepared operation-specific transition"
    );
    assert!(
        !commit.contains("self.owner"),
        "guard commit must not inspect the borrowed owner after preflight"
    );

    let outstanding_retire =
        match_arm_body_after_marker(commit, "BackCapacityCommitTransition::OutstandingRetire");
    assert_eq!(
        outstanding_retire.match_indices("live.remove()").count(),
        1,
        "OutstandingRetire must consume its already-held OccupiedEntry exactly once"
    );
    assert_eq!(
        outstanding_retire
            .match_indices("cell.state = BackCapacityCellState::Free;")
            .count(),
        1,
        "OutstandingRetire must return the exact cell lifecycle to Free"
    );

    let reusable = match_arm_body_after_marker(
        outstanding_retire,
        "PreparedRetiredCell::ReusableFree { next_generation }",
    );
    assert_eq!(
        reusable
            .match_indices("cell.generation = next_generation;")
            .count(),
        1,
        "ReusableFree must install its precomputed successor generation"
    );
    assert_eq!(
        reusable
            .match_indices("cell.reuse = BackCapacityReuse::Reusable;")
            .count(),
        1,
        "ReusableFree must keep the exact cell reusable"
    );

    let permanently_retired = match_arm_body_after_marker(
        outstanding_retire,
        "PreparedRetiredCell::PermanentlyRetired { generation }",
    );
    assert_eq!(
        permanently_retired
            .match_indices("cell.generation = generation;")
            .count(),
        1,
        "PermanentlyRetired must preserve its final generation"
    );
    assert_eq!(
        permanently_retired
            .match_indices("cell.reuse = BackCapacityReuse::PermanentlyRetired;")
            .count(),
        1,
        "PermanentlyRetired must install the orthogonal non-reusable marker"
    );

    for forbidden in [
        "self.live",
        "self.cells",
        "self.outstanding",
        ".entry(",
        ".get(",
        ".get_mut(",
        "[",
        "Vec::",
        "vec!",
        "Box::",
        "String::",
        ".collect",
        ".reserve",
        ".try_reserve",
        ".validate",
        ".expect",
        ".unwrap",
        "panic!",
        "unreachable!",
        "unsafe",
        ".query",
        ".resource",
    ] {
        assert!(
            !commit.contains(forbidden),
            "the exact capacity guard commit contains forbidden lookup, index, allocation, validation or failure work: {forbidden}"
        );
    }

    for forbidden in [".entry(", ".get(", ".get_mut(", "[", ".expect", ".unwrap"] {
        assert!(
            !outstanding_retire.contains(forbidden),
            "OutstandingRetire must use only its held live entry, exact cell and prepared successor: {forbidden}"
        );
    }
}

#[test]
fn back_no_target_authority_callbacks_only_publish_prepared_state() {
    let navigation = include_str!("../src/navigation.rs");
    let core_integration = include_str!("../../bevy_ui_router_core/src/integration.rs");
    let tests_start = navigation
        .find("pub(crate) mod tests {")
        .expect("navigation source retains its crate-private test module");
    let production_navigation = &navigation[..tests_start];

    let accept_receiver = ".accept_back_no_target_authority_lost_with_retirement(";
    let begin_receiver = ".begin_back_no_target_authority_settlement(";
    let completion_receiver = ".complete_back_no_target_authority_cleanup(";
    assert_eq!(
        production_navigation.match_indices(accept_receiver).count(),
        1,
        "ECS must keep one audited no-target authority accept callsite"
    );
    assert_eq!(
        production_navigation.match_indices(begin_receiver).count(),
        1,
        "ECS must keep one audited no-target authority begin callsite"
    );
    assert_eq!(
        production_navigation
            .match_indices(completion_receiver)
            .count(),
        1,
        "ECS must keep one audited no-target authority completion callsite"
    );

    let accept = body_after_marker(
        production_navigation,
        "pub(crate) fn classify_back_no_target(",
    );
    let accept_callback = closure_body_after_receiver(accept, accept_receiver, "move ||");
    assert_eq!(
        compact_static_source(accept_callback),
        "guard.commit()",
        "the no-target authority accept callback may only consume its prepared guard"
    );

    let begin = body_after_marker(
        production_navigation,
        "pub(crate) fn dispatch_back_no_target_authority_settlement(",
    );
    let begin_callback = closure_body_after_receiver(begin, begin_receiver, "|batch|");
    assert_eq!(
        compact_static_source(begin_callback),
        "*batch_slot=Some(batch)",
        "the ECS authority begin callback may only install the prepared batch"
    );

    let completion = body_after_marker(
        production_navigation,
        "fn drive_back_no_target_authority_cleanup(",
    );
    let completion_callback = closure_body_after_receiver(completion, completion_receiver, "||");
    assert_eq!(
        compact_static_source(completion_callback),
        "*pending=None",
        "the ECS authority completion callback may only clear the prepared holder"
    );

    let core_begin = body_after_marker(
        core_integration,
        "pub fn begin_back_no_target_authority_settlement<F>(",
    );
    assert_eq!(
        core_begin.match_indices(begin_receiver).count(),
        1,
        "Core integration must keep one audited inner authority begin callsite"
    );
    let core_begin_callback = closure_body_after_receiver(core_begin, begin_receiver, "|batch|");
    assert_eq!(
        compact_static_source(core_begin_callback),
        "install(BackNoTargetAuthorityCleanupBatchOwner{inner:batch});",
        "the Core integration callback may only install the prepared batch owner"
    );
    let begin_receiver_index = core_begin
        .find(begin_receiver)
        .expect("the inner begin receiver was counted");
    let settlement_started_marker = "let settlement_started";
    assert_eq!(
        core_begin.match_indices(settlement_started_marker).count(),
        1,
        "Core integration must prepare exactly one SettlementStarted event"
    );
    let settlement_started_index = core_begin
        .find(settlement_started_marker)
        .expect("the prepared SettlementStarted event was counted");
    assert!(
        settlement_started_index < begin_receiver_index,
        "SettlementStarted must be prepared before entering the inner begin receiver"
    );
    let prepared_event = &core_begin[settlement_started_index..begin_receiver_index];
    assert_eq!(
        prepared_event
            .match_indices("HostTraceEvent::project(")
            .count(),
        1,
        "the settlement_started binding must prepare exactly one host-trace event"
    );
    assert_eq!(
        prepared_event
            .match_indices("HostTraceEventKind::SettlementStarted")
            .count(),
        1,
        "the event prepared before begin must be SettlementStarted"
    );
    let begun =
        match_arm_body_after_marker(core_begin, "CoreBackNoTargetAuthorityBeginOutcome::Begun");
    assert_eq!(
        begun
            .match_indices("host_trace.push(settlement_started);")
            .count(),
        1,
        "the outer Begun arm must publish the prepared SettlementStarted event exactly once"
    );
    assert!(
        !begun.contains("HostTraceEvent::project("),
        "the outer Begun arm must not rebuild the prepared event"
    );

    for (callback_name, callback) in [
        ("ECS accept", accept_callback),
        ("ECS begin", begin_callback),
        ("ECS completion", completion_callback),
        ("Core integration begin", core_begin_callback),
    ] {
        for forbidden in [
            "world",
            ".query",
            ".resource",
            ".get(",
            ".get_mut(",
            "Vec::",
            "Box::",
            "String::",
            "vec!",
            ".collect",
            ".reserve",
            ".try_reserve",
            ".sort",
            ".validate",
            ".expect",
            ".unwrap",
            "panic!",
            "unreachable!",
            "unsafe",
            ".push",
            ".insert",
            ".remove",
            "despawn",
            "entity_mut",
            "commands",
            ".trigger",
            ".observe",
            "add_child",
            "replace_children",
            "set_parent",
        ] {
            assert!(
                !callback.contains(forbidden),
                "{callback_name} callback contains forbidden query, allocation, validation, failure or hookful work: {forbidden}"
            );
        }
    }
}

#[test]
fn cleanup_ledger_fault_seams_only_change_real_predicate_inputs() {
    let cleanup = include_str!("../src/cleanup.rs");
    let back_capacity_impl = cleanup
        .split_once("impl BackCapacityIssuer {")
        .expect("cleanup source retains the BackCapacityIssuer implementation")
        .1;
    let capacity_reserve = body_after_marker(back_capacity_impl, "pub(crate) fn reserve(");
    let cleanup_ledger_impl = cleanup
        .split_once("impl CleanupLedger {")
        .expect("cleanup source retains the CleanupLedger implementation")
        .1;
    let reserve = body_after_marker(cleanup_ledger_impl, "fn reserve_with_domain_counter(");
    let prepare_physical = body_after_marker(cleanup_ledger_impl, "fn prepare_physical(");
    let inject_unfinalized = body_after_marker(cleanup, "fn inject_unfinalized_boundary_drift(");
    let ledger_domain_counter_input = body_after_marker(cleanup, "fn ledger_domain_counter_input(");

    assert!(
        !reserve.contains("unfinalized_ledger_fault_injected()"),
        "unfinalized cleanup facts must be classified only from the manifest input"
    );
    assert_eq!(
        reserve
            .match_indices("!manifest.boundary_drifts.is_empty()")
            .count(),
        1,
        "cleanup reserve must use the real manifest boundary-drift predicate exactly once"
    );
    assert_eq!(
        reserve
            .match_indices("inject_unfinalized_boundary_drift(&mut manifest);")
            .count(),
        1,
        "cleanup reserve must apply its test seam to the manifest input exactly once"
    );
    let inject_index = reserve
        .find("inject_unfinalized_boundary_drift(&mut manifest);")
        .expect("the manifest-input seam call was counted");
    let unfinalized_predicate_index = reserve
        .find("if !manifest.boundary_drifts.is_empty()")
        .expect("the real unfinalized predicate was counted");
    assert!(
        inject_index < unfinalized_predicate_index,
        "the seam may only alter manifest input before the real unfinalized predicate"
    );
    let unfinalized_branch = body_after_marker(reserve, "if !manifest.boundary_drifts.is_empty()");
    assert_eq!(
        reserve
            .match_indices("CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts")
            .count(),
        1,
        "cleanup reserve must contain one UnfinalizedBoundaryFacts classification"
    );
    assert_eq!(
        unfinalized_branch
            .match_indices("CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts")
            .count(),
        1,
        "UnfinalizedBoundaryFacts must be derived only inside the real boundary-drift branch"
    );

    assert_eq!(
        inject_unfinalized
            .match_indices("FAIL_NEXT_UNFINALIZED_LEDGER.replace(false)")
            .count(),
        1,
        "the unfinalized seam helper must consume its one-shot input exactly once"
    );
    for required in [
        "manifest.obligations.iter().find_map(|obligation|",
        "RegistryOwner::Committed(EntityOwner::FrameShell(frame)) = obligation.claim.owner()",
        "obligation.expected_parent",
        "removed_child: claim.entity()",
        "expected: FailedBoundaryExpectation::ExactClaim(claim)",
        "manifest.boundary_drifts.push(FailedBoundaryDrift {",
    ] {
        assert!(
            inject_unfinalized.contains(required),
            "the unfinalized seam helper must derive drift from one exact committed frame-shell obligation: {required}"
        );
    }
    assert_eq!(
        inject_unfinalized
            .match_indices("manifest.boundary_drifts.push(FailedBoundaryDrift {")
            .count(),
        1,
        "the unfinalized seam helper must append one predicate input"
    );
    for forbidden in [
        "CleanupLedgerReserveError",
        "UnfinalizedBoundaryFacts",
        "DomainExhausted",
        "Result<",
        "Err(",
        "return Err",
        "Entity::",
        "from_raw(",
        "from_bits(",
        "PLACEHOLDER",
        ".unwrap_or(",
        ".unwrap_or_else(",
        ".unwrap_or_default(",
        ".unwrap(",
        ".expect(",
        "world",
        ".query(",
        ".query::<",
        ".resource(",
        ".resource::<",
        ".get(",
        ".get_mut(",
        "allocate_ledger_domain",
        "NEXT_LEDGER_DOMAIN",
        "observe_ledger_domain_allocation",
        ".insert(",
        ".remove(",
        ".remove::<",
        "despawn(",
    ] {
        assert!(
            !inject_unfinalized.contains(forbidden),
            "the unfinalized seam helper must not fabricate identity or classify the result: {forbidden}"
        );
    }

    assert_eq!(
        compact_static_source(ledger_domain_counter_input),
        "ifledger_domain_fault_injected(){&EXHAUSTED_LEDGER_DOMAIN_COUNTER}else{counter}",
        "the ledger-domain seam helper may only select the exhausted or supplied counter input"
    );
    for forbidden in [
        "DomainExhausted",
        "CleanupLedgerReserveError",
        "BackCapacityReserveError",
        "Result<",
        "Err(",
        "allocate_ledger_domain",
        "NEXT_LEDGER_DOMAIN",
        "LEDGER_DOMAIN_ALLOCATIONS",
        "observe_ledger_domain_allocation",
        ".fetch_",
        ".store(",
        ".swap(",
        ".compare_exchange",
        "Ordering::",
    ] {
        assert!(
            !ledger_domain_counter_input.contains(forbidden),
            "the ledger-domain seam helper must not allocate, advance, observe or classify: {forbidden}"
        );
    }

    for (owner, allocation) in [
        ("capacity reserve", capacity_reserve),
        ("cleanup ledger prepare", prepare_physical),
    ] {
        assert!(
            !allocation.contains("ledger_domain_fault_injected()"),
            "{owner} must not inject DomainExhausted as a classification"
        );
        assert_eq!(
            allocation.match_indices("allocate_ledger_domain(").count(),
            1,
            "{owner} must derive its ledger domain from one real allocation"
        );
        assert_eq!(
            allocation
                .match_indices("BackCapacityReserveError::DomainExhausted")
                .count()
                + allocation
                    .match_indices("CleanupLedgerReserveErrorKind::DomainExhausted")
                    .count(),
            1,
            "{owner} must derive DomainExhausted only from the failed real allocation"
        );
        let allocator_index = allocation
            .find("allocate_ledger_domain(")
            .expect("the real allocator call was counted");
        let binding_index = allocation
            .find("let Some(ledger_domain)")
            .unwrap_or_else(|| panic!("{owner} must use a let-else allocation binding"));
        assert!(
            binding_index < allocator_index,
            "{owner} must classify only after its real allocation binding"
        );
        let binding_prefix = compact_static_source(&allocation[binding_index..allocator_index]);
        assert_eq!(
            binding_prefix, "letSome(ledger_domain)=",
            "{owner} must bind the real allocator result directly"
        );
        let exhausted_branch = body_after_marker(&allocation[allocator_index..], "else");
        assert_eq!(
            exhausted_branch
                .match_indices("BackCapacityReserveError::DomainExhausted")
                .count()
                + exhausted_branch
                    .match_indices("CleanupLedgerReserveErrorKind::DomainExhausted")
                    .count(),
            1,
            "{owner} must return its unique DomainExhausted only from the allocator let-else"
        );
    }
    let compact_capacity_reserve = compact_static_source(capacity_reserve);
    assert!(
        compact_capacity_reserve
            .contains("allocate_ledger_domain(ledger_domain_counter_input(&NEXT_LEDGER_DOMAIN))"),
        "capacity reserve must pass its real issuer counter through the test-input seam before allocation"
    );
    let compact_prepare_physical = compact_static_source(prepare_physical);
    assert!(
        compact_prepare_physical
            .contains("letallocation_counter=ledger_domain_counter_input(domain_counter);"),
        "cleanup ledger prepare must derive its local allocation counter from the supplied counter input"
    );
    assert!(
        compact_prepare_physical.contains("allocate_ledger_domain(allocation_counter)"),
        "cleanup ledger prepare must classify exhaustion through the seam-derived real allocation"
    );
}

#[test]
fn back_no_target_completion_callback_tail_is_infallible_and_query_free() {
    let navigation = include_str!("../src/navigation.rs");
    let cleanup = include_str!("../src/cleanup.rs");
    let core_engine = include_str!("../../bevy_ui_router_core/src/engine.rs");
    let helper_marker = "fn drive_back_no_target_zero_completion(";
    let receiver = ".complete_back_no_target_cleanup(";
    let tests_start = navigation
        .find("pub(crate) mod tests {")
        .expect("navigation source retains its crate-private test module");
    let production_navigation = &navigation[..tests_start];

    assert!(
        !production_navigation.contains("installed_back_no_target_cleanup"),
        "no-target completion must use one canonical RouterEntry holder"
    );
    let publication = body_after_marker(
        production_navigation,
        "fn consume_prepared_back_no_target_with_observers",
    );
    assert_eq!(
        publication
            .match_indices("pending_back_no_target_completion.insert(")
            .count(),
        1,
        "the first publication closure must install the canonical holder once"
    );
    let ledger_bind = publication
        .find("slot.bind_back_no_target(batch)")
        .expect("the publication closure must bind the prepared ledger");
    let installed_holder = publication
        .find("PendingBackNoTargetCompletion::Installed(")
        .expect("the publication closure must install the whole Installed variant");
    let holder_insert = publication
        .find("pending_back_no_target_completion.insert(")
        .expect("the publication closure must write the canonical holder");
    let installation_observer = publication
        .find("observe_installed(installed);")
        .expect("the whole holder must be observable before Core publication");
    assert!(
        ledger_bind < holder_insert
            && holder_insert < installed_holder
            && installed_holder < installation_observer,
        "ledger bind, single-holder install and observation must remain ordered"
    );

    assert_eq!(
        navigation.match_indices(receiver).count(),
        1,
        "the no-target completion receiver must have one ECS production callsite"
    );
    let helper = body_after_marker(navigation, helper_marker);
    assert_eq!(
        helper.match_indices(receiver).count(),
        1,
        "the unique receiver callsite must remain in the whole-owner completion driver"
    );
    let holder_classification = helper
        .find("match entry.pending_back_no_target_completion.as_ref()")
        .expect("the driver must classify the canonical holder before taking it");
    let live_installed = helper
        .find("if installed.inert.is_none()")
        .expect("only a non-inert Installed owner may proceed");
    let inert_return = helper
        .find(") => return true,")
        .expect("inert Installed and Settled holders must return handled");
    let installed_take = helper
        .find("pending_back_no_target_completion.take()")
        .expect("the driver must whole-take the canonical holder");
    assert!(
        holder_classification < live_installed
            && live_installed < inert_return
            && inert_return < installed_take,
        "inert Installed and Settled holders must stop before the unique whole take"
    );
    assert_eq!(
        helper
            .match_indices("pending_back_no_target_completion.take()")
            .count(),
        1,
        "the driver may take the canonical holder only once"
    );
    let proof = helper
        .find(".prepare_back_no_target_zero_completion()")
        .expect("the driver must prove the installed ledger exact-zero");
    let preflight = helper
        .find(".preflight_installed_back_capacity_reclaim(")
        .expect("the driver must preflight the exact installed lease");
    let completion = helper
        .find("installation.complete(")
        .expect("the driver must form one whole Core completion owner");
    let core_call = helper
        .find(receiver)
        .expect("the driver must call the exact Core receiver");
    assert!(
        proof < preflight && preflight < completion && completion < core_call,
        "proof, reclaim preflight, completion owner construction and Core receive must stay ordered"
    );

    let callback = callback_after_receiver(helper, receiver);
    let normalized_callback = callback.split_whitespace().collect::<String>();
    assert_eq!(
        callback.match_indices("guard.commit()").count(),
        1,
        "the exact receiver callback must commit the prebuilt reclaim once"
    );
    assert!(
        matches!(
            normalized_callback.as_str(),
            "guard.commit()" | "guard.commit();"
        ),
        "the callback may contain only the single reclaim commit; got {callback:?}"
    );
    for forbidden in [
        "prepared",
        "completion",
        "world",
        ".query",
        ".resource",
        ".entry(",
        ".get(",
        ".get_mut(",
        "[",
        "Vec::",
        "Box::",
        "String::",
        "vec!",
        ".collect",
        ".reserve",
        ".try_reserve",
        ".sort",
        ".validate",
        ".expect",
        ".unwrap",
        "panic!",
        "unreachable!",
        "unsafe",
        ".insert",
        ".remove",
        "despawn",
        "entity_mut",
        "commands",
        ".trigger",
        ".observe",
    ] {
        assert!(
            !callback.contains(forbidden),
            "the no-target reclaim callback contains forbidden work: {forbidden}"
        );
    }

    let core_receiver =
        body_after_marker(core_engine, "pub(crate) fn complete_back_no_target_cleanup");
    let terminal_preparation = core_receiver
        .find("self.prepare_back_no_target_cleanup_terminal(")
        .expect("Core must prepare the complete terminal before reclaim");
    let reclaim_commit = core_receiver
        .find("commit_reclaim();")
        .expect("Core must invoke the reclaim callback once");
    assert!(
        terminal_preparation < reclaim_commit,
        "all fallible terminal preparation must precede reclaim"
    );
    let terminal_tail = &core_receiver[reclaim_commit + "commit_reclaim();".len()..];
    for forbidden in [
        ".clone(",
        ".iter(",
        ".find(",
        ".retain(",
        ".remove(",
        ".push(",
        ".reserve(",
        ".try_reserve(",
        ".transition(",
        ".append_trace(",
        ".validate(",
        ".expect(",
        ".unwrap(",
        "panic!",
        "unreachable!",
        "unsafe",
        "?",
        ".query",
        ".resource",
    ] {
        assert!(
            !terminal_tail.contains(forbidden),
            "the Core terminal tail contains forbidden fallible or reconstructive work: {forbidden}"
        );
    }

    let commit = body_after_marker(cleanup, "pub(crate) fn commit(self)");
    let reclaim = match_arm_body_after_marker(
        commit,
        "BackCapacityCommitTransition::InstalledLeaseReclaim",
    );
    assert_eq!(
        reclaim
            .match_indices("cell.state = BackCapacityCellState::Free;")
            .count(),
        1,
        "installed lease reclaim must return the exact held cell to Free once"
    );
    for forbidden in [
        "self.live",
        "self.cells",
        ".entry(",
        ".get(",
        ".get_mut(",
        "[",
        "Vec::",
        "vec!",
        ".collect",
        ".reserve",
        ".try_reserve",
        ".validate",
        ".expect",
        ".unwrap",
        "panic!",
        "unreachable!",
        "unsafe",
        ".query",
        ".resource",
    ] {
        assert!(
            !reclaim.contains(forbidden),
            "installed lease reclaim must use only its held cell and prepared successor: {forbidden}"
        );
    }
}

#[test]
fn back_target_permanent_completion_callback_tail_is_infallible_and_publication_last() {
    let navigation = include_str!("../src/navigation.rs");
    let core_engine = include_str!("../../bevy_ui_router_core/src/engine.rs");
    let tests_start = navigation
        .find("pub(crate) mod tests {")
        .expect("navigation source retains its crate-private test module");
    let production_navigation = &navigation[..tests_start];
    let receiver = ".complete_back_target_permanent_cleanup(";

    assert_eq!(
        production_navigation.match_indices(receiver).count(),
        1,
        "target-permanent completion must have one ECS production receiver callsite"
    );
    let helper = body_after_marker(
        production_navigation,
        "fn drive_target_preparation_permanent_cleanup",
    );
    assert_eq!(helper.match_indices(receiver).count(), 1);
    let completing_install = helper
        .find("PendingTargetPreparationPermanentCleanup::Completing(")
        .expect("ECS must install the exact Completing holder before Core publication");
    let core_call = helper
        .find(receiver)
        .expect("the completion driver must call the trusted Core receiver");
    assert!(
        completing_install < core_call,
        "the whole Completing owner must be installed before the trusted receiver call"
    );

    let callback = callback_after_receiver(helper, receiver);
    let normalized_callback = callback.split_whitespace().collect::<String>();
    assert_eq!(normalized_callback.matches("*pending=None").count(), 1);
    assert!(
        matches!(
            normalized_callback.as_str(),
            "*pending=None" | "*pending=None;"
        ),
        "the callback may only clear the preinstalled exact Completing holder; got {callback:?}"
    );
    for forbidden in [
        "world",
        ".query",
        ".resource",
        ".entry(",
        ".get(",
        ".get_mut(",
        "[",
        "Vec::",
        "Box::",
        "String::",
        "vec!",
        ".collect",
        ".reserve",
        ".try_reserve",
        ".sort",
        ".validate",
        ".expect",
        ".unwrap",
        "panic!",
        "unreachable!",
        "unsafe",
        ".insert",
        ".remove",
        "despawn",
        "entity_mut",
        "commands",
        ".trigger",
        ".observe",
    ] {
        assert!(
            !callback.contains(forbidden),
            "the target completion callback contains forbidden work: {forbidden}"
        );
    }

    let core_receiver = body_after_marker(
        core_engine,
        "pub(crate) fn complete_back_target_permanent_cleanup",
    );
    let receiver_mismatch = body_after_marker(
        core_engine,
        "struct CoreBackTargetPermanentCleanupReceiverMismatchProjection",
    );
    assert!(
        receiver_mismatch
            .contains("expected: CoreBackTargetPermanentCleanupCompletionExpectedProjection")
            && receiver_mismatch
                .contains("observed: CoreBackTargetPermanentCleanupCompletionObservedProjection"),
        "completion receiver mismatch must retain the complete purpose-specific expected/observed boundary projections"
    );
    assert!(
        !core_receiver.contains("prepare_back_target_permanent_cleanup_terminal("),
        "target completion must consume the terminal plan prepared before host retirement"
    );
    let receiver_rejection = core_receiver
        .find("if !exact_receiver")
        .expect("Core must reject receiver drift before the callback");
    let facts_rejection = core_receiver
        .find("if !exact_facts")
        .expect("Core must reject completion-fact drift before the callback");
    let callback_commit = core_receiver
        .find("commit_completion();")
        .expect("Core must invoke the ECS completion callback once");
    assert!(receiver_rejection < callback_commit && facts_rejection < callback_commit);
    let terminal_tail = &core_receiver[callback_commit + "commit_completion();".len()..];
    assert!(
        !terminal_tail.contains("if let Some(cleanup)")
            && !terminal_tail.contains("let Some(cleanup)"),
        "Core must move and validate the exact private Cleanup before the callback, then consume it unconditionally"
    );
    for forbidden in [
        ".clone(",
        ".iter(",
        ".find(",
        ".retain(",
        ".remove(",
        ".push(",
        ".reserve(",
        ".try_reserve(",
        ".transition(",
        ".append_trace(",
        ".validate(",
        ".expect(",
        ".unwrap(",
        "panic!",
        "unreachable!",
        "unsafe",
        "?",
        ".query",
        ".resource",
        ".clear(",
    ] {
        assert!(
            !terminal_tail.contains(forbidden),
            "the callback/post-callback terminal tail contains forbidden work: {forbidden}"
        );
    }
    let private_cleanup_take = core_receiver[..callback_commit]
        .find("self.pending_back_target_permanent_cleanup.take()")
        .expect("Core must move the exact private Cleanup into a prepared local before the ECS callback");
    let retired_consumption = terminal_tail
        .find("drop(retired_host);")
        .expect("Core must consume its matching retired-host authority in the atomic tail");
    let state_publication = terminal_tail
        .find("self.state = state;")
        .expect("Core must publish the begin-prepared AwaitingHost state by assignment");
    assert!(
        private_cleanup_take < callback_commit && retired_consumption < state_publication,
        "Core must prepare the whole Cleanup before the callback, then consume retired-host authority before AwaitingHost publication"
    );
}

#[test]
fn back_target_dispatch_owners_are_linear_and_exact() {
    let navigation = include_str!("../src/navigation.rs");
    let tests_start = navigation
        .find("pub(crate) mod tests {")
        .expect("navigation source retains its crate-private test module");
    let production_navigation = &navigation[..tests_start];
    let integration = include_str!("../../bevy_ui_router_core/src/integration.rs");

    assert_eq!(
        navigation
            .match_indices("pending_back_target_execution: Option<PendingBackTargetExecution>",)
            .count(),
        1,
        "RouterEntry must retain exactly one target execution stage field"
    );
    assert!(
        !navigation.contains("pending_back_target_dispatch_safe_stop"),
        "target dispatch must not introduce a second safe-stop owner field"
    );

    let stages = body_after_marker(navigation, "pub(crate) enum PendingBackTargetExecution");
    for variant in [
        "ResumeBlocked(",
        "Driving(",
        "Classifying(",
        "Prepared(",
        "CommitFailure(",
    ] {
        assert!(
            stages.contains(variant),
            "the sole target stage enum is missing {variant}"
        );
    }
    for branch in [
        "ReadyPlanRetry",
        "ReadyPlanPermanent",
        "PendingRejectedAcceptanceMismatch",
        "PendingRejected",
        "AuthorityLostAcceptanceMismatch",
        "AuthorityLost",
        "ReadyAccepted",
        "ReadyReserveFailure",
    ] {
        assert!(
            navigation.contains(branch),
            "target classification must retain the typed {branch} owner"
        );
    }

    assert_eq!(
        production_navigation
            .match_indices(".validate_back_target_execution_pairing(")
            .count(),
        11,
        "action injection, retry restore, source freeze, failure routing, applied MountRequest/Commit outcomes, classifier and ReadyPlan permanent preparation, commit transfer, and prepared publication must each use the one opaque pairing predicate"
    );
    assert!(
        navigation.contains("ReconciliationPreparationOutcome::PairingMismatch"),
        "classifier pairing mismatch must retain the Core-owned opaque evidence"
    );
    assert_eq!(
        navigation
            .match_indices(".prepare_back_target_commit(")
            .count(),
        1,
        "only the prepared target stage may enter the Core commit bridge"
    );
    assert!(
        navigation.contains(
            "pub(crate) fn classify_back_target(\n    world: &World,\n    entry: &mut RouterEntry,"
        ),
        "the physical target classifier must receive the current World explicitly"
    );

    let ready_driver = body_after_marker(navigation, "fn publish_ready_back_target(");
    let acceptance = ready_driver
        .find(".accept_back_target_ready(")
        .expect("the scoped Ready driver must call typed acceptance");
    for preparation in [
        ".prepare_lifecycle_mutation(",
        "prepare_input_gate_mutation(",
        "prepare_focus_mutation(",
        ".prepare_claim_rebind(",
        "prepare_presentation_mutation(",
    ] {
        assert!(
            ready_driver
                .find(preparation)
                .is_some_and(|index| index < acceptance),
            "{preparation} must complete before Ready acceptance"
        );
    }
    assert!(
        ready_driver.contains("if frozen_again != ready.frozen_manifest"),
        "the scoped refreeze must compare the complete exact Entity manifest"
    );
    assert!(
        ready_driver
            .find("pending.ticket.take()")
            .is_some_and(|index| index < ready_driver.find("world.resource_scope(").unwrap()),
        "the real ticket must move outside the scoped borrowed plans before acceptance"
    );
    let accepted_install = ready_driver
        .find("PendingBackTargetClassification::ReadyAccepted(")
        .expect("Matched acceptance must install ReadyAccepted");
    let reserve = ready_driver
        .find("CleanupLedger::reserve(")
        .expect("ReadyAccepted must reserve the exact Entity ledger");
    let prepared_install = ready_driver
        .find("PendingBackTargetExecution::Prepared(")
        .expect("reserve success must install Prepared");
    let bridge = ready_driver
        .find(".prepare_back_target_commit(")
        .expect("Prepared must enter the target commit bridge");
    let publisher = ready_driver
        .find("engine.publish(||")
        .expect("Validated target commit must reuse the existing publisher");
    assert!(
        acceptance < accepted_install
            && accepted_install < reserve
            && reserve < prepared_install
            && prepared_install < bridge
            && bridge < publisher,
        "Ready success must remain one ordered no-yield owner transition"
    );
    assert_eq!(
        integration
            .match_indices("pub fn publish<F>(self, mutation: F)")
            .count(),
        1,
        "ValidatedCommit must retain one publication consumer"
    );
}

#[test]
fn back_target_applied_report_outcome_domain_is_exact() {
    let core_engine = include_str!("../../bevy_ui_router_core/src/engine.rs");
    let integration = include_str!("../../bevy_ui_router_core/src/integration.rs");
    let navigation = include_str!("../src/navigation.rs");

    assert_eq!(
        enum_variant_names(core_engine, "AppliedReportTransition"),
        ["MountRequest", "Commit", "FailureCleanup"]
    );
    assert_eq!(
        enum_variant_names(integration, "BackTargetAppliedReportOutcome"),
        ["MountRequest", "Commit", "FailureCleanup", "Mismatch"]
    );
    assert_eq!(
        enum_variant_names(integration, "BackTargetResumeAction"),
        ["FrameTask", "MountRequest", "Commit"],
        "initial target resume must retain the exact normal action domain"
    );
    let resume = body_after_marker(integration, "pub fn resume_back_target_with_retirement<F>(");
    assert!(
        resume.contains("CoreBackTargetResumeAction::FrameTask(inner)")
            && resume.contains("BackTargetResumeAction::FrameTask(FrameTask { inner })"),
        "initial target resume must preserve the Core FrameTask owner"
    );
    let mount_completion = body_after_marker(core_engine, "fn apply_confirmed_child(");
    assert!(
        mount_completion.contains("self.action = Some(EngineAction::FrameTask(FrameTask {"),
        "mount completion must install the confirmed child FrameTask"
    );
    assert!(
        !enum_variant_names(navigation, "PendingBackTargetResumeBlocked")
            .iter()
            .any(|variant| variant == "AppliedFrameTaskPending"),
        "applied report must not retain a dead FrameTask continuation stage"
    );
    assert!(
        !integration.contains("BackTargetAppliedReportOutcome::FrameTask"),
        "the integration receiver must not reintroduce an unproducible applied FrameTask"
    );
}

#[test]
fn back_target_zero_entering_commit_reuses_the_single_target_publisher() {
    let integration = include_str!("../../bevy_ui_router_core/src/integration.rs");
    let navigation = include_str!("../src/navigation.rs");
    let tests_start = navigation
        .find("pub(crate) mod tests {")
        .expect("navigation source retains its crate-private test module");
    let production_navigation = &navigation[..tests_start];

    assert_eq!(
        integration
            .match_indices("pub fn into_commit_parts(")
            .count(),
        1,
        "BackTargetResume must expose one consuming Commit splitter"
    );
    let commit_parts = body_after_marker(integration, "pub fn into_commit_parts(");
    for required in [
        "let Self {\n            action,\n            execution,\n            continuation,\n        } = self;",
        "BackTargetResumeAction::Commit(ticket) => Ok((ticket, execution, continuation))",
        "action => Err(Self {",
    ] {
        assert!(
            commit_parts.contains(required),
            "the consuming Commit splitter must move the exact ticket, execution and continuation or reconstruct the untouched resume: {required}"
        );
    }
    assert!(
        !commit_parts.contains(".clone()"),
        "the Commit splitter must not duplicate any resume owner"
    );

    let dispatch = body_after_marker(production_navigation, "fn dispatch_pending_back_target(");
    assert_eq!(
        production_navigation
            .match_indices(".into_commit_parts()")
            .count(),
        1,
        "production ECS must consume BackTargetResume Commit at one dispatch boundary"
    );
    for required in [
        "Err(resume) => match resume.into_commit_parts()",
        "Ok((ticket, history, continuation))",
        "PendingBackTargetExecution::Driving(PendingBackTargetDriving {",
        "BackTargetDispatchOutcome::Action(EngineAction::Commit(ticket))",
    ] {
        assert!(
            dispatch.contains(required),
            "zero-entering Commit dispatch must install Driving and return the same shared action: {required}"
        );
    }
    let split = dispatch
        .find("resume.into_commit_parts()")
        .expect("the unique Commit split was required");
    let driving = dispatch[split..]
        .find("PendingBackTargetExecution::Driving(PendingBackTargetDriving {")
        .map(|offset| split + offset)
        .expect("Commit dispatch must install Driving");
    let action = dispatch[driving..]
        .find("BackTargetDispatchOutcome::Action(EngineAction::Commit(ticket))")
        .map(|offset| driving + offset)
        .expect("Commit dispatch must return the shared Commit action");
    assert!(
        split < driving && driving < action,
        "Commit split, Driving installation and action handoff must remain one ordered move"
    );
    for forbidden in [
        "classify_back_target(",
        "publish_ready_back_target(",
        "engine.publish(",
    ] {
        assert!(
            !dispatch.contains(forbidden),
            "zero-entering dispatch must not bypass the shared reconciliation/publisher path: {forbidden}"
        );
    }

    let navigation_pass = body_after_marker(production_navigation, "fn drive_navigation_pass(");
    let target_action = navigation_pass
        .find("BackTargetDispatchOutcome::Action(action) => Some(action)")
        .expect("target dispatch must return through shared action selection");
    let action_selection = navigation_pass
        .find("if let Some(action) = target_action")
        .expect("the shared action selector must prefer the target action");
    let commit_arm = navigation_pass
        .find("EngineAction::Commit(ticket) => {")
        .expect("the shared action driver must retain its Commit arm");
    assert!(
        target_action < action_selection && action_selection < commit_arm,
        "target Commit must flow through the normal action selector before pending installation"
    );
    let commit_install =
        match_arm_body_after_marker(navigation_pass, "EngineAction::Commit(ticket)");
    assert!(
        commit_install.contains("let mut pending = PendingCommit::new();")
            && commit_install.contains("pending.ticket = Some(ticket);")
            && commit_install.contains("entry.pending_commit = Some(pending);"),
        "the shared Commit arm must install the exact ticket into PendingCommit"
    );
    assert!(
        !commit_install.contains("publish"),
        "pending Commit installation must not publish"
    );

    let root_commit = body_after_marker(production_navigation, "fn drive_back_target_root_commit(");
    for required in [
        "if drive_back_target_applied_commit(world, entry)",
        "Some(PendingBackTargetExecution::Driving(_))",
        "let Some(mut pending) = entry.pending_commit.take()",
        "if pending.ticket.is_some()",
        "let Some(PendingBackTargetExecution::Driving(driving))",
        "let ticket = pending\n            .ticket\n            .take()",
        "PendingBackTargetResumeBlocked::AppliedCommitPending(PendingBackTargetAppliedCommit {",
        "let _ = drive_back_target_applied_commit(world, entry);",
    ] {
        assert!(
            root_commit.contains(required),
            "root Commit must move the exact Driving, ticket and pending owners into the applied Commit driver: {required}"
        );
    }
    assert_eq!(
        root_commit
            .match_indices("PendingBackTargetAppliedCommit {")
            .count(),
        1,
        "zero-entering root Commit must construct one applied Commit owner"
    );
    let zero_entering_root_commit = root_commit
        .split_once("let Some(task) = pending.final_task.take()")
        .map(|(zero_entering, _)| zero_entering)
        .expect("the zero-entering Commit path must precede the legacy frame-task path");
    assert_eq!(
        zero_entering_root_commit
            .match_indices("drive_back_target_applied_commit(world, entry)")
            .count(),
        2,
        "root Commit must first resume an existing applied owner and then drive its newly installed owner"
    );
    for forbidden in [
        "classify_back_target(",
        "publish_ready_back_target(",
        "engine.publish(",
    ] {
        assert!(
            !root_commit.contains(forbidden),
            "root Commit staging must not skip the applied Commit driver: {forbidden}"
        );
    }

    let applied_commit = body_after_marker(
        production_navigation,
        "fn drive_back_target_applied_commit(",
    );
    for required in [
        "PendingBackTargetResumeBlocked::AppliedCommitPending(_)",
        "PendingBackTargetResumeBlocked::AppliedCommitPending(mut owner)",
        "BackTargetPhysicalOwnerRef::CommitTicket(&owner.ticket)",
        "pending.ticket = Some(ticket);",
        "PendingBackTargetClassification::Active(TargetPreparationOwner {",
        "match classify_back_target(world, entry, owner)",
        "ReconciliationPreparationDecision::Ready(",
        "publish_ready_back_target(world, entry, pending, owner);",
    ] {
        assert!(
            applied_commit.contains(required),
            "the applied Commit driver must preserve the exact pairing, reconciliation and Ready handoff: {required}"
        );
    }
    let classify = applied_commit
        .find("match classify_back_target(world, entry, owner)")
        .expect("the applied Commit classifier was required");
    let ready = applied_commit[classify..]
        .find("ReconciliationPreparationDecision::Ready(")
        .map(|offset| classify + offset)
        .expect("classification must produce the typed Ready arm");
    let target_publisher = applied_commit[ready..]
        .find("publish_ready_back_target(world, entry, pending, owner);")
        .map(|offset| ready + offset)
        .expect("Ready must enter the shared target publisher");
    assert!(
        classify < ready && ready < target_publisher,
        "applied Commit classification must precede the sole target publisher"
    );
    assert_eq!(
        applied_commit
            .match_indices("publish_ready_back_target(")
            .count(),
        1,
        "the applied Commit driver must have one Ready publisher handoff"
    );
    assert!(
        !applied_commit.contains("engine.publish("),
        "the applied Commit driver must not publish before Ready preparation"
    );

    assert_eq!(
        production_navigation
            .match_indices("publish_ready_back_target(")
            .count(),
        2,
        "production must contain one target publisher declaration and one callsite"
    );
    let publisher = body_after_marker(production_navigation, "fn publish_ready_back_target(");
    assert_eq!(
        publisher.match_indices("engine.publish(|| {").count(),
        1,
        "the shared target publisher must retain one existing publication tail"
    );
}

#[test]
fn back_target_frame_task_failures_preserve_one_typed_execution_owner() {
    let navigation = include_str!("../src/navigation.rs");
    let tests_start = navigation
        .find("pub(crate) mod tests {")
        .expect("navigation source retains its crate-private test module");
    let production_navigation = &navigation[..tests_start];
    let drive = body_after_marker(production_navigation, "fn drive_navigation_pass(");

    let expected_failure_sites = [
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "route view report application became stale",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "router host unavailable during frame apply",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "router host authority changed before frame realization",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Build",
            "route view binding unavailable",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Build",
            "route view driver unavailable",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "route view report identity became stale",
        ),
        (
            "Pending",
            "pending",
            "failure",
            "route view driver rejected the frame",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "route view report application became stale",
        ),
        (
            "Residual",
            "failed_branch",
            "ViewFailureKind::Apply",
            "frame realization rejected",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "frame realization rejected",
        ),
        (
            "Residual",
            "failed_branch",
            "ViewFailureKind::Apply",
            "frame branch append rejected",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "parent frame unavailable after realization",
        ),
        (
            "Pending",
            "pending",
            "ViewFailureKind::Apply",
            "parent frame preflight rejected",
        ),
    ];
    let expected_calls = expected_failure_sites
        .iter()
        .map(|(variant, owner, kind, message)| {
            format!(
                "world,entry,BackTargetDriverUnverifiedPhysicalOwner::{variant}({owner}),task,{kind},\"{message}\","
            )
        })
        .collect::<Vec<_>>();
    let actual_calls = parenthesized_call_arguments(drive, "route_back_target_frame_failure")
        .into_iter()
        .map(compact_static_source)
        .collect::<Vec<_>>();
    assert_eq!(
        actual_calls, expected_calls,
        "every normal dispatcher FrameTask failure exit must remain in the reviewed semantic manifest and enter the target-aware route first"
    );

    let expected_generic_reports = [
        (
            "ViewFailureKind::Apply",
            "router host unavailable during frame apply",
        ),
        (
            "ViewFailureKind::Apply",
            "router host authority changed before frame realization",
        ),
        ("ViewFailureKind::Build", "route view binding unavailable"),
        ("ViewFailureKind::Build", "route view driver unavailable"),
        ("failure", "route view driver rejected the frame"),
        ("ViewFailureKind::Apply", "frame realization rejected"),
    ]
    .map(|(kind, message)| format!("task,{kind},\"{message}\","));
    let actual_generic_reports = parenthesized_call_arguments(drive, ".report_failed")
        .into_iter()
        .map(compact_static_source)
        .collect::<Vec<_>>();
    assert_eq!(
        actual_generic_reports, expected_generic_reports,
        "generic failure reporting must remain limited to reviewed NonTarget fallthroughs"
    );
    for (report_offset, _) in drive.match_indices(".report_failed(") {
        let before_report = &drive[..report_offset];
        let route_offset = before_report
            .rfind("route_back_target_frame_failure(")
            .expect("every generic FrameTask report must follow the target-aware route");
        let routed_fallthrough = &before_report[route_offset..];
        let handled = routed_fallthrough
            .find("RouteBackTargetFrameFailureOutcome::Handled")
            .expect("the target-aware match must retain its handled branch");
        let stop = routed_fallthrough[handled..]
            .find("continue;")
            .map(|offset| handled + offset)
            .expect("Handled must stop before generic reporting");
        let non_target = routed_fallthrough
            .find("RouteBackTargetFrameFailureOutcome::NonTarget")
            .expect("generic reporting must receive the proven NonTarget owner");
        assert!(
            handled < stop && stop < non_target,
            "Handled must stop the branch before only NonTarget may fall through to generic reporting"
        );
    }
    assert!(
        !drive.contains(".report_failed_with_physical_residual("),
        "the FrameTask dispatcher must not bypass the target-aware route through direct residual reporting"
    );

    let route = body_after_marker(production_navigation, "fn route_back_target_frame_failure(");
    assert_eq!(
        route
            .match_indices("entry.engine.is_current_root_back_frame_task(&task)")
            .count(),
        1,
        "the target-aware route must classify the exact task once"
    );
    assert_eq!(
        route
            .match_indices("RouteBackTargetFrameFailureOutcome::NonTarget")
            .count(),
        1,
        "the route must expose only one proven generic fallthrough"
    );
    assert!(
        route.contains("return if current_root_back_task {")
            && route.contains(
                "BackTargetExecutionPairingOutcome::Matched\n            if matches!(task.mount_target(), FrameMountTarget::TopSlot { .. })\n                && !current_root_back_task"
            ),
        "missing staging and non-current TopSlot cases must use the exact current-task predicate before any generic or mismatch disposition"
    );
    let current_predicate = route
        .find("entry.engine.is_current_root_back_frame_task(&task)")
        .expect("the exact current-task predicate was counted");
    let generic_fallthrough = route
        .find("RouteBackTargetFrameFailureOutcome::NonTarget")
        .expect("the sole generic fallthrough was counted");
    assert!(
        current_predicate < generic_fallthrough,
        "the exact target predicate must precede the sole generic fallthrough"
    );

    assert_eq!(
        enum_variant_names(navigation, "BackTargetDriverUnverifiedPhysicalOwner"),
        ["Pending", "Residual"],
        "unverified physical ownership must stay whole rather than collapse into a binary zero/residual result"
    );
    assert_eq!(
        enum_variant_names(navigation, "BackTargetDriverReportablePhysicalOwner"),
        ["ProvenZero", "Residual"],
        "only classified owners may enter the report-mode domain"
    );
    assert_eq!(
        enum_variant_names(navigation, "BackTargetDriverPhysicalClassification"),
        ["Reportable", "Indeterminate"],
        "pre-report classification must preserve the exact two-way typed domain"
    );
    assert_eq!(
        enum_variant_names(navigation, "PendingBackTargetDriverFailure"),
        [
            "PhysicalIndeterminate",
            "AwaitingReport",
            "ReceiverMismatch",
            "ConsumeMismatch",
            "PhysicalMismatch",
            "PostReportPhysicalIndeterminate",
        ],
        "Unaccepted or Reported must never become a schedule-surviving driver stage"
    );

    let freezer = body_after_marker(
        production_navigation,
        "pub(crate) fn freeze_back_target_driver_physical_partition_source(",
    );
    assert_eq!(
        production_navigation
            .match_indices("BackTargetDriverPhysicalPartitionSource {")
            .count(),
        3,
        "the source declaration, its impl and the unique freezer must be its only structural occurrences"
    );
    assert!(
        freezer.contains("Ok(BackTargetDriverPhysicalPartitionSource { frozen })"),
        "the unique zero-write freezer must be the sole physical source producer"
    );
    for forbidden in [
        "BackTargetDriverUnverifiedPhysicalOwner",
        "PendingCommit",
        "FailedPreparedBranch",
    ] {
        assert!(
            !freezer.contains(forbidden),
            "the source freezer must not consume or reconstruct the physical owner: {forbidden}"
        );
    }
    let source_seal = body_after_marker(
        production_navigation,
        "impl BackTargetDriverPhysicalPartitionSource",
    );
    assert!(
        !source_seal.contains("Self {"),
        "the source impl must not introduce a second source producer"
    );
    assert_eq!(
        production_navigation
            .match_indices("BackTargetDriverPostReportRefreezeSource {")
            .count(),
        3,
        "the post-report source declaration, seal return boundary and consuming construction must be its only structural occurrences"
    );
    assert!(
        source_seal.contains("fn seal_for_report(self)")
            && source_seal.contains("frozen: self.frozen"),
        "the only post-report source producer must consume the exact pre-report source"
    );

    let classifier_tail = production_navigation
        .split_once("pub(crate) fn classify_back_target_driver_physical(")
        .expect("the pre-report classifier must exist")
        .1;
    let classifier_signature = classifier_tail
        .split_once('{')
        .expect("the pre-report classifier must have a body")
        .0;
    assert!(
        classifier_signature.contains("world: &World")
            && classifier_signature.contains("entry: &RouterEntry")
            && classifier_signature.contains("source: &BackTargetDriverPhysicalPartitionSource")
            && classifier_signature.contains("owner: BackTargetDriverUnverifiedPhysicalOwner")
            && classifier_signature.contains(") -> BackTargetDriverPhysicalClassification")
            && !classifier_signature.contains("RootFrameTask"),
        "the pre-report classifier must accept the current World, exact entry, frozen source and whole unverified owner without a detached task"
    );
    let classifier = braced_body(classifier_tail);
    assert!(
        classifier.contains(
            "classify_back_target_driver_frozen_physical(world, entry, &source.frozen, owner)"
        ),
        "the public pre-classifier must delegate the same World, entry, source and owner without scanning or reconstruction"
    );

    let frozen_classifier = body_after_marker(
        production_navigation,
        "fn classify_back_target_driver_frozen_physical(",
    );
    for required in [
        "if observed != *frozen",
        "if !back_target_driver_root_lineage_is_exact(frozen)",
        "if !back_target_driver_active_registry_is_complete(frozen)",
        "pending.ticket.is_none()",
        "pending.final_task.is_none()",
        "pending.branch.frames().is_empty()",
        "pending.branch.child_slots().is_empty()",
        "if !frozen.target_claims.is_empty()",
        "BackTargetDriverPhysicalIndeterminateReason::PendingPhysicalResidual",
        "back_target_driver_residual_reportable(world, frozen, branch)",
    ] {
        assert!(
            frozen_classifier.contains(required),
            "the classifier must retain exact drift, completeness and typed-owner proof: {required}"
        );
    }
    for forbidden in [
        "entry.pending_commit",
        "entry.pending_failed_branch",
        "entry.pending_frame_drive",
        "entry.pending_mount_frame_task",
        "entry.pending_mount_request",
        "entry.pending_back_target_execution",
        "entry.engine.action()",
    ] {
        assert!(
            !frozen_classifier.contains(forbidden),
            "the classifier must consume the supplied owner rather than reconstruct one from a scanner or staging field: {forbidden}"
        );
    }
    let root_lineage = body_after_marker(
        production_navigation,
        "fn back_target_driver_root_lineage_is_exact(",
    );
    for required in [
        "let FrameMountTarget::TopSlot { host, slot } = &source.task.mount",
        "source.active_frames.len() == 1",
        "source.active_child_slots.is_empty()",
        "root.slot == source.host.top_slot",
        "root.slot_claim == source.host.top_slot_claim",
        "BackTargetDriverRootPhysicalParentProjection::Host(source.host.host)",
    ] {
        assert!(
            root_lineage.contains(required),
            "ProvenZero must remain restricted to one exact root TopSlot lineage: {required}"
        );
    }
    let residual_classifier = body_after_marker(
        production_navigation,
        "fn back_target_driver_residual_reportable(",
    );
    for required in [
        "branch.frames().len() != 1",
        "!branch.child_slots().is_empty()",
        "!branch.mount_residuals().is_empty()",
        "!branch.detached_frames().is_empty()",
        "!branch.rejected_child_slots().is_empty()",
        "branch.mount_new_allowance().is_some()",
        "manifest.len() != target_claims.len()",
        "!manifest.boundary_drifts().is_empty()",
        "!manifest.preflight(world)",
        "source.protected.contains(&claim.entity())",
        "BackTargetDriverPhysicalIndeterminateReason::TargetProtectedOverlap",
    ] {
        assert!(
            residual_classifier.contains(required),
            "partial, sparse, drifting or protected residual topology must remain Indeterminate: {required}"
        );
    }

    assert_eq!(
        production_navigation
            .match_indices("pub(crate) const fn report_mode(&self)")
            .count(),
        1,
        "only the reportable physical owner may derive a Core report mode"
    );
    assert_eq!(
        production_navigation
            .match_indices(".report_back_target_failed(")
            .count(),
        1,
        "the current driver must have one dedicated failure-report receiver callsite"
    );
    let driver = body_after_marker(
        production_navigation,
        "pub(crate) fn drive_back_target_driver_failure(",
    );
    for required in [
        "let PendingBackTargetDriverFailure::AwaitingReport(owner) = owner else",
        "let physical = match classify_back_target_driver_physical(world, entry, &source, physical)",
        "BackTargetDriverPhysicalClassification::Reportable(physical) => physical",
        "let report_mode = physical.report_mode();",
        "let post_source = source.seal_for_report();",
        "entry.engine.report_back_target_failed(",
        "BackTargetDriverFailureReportOutcome::Reported(outcome) => outcome",
        "consume_back_target_driver_reported(",
    ] {
        assert!(
            driver.contains(required),
            "the sole driver must preserve the classify-report-consume owner chain: {required}"
        );
    }
    let classify = driver
        .find("classify_back_target_driver_physical(")
        .expect("the driver classifier was required");
    let report_mode = driver
        .find("let report_mode = physical.report_mode();")
        .expect("the reportable-mode derivation was required");
    let source_seal = driver
        .find("let post_source = source.seal_for_report();")
        .expect("the source seal was required");
    let report = driver
        .find("entry.engine.report_back_target_failed(")
        .expect("the report receiver was required");
    let consume = driver
        .find("consume_back_target_driver_reported(")
        .expect("the consume handoff was required");
    assert!(
        classify < report_mode
            && report_mode < source_seal
            && source_seal < report
            && report < consume,
        "classification, reportable mode, source seal, exact report and consume must remain one ordered owner transfer"
    );
    let report_mismatch =
        match_arm_body_after_marker(driver, "BackTargetDriverFailureReportOutcome::Mismatch");
    for required in [
        "drop(post_source);",
        "task,",
        "physical: physical.into_unverified()",
        "mismatch,",
    ] {
        assert!(
            report_mismatch.contains(required),
            "report mismatch must retain the task and whole Unverified owner while discarding the unconsumed post source: {required}"
        );
    }

    let receiver_mismatch = body_after_marker(
        navigation,
        "pub(crate) struct PendingBackTargetDriverReceiverMismatch",
    );
    for required in [
        "task: RootFrameTask",
        "physical: BackTargetDriverUnverifiedPhysicalOwner",
        "mismatch: PreparationReceiverMismatchView",
    ] {
        assert!(
            receiver_mismatch.contains(required),
            "receiver mismatch must retain the exact task and unverified physical owner: {required}"
        );
    }
    let reported = body_after_marker(navigation, "struct BackTargetDriverFailureReported");
    for required in [
        "source: BackTargetDriverPostReportRefreezeSource",
        "history: HistoryExecutionBindingView",
        "physical: BackTargetDriverReportablePhysicalOwner",
        "outcome: BackTargetDriverFailureCoreOutcome",
    ] {
        assert!(
            reported.contains(required),
            "the transient reported owner must carry one sealed post-report tuple: {required}"
        );
    }
    for owner in [
        "pub(crate) struct PendingBackTargetDriverConsumeMismatch",
        "pub(crate) struct PendingBackTargetDriverPhysicalMismatch",
        "pub(crate) struct PendingBackTargetDriverPostReportPhysicalIndeterminate",
    ] {
        let body = body_after_marker(navigation, owner);
        for required in [
            "source: BackTargetDriverPostReportRefreezeSource",
            "history: HistoryExecutionBindingView",
            "physical: BackTargetDriverUnverifiedPhysicalOwner",
            "outcome: BackTargetDriverFailureCoreOutcome",
        ] {
            assert!(
                body.contains(required),
                "{owner} must retain the complete post-report owner tuple: {required}"
            );
        }
    }

    let reclassifier_tail = production_navigation
        .split_once("fn reclassify_back_target_driver_post_report(")
        .expect("the dedicated post-report reclassifier must exist")
        .1;
    let reclassifier_signature = reclassifier_tail
        .split_once('{')
        .expect("the post-report reclassifier must have a body")
        .0;
    assert!(
        reclassifier_signature.contains("source: &BackTargetDriverPostReportRefreezeSource")
            && reclassifier_signature.contains("owner: BackTargetDriverUnverifiedPhysicalOwner")
            && !reclassifier_signature.contains("RootFrameTask"),
        "post-report reclassification must require the sealed source and whole owner without reconstructing a task"
    );
    assert_eq!(
        production_navigation
            .match_indices("reclassify_back_target_driver_post_report(")
            .count(),
        2,
        "the dedicated post-report reclassifier must have one declaration and one applied FailureCleanup callsite"
    );
    let reclassifier = braced_body(reclassifier_tail);
    assert!(
        !reclassifier.contains("report_back_target_failed")
            && !reclassifier.contains("report_mode"),
        "post-report Indeterminate must never re-enter report"
    );
    let applied = body_after_marker(
        production_navigation,
        "fn route_back_target_applied_report(",
    );
    let failure_cleanup = match_arm_body_after_marker(
        applied,
        "BackTargetAppliedReportOutcome::FailureCleanup(owner)",
    );
    for required in [
        "reclassify_back_target_driver_post_report(",
        "BackTargetDriverPhysicalClassification::Reportable(physical)",
        "consume_back_target_driver_reported(",
        "BackTargetDriverPhysicalClassification::Indeterminate(indeterminate)",
        "PendingBackTargetDriverFailure::PostReportPhysicalIndeterminate(",
    ] {
        assert!(
            failure_cleanup.contains(required),
            "applied FailureCleanup must use the dedicated no-task reclassifier before consume or typed Indeterminate storage: {required}"
        );
    }
    assert!(
        !failure_cleanup.contains("report_back_target_failed"),
        "the applied post-report FailureCleanup branch must never report again"
    );
}
