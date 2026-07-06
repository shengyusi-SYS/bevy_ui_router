# Phase 0 Contract Freeze Implementation Plan Review Log

## Target

- Plan: `docs/superpowers/plans/2026-07-07-phase-0-contract-freeze-implementation-plan.md`
- Scope: Phase 0 workspace migration and core contract freeze planning.
- Status: Passed multi-round review on 2026-07-07.

This log records review outcomes only. It does not authorize code implementation. Code implementation still requires the user to confirm the execution mode described in the plan.

---

## Lead Evaluation Summary

The first review round exposed a shared root cause: the draft declared key Phase 0 boundaries but did not prove enough of them through executable evidence. The issue did not require reversing the workspace split, facade feature baseline, or public type ownership decisions.

The plan was revised to add:

- A pre-implementation acceptance checklist.
- Stronger evidence ledger fields for command, exit code, output summary, blocked/unverified state, and user-accepted exceptions.
- `compile_fail` doctests for public constructor boundaries.
- A core-neutral ownership cleanup scope contract.
- A regression/gate task for dependency and facade boundaries.
- `cargo tree` dependency graph checks for no-default, default, and `bsn` stub feature paths.

Later review rounds found only local documentation or command-hardening issues. Those were fixed without changing higher-level architecture.

---

## Round 1

Review directions:

- Process and gate alignment.
- Phase 0 scope and task granularity.
- Architecture/API boundary and dependency ownership.
- TDD, verification, and evidence adequacy.
- Fact consistency with the current repository and confirmed documents.

Results:

- Process/gate: Nonblocking. Suggested centralizing pre-implementation checklist and required-reading evidence.
- Phase 0 scope/granularity: Blocking. Ownership boundary evidence was too abstract, and a post-hoc boundary task was incorrectly framed as TDD RED/GREEN.
- Architecture/API: Blocking. Adapter/id/generation anti-forgery proof was not executable enough; dependency graph proof was too weak.
- TDD/verification: Blocking. Boundary regression tests could not produce a stable RED/GREEN story; gate evidence lacked enough command detail.
- Fact check: Blocking. Ownership/root layout freeze was missing from the Phase 0 plan.

Lead decision:

- Do not revise higher-level planning documents.
- Revise the Phase 0 plan to convert boundary declarations into executable evidence.

Actions taken:

- Added `ownership.rs` as a Phase 0 file.
- Added a pre-implementation checklist.
- Expanded the evidence ledger.
- Added `compile_fail` doctests for identity and diagnostic constructor boundaries.
- Added an ownership cleanup scope task.
- Replaced the old boundary TDD task with a regression/gate task.
- Added dependency graph checks with `cargo tree`.

---

## Round 2

Results:

- Process/gate: Pass.
- Fact check: Pass.
- Architecture/API: Nonblocking. Suggested broadening dependency graph checks to Bevy split packages.
- TDD/verification: Nonblocking. Suggested checking `cargo tree` command exit codes explicitly.
- Phase 0 scope/granularity: Blocking. `CleanupScope::phase_3b_verification_note()` leaked implementation-plan text into public core API, and the wording risked over-constraining later ECS host relationship updates.

Lead decision:

- Do not revise higher-level planning documents.
- Fix the local API leak and harden dependency graph commands.

Actions taken:

- Moved Phase 3B carry-forward text into test evidence rather than public API.
- Removed broad "mutate app-owned host" wording.
- Added `$LASTEXITCODE` checks after `cargo tree`.
- Broadened dependency graph checks from only `bevy` to the Bevy package family.

---

## Round 3

Results:

- Process/gate: Pass.
- TDD/verification: Pass.
- Phase 0 scope/granularity: Nonblocking. Confirmed public API leak was fixed; suggested matching multi-segment Bevy package names.
- Architecture/API: Nonblocking. Suggested making doctests more precise by checking `RouteFrameId::new`, actual `DiagnosticEvent::pending_overwrite` calls, and active diagnostic snapshot construction.
- Fact check: Blocking. The Bevy package-family regex still missed multi-segment package names such as `bevy_core_pipeline` and `bevy_dev_tools`.

Lead decision:

- Do not revise higher-level planning documents.
- Fix the command-level false negative and harden doctest examples.

Actions taken:

- Changed the forbidden package regex to `(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v`.
- Documented that the negative lookahead excludes workspace crates named `bevy_ui_router_*`.
- Added more precise `compile_fail` examples for `RouteFrameId::new`, `DiagnosticIdentity::new`, `DiagnosticEvent::pending_overwrite`, and `DiagnosticSnapshot::with_active`.

---

## Round 4

Results:

- Process/gate: Pass.
- Phase 0 scope/granularity: Pass.
- Architecture/API: Pass.
- TDD/verification: Pass.
- Fact check: Pass.

Shared conclusion:

- The original root cause has been resolved.
- No higher-level planning decision needs to be reversed.
- No additional task split is required before execution-mode confirmation.
- The current document may proceed to versioned commit as planning evidence.

---

## Rewrite Necessity Evaluation

After the plan passed review, the same review directions assessed whether the document needed a cleanup rewrite before commit.

Results:

- Process/gate: No Rewrite.
- Phase 0 scope/granularity: No Rewrite.
- Architecture/API: No Rewrite.
- TDD/verification: No Rewrite.
- Fact/versioning: No Rewrite.

Lead decision:

- Do not rewrite the plan.
- The document is long because it is an executable implementation plan with repeated code snippets and verification commands.
- Rewriting after review would add more risk than value.

---

## Final Planning State

The Phase 0 plan is accepted as a planning document. The next step is to ask the user to confirm the execution mode:

- Subagent-Driven execution.
- Inline Execution.

No code implementation should begin until the user confirms one execution mode.
