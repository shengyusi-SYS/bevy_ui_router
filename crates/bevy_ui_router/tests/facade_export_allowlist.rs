#[path = "support/facade_contract_cases.rs"]
mod facade_contract_cases;

use std::collections::BTreeSet;

use facade_contract_cases::{
    assert_consumer_case_files, assert_devtools_adds_no_public_symbols, assert_facade_exports,
    assert_golden_covers_symbols, assert_owner_source_matches_golden, authority_public_activation,
    facade_dir, history_contract_profile, markdown_table, parse_authority_public_activation,
    parse_golden, parse_recovery_contract_profile, positive_consumer_cases, prepare_consumer_cases,
    public_facade_source_violations, read_workspace, root_symbols, run_positive_consumer,
    stage_contract_profile,
};

fn recovery_profile_source() -> String {
    read_workspace("docs/superpowers/specs/2026-08-12-recovery-design.md")
}

#[test]
fn recovery_profile_has_exact_activation_machine_state() {
    let profile = parse_recovery_contract_profile(&recovery_profile_source());
    assert_eq!(profile.exclusions.len(), 8);
    assert_eq!(
        profile
            .contract_deltas
            .iter()
            .map(|row| (
                row.item.as_str(),
                row.owning_slice.as_str(),
                row.state.as_str()
            ))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            (
                "boundary-bound error presentation complete evidence",
                "R1 Error presentation",
                "active",
            ),
            (
                "post-commit compound settlement complete evidence",
                "R2 Compound settlement",
                "active",
            ),
            (
                "host re-admission fresh-epoch complete evidence",
                "R3 Re-admission/stale output",
                "active",
            ),
        ])
    );
    assert_eq!(
        profile
            .authority_additions
            .iter()
            .map(|row| (
                row.item.as_str(),
                row.owning_slice.as_str(),
                row.state.as_str()
            ))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            (
                "error_outlet_role_classification_is_preserved",
                "R1 Error presentation",
                "active",
            ),
            (
                "post_commit_host_loss_preserves_committed_history",
                "R2 Compound settlement",
                "active",
            ),
            (
                "host_reacceptance_detaches_old_top_slot",
                "R3 Re-admission/stale output",
                "active",
            ),
        ])
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
            "host_reacceptance_detaches_old_top_slot",
            "post_commit_host_loss_preserves_committed_history",
        ]),
        "Recovery must activate exactly the completed R1–R3 authority rows"
    );
}

#[test]
fn authority_public_activation_has_exact_machine_state() {
    let rows = authority_public_activation();
    assert_eq!(rows.len(), 8);
    assert!(rows.iter().all(|row| row.state == "active"));
    assert!(
        rows.iter()
            .all(|row| row.evidence == "public_navigation_input_isolation PASS")
    );

    let source = read_workspace("docs/implementation-goals.md");
    for malformed in [
        source.replacen(
            "| `bevy_ui_router::ecs::InputProbeResult` | active | public_navigation_input_isolation PASS |",
            "| `bevy_ui_router::ecs::UnknownProbe` | active | public_navigation_input_isolation PASS |",
            1,
        ),
        source.replacen(
            "| `bevy_ui_router::ecs::InputProbeResult` | active | public_navigation_input_isolation PASS |",
            "| `bevy_ui_router::ecs::InputProbeResult` | planned | public_navigation_input_isolation PASS |",
            1,
        ),
        source.replacen(
            "| `bevy_ui_router::ecs::InputProbeResult` | active | public_navigation_input_isolation PASS |",
            "| `bevy_ui_router::ecs::InputProbeResult` | active | private unit evidence |",
            1,
        ),
    ] {
        assert!(
            std::panic::catch_unwind(|| parse_authority_public_activation(&malformed)).is_err(),
            "Authority activation must reject noncanonical rows"
        );
    }
}

#[test]
fn active_stage_profile_preserves_history_union_at_authority_activation() {
    let profile = stage_contract_profile();
    assert!(profile.exclusions.is_empty());
    assert_eq!(profile.contract_deltas.len(), 6);
    assert_eq!(profile.authority_additions.len(), 4);
    assert_eq!(
        profile
            .authority_additions
            .iter()
            .filter(|row| row.state == "active")
            .count(),
        4,
        "the completed History and Recovery R1–R3 authority rows are active"
    );
    assert_eq!(
        profile
            .contract_deltas
            .iter()
            .filter(|row| row.state == "active")
            .map(|row| row.item.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "NavigationRequestSnapshot::requested -> Option",
            "NavigationEnqueueErrorKind::IdentityExhausted",
            "boundary-bound error presentation complete evidence",
            "late-bound back complete evidence",
            "post-commit compound settlement complete evidence",
            "host re-admission fresh-epoch complete evidence",
        ])
    );
    assert_eq!(
        profile
            .contract_deltas
            .iter()
            .filter(|row| row.state == "planned")
            .map(|row| row.item.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::new()
    );
}

#[test]
fn recovery_profile_keys_are_disjoint() {
    let profile = parse_recovery_contract_profile(&recovery_profile_source());
    profile.assert_disjoint_keys();
}

#[test]
fn recovery_profile_rejects_invalid_10_1_rows() {
    let source = recovery_profile_source();
    let _control = parse_recovery_contract_profile(&source);
    let rows = [
        source.replacen(
            "| symbol | `bevy_ui_router::ecs::QuarantineLeaseId` | Authority closure |",
            "| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |",
            1,
        ),
        source.replacen(
            "| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |",
            "| unknown | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |",
            1,
        ),
        source.replacen(
            "| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |",
            "| symbol | `InputProbeResult` | Authority closure |",
            1,
        ),
        source.replacen(
            "| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |",
            "| symbol | `bevy_ui_router::ecs::InputProbeResult` | Unknown slice |",
            1,
        ),
        source.replacen(
            "| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |",
            "| symbol |  | Authority closure |",
            1,
        ),
    ];
    for row in rows {
        assert!(
            std::panic::catch_unwind(|| parse_recovery_contract_profile(&row)).is_err(),
            "Recovery 10.1 invalid exclusion rows must be rejected"
        );
    }
}

#[test]
fn recovery_profile_rejects_invalid_10_2_rows() {
    let source = recovery_profile_source();
    let _control = parse_recovery_contract_profile(&source);
    let rows = [
        source.replacen(
            "| post-commit compound settlement complete evidence | R2 Compound settlement | active | Journey G post-commit foreground rows PASS |",
            "| boundary-bound error presentation complete evidence | R1 Error presentation | active | Journey D public contract PASS |",
            1,
        ),
        source.replacen(
            "| boundary-bound error presentation complete evidence | R1 Error presentation | active | Journey D public contract PASS |",
            "|  | R1 Error presentation | active | Journey D public contract PASS |",
            1,
        ),
        source.replacen(
            "| boundary-bound error presentation complete evidence | R1 Error presentation | active | Journey D public contract PASS |",
            "| unknown Recovery item | R1 Error presentation | active | Journey D public contract PASS |",
            1,
        ),
        source.replacen(
            "| boundary-bound error presentation complete evidence | R1 Error presentation | active | Journey D public contract PASS |",
            "| boundary-bound error presentation complete evidence | Unknown slice | active | Journey D public contract PASS |",
            1,
        ),
        source.replacen(
            "| boundary-bound error presentation complete evidence | R1 Error presentation | active | Journey D public contract PASS |",
            "| boundary-bound error presentation complete evidence | R1 Error presentation | unknown | Journey D public contract PASS |",
            1,
        ),
    ];
    for row in rows {
        assert!(
            std::panic::catch_unwind(|| parse_recovery_contract_profile(&row)).is_err(),
            "Recovery 10.2 invalid contract-delta rows must be rejected"
        );
    }
}

#[test]
fn recovery_profile_rejects_invalid_10_3_rows() {
    let source = recovery_profile_source();
    let _control = parse_recovery_contract_profile(&source);
    let rows = [
        source.replacen(
            "| `post_commit_host_loss_preserves_committed_history` | R2 Compound settlement | active |",
            "| `error_outlet_role_classification_is_preserved` | R1 Error presentation | active |",
            1,
        ),
        source.replacen(
            "| `error_outlet_role_classification_is_preserved` | R1 Error presentation | active |",
            "| `` | R1 Error presentation | active |",
            1,
        ),
        source.replacen(
            "| `error_outlet_role_classification_is_preserved` | R1 Error presentation | active |",
            "| `unknown_recovery_test` | R1 Error presentation | active |",
            1,
        ),
        source.replacen(
            "| `error_outlet_role_classification_is_preserved` | R1 Error presentation | active |",
            "| `error_outlet_role_classification_is_preserved` | Unknown slice | active |",
            1,
        ),
        source.replacen(
            "| `error_outlet_role_classification_is_preserved` | R1 Error presentation | active |",
            "| `error_outlet_role_classification_is_preserved` | R1 Error presentation | unknown |",
            1,
        ),
    ];
    for row in rows {
        assert!(
            std::panic::catch_unwind(|| parse_recovery_contract_profile(&row)).is_err(),
            "Recovery 10.3 invalid authority rows must be rejected"
        );
    }
}

#[test]
fn recovery_profile_requires_exact_unique_section_tables() {
    let source = recovery_profile_source()
        .replace("\r\n", "\n")
        .replace('\n', "\r\n");
    let _control = parse_recovery_contract_profile(&source);
    let misordered = source
        .replacen(
            "### 10.1 Public contract and stage exclusions",
            "### temporary Recovery heading",
            1,
        )
        .replacen(
            "### 10.2 Contract delta activation",
            "### 10.1 Public contract and stage exclusions",
            1,
        )
        .replacen(
            "### temporary Recovery heading",
            "### 10.2 Contract delta activation",
            1,
        );
    let malformed_10_3_separator = {
        let mut document = source.clone();
        let section = document
            .find("### 10.3 Recovery authority regression additions")
            .expect("Recovery 10.3 section must exist");
        let separator_text = "| --- | --- | --- |";
        let separator = section
            + document[section..]
                .find(separator_text)
                .expect("Recovery 10.3 separator must exist");
        document.replace_range(
            separator..separator + separator_text.len(),
            "| --- | --- | -- |",
        );
        document
    };
    let malformed = [
        format!("{source}\n### 10.1 Public contract and stage exclusions\n"),
        misordered,
        source.replacen(
            "### 10.1 Public contract and stage exclusions",
            "### 10.1 Public contract and stage exclusions\n\n | Kind | Canonical path or normalized item | First slice |\n | --- | --- | --- |\n | symbol | `bevy_ui_router::ecs::IndentedBefore` | Authority closure |",
            1,
        ),
        source.replacen(
            "\n### 10.2 Contract delta activation",
            "\n | Kind | Canonical path or normalized item | First slice |\n | --- | --- | --- |\n | symbol | `bevy_ui_router::ecs::IndentedAfter` | Authority closure |\n\n### 10.2 Contract delta activation",
            1,
        ),
        source.replacen(
            "| Item | Owning slice | State | Claim boundary |",
            "| Wrong | Owning slice | State | Claim boundary |\n| --- | --- | --- | --- |\n| wrong | R1 Error presentation | planned | Journey D public contract PASS |\n\n| Item | Owning slice | State | Claim boundary |",
            1,
        ),
        malformed_10_3_separator,
    ];
    for (index, document) in malformed.into_iter().enumerate() {
        assert_ne!(
            document, source,
            "negative fixture {index} must mutate canonical source"
        );
        assert!(
            std::panic::catch_unwind(|| parse_recovery_contract_profile(&document)).is_err(),
            "Recovery profile fixture {index} must use one ordered, exact first table"
        );
    }
}

#[test]
fn history_profile_has_exact_back_execution_slice_machine_state() {
    let profile = history_contract_profile();
    assert_eq!(profile.exclusions.len(), 10);
    assert_eq!(profile.contract_deltas.len(), 3);
    assert_eq!(
        profile
            .contract_deltas
            .iter()
            .map(|row| (row.item.as_str(), row.state.as_str()))
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            ("NavigationRequestSnapshot::requested -> Option", "active",),
            ("NavigationEnqueueErrorKind::IdentityExhausted", "active",),
            ("late-bound back complete evidence", "active"),
        ])
    );
    assert_eq!(profile.authority_additions.len(), 1);
    assert!(
        profile
            .authority_additions
            .iter()
            .all(|row| row.state == "active")
    );
}

#[test]
fn history_profile_keys_are_disjoint() {
    let profile = history_contract_profile();
    profile.assert_disjoint_keys();
}

#[test]
fn active_stage_profile_tables_and_curated_facade_source_are_closed_world() {
    let allowlist = read_workspace("docs/facade-export-allowlist.md");
    let profile = stage_contract_profile();
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
    let exclusions = profile.exclusions;
    assert_eq!(modules.len(), 7);
    assert!(symbols.len() > 100);
    assert!(exclusions.is_empty());

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
fn isolated_positive_consumers_cover_the_active_stage_feature_matrix() {
    let allowlist = read_workspace("docs/facade-export-allowlist.md");
    let profile = stage_contract_profile();
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
    let exclusions = profile.exclusions;
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
