struct AllowedPublicSurface<'a> {
    file: &'a str,
    fragment: &'a str,
}

const ALLOWED_REPORTING_SURFACE: &[AllowedPublicSurface<'_>] = &[
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub struct ViewReportSession",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub struct ViewReportedOutput",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn from_frame_request(",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn frame_request(&self) -> &FrameApplyRequest",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn build_request(&self) -> ViewBuildRequest",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn build_success(&mut self) -> ViewBuildOutcome",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn build_failed(&mut self, redacted_message: impl Into<String>) -> ViewBuildOutcome",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn applied(",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn apply_failed(",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn stale(&mut self, redacted_message: impl Into<String>) -> ViewApplyOutcome",
    },
    AllowedPublicSurface {
        file: "src/contract.rs",
        fragment: "pub fn outcome(&self) -> &ViewApplyOutcome",
    },
    AllowedPublicSurface {
        file: "src/lib.rs",
        fragment: "ViewReportSession",
    },
    AllowedPublicSurface {
        file: "src/lib.rs",
        fragment: "ViewReportedOutput",
    },
];

#[test]
fn view_crate_allow_lists_reporting_only_surface() {
    for allowed in ALLOWED_REPORTING_SURFACE {
        let source = source_for(allowed.file);
        assert!(
            source.contains(allowed.fragment),
            "{} is missing allowed reporting surface fragment `{}`",
            allowed.file,
            allowed.fragment
        );
    }
}

#[test]
fn view_crate_does_not_expose_active_commit_or_acceptance_surface() {
    assert_forbidden_public_fragments_absent("src/lib.rs", include_str!("../src/lib.rs"));
    assert_forbidden_public_fragments_absent("src/contract.rs", include_str!("../src/contract.rs"));
    assert_forbidden_public_fragments_absent("src/testing.rs", include_str!("../src/testing.rs"));
}

fn source_for(file: &str) -> &'static str {
    match file {
        "src/lib.rs" => include_str!("../src/lib.rs"),
        "src/contract.rs" => include_str!("../src/contract.rs"),
        "src/testing.rs" => include_str!("../src/testing.rs"),
        other => panic!("unexpected source file `{other}`"),
    }
}

fn assert_forbidden_public_fragments_absent(file: &str, source: &str) {
    let source = source_without_doc_text(source);
    let forbidden = [
        "pub fn commit",
        "pub fn finalize",
        "pub fn accept(",
        "pub fn accept_",
        "pub fn approve",
        "pub fn approve_",
        "pub fn accept_apply_token",
        "pub fn accept_frame_apply",
        "pub fn activate",
        "pub fn activate_",
        "pub fn set_active",
        "pub fn make_active",
        "pub fn mark_active",
        "pub fn into_frame_apply",
        "pub fn to_frame_apply",
        "pub fn into_core_apply",
        "pub fn to_core_apply",
        "pub fn cleanup",
        "pub fn into_cleanup",
        "pub fn to_cleanup",
        "pub fn advance_generation",
        "pub fn next_generation",
        "pub fn emit_diagnostic",
        "pub fn diagnostic",
        "Accept",
        "Approve",
        "Activate",
        "SetActive",
        "MakeActive",
        "MarkActive",
        "Commit",
        "Finalize",
        "ViewCommitAuthority",
        "ViewFinalizeAuthority",
    ];

    for fragment in forbidden {
        assert!(
            !source.contains(fragment),
            "{file} exposes forbidden authority surface fragment `{fragment}`"
        );
    }
}

fn source_without_doc_text(source: &str) -> String {
    let mut cleaned = String::new();
    let mut in_doc_attr = false;

    for line in source.lines() {
        let trimmed = line.trim_start();

        if in_doc_attr {
            if trimmed.starts_with("\"#") {
                in_doc_attr = false;
            }
            continue;
        }

        if trimmed.starts_with("//!") || trimmed.starts_with("///") {
            continue;
        }

        if trimmed.contains("doc = r#\"") {
            in_doc_attr = true;
            continue;
        }

        cleaned.push_str(line);
        cleaned.push('\n');
    }

    cleaned
}
