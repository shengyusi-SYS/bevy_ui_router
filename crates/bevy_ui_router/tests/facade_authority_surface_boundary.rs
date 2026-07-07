#[test]
fn facade_does_not_expose_active_commit_or_acceptance_surface() {
    let source = source_without_doc_text(include_str!("../src/lib.rs"));
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
        "Accept",
        "Approve",
        "Activate",
        "SetActive",
        "MakeActive",
        "MarkActive",
        "Commit",
        "Finalize",
        "FrameApply",
        "CoreApply",
        "pub trait ViewCommitAuthority",
        "pub trait ViewFinalizeAuthority",
        "pub struct ViewCommitAuthority",
        "pub struct ViewFinalizeAuthority",
    ];

    for fragment in forbidden {
        assert!(
            !source.contains(fragment),
            "facade exposes forbidden Phase 3A authority surface fragment `{fragment}`"
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
