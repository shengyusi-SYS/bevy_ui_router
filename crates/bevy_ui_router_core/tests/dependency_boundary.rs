use std::fs;
use std::path::Path;

fn section_name(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']')
}

fn dependency_section_kind(section: &str) -> Option<&str> {
    let section = if let Some(target_tail) = section.strip_prefix("target.") {
        let (_, remainder) = target_tail.split_once('.')?;
        remainder
    } else {
        section
    };

    ["dependencies", "dev-dependencies", "build-dependencies"]
        .into_iter()
        .find(|kind| *kind == section || section.starts_with(&format!("{kind}.")))
}

fn dependency_subtable_alias(section: &str) -> Option<&str> {
    let kind = dependency_section_kind(section)?;
    section
        .strip_prefix("target.")
        .and_then(|tail| tail.split_once('.').map(|(_, remainder)| remainder))
        .unwrap_or(section)
        .strip_prefix(kind)?
        .strip_prefix('.')
}

fn dependency_entries(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut dependency_entries = Vec::new();

    for (line_number, raw_line) in manifest.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);

            if dependency_subtable_alias(section).is_some() {
                dependency_entries.push(format!(
                    "line {}: forbidden dependency subtable `{}`",
                    line_number + 1,
                    section
                ));
            }
            continue;
        }

        if current_section.is_some_and(|section| dependency_section_kind(section).is_some()) {
            dependency_entries.push(format!("line {}: {}", line_number + 1, line));
        }
    }

    dependency_entries
}

#[test]
fn core_manifest_does_not_define_dependency_entries() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "expected dependency-like sections to stay empty, found entries:\n{}",
        dependency_entries.join("\n")
    );
}

#[test]
fn core_manifest_still_has_no_dependency_entries_after_matcher() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "matcher phase must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}

#[test]
fn branch_planning_manifest_stays_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "Phase 1B branch planning must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}

#[test]
fn dependency_subtables_are_treated_as_violations() {
    let manifest = r#"
[dependencies]

[dependencies.some_alias]
package = "bevy_app"

[package.metadata.dependencies]
package = "bsn"

[target.'cfg(windows)'.dependencies]
bevy_ui_router_core = { path = "../core" }

[target.'cfg(unix)'.dev-dependencies.tooling]
version = "1"

[target.'cfg(unix)'.package.metadata.dependencies]
package = "bevy_ecs"
"#;

    let dependency_entries = dependency_entries(manifest);

    assert_eq!(
        dependency_entries,
        vec![
            "line 4: forbidden dependency subtable `dependencies.some_alias`",
            "line 5: package = \"bevy_app\"",
            "line 11: bevy_ui_router_core = { path = \"../core\" }",
            "line 13: forbidden dependency subtable `target.'cfg(unix)'.dev-dependencies.tooling`",
            "line 14: version = \"1\"",
        ]
    );
}

#[test]
fn phase_2_frame_transaction_manifest_stays_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "Phase 2 frame and transaction work must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}

#[test]
fn phase_3b_runtime_bridge_manifest_stays_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "Phase 3B runtime bridge work must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}
