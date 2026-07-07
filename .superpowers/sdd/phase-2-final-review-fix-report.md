# Phase 2 Final Review Fix Report

## Status

- Status: DONE_WITH_CONCERNS
- Commit: `964321c`

## Files Changed

- `crates/bevy_ui_router_core/src/frame.rs`
- `crates/bevy_ui_router_core/src/transaction.rs`
- `.superpowers/sdd/phase-2-final-review-fix-report.md`

## RED Command / Output Summary

- Command:
  `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests::commit_rejects_remounted_current_active_branch_with_same_fingerprint -- --nocapture`
- Result: expected failure, nonzero intended harness count (`running 1 test`)
- Failure summary:
  `commit_rejects_remounted_current_active_branch_with_same_fingerprint` failed because `transaction.commit(Some(&remounted_active))` returned `Ok(RouteFrameBranch { .. })`, so `unwrap_err()` panicked. This reproduced the blocker: commit accepted a remounted active branch that kept the same `RouteBranchFingerprint` but changed frame identities/generations.

## GREEN Command / Output Summary

- Command:
  `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests -- --nocapture`
  - Result: pass, intended harness count nonzero (`running 11 tests`)
  - Summary: `11 passed; 0 failed; 0 ignored; 0 measured; 76 filtered out`
- Command:
  `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core frame::tests -- --nocapture`
  - Result: pass, intended harness count nonzero (`running 13 tests`)
  - Summary: `13 passed; 0 failed; 0 ignored; 0 measured; 74 filtered out`
- Command:
  `cargo fmt --check`
  - First run: failed with formatting diff in `crates/bevy_ui_router_core/src/frame.rs`
  - Follow-up: ran `cargo fmt --package bevy_ui_router_core`
  - Final rerun result: pass

## Self-Review

- Added a regression test that specifically reproduces the stale-transaction hole: old transaction, remounted current active branch, same branch fingerprint, different frame identities/generations, all apply requests accepted, commit rejected.
- Replaced the commit-time baseline recheck from fingerprint-only comparison with a private active-branch baseline snapshot comparison.
- The new baseline comparison checks branch fingerprint plus per-frame router, host, slot, route, frame id, generation, and branch index before commit publishes the new active branch.
- First navigation semantics remain fail-closed: baseline `None` only matches `current_active == None`.
- No new public authority or constructor APIs were added.

## RouteError Helper Rationale

- I did not add `RouteError` helper constructors. The current code path still uses `RouteError::new`, and this fix did not need extra helpers to express or test the stale baseline behavior. Adding unused helpers here would broaden the change without improving the blocker fix.

## Concerns

- Residual risk remains around public API/facade observation coverage, which is still mostly smoke-test level as noted in the brief.
- There is an untracked `Cargo.lock` in the worktree outside this fix scope; I left it untouched.
