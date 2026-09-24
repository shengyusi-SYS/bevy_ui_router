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

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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
    let _scratch = Scratch(root.clone());
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
        .arg("--offline")
        .arg("--config")
        .arg("build.rustc-wrapper = ''")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("../../target/core-compile-fail"),
        )
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
        // Stable and nightly phrase this differently. E0423 on this type is
        // the contract: a named-field error cannot be used as a unit value.
        expected_stderr: &["error[E0423]", "`RouteViewKeyError`"],
    });
}

#[test]
fn consumer_cannot_open_sealed_view_report_identity() {
    assert_compile_fails(Fixture {
        name: "view_report_identity_inner_private",
        code: "use bevy_ui_router_core::integration::ViewReportIdentity; fn inspect(identity: ViewReportIdentity) { let _ = identity.inner; } fn main() {}",
        features: &[],
        expected_stderr: &["error[E0616]", "field `inner`", "private"],
    });
    for method in [
        "report_router_provenance",
        "report_host_epoch",
        "report_transaction_sequence",
        "report_frame_task_sequence",
    ] {
        let code = format!(
            "use bevy_ui_router_core::integration::ViewReportIdentity; fn inspect(identity: &ViewReportIdentity) {{ let _ = identity.{method}(); }} fn main() {{}}"
        );
        assert_compile_fails(Fixture {
            name: method,
            code: &code,
            features: &[],
            expected_stderr: &["error[E0599]", method],
        });
    }
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
fn admission_module_and_protocol_names_are_unreachable() {
    assert_compile_fails(Fixture {
        name: "admission_module_private",
        code: "use bevy_ui_router_core::admission::HostAdmissionToken; fn main() {}",
        features: &[],
        expected_stderr: &["error[E0603]", "module `admission` is private"],
    });
    for (name, code) in [
        (
            "host_admission_token_unreachable",
            "use bevy_ui_router_core::HostAdmissionToken; fn main() {}",
        ),
        (
            "host_grant_unreachable",
            "use bevy_ui_router_core::HostGrant; fn main() {}",
        ),
        (
            "host_confirmation_unreachable",
            "use bevy_ui_router_core::HostAdmissionConfirmation; fn main() {}",
        ),
    ] {
        assert_compile_fails(Fixture {
            name,
            code,
            features: &[],
            expected_stderr: &["error[E0432]", "unresolved import"],
        });
    }
}

#[test]
fn host_version_identity_accessors_are_not_public() {
    for (name, accessor) in [
        ("host_version_router_private", "router"),
        ("host_version_host_private", "host"),
        ("host_version_epoch_private", "epoch"),
    ] {
        let code = format!(
            "use bevy_ui_router_core::HostVersion; fn inspect(version: &HostVersion) {{ let _ = version.{accessor}(); }} fn main() {{}}"
        );
        assert_compile_fails(Fixture {
            name,
            code: Box::leak(code.into_boxed_str()),
            features: &[],
            expected_stderr: &["error[E0624]", "private"],
        });
    }
}

#[test]
fn exact_router_identity_is_core_private() {
    assert_compile_fails(Fixture {
        name: "router_identity_private",
        code: "use bevy_ui_router_core::ids::RouterIdentity; fn main() {}",
        features: &[],
        expected_stderr: &["error[E0603]", "private"],
    });
    assert_compile_fails(Fixture {
        name: "host_version_router_identity_private",
        code: "use bevy_ui_router_core::HostVersion; fn inspect(version: &HostVersion) { let _ = version.router_identity(); } fn main() {}",
        features: &[],
        expected_stderr: &["error[E0624]", "private"],
    });
}

#[test]
fn consumer_cannot_mint_logical_host_slot_frame_or_generation_ids() {
    for fixture in [
        Fixture {
            name: "router_instance_id_constructor",
            code: "use bevy_ui_router_core::ids::RouterInstanceId; fn main() { let _ = RouterInstanceId::new(1); }",
            features: &[],
            expected_stderr: &["error[E0624]", "associated function `new` is private"],
        },
        Fixture {
            name: "route_host_id_constructor",
            code: "use bevy_ui_router_core::ids::RouteHostId; fn main() { let _ = RouteHostId::new(1); }",
            features: &[],
            expected_stderr: &["error[E0624]", "associated function `new` is private"],
        },
        Fixture {
            name: "route_slot_id_constructor",
            code: "use bevy_ui_router_core::ids::RouteSlotId; fn main() { let _ = RouteSlotId::new(1); }",
            features: &[],
            expected_stderr: &["error[E0624]", "associated function `new` is private"],
        },
        Fixture {
            name: "route_frame_id_constructor",
            code: "use bevy_ui_router_core::ids::RouteFrameId; fn main() { let _ = RouteFrameId::new(1); }",
            features: &[],
            expected_stderr: &["error[E0624]", "associated function `new` is private"],
        },
        Fixture {
            name: "route_generation_constructor",
            code: "use bevy_ui_router_core::ids::RouteGeneration; fn main() { let _ = RouteGeneration::initial(); }",
            features: &[],
            expected_stderr: &["error[E0624]", "associated function `initial` is private"],
        },
    ] {
        assert_compile_fails(fixture);
    }
}

#[test]
fn route_view_input_has_no_unlisted_public_traits() {
    for (name, bound) in [
        ("route_view_input_not_clone", "Clone"),
        ("route_view_input_not_debug", "std::fmt::Debug"),
        ("route_view_input_not_partial_eq", "PartialEq"),
        ("route_view_input_not_eq", "Eq"),
    ] {
        let code = format!(
            "use bevy_ui_router_core::prelude::RouteViewInput; fn require<T: {bound}>() {{}} fn main() {{ require::<RouteViewInput>(); }}"
        );
        assert_compile_fails(Fixture {
            name,
            code: Box::leak(code.into_boxed_str()),
            features: &[],
            expected_stderr: &["error[E0277]"],
        });
    }
}

#[test]
fn consumer_cannot_name_core_private_task_or_transaction_identities() {
    for fixture in [
        Fixture {
            name: "frame_task_id_private",
            code: "use bevy_ui_router_core::ids::FrameTaskId; fn main() {}",
            features: &[],
            expected_stderr: &["error[E0603]", "struct `FrameTaskId` is private"],
        },
        Fixture {
            name: "transaction_id_private",
            code: "use bevy_ui_router_core::ids::TransactionId; fn main() {}",
            features: &[],
            expected_stderr: &["error[E0603]", "struct `TransactionId` is private"],
        },
        Fixture {
            name: "navigation_request_id_constructor",
            code: "use bevy_ui_router_core::ids::NavigationRequestId; fn main() { let _ = NavigationRequestId::new(); }",
            features: &[],
            expected_stderr: &[
                "error[E0599]",
                "no associated function or constant named `new`",
            ],
        },
        Fixture {
            name: "navigation_request_id_raw_accessor",
            code: "use bevy_ui_router_core::ids::NavigationRequestId; fn inspect(id: NavigationRequestId) { let _ = id.raw(); } fn main() {}",
            features: &[],
            expected_stderr: &["error[E0599]", "no method named `raw`"],
        },
    ] {
        assert_compile_fails(fixture);
    }
}

#[test]
fn consumer_cannot_bypass_mount_registration_session() {
    for fixture in [
        Fixture {
            name: "mount_registration_permit_slot_private",
            code: "use bevy_ui_router_core::integration::MountRegistrationPermit; fn inspect(permit: &MountRegistrationPermit) { let _ = permit.slot(); } fn main() {}",
            features: &[],
            expected_stderr: &["error[E0599]", "no method named `slot`"],
        },
        Fixture {
            name: "mount_registration_permit_complete_private",
            code: "use bevy_ui_router_core::integration::MountRegistrationPermit; fn complete(permit: MountRegistrationPermit) { let _ = permit.complete(1); } fn main() {}",
            features: &[],
            expected_stderr: &["error[E0599]", "no method named `complete`"],
        },
        Fixture {
            name: "mount_registration_engine_complete_private",
            code: "use bevy_ui_router_core::integration::RouterEngine; fn complete(engine: &mut RouterEngine) { let _ = engine.complete_mount_registration(todo!()); } fn main() {}",
            features: &[],
            expected_stderr: &[
                "error[E0599]",
                "no method named `complete_mount_registration`",
            ],
        },
        Fixture {
            name: "test_support_mount_permit_cannot_complete_forward",
            code: "use bevy_ui_router_core::integration::MountRegistrationPermit; fn complete(permit: MountRegistrationPermit) { let _ = permit.testing_complete(1); } fn main() {}",
            features: &["test-support"],
            expected_stderr: &["error[E0599]", "no method named `testing_complete`"],
        },
        Fixture {
            name: "test_support_engine_cannot_complete_forward",
            code: "use bevy_ui_router_core::integration::RouterEngine; fn complete(engine: &mut RouterEngine) { let _ = engine.testing_complete_mount_registration(todo!()); } fn main() {}",
            features: &["test-support"],
            expected_stderr: &[
                "error[E0599]",
                "no method named `testing_complete_mount_registration`",
            ],
        },
        Fixture {
            name: "test_support_engine_has_no_retaining_confirmation_forwarder",
            code: "use bevy_ui_router_core::integration::RouterEngine; fn complete(engine: &mut RouterEngine) { let _ = engine.testing_complete_mount_registration_retaining_confirmation(todo!()); } fn main() {}",
            features: &["test-support"],
            expected_stderr: &[
                "error[E0599]",
                "no method named `testing_complete_mount_registration_retaining_confirmation`",
            ],
        },
    ] {
        assert_compile_fails(fixture);
    }
}

#[test]
fn test_support_mount_registration_replays_are_opaque_and_non_forgeable() {
    for fixture in [
        Fixture {
            name: "mount_registration_replays_not_clone",
            code: "use bevy_ui_router_core::integration::MountRegistrationReplays; fn require_clone<T: Clone>() {} fn main() { require_clone::<MountRegistrationReplays>(); }",
            features: &["test-support"],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "mount_registration_replays_not_default",
            code: "use bevy_ui_router_core::integration::MountRegistrationReplays; fn require_default<T: Default>() {} fn main() { require_default::<MountRegistrationReplays>(); }",
            features: &["test-support"],
            expected_stderr: &["error[E0277]", "trait `Default` is not implemented"],
        },
        Fixture {
            name: "mount_registration_replays_fields_private",
            code: "use bevy_ui_router_core::integration::MountRegistrationReplays; fn main() { let _ = MountRegistrationReplays { completion: todo!(), confirmation: todo!() }; }",
            features: &["test-support"],
            expected_stderr: &["error[E0451]", "private"],
        },
    ] {
        assert_compile_fails(fixture);
    }
}

#[test]
fn mount_receipt_proof_and_registration_session_are_not_clone() {
    for fixture in [
        Fixture {
            name: "mount_receipt_proof_not_clone",
            code: "use bevy_ui_router_core::integration::MountReceiptProof; fn require_clone<T: Clone>() {} fn main() { require_clone::<MountReceiptProof>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "mount_registration_session_not_clone",
            code: "use bevy_ui_router_core::integration::MountRegistrationSession; fn require_clone<T: Clone>() {} fn check<'a>() { require_clone::<MountRegistrationSession<'a>>(); } fn main() {}",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "mount_registration_begin_error_not_clone",
            code: "use bevy_ui_router_core::integration::MountRegistrationBeginError; fn require_clone<T: Clone>() {} fn main() { require_clone::<MountRegistrationBeginError>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "resolve_mount_owner_not_clone",
            code: "use bevy_ui_router_core::integration::ResolveMountOwner; fn require_clone<T: Clone>() {} fn main() { require_clone::<ResolveMountOwner>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "resolve_mount_error_not_clone",
            code: "use bevy_ui_router_core::integration::ResolveMountError; fn require_clone<T: Clone>() {} fn main() { require_clone::<ResolveMountError>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "reject_mount_error_not_clone",
            code: "use bevy_ui_router_core::integration::RejectMountError; fn require_clone<T: Clone>() {} fn main() { require_clone::<RejectMountError>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "accept_mount_receipt_error_not_clone",
            code: "use bevy_ui_router_core::integration::AcceptMountReceiptError; fn require_clone<T: Clone>() {} fn main() { require_clone::<AcceptMountReceiptError>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "reject_mount_mutation_error_not_clone",
            code: "use bevy_ui_router_core::integration::RejectMountMutationError; fn require_clone<T: Clone>() {} fn main() { require_clone::<RejectMountMutationError>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
        Fixture {
            name: "prepare_failure_cleanup_error_not_clone",
            code: "use bevy_ui_router_core::integration::PrepareFailureCleanupError; fn require_clone<T: Clone>() {} fn main() { require_clone::<PrepareFailureCleanupError>(); }",
            features: &[],
            expected_stderr: &["error[E0277]", "trait `Clone` is not implemented"],
        },
    ] {
        assert_compile_fails(fixture);
    }
}

#[test]
fn consumer_cannot_construct_mount_capability_errors() {
    for fixture in [
        Fixture {
            name: "resolve_mount_error_fields_private",
            code: "use bevy_ui_router_core::integration::{IntegrationError, ResolveMountError}; fn main() { let _ = ResolveMountError { kind: IntegrationError::Invalid, owner: panic!() }; }",
            features: &[],
            expected_stderr: &["error[E0451]", "private"],
        },
        Fixture {
            name: "reject_mount_error_fields_private",
            code: "use bevy_ui_router_core::integration::{IntegrationError, RejectMountError}; fn main() { let _ = RejectMountError { kind: IntegrationError::Invalid, owner: panic!() }; }",
            features: &[],
            expected_stderr: &["error[E0451]", "private"],
        },
        Fixture {
            name: "accept_mount_receipt_error_fields_private",
            code: "use bevy_ui_router_core::integration::{AcceptMountReceiptError, IntegrationError}; fn main() { let _ = AcceptMountReceiptError { kind: IntegrationError::Invalid, owner: panic!() }; }",
            features: &[],
            expected_stderr: &["error[E0451]", "private"],
        },
        Fixture {
            name: "reject_mount_mutation_error_fields_private",
            code: "use bevy_ui_router_core::integration::{IntegrationError, RejectMountMutationError}; fn main() { let _ = RejectMountMutationError { kind: IntegrationError::Invalid, owner: panic!() }; }",
            features: &[],
            expected_stderr: &["error[E0451]", "private"],
        },
        Fixture {
            name: "prepare_failure_cleanup_error_fields_private",
            code: "use bevy_ui_router_core::integration::{IntegrationError, PrepareFailureCleanupError}; fn main() { let _ = PrepareFailureCleanupError { kind: IntegrationError::Invalid, owner: panic!() }; }",
            features: &[],
            expected_stderr: &["error[E0451]", "private"],
        },
    ] {
        assert_compile_fails(fixture);
    }
}
