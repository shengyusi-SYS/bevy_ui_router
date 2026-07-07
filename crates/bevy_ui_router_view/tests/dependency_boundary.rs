use std::collections::BTreeMap;
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

fn dependency_name_is_forbidden(name: &str) -> bool {
    name == "bsn" || name.starts_with("bevy") && !name.starts_with("bevy_ui_router")
}

fn quoted_string(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(&value[..end])
}

fn package_rename(key: &str, value: &str) -> Option<String> {
    let value = value.split('#').next().unwrap_or(value).trim();

    if key == "package" {
        return quoted_string(value).map(str::to_owned);
    }

    let inline_table = value.strip_prefix('{')?.trim_end_matches('}').trim();
    for entry in inline_table.split(',') {
        let Some((field, field_value)) = entry.split_once('=') else {
            continue;
        };

        if field.trim() == "package" {
            return quoted_string(field_value).map(str::to_owned);
        }
    }

    None
}

fn dependency_entries(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut entries = Vec::new();
    let mut in_dependency_subtable = false;

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);
            if let Some(alias) = dependency_subtable_alias(section) {
                entries.push(alias.to_owned());
                in_dependency_subtable = true;
            } else {
                in_dependency_subtable = false;
            }
            continue;
        }

        if !current_section.is_some_and(|section| dependency_section_kind(section).is_some()) {
            continue;
        }

        if in_dependency_subtable {
            continue;
        }

        let Some((name, _value)) = line.split_once('=') else {
            continue;
        };
        entries.push(name.trim().to_owned());
    }

    entries
}

fn dependency_violations(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut violations = Vec::new();

    for (line_number, raw_line) in manifest.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);

            if let Some(alias) = dependency_subtable_alias(section) {
                if dependency_name_is_forbidden(alias) {
                    violations.push(format!(
                        "line {}: forbidden dependency subtable `{}`",
                        line_number + 1,
                        section
                    ));
                }
            }
            continue;
        }

        if !current_section.is_some_and(|section| dependency_section_kind(section).is_some()) {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        if dependency_subtable_alias(current_section.expect("checked above")).is_none()
            && dependency_name_is_forbidden(key)
        {
            violations.push(format!(
                "line {}: forbidden dependency key `{}`",
                line_number + 1,
                key
            ));
        }

        if let Some(package) = package_rename(key, value) {
            if package == "bsn" {
                violations.push(format!(
                    "line {}: forbidden package rename to `bsn`",
                    line_number + 1
                ));
            } else if package.starts_with("bevy") && !package.starts_with("bevy_ui_router") {
                violations.push(format!(
                    "line {}: forbidden package rename to Bevy-family crate",
                    line_number + 1
                ));
            }
        }
    }

    violations
}

fn feature_entries(manifest: &str) -> BTreeMap<String, String> {
    let mut current_section = None;
    let mut features = BTreeMap::new();

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);
            continue;
        }

        if current_section == Some("features") {
            let (name, value) = line
                .split_once('=')
                .expect("feature entries should contain '='");
            features.insert(name.trim().to_owned(), value.trim().to_owned());
        }
    }

    features
}

fn dependency_default_features_disabled(manifest: &str, dependency: &str) -> bool {
    let mut in_dependency_subtable = false;

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            in_dependency_subtable = dependency_subtable_alias(section) == Some(dependency);
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == dependency {
                return value.contains("default-features = false")
                    || value.contains("default-features=false");
            }

            if in_dependency_subtable && key == "default-features" {
                return value == "false";
            }
        }
    }

    false
}

#[test]
fn view_manifest_depends_only_on_core_and_keeps_test_support_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("view manifest should be readable");

    assert_eq!(dependency_entries(&manifest), vec!["bevy_ui_router_core"]);
    assert!(dependency_violations(&manifest).is_empty());
    assert_eq!(
        feature_entries(&manifest),
        BTreeMap::from([
            ("default".to_owned(), "[]".to_owned()),
            ("test-support".to_owned(), "[]".to_owned()),
        ])
    );
}

#[test]
fn dependency_subtables_and_package_renames_are_checked() {
    let manifest = r#"
[dependencies]
render_inline = { package = "bevy_render", version = "1" }
core_inline = { package = "bevy_ui_router_core", path = "../core" }

[dependencies.bsn]
version = "1"

[target.'cfg(target_os = "linux")'.dev-dependencies.bevy_input]
version = "1"

[target.'cfg(target_os = "windows")'.build-dependencies.integration]
package = "bsn"
"#;

    assert_eq!(
        dependency_violations(manifest),
        vec![
            "line 3: forbidden package rename to Bevy-family crate",
            "line 6: forbidden dependency subtable `dependencies.bsn`",
            "line 9: forbidden dependency subtable `target.'cfg(target_os = \"linux\")'.dev-dependencies.bevy_input`",
            "line 13: forbidden package rename to `bsn`",
        ]
    );
}

#[test]
fn dependency_entries_include_dependency_subtable_aliases() {
    let manifest = r#"
[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }

[dependencies.some_allowed_crate]
version = "1"

[target.'cfg(target_os = "linux")'.dev-dependencies.target_helper]
version = "1"
"#;

    assert_eq!(
        dependency_entries(manifest),
        vec!["bevy_ui_router_core", "some_allowed_crate", "target_helper",]
    );
}

#[test]
fn dependency_default_features_helper_accepts_inline_and_subtable_forms() {
    let inline = r#"
[dependencies]
bevy_ui_router_view = { path = "../view", default-features = false }
"#;
    let subtable = r#"
[dependencies.bevy_ui_router_view]
path = "../view"
default-features = false
"#;

    assert!(dependency_default_features_disabled(
        inline,
        "bevy_ui_router_view"
    ));
    assert!(dependency_default_features_disabled(
        subtable,
        "bevy_ui_router_view"
    ));
}

#[test]
fn facade_depends_on_view_with_default_features_disabled() {
    let facade_manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("view crate should have a workspace parent")
        .join("bevy_ui_router")
        .join("Cargo.toml");
    let manifest =
        fs::read_to_string(facade_manifest_path).expect("facade manifest should be readable");

    assert!(dependency_default_features_disabled(
        &manifest,
        "bevy_ui_router_view"
    ));
}
