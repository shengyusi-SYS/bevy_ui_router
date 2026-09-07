#[path = "support/dependency_policy.rs"]
mod dependency_policy;

use dependency_policy::{config_violations, is_bevy_package, lock_violations, manifest_violations};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml::Value;

fn parse(source: &str) -> Value {
    toml::from_str(source).expect("valid Cargo TOML")
}

fn read(path: &Path) -> Value {
    parse(&fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

#[test]
fn workspace_uses_official_bevy_without_source_overrides() {
    let root = workspace();
    let manifest = read(&root.join("Cargo.toml"));
    assert_eq!(manifest_violations(&manifest), Vec::<String>::new());
    let mut config_bases = root
        .ancestors()
        .map(Path::to_owned)
        .collect::<BTreeSet<_>>();
    for member in manifest["workspace"]["members"].as_array().unwrap() {
        let directory = root.join(member.as_str().unwrap());
        assert_eq!(
            manifest_violations(&read(&directory.join("Cargo.toml"))),
            Vec::<String>::new(),
            "{}",
            directory.display()
        );
        config_bases.extend(directory.ancestors().map(Path::to_owned));
    }
    let mut configs = config_bases
        .into_iter()
        .map(|base| (base.join(".cargo"), base))
        .collect::<Vec<_>>();
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).join(".cargo"))
        });
    if let Some(cargo_home) = cargo_home {
        let cargo_home = root.join(cargo_home);
        // Cargo uses the directory containing CARGO_HOME for relative config paths too.
        let base = cargo_home.parent().expect("Cargo home parent").to_owned();
        configs.push((cargo_home, base));
    }
    for (directory, base) in configs {
        let legacy = directory.join("config");
        let config = if legacy.is_file() {
            legacy
        } else {
            directory.join("config.toml")
        };
        if config.is_file() {
            assert_eq!(
                config_violations(&read(&config), &base),
                Vec::<String>::new(),
                "{}",
                config.display()
            );
        }
    }
    assert_eq!(
        lock_violations(&read(&root.join("Cargo.lock"))),
        Vec::<String>::new()
    );
}

#[test]
fn renderer_neutral_packages_and_default_facade_resolve_without_bevy() {
    for (package, features, all_features) in [
        ("bevy_ui_router_core", None, false),
        ("bevy_ui_router_view", None, false),
        ("bevy_ui_router_core", None, true),
        ("bevy_ui_router_view", None, true),
        ("bevy_ui_router", None, false),
        ("bevy_ui_router", Some(""), false),
        ("bevy_ui_router", Some("view"), false),
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
                is_bevy_package(name)
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
            "{p}",
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
    }
}

#[test]
fn dependency_policy_accepts_aliases_features_and_both_line_endings() {
    let source = r#"
[dependencies]
ui = { package = "bevy_ui", version = "=0.19.1", features = ["bevy_picking"], optional = true }
[target.'cfg(windows)'.dev-dependencies]
bevy_app = "0.19.1"
[workspace.dependencies]
bevy_ecs = { version = "0.19.1", default-features = false }
[package.metadata.bevy_app]
path = "not a dependency"
"#;
    for text in [source.to_owned(), source.replace('\n', "\r\n")] {
        assert_eq!(manifest_violations(&parse(&text)), Vec::<String>::new());
    }
}

#[test]
fn dependency_policy_rejects_fork_patch_and_replacement_inputs() {
    for (source, expected) in [
        (
            "[target.'cfg(windows)'.build-dependencies]\nui = { package = 'bevy_ui', git = 'https://example.invalid/fork' }",
            "bevy_ui: source override `git` is forbidden",
        ),
        (
            "[workspace.dependencies]\nbevy_ecs = { path = '../bevy_ecs' }",
            "bevy_ecs: source override `path` is forbidden",
        ),
        (
            "[dependencies]\nbevy_scene = { version = '0.19.1', registry = 'alternate' }",
            "bevy_scene: source override `registry` is forbidden",
        ),
        (
            "[patch.crates-io]\nscene = { package = 'bevy_scene', path = '../scene' }",
            "bevy_scene: Bevy patch is forbidden",
        ),
        (
            "[replace]\n'bevy_ecs:0.19.1' = { path = '../bevy_ecs' }",
            "bevy_ecs: Bevy replacement is forbidden",
        ),
    ] {
        assert_eq!(
            manifest_violations(&parse(source)),
            vec![expected.to_owned()]
        );
    }
    assert_eq!(
        config_violations(
            &parse("[source.crates-io]\nreplace-with = 'vendor'"),
            Path::new(".")
        ),
        vec!["crates.io source override `replace-with` is forbidden"]
    );
}

#[test]
fn config_patch_and_qualified_replacement_cannot_bypass_source_policy() {
    assert_eq!(
        config_violations(
            &parse("[patch.crates-io]\nbevy_ecs = { path = '../ecs' }"),
            Path::new(".")
        ),
        vec!["bevy_ecs: Bevy patch is forbidden"]
    );
    for identity in [
        "bevy_ecs@0.19.1",
        "registry+https://github.com/rust-lang/crates.io-index#bevy_ecs@0.19.1",
        "https://example.invalid/bevy_ecs#0.19.1",
    ] {
        assert_eq!(
            manifest_violations(&parse(&format!(
                "[replace]\n'{identity}' = {{ path = '../ecs' }}"
            ))),
            vec!["bevy_ecs: Bevy replacement is forbidden"]
        );
    }
}

#[test]
fn cargo_paths_override_checks_target_package_not_directory_name() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = workspace()
        .join("target/dependency-policy-config")
        .join(format!("{}-{stamp}", std::process::id()));
    for (directory, package) in [
        ("ordinary-path", "bevy_ecs"),
        ("bevy-path", "ordinary_dependency"),
    ] {
        let directory = root.join(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("Cargo.toml"),
            format!("[package]\nname = '{package}'\nversion = '0.19.1'\n[lib]\npath = 'lib.rs'\n"),
        )
        .unwrap();
        fs::write(directory.join("lib.rs"), "").unwrap();
    }
    assert_eq!(
        config_violations(&parse("paths = ['ordinary-path', 'bevy-path']"), &root),
        vec!["bevy_ecs: Cargo paths override is forbidden"]
    );
    assert_eq!(
        config_violations(&parse("paths = ['bevy-path']"), &root),
        Vec::<String>::new()
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn lock_policy_rejects_versions_outside_the_expected_release_line() {
    for version in ["0.18.1", "0.19.0", "0.20.0"] {
        let lock = parse(&format!(
            "[[package]]\nname = 'bevy_ecs'\nversion = '{version}'\nsource = 'registry+https://github.com/rust-lang/crates.io-index'"
        ));
        assert_eq!(
            lock_violations(&lock),
            vec!["bevy_ecs: must resolve to Bevy 0.19.1"]
        );
    }
    for source in ["", "source = 'git+https://example.invalid/fork'"] {
        let lock = parse(&format!(
            "[[package]]\nname = 'bevy_ecs'\nversion = '0.19.1'\n{source}"
        ));
        assert_eq!(
            lock_violations(&lock),
            vec!["bevy_ecs: must resolve from crates.io"]
        );
    }
}
