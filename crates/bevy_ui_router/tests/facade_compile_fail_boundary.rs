use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture<'a> {
    name: &'a str,
    code: &'a str,
    default_features: bool,
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
    std::env::temp_dir().join(format!("bevy-ui-router-facade-ui-{name}-{stamp}"))
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
bevy_ui_router = {{ path = "{path}", default-features = {default_features}{features} }}
"#,
            name = fixture.name.replace('_', "-"),
            path = path_for_toml(manifest_dir),
            default_features = fixture.default_features,
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
fn no_default_view_module_names_missing_module() {
    assert_compile_fails(Fixture {
        name: "no_default_has_no_view_module",
        code: "use bevy_ui_router::view::RouteViewDescriptor; fn main() {}",
        default_features: false,
        features: &[],
        expected_stderr: &["error[E0432]", "could not find `view` in `bevy_ui_router`"],
    });
}

#[test]
fn default_protocol_record_not_in_prelude_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "default_protocol_not_in_prelude",
        code: "use bevy_ui_router::prelude::ViewBuildRequest; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &["error[E0432]", "no `ViewBuildRequest` in `prelude`"],
    });
}

#[test]
fn default_fake_renderer_view_root_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "default_fake_renderer_not_exposed",
        code: "use bevy_ui_router::view::FakeViewRenderer; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &["error[E0432]", "no `FakeViewRenderer` in the root"],
    });
}

#[test]
fn default_fake_renderer_prelude_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "default_fake_renderer_not_in_prelude",
        code: "use bevy_ui_router::prelude::FakeViewRenderer; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &["error[E0432]", "no `FakeViewRenderer` in `prelude`"],
    });
}

#[test]
fn default_view_test_support_fake_renderer_names_missing_module() {
    assert_compile_fails(Fixture {
        name: "default_fake_renderer_no_view_test_support",
        code: "use bevy_ui_router::view::test_support::FakeViewRenderer; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &["error[E0432]", "could not find `test_support` in `view`"],
    });
}

#[test]
fn default_view_prelude_fake_renderer_mode_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "default_fake_renderer_mode_not_in_view_prelude",
        code: "use bevy_ui_router::view::prelude::FakeViewRendererMode; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &["error[E0432]", "no `FakeViewRendererMode` in `prelude`"],
    });
}

#[test]
fn default_prelude_runtime_bridge_names_missing_router_runtime() {
    assert_compile_fails(Fixture {
        name: "default_prelude_no_router_runtime",
        code: "use bevy_ui_router::prelude::RouterRuntime; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &["error[E0432]", "no `RouterRuntime` in `prelude`"],
    });
}

#[test]
fn default_prelude_runtime_bridge_error_types_name_missing_symbols() {
    assert_compile_fails(Fixture {
        name: "default_prelude_no_runtime_bridge_errors",
        code: "use bevy_ui_router::prelude::{BeginNavigationError, FrameApplyReportError, FailureCleanupAckError}; fn main() {}",
        default_features: true,
        features: &[],
        expected_stderr: &[
            "error[E0432]",
            "no `BeginNavigationError` in `prelude`",
            "no `FrameApplyReportError` in `prelude`",
            "no `FailureCleanupAckError` in `prelude`",
        ],
    });
}
