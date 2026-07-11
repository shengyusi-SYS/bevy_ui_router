use std::fs;
use std::path::Path;

const BEVY_GIT: &str = "git = \"https://github.com/shengyusi-SYS/bevy.git\"";
const BEVY_REV: &str = "rev = \"61f78f75684e6b0bc28a13a5933f8f55ca653fa7\"";

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

fn manifest_contains_dependency_config(manifest: &str, dependency: &str, fragment: &str) -> bool {
    manifest.lines().any(|line| {
        let line = line.trim();
        line.starts_with(&format!("{dependency} = {{")) && line.contains(fragment)
    })
}

fn feature_entries(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut entries = Vec::new();

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if let Some(section) = section_name(line) {
            current_section = Some(section);
        } else if current_section == Some("features") && !line.is_empty() && !line.starts_with('#')
        {
            entries.push(line.to_owned());
        }
    }

    entries
}

#[test]
fn ecs_manifest_locks_the_dependency_boundary() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("ecs manifest should be readable");
    let entries = dependency_entries(&manifest);

    assert_eq!(
        entries,
        vec![
            "bevy_app",
            "bevy_camera",
            "bevy_ecs",
            "bevy_input_focus",
            "bevy_picking",
            "bevy_ui",
            "bevy_ui_router_core",
            "bevy_ui_router_view",
            "bevy_ui_router_core",
            "bevy_internal",
            "syn",
        ]
    );

    assert_eq!(
        feature_entries(&manifest),
        vec!["default = []", "test-support = []"]
    );

    for forbidden in [
        "bevy_ui_router",
        "bevy_ui_router_bsn",
        "bevy_ui_router_platform",
        "app_profile",
        "theme",
        "permissions",
        "data_service",
    ] {
        assert!(
            !entries.iter().any(|entry| entry == forbidden),
            "ecs manifest should not define forbidden dependency entry `{forbidden}`"
        );
    }

    for dependency in [
        "bevy_app",
        "bevy_camera",
        "bevy_ecs",
        "bevy_input_focus",
        "bevy_picking",
        "bevy_ui",
    ] {
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, BEVY_GIT),
            "{dependency} should use the approved Bevy git source"
        );
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, BEVY_REV),
            "{dependency} should pin the approved Bevy revision"
        );
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, "version = \"0.19.0\""),
            "{dependency} should pin the approved Bevy version"
        );
        assert!(
            manifest_contains_dependency_config(&manifest, dependency, "default-features = false"),
            "{dependency} should disable default features"
        );
        assert!(
            !manifest_contains_dependency_config(
                &manifest,
                dependency,
                "branch = \"sys/bevy-0.19-router\""
            ),
            "{dependency} must not use a movable branch"
        );
    }

    assert!(
        manifest_contains_dependency_config(&manifest, "bevy_ui", "features = [\"bevy_picking\"]"),
        "normal bevy_ui should enable bevy_picking"
    );
    assert!(
        !manifest_contains_dependency_config(&manifest, "bevy_ui", "ghost_nodes"),
        "normal bevy_ui should not enable ghost_nodes"
    );

    assert!(
        manifest_contains_dependency_config(&manifest, "bevy_internal", BEVY_GIT)
            && manifest_contains_dependency_config(&manifest, "bevy_internal", BEVY_REV)
            && manifest_contains_dependency_config(
                &manifest,
                "bevy_internal",
                "version = \"0.19.0\""
            )
            && manifest_contains_dependency_config(
                &manifest,
                "bevy_internal",
                "default-features = false"
            )
            && manifest_contains_dependency_config(
                &manifest,
                "bevy_internal",
                "features = [\"bevy_window\", \"bevy_camera\", \"bevy_ui\", \"bevy_picking\"]"
            )
            && !manifest_contains_dependency_config(
                &manifest,
                "bevy_internal",
                "branch = \"sys/bevy-0.19-router\""
            ),
        "ECS dev-dependencies should pin bevy_internal with exactly the approved feature set"
    );
    assert!(
        manifest_contains_dependency_config(&manifest, "syn", "version = \"2\"")
            && manifest_contains_dependency_config(&manifest, "syn", "features = [\"full\"]"),
        "ECS dev-dependencies should include syn with its full feature"
    );

    assert!(
        manifest_contains_dependency_config(
            &manifest,
            "bevy_ui_router_core",
            "features = [\"test-support\"]"
        ),
        "ecs tests should enable only the narrow core test-support feature through dev-dependencies"
    );
}
