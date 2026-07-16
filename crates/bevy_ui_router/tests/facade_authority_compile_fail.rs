#![cfg(feature = "ecs")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

fn scratch_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be valid")
        .as_nanos();
    std::env::temp_dir().join(format!("bevy-ui-router-cutover-{name}-{stamp}"))
}

fn shared_target_dir() -> &'static Path {
    static TARGET_DIR: OnceLock<PathBuf> = OnceLock::new();
    TARGET_DIR
        .get_or_init(|| scratch_dir(&format!("shared-target-{}", std::process::id())))
        .as_path()
}

fn assert_rejected(name: &str, code: &str, expected: &[&str]) {
    let root = scratch_dir(name);
    fs::create_dir_all(root.join("src")).unwrap();
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .display()
        .to_string()
        .replace('\\', "/");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nbevy_ui_router = {{ path = \"{package}\", default-features = false, features = [\"ecs\"] }}\n"
        ),
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), code).unwrap();
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .arg("check")
        .arg("--offline")
        .arg("--config")
        .arg("build.rustc-wrapper = ''")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", shared_target_dir())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "fixture `{name}` unexpectedly compiled"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for fragment in expected {
        assert!(
            stderr.contains(fragment),
            "fixture `{name}` missing `{fragment}`:\n{stderr}"
        );
    }
}

#[test]
fn cutover_rejects_legacy_core_runtime_publisher() {
    assert_rejected(
        "legacy-core-runtime",
        "use bevy_ui_router::core::runtime::RouterRuntime; fn main() {}",
        &["could not find `runtime` in `core`"],
    );
}

#[test]
fn cutover_rejects_legacy_ecs_state_publisher() {
    assert_rejected(
        "legacy-ecs-state",
        "use bevy_ui_router::ecs::RouterEcsState; fn main() {}",
        &["no `RouterEcsState` in `ecs`"],
    );
}

#[test]
fn cutover_rejects_legacy_view_report_bridge() {
    assert_rejected(
        "legacy-view-report",
        "use bevy_ui_router::view::ViewReportSession; fn main() {}",
        &["no `ViewReportSession` in `view`"],
    );
}

#[test]
fn cutover_rejects_nested_route_declaration_in_root_profile() {
    assert_rejected(
        "deferred-nested-route",
        "use bevy_ui_router::prelude::*; fn main() { let mut builder = RouteTree::builder(); let parent: RouteId = unimplemented!(); let _ = builder.route(parent, RoutePattern::path(\"child\").unwrap()); }",
        &["method `route` is private"],
    );
}

#[test]
fn cutover_rejects_core_engine_and_internal_frame_authority() {
    assert_rejected(
        "core-engine-authority",
        "use bevy_ui_router::core::integration::RouterEngine; fn main() {}",
        &["could not find `integration` in `core`"],
    );
    assert_rejected(
        "core-frame-authority",
        "use bevy_ui_router::core::FrameReconciliationPlan; fn main() {}",
        &["no `FrameReconciliationPlan` in `core`"],
    );
    assert_rejected(
        "core-diagnostic-identity",
        "use bevy_ui_router::prelude::DiagnosticIdentity; fn main() {}",
        &["no `DiagnosticIdentity` in `prelude`"],
    );
}

#[test]
fn cutover_rejects_ecs_registry_authority_and_keeps_schedule_labels_out_of_prelude() {
    assert_rejected(
        "ecs-registry-authority",
        "use bevy_ui_router::ecs::RouteEntityRegistry; fn main() {}",
        &["no `RouteEntityRegistry` in `ecs`"],
    );
    assert_rejected(
        "ecs-schedule-prelude",
        "use bevy_ui_router::prelude::RouterScheduleSet; fn main() {}",
        &["no `RouterScheduleSet` in `prelude`"],
    );
}

#[test]
fn curated_prelude_rejects_core_only_diagnostic_vocabulary() {
    assert_rejected(
        "diagnostic-event-prelude",
        "use bevy_ui_router::prelude::DiagnosticEvent; fn main() {}",
        &["no `DiagnosticEvent` in `prelude`"],
    );
    assert_rejected(
        "diagnostic-kind-prelude",
        "use bevy_ui_router::prelude::DiagnosticEventKind; fn main() {}",
        &["no `DiagnosticEventKind` in `prelude`"],
    );
}
