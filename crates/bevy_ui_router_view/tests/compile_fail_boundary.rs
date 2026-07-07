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
    std::env::temp_dir().join(format!("bevy-ui-router-view-ui-{name}-{stamp}"))
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
    let core_dir = manifest_dir
        .parent()
        .expect("view crate should have a workspace parent")
        .join("bevy_ui_router_core");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2024"

[dependencies]
bevy_ui_router_view = {{ path = "{path}", default-features = false{features} }}
bevy_ui_router_core = {{ path = "{core_path}" }}
"#,
            name = fixture.name.replace('_', "-"),
            path = path_for_toml(manifest_dir),
            core_path = path_for_toml(&core_dir),
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
fn default_fake_renderer_root_names_missing_root_symbol() {
    assert_compile_fails(Fixture {
        name: "default_fake_renderer_root",
        code: "use bevy_ui_router_view::FakeViewRenderer; fn main() {}",
        features: &[],
        expected_stderr: &["error[E0432]", "no `FakeViewRenderer` in the root"],
    });
}

#[test]
fn default_protocol_record_not_in_prelude_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "default_protocol_not_in_prelude",
        code: "use bevy_ui_router_view::prelude::ViewBuildRequest; fn main() {}",
        features: &[],
        expected_stderr: &["error[E0432]", "no `ViewBuildRequest` in `prelude`"],
    });
}

#[test]
fn private_request_id_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_request_id_constructor",
        code: "use bevy_ui_router_view::ViewRequestId; fn main() { let _ = ViewRequestId::from_raw(1); }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `from_raw` is private"],
    });
}

#[test]
fn private_build_request_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_build_request_constructor",
        code: "use bevy_ui_router_view::ViewBuildRequest; fn main() { let _ = ViewBuildRequest::new; }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `new` is private"],
    });
}

#[test]
fn private_output_id_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_output_id_constructor",
        code: "use bevy_ui_router_view::ViewOutputId; fn main() { let _ = ViewOutputId::from_raw(1); }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `from_raw` is private"],
    });
}

#[test]
fn private_render_plan_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_render_plan_constructor",
        code: "use bevy_ui_router_view::ViewRenderPlan; fn main() { let _ = ViewRenderPlan::new; }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `new` is private"],
    });
}

#[test]
fn private_partial_output_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_partial_output_constructor",
        code: "use bevy_ui_router_view::ViewPartialOutput; fn main() { let _ = ViewPartialOutput::new; }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `new` is private"],
    });
}

#[test]
fn private_disposal_hint_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_disposal_hint_constructor",
        code: "use bevy_ui_router_view::ViewDisposalHint; fn main() { let _ = ViewDisposalHint::new; }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `new` is private"],
    });
}

#[test]
fn private_view_failure_build_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_view_failure_build_constructor",
        code: "use bevy_ui_router_view::ViewFailure; fn main() { let _ = ViewFailure::build_failed; }",
        features: &[],
        expected_stderr: &[
            "error[E0624]",
            "associated function `build_failed` is private",
        ],
    });
}

#[test]
fn private_view_failure_apply_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_view_failure_apply_constructor",
        code: "use bevy_ui_router_view::ViewFailure; fn main() { let _ = ViewFailure::apply_failed; }",
        features: &[],
        expected_stderr: &[
            "error[E0624]",
            "associated function `apply_failed` is private",
        ],
    });
}

#[test]
fn private_view_failure_stale_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_view_failure_stale_constructor",
        code: "use bevy_ui_router_view::ViewFailure; fn main() { let _ = ViewFailure::stale_result; }",
        features: &[],
        expected_stderr: &[
            "error[E0624]",
            "associated function `stale_result` is private",
        ],
    });
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_fake_renderer_root_still_names_missing_root_symbol() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_not_root",
        code: "use bevy_ui_router_view::FakeViewRenderer; fn main() {}",
        features: &["test-support"],
        expected_stderr: &["error[E0432]", "no `FakeViewRenderer` in the root"],
    });
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_fake_renderer_commit_names_missing_method() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_has_no_commit",
        code: "use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode}; fn main() { let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success); renderer.commit(); }",
        features: &["test-support"],
        expected_stderr: &["error[E0599]", "no method named `commit` found"],
    });
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_fake_renderer_token_accept_names_missing_method() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_has_no_token_accept",
        code: "use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode}; use bevy_ui_router_core::prelude::FrameApplyToken; fn main() { let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success); let token: FrameApplyToken = unreachable!(); renderer.accept_apply_token(token); }",
        features: &["test-support"],
        expected_stderr: &["error[E0599]", "no method named `accept_apply_token` found"],
    });
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_fake_renderer_finalize_names_missing_method() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_has_no_finalize",
        code: "use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode}; use bevy_ui_router_core::prelude::FrameApplyRequest; fn main() { let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success); let request: FrameApplyRequest = unreachable!(); renderer.finalize(request); }",
        features: &["test-support"],
        expected_stderr: &["error[E0599]", "no method named `finalize` found"],
    });
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_commit_authority_trait_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "test_support_no_named_commit_authority",
        code: "use bevy_ui_router_view::prelude::ViewCommitAuthority; fn main() {}",
        features: &["test-support"],
        expected_stderr: &["error[E0432]", "no `ViewCommitAuthority` in `prelude`"],
    });
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_finalize_authority_trait_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "test_support_no_named_finalize_authority",
        code: "use bevy_ui_router_view::test_support::ViewFinalizeAuthority; fn main() {}",
        features: &["test-support"],
        expected_stderr: &[
            "error[E0432]",
            "no `ViewFinalizeAuthority` in `test_support`",
        ],
    });
}
