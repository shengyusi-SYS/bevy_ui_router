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
        9,
        "action injection, retry restore, source freeze, failure routing, applied MountRequest/Commit outcomes, classifier entry, commit transfer, and prepared publication must each use the one opaque pairing predicate"
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
