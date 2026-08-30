#![allow(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use syn::visit::Visit;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TableRow(pub Vec<String>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedProfileRow {
    pub item: String,
    pub owning_slice: String,
    pub state: String,
    pub claim_boundary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryContractProfile {
    pub exclusions: Vec<TableRow>,
    pub contract_deltas: Vec<StagedProfileRow>,
    pub authority_additions: Vec<StagedProfileRow>,
}

impl HistoryContractProfile {
    fn parse(source: &str) -> Self {
        let exclusions = strict_history_table(
            source,
            "### 10.1 Public contract and stage exclusions",
            &["Kind", "Canonical path or normalized item", "First slice"],
        );
        let contract_deltas = strict_history_table(
            source,
            "### 10.2 Contract delta activation",
            &["Item", "Owning slice", "State", "Claim boundary"],
        )
        .into_iter()
        .map(|row| StagedProfileRow {
            item: row.0[0].clone(),
            owning_slice: row.0[1].clone(),
            state: row.0[2].clone(),
            claim_boundary: Some(row.0[3].clone()),
        })
        .collect::<Vec<_>>();
        let authority_additions = strict_history_table(
            source,
            "### 10.3 History authority regression additions",
            &["Test function", "Owning slice", "State"],
        )
        .into_iter()
        .map(|row| StagedProfileRow {
            item: row.0[0].clone(),
            owning_slice: row.0[1].clone(),
            state: row.0[2].clone(),
            claim_boundary: None,
        })
        .collect::<Vec<_>>();
        let profile = Self {
            exclusions,
            contract_deltas,
            authority_additions,
        };
        profile.validate();
        profile
    }

    pub fn assert_disjoint_keys(&self) {
        let exclusions = self
            .exclusions
            .iter()
            .map(|row| row.0[1].as_str())
            .collect::<BTreeSet<_>>();
        let contract_deltas = self
            .contract_deltas
            .iter()
            .map(|row| row.item.as_str())
            .collect::<BTreeSet<_>>();
        let authority_additions = self
            .authority_additions
            .iter()
            .map(|row| row.item.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            exclusions.len(),
            self.exclusions.len(),
            "History exclusion keys must be unique"
        );
        assert_eq!(
            contract_deltas.len(),
            self.contract_deltas.len(),
            "History contract-delta keys must be unique"
        );
        assert_eq!(
            authority_additions.len(),
            self.authority_additions.len(),
            "History authority-addition keys must be unique"
        );
        assert!(
            exclusions.is_disjoint(&contract_deltas),
            "History exclusions and contract deltas must be disjoint"
        );
        assert!(
            exclusions.is_disjoint(&authority_additions),
            "History exclusions and authority additions must be disjoint"
        );
        assert!(
            contract_deltas.is_disjoint(&authority_additions),
            "History contract deltas and authority additions must be disjoint"
        );
    }

    fn validate(&self) {
        validate_exclusion_vocabulary(&self.exclusions);
        self.assert_disjoint_keys();
        validate_staged_rows(&self.contract_deltas, true);
        validate_staged_rows(&self.authority_additions, false);
        for row in self
            .contract_deltas
            .iter()
            .filter(|row| row.state == "active")
        {
            assert_active_contract_delta_is_evidenced(row);
        }
        validate_active_history_authority_rows(&self.authority_additions);
    }
}

pub fn history_contract_profile() -> HistoryContractProfile {
    let source = read_workspace("docs/superpowers/specs/2026-07-20-history-design.md");
    parse_history_contract_profile(&source)
}

pub fn parse_history_contract_profile(source: &str) -> HistoryContractProfile {
    HistoryContractProfile::parse(source)
}

pub fn parse_recovery_contract_profile(source: &str) -> HistoryContractProfile {
    let headings = [
        "### 10.1 Public contract and stage exclusions",
        "### 10.2 Contract delta activation",
        "### 10.3 Recovery authority regression additions",
    ];
    assert_profile_headings_are_exact_and_ordered(source, &headings, "Recovery");
    let exclusions = strict_profile_table(
        source,
        headings[0],
        &["Kind", "Canonical path or normalized item", "First slice"],
        "Recovery",
    );
    let contract_deltas = strict_profile_table(
        source,
        headings[1],
        &["Item", "Owning slice", "State", "Claim boundary"],
        "Recovery",
    )
    .into_iter()
    .map(|row| StagedProfileRow {
        item: row.0[0].clone(),
        owning_slice: row.0[1].clone(),
        state: row.0[2].clone(),
        claim_boundary: Some(row.0[3].clone()),
    })
    .collect::<Vec<_>>();
    let authority_additions = strict_profile_table(
        source,
        headings[2],
        &["Test function", "Owning slice", "State"],
        "Recovery",
    )
    .into_iter()
    .map(|row| StagedProfileRow {
        item: row.0[0].clone(),
        owning_slice: row.0[1].clone(),
        state: row.0[2].clone(),
        claim_boundary: None,
    })
    .collect::<Vec<_>>();
    let profile = HistoryContractProfile {
        exclusions,
        contract_deltas,
        authority_additions,
    };
    validate_recovery_contract_profile(&profile);
    profile
}

pub fn stage_contract_profile() -> HistoryContractProfile {
    let history = history_contract_profile();
    let recovery = parse_recovery_contract_profile(&read_workspace(
        "docs/superpowers/specs/2026-08-12-recovery-design.md",
    ));
    let mut contract_deltas = history.contract_deltas;
    contract_deltas.extend(recovery.contract_deltas);
    let mut authority_additions = history
        .authority_additions
        .into_iter()
        .filter(|row| row.state == "active")
        .collect::<Vec<_>>();
    authority_additions.extend(
        recovery
            .authority_additions
            .into_iter()
            .filter(|row| row.state == "active"),
    );
    let profile = HistoryContractProfile {
        exclusions: recovery.exclusions,
        contract_deltas,
        authority_additions,
    };
    profile.assert_disjoint_keys();
    profile
}

fn strict_history_table(input: &str, heading: &str, expected_header: &[&str]) -> Vec<TableRow> {
    strict_profile_table(input, heading, expected_header, "History")
}

fn assert_profile_headings_are_exact_and_ordered(source: &str, headings: &[&str], profile: &str) {
    let lines = source.lines().collect::<Vec<_>>();
    let positions = headings
        .iter()
        .map(|heading| {
            let matches = lines
                .iter()
                .enumerate()
                .filter_map(|(index, line)| (*line == *heading).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "{profile} table heading {heading} must occur exactly once"
            );
            matches[0]
        })
        .collect::<Vec<_>>();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{profile} table headings must occur in canonical order"
    );
}

fn strict_profile_table(
    input: &str,
    heading: &str,
    expected_header: &[&str],
    profile: &str,
) -> Vec<TableRow> {
    let lines = input.lines().collect::<Vec<_>>();
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (*line == heading).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        headings.len(),
        1,
        "{profile} table heading {heading} must occur exactly once"
    );

    let heading_level = markdown_heading_level(heading).expect("profile heading must be valid");
    let section_end = (headings[0] + 1..lines.len())
        .find(|index| {
            markdown_heading_level(lines[*index]).is_some_and(|level| level <= heading_level)
        })
        .unwrap_or(lines.len());
    let section = &lines[headings[0] + 1..section_end];
    let table_start = section
        .iter()
        .position(|line| line.trim_start().starts_with('|'))
        .unwrap_or_else(|| panic!("{profile} table {heading} must exist in its own section"));
    let table_end = table_start
        + section[table_start..]
            .iter()
            .take_while(|line| line.trim_start().starts_with('|'))
            .count();
    assert!(
        !section[table_end..]
            .iter()
            .any(|line| line.trim_start().starts_with('|')),
        "{profile} section {heading} must contain one contiguous pipe-table block"
    );
    let table = &section[table_start..table_end];

    let columns = expected_header.len();
    let expected_header = format!("| {} |", expected_header.join(" | "));
    let expected_separator = format!(
        "| {} |",
        std::iter::repeat("---")
            .take(columns)
            .collect::<Vec<_>>()
            .join(" | ")
    );
    assert_eq!(
        table.first().copied(),
        Some(expected_header.as_str()),
        "{profile} table {heading} must use its exact header"
    );
    assert_eq!(
        table.get(1).copied(),
        Some(expected_separator.as_str()),
        "{profile} table {heading} must use its exact separator"
    );

    let mut rows = Vec::new();
    for line in &table[2..] {
        let cells = parse_pipe_row(line);
        assert_eq!(
            cells.len(),
            columns,
            "{profile} table {heading} row width drifted"
        );
        rows.push(TableRow(cells));
    }
    assert!(
        !rows.is_empty(),
        "{profile} table {heading} must not be empty"
    );
    let unique = rows.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        rows.len(),
        "{profile} table {heading} must not contain duplicate rows"
    );
    rows
}

fn markdown_heading_level(line: &str) -> Option<usize> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (level > 0 && line.chars().nth(level) == Some(' ')).then_some(level)
}

fn validate_recovery_contract_profile(profile: &HistoryContractProfile) {
    validate_recovery_exclusions(&profile.exclusions);
    profile.assert_disjoint_keys();
    validate_recovery_rows(
        &profile.contract_deltas,
        true,
        &[
            (
                "boundary-bound error presentation complete evidence",
                "R1 Error presentation",
                "active",
                "Journey D public contract PASS",
            ),
            (
                "post-commit compound settlement complete evidence",
                "R2 Compound settlement",
                "active",
                "Journey G post-commit foreground rows PASS",
            ),
            (
                "host re-admission fresh-epoch complete evidence",
                "R3 Re-admission/stale output",
                "planned",
                "Journey G foreground re-admission rows PASS",
            ),
        ],
    );
    validate_recovery_rows(
        &profile.authority_additions,
        false,
        &[
            (
                "error_outlet_role_classification_is_preserved",
                "R1 Error presentation",
                "active",
                "",
            ),
            (
                "post_commit_host_loss_preserves_committed_history",
                "R2 Compound settlement",
                "active",
                "",
            ),
            (
                "host_reacceptance_detaches_old_top_slot",
                "R3 Re-admission/stale output",
                "planned",
                "",
            ),
        ],
    );
    assert_eq!(
        profile
            .authority_additions
            .iter()
            .filter(|row| row.state == "active")
            .map(|row| row.item.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "error_outlet_role_classification_is_preserved",
            "post_commit_host_loss_preserves_committed_history",
        ]),
        "Recovery must activate exactly the completed R1 and R2 authority rows"
    );
}

fn validate_recovery_exclusions(exclusions: &[TableRow]) {
    let actual = exclusions
        .iter()
        .map(|row| {
            assert_eq!(row.0.len(), 3, "Recovery exclusion row width drifted");
            assert!(
                matches!(row.0[0].as_str(), "symbol" | "associated"),
                "unknown Recovery exclusion kind: {}",
                row.0[0]
            );
            assert!(
                row.0[1].starts_with("bevy_ui_router::"),
                "Recovery exclusion path is not canonical: {}",
                row.0[1]
            );
            (row.0[0].as_str(), row.0[1].as_str(), row.0[2].as_str())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual.len(),
        exclusions.len(),
        "Recovery exclusions must be unique"
    );
    let recovery_expected = BTreeSet::from([
        (
            "symbol",
            "bevy_ui_router::ecs::InputProbeResult",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineLeaseId",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineOwnerKind",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineOwnerSnapshot",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineSnapshot",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::RouterInputModality",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::RouterInputProbes",
            "Authority closure",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines",
            "Authority closure",
        ),
    ]);
    assert_eq!(
        actual, recovery_expected,
        "Recovery exclusions must match the canonical vocabulary"
    );
}

fn validate_recovery_rows(
    rows: &[StagedProfileRow],
    has_claim_boundary: bool,
    expected: &[(&str, &str, &str, &str)],
) {
    let actual = rows
        .iter()
        .map(|row| {
            assert!(
                !row.item.is_empty(),
                "Recovery staged item must not be empty"
            );
            assert!(
                matches!(row.state.as_str(), "planned" | "active"),
                "unknown Recovery state: {}",
                row.state
            );
            assert_eq!(
                row.claim_boundary.is_some(),
                has_claim_boundary,
                "Recovery claim-boundary shape drifted for {}",
                row.item
            );
            let boundary = row.claim_boundary.as_deref().unwrap_or("");
            assert!(
                !has_claim_boundary || !boundary.is_empty(),
                "Recovery claim boundary is empty"
            );
            (
                row.item.as_str(),
                row.owning_slice.as_str(),
                row.state.as_str(),
                boundary,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual.len(), rows.len(), "Recovery rows must be unique");
    assert_eq!(
        actual,
        expected.iter().copied().collect::<BTreeSet<_>>(),
        "Recovery staged rows must match the canonical vocabulary"
    );
}

fn validate_staged_rows(rows: &[StagedProfileRow], requires_claim_boundary: bool) {
    let owning_slices = BTreeSet::from(["Replace", "Back execution", "Back failure"]);
    for row in rows {
        assert!(
            !row.item.is_empty(),
            "staged profile item must not be empty"
        );
        assert!(
            owning_slices.contains(row.owning_slice.as_str()),
            "unknown History owning slice: {}",
            row.owning_slice
        );
        assert!(
            matches!(row.state.as_str(), "planned" | "active"),
            "unknown History stage state: {}",
            row.state
        );
        assert_eq!(
            row.claim_boundary.is_some(),
            requires_claim_boundary,
            "staged profile claim-boundary shape drifted for {}",
            row.item
        );
        if let Some(boundary) = &row.claim_boundary {
            assert!(
                !boundary.is_empty(),
                "staged profile claim boundary must not be empty for {}",
                row.item
            );
        }
    }
}

fn assert_active_contract_delta_is_evidenced(row: &StagedProfileRow) {
    let fixture = read_workspace("docs/facade-contract-fixture.md");
    let golden = read_workspace("docs/facade-associated-api-golden.txt");
    let journeys = read_workspace("docs/mvp-acceptance-journeys.md");
    for (evidence, label) in [(&fixture, "fixture"), (&golden, "golden")] {
        assert!(
            evidence.contains(&row.item),
            "active contract delta `{}` is absent from {label}",
            row.item
        );
    }
    let claim_boundary = row
        .claim_boundary
        .as_deref()
        .expect("contract delta must retain its claim boundary");
    assert_claim_boundary_and_item_share_section(&journeys, &row.item, claim_boundary);
}

fn assert_claim_boundary_and_item_share_section(markdown: &str, item: &str, claim_boundary: &str) {
    let occurrences = markdown.match_indices(claim_boundary).collect::<Vec<_>>();
    assert_eq!(
        occurrences.len(),
        1,
        "claim boundary `{claim_boundary}` must occur exactly once"
    );
    let claim_line = markdown[..occurrences[0].0]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let lines = markdown.lines().collect::<Vec<_>>();
    let heading = (0..=claim_line)
        .rev()
        .find(|index| lines[*index].starts_with('#'))
        .unwrap_or_else(|| {
            panic!("claim boundary `{claim_boundary}` must belong to a markdown heading section")
        });
    let section_end = (heading + 1..lines.len())
        .find(|index| lines[*index].starts_with('#'))
        .unwrap_or(lines.len());
    let section = lines[heading..section_end].join("\n");
    assert!(
        section.contains(item),
        "claim boundary `{claim_boundary}` and active item `{item}` must share one markdown evidence section"
    );
}

fn validate_active_history_authority_rows(rows: &[StagedProfileRow]) {
    let active = rows
        .iter()
        .filter(|row| row.state == "active")
        .collect::<Vec<_>>();
    if active.is_empty() {
        return;
    }
    let final_mvp = final_mvp_authority_vocabulary();
    let root = markdown_table(
        &read_workspace("docs/root-push-contract-profile.md"),
        "### 3.2 Root Authority Regression Catalogue",
        &["Test function"],
    );
    let nested = markdown_table(
        &read_workspace("docs/nested-push-contract-profile.md"),
        "### 7.1 Nested Authority Regression Additions",
        &["Test function"],
    );
    let existing = root
        .into_iter()
        .chain(nested)
        .map(|row| row.0[0].clone())
        .collect::<BTreeSet<_>>();
    for row in active {
        assert!(
            final_mvp.contains(&row.item),
            "active History authority row `{}` is absent from the final MVP vocabulary",
            row.item
        );
        assert!(
            !existing.contains(&row.item),
            "active History authority row `{}` overlaps Root/Nested",
            row.item
        );
    }
}

fn final_mvp_authority_vocabulary() -> BTreeSet<String> {
    let source = read_workspace("docs/mvp-acceptance-journeys.md");
    final_mvp_authority_vocabulary_from_source(&source)
}

fn final_mvp_authority_vocabulary_from_source(source: &str) -> BTreeSet<String> {
    let marker = "MVP gate必须精确等于以下完整集合：";
    assert_eq!(
        source.matches(marker).count(),
        1,
        "final MVP catalogue marker must occur exactly once"
    );
    let tail = source
        .split_once(marker)
        .expect("final MVP catalogue marker must exist")
        .1
        .trim_start_matches(|character| matches!(character, '\r' | '\n'));
    let fence = char::from(96).to_string().repeat(3);
    let opening = format!("{fence}text");
    let block = tail
        .strip_prefix(&opening)
        .expect("final MVP text code block must immediately follow its marker");
    let block = block
        .strip_prefix("\r\n")
        .or_else(|| block.strip_prefix('\n'))
        .expect("final MVP text code block must start on its own line");
    let block = block
        .split_once(&fence)
        .expect("final MVP text code block must terminate")
        .0;
    let ordered = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(
        !ordered.is_empty(),
        "final MVP authority vocabulary must not be empty"
    );
    let names = ordered.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        names.len(),
        ordered.len(),
        "final MVP authority vocabulary must not contain duplicates"
    );
    names
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SymbolRow {
    pub symbol: String,
    pub module: String,
    pub prelude: bool,
    pub feature: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GoldenRow {
    pub feature: String,
    pub path: String,
    pub owner: String,
    pub kind: String,
    pub api: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FacadeExport {
    module: String,
    symbol: String,
    feature: String,
    source: String,
}

pub fn facade_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn workspace_dir() -> PathBuf {
    facade_dir()
        .parent()
        .and_then(Path::parent)
        .expect("facade crate is nested under the workspace crates directory")
        .to_path_buf()
}

pub fn read_workspace(path: &str) -> String {
    fs::read_to_string(workspace_dir().join(path))
        .unwrap_or_else(|error| panic!("failed to read workspace file `{path}`: {error}"))
}

pub fn markdown_table(input: &str, heading: &str, expected_header: &[&str]) -> Vec<TableRow> {
    let lines = input.lines().collect::<Vec<_>>();
    let heading_index = lines
        .iter()
        .position(|line| line.trim() == heading)
        .unwrap_or_else(|| panic!("missing heading `{heading}`"));
    let header_index = (heading_index + 1..lines.len())
        .find(|index| lines[*index].trim_start().starts_with('|'))
        .unwrap_or_else(|| panic!("missing table after `{heading}`"));
    let header = parse_pipe_row(lines[header_index]);
    assert_eq!(
        header,
        expected_header
            .iter()
            .map(|cell| (*cell).to_owned())
            .collect::<Vec<_>>(),
        "table `{heading}` header drifted"
    );
    let separator = parse_pipe_row(lines[header_index + 1]);
    assert_eq!(
        separator.len(),
        header.len(),
        "table separator width drifted"
    );
    assert!(
        separator.iter().all(|cell| cell
            .chars()
            .all(|character| character == '-' || character == ':')),
        "table `{heading}` has an invalid separator"
    );

    let mut rows = Vec::new();
    for line in &lines[header_index + 2..] {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let cells = parse_pipe_row(line);
        assert_eq!(
            cells.len(),
            header.len(),
            "table `{heading}` row width drifted"
        );
        rows.push(TableRow(cells));
    }
    assert!(!rows.is_empty(), "table `{heading}` must not be empty");
    let unique = rows.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), rows.len(), "table `{heading}` has duplicates");
    rows
}

pub fn markdown_table_by_header(
    input: &str,
    expected_header: &[&str],
    occurrence: usize,
) -> Vec<TableRow> {
    let expected = expected_header
        .iter()
        .map(|cell| (*cell).to_owned())
        .collect::<Vec<_>>();
    let lines = input.lines().collect::<Vec<_>>();
    let header_indices = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (parse_pipe_row(line) == expected).then_some(index))
        .collect::<Vec<_>>();
    let header_index = *header_indices
        .get(occurrence)
        .unwrap_or_else(|| panic!("missing table header occurrence {occurrence}: {expected:?}"));
    let separator = parse_pipe_row(lines[header_index + 1]);
    assert_eq!(separator.len(), expected.len());
    let mut rows = Vec::new();
    for line in &lines[header_index + 2..] {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let cells = parse_pipe_row(line);
        assert_eq!(cells.len(), expected.len());
        rows.push(TableRow(cells));
    }
    assert!(!rows.is_empty(), "controlled table must not be empty");
    assert_eq!(
        rows.iter().cloned().collect::<BTreeSet<_>>().len(),
        rows.len(),
        "controlled table must not contain duplicates"
    );
    rows
}

fn parse_pipe_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().trim_matches('`').to_owned())
        .collect()
}

pub fn root_symbols(symbols: &[TableRow], exclusions: &[TableRow]) -> Vec<SymbolRow> {
    validate_exclusion_vocabulary(exclusions);
    let excluded = exclusions
        .iter()
        .filter(|row| row.0[0] == "symbol")
        .map(|row| row.0[1].clone())
        .collect::<BTreeSet<_>>();
    for path in &excluded {
        assert_eq!(
            symbols
                .iter()
                .filter(|row| format!("bevy_ui_router::{}::{}", row.0[1], row.0[0]) == *path)
                .count(),
            1,
            "symbol exclusion `{path}` must match exactly one allowlist row"
        );
    }
    let rows = symbols
        .iter()
        .map(|row| {
            assert!(matches!(row.0[2].as_str(), "yes" | "no"));
            SymbolRow {
                symbol: row.0[0].clone(),
                module: row.0[1].clone(),
                prelude: row.0[2] == "yes",
                feature: row.0[3].trim_matches('`').to_owned(),
            }
        })
        .filter(|row| {
            !excluded.contains(&format!("bevy_ui_router::{}::{}", row.module, row.symbol))
        })
        .collect::<Vec<_>>();
    let unique = rows.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        rows.len(),
        "allowed symbol rows must be unique"
    );
    rows
}

pub fn parse_golden(input: &str, exclusions: &[TableRow]) -> Vec<GoldenRow> {
    validate_exclusion_vocabulary(exclusions);
    let header = input.lines().take(4).collect::<Vec<_>>();
    assert_eq!(
        header,
        vec![
            "# bevy-ui-router-facade-associated-api-v1",
            "# columns: feature | facade canonical path | owner crate | item kind | normalized public API",
            "# sort: feature-rank(default,view,ecs,ecs-test-support,platform), canonical-path ASCII, source/fixture declaration order; compile-assert entries follow their associated source contract",
            "# normalization: Self names the canonical-path type; lifetimes are normalized to 'a; impl Trait syntax is preserved; omitted field records mean zero public fields; compile-assert records are compiler-verified requirements rather than source-enumerated impls; the sole allowed public unit/zero-field struct is bevy_ui_router::ecs::BevyUiRouterPlugin",
        ],
        "associated API golden metadata drifted"
    );
    let excluded_symbols = exclusions
        .iter()
        .filter(|row| row.0[0] == "symbol")
        .map(|row| row.0[1].clone())
        .collect::<BTreeSet<_>>();
    let excluded_associated = exclusions
        .iter()
        .filter(|row| row.0[0] == "associated")
        .map(|row| row.0[1].clone())
        .collect::<BTreeSet<_>>();
    assert!(
        exclusions
            .iter()
            .all(|row| matches!(row.0[0].as_str(), "symbol" | "associated"))
    );

    let mut raw_rows = Vec::new();
    for (line_number, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split(" | ").collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            5,
            "golden line {} must have exactly five columns",
            line_number + 1
        );
        assert!(matches!(
            fields[0],
            "default" | "view" | "ecs" | "ecs-test-support" | "platform"
        ));
        assert!(matches!(
            fields[2],
            "bevy_ui_router_core" | "bevy_ui_router_view" | "bevy_ui_router_ecs"
        ));
        assert!(matches!(
            fields[3],
            "struct"
                | "enum"
                | "trait"
                | "variant"
                | "method"
                | "trait-method"
                | "trait-impl"
                | "compile-assert"
        ));
        let row = GoldenRow {
            feature: fields[0].to_owned(),
            path: fields[1].to_owned(),
            owner: fields[2].to_owned(),
            kind: fields[3].to_owned(),
            api: fields[4].to_owned(),
        };
        let associated_path = method_name(&row).map(|method| format!("{}::{method}", row.path));
        raw_rows.push((row, associated_path));
    }
    let feature_rank = |feature: &str| match feature {
        "default" => 0,
        "view" => 1,
        "ecs" => 2,
        "ecs-test-support" => 3,
        "platform" => 4,
        _ => unreachable!(),
    };
    for pair in raw_rows.windows(2) {
        let before = &pair[0].0;
        let after = &pair[1].0;
        assert!(
            (feature_rank(&before.feature), &before.path)
                <= (feature_rank(&after.feature), &after.path),
            "associated API golden sort drifted between '{}' and '{}'",
            before.path,
            after.path
        );
    }
    let kind_rank = |kind: &str| match kind {
        "struct" | "enum" | "trait" => 0,
        "variant" => 1,
        "method" | "trait-method" => 2,
        "trait-impl" => 3,
        "compile-assert" => 4,
        other => panic!("unknown golden order kind: {other}"),
    };
    let mut offset = 0;
    while offset < raw_rows.len() {
        let feature = &raw_rows[offset].0.feature;
        let path = &raw_rows[offset].0.path;
        let end = (offset + 1..raw_rows.len())
            .find(|index| {
                raw_rows[*index].0.feature != *feature || raw_rows[*index].0.path != *path
            })
            .unwrap_or(raw_rows.len());
        let group = &raw_rows[offset..end];
        for pair in group.windows(2) {
            assert!(
                kind_rank(&pair[0].0.kind) <= kind_rank(&pair[1].0.kind),
                "golden declaration/fixture order drifted within {path}: {} before {}",
                pair[0].0.kind,
                pair[1].0.kind
            );
        }
        let trait_impls = group
            .iter()
            .filter(|(row, _)| row.kind == "trait-impl")
            .map(|(row, _)| row.api.as_str())
            .collect::<Vec<_>>();
        assert!(
            trait_impls.windows(2).all(|pair| pair[0] <= pair[1]),
            "trait-impl fixture order drifted within {path}"
        );
        let compile_asserts = group
            .iter()
            .filter(|(row, _)| row.kind == "compile-assert")
            .map(|(row, _)| row.api.as_str())
            .collect::<Vec<_>>();
        assert!(
            compile_asserts.is_empty()
                || compile_asserts == ["Self: Send", "Self: Sync", "Self: 'static"],
            "compile-assert placement/order drifted within {path}: {compile_asserts:?}"
        );
        offset = end;
    }
    let required_error_bounds = BTreeSet::from([
        "bevy_ui_router::core::RouteError",
        "bevy_ui_router::core::RouteViewKeyError",
        "bevy_ui_router::view::RouteViewRegistryError",
        "bevy_ui_router::view::ViewDriverKeyError",
        "bevy_ui_router::ecs::ContentPrimitiveError",
        "bevy_ui_router::ecs::ContentWriteError",
        "bevy_ui_router::ecs::NavigationEnqueueError",
        "bevy_ui_router::ecs::RouterInstallError",
        "bevy_ui_router::ecs::ViewDriverRegistrationError",
    ]);
    let bounded_paths = raw_rows
        .iter()
        .filter(|(row, _)| row.kind == "compile-assert")
        .map(|(row, _)| row.path.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        bounded_paths, required_error_bounds,
        "required error compile-assert type set drifted"
    );
    for path in &excluded_associated {
        assert_eq!(
            raw_rows
                .iter()
                .filter(|(_, associated)| associated.as_deref() == Some(path.as_str()))
                .count(),
            1,
            "associated exclusion `{path}` must match exactly one golden method"
        );
    }
    let rows = raw_rows
        .into_iter()
        .filter_map(|(row, associated_path)| {
            (!excluded_symbols.contains(&row.path)
                && !associated_path
                    .as_ref()
                    .is_some_and(|path| excluded_associated.contains(path)))
            .then_some(row)
        })
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "golden must contain records");
    assert_eq!(
        rows.iter().cloned().collect::<BTreeSet<_>>().len(),
        rows.len(),
        "golden must not contain duplicate records"
    );
    rows
}

fn validate_exclusion_vocabulary(exclusions: &[TableRow]) {
    let slices = BTreeSet::from([
        "Authority closure",
        "History",
        "Nested push",
        "Recovery",
        "R1 Error presentation",
    ]);
    for row in exclusions {
        assert_eq!(row.0.len(), 3, "exclusion row width drifted");
        assert!(matches!(row.0[0].as_str(), "symbol" | "associated"));
        assert!(
            row.0[1].starts_with("bevy_ui_router::"),
            "exclusion path is not canonical: {}",
            row.0[1]
        );
        assert!(
            slices.contains(row.0[2].as_str()),
            "unknown First slice vocabulary: {}",
            row.0[2]
        );
    }
    let actual = exclusions
        .iter()
        .map(|row| (row.0[0].as_str(), row.0[1].as_str(), row.0[2].as_str()))
        .collect::<BTreeSet<_>>();
    let recovery_expected = BTreeSet::from([
        (
            "symbol",
            "bevy_ui_router::ecs::InputProbeResult",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineLeaseId",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineOwnerKind",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineOwnerSnapshot",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::QuarantineSnapshot",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::RouterInputModality",
            "Authority closure",
        ),
        (
            "symbol",
            "bevy_ui_router::ecs::RouterInputProbes",
            "Authority closure",
        ),
        (
            "associated",
            "bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines",
            "Authority closure",
        ),
    ]);
    let mut history_expected = recovery_expected.clone();
    history_expected.insert((
        "associated",
        "bevy_ui_router::core::RouteNodeOptions::with_error_boundary",
        "Recovery",
    ));
    history_expected.insert((
        "associated",
        "bevy_ui_router::core::RouteNodeOptions::with_error_view",
        "Recovery",
    ));
    assert!(
        actual == recovery_expected || actual == history_expected,
        "profile exclusion path/slice mapping drifted: {actual:?}"
    );
}

fn method_name(row: &GoldenRow) -> Option<String> {
    if !matches!(row.kind.as_str(), "method" | "trait-method") {
        return None;
    }
    let before_parameters = row.api.split('(').next()?;
    before_parameters
        .split_whitespace()
        .last()
        .map(|name| name.trim_start_matches("r#").to_owned())
}

pub fn assert_golden_covers_symbols(symbols: &[SymbolRow], golden: &[GoldenRow]) {
    let expected = symbols
        .iter()
        .map(|row| format!("bevy_ui_router::{}::{}", row.module, row.symbol))
        .collect::<BTreeSet<_>>();
    let actual = golden
        .iter()
        .map(|row| row.path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "golden canonical type paths drifted");
    for row in golden {
        let symbol = symbols
            .iter()
            .find(|symbol| {
                row.path == format!("bevy_ui_router::{}::{}", symbol.module, symbol.symbol)
            })
            .expect("golden path was checked above");
        assert_eq!(
            row.feature, symbol.feature,
            "golden feature drifted for {}",
            row.path
        );
    }
}

pub fn assert_facade_exports(source: &str, symbols: &[SymbolRow]) {
    let file = syn::parse_file(source).expect("facade source must parse");
    let mut actual = BTreeSet::new();
    for item in &file.items {
        if let syn::Item::Mod(module) = item
            && matches!(module.vis, syn::Visibility::Public(_))
        {
            collect_module_exports(module, None, &module.ident.to_string(), &mut actual);
        }
    }

    let source_scopes = project_source_scopes();
    let public_modules = owner_public_modules();
    let mut expected = BTreeSet::new();
    for row in symbols {
        expected.insert(FacadeExport {
            module: row.module.clone(),
            symbol: row.symbol.clone(),
            feature: row.feature.clone(),
            source: owner_source_path(&row.module, &row.symbol, &source_scopes, &public_modules),
        });
        if row.prelude {
            expected.insert(FacadeExport {
                module: "prelude".to_owned(),
                symbol: row.symbol.clone(),
                feature: row.feature.clone(),
                source: format!("crate::{}::{}", row.module, row.symbol),
            });
        }
    }
    assert_eq!(actual, expected, "facade public re-export set drifted");
}

fn collect_module_exports(
    module: &syn::ItemMod,
    inherited_feature: Option<String>,
    module_path: &str,
    exports: &mut BTreeSet<FacadeExport>,
) {
    let module_feature = cfg_feature(&module.attrs).or(inherited_feature);
    let Some((_, items)) = &module.content else {
        return;
    };
    for item in items {
        match item {
            syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                let feature = cfg_feature(&item.attrs)
                    .or_else(|| module_feature.clone())
                    .unwrap_or_else(|| "default".to_owned());
                let mut leaves = Vec::new();
                collect_use_leaves(&item.tree, &mut Vec::new(), &mut leaves);
                for (source, symbol) in leaves {
                    let source = normalized_facade_source(module_path, &source, &symbol);
                    exports.insert(FacadeExport {
                        module: module_path.to_owned(),
                        symbol,
                        feature: feature.clone(),
                        source,
                    });
                }
            }
            syn::Item::Mod(child) if matches!(child.vis, syn::Visibility::Public(_)) => {
                collect_module_exports(
                    child,
                    module_feature.clone(),
                    &format!("{module_path}::{}", child.ident),
                    exports,
                );
            }
            _ => {}
        }
    }
}

fn normalized_facade_source(module: &str, source: &str, symbol: &str) -> String {
    let expected_root = match module {
        "core" => "bevy_ui_router_core",
        "view" => "bevy_ui_router_view",
        "ecs" => "bevy_ui_router_ecs",
        "prelude" => "crate",
        "platform::prelude" => "crate",
        other => panic!("unknown facade export module: {other}"),
    };
    let segments = source.split("::").collect::<Vec<_>>();
    assert_eq!(
        segments.first().copied(),
        Some(expected_root),
        "facade `{module}::{symbol}` uses unexpected source `{source}`"
    );
    assert_eq!(
        segments.last().copied(),
        Some(symbol),
        "facade `{module}::{symbol}` source leaf is `{source}`"
    );
    if module == "prelude" {
        assert!(
            matches!(segments.get(1).copied(), Some("core" | "view" | "ecs")),
            "prelude source must route through a curated facade module: {source}"
        );
        format!("crate::{}::{symbol}", segments[1])
    } else if module == "platform::prelude" {
        assert_eq!(segments.get(1).copied(), Some("core"));
        format!("crate::core::{symbol}")
    } else {
        source.to_owned()
    }
}

fn owner_source_path(
    module: &str,
    symbol: &str,
    scopes: &ProjectSourceScopes,
    public_modules: &BTreeMap<String, BTreeSet<String>>,
) -> String {
    if module == "platform::prelude" {
        return format!("crate::core::{symbol}");
    }
    let owner = match module {
        "core" => "bevy_ui_router_core",
        "view" => "bevy_ui_router_view",
        "ecs" => "bevy_ui_router_ecs",
        other => panic!("unknown facade symbol module: {other}"),
    };
    let scope = &scopes.declarations[&(owner.to_owned(), symbol.to_owned())];
    if !scope.module.is_empty()
        && public_modules
            .get(owner)
            .is_some_and(|modules| modules.contains(&scope.module))
    {
        format!("{owner}::{}::{symbol}", scope.module)
    } else {
        format!("{owner}::{symbol}")
    }
}

fn owner_public_modules() -> BTreeMap<String, BTreeSet<String>> {
    let owners = [
        (
            "bevy_ui_router_core",
            "crates/bevy_ui_router_core/src/lib.rs",
        ),
        (
            "bevy_ui_router_view",
            "crates/bevy_ui_router_view/src/lib.rs",
        ),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src/lib.rs"),
    ];
    owners
        .into_iter()
        .map(|(owner, path)| {
            let parsed = syn::parse_file(&read_workspace(path)).unwrap();
            let modules = parsed
                .items
                .iter()
                .filter_map(|item| match item {
                    syn::Item::Mod(module) if is_public(&module.vis) => {
                        Some(module.ident.to_string())
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            (owner.to_owned(), modules)
        })
        .collect()
}

fn collect_use_leaves(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    leaves: &mut Vec<(String, String)>,
) {
    match tree {
        syn::UseTree::Name(name) => {
            let symbol = name.ident.to_string();
            let mut source = prefix.clone();
            source.push(symbol.clone());
            leaves.push((source.join("::"), symbol));
        }
        syn::UseTree::Rename(rename) => {
            let mut source = prefix.clone();
            source.push(rename.ident.to_string());
            leaves.push((source.join("::"), rename.rename.to_string()));
        }
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_leaves(&path.tree, prefix, leaves);
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_leaves(item, prefix, leaves);
            }
        }
        syn::UseTree::Glob(_) => panic!("public glob was rejected before export collection"),
    }
}

fn cfg_feature(attributes: &[syn::Attribute]) -> Option<String> {
    let mut feature = None;
    for attribute in attributes {
        assert!(
            !attribute.path().is_ident("cfg_attr"),
            "facade public API cannot use cfg_attr"
        );
        if !attribute.path().is_ident("cfg") {
            continue;
        }
        let predicate = attribute
            .parse_args::<syn::Meta>()
            .expect("facade cfg predicate must parse");
        let syn::Meta::NameValue(predicate) = predicate else {
            panic!("facade cfg must be an exact single feature predicate");
        };
        assert!(
            predicate.path.is_ident("feature"),
            "facade cfg must be an exact single feature predicate"
        );
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(value),
            ..
        }) = predicate.value
        else {
            panic!("facade feature cfg must use a string literal");
        };
        assert!(
            feature.replace(value.value()).is_none(),
            "facade public API cannot combine cfg predicates"
        );
    }
    feature
}

pub fn assert_devtools_adds_no_public_symbols(source: &str) {
    let file = syn::parse_file(source).expect("facade source must parse");
    for item in &file.items {
        if let syn::Item::Mod(module) = item
            && is_public(&module.vis)
        {
            assert_public_item_is_not_devtools(&module.attrs, &module.ident.to_string());
            assert_module_has_no_devtools_public_items(module, &module.ident.to_string());
        }
    }
}

fn assert_module_has_no_devtools_public_items(module: &syn::ItemMod, module_path: &str) {
    let Some((_, items)) = &module.content else {
        return;
    };
    for item in items {
        match item {
            syn::Item::Use(item) if is_public(&item.vis) => {
                assert_public_item_is_not_devtools(&item.attrs, module_path);
            }
            syn::Item::Mod(child) if is_public(&child.vis) => {
                let child_path = format!("{module_path}::{}", child.ident);
                assert_public_item_is_not_devtools(&child.attrs, &child_path);
                assert_module_has_no_devtools_public_items(child, &child_path);
            }
            item if public_definition_kind(item).is_some() => {
                assert_public_item_is_not_devtools(item_attributes(item), module_path);
            }
            _ => {}
        }
    }
}

fn assert_public_item_is_not_devtools(attributes: &[syn::Attribute], path: &str) {
    assert_ne!(
        cfg_feature(attributes).as_deref(),
        Some("devtools"),
        "devtools adds public API at `{path}`"
    );
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceRecord {
    owner: String,
    symbol: String,
    kind: String,
    api: String,
}

#[derive(Clone, Debug)]
struct SourceScope {
    module: String,
    imports: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct ProjectSourceScopes {
    declarations: BTreeMap<(String, String), SourceScope>,
    items: BTreeMap<SourceRecord, SourceScope>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicStructShape {
    Unit,
    Tuple { fields: usize, public_fields: usize },
    Named { fields: usize, public_fields: usize },
}

pub fn assert_owner_source_matches_golden(_symbols: &[SymbolRow], golden: &[GoldenRow]) {
    let canonical_by_owner_symbol = golden
        .iter()
        .map(|row| {
            (
                (row.owner.clone(), path_symbol(&row.path).to_owned()),
                row.path.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut actual = BTreeSet::new();
    let mut all_public_names = BTreeSet::new();
    for (owner, source_dir) in owners {
        let source_root = workspace_dir().join(source_dir);
        let parsed_sources = production_source_modules(&source_root)
            .into_iter()
            .map(|(file, module)| {
                let source = fs::read_to_string(file).unwrap();
                let parsed = syn::parse_file(&source).unwrap();
                (module, parsed)
            })
            .collect::<Vec<_>>();
        let mut aliases = BTreeSet::new();
        let mut index = ProductionScopeIndex::default();
        for (module, parsed) in &parsed_sources {
            collect_type_alias_names(&parsed.items, &mut aliases);
            collect_production_module_scopes(module, &parsed.items, &mut index);
        }
        for (module, parsed) in &parsed_sources {
            collect_owner_items_with_index(
                owner,
                &parsed.items,
                module,
                &index,
                &canonical_by_owner_symbol,
                &aliases,
                &mut actual,
                &mut all_public_names,
            );
        }
    }

    let expected = golden
        .iter()
        .filter(|row| row.kind != "compile-assert")
        .map(|row| SourceRecord {
            owner: row.owner.clone(),
            symbol: path_symbol(&row.path).to_owned(),
            kind: row.kind.clone(),
            api: compact(&row.api),
        })
        .collect::<BTreeSet<_>>();
    let actual = actual
        .into_iter()
        .filter(|row| {
            canonical_by_owner_symbol.contains_key(&(row.owner.clone(), row.symbol.clone()))
        })
        .collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
    let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "owner source differs from the filtered associated API golden\nmissing: {missing:#?}\nextra: {extra:#?}"
    );

    let project_types = project_type_catalogue();
    let source_scopes = project_source_scopes();
    assert_public_signature_closure(golden, &project_types, &source_scopes);
    assert_source_declaration_order(golden);
    let shapes = public_struct_shapes(golden);
    for row in golden.iter().filter(|row| row.kind == "struct") {
        let key = (row.owner.clone(), path_symbol(&row.path).to_owned());
        let shape = shapes
            .get(&key)
            .unwrap_or_else(|| panic!("missing public struct declaration for {}", row.path));
        if row.path == "bevy_ui_router::ecs::BevyUiRouterPlugin" {
            assert_eq!(*shape, PublicStructShape::Unit);
            continue;
        }
        match shape {
            PublicStructShape::Unit => panic!("unexpected public unit struct: {}", row.path),
            PublicStructShape::Tuple {
                fields,
                public_fields,
            }
            | PublicStructShape::Named {
                fields,
                public_fields,
            } => {
                assert!(*fields > 0, "opaque struct has no fields: {}", row.path);
                assert_eq!(
                    *public_fields, 0,
                    "allowed struct exposes public fields: {}",
                    row.path
                );
            }
        }
    }
}

fn assert_public_signature_closure(
    golden: &[GoldenRow],
    project_types: &BTreeMap<String, BTreeSet<(String, String, bool)>>,
    source_scopes: &ProjectSourceScopes,
) {
    let allowed_features = golden
        .iter()
        .map(|row| {
            (
                (row.owner.clone(), path_symbol(&row.path).to_owned()),
                row.feature.clone(),
            )
        })
        .fold(
            BTreeMap::<_, BTreeSet<_>>::new(),
            |mut map, (key, feature)| {
                map.entry(key).or_default().insert(feature);
                map
            },
        );
    for row in golden {
        for identifier in signature_identifiers(row)
            .into_iter()
            .filter(|identifier| identifier != "Self")
        {
            let source_record = SourceRecord {
                owner: row.owner.clone(),
                symbol: path_symbol(&row.path).to_owned(),
                kind: row.kind.clone(),
                api: compact(&row.api),
            };
            let scope = source_scopes.items.get(&source_record).unwrap_or_else(|| {
                panic!(
                    "missing unique source scope for {} {} '{}'",
                    row.path, row.kind, row.api
                )
            });
            let Some((resolved_identifier, resolved)) =
                resolve_project_type(&row.owner, scope, &row.api, &identifier, project_types)
            else {
                continue;
            };
            assert_eq!(
                resolved.len(),
                1,
                "public {} signature '{}' has ambiguous or noncanonical project type `{identifier}` candidates {resolved:?}",
                row.path,
                row.api
            );
            assert!(
                resolved.iter().all(|(owner, _, public)| {
                    *public
                        && allowed_features
                            .get(&(owner.clone(), resolved_identifier.clone()))
                            .is_some_and(|features| {
                                features
                                    .iter()
                                    .any(|feature| feature_is_visible_from(feature, &row.feature))
                            })
                }),
                "public {} signature '{}' reaches private, noncanonical, or stronger-feature project type `{identifier}` resolved as {resolved:?}",
                row.path,
                row.api
            );
        }
    }
}

fn assert_source_declaration_order(golden: &[GoldenRow]) {
    let canonical = golden
        .iter()
        .map(|row| (row.owner.clone(), path_symbol(&row.path).to_owned()))
        .collect::<BTreeSet<_>>();
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut actual = BTreeMap::<(String, String, String), Vec<String>>::new();
    for (owner, source_dir) in owners {
        let source_root = workspace_dir().join(source_dir);
        let parsed_sources = production_source_modules(&source_root)
            .into_iter()
            .map(|(file, module)| {
                (
                    module,
                    syn::parse_file(&fs::read_to_string(&file).unwrap()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let mut index = ProductionScopeIndex::default();
        for (module, parsed) in &parsed_sources {
            collect_production_module_scopes(module, &parsed.items, &mut index);
        }
        for (module, parsed) in &parsed_sources {
            collect_ordered_source_api_with_index(
                owner,
                &parsed.items,
                module,
                &index,
                &canonical,
                &mut actual,
            );
        }
    }
    let mut checked = BTreeSet::new();
    for row in golden
        .iter()
        .filter(|row| matches!(row.kind.as_str(), "variant" | "method" | "trait-method"))
    {
        let key = (
            row.owner.clone(),
            path_symbol(&row.path).to_owned(),
            row.kind.clone(),
        );
        if !checked.insert((row.path.clone(), row.kind.clone())) {
            continue;
        }
        let expected = golden
            .iter()
            .filter(|candidate| {
                candidate.path == row.path
                    && candidate.owner == row.owner
                    && candidate.kind == row.kind
            })
            .map(|candidate| compact(&candidate.api))
            .collect::<Vec<_>>();
        assert_eq!(
            actual.get(&key).cloned().unwrap_or_default(),
            expected,
            "source declaration order drifted for {} {}",
            row.path,
            row.kind
        );
    }
}

fn collect_ordered_source_api(
    owner: &str,
    items: &[syn::Item],
    canonical: &BTreeSet<(String, String)>,
    records: &mut BTreeMap<(String, String, String), Vec<String>>,
) {
    let index = production_scope_index("", items);
    collect_ordered_source_api_with_index(owner, items, "", &index, canonical, records);
}

fn collect_ordered_source_api_with_index(
    owner: &str,
    items: &[syn::Item],
    module: &str,
    index: &ProductionScopeIndex,
    canonical: &BTreeSet<(String, String)>,
    records: &mut BTreeMap<(String, String, String), Vec<String>>,
) {
    use syn::__private::ToTokens;
    walk_production_items(items, module, index, &mut |item, scope| match item {
        syn::Item::Enum(item) if is_public(&item.vis) => {
            let symbol = item.ident.to_string();
            if canonical.contains(&(owner.to_owned(), symbol.clone())) {
                let key = (owner.to_owned(), symbol, "variant".to_owned());
                records.entry(key).or_default().extend(
                    item.variants
                        .iter()
                        .map(|variant| compact(&render_variant(variant))),
                );
            }
        }
        syn::Item::Trait(item) if is_public(&item.vis) => {
            reject_trait_item_macros(item);
            let symbol = item.ident.to_string();
            if canonical.contains(&(owner.to_owned(), symbol.clone())) {
                let key = (owner.to_owned(), symbol, "trait-method".to_owned());
                records
                    .entry(key)
                    .or_default()
                    .extend(item.items.iter().filter_map(|item| match item {
                        syn::TraitItem::Fn(method) => {
                            Some(compact(&method.sig.to_token_stream().to_string()))
                        }
                        _ => None,
                    }));
            }
        }
        syn::Item::Impl(item) if item.trait_.is_none() => {
            reject_impl_aliases(item, &BTreeSet::new(), scope, index);
            let Some(symbol) = type_last_ident(&item.self_ty) else {
                return;
            };
            if canonical.contains(&(owner.to_owned(), symbol.clone())) {
                let key = (owner.to_owned(), symbol, "method".to_owned());
                records
                    .entry(key)
                    .or_default()
                    .extend(item.items.iter().filter_map(|item| match item {
                        syn::ImplItem::Fn(method) if is_public(&method.vis) => {
                            Some(compact(&format!(
                                "{}{}",
                                method.vis.to_token_stream(),
                                method.sig.to_token_stream()
                            )))
                        }
                        _ => None,
                    }));
            }
        }
        _ => {}
    });
}

fn render_variant(variant: &syn::Variant) -> String {
    use syn::__private::ToTokens;
    let mut rendered = variant.ident.to_string();
    match &variant.fields {
        syn::Fields::Unit => {}
        syn::Fields::Unnamed(fields) => {
            rendered.push('(');
            rendered.push_str(
                &fields
                    .unnamed
                    .iter()
                    .map(|field| compact(&field.ty.to_token_stream().to_string()))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            rendered.push(')');
        }
        syn::Fields::Named(fields) => {
            rendered.push('{');
            rendered.push_str(
                &fields
                    .named
                    .iter()
                    .map(|field| {
                        format!(
                            "{}:{}",
                            field.ident.as_ref().unwrap(),
                            compact(&field.ty.to_token_stream().to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            );
            rendered.push('}');
        }
    }
    rendered
}

fn signature_identifiers(row: &GoldenRow) -> BTreeSet<String> {
    let signature = match row.kind.as_str() {
        "method" | "trait-method" => row.api.as_str(),
        "variant" => row
            .api
            .find(|character| matches!(character, '(' | '{'))
            .map(|index| &row.api[index + 1..row.api.len() - 1])
            .unwrap_or(""),
        "trait" => row
            .api
            .split_once(':')
            .map(|(_, bounds)| bounds)
            .unwrap_or(""),
        "trait-impl" => row.api.as_str(),
        _ => "",
    };
    identifiers(signature).map(str::to_owned).collect()
}

fn feature_is_visible_from(referenced: &str, consumer: &str) -> bool {
    match consumer {
        "default" => referenced == "default",
        "view" => matches!(referenced, "default" | "view"),
        "ecs" => matches!(referenced, "default" | "view" | "ecs"),
        "ecs-test-support" => {
            matches!(referenced, "default" | "view" | "ecs" | "ecs-test-support")
        }
        "platform" => matches!(referenced, "default" | "platform"),
        other => panic!("unknown feature strength: {other}"),
    }
}

fn project_type_catalogue() -> BTreeMap<String, BTreeSet<(String, String, bool)>> {
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut catalogue = BTreeMap::<String, BTreeSet<(String, String, bool)>>::new();
    for (owner, source_dir) in owners {
        for (file, module) in production_source_modules(&workspace_dir().join(source_dir)) {
            let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
            collect_project_types(owner, &module, &parsed.items, &mut catalogue);
        }
    }
    catalogue
}

fn collect_project_types(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    catalogue: &mut BTreeMap<String, BTreeSet<(String, String, bool)>>,
) {
    for item in items {
        if item_is_test(item) {
            continue;
        }
        let definition = match item {
            syn::Item::Enum(item) => Some((&item.ident, &item.vis)),
            syn::Item::Struct(item) => Some((&item.ident, &item.vis)),
            syn::Item::Trait(item) => Some((&item.ident, &item.vis)),
            syn::Item::Type(item) => Some((&item.ident, &item.vis)),
            syn::Item::Union(item) => Some((&item.ident, &item.vis)),
            _ => None,
        };
        if let Some((identifier, visibility)) = definition {
            catalogue
                .entry(identifier.to_string())
                .or_default()
                .insert((owner.to_owned(), module.to_owned(), is_public(visibility)));
        }
        if let syn::Item::Mod(child) = item
            && let Some((_, nested)) = &child.content
        {
            let nested_module = module_ident(module, &child.ident.to_string());
            collect_project_types(owner, &nested_module, nested, catalogue);
        }
    }
}

fn source_module(source_root: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(source_root).unwrap();
    let mut parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let file_name = parts.pop().unwrap();
    let stem = file_name.trim_end_matches(".rs");
    if !matches!(stem, "lib" | "mod") {
        parts.push(stem.to_owned());
    }
    parts.join("::")
}

fn module_ident(module: &str, child: &str) -> String {
    if module.is_empty() {
        child.to_owned()
    } else {
        format!("{module}::{child}")
    }
}

fn project_source_scopes() -> ProjectSourceScopes {
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut sources = Vec::new();
    for (owner, source_dir) in owners {
        let source_root = workspace_dir().join(source_dir);
        for (file, module) in production_source_modules(&source_root) {
            sources.push((owner, module, file));
        }
    }
    let mut scopes = ProjectSourceScopes::default();
    let mut indexes = BTreeMap::<&str, ProductionScopeIndex>::new();
    for (owner, module, file) in &sources {
        let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
        collect_production_module_scopes(module, &parsed.items, indexes.entry(owner).or_default());
        collect_source_declarations(owner, module, &parsed.items, &mut scopes);
    }
    for (owner, module, file) in &sources {
        let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
        collect_source_item_scopes_with_index(
            owner,
            module,
            &parsed.items,
            indexes.get(owner).unwrap(),
            &mut scopes,
        );
    }
    scopes
}

fn collect_source_scopes(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    scopes: &mut ProjectSourceScopes,
) {
    collect_source_declarations(owner, module, items, scopes);
    collect_source_item_scopes(owner, module, items, scopes);
}

fn collect_source_declarations(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    scopes: &mut ProjectSourceScopes,
) {
    let imports = module_imports(items);
    for item in items {
        if item_is_test(item) {
            continue;
        }
        let definition = match item {
            syn::Item::Enum(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Struct(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Trait(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Type(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            syn::Item::Union(item) if is_public(&item.vis) => Some(item.ident.to_string()),
            _ => None,
        };
        if let Some(symbol) = definition {
            let key = (owner.to_owned(), symbol);
            assert!(
                scopes
                    .declarations
                    .insert(
                        key.clone(),
                        SourceScope {
                            module: module.to_owned(),
                            imports: imports.clone(),
                        },
                    )
                    .is_none(),
                "duplicate public type declaration scope: {key:?}"
            );
        }
        if let syn::Item::Mod(child) = item
            && let Some((_, nested)) = &child.content
        {
            collect_source_declarations(
                owner,
                &module_ident(module, &child.ident.to_string()),
                nested,
                scopes,
            );
        }
    }
}

fn collect_source_item_scopes(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    scopes: &mut ProjectSourceScopes,
) {
    let index = production_scope_index(module, items);
    collect_source_item_scopes_with_index(owner, module, items, &index, scopes);
}

fn collect_source_item_scopes_with_index(
    owner: &str,
    module: &str,
    items: &[syn::Item],
    index: &ProductionScopeIndex,
    scopes: &mut ProjectSourceScopes,
) {
    use syn::__private::ToTokens;
    walk_production_items(items, module, index, &mut |item, lexical_scope| {
        let scope = SourceScope {
            module: lexical_scope.module.clone(),
            imports: lexical_scope.imports.clone(),
        };
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                insert_source_item_scope(
                    scopes,
                    source_record(
                        owner,
                        &symbol,
                        "struct",
                        &format!("pubstruct{symbol}{}", generic_parameters(&item.generics)),
                    ),
                    &scope,
                );
                collect_derive_source_scopes(owner, &symbol, &item.attrs, &scope, scopes);
                for field in &item.fields {
                    if is_public(&field.vis) {
                        insert_source_item_scope(
                            scopes,
                            source_record(
                                owner,
                                &symbol,
                                "field",
                                &field.to_token_stream().to_string(),
                            ),
                            &scope,
                        );
                    }
                }
            }
            syn::Item::Enum(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                insert_source_item_scope(
                    scopes,
                    source_record(
                        owner,
                        &symbol,
                        "enum",
                        &format!("pubenum{symbol}{}", generic_parameters(&item.generics)),
                    ),
                    &scope,
                );
                collect_derive_source_scopes(owner, &symbol, &item.attrs, &scope, scopes);
                for variant in &item.variants {
                    insert_source_item_scope(
                        scopes,
                        source_record(owner, &symbol, "variant", &render_variant(variant)),
                        &scope,
                    );
                }
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                reject_trait_item_macros(item);
                let symbol = item.ident.to_string();
                let bounds = if item.supertraits.is_empty() {
                    String::new()
                } else {
                    format!(":{}", item.supertraits.to_token_stream())
                };
                insert_source_item_scope(
                    scopes,
                    source_record(
                        owner,
                        &symbol,
                        "trait",
                        &format!("pubtrait{symbol}{bounds}"),
                    ),
                    &scope,
                );
                for trait_item in &item.items {
                    let record = match trait_item {
                        syn::TraitItem::Fn(method) => Some(source_record(
                            owner,
                            &symbol,
                            "trait-method",
                            &method.sig.to_token_stream().to_string(),
                        )),
                        syn::TraitItem::Const(value) => Some(source_record(
                            owner,
                            &symbol,
                            "associated-const",
                            &value.to_token_stream().to_string(),
                        )),
                        syn::TraitItem::Type(value) => Some(source_record(
                            owner,
                            &symbol,
                            "associated-type",
                            &value.to_token_stream().to_string(),
                        )),
                        _ => None,
                    };
                    if let Some(record) = record {
                        insert_source_item_scope(scopes, record, &scope);
                    }
                }
            }
            syn::Item::Impl(item) => {
                reject_impl_aliases(item, &BTreeSet::new(), lexical_scope, index);
                collect_impl_source_scopes(owner, item, &scope, scopes);
            }
            _ => {}
        }
    });
}

fn collect_impl_source_scopes(
    owner: &str,
    item: &syn::ItemImpl,
    scope: &SourceScope,
    scopes: &mut ProjectSourceScopes,
) {
    use syn::__private::ToTokens;
    let self_symbol = type_last_ident(&item.self_ty);
    if item.trait_.is_some() {
        let candidates = scopes
            .declarations
            .keys()
            .filter(|(candidate_owner, _)| candidate_owner == owner)
            .map(|(_, symbol)| symbol.clone())
            .collect::<BTreeSet<_>>();
        for symbol in trait_impl_related_symbols(item, &candidates) {
            insert_source_item_scope(
                scopes,
                source_record(
                    owner,
                    &symbol,
                    "trait-impl",
                    &normalized_trait_impl(item, &symbol),
                ),
                scope,
            );
        }
        return;
    }
    let Some(symbol) = self_symbol else {
        return;
    };
    if !scopes
        .declarations
        .contains_key(&(owner.to_owned(), symbol.clone()))
    {
        return;
    }
    for impl_item in &item.items {
        let record = match impl_item {
            syn::ImplItem::Fn(method) if is_public(&method.vis) => Some(source_record(
                owner,
                &symbol,
                "method",
                &format!(
                    "{}{}",
                    method.vis.to_token_stream(),
                    method.sig.to_token_stream()
                ),
            )),
            syn::ImplItem::Const(value) if is_public(&value.vis) => Some(source_record(
                owner,
                &symbol,
                "associated-const",
                &value.to_token_stream().to_string(),
            )),
            syn::ImplItem::Type(value) if is_public(&value.vis) => Some(source_record(
                owner,
                &symbol,
                "associated-type",
                &value.to_token_stream().to_string(),
            )),
            _ => None,
        };
        if let Some(record) = record {
            insert_source_item_scope(scopes, record, scope);
        }
    }
}

fn collect_derive_source_scopes(
    owner: &str,
    symbol: &str,
    attributes: &[syn::Attribute],
    scope: &SourceScope,
    scopes: &mut ProjectSourceScopes,
) {
    let mut records = BTreeSet::new();
    collect_derives(owner, symbol, attributes, &mut records);
    for record in records {
        insert_source_item_scope(scopes, record, scope);
    }
}

fn insert_source_item_scope(
    scopes: &mut ProjectSourceScopes,
    record: SourceRecord,
    scope: &SourceScope,
) {
    assert!(
        scopes.items.insert(record.clone(), scope.clone()).is_none(),
        "duplicate source scope for {record:?}"
    );
}

fn module_imports(items: &[syn::Item]) -> BTreeMap<String, String> {
    use std::collections::btree_map::Entry;

    let mut imports = BTreeMap::new();
    for item in items {
        if let syn::Item::Use(item) = item {
            let mut leaves = Vec::new();
            collect_internal_use_leaves(&item.tree, &mut Vec::new(), &mut leaves);
            for (source, name) in leaves {
                match imports.entry(name) {
                    Entry::Vacant(entry) => {
                        entry.insert(source);
                    }
                    Entry::Occupied(entry) => panic!(
                        "duplicate import alias `{}` in one source scope: `{}` and `{source}`",
                        entry.key(),
                        entry.get()
                    ),
                }
            }
        }
    }
    imports
}

fn collect_internal_use_leaves(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    leaves: &mut Vec<(String, String)>,
) {
    match tree {
        syn::UseTree::Name(name) => {
            let mut source = prefix.clone();
            source.push(name.ident.to_string());
            leaves.push((source.join("::"), name.ident.to_string()));
        }
        syn::UseTree::Rename(rename) => {
            let mut source = prefix.clone();
            source.push(rename.ident.to_string());
            leaves.push((source.join("::"), rename.rename.to_string()));
        }
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_internal_use_leaves(&path.tree, prefix, leaves);
            prefix.pop();
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_internal_use_leaves(tree, prefix, leaves);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn resolve_project_type(
    owner: &str,
    scope: &SourceScope,
    api: &str,
    identifier: &str,
    catalogue: &BTreeMap<String, BTreeSet<(String, String, bool)>>,
) -> Option<(String, Vec<(String, String, bool)>)> {
    for explicit_owner in [
        "bevy_ui_router_core",
        "bevy_ui_router_view",
        "bevy_ui_router_ecs",
    ] {
        if api.contains(&format!("{explicit_owner}::{identifier}"))
            && let Some(original_candidates) = catalogue.get(identifier)
        {
            return Some((
                identifier.to_owned(),
                original_candidates
                    .iter()
                    .filter(|(candidate_owner, _, _)| candidate_owner == explicit_owner)
                    .cloned()
                    .collect(),
            ));
        }
    }
    if let Some(import) = scope.imports.get(identifier) {
        let imported_identifier = import.rsplit("::").next().unwrap();
        let candidates = catalogue.get(imported_identifier)?;
        for imported_owner in [
            "bevy_ui_router_core",
            "bevy_ui_router_view",
            "bevy_ui_router_ecs",
        ] {
            if import.starts_with(imported_owner) {
                return Some((
                    imported_identifier.to_owned(),
                    candidates
                        .iter()
                        .filter(|(candidate_owner, _, _)| candidate_owner == imported_owner)
                        .cloned()
                        .collect(),
                ));
            }
        }
        let (module, root_reexport) =
            relative_import_module(&scope.module, import, imported_identifier);
        let exact = candidates
            .iter()
            .filter(|(candidate_owner, candidate_module, _)| {
                candidate_owner == owner && candidate_module == &module
            })
            .cloned()
            .collect::<Vec<_>>();
        if exact.is_empty()
            && !root_reexport
            && !matches!(import.split("::").next(), Some("crate" | "self" | "super"))
        {
            return None;
        }
        let resolved = if exact.is_empty() && root_reexport {
            candidates
                .iter()
                .filter(|(candidate_owner, _, public)| candidate_owner == owner && *public)
                .cloned()
                .collect()
        } else {
            exact
        };
        return Some((imported_identifier.to_owned(), resolved));
    }
    let original_candidates = catalogue.get(identifier)?;
    let local = original_candidates
        .iter()
        .filter(|(candidate_owner, candidate_module, _)| {
            candidate_owner == owner && candidate_module == &scope.module
        })
        .cloned()
        .collect::<Vec<_>>();
    if local.is_empty() {
        Some((
            identifier.to_owned(),
            original_candidates
                .iter()
                .filter(|(_, _, public)| *public)
                .cloned()
                .collect(),
        ))
    } else {
        Some((identifier.to_owned(), local))
    }
}

fn relative_import_module(
    current_module: &str,
    import: &str,
    imported_identifier: &str,
) -> (String, bool) {
    let module_path = import
        .strip_suffix(imported_identifier)
        .and_then(|path| path.strip_suffix("::"))
        .unwrap_or("");
    let mut relative = module_path.split("::").filter(|part| !part.is_empty());
    let first = relative.next();
    let mut module = current_module
        .split("::")
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut remainder = Vec::new();
    let crate_absolute = first == Some("crate");
    match first {
        Some("crate") => module.clear(),
        Some("self") => {}
        Some("super") => {
            assert!(
                module.pop().is_some(),
                "import `{import}` traverses above the crate root"
            );
            while let Some(part) = relative.next() {
                if part != "super" {
                    remainder.push(part.to_owned());
                    break;
                }
                assert!(
                    module.pop().is_some(),
                    "import `{import}` traverses above the crate root"
                );
            }
        }
        Some(part) => remainder.push(part.to_owned()),
        None => {}
    }
    remainder.extend(relative.map(str::to_owned));
    module.extend(remainder);
    let module = module.join("::");
    let root_reexport = crate_absolute && (module.is_empty() || module == "prelude");
    (module, root_reexport)
}

fn public_struct_shapes(golden: &[GoldenRow]) -> BTreeMap<(String, String), PublicStructShape> {
    let wanted = golden
        .iter()
        .filter(|row| row.kind == "struct")
        .map(|row| (row.owner.clone(), path_symbol(&row.path).to_owned()))
        .collect::<BTreeSet<_>>();
    let owners = BTreeMap::from([
        ("bevy_ui_router_core", "crates/bevy_ui_router_core/src"),
        ("bevy_ui_router_view", "crates/bevy_ui_router_view/src"),
        ("bevy_ui_router_ecs", "crates/bevy_ui_router_ecs/src"),
    ]);
    let mut shapes = BTreeMap::new();
    for (owner, source_dir) in owners {
        let mut files = Vec::new();
        rust_files(&workspace_dir().join(source_dir), &mut files);
        for file in files {
            let source = fs::read_to_string(&file).unwrap();
            let parsed = syn::parse_file(&source).unwrap();
            collect_public_struct_shapes(owner, &parsed.items, &wanted, &mut shapes);
        }
    }
    assert_eq!(
        shapes.len(),
        wanted.len(),
        "allowed struct declaration set drifted"
    );
    shapes
}

fn collect_public_struct_shapes(
    owner: &str,
    items: &[syn::Item],
    wanted: &BTreeSet<(String, String)>,
    shapes: &mut BTreeMap<(String, String), PublicStructShape>,
) {
    for item in items {
        if item_is_test(item) {
            continue;
        }
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                let key = (owner.to_owned(), item.ident.to_string());
                if wanted.contains(&key) {
                    let shape = match &item.fields {
                        syn::Fields::Unit => PublicStructShape::Unit,
                        syn::Fields::Unnamed(fields) => PublicStructShape::Tuple {
                            fields: fields.unnamed.len(),
                            public_fields: fields
                                .unnamed
                                .iter()
                                .filter(|field| is_public(&field.vis))
                                .count(),
                        },
                        syn::Fields::Named(fields) => PublicStructShape::Named {
                            fields: fields.named.len(),
                            public_fields: fields
                                .named
                                .iter()
                                .filter(|field| is_public(&field.vis))
                                .count(),
                        },
                    };
                    assert!(
                        shapes.insert(key.clone(), shape).is_none(),
                        "duplicate public struct declaration: {key:?}"
                    );
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    collect_public_struct_shapes(owner, nested, wanted, shapes);
                }
            }
            _ => {}
        }
    }
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    files.extend(
        production_source_modules(directory)
            .into_iter()
            .map(|(file, _)| file)
            .collect::<BTreeSet<_>>(),
    );
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", directory.display()))
    {
        let path = entry
            .expect("source directory entry must be readable")
            .path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn production_source_modules(source_root: &Path) -> Vec<(PathBuf, String)> {
    let mut discovered = Vec::new();
    collect_rust_files(source_root, &mut discovered);
    let reachability = module_file_reachability(source_root);
    let registered_files = reachability
        .keys()
        .map(|(file, _)| file.clone())
        .collect::<BTreeSet<_>>();
    let mut sources = reachability
        .into_iter()
        .filter_map(|((file, module), reachable)| reachable.then_some((file, module)))
        .collect::<BTreeSet<_>>();
    for file in discovered {
        if !registered_files.contains(&file) {
            sources.insert((file.clone(), source_module(source_root, &file)));
        }
    }
    sources.into_iter().collect()
}

fn module_file_reachability(source_root: &Path) -> BTreeMap<(PathBuf, String), bool> {
    let root = source_root.join("lib.rs");
    if !root.is_file() {
        return BTreeMap::new();
    }
    let mut reachability = BTreeMap::new();
    let mut visited = BTreeSet::new();
    visit_module_file(&root, "", true, &mut reachability, &mut visited);
    reachability
}

fn visit_module_file(
    file: &Path,
    logical_module: &str,
    production_reachable: bool,
    reachability: &mut BTreeMap<(PathBuf, String), bool>,
    visited: &mut BTreeSet<(PathBuf, String, bool)>,
) {
    let file = file.to_path_buf();
    let key = (file.clone(), logical_module.to_owned());
    reachability
        .entry(key.clone())
        .and_modify(|reachable| *reachable |= production_reachable)
        .or_insert(production_reachable);
    if !visited.insert((
        file.clone(),
        logical_module.to_owned(),
        production_reachable,
    )) {
        return;
    }
    let source = fs::read_to_string(&file)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", file.display()));
    let parsed = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse '{}': {error}", file.display()));
    let module_dir = module_directory(&file);
    let path_attribute_base = file.parent().expect("Rust source file has a parent");
    visit_external_modules(
        &parsed.items,
        &module_dir,
        path_attribute_base,
        logical_module,
        production_reachable,
        reachability,
        visited,
    );
}

fn visit_external_modules(
    items: &[syn::Item],
    module_dir: &Path,
    path_attribute_base: &Path,
    logical_module: &str,
    production_reachable: bool,
    reachability: &mut BTreeMap<(PathBuf, String), bool>,
    visited: &mut BTreeSet<(PathBuf, String, bool)>,
) {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let child_reachable = production_reachable && !item_is_test(item);
        let child_module = module_ident(logical_module, &module.ident.to_string());
        if let Some((_, nested)) = &module.content {
            visit_external_modules(
                nested,
                &module_dir.join(module.ident.to_string()),
                &path_attribute_base.join(module.ident.to_string()),
                &child_module,
                child_reachable,
                reachability,
                visited,
            );
            continue;
        }
        let path_override = module.attrs.iter().find_map(|attribute| {
            if !attribute.path().is_ident("path") {
                return None;
            }
            let syn::Meta::NameValue(path) = &attribute.meta else {
                return None;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(path),
                ..
            }) = &path.value
            else {
                return None;
            };
            Some(path.value())
        });
        let resolved = if let Some(path) = path_override {
            path_attribute_base.join(path)
        } else {
            let direct = module_dir.join(format!("{}.rs", module.ident));
            let nested = module_dir.join(module.ident.to_string()).join("mod.rs");
            match (direct.is_file(), nested.is_file()) {
                (true, false) => direct,
                (false, true) => nested,
                (false, false) => continue,
                (true, true) => panic!(
                    "ambiguous module source for `{}`: '{}' and '{}'",
                    module.ident,
                    direct.display(),
                    nested.display()
                ),
            }
        };
        if resolved.is_file() {
            visit_module_file(
                &resolved,
                &child_module,
                child_reachable,
                reachability,
                visited,
            );
        }
    }
}

fn module_directory(file: &Path) -> PathBuf {
    let parent = file.parent().expect("Rust source file has a parent");
    match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "mod") => parent.to_path_buf(),
        Some(stem) => parent.join(stem),
        None => parent.to_path_buf(),
    }
}

#[derive(Clone, Debug, Default)]
struct ProductionBindings {
    aliases: BTreeSet<String>,
    binding_risks: BTreeSet<String>,
    declarations: BTreeSet<String>,
    imports: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct ProductionScopeIndex {
    modules: BTreeMap<String, ProductionBindings>,
}

#[derive(Clone, Debug, Default)]
struct ProductionScope {
    module: String,
    aliases: BTreeSet<String>,
    declarations: BTreeSet<String>,
    imports: BTreeMap<String, String>,
    ancestors: Vec<(String, ProductionBindings)>,
    binding_risks: BTreeSet<String>,
    safe_macros: BTreeSet<String>,
    block_depth: usize,
}

fn production_scope_index(module: &str, items: &[syn::Item]) -> ProductionScopeIndex {
    let mut index = ProductionScopeIndex::default();
    collect_production_module_scopes(module, items, &mut index);
    index
}

fn collect_production_module_scopes(
    module: &str,
    items: &[syn::Item],
    index: &mut ProductionScopeIndex,
) {
    let bindings = production_bindings(items.iter());
    assert!(
        index.modules.insert(module.to_owned(), bindings).is_none(),
        "duplicate production module scope `{module}`"
    );
    for item in items {
        if item_is_test(item) {
            continue;
        }
        if let syn::Item::Mod(child) = item
            && let Some((_, nested)) = &child.content
        {
            collect_production_module_scopes(
                &module_ident(module, &child.ident.to_string()),
                nested,
                index,
            );
        }
    }
}

fn production_bindings<'a>(items: impl IntoIterator<Item = &'a syn::Item>) -> ProductionBindings {
    let mut bindings = ProductionBindings::default();
    for item in items {
        if item_is_test(item) {
            continue;
        }
        if let Some(identifier) = item_identifier(item) {
            bindings.declarations.insert(identifier);
        }
        if let syn::Item::Use(item) = item {
            if use_tree_contains_glob(&item.tree) {
                bindings
                    .binding_risks
                    .insert("glob import leaves lexical authority unresolved".to_owned());
            }
            collect_explicit_use_aliases(&item.tree, &mut bindings.aliases);
            let mut leaves = Vec::new();
            collect_internal_use_leaves(&item.tree, &mut Vec::new(), &mut leaves);
            for (source, name) in leaves {
                bindings.imports.insert(name, source);
            }
        }
        if let syn::Item::ExternCrate(item) = item
            && item.rename.is_some()
        {
            bindings
                .binding_risks
                .insert("extern-crate rename leaves lexical authority unresolved".to_owned());
        }
    }
    bindings
}

fn use_tree_contains_glob(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Glob(_) => true,
        syn::UseTree::Group(group) => group.items.iter().any(use_tree_contains_glob),
        syn::UseTree::Path(path) => use_tree_contains_glob(&path.tree),
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) => false,
    }
}

fn item_identifier(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Enum(item) => Some(item.ident.to_string()),
        syn::Item::ExternCrate(item) => Some(item.ident.to_string()),
        syn::Item::Fn(item) => Some(item.sig.ident.to_string()),
        syn::Item::Mod(item) => Some(item.ident.to_string()),
        syn::Item::Static(item) => Some(item.ident.to_string()),
        syn::Item::Struct(item) => Some(item.ident.to_string()),
        syn::Item::Trait(item) => Some(item.ident.to_string()),
        syn::Item::TraitAlias(item) => Some(item.ident.to_string()),
        syn::Item::Type(item) => Some(item.ident.to_string()),
        syn::Item::Union(item) => Some(item.ident.to_string()),
        _ => None,
    }
}

fn module_production_scope(module: &str, index: &ProductionScopeIndex) -> ProductionScope {
    let bindings = index.modules.get(module).cloned().unwrap_or_default();
    ProductionScope {
        module: module.to_owned(),
        aliases: bindings.aliases,
        declarations: bindings.declarations,
        imports: bindings.imports,
        ancestors: Vec::new(),
        binding_risks: bindings.binding_risks,
        safe_macros: BTreeSet::new(),
        block_depth: 0,
    }
}

fn block_production_scope(block: &syn::Block, parent: &ProductionScope) -> ProductionScope {
    let bindings =
        production_bindings(block.stmts.iter().filter_map(|statement| match statement {
            syn::Stmt::Item(item) => Some(item),
            _ => None,
        }));
    let mut scope = parent.clone();
    scope.block_depth += 1;
    scope.binding_risks = bindings.binding_risks;
    for declaration in &bindings.declarations {
        scope.aliases.remove(declaration);
        scope.imports.remove(declaration);
    }
    scope.declarations.extend(bindings.declarations);
    for (name, source) in bindings.imports {
        scope.imports.insert(name, source);
    }
    scope.aliases.extend(bindings.aliases);
    scope
}

fn walk_production_items<'a>(
    items: &'a [syn::Item],
    module: &str,
    index: &ProductionScopeIndex,
    visitor: &mut impl FnMut(&'a syn::Item, &ProductionScope),
) {
    walk_production_items_with_risks(items, module, index, visitor, &mut |risk, scope| {
        panic!(
            "production source contains opaque macro risk in `{}`: {risk}",
            scope.module
        )
    });
}

fn walk_production_items_with_risks<'a>(
    items: &'a [syn::Item],
    module: &str,
    index: &ProductionScopeIndex,
    visitor: &mut impl FnMut(&'a syn::Item, &ProductionScope),
    risk_visitor: &mut impl FnMut(String, &ProductionScope),
) {
    let scope = module_production_scope(module, index);
    let mut walker = ProductionWalker {
        index,
        scopes: vec![scope],
        visitor,
        risk_visitor,
    };
    walker.report_scope_binding_risks();
    for item in items {
        walker.visit_item(item);
    }
}

struct ProductionWalker<'a, ItemVisitor, RiskVisitor> {
    index: &'a ProductionScopeIndex,
    scopes: Vec<ProductionScope>,
    visitor: &'a mut ItemVisitor,
    risk_visitor: &'a mut RiskVisitor,
}

impl<ItemVisitor, RiskVisitor> ProductionWalker<'_, ItemVisitor, RiskVisitor> {
    fn report_macro_risk(&mut self, risk: String)
    where
        RiskVisitor: FnMut(String, &ProductionScope),
    {
        let scope = self.scopes.last().expect("production scope exists").clone();
        (self.risk_visitor)(risk, &scope);
    }

    fn report_scope_binding_risks(&mut self)
    where
        RiskVisitor: FnMut(String, &ProductionScope),
    {
        let risks = self
            .scopes
            .last()
            .expect("production scope exists")
            .binding_risks
            .clone();
        for risk in risks {
            self.report_macro_risk(risk);
        }
    }

    fn visit_source_macro(&mut self, item: &syn::ItemMacro)
    where
        RiskVisitor: FnMut(String, &ProductionScope),
    {
        if item.ident.is_some() && item.mac.path.is_ident("macro_rules") {
            let name = item.ident.as_ref().unwrap().to_string();
            if !item.attrs.is_empty() || !local_macro_is_expression_only(item) {
                self.report_macro_risk(format!(
                    "local macro `{name}` is not structurally expression-only"
                ));
                return;
            }
            self.scopes
                .last_mut()
                .expect("production scope exists")
                .safe_macros
                .insert(name);
            return;
        }
        self.check_macro_invocation(&item.mac.path);
    }

    fn check_macro_invocation(&mut self, path: &syn::Path)
    where
        RiskVisitor: FnMut(String, &ProductionScope),
    {
        let scope = self.scopes.last().expect("production scope exists");
        if macro_invocation_is_trusted(path, scope) {
            return;
        }
        use syn::__private::ToTokens;
        self.report_macro_risk(format!(
            "unknown or shadowed macro invocation `{}`",
            path.to_token_stream()
        ));
    }
}

impl<'ast, ItemVisitor, RiskVisitor> Visit<'ast> for ProductionWalker<'_, ItemVisitor, RiskVisitor>
where
    ItemVisitor: FnMut(&'ast syn::Item, &ProductionScope),
    RiskVisitor: FnMut(String, &ProductionScope),
{
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if item_is_test(item) {
            return;
        }
        if let syn::Item::Macro(item) = item {
            self.visit_source_macro(item);
            return;
        }
        let scope = self.scopes.last().expect("production scope exists").clone();
        (self.visitor)(item, &scope);
        syn::visit::visit_item(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let Some((_, nested)) = &item.content else {
            syn::visit::visit_item_mod(self, item);
            return;
        };
        let parent = self.scopes.last().expect("production scope exists");
        let child_module = module_ident(&parent.module, &item.ident.to_string());
        let mut bindings = if parent.block_depth > 0 {
            production_bindings(nested.iter())
        } else {
            self.index
                .modules
                .get(&child_module)
                .cloned()
                .unwrap_or_else(|| production_bindings(nested.iter()))
        };
        let mut ancestors = parent.ancestors.clone();
        ancestors.push((
            parent.module.clone(),
            ProductionBindings {
                aliases: parent.aliases.clone(),
                binding_risks: parent.binding_risks.clone(),
                declarations: parent.declarations.clone(),
                imports: parent.imports.clone(),
            },
        ));
        let child_scope = ProductionScope {
            module: child_module,
            aliases: std::mem::take(&mut bindings.aliases),
            declarations: std::mem::take(&mut bindings.declarations),
            imports: std::mem::take(&mut bindings.imports),
            ancestors,
            binding_risks: std::mem::take(&mut bindings.binding_risks),
            safe_macros: BTreeSet::new(),
            block_depth: parent.block_depth,
        };
        self.scopes.push(child_scope);
        self.report_scope_binding_risks();
        syn::visit::visit_item_mod(self, item);
        self.scopes.pop();
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let parent = self.scopes.last().expect("production scope exists").clone();
        self.scopes.push(block_production_scope(block, &parent));
        self.report_scope_binding_risks();
        syn::visit::visit_block(self, block);
        self.scopes.pop();
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        self.check_macro_invocation(&invocation.path);
    }
}

fn local_macro_is_expression_only(item: &syn::ItemMacro) -> bool {
    let rendered = item.mac.tokens.to_string();
    if rendered.matches("=>").count() != 1 || rendered.contains(['$', '!']) {
        return false;
    }
    let forbidden = [
        "const", "enum", "extern", "fn", "impl", "item", "macro", "mod", "pub", "static", "struct",
        "trait", "type", "union", "use",
    ];
    if rendered
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .any(|token| forbidden.contains(&token))
    {
        return false;
    }
    let Some((_, expression)) = rendered.split_once("=>") else {
        return false;
    };
    syn::parse_str::<syn::Expr>(expression.trim().trim_end_matches(';').trim()).is_ok()
}

fn macro_invocation_is_trusted(path: &syn::Path, scope: &ProductionScope) -> bool {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let builtin = [
        "assert",
        "assert_eq",
        "assert_ne",
        "cfg",
        "column",
        "compile_error",
        "concat",
        "dbg",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "env",
        "file",
        "format",
        "format_args",
        "include_bytes",
        "include_str",
        "line",
        "matches",
        "module_path",
        "option_env",
        "panic",
        "stringify",
        "todo",
        "unimplemented",
        "unreachable",
        "vec",
        "write",
        "writeln",
    ];
    match segments.as_slice() {
        [name] if scope.safe_macros.contains(name) => true,
        [name] => {
            builtin.contains(&name.as_str())
                && !scope.imports.contains_key(name)
                && !scope.declarations.contains(name)
        }
        [root, name] if matches!(root.as_str(), "core" | "std") => {
            builtin.contains(&name.as_str())
                && !scope.imports.contains_key(root)
                && !scope.declarations.contains(root)
        }
        _ => false,
    }
}

fn collect_owner_items(
    owner: &str,
    items: &[syn::Item],
    canonical: &BTreeMap<(String, String), String>,
    type_aliases: &BTreeSet<String>,
    records: &mut BTreeSet<SourceRecord>,
    public_names: &mut BTreeSet<String>,
) {
    let index = production_scope_index("", items);
    collect_owner_items_with_index(
        owner,
        items,
        "",
        &index,
        canonical,
        type_aliases,
        records,
        public_names,
    );
}

fn collect_owner_items_with_index(
    owner: &str,
    items: &[syn::Item],
    module: &str,
    index: &ProductionScopeIndex,
    canonical: &BTreeMap<(String, String), String>,
    type_aliases: &BTreeSet<String>,
    records: &mut BTreeSet<SourceRecord>,
    public_names: &mut BTreeSet<String>,
) {
    use syn::__private::ToTokens;
    walk_production_items(items, module, index, &mut |item, scope| {
        reject_unexpanded_public_api_risks(item, scope);
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                public_names.insert(symbol.clone());
                if canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
                    records.insert(source_record(
                        owner,
                        &symbol,
                        "struct",
                        &format!("pubstruct{symbol}{}", generic_parameters(&item.generics)),
                    ));
                    collect_derives(owner, &symbol, &item.attrs, records);
                    for field in &item.fields {
                        if is_public(&field.vis) {
                            records.insert(source_record(
                                owner,
                                &symbol,
                                "field",
                                &field.to_token_stream().to_string(),
                            ));
                        }
                    }
                }
            }
            syn::Item::Enum(item) if is_public(&item.vis) => {
                let symbol = item.ident.to_string();
                public_names.insert(symbol.clone());
                if canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
                    records.insert(source_record(
                        owner,
                        &symbol,
                        "enum",
                        &format!("pubenum{symbol}{}", generic_parameters(&item.generics)),
                    ));
                    collect_derives(owner, &symbol, &item.attrs, records);
                    for variant in &item.variants {
                        let mut rendered = variant.ident.to_string();
                        match &variant.fields {
                            syn::Fields::Unit => {}
                            syn::Fields::Unnamed(fields) => {
                                rendered.push('(');
                                rendered.push_str(
                                    &fields
                                        .unnamed
                                        .iter()
                                        .map(|field| {
                                            compact(&field.ty.to_token_stream().to_string())
                                        })
                                        .collect::<Vec<_>>()
                                        .join(","),
                                );
                                rendered.push(')');
                            }
                            syn::Fields::Named(fields) => {
                                rendered.push('{');
                                rendered.push_str(
                                    &fields
                                        .named
                                        .iter()
                                        .map(|field| {
                                            format!(
                                                "{}:{}",
                                                field.ident.as_ref().unwrap(),
                                                compact(&field.ty.to_token_stream().to_string())
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .join(","),
                                );
                                rendered.push('}');
                            }
                        }
                        records.insert(source_record(owner, &symbol, "variant", &rendered));
                    }
                }
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                reject_trait_item_macros(item);
                let symbol = item.ident.to_string();
                public_names.insert(symbol.clone());
                if canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
                    let bounds = if item.supertraits.is_empty() {
                        String::new()
                    } else {
                        format!(":{}", item.supertraits.to_token_stream())
                    };
                    records.insert(source_record(
                        owner,
                        &symbol,
                        "trait",
                        &format!("pubtrait{symbol}{bounds}"),
                    ));
                    for trait_item in &item.items {
                        match trait_item {
                            syn::TraitItem::Fn(method) => {
                                records.insert(source_record(
                                    owner,
                                    &symbol,
                                    "trait-method",
                                    &method.sig.to_token_stream().to_string(),
                                ));
                            }
                            syn::TraitItem::Const(value) => {
                                records.insert(source_record(
                                    owner,
                                    &symbol,
                                    "associated-const",
                                    &value.to_token_stream().to_string(),
                                ));
                            }
                            syn::TraitItem::Type(value) => {
                                records.insert(source_record(
                                    owner,
                                    &symbol,
                                    "associated-type",
                                    &value.to_token_stream().to_string(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
            syn::Item::Impl(item) => {
                reject_impl_aliases(item, type_aliases, scope, index);
                collect_impl(owner, item, canonical, records);
            }
            syn::Item::Macro(item)
                if scope.block_depth > 0
                    && item.ident.is_some()
                    && !item
                        .attrs
                        .iter()
                        .any(|attribute| attribute.path().is_ident("macro_export")) => {}
            syn::Item::Macro(item) => panic!(
                "owner source macro `{}` can generate unenumerated public API",
                item.mac.path.to_token_stream()
            ),
            _ => {}
        }
    });
}

fn collect_impl(
    owner: &str,
    item: &syn::ItemImpl,
    canonical: &BTreeMap<(String, String), String>,
    records: &mut BTreeSet<SourceRecord>,
) {
    use syn::__private::ToTokens;
    let self_symbol = type_last_ident(&item.self_ty);
    if item.trait_.is_some() {
        let candidates = canonical
            .keys()
            .filter(|(candidate_owner, _)| candidate_owner == owner)
            .map(|(_, symbol)| symbol.clone())
            .collect::<BTreeSet<_>>();
        for symbol in trait_impl_related_symbols(item, &candidates) {
            records.insert(source_record(
                owner,
                &symbol,
                "trait-impl",
                &normalized_trait_impl(item, &symbol),
            ));
        }
        return;
    }
    let Some(symbol) = self_symbol else {
        return;
    };
    if !canonical.contains_key(&(owner.to_owned(), symbol.clone())) {
        return;
    }
    for impl_item in &item.items {
        match impl_item {
            syn::ImplItem::Fn(method) if is_public(&method.vis) => {
                let api = format!(
                    "{}{}",
                    method.vis.to_token_stream(),
                    method.sig.to_token_stream()
                );
                records.insert(source_record(owner, &symbol, "method", &api));
            }
            syn::ImplItem::Const(value) if is_public(&value.vis) => {
                records.insert(source_record(
                    owner,
                    &symbol,
                    "associated-const",
                    &value.to_token_stream().to_string(),
                ));
            }
            syn::ImplItem::Type(value) if is_public(&value.vis) => {
                records.insert(source_record(
                    owner,
                    &symbol,
                    "associated-type",
                    &value.to_token_stream().to_string(),
                ));
            }
            syn::ImplItem::Macro(value) => panic!(
                "macro `{}` inside impl for `{symbol}` can generate unenumerated associated API",
                value.mac.path.to_token_stream()
            ),
            _ => {}
        }
    }
}

fn collect_type_alias_names(items: &[syn::Item], aliases: &mut BTreeSet<String>) {
    for item in items {
        if item_is_test(item) {
            continue;
        }
        match item {
            syn::Item::Type(item) => {
                aliases.insert(item.ident.to_string());
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    collect_type_alias_names(nested, aliases);
                }
            }
            _ => {}
        }
    }
}

fn explicit_use_aliases(items: &[syn::Item]) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    for item in items {
        if item_is_test(item) {
            continue;
        }
        if let syn::Item::Use(item) = item {
            collect_explicit_use_aliases(&item.tree, &mut aliases);
        }
    }
    aliases
}

fn collect_explicit_use_aliases(tree: &syn::UseTree, aliases: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Rename(rename) if rename.rename != "_" => {
            aliases.insert(rename.rename.to_string());
        }
        syn::UseTree::Path(path) => collect_explicit_use_aliases(&path.tree, aliases),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_explicit_use_aliases(tree, aliases);
            }
        }
        _ => {}
    }
}

fn reject_impl_aliases(
    item: &syn::ItemImpl,
    type_aliases: &BTreeSet<String>,
    scope: &ProductionScope,
    index: &ProductionScopeIndex,
) {
    use syn::__private::ToTokens;
    let mut signature = item.self_ty.to_token_stream().to_string();
    if let Some((_, trait_path, _)) = &item.trait_ {
        signature.push_str(&trait_path.to_token_stream().to_string());
    }
    signature.push_str(&item.generics.to_token_stream().to_string());
    let signature = compact(&signature);
    if let Some(alias) = type_aliases
        .iter()
        .find(|alias| replace_identifier(&signature, alias, "__facade_alias__") != signature)
    {
        panic!(
            "impl signature uses type alias `{alias}`; alias-based impls are rejected fail closed"
        );
    }
    if let Some(alias) = scope
        .aliases
        .iter()
        .find(|alias| replace_identifier(&signature, alias, "__facade_alias__") != signature)
    {
        panic!(
            "impl signature uses explicit import alias `{alias}`; alias-based impls are rejected fail closed"
        );
    }
    let current = scope
        .module
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let reject_ancestor_aliases = |module: &str, bindings: &ProductionBindings| {
        let ancestor = module
            .split("::")
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if ancestor.len() >= current.len() || current[..ancestor.len()] != ancestor {
            return;
        }
        let distance = current.len() - ancestor.len();
        for alias in &bindings.aliases {
            let relative = format!("{}{}", "super::".repeat(distance), alias);
            let absolute = if module.is_empty() {
                format!("crate::{alias}")
            } else {
                format!("crate::{module}::{alias}")
            };
            if [relative, absolute]
                .iter()
                .any(|path| signature.contains(&compact(path)))
            {
                panic!(
                    "impl signature uses ancestor import alias `{module}::{alias}`; alias-based impls are rejected fail closed"
                );
            }
        }
    };
    for (module, bindings) in &scope.ancestors {
        reject_ancestor_aliases(module, bindings);
    }
    for (module, bindings) in &index.modules {
        reject_ancestor_aliases(module, bindings);
    }
}

fn reject_trait_item_macros(item: &syn::ItemTrait) {
    use syn::__private::ToTokens;
    if let Some(item) = item.items.iter().find_map(|item| match item {
        syn::TraitItem::Macro(item) => Some(item),
        _ => None,
    }) {
        panic!(
            "macro `{}` inside public trait can generate unenumerated associated API",
            item.mac.path.to_token_stream()
        );
    }
}

fn reject_unexpanded_public_api_risks(item: &syn::Item, scope: &ProductionScope) {
    let risks = unexpanded_public_api_risks_in_scope(item, scope);
    assert!(
        risks.is_empty(),
        "owner source contains unexpanded public API risk: {}",
        risks.join("; ")
    );
}

fn unexpanded_public_api_risks(item: &syn::Item) -> Vec<String> {
    unexpanded_public_api_risks_in_scope(item, &ProductionScope::default())
}

fn unexpanded_public_api_risks_in_scope(item: &syn::Item, scope: &ProductionScope) -> Vec<String> {
    use syn::__private::ToTokens;
    let mut risks = BTreeSet::new();
    let helpers = item_derive_helpers(item, scope, &mut risks);
    for attribute in item_attributes(item) {
        inspect_api_risk_meta(&attribute.meta, &helpers, scope, &mut risks);
    }
    match item {
        syn::Item::Enum(item) => {
            for variant in &item.variants {
                for attribute in &variant.attrs {
                    inspect_api_risk_meta(&attribute.meta, &helpers, scope, &mut risks);
                }
                for field in &variant.fields {
                    for attribute in &field.attrs {
                        inspect_api_risk_meta(&attribute.meta, &helpers, scope, &mut risks);
                    }
                }
            }
        }
        syn::Item::Struct(item) => {
            for field in &item.fields {
                for attribute in &field.attrs {
                    inspect_api_risk_meta(&attribute.meta, &helpers, scope, &mut risks);
                }
            }
        }
        syn::Item::Trait(item) => {
            for trait_item in &item.items {
                let attributes = match trait_item {
                    syn::TraitItem::Const(item) => &item.attrs,
                    syn::TraitItem::Fn(item) => &item.attrs,
                    syn::TraitItem::Macro(item) => &item.attrs,
                    syn::TraitItem::Type(item) => &item.attrs,
                    _ => continue,
                };
                for attribute in attributes {
                    inspect_api_risk_meta(&attribute.meta, &BTreeSet::new(), scope, &mut risks);
                }
            }
        }
        syn::Item::Impl(item) => {
            for impl_item in &item.items {
                let attributes = match impl_item {
                    syn::ImplItem::Const(item) => &item.attrs,
                    syn::ImplItem::Fn(item) => &item.attrs,
                    syn::ImplItem::Macro(item) => &item.attrs,
                    syn::ImplItem::Type(item) => &item.attrs,
                    _ => continue,
                };
                for attribute in attributes {
                    inspect_api_risk_meta(&attribute.meta, &BTreeSet::new(), scope, &mut risks);
                }
            }
        }
        _ => {}
    }
    if compact(&item.to_token_stream().to_string()).contains("#[macro_export]") {
        risks.insert("#[macro_export] can create root public API".to_owned());
    }
    risks.into_iter().collect()
}

fn item_derive_helpers(
    item: &syn::Item,
    scope: &ProductionScope,
    risks: &mut BTreeSet<String>,
) -> BTreeSet<String> {
    use syn::punctuated::Punctuated;
    use syn::token::Comma;
    let mut helpers = BTreeSet::new();
    for attribute in item_attributes(item) {
        if !attribute.path().is_ident("derive") {
            continue;
        }
        let derives =
            match attribute.parse_args_with(Punctuated::<syn::Path, Comma>::parse_terminated) {
                Ok(derives) => derives,
                Err(_) => {
                    risks.insert("unparseable derive may generate public API".to_owned());
                    continue;
                }
            };
        for derive in derives {
            let resolved = resolved_macro_path(&derive, scope);
            if matches!(
                resolved.as_str(),
                "Clone"
                    | "Copy"
                    | "Debug"
                    | "Default"
                    | "Eq"
                    | "Hash"
                    | "Ord"
                    | "PartialEq"
                    | "PartialOrd"
            ) {
                continue;
            }
            match resolved.as_str() {
                "bevy_ecs::component::Component" | "bevy_ecs::prelude::Component" => {
                    helpers.extend([
                        "component",
                        "entities",
                        "relationship",
                        "relationship_target",
                        "require",
                    ]);
                }
                "bevy_ecs::prelude::Resource" | "bevy_ecs::resource::Resource" => {
                    helpers.extend(["component", "require"]);
                }
                "bevy_ecs::prelude::SystemSet" | "bevy_ecs::schedule::SystemSet" => {}
                _ => {
                    risks.insert(format!(
                        "possible derive macro `#[derive({resolved})]` can generate public API"
                    ));
                }
            }
        }
    }
    helpers.into_iter().map(str::to_owned).collect()
}

fn resolved_macro_path(path: &syn::Path, scope: &ProductionScope) -> String {
    use syn::__private::ToTokens;
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let Some(first) = segments.first() else {
        return compact(&path.to_token_stream().to_string());
    };
    if let Some(import) = scope.imports.get(first) {
        return std::iter::once(import.clone())
            .chain(segments.into_iter().skip(1))
            .collect::<Vec<_>>()
            .join("::");
    }
    if scope.declarations.contains(first) {
        return std::iter::once("local".to_owned())
            .chain(segments)
            .collect::<Vec<_>>()
            .join("::");
    }
    compact(&path.to_token_stream().to_string())
}

fn inspect_api_risk_meta(
    meta: &syn::Meta,
    derive_helpers: &BTreeSet<String>,
    scope: &ProductionScope,
    risks: &mut BTreeSet<String>,
) {
    use syn::parse::Parser;
    use syn::punctuated::Punctuated;
    use syn::token::Comma;

    if meta.path().is_ident("macro_export") {
        risks.insert("#[macro_export] can create root public API".to_owned());
        return;
    }
    if meta.path().is_ident("cfg_attr") {
        let syn::Meta::List(list) = meta else {
            risks.insert("unparseable cfg_attr may contain an attribute macro".to_owned());
            return;
        };
        match Punctuated::<syn::Meta, Comma>::parse_terminated.parse2(list.tokens.clone()) {
            Ok(arguments) => {
                for attribute in arguments.iter().skip(1) {
                    inspect_api_risk_meta(attribute, derive_helpers, scope, risks);
                }
            }
            Err(_) => {
                risks.insert("unparseable cfg_attr may contain an attribute macro".to_owned());
            }
        }
        return;
    }
    let safe_builtin = [
        "allow", "cfg", "derive", "doc", "inline", "must_use", "path", "test",
    ]
    .iter()
    .any(|name| meta.path().is_ident(name));
    let safe_helper = meta
        .path()
        .get_ident()
        .is_some_and(|name| derive_helpers.contains(&name.to_string()));
    let safe = safe_builtin || safe_helper;
    if !safe {
        use syn::__private::ToTokens;
        risks.insert(format!(
            "possible attribute macro `#[{}]`",
            meta.path().to_token_stream()
        ));
    }
    let _ = scope;
}

fn collect_derives(
    owner: &str,
    symbol: &str,
    attributes: &[syn::Attribute],
    records: &mut BTreeSet<SourceRecord>,
) {
    use syn::punctuated::Punctuated;
    use syn::token::Comma;
    for attribute in attributes {
        if !attribute.path().is_ident("derive") {
            continue;
        }
        let derives = attribute
            .parse_args_with(Punctuated::<syn::Path, Comma>::parse_terminated)
            .expect("derive list must parse");
        for derive in derives {
            let name = derive.segments.last().unwrap().ident.to_string();
            let normalized = match name.as_str() {
                "Resource" => "bevy_ecs::resource::Resource",
                "Component" => "bevy_ecs::component::Component",
                "SystemSet" => "bevy_ecs::schedule::SystemSet",
                other => other,
            };
            records.insert(source_record(
                owner,
                symbol,
                "trait-impl",
                &format!("impl{normalized}forSelf"),
            ));
            if name == "Resource" {
                records.insert(source_record(
                    owner,
                    symbol,
                    "trait-impl",
                    "implbevy_ecs::component::ComponentforSelf",
                ));
            }
        }
    }
}

fn trait_impl_related_symbols(item: &syn::ItemImpl, candidates: &BTreeSet<String>) -> Vec<String> {
    use syn::__private::ToTokens;
    let (_, trait_path, _) = item
        .trait_
        .as_ref()
        .expect("trait impl record requires an explicit trait");
    let signature = compact(&format!(
        "{}|{}|{}",
        trait_path.to_token_stream(),
        item.self_ty.to_token_stream(),
        item.generics.to_token_stream()
    ));
    candidates
        .iter()
        .filter(|symbol| replace_identifier(&signature, symbol, "__facade_type__") != signature)
        .cloned()
        .collect()
}

fn normalized_trait_impl(item: &syn::ItemImpl, record_symbol: &str) -> String {
    use syn::__private::ToTokens;
    let (_, trait_path, _) = item
        .trait_
        .as_ref()
        .expect("trait impl normalization requires an explicit trait");
    let parameters = generic_parameters(&item.generics);
    let trait_path = normalized_trait_path(trait_path);
    let self_type = normalized_impl_self_type(&item.self_ty, record_symbol);
    let where_clause = item
        .generics
        .where_clause
        .as_ref()
        .map(|clause| {
            format!(
                "where{}",
                clause
                    .predicates
                    .iter()
                    .map(|predicate| compact(&predicate.to_token_stream().to_string()))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .unwrap_or_default();
    replace_identifier(
        &compact(&format!(
            "impl{parameters}{trait_path}for{self_type}{where_clause}"
        )),
        record_symbol,
        "Self",
    )
}

fn normalized_trait_path(path: &syn::Path) -> String {
    use syn::__private::ToTokens;
    let last = path.segments.last().unwrap();
    let leaf = last.ident.to_string();
    let mut base_path = path.clone();
    base_path.segments.last_mut().unwrap().arguments = syn::PathArguments::None;
    let base_path = compact(&base_path.to_token_stream().to_string());
    let base = match leaf.as_str() {
        "Error" if matches!(base_path.as_str(), "Error" | "std::error::Error") => {
            "std::error::Error"
        }
        "Plugin" if matches!(base_path.as_str(), "Plugin" | "bevy_app::Plugin") => {
            "bevy_app::Plugin"
        }
        "Resource"
            if matches!(
                base_path.as_str(),
                "Resource" | "bevy_ecs::resource::Resource"
            ) =>
        {
            "bevy_ecs::resource::Resource"
        }
        "Component"
            if matches!(
                base_path.as_str(),
                "Component" | "bevy_ecs::component::Component"
            ) =>
        {
            "bevy_ecs::component::Component"
        }
        "SystemSet"
            if matches!(
                base_path.as_str(),
                "SystemSet" | "bevy_ecs::schedule::SystemSet"
            ) =>
        {
            "bevy_ecs::schedule::SystemSet"
        }
        "Debug"
            if matches!(
                base_path.as_str(),
                "Debug" | "fmt::Debug" | "std::fmt::Debug"
            ) =>
        {
            "Debug"
        }
        "Display"
            if matches!(
                base_path.as_str(),
                "Display" | "fmt::Display" | "std::fmt::Display"
            ) =>
        {
            "Display"
        }
        _ => &base_path,
    };
    format!(
        "{base}{}",
        compact(&last.arguments.to_token_stream().to_string())
    )
}

fn normalized_impl_self_type(ty: &syn::Type, record_symbol: &str) -> String {
    use syn::__private::ToTokens;
    if let syn::Type::Path(path) = ty
        && path.qself.is_none()
        && path.path.segments.last().is_some_and(|segment| {
            segment.ident == record_symbol && matches!(segment.arguments, syn::PathArguments::None)
        })
    {
        return "Self".to_owned();
    }
    let normalized = compact(&ty.to_token_stream().to_string());
    if normalized == "App" {
        "bevy_app::App".to_owned()
    } else {
        normalized
    }
}

fn item_is_test(item: &syn::Item) -> bool {
    item_attributes(item).iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute.parse_args::<syn::Meta>().is_ok_and(
                |predicate| matches!(predicate, syn::Meta::Path(path) if path.is_ident("test")),
            )
    })
}

fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn source_record(owner: &str, symbol: &str, kind: &str, api: &str) -> SourceRecord {
    SourceRecord {
        owner: owner.to_owned(),
        symbol: symbol.to_owned(),
        kind: kind.to_owned(),
        api: compact(api),
    }
}

fn is_public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
}

fn type_last_ident(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn generic_parameters(generics: &syn::Generics) -> String {
    use syn::__private::ToTokens;
    if generics.params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            compact(&generics.params.to_token_stream().to_string())
        )
    }
}

fn path_symbol(path: &str) -> &str {
    path.rsplit("::")
        .next()
        .expect("canonical path has a symbol")
}

fn compact(input: &str) -> String {
    let mut compact = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    while compact.contains(",)") || compact.contains(",>") {
        compact = compact.replace(",)", ")").replace(",>", ">");
    }
    compact = compact
        .replace("(mutself", "(self")
        .replace(",mutself", ",self");
    if compact.contains("bevy_ecs::entity::Entity") {
        compact
    } else {
        replace_identifier(&compact, "Entity", "bevy_ecs::entity::Entity")
    }
}

fn replace_identifier(input: &str, from: &str, to: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut offset = 0;
    while let Some(relative) = input[offset..].find(from) {
        let start = offset + relative;
        let end = start + from.len();
        let before = input[..start].chars().next_back();
        let after = input[end..].chars().next();
        let boundary = |character: Option<char>| {
            character
                .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
        };
        output.push_str(&input[offset..start]);
        if boundary(before) && boundary(after) {
            output.push_str(to);
        } else {
            output.push_str(from);
        }
        offset = end;
    }
    output.push_str(&input[offset..]);
    output
}

fn identifiers(input: &str) -> impl Iterator<Item = &str> {
    input
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| token.chars().next().is_some_and(char::is_uppercase))
}

#[derive(Clone, Debug)]
pub struct ConsumerCase {
    pub kind: &'static str,
    pub feature_set: &'static str,
    pub slug: String,
    pub default_features: bool,
    pub features: Vec<String>,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct NegativeCase {
    pub consumer: ConsumerCase,
    pub expected_codes: Vec<&'static str>,
}

pub fn positive_consumer_cases(
    module_rows: &[TableRow],
    symbols: &[SymbolRow],
    golden: &[GoldenRow],
) -> Vec<ConsumerCase> {
    let configurations = [
        ("core-only", false, Vec::<String>::new(), vec!["default"]),
        (
            "default-enabled",
            true,
            Vec::<String>::new(),
            vec!["default", "view"],
        ),
        (
            "view-explicit",
            false,
            vec!["view".to_owned()],
            vec!["default", "view"],
        ),
        (
            "ecs",
            false,
            vec!["ecs".to_owned()],
            vec!["default", "view", "ecs"],
        ),
        (
            "test-support",
            false,
            vec!["test-support".to_owned()],
            vec!["default", "view", "ecs", "ecs-test-support"],
        ),
        (
            "bsn",
            false,
            vec!["bsn".to_owned()],
            vec!["default", "view", "ecs"],
        ),
        (
            "platform",
            false,
            vec!["platform".to_owned()],
            vec!["default", "platform"],
        ),
        (
            "devtools",
            false,
            vec!["devtools".to_owned()],
            vec!["default"],
        ),
    ];
    configurations
        .into_iter()
        .map(|(feature_set, default_features, features, visible)| {
            let visible = visible.into_iter().collect::<BTreeSet<_>>();
            let mut lines = vec!["#![allow(unused_imports)]".to_owned()];
            lines.push("use bevy_ui_router::core::*;".to_owned());
            if visible.contains("view") {
                lines.push("use bevy_ui_router::view::*;".to_owned());
            }
            if visible.contains("ecs") || visible.contains("ecs-test-support") {
                lines.push("use bevy_ui_router::ecs::*;".to_owned());
            }
            let mut index = 0usize;
            for row in module_rows {
                let module_feature = row.0[1].trim_matches(char::from(96));
                let path = &row.0[0];
                if visible.contains(module_feature)
                    || (feature_set == "bsn" && path == "bevy_ui_router::bsn")
                {
                    lines.push(format!("pub use {path} as UsedModule{index};"));
                    index += 1;
                }
            }
            for row in symbols {
                if visible.contains(row.feature.as_str()) {
                    let path = format!("bevy_ui_router::{}::{}", row.module, row.symbol);
                    lines.push(format!("pub use {path} as UsedSymbol{index};"));
                    index += 1;
                    if row.prelude {
                        lines.push(format!(
                            "pub use bevy_ui_router::prelude::{} as UsedPrelude{index};",
                            row.symbol
                        ));
                        index += 1;
                    }
                }
            }
            lines.push("fn contract_value<T>() -> T { panic!(\"compile-only value\") }".to_owned());
            for (golden_index, row) in golden.iter().enumerate() {
                if visible.contains(row.feature.as_str()) {
                    if matches!(row.kind.as_str(), "method" | "trait-method") {
                        lines.push(method_compile_probe(row, golden_index));
                    } else if row.kind == "compile-assert" {
                        lines.push(compile_assert_probe(row, golden_index));
                    }
                }
            }
            assert!(index > 0, "positive case must reference public paths");
            ConsumerCase {
                kind: "pass",
                feature_set,
                slug: "all-public-paths".to_owned(),
                default_features,
                features,
                source: lines.join("\n"),
            }
        })
        .collect()
}

fn method_compile_probe(row: &GoldenRow, index: usize) -> String {
    use syn::__private::ToTokens;

    let parsed = syn::parse_file(&format!(
        "struct ContractProbe; impl ContractProbe {{ {} {{ panic!(\"compile only\") }} }}",
        row.api
    ))
    .unwrap_or_else(|error| panic!("failed to parse golden method '{}': {error}", row.api));
    let method = match &parsed.items[1] {
        syn::Item::Impl(item) => match &item.items[0] {
            syn::ImplItem::Fn(method) => method,
            _ => panic!("golden associated record did not parse as a method"),
        },
        _ => panic!("golden associated record did not parse as an impl"),
    };
    let method_name = &method.sig.ident;
    let mut wrapper_generics = Vec::new();
    let target = if matches!(
        path_symbol(&row.path),
        "EcsViewReportSession" | "FrameContentWriter"
    ) {
        wrapper_generics.push("'a".to_owned());
        format!("{}<'a>", row.path)
    } else {
        row.path.clone()
    };
    for parameter in &method.sig.generics.params {
        wrapper_generics.push(parameter.to_token_stream().to_string());
    }
    let method_type_parameters = method
        .sig
        .generics
        .type_params()
        .map(|parameter| parameter.ident.to_string())
        .collect::<BTreeSet<_>>();

    let receiver = method.sig.inputs.iter().find_map(|input| match input {
        syn::FnArg::Receiver(receiver) => Some(receiver),
        syn::FnArg::Typed(_) => None,
    });
    if row.kind == "trait-method" {
        wrapper_generics.insert(0, format!("ContractReceiver: {}", row.path));
    }
    let generics = if wrapper_generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", wrapper_generics.join(", "))
    };
    let where_clause = method
        .sig
        .generics
        .where_clause
        .as_ref()
        .map(ToTokens::to_token_stream)
        .map(|tokens| tokens.to_string())
        .unwrap_or_default();
    let arguments = method
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(argument) => {
                let ty = compact(&argument.ty.to_token_stream().to_string());
                Some(if ty.starts_with("implInto<String>") {
                    "String::new()".to_owned()
                } else if method_type_parameters.contains(&ty) {
                    format!("contract_value::<{ty}>()")
                } else {
                    "contract_value()".to_owned()
                })
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let call = if receiver.is_some() {
        format!("contract_receiver.{method_name}({arguments})")
    } else {
        format!("{target}::{method_name}({arguments})")
    };
    let receiver_parameter = if row.kind == "trait-method" {
        receiver.map(|_| "mut contract_receiver: ContractReceiver, ".to_owned())
    } else {
        receiver.map(|_| format!("mut contract_receiver: {target}, "))
    }
    .unwrap_or_default();
    format!(
        "#[allow(dead_code, unused_mut)] fn contract_method_{index}{generics}({receiver_parameter}) {where_clause} {{ let _ = {call}; }}"
    )
}

fn compile_assert_probe(row: &GoldenRow, index: usize) -> String {
    let bound = row.api.replacen("Self", &row.path, 1);
    format!(
        "#[allow(dead_code)] fn contract_bound_{index}() {{ fn require<T>() where {bound} {{}} require::<{}>(); }}",
        row.path
    )
}

pub fn forbidden_name_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let symbol = &row.0[0];
            let module = &row.0[1];
            if symbol.contains("::")
                || symbol.contains('{')
                || symbol.contains('/')
                || symbol.contains("->")
                || symbol.contains(':')
            {
                return None;
            }
            let feature = match module.as_str() {
                "view" => Some("view"),
                "ecs" => Some("ecs"),
                "core" => None,
                _ => return None,
            };
            Some(unresolved_case(
                &format!("forbidden-{index:03}-{}", slug(symbol)),
                feature.into_iter().map(str::to_owned).collect(),
                &format!("pub use bevy_ui_router::{module}::{symbol} as Probe;"),
            ))
        })
        .collect()
}

pub fn forbidden_capability_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| is_forbidden_capability(row))
        .flat_map(|(index, row)| {
            let name = row.0[0].as_str();
            capability_fixtures(name).into_iter().enumerate().map(
                move |(fixture_index, (features, source, codes))| NegativeCase {
                    consumer: ConsumerCase {
                        kind: "fail",
                        feature_set: feature_set_name(&features),
                        slug: format!(
                            "forbidden-capability-{index:03}-{fixture_index:02}-{}",
                            slug(name)
                        ),
                        default_features: false,
                        features,
                        source,
                    },
                    expected_codes: codes,
                },
            )
        })
        .collect()
}

pub fn is_forbidden_capability(row: &TableRow) -> bool {
    let symbol = &row.0[0];
    symbol.contains("::")
        || symbol.contains('{')
        || symbol.contains('/')
        || symbol.contains("->")
        || symbol.contains(':')
}

type CapabilityFixture = (Vec<String>, String, Vec<&'static str>);

fn capability_fixtures(name: &str) -> Vec<CapabilityFixture> {
    let ecs = || vec!["ecs".to_owned()];
    let audit = || vec!["test-support".to_owned()];
    let one = |features: Vec<String>, source: &str, codes: Vec<&'static str>| {
        vec![(features, source.to_owned(), codes)]
    };
    let methods = |features: Vec<String>,
                   receiver: &str,
                   names: &[&str],
                   codes: Vec<&'static str>| {
        names
            .iter()
            .map(|method| {
                (
                    features.clone(),
                    format!(
                        "use bevy_ui_router::prelude::*; fn probe({receiver}) {{ let _ = value.{method}(); }}"
                    ),
                    codes.clone(),
                )
            })
            .collect()
    };
    match name {
        "EcsViewReportSession::new" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn probe() { let _ = EcsViewReportSession::new; }",
            vec!["E0624"],
        ),
        "FrameContentWriter::spawn" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn probe(value: &mut FrameContentWriter<'_>) { let _ = value.spawn(()); }",
            vec!["E0599", "E0624"],
        ),
        "FrameContentWriter::insert" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn probe(value: &mut FrameContentWriter<'_>, entity: ContentEntity) { let _ = value.insert(entity, ()); }",
            vec!["E0599", "E0624"],
        ),
        "FrameContentWriter::world_mut" => methods(
            ecs(),
            "value: &mut FrameContentWriter<'_>",
            &["world_mut"],
            vec!["E0599"],
        ),
        "FrameContentWriter::commands" => methods(
            ecs(),
            "value: &mut FrameContentWriter<'_>",
            &["commands"],
            vec!["E0599"],
        ),
        "ContentEntity::{raw,entity,from_raw}" => id_method_fixtures("ContentEntity", ecs(), &["raw", "entity", "from_raw"]),
        "AuditEntityId::{raw,entity,from_raw}" => id_method_fixtures("AuditEntityId", audit(), &["raw", "entity", "from_raw"]),
        "QuarantineLeaseId::{raw,from_raw}" => id_method_fixtures("QuarantineLeaseId", audit(), &["raw", "from_raw"]),
        "RouterHandle::{raw,from_raw}" => id_method_fixtures("RouterHandle", ecs(), &["raw", "from_raw"]),
        "NavigationRequestId::{raw,from_raw}" => id_method_fixtures("NavigationRequestId", ecs(), &["raw", "from_raw"]),
        "ContentEntity/AuditEntityId -> Entity" => [
            ("ContentEntity", ecs()),
            ("AuditEntityId", audit()),
        ]
        .into_iter()
        .map(|(ty, features)| (
            features,
            format!("use bevy_ui_router::prelude::*; fn probe(value: {ty}, gate: &RouterInputGate) {{ let _ = gate.allows_pointer(value.into()); }}"),
            vec!["E0277"],
        ))
        .collect(),
        "EcsViewReportSession: Clone" => one(
            ecs(),
            "use bevy_ui_router::prelude::*; fn require<T: Clone>() {} fn probe() { require::<EcsViewReportSession<'static>>(); }",
            vec!["E0277"],
        ),
        "ViewBuildReport::{new,success,failure}" => ["new", "success", "failure"]
            .into_iter()
            .map(|method| (
                ecs(),
                format!("use bevy_ui_router::prelude::*; fn probe() {{ let _ = ViewBuildReport::{method}; }}"),
                vec!["E0599"],
            ))
            .collect(),
        "ViewBuildReport: Clone/Default" => ["Clone", "Default"]
            .into_iter()
            .map(|bound| (
                ecs(),
                format!("use bevy_ui_router::prelude::*; fn require<T: {bound}>() {{}} fn probe() {{ require::<ViewBuildReport>(); }}"),
                vec!["E0277"],
            ))
            .collect(),
        "AppRouterExt::{set_active,register_authority}" => ["set_active", "register_authority"]
            .into_iter()
            .map(|method| (
                ecs(),
                format!("use bevy_ui_router::prelude::*; fn probe<T: AppRouterExt>(value: &mut T) {{ value.{method}(); }}"),
                vec!["E0599"],
            ))
            .collect(),
        "RouterNavigation::{commit,set_active,apply_report}" => methods(
            ecs(),
            "value: &mut RouterNavigation",
            &["commit", "set_active", "apply_report"],
            vec!["E0599"],
        ),
        "RouterAuthorityAuditSnapshot::{world,entity,registry}" => methods(
            audit(),
            "value: &RouterAuthorityAuditSnapshot",
            &["world", "entity", "registry"],
            vec!["E0599"],
        ),
        other => panic!("missing forbidden capability fixture: {other}"),
    }
}

fn id_method_fixtures(
    type_name: &str,
    features: Vec<String>,
    methods: &[&str],
) -> Vec<CapabilityFixture> {
    methods
        .iter()
        .map(|method| {
            let module = if type_name == "NavigationRequestId" {
                "core"
            } else {
                "ecs"
            };
            let source = if *method == "from_raw" {
                format!("use bevy_ui_router::{module}::{type_name}; fn probe() {{ let _ = {type_name}::from_raw(0); }}")
            } else {
                format!("use bevy_ui_router::{module}::{type_name}; fn probe(value: {type_name}) {{ let _ = value.{method}(); }}")
            };
            (features.clone(), source, vec!["E0432", "E0599"])
        })
        .collect()
}

pub fn forbidden_module_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    let mut cases = Vec::new();
    for row in rows {
        let expression = &row.0[0];
        let (module, names) = expression
            .split_once("::{")
            .map(|(module, names)| (module, names.trim_end_matches('}')))
            .or_else(|| expression.split_once("::"))
            .unwrap_or_else(|| panic!("invalid forbidden module row: {expression}"));
        for name in names.split(',') {
            cases.push(unresolved_case(
                &format!("forbidden-module-{module}-{}", slug(name)),
                match module {
                    "view" => vec!["view".to_owned()],
                    "ecs" => vec!["ecs".to_owned()],
                    _ => Vec::new(),
                },
                &format!("pub use bevy_ui_router::{module}::{name} as Probe;"),
            ));
        }
    }
    cases
}

pub fn profile_exclusion_cases(rows: &[TableRow]) -> Vec<NegativeCase> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| match row.0[0].as_str() {
            "symbol" => unresolved_case(
                &format!("profile-symbol-{index:02}-{}", slug(&row.0[1])),
                vec!["test-support".to_owned()],
                &format!("pub use {} as Probe;", row.0[1]),
            ),
            "associated" => associated_exclusion_case(index, &row.0[1]),
            kind => panic!("unknown profile exclusion kind: {kind}"),
        })
        .collect()
}

pub fn feature_disabled_cases(
    module_rows: &[TableRow],
    symbols: &[SymbolRow],
) -> Vec<NegativeCase> {
    let mut cases = Vec::new();
    for (index, row) in module_rows.iter().enumerate() {
        let feature = row.0[1].trim_matches(char::from(96));
        if feature == "default" {
            continue;
        }
        cases.push(unresolved_case(
            &format!("disabled-module-{index:02}-{}", slug(&row.0[0])),
            weaker_features(feature),
            &format!("pub use {} as Probe;", row.0[0]),
        ));
    }
    for (index, row) in symbols.iter().enumerate() {
        if row.feature == "default" {
            continue;
        }
        cases.push(unresolved_case(
            &format!("disabled-symbol-{index:03}-{}", slug(&row.symbol)),
            weaker_features(&row.feature),
            &format!(
                "pub use bevy_ui_router::{}::{} as Probe;",
                row.module, row.symbol
            ),
        ));
        if row.prelude {
            cases.push(unresolved_case(
                &format!("disabled-prelude-{index:03}-{}", slug(&row.symbol)),
                weaker_features(&row.feature),
                &format!("pub use bevy_ui_router::prelude::{} as Probe;", row.symbol),
            ));
        }
    }
    cases
}

pub fn opaque_struct_cases(golden: &[GoldenRow]) -> Vec<NegativeCase> {
    let shapes = public_struct_shapes(golden);
    let mut cases = Vec::new();
    for (index, row) in golden.iter().filter(|row| row.kind == "struct").enumerate() {
        if row.path == "bevy_ui_router::ecs::BevyUiRouterPlugin" {
            continue;
        }
        let key = (row.owner.clone(), path_symbol(&row.path).to_owned());
        let shape = shapes[&key];
        let features = consumer_features(&row.feature);
        cases.push(NegativeCase {
            consumer: ConsumerCase {
                kind: "fail",
                feature_set: feature_set_name(&features),
                slug: format!("opaque-value-{index:03}-{}", slug(&row.path)),
                default_features: false,
                features: features.clone(),
                source: format!("fn probe() {{ let _ = {}; }}", row.path),
            },
            expected_codes: vec!["E0423", "E0603"],
        });
        if matches!(shape, PublicStructShape::Named { .. }) {
            cases.push(NegativeCase {
                consumer: ConsumerCase {
                    kind: "fail",
                    feature_set: feature_set_name(&features),
                    slug: format!("opaque-literal-{index:03}-{}", slug(&row.path)),
                    default_features: false,
                    features,
                    source: format!("fn probe() {{ let _ = {} {{}}; }}", row.path),
                },
                expected_codes: vec!["E0063", "E0451"],
            });
        }
    }
    assert!(
        !cases.is_empty(),
        "opaque struct consumer catalogue is empty"
    );
    cases
}

fn consumer_features(feature: &str) -> Vec<String> {
    match feature {
        "default" => Vec::new(),
        "view" => vec!["view".to_owned()],
        "ecs" => vec!["ecs".to_owned()],
        "ecs-test-support" => vec!["test-support".to_owned()],
        "platform" => vec!["platform".to_owned()],
        other => panic!("unknown consumer feature: {other}"),
    }
}

fn unresolved_case(slug_value: &str, features: Vec<String>, source: &str) -> NegativeCase {
    NegativeCase {
        consumer: ConsumerCase {
            kind: "fail",
            feature_set: feature_set_name(&features),
            slug: slug_value.to_owned(),
            default_features: false,
            features,
            source: source.to_owned(),
        },
        expected_codes: vec!["E0432", "E0433", "E0603"],
    }
}

fn weaker_features(feature: &str) -> Vec<String> {
    match feature {
        "view" | "platform" => Vec::new(),
        "ecs" | "bsn" => vec!["view".to_owned()],
        "ecs-test-support" | "test-support" => vec!["ecs".to_owned()],
        other => panic!("unknown feature predecessor: {other}"),
    }
}

fn feature_set_name(features: &[String]) -> &'static str {
    match features {
        [] => "core-only",
        [feature] if feature == "view" => "view",
        [feature] if feature == "ecs" => "ecs",
        [feature] if feature == "test-support" => "test-support",
        _ => "custom",
    }
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_matches('-').to_owned()
}

fn associated_exclusion_case(index: usize, path: &str) -> NegativeCase {
    let (feature, source) = if path.ends_with("FrameContentWriter::declare_outlet") {
        (
            "ecs",
            "use bevy_ui_router::prelude::*; fn probe(writer: &mut FrameContentWriter<'_>, entity: ContentEntity, slot: RouteSlotKey) { let _ = writer.declare_outlet(entity, slot); }".to_owned(),
        )
    } else if path.ends_with("RouterNavigation::replace") {
        (
            "ecs",
            "use bevy_ui_router::prelude::*; fn probe(nav: &mut RouterNavigation, router: RouterHandle) { let _ = nav.replace(router, RouteLocation::new(\"/\")); }".to_owned(),
        )
    } else if path.ends_with("RouterNavigation::back") {
        (
            "ecs",
            "use bevy_ui_router::prelude::*; fn probe(nav: &mut RouterNavigation, router: RouterHandle) { let _ = nav.back(router); }".to_owned(),
        )
    } else if path.ends_with("RouteTreeBuilder::route_with_options") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(mut builder: RouteTreeBuilder, parent: RouteId) { let _ = builder.route_with_options(parent, RoutePattern::path(\"child\").unwrap(), RouteNodeOptions::new()); }".to_owned(),
        )
    } else if path.ends_with("RouteTreeBuilder::route") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(mut builder: RouteTreeBuilder, parent: RouteId) { let _ = builder.route(parent, RoutePattern::path(\"child\").unwrap()); }".to_owned(),
        )
    } else if path.ends_with("RouteNodeOptions::with_error_boundary") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(options: RouteNodeOptions, key: ErrorBoundaryKey) { let _ = options.with_error_boundary(key); }".to_owned(),
        )
    } else if path.ends_with("RouteNodeOptions::with_error_view") {
        (
            "core-only",
            "use bevy_ui_router::prelude::*; fn probe(options: RouteNodeOptions, boundary: ErrorBoundaryKey, view: RouteViewKey) { let _ = options.with_error_view(boundary, view); }".to_owned(),
        )
    } else if path.ends_with("RouterAuthorityAuditSnapshot::outlets") {
        (
            "test-support",
            "use bevy_ui_router::prelude::*; fn probe(snapshot: &RouterAuthorityAuditSnapshot) { let _ = snapshot.outlets(); }".to_owned(),
        )
    } else if path.ends_with("RouterAuthorityAuditSnapshot::quarantines") {
        (
            "test-support",
            "use bevy_ui_router::prelude::*; fn probe(snapshot: &RouterAuthorityAuditSnapshot) { let _ = snapshot.quarantines(); }".to_owned(),
        )
    } else {
        panic!("missing associated exclusion fixture: {path}");
    };
    let features = match feature {
        "ecs" => vec!["ecs".to_owned()],
        "test-support" => vec!["test-support".to_owned()],
        _ => Vec::new(),
    };
    NegativeCase {
        consumer: ConsumerCase {
            kind: "fail",
            feature_set: feature,
            slug: format!("profile-associated-{index:02}-{}", slug(path)),
            default_features: false,
            features,
            source,
        },
        expected_codes: vec!["E0599", "E0624"],
    }
}

pub fn run_positive_consumer(case: &ConsumerCase) -> String {
    let case_dir = write_consumer(case);
    let manifest = case_dir.join("Cargo.toml");
    let target = consumer_root().join("cargo-target");
    run_success(
        cargo(
            &[
                "generate-lockfile",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        case,
        "generate-lockfile",
    );
    let tree = run_success(
        cargo(
            &[
                "tree",
                "--locked",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "-e",
                "normal",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        case,
        "tree",
    );
    run_success(
        cargo(
            &[
                "check",
                "--locked",
                "--offline",
                "--color",
                "never",
                "--config",
                "build.rustc-wrapper = ''",
                "--manifest-path",
            ],
            &manifest,
            Some(&target),
        ),
        case,
        "check",
    );
    assert_lock_is_pinned(&case_dir.join("Cargo.lock"));
    String::from_utf8(tree.stdout).expect("cargo tree output must be UTF-8")
}

pub fn prepare_consumer_cases(kind: &str) {
    assert!(matches!(kind, "pass" | "fail"));
    let root = consumer_root().join(kind);
    assert!(
        root.starts_with(workspace_dir().join("target/facade-contract-cases")),
        "consumer cleanup escaped the fixed target root"
    );
    if root.exists() {
        fs::remove_dir_all(&root)
            .unwrap_or_else(|error| panic!("failed to clear '{}': {error}", root.display()));
    }
    fs::create_dir_all(&root)
        .unwrap_or_else(|error| panic!("failed to recreate '{}': {error}", root.display()));
}

pub fn assert_consumer_case_files(kind: &str, cases: &[ConsumerCase]) {
    assert!(matches!(kind, "pass" | "fail"));
    let root = consumer_root().join(kind);
    let expected = cases
        .iter()
        .flat_map(|case| {
            let case_root = PathBuf::from(case.feature_set).join(&case.slug);
            [
                case_root.join("Cargo.toml"),
                case_root.join("Cargo.lock"),
                case_root.join("src/lib.rs"),
            ]
        })
        .collect::<BTreeSet<_>>();
    let mut files = Vec::new();
    consumer_files(&root, &root, &mut files);
    let actual = files.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "generated `{kind}` consumer file set differs from the current catalogue"
    );
}

fn consumer_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", directory.display()))
    {
        let path = entry.unwrap().path();
        if path.is_dir() {
            consumer_files(root, &path, files);
        } else {
            files.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

pub fn run_negative_consumer(case: &NegativeCase) {
    let consumer = &case.consumer;
    let case_dir = write_consumer(consumer);
    let manifest = case_dir.join("Cargo.toml");
    let target = consumer_root().join("cargo-target");
    run_success(
        cargo(
            &[
                "generate-lockfile",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        consumer,
        "generate-lockfile",
    );
    run_success(
        cargo(
            &[
                "tree",
                "--locked",
                "--offline",
                "--config",
                "build.rustc-wrapper = ''",
                "-e",
                "normal",
                "--manifest-path",
            ],
            &manifest,
            None,
        ),
        consumer,
        "tree",
    );
    let output = cargo(
        &[
            "check",
            "--locked",
            "--offline",
            "--color",
            "never",
            "--config",
            "build.rustc-wrapper = ''",
            "--manifest-path",
        ],
        &manifest,
        Some(&target),
    );
    assert!(
        !output.status.success(),
        "negative consumer unexpectedly compiled: {}/{}/{}",
        consumer.kind,
        consumer.feature_set,
        consumer.slug
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("src\\lib.rs") || stderr.contains("src/lib.rs"),
        "negative consumer failed outside its generated src/lib.rs:\n{stderr}"
    );
    let expected_category = case
        .expected_codes
        .iter()
        .any(|code| stderr.contains(&format!("error[{code}]")));
    let nightly_private_literal = consumer.slug.starts_with("opaque-literal-")
        && stderr.contains("cannot construct")
        && stderr.contains("with struct literal syntax due to private fields");
    assert!(
        expected_category || nightly_private_literal,
        "negative consumer {}/{}/{} missed expected categories {:?}:\n{stderr}",
        consumer.kind,
        consumer.feature_set,
        consumer.slug,
        case.expected_codes
    );
    for infrastructure in [
        "failed to download",
        "failed to parse manifest",
        "current package believes it's in a workspace",
        "failed to get",
        "linking with",
    ] {
        assert!(
            !stderr.contains(infrastructure),
            "negative consumer failed for infrastructure reason '{infrastructure}':\n{stderr}"
        );
    }
    assert_lock_is_pinned(&case_dir.join("Cargo.lock"));
}

fn consumer_root() -> PathBuf {
    workspace_dir().join("target/facade-contract-cases")
}

fn write_consumer(case: &ConsumerCase) -> PathBuf {
    let root = consumer_root()
        .join(case.kind)
        .join(case.feature_set)
        .join(&case.slug);
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| panic!("failed to create '{}': {error}", root.display()));
    let facade = facade_dir()
        .canonicalize()
        .expect("facade path must canonicalize")
        .display()
        .to_string();
    let facade = facade
        .strip_prefix(r"\\?\")
        .unwrap_or(&facade)
        .replace('\\', "/");
    let features = case
        .features
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let package = format!(
        "facade-contract-{}-{}",
        case.feature_set,
        case.slug.replace('_', "-")
    );
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\nbevy_ui_router = {{ path = \"{facade}\", default-features = {}, features = [{features}] }}\n",
            case.default_features
        ),
    )
    .expect("consumer manifest must be writable");
    fs::write(root.join("src/lib.rs"), &case.source).expect("consumer source must be writable");
    root
}

fn cargo(prefix: &[&str], manifest: &Path, target_dir: Option<&Path>) -> Output {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.args(prefix).arg(manifest);
    if let Some(target_dir) = target_dir {
        command.arg("--target-dir").arg(target_dir);
    }
    command.output().expect("isolated cargo command must run")
}

fn run_success(output: Output, case: &ConsumerCase, command: &str) -> Output {
    assert!(
        output.status.success(),
        "{} failed for {}/{}/{}:\n{}",
        command,
        case.kind,
        case.feature_set,
        case.slug,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_lock_is_pinned(path: &Path) {
    let lock = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", path.display()));
    assert!(
        !lock.contains("github.com/shengyusi-SYS/bevy"),
        "isolated lock still contains the retired Bevy fork"
    );
    let lock: toml::Value = toml::from_str(&lock)
        .unwrap_or_else(|error| panic!("failed to parse '{}': {error}", path.display()));
    for package in lock["package"].as_array().unwrap() {
        let name = package["name"].as_str().unwrap();
        if !name.starts_with("bevy") || name.starts_with("bevy_ui_router") {
            continue;
        }
        let source = package
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("isolated lock Bevy package `{name}` must have a source"));
        assert_eq!(
            source, "registry+https://github.com/rust-lang/crates.io-index",
            "isolated lock Bevy package `{name}` must come from crates.io"
        );
        let version = package["version"].as_str().unwrap();
        if version.starts_with("0.19") {
            assert_eq!(
                version, "0.19.1",
                "isolated lock Bevy package `{name}` must resolve to 0.19.1"
            );
        }
    }
}

pub fn public_facade_source_violations(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("facade source must parse");
    let mut violations = Vec::new();
    let mut macro_violations = Vec::new();
    let mut modules = BTreeSet::new();
    let index = production_scope_index("", &file.items);
    walk_production_items_with_risks(
        &file.items,
        "",
        &index,
        &mut |item, scope| {
            violations.extend(unexpanded_public_api_risks_in_scope(item, scope));
            match item {
                syn::Item::Mod(module) => {
                    let path = module_ident(&scope.module, &module.ident.to_string());
                    if scope.block_depth == 0 && is_public(&module.vis) {
                        modules.insert(path.clone());
                    }
                    if scope.block_depth == 0 && is_public(&module.vis) && module.content.is_none()
                    {
                        violations.push(format!("public module `{}` is not inline", module.ident));
                    }
                }
                syn::Item::Use(item)
                    if scope.block_depth == 0 && matches!(item.vis, syn::Visibility::Public(_)) =>
                {
                    if scope.module.is_empty() {
                        violations.push(
                            "facade root contains a public use instead of a curated module".into(),
                        );
                    }
                    inspect_use_tree(&item.tree, &mut violations);
                }
                syn::Item::ExternCrate(item)
                    if scope.block_depth == 0 && matches!(item.vis, syn::Visibility::Public(_)) =>
                {
                    violations.push("facade contains pub extern crate".into());
                }
                syn::Item::Impl(_) => {
                    violations.push(format!(
                        "facade module `{}` contains an impl that can create public API",
                        scope.module
                    ));
                }
                item if scope.block_depth == 0 && public_definition_kind(item).is_some() => {
                    violations.push(format!(
                        "facade module `{}` contains public {} definition",
                        scope.module,
                        public_definition_kind(item).unwrap()
                    ));
                }
                _ => {}
            }
        },
        &mut |risk, scope| {
            macro_violations.push(format!(
                "facade module `{}` contains a source macro risk: {risk}",
                scope.module
            ));
        },
    );
    violations.extend(macro_violations);
    let expected = BTreeSet::from([
        "bsn".to_owned(),
        "core".to_owned(),
        "ecs".to_owned(),
        "platform".to_owned(),
        "platform::prelude".to_owned(),
        "prelude".to_owned(),
        "view".to_owned(),
    ]);
    if modules != expected {
        violations.push(format!("public module set mismatch: {modules:?}"));
    }
    violations
}

fn public_definition_kind(item: &syn::Item) -> Option<&'static str> {
    let (visibility, kind) = match item {
        syn::Item::Const(item) => (&item.vis, "const"),
        syn::Item::Enum(item) => (&item.vis, "enum"),
        syn::Item::Fn(item) => (&item.vis, "fn"),
        syn::Item::Static(item) => (&item.vis, "static"),
        syn::Item::Struct(item) => (&item.vis, "struct"),
        syn::Item::Trait(item) => (&item.vis, "trait"),
        syn::Item::TraitAlias(item) => (&item.vis, "trait alias"),
        syn::Item::Type(item) => (&item.vis, "type"),
        syn::Item::Union(item) => (&item.vis, "union"),
        _ => return None,
    };
    is_public(visibility).then_some(kind)
}

fn inspect_use_tree(tree: &syn::UseTree, violations: &mut Vec<String>) {
    match tree {
        syn::UseTree::Glob(_) => violations.push("facade contains a public glob re-export".into()),
        syn::UseTree::Path(path) => inspect_use_tree(&path.tree, violations),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                inspect_use_tree(item, violations);
            }
        }
        syn::UseTree::Rename(rename) => violations.push(format!(
            "facade contains public rename `{} as {}`",
            rename.ident, rename.rename
        )),
        syn::UseTree::Name(_) => {}
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static RUSTC_FIXTURE_RUN: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn history_profile_rejects_a_10_3_table_moved_to_the_next_section() {
        let heading = "### 10.3 History authority regression additions";
        let source = read_workspace("docs/superpowers/specs/2026-07-20-history-design.md");
        let source = source.replacen(
            heading,
            &format!("{heading}\n\n## 11 Canonical amendments before runtime"),
            1,
        );
        assert!(
            std::panic::catch_unwind(|| HistoryContractProfile::parse(&source)).is_err(),
            "History profile must not search across the next heading for the 10.3 table"
        );
    }

    #[test]
    fn history_profile_rejects_a_wrong_first_10_3_table() {
        let header = "| Test function | Owning slice | State |";
        let source = read_workspace("docs/superpowers/specs/2026-07-20-history-design.md");
        let source = source.replacen(
            header,
            &format!(
                "| Wrong | Owning slice | State |\n| --- | --- | --- |\n| wrong | Back failure | planned |\n\n{header}"
            ),
            1,
        );
        assert!(
            std::panic::catch_unwind(|| HistoryContractProfile::parse(&source)).is_err(),
            "History profile must not skip a wrong first table for a later compatible table"
        );
    }

    #[test]
    fn history_profile_rejects_a_duplicate_10_3_heading() {
        let heading = "### 10.3 History authority regression additions";
        let source = read_workspace("docs/superpowers/specs/2026-07-20-history-design.md");
        let source = format!("{source}\n{heading}\n");
        assert!(
            std::panic::catch_unwind(|| HistoryContractProfile::parse(&source)).is_err(),
            "History profile headings must be unique"
        );
    }

    #[test]
    fn final_mvp_vocabulary_rejects_an_unmarked_text_block() {
        let fence = char::from(96).to_string().repeat(3);
        let source = format!("{fence}text\ncase\n{fence}");
        assert!(
            std::panic::catch_unwind(|| final_mvp_authority_vocabulary_from_source(&source))
                .is_err(),
            "only the marker-bound final MVP text block may define authority vocabulary"
        );
    }

    #[test]
    fn final_mvp_vocabulary_ignores_a_later_unbound_text_block() {
        let fence = char::from(96).to_string().repeat(3);
        let source = format!(
            "MVP gate必须精确等于以下完整集合：\n\n{fence}text\nmarker_case\n{fence}\n\n# Other\n\n{fence}text\nother_case\n{fence}"
        );
        assert_eq!(
            final_mvp_authority_vocabulary_from_source(&source),
            BTreeSet::from(["marker_case".to_owned()])
        );
    }

    #[test]
    fn claim_boundary_evidence_rejects_an_item_in_a_different_section() {
        let journeys = "# Claim\nclaim boundary\n\n# Item\ncontract item\n";
        assert!(
            std::panic::catch_unwind(|| {
                assert_claim_boundary_and_item_share_section(
                    journeys,
                    "contract item",
                    "claim boundary",
                )
            })
            .is_err(),
            "claim boundary and item must be bound to one markdown evidence section"
        );
    }

    fn rustc_fixture_output_root() -> PathBuf {
        let run = RUSTC_FIXTURE_RUN.fetch_add(1, Ordering::Relaxed);
        let root = workspace_dir()
            .join("target/facade-contract-cases/rustc-fixtures")
            .join(format!("{}-{run}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn synthetic_canonical(symbol: &str) -> BTreeMap<(String, String), String> {
        BTreeMap::from([(
            ("owner".to_owned(), symbol.to_owned()),
            format!("bevy_ui_router::ecs::{symbol}"),
        )])
    }

    fn synthetic_owner_records(source: &str, symbol: &str) -> BTreeSet<SourceRecord> {
        synthetic_owner_records_for(source, &[symbol])
    }

    fn synthetic_owner_records_for(source: &str, symbols: &[&str]) -> BTreeSet<SourceRecord> {
        let parsed = syn::parse_file(source).unwrap();
        let canonical = symbols
            .iter()
            .map(|symbol| {
                (
                    ("owner".to_owned(), (*symbol).to_owned()),
                    format!("bevy_ui_router::ecs::{symbol}"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut aliases = BTreeSet::new();
        collect_type_alias_names(&parsed.items, &mut aliases);
        let mut records = BTreeSet::new();
        let mut public_names = BTreeSet::new();
        collect_owner_items(
            "owner",
            &parsed.items,
            &canonical,
            &aliases,
            &mut records,
            &mut public_names,
        );
        records
    }

    fn compiled_hostile_fixture(name: &str) -> String {
        let fixture = facade_dir()
            .join("tests/support/fixtures/hostile_source")
            .join(name);
        let output_root = rustc_fixture_output_root();
        let metadata = output_root.join(format!("{}.rmeta", name.trim_end_matches(".rs")));
        let output = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
            .arg("--edition=2024")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("-o")
            .arg(metadata)
            .arg(&fixture)
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to invoke rustc for '{}': {error}",
                    fixture.display()
                )
            });
        assert!(
            output.status.success(),
            "hostile Rust fixture '{}' must remain legal:\n{}",
            fixture.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read_to_string(&fixture).unwrap()
    }

    fn compiled_module_fixture(name: &str) -> PathBuf {
        let root = facade_dir().join("tests/support/fixtures").join(name);
        let source = root.join("lib.rs");
        let output_root = rustc_fixture_output_root();
        let metadata = output_root.join(format!("{name}.rmeta"));
        let output = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
            .arg("--edition=2024")
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("-o")
            .arg(metadata)
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "module fixture '{}' must remain legal:\n{}",
            source.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        root
    }

    fn compiled_derive_binding_spoofs() -> [String; 2] {
        let root = facade_dir().join("tests/support/fixtures/derive_binding_spoof");
        let output_root = rustc_fixture_output_root();
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
        let proc_macro = output_root.join(format!(
            "{}evil_component{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
        let wrapper = output_root.join("libevil_bevy_ecs.rlib");
        let compile = |mut command: Command, label: &str| {
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "derive binding fixture {label} must remain legal:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };

        let mut proc_macro_command = Command::new(&rustc);
        proc_macro_command
            .arg("--edition=2024")
            .arg("--crate-type=proc-macro")
            .arg("--crate-name=evil_component")
            .arg("-o")
            .arg(&proc_macro)
            .arg(root.join("evil_component.rs"));
        compile(proc_macro_command, "proc macro");

        let mut wrapper_command = Command::new(&rustc);
        wrapper_command
            .arg("--edition=2024")
            .arg("--crate-type=rlib")
            .arg("--crate-name=evil_bevy_ecs")
            .arg("--extern")
            .arg(format!("evil_component={}", proc_macro.display()))
            .arg("-o")
            .arg(&wrapper)
            .arg(root.join("evil_bevy_ecs.rs"));
        compile(wrapper_command, "extern rename wrapper");

        for (name, extern_wrapper) in [
            ("glob_consumer.rs", false),
            ("extern_rename_consumer.rs", true),
        ] {
            let mut consumer = Command::new(&rustc);
            consumer
                .arg("--edition=2024")
                .arg("--crate-type=lib")
                .arg("--emit=metadata")
                .arg("--extern")
                .arg(format!("evil_component={}", proc_macro.display()));
            if extern_wrapper {
                consumer
                    .arg("-L")
                    .arg(format!("dependency={}", output_root.display()))
                    .arg("--extern")
                    .arg(format!("evil_bevy_ecs={}", wrapper.display()));
            }
            consumer
                .arg("-o")
                .arg(output_root.join(format!("{}.rmeta", name.trim_end_matches(".rs"))))
                .arg(root.join(name));
            compile(consumer, name);
        }

        [
            fs::read_to_string(root.join("glob_consumer.rs")).unwrap(),
            fs::read_to_string(root.join("extern_rename_consumer.rs")).unwrap(),
        ]
    }

    fn contract_rows() -> (Vec<SymbolRow>, Vec<TableRow>) {
        let allowlist = read_workspace("docs/facade-export-allowlist.md");
        let symbols = markdown_table(
            &allowlist,
            "## 3. Allowed Type/Value/Trait Symbols",
            &["Symbol", "Module path", "Prelude", "Required feature"],
        );
        let exclusions = history_contract_profile().exclusions;
        (root_symbols(&symbols, &exclusions), exclusions)
    }

    #[test]
    fn public_rename_and_intermediate_source_mutations_are_rejected() {
        let rename = "pub mod core { pub use bevy_ui_router_core::SecretAuthority as BackIntent; }";
        assert!(
            public_facade_source_violations(rename)
                .iter()
                .any(|violation| violation.contains("public rename"))
        );

        let (symbols, _) = contract_rows();
        let source = fs::read_to_string(facade_dir().join("src/lib.rs"))
            .unwrap()
            .replace(
                "bevy_ui_router_core::back::{",
                "bevy_ui_router_core::fake::{",
            );
        assert!(
            std::panic::catch_unwind(|| assert_facade_exports(&source, &symbols)).is_err(),
            "intermediate owner module mutation escaped the export gate"
        );
    }

    #[test]
    fn owner_impl_through_type_alias_cannot_escape_associated_api_closure() {
        let source = r#"
            pub struct RouterNavigation;
            type NavigationAlias = RouterNavigation;

            impl NavigationAlias {
                pub fn force_private_authority(&mut self) {}
            }
        "#;
        let canonical = synthetic_canonical("RouterNavigation");
        let parsed = syn::parse_file(source).unwrap();
        let mut aliases = BTreeSet::new();
        collect_type_alias_names(&parsed.items, &mut aliases);
        let mut records = BTreeSet::new();
        let mut public_names = BTreeSet::new();
        let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            collect_owner_items(
                "owner",
                &parsed.items,
                &canonical,
                &aliases,
                &mut records,
                &mut public_names,
            );
        }))
        .is_err();
        let enumerated = records.contains(&source_record(
            "owner",
            "RouterNavigation",
            "method",
            "pub fn force_private_authority(&mut self)",
        ));
        assert!(
            rejected || enumerated,
            "type-alias inherent impl escaped without enumeration or fail-closed rejection"
        );
    }

    #[test]
    fn owner_item_and_impl_macros_fail_closed() {
        for (shape, source) in [
            (
                "top-level invocation",
                r#"
                    pub struct RouterNavigation;
                    leaked_owner_impl!(RouterNavigation);
                "#,
            ),
            (
                "canonical owner impl invocation",
                r#"
                    pub struct RouterNavigation;
                    impl RouterNavigation {
                        leaked_method!();
                    }
                "#,
            ),
            (
                "private-module macro_export",
                r#"
                    pub struct RouterNavigation;
                    mod hidden {
                        #[macro_export]
                        macro_rules! leaked_facade_api {
                            () => { pub struct LeakedAuthority; };
                        }
                    }
                "#,
            ),
        ] {
            assert!(
                std::panic::catch_unwind(|| {
                    synthetic_owner_records(source, "RouterNavigation")
                })
                .is_err(),
                "owner {shape} was silently ignored"
            );
        }
    }

    #[test]
    fn private_facade_module_macro_export_is_inspected() {
        let original = fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
        let source = format!(
            r#"{original}
                mod hidden {{
                    #[macro_export]
                    macro_rules! leaked_facade_api {{
                        () => {{ pub struct LeakedAuthority; }};
                    }}
                }}
            "#
        );
        assert!(
            public_facade_source_violations(&source)
                .iter()
                .any(|violation| violation.contains("source macro")),
            "private-module macro_export escaped facade source inspection"
        );
    }

    #[test]
    fn source_enumeration_does_not_trust_tests_filename_suffix() {
        let root = facade_dir().join("tests/support/fixtures/source_enumeration");
        let mut files = Vec::new();
        rust_files(&root, &mut files);
        let names = files
            .iter()
            .filter_map(|path| path.file_name()?.to_str())
            .collect::<BTreeSet<_>>();
        assert!(
            names.contains("capability_tests.rs"),
            "production-reachable `_tests.rs` file was omitted"
        );
        assert!(
            !names.contains("pure_tests.rs"),
            "a provably cfg(test)-only file should remain excluded"
        );
    }

    #[test]
    fn only_exact_cfg_test_items_are_omitted_from_production_scan() {
        let parsed = syn::parse_file(
            r#"
                #[cfg(test)]
                impl RouterNavigation { pub fn test_helper(&self) {} }

                #[cfg(not(test))]
                impl RouterNavigation { pub fn production_authority(&self) {} }

                #[cfg(any(test, feature = "ecs"))]
                impl RouterNavigation { pub fn production_alternative(&self) {} }
            "#,
        )
        .unwrap();
        assert!(item_is_test(&parsed.items[0]));
        assert!(!item_is_test(&parsed.items[1]));
        assert!(!item_is_test(&parsed.items[2]));
    }

    #[test]
    fn reverse_conversion_trait_impl_records_the_complete_type_graph() {
        let records = synthetic_owner_records(
            r#"
                pub struct RouterHandle;
                impl From<RouterHandle> for bevy_ecs::entity::Entity {
                    fn from(_: RouterHandle) -> Self { unreachable!() }
                }
            "#,
            "RouterHandle",
        );
        assert!(
            records.contains(&source_record(
                "owner",
                "RouterHandle",
                "trait-impl",
                "impl From<Self> for bevy_ecs::entity::Entity",
            )),
            "reverse conversion through a trait generic was not attributed to RouterHandle: {records:#?}"
        );
    }

    #[test]
    fn trait_impl_keeps_generics_bindings_where_clause_and_full_self_type() {
        let records = synthetic_owner_records(
            r#"
                pub struct RouterHandle;
                impl<T> private::Capability<T, Item = PrivateAuthority>
                    for foreign::Wrapper<RouterHandle>
                where
                    T: Into<PrivateAuthority>,
                {}
            "#,
            "RouterHandle",
        );
        assert!(
            records.contains(&source_record(
                "owner",
                "RouterHandle",
                "trait-impl",
                "impl<T> private::Capability<T, Item = PrivateAuthority> for foreign::Wrapper<Self> where T: Into<PrivateAuthority>",
            )),
            "trait impl signature graph was lossy: {records:#?}"
        );
    }

    #[test]
    fn nested_use_rename_inherent_impl_is_rejected_fail_closed() {
        let source = compiled_hostile_fixture("use_alias_inherent.rs");
        assert!(
            std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterNavigation"))
                .is_err(),
            "nested `use ... as ...` inherent impl escaped owner selection"
        );
    }

    #[test]
    fn use_rename_reverse_conversion_is_rejected_fail_closed() {
        let source = compiled_hostile_fixture("use_alias_reverse.rs");
        assert!(
            std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterHandle")).is_err(),
            "`impl From<import alias> for Entity` escaped the trait type graph"
        );
    }

    #[test]
    fn self_as_use_rename_impl_is_rejected_fail_closed() {
        let source = compiled_hostile_fixture("use_alias_self_as.rs");
        assert!(
            std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterNavigation"))
                .is_err(),
            "`use module::{{self as alias}}` impl escaped fail-closed selection"
        );
    }

    #[test]
    fn use_rename_scope_does_not_leak_into_shadowing_nested_module() {
        let records = synthetic_owner_records(
            r#"
                pub struct RouterNavigation;
                mod outer {
                    use crate::RouterNavigation as H;

                    mod inner {
                        pub struct H;
                        impl H {
                            pub fn local_method(&self) {}
                        }
                    }
                }
            "#,
            "H",
        );
        assert!(records.contains(&source_record(
            "owner",
            "H",
            "method",
            "pub fn local_method(&self)",
        )));
    }

    #[test]
    fn ancestor_qualified_import_aliases_fail_closed() {
        let source = compiled_hostile_fixture("ancestor_qualified_aliases.rs");
        assert!(
            std::panic::catch_unwind(|| {
                synthetic_owner_records_for(&source, &["RouterNavigation", "RouterHandle"])
            })
            .is_err(),
            "ancestor-qualified inherent/reverse impl aliases escaped owner selection"
        );
    }

    #[test]
    fn path_override_keeps_semantic_module_identity() {
        let root = facade_dir().join("tests/support/fixtures/path_semantic_module");
        let sources = production_source_modules(&root);
        let physical = root.join("facade_path_override_physical.rs");
        assert!(
            sources
                .iter()
                .any(|(file, module)| file == &physical && module == "outer"),
            "#[path] source was not assigned its declared logical module"
        );
        assert!(
            !sources.iter().any(|(file, module)| {
                file == &physical && module == "facade_path_override_physical"
            }),
            "#[path] physical filename leaked into semantic module identity"
        );
        let canonical = synthetic_canonical("RouterNavigation");
        let mut aliases = BTreeSet::new();
        let mut index = ProductionScopeIndex::default();
        let parsed = sources
            .into_iter()
            .map(|(file, module)| {
                let parsed = syn::parse_file(&fs::read_to_string(file).unwrap()).unwrap();
                collect_type_alias_names(&parsed.items, &mut aliases);
                collect_production_module_scopes(&module, &parsed.items, &mut index);
                (module, parsed)
            })
            .collect::<Vec<_>>();
        let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut records = BTreeSet::new();
            let mut names = BTreeSet::new();
            for (module, source) in &parsed {
                collect_owner_items_with_index(
                    "owner",
                    &source.items,
                    module,
                    &index,
                    &canonical,
                    &aliases,
                    &mut records,
                    &mut names,
                );
            }
        }))
        .is_err();
        assert!(
            rejected,
            "#[path] physical filename replaced semantic module identity"
        );
    }

    #[test]
    fn inline_path_override_uses_inline_module_directory() {
        let root = compiled_module_fixture("inline_path_semantic_module");
        let sources = production_source_modules(&root);
        let physical = root.join("wrapper/physical.rs");
        assert!(
            sources
                .iter()
                .any(|(file, module)| file == &physical && module == "wrapper::outer"),
            "inline #[path] did not retain its declared logical module: {sources:#?}"
        );
        assert!(
            !sources
                .iter()
                .any(|(file, module)| file == &physical && module == "wrapper::physical"),
            "inline #[path] fell back to its physical filename"
        );
    }

    #[test]
    fn path_attribute_base_resets_for_external_source_files() {
        let root = compiled_module_fixture("inline_path_semantic_module");
        let sources = production_source_modules(&root);
        let external_physical = root.join("physical.rs");
        assert!(
            sources.iter().any(|(file, module)| {
                file == &external_physical && module == "outer::semantic"
            }),
            "external source #[path] did not use its source-file parent: {sources:#?}"
        );
        let inline_physical = root.join("wrapper/physical.rs");
        assert!(
            sources
                .iter()
                .any(|(file, module)| file == &inline_physical && module == "wrapper::outer"),
            "external source reset regressed inline-module path accumulation"
        );
    }

    #[test]
    fn block_local_inline_module_uses_live_ancestor_bindings() {
        let source = compiled_hostile_fixture("block_local_inline_module.rs");
        assert!(
            std::panic::catch_unwind(|| { synthetic_owner_records(&source, "RouterNavigation") })
                .is_err(),
            "block-local module ancestor alias escaped live lexical scope checks"
        );
    }

    #[test]
    fn include_macro_cannot_inject_owner_items() {
        let source = compiled_hostile_fixture("include_owner_impl.rs");
        assert!(
            std::panic::catch_unwind(|| { synthetic_owner_records(&source, "RouterNavigation") })
                .is_err(),
            "trusted include! injected an unenumerated owner impl"
        );
    }

    #[test]
    fn thread_local_macro_cannot_inject_public_static() {
        let original = fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
        let injected = compiled_hostile_fixture("thread_local_public_static.rs");
        let source = format!("{original}\n{injected}");
        assert!(
            public_facade_source_violations(&source)
                .iter()
                .any(|violation| violation.contains("thread_local")),
            "trusted thread_local! injected an unenumerated public facade static"
        );
    }

    #[test]
    fn opaque_scope_bindings_cannot_spoof_derive_authority() {
        for source in compiled_derive_binding_spoofs() {
            assert!(
                std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterHandle"))
                    .is_err(),
                "glob or extern-crate rename spoofed the trusted derive authority"
            );
        }
    }

    #[test]
    fn nested_expression_items_are_visited_and_stmt_macro_impl_fails_closed() {
        let source = compiled_hostile_fixture("nested_expr_items.rs");
        let records = synthetic_owner_records(&source, "RouterNavigation");
        for method in ["call_authority", "tuple_authority", "return_authority"] {
            assert!(records.contains(&source_record(
                "owner",
                "RouterNavigation",
                "method",
                &format!("pub fn {method}(&self)"),
            )));
        }

        let source = compiled_hostile_fixture("stmt_macro_impl.rs");
        assert!(
            std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterNavigation"))
                .is_err(),
            "block statement macro emitted an unenumerated inherent impl"
        );
    }

    #[test]
    fn derive_prefix_alias_cannot_spoof_bevy_authority() {
        assert!(
            std::panic::catch_unwind(|| {
                synthetic_owner_records(
                    r#"
                        mod evil { pub mod component { pub struct Component; } }
                        use crate::evil as bevy_ecs;
                        #[derive(bevy_ecs::component::Component)]
                        #[component]
                        pub struct RouterHandle;
                    "#,
                    "RouterHandle",
                )
            })
            .is_err(),
            "first-segment import alias spoofed the trusted Bevy derive path"
        );
    }

    #[test]
    fn block_local_nonlocal_impls_are_enumerated() {
        let source = compiled_hostile_fixture("block_local_nonlocal_impls.rs");
        let records = synthetic_owner_records_for(&source, &["RouterNavigation", "RouterHandle"]);
        assert!(records.contains(&source_record(
            "owner",
            "RouterNavigation",
            "method",
            "pub fn force_private_authority(&mut self)",
        )));
        assert!(records.contains(&source_record(
            "owner",
            "RouterHandle",
            "trait-impl",
            "impl From<Self> for bevy_ecs::entity::Entity",
        )));
    }

    #[test]
    fn block_local_attribute_expansion_fails_closed() {
        for attribute in [
            "#[leaked_api]",
            "#[cfg_attr(not(test), leaked_api)]",
            "#[component]",
        ] {
            let source =
                format!("fn hidden() {{ {attribute} struct Local; }} pub struct RouterHandle;");
            assert!(
                std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterHandle"))
                    .is_err(),
                "block-local attribute escaped fail-closed inspection: {attribute}"
            );
        }
    }

    #[test]
    fn bevy_component_derive_helpers_are_structurally_allowed() {
        for helper in [
            "#[component(on_insert)]",
            "#[relationship(relationship_target = Target)]",
        ] {
            let source = format!(
                r#"
                    use bevy_ecs::component::Component;
                    #[derive(Component)]
                    {helper}
                    pub struct RouterHandle;
                "#
            );
            assert!(
                std::panic::catch_unwind(|| synthetic_owner_records(&source, "RouterHandle"))
                    .is_ok(),
                "registered Component derive helper was rejected: {helper}"
            );
        }
    }

    #[test]
    fn block_exported_macro_and_public_trait_item_macro_fail_closed() {
        let source = compiled_hostile_fixture("deep_exported_trait_macro.rs");
        assert!(
            std::panic::catch_unwind(|| synthetic_owner_records(&source, "AppRouterExt")).is_err(),
            "block-nested macro_export plus public TraitItem::Macro escaped owner inspection"
        );
    }

    #[test]
    fn public_trait_item_macro_fails_closed_independently() {
        assert!(
            std::panic::catch_unwind(|| {
                synthetic_owner_records(
                    "pub trait AppRouterExt { leaked_trait_items!(); }",
                    "AppRouterExt",
                )
            })
            .is_err(),
            "public TraitItem::Macro escaped owner inspection"
        );
    }

    #[test]
    fn private_facade_function_cannot_hide_macro_export() {
        let original = fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
        let source = format!(
            r#"{original}
                mod hidden {{
                    fn container() {{
                        #[macro_export]
                        macro_rules! leaked_from_block {{
                            () => {{ pub struct LeakedAuthority; }};
                        }}
                    }}
                }}
            "#
        );
        assert!(
            public_facade_source_violations(&source)
                .iter()
                .any(|violation| violation.contains("macro_export")),
            "private function block hid a root-exported macro from facade inspection"
        );
    }

    #[test]
    fn ordinary_nonexported_function_local_macro_is_not_public_api_risk() {
        let records = synthetic_owner_records(
            r#"
                fn private_container() {
                    macro_rules! local_value { () => { 7_u8 }; }
                    let _ = local_value!();
                }
                pub struct RouterHandle;
            "#,
            "RouterHandle",
        );
        assert!(records.contains(&source_record(
            "owner",
            "RouterHandle",
            "struct",
            "pub struct RouterHandle",
        )));
    }

    #[test]
    fn owner_public_surface_attribute_macro_fails_closed() {
        for source in [
            "#[leaked_api] pub struct RouterHandle;",
            "#[cfg_attr(not(test), leaked_api)] pub struct RouterHandle;",
        ] {
            assert!(
                std::panic::catch_unwind(|| synthetic_owner_records(source, "RouterHandle"))
                    .is_err(),
                "possible attribute proc macro escaped owner public-surface inspection: {source}"
            );
        }
    }

    #[test]
    fn owner_public_surface_must_use_builtin_is_not_reported_as_an_attribute_macro() {
        let records = synthetic_owner_records(
            r#"#[must_use = "RouterHandle must be used"] pub struct RouterHandle;"#,
            "RouterHandle",
        );
        assert!(records.contains(&source_record(
            "owner",
            "RouterHandle",
            "struct",
            "pub struct RouterHandle",
        )));
    }

    #[test]
    fn facade_public_surface_attribute_macro_fails_closed() {
        let original = fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
        let source = original.replacen("pub mod core {", "#[leaked_api]\npub mod core {", 1);
        assert_ne!(source, original);
        assert!(
            public_facade_source_violations(&source)
                .iter()
                .any(|violation| violation.contains("attribute macro")),
            "possible facade attribute proc macro escaped source inspection"
        );
    }

    #[test]
    fn composite_feature_cfg_cannot_merge_devtools_with_ecs_public_surface() {
        let (symbols, _) = contract_rows();
        let original = fs::read_to_string(facade_dir().join("src/lib.rs")).unwrap();
        let source = original.replace(
            "#[cfg(feature = \"ecs\")]\npub mod ecs {",
            "#[cfg(any(feature = \"ecs\", feature = \"devtools\"))]\npub mod ecs {",
        );
        assert_ne!(
            source, original,
            "feature mutation did not modify facade source"
        );

        let rejected_by_source_gate = !public_facade_source_violations(&source).is_empty();
        let rejected_by_export_gate =
            std::panic::catch_unwind(|| assert_facade_exports(&source, &symbols)).is_err();
        assert!(
            rejected_by_source_gate || rejected_by_export_gate,
            "composite feature cfg exposed the ECS surface under devtools without rejection"
        );
    }

    #[test]
    fn absolute_and_relative_import_renames_resolve_to_the_real_private_type() {
        let catalogue = BTreeMap::from([
            (
                "RouteId".to_owned(),
                BTreeSet::from([("owner".to_owned(), "ids".to_owned(), true)]),
            ),
            (
                "SecretAuthority".to_owned(),
                BTreeSet::from([
                    ("owner".to_owned(), "secret".to_owned(), false),
                    ("owner".to_owned(), "public_api::secret".to_owned(), false),
                ]),
            ),
        ]);
        for (module, alias, import, expected_module) in [
            (
                "public_api",
                "RouteId",
                "crate::secret::SecretAuthority",
                "secret",
            ),
            (
                "public_api",
                "RouteId",
                "self::secret::SecretAuthority",
                "public_api::secret",
            ),
            (
                "public_api::nested",
                "RouteId",
                "super::secret::SecretAuthority",
                "public_api::secret",
            ),
            (
                "public_api::nested::deeper",
                "RouteId",
                "super::super::secret::SecretAuthority",
                "public_api::secret",
            ),
            (
                "public_api",
                "String",
                "secret::SecretAuthority",
                "public_api::secret",
            ),
        ] {
            let scope = SourceScope {
                module: module.to_owned(),
                imports: BTreeMap::from([(alias.to_owned(), import.to_owned())]),
            };
            let (identifier, resolved) = resolve_project_type(
                "owner",
                &scope,
                &format!("pub fn authority(&self) -> {alias}"),
                alias,
                &catalogue,
            )
            .unwrap();
            assert_eq!(identifier, "SecretAuthority", "import `{import}`");
            assert_eq!(
                resolved,
                vec![("owner".to_owned(), expected_module.to_owned(), false)],
                "import `{import}`"
            );
        }
    }

    #[test]
    fn public_signature_closure_rejects_private_aliases_before_external_leaf_filtering() {
        let catalogue = BTreeMap::from([
            (
                "RouteId".to_owned(),
                BTreeSet::from([("owner".to_owned(), "ids".to_owned(), true)]),
            ),
            (
                "SecretAuthority".to_owned(),
                BTreeSet::from([
                    ("owner".to_owned(), "secret".to_owned(), false),
                    ("owner".to_owned(), "public_api::secret".to_owned(), false),
                ]),
            ),
        ]);
        let golden = vec![GoldenRow {
            feature: "default".to_owned(),
            path: "bevy_ui_router::core::RouteId".to_owned(),
            owner: "owner".to_owned(),
            kind: "method".to_owned(),
            api: "pub fn authority(&self) -> String".to_owned(),
        }];
        for (module, import) in [
            ("public_api", "crate::secret::SecretAuthority"),
            ("public_api", "self::secret::SecretAuthority"),
            ("public_api::nested", "super::secret::SecretAuthority"),
            (
                "public_api::nested::deeper",
                "super::super::secret::SecretAuthority",
            ),
            ("public_api", "secret::SecretAuthority"),
        ] {
            let scopes = ProjectSourceScopes {
                declarations: BTreeMap::new(),
                items: BTreeMap::from([(
                    source_record(
                        "owner",
                        "RouteId",
                        "method",
                        "pub fn authority(&self) -> String",
                    ),
                    SourceScope {
                        module: module.to_owned(),
                        imports: BTreeMap::from([("String".to_owned(), import.to_owned())]),
                    },
                )]),
            };
            assert!(
                std::panic::catch_unwind(|| {
                    assert_public_signature_closure(&golden, &catalogue, &scopes)
                })
                .is_err(),
                "private alias `{import} as String` escaped the complete signature closure gate"
            );
        }
    }

    #[test]
    fn public_signature_closure_ignores_real_external_generic_types() {
        let catalogue = BTreeMap::from([
            (
                "RouteId".to_owned(),
                BTreeSet::from([("owner".to_owned(), "ids".to_owned(), true)]),
            ),
            (
                "String".to_owned(),
                BTreeSet::from([("other_owner".to_owned(), "text".to_owned(), true)]),
            ),
        ]);
        let golden = vec![GoldenRow {
            feature: "default".to_owned(),
            path: "bevy_ui_router::core::RouteId".to_owned(),
            owner: "owner".to_owned(),
            kind: "method".to_owned(),
            api: "pub fn labels(&self) -> Vec<Option<String>>".to_owned(),
        }];
        let scopes = ProjectSourceScopes {
            declarations: BTreeMap::new(),
            items: BTreeMap::from([(
                source_record(
                    "owner",
                    "RouteId",
                    "method",
                    "pub fn labels(&self) -> Vec<Option<String>>",
                ),
                SourceScope {
                    module: "public_api".to_owned(),
                    imports: BTreeMap::from([(
                        "String".to_owned(),
                        "std::string::String".to_owned(),
                    )]),
                },
            )]),
        };
        assert_public_signature_closure(&golden, &catalogue, &scopes);
    }

    #[test]
    fn associated_signature_uses_the_impl_modules_real_import_scope() {
        let source = r#"
            mod secret {
                pub(crate) struct SecretAuthority;
            }
            mod public_api {
                pub struct RouteId(u64);
            }
            mod sibling {
                use crate::secret::SecretAuthority as String;

                impl crate::public_api::RouteId {
                    pub fn authority(&self) -> String {
                        unreachable!()
                    }
                }
            }
        "#;
        let parsed = syn::parse_file(source).unwrap();
        let mut catalogue = BTreeMap::new();
        collect_project_types("owner", "", &parsed.items, &mut catalogue);
        let mut scopes = ProjectSourceScopes::default();
        collect_source_scopes("owner", "", &parsed.items, &mut scopes);
        let golden = vec![GoldenRow {
            feature: "default".to_owned(),
            path: "bevy_ui_router::core::RouteId".to_owned(),
            owner: "owner".to_owned(),
            kind: "method".to_owned(),
            api: "pub fn authority(&self) -> String".to_owned(),
        }];

        assert!(
            std::panic::catch_unwind(|| {
                assert_public_signature_closure(&golden, &catalogue, &scopes)
            })
            .is_err(),
            "private alias in a sibling impl module escaped associated signature closure"
        );
    }

    #[test]
    fn source_scope_rejects_duplicate_aliases_from_every_use_shape() {
        let conditional_private =
            "#[cfg(feature = \"test-support\")] use crate::secret::SecretAuthority as String;";
        let conditional_external =
            "#[cfg(not(feature = \"test-support\"))] use std::string::String;";
        let import_cases = [
            format!("{conditional_private}\n{conditional_external}"),
            format!("{conditional_external}\n{conditional_private}"),
            "use crate::{secret::SecretAuthority as String, alternate::AlternateString as String};"
                .to_owned(),
            "use crate::secret::String;\nuse std::string::String;".to_owned(),
        ];

        for imports in import_cases {
            let source = format!(
                r#"
                    mod secret {{
                        pub(crate) struct SecretAuthority;
                        pub(crate) struct String;
                    }}
                    mod alternate {{
                        pub(crate) struct AlternateString;
                    }}
                    mod public_api {{
                        pub struct RouteId(u64);
                    }}
                    mod sibling {{
                        {imports}

                        impl crate::public_api::RouteId {{
                            pub fn authority(&self) -> impl Into<String> {{
                                unreachable!()
                            }}
                        }}
                    }}
                "#
            );
            let parsed = syn::parse_file(&source).unwrap();
            let mut scopes = ProjectSourceScopes::default();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    collect_source_scopes("owner", "", &parsed.items, &mut scopes)
                }))
                .is_err(),
                "duplicate alias escaped source-scope construction:\n{imports}"
            );
        }
    }

    #[test]
    fn required_compile_assert_trio_cannot_disappear() {
        let (_, exclusions) = contract_rows();
        let golden = read_workspace("docs/facade-associated-api-golden.txt").replace(
            "default | bevy_ui_router::core::RouteError | bevy_ui_router_core | compile-assert | Self: Send\n",
            "",
        );
        assert!(
            std::panic::catch_unwind(|| parse_golden(&golden, &exclusions)).is_err(),
            "required error bound mutation escaped the golden parser"
        );
    }
}
