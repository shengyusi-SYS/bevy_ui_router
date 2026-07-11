use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture<'a> {
    name: &'a str,
    code: &'a str,
    features: &'a [&'a str],
    expected_stderr: &'a [&'a str],
}

fn path_for_toml(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn scratch_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("bevy-ui-router-core-{name}-{stamp}"))
}

fn dependency_features(features: &[&str]) -> String {
    if features.is_empty() {
        String::new()
    } else {
        format!(
            ", features = [{}]",
            features
                .iter()
                .map(|feature| format!("\"{feature}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn assert_compile_fails(fixture: Fixture<'_>) {
    let root = scratch_dir(fixture.name);
    fs::create_dir_all(root.join("src")).expect("fixture src directory should be created");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2024"

[dependencies]
bevy_ui_router_core = {{ path = "{path}"{features} }}
"#,
            name = fixture.name.replace('_', "-"),
            path = path_for_toml(manifest_dir),
            features = dependency_features(fixture.features),
        ),
    )
    .expect("fixture manifest should be written");
    fs::write(root.join("src/main.rs"), fixture.code).expect("fixture source should be written");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .arg("check")
        .arg("--config")
        .arg("build.rustc-wrapper = ''")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("cargo check should run");

    assert!(
        !output.status.success(),
        "fixture `{}` unexpectedly compiled",
        fixture.name
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in fixture.expected_stderr {
        assert!(
            stderr.contains(expected),
            "fixture `{}` stderr did not contain `{expected}`:\n{stderr}",
            fixture.name
        );
    }
}

#[test]
fn route_view_key_error_unit_literal_is_private() {
    assert_compile_fails(Fixture {
        name: "route_view_key_error_unit_literal",
        code: "use bevy_ui_router_core::view_key::RouteViewKeyError; fn main() { let _ = RouteViewKeyError; }",
        features: &[],
        expected_stderr: &[
            "error[E0423]",
            "expected value, found struct `RouteViewKeyError`",
        ],
    });
}

#[test]
fn route_view_key_error_struct_literal_is_private() {
    assert_compile_fails(Fixture {
        name: "route_view_key_error_struct_literal",
        code: "use bevy_ui_router_core::view_key::RouteViewKeyError; fn main() { let _ = RouteViewKeyError { _private: () }; }",
        features: &[],
        expected_stderr: &[
            "error[E0451]",
            "field `_private` of struct `RouteViewKeyError` is private",
        ],
    });
}

#[test]
fn default_consumer_cannot_declare_child_route() {
    assert_compile_fails(Fixture {
        name: "default_child_route",
        code: "use bevy_ui_router_core::pattern::RoutePattern; use bevy_ui_router_core::route_tree::RouteTree; fn main() { let mut builder = RouteTree::builder().root(RoutePattern::path(\"/\").unwrap()).unwrap(); let root = builder.root_id().unwrap(); let _ = builder.route(root, RoutePattern::path(\"child\").unwrap()); }",
        features: &[],
        expected_stderr: &["error[E0624]", "method `route` is private"],
    });
}

#[test]
fn test_support_consumer_cannot_declare_child_route() {
    assert_compile_fails(Fixture {
        name: "test_support_child_route",
        code: "use bevy_ui_router_core::pattern::RoutePattern; use bevy_ui_router_core::route_tree::RouteTree; fn main() { let mut builder = RouteTree::builder().root(RoutePattern::path(\"/\").unwrap()).unwrap(); let root = builder.root_id().unwrap(); let _ = builder.route(root, RoutePattern::path(\"child\").unwrap()); }",
        features: &["test-support"],
        expected_stderr: &["error[E0624]", "method `route` is private"],
    });
}

#[test]
fn default_consumer_cannot_declare_child_route_with_options() {
    assert_compile_fails(Fixture {
        name: "default_child_route_with_options",
        code: "use bevy_ui_router_core::pattern::RoutePattern; use bevy_ui_router_core::route_tree::{RouteNodeOptions, RouteTree}; fn main() { let mut builder = RouteTree::builder().root(RoutePattern::path(\"/\").unwrap()).unwrap(); let root = builder.root_id().unwrap(); let _ = builder.route_with_options(root, RoutePattern::path(\"child\").unwrap(), RouteNodeOptions::new()); }",
        features: &[],
        expected_stderr: &["error[E0624]", "method `route_with_options` is private"],
    });
}

#[test]
fn test_support_consumer_cannot_declare_child_route_with_options() {
    assert_compile_fails(Fixture {
        name: "test_support_child_route_with_options",
        code: "use bevy_ui_router_core::pattern::RoutePattern; use bevy_ui_router_core::route_tree::{RouteNodeOptions, RouteTree}; fn main() { let mut builder = RouteTree::builder().root(RoutePattern::path(\"/\").unwrap()).unwrap(); let root = builder.root_id().unwrap(); let _ = builder.route_with_options(root, RoutePattern::path(\"child\").unwrap(), RouteNodeOptions::new()); }",
        features: &["test-support"],
        expected_stderr: &["error[E0624]", "method `route_with_options` is private"],
    });
}
