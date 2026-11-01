#[path = "support/dependency_graph.rs"]
mod dependency_graph;

use dependency_graph::is_bevy_dependency;
use std::{collections::BTreeSet, path::PathBuf, process::Command};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

#[test]
fn renderer_neutral_packages_and_default_facade_resolve_without_bevy() {
    for (package, features, all_features) in [
        ("bevy_ui_router_core", None, false),
        ("bevy_ui_router_view", None, false),
        ("bevy_ui_router_platform", None, false),
        ("bevy_ui_router_core", None, true),
        ("bevy_ui_router_view", None, true),
        ("bevy_ui_router", None, false),
        ("bevy_ui_router", Some(""), false),
        ("bevy_ui_router", Some("view"), false),
        ("bevy_ui_router", Some("platform"), false),
    ] {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(workspace()).args([
            "tree",
            "--offline",
            "--locked",
            "--edges",
            "normal,build",
            "--target",
            "all",
            "--no-dedupe",
            "--prefix",
            "none",
            "--format",
            "{p}|{f}",
            "--package",
            package,
        ]);
        if all_features {
            command.arg("--all-features");
        }
        if let Some(features) = features {
            command.arg("--no-default-features");
            if !features.is_empty() {
                command.args(["--features", features]);
            }
        }
        let output = command.output().expect("Cargo tree runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tree = String::from_utf8(output.stdout).unwrap();
        assert!(
            !tree.trim().is_empty(),
            "Cargo must return an actual dependency graph"
        );
        let forbidden = tree
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|name| {
                is_bevy_dependency(name)
                    || matches!(*name, "bevy_ui_router_ecs" | "bevy_ui_router_bsn")
            })
            .collect::<Vec<_>>();
        assert!(
            forbidden.is_empty(),
            "{package} ({features:?}) resolved renderer dependencies: {forbidden:?}"
        );
        for line in tree.lines() {
            let (_, enabled) = line.split_once('|').expect("Cargo feature output");
            assert!(
                all_features
                    || !enabled
                        .split(',')
                        .any(|feature| matches!(feature, "test-support" | "ecs-test-support")),
                "{package} ({features:?}) enables test support in its production graph: {line}"
            );
        }
    }
}

#[test]
fn bsn_dependency_direction_and_optional_bridge_resolve_as_declared() {
    for (package, features, expect_bsn, expect_ecs, expect_scene) in [
        ("bevy_ui_router_bsn", None, true, false, true),
        ("bevy_ui_router_ecs", None, false, true, false),
        ("bevy_ui_router_ecs", Some("bsn"), true, true, true),
        ("bevy_ui_router", Some("bsn"), true, true, true),
        (
            "bevy_ui_router_platform",
            Some("desktop-back"),
            false,
            false,
            false,
        ),
        ("bevy_ui_router", Some("desktop-back"), false, false, false),
        (
            "bevy_ui_router_platform",
            Some("android-back"),
            false,
            false,
            false,
        ),
        ("bevy_ui_router", Some("android-back"), false, false, false),
    ] {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(workspace()).args([
            "tree",
            "--offline",
            "--locked",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}|{f}",
            "--no-default-features",
            "--package",
            package,
        ]);
        if let Some(features) = features {
            command.args(["--features", features]);
        }
        let output = command.output().expect("Cargo tree runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tree = String::from_utf8(output.stdout).unwrap();
        let packages = tree
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .collect::<BTreeSet<_>>();
        assert!(
            packages.contains(package),
            "Cargo must return the requested graph"
        );
        for (dependency, expected) in [
            ("bevy_ui_router_bsn", expect_bsn),
            ("bevy_ui_router_ecs", expect_ecs),
            ("bevy_scene", expect_scene),
        ] {
            assert_eq!(
                packages.contains(dependency),
                expected,
                "{package} ({features:?}): {dependency}"
            );
        }
        if let Some(feature @ ("desktop-back" | "android-back")) = features {
            for dependency in ["bevy_window", "bevy_winit", "winit", "android-activity"] {
                assert!(
                    !packages.contains(dependency),
                    "{feature} pulled in {dependency}"
                );
            }
            let (_, input_features) = tree
                .lines()
                .find(|line| line.starts_with("bevy_input "))
                .expect("Back adapters use Bevy input")
                .split_once('|')
                .unwrap();
            let enabled = input_features.split(',').collect::<BTreeSet<_>>();
            assert!(enabled.contains("std"));
            assert_eq!(enabled.contains("keyboard"), feature == "android-back");
            assert_eq!(enabled.contains("mouse"), feature == "desktop-back");
        }
    }
}
