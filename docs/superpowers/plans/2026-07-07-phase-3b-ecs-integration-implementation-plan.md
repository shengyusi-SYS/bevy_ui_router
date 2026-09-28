# Phase 3B ECS Integration Implementation Plan

> Historical plan only. The Bevy fork references below describe the former development baseline and must not be used for current builds or releases. Current dependencies use official crates.io Bevy; see the [current MVP spec](../specs/2026-09-07-router-mvp-design.md).

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to execute this plan task-by-task. Use `superpowers:test-driven-development` for every Rust behavior change and `superpowers:verification-before-completion` before any completion claim.

**Goal:** Build the ECS integration slice that turns Phase 2 core frame transactions and Phase 3A renderer-neutral view results into router-owned Bevy ECS hierarchy changes, without moving lifecycle authority out of core.

**Architecture:** Phase 3B introduces the controlled bridge between core and ECS. Core issues router, host, slot, frame, generation, transaction, and `FrameApplyRequest` identities. ECS owns Bevy components, schedule labels, hierarchy mutation, cleanup execution, and finalization state. View and renderer adapters may report renderer observations only through a session/reporter minted from a core-issued request. Core remains the only authority that accepts stale-sensitive apply results and commits the active branch.

**Tech Stack:** Rust 2024, existing Cargo workspace, `bevy_ui_router_core`, `bevy_ui_router_view`, `bevy_ui_router_ecs`, facade feature wiring in `bevy_ui_router`, Bevy-dependent crates pinned to the project-approved Bevy source, `cargo fmt`, `cargo test`, `cargo check`, dependency-boundary tests, compile-fail authority tests, multi-agent review.

---

## Source Inputs And Baseline

- `AGENTS.md` hard constraints for router-owned subtree, core boundaries, generation tracking, stale checks, failure cleanup, hidden/retained input isolation, query ordering, observer authority, adapter authority, and verification claims.
- `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md` Task 7B and Task 8B.
- `docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md`, especially the Phase 3B handoff statement and Phase 3A verification record.
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`, especially sections 5.3, 8.2, 8.3, 8.4, 8.5, 10.1, and Phase 3 minimum verification.
- `docs/superpowers/specs/2026-07-07-project-decisions-design.md`, especially Phase 3 execution split and ECS crate responsibility.
- Current source:
  - `crates/bevy_ui_router_core/src/ids.rs`
  - `crates/bevy_ui_router_core/src/frame.rs`
  - `crates/bevy_ui_router_core/src/transaction.rs`
  - `crates/bevy_ui_router_core/src/ownership.rs`
  - `crates/bevy_ui_router_core/src/lib.rs`
  - `crates/bevy_ui_router_view/src/contract.rs`
  - `crates/bevy_ui_router_view/src/testing.rs`
  - `crates/bevy_ui_router_view/src/lib.rs`
  - `crates/bevy_ui_router_ecs/Cargo.toml`
  - `crates/bevy_ui_router_ecs/src/lib.rs`
  - `crates/bevy_ui_router/Cargo.toml`
  - `crates/bevy_ui_router/src/lib.rs`

Current factual baseline:

- Latest Phase 3A commit is `3a29754 docs: record phase 3a view contract verification`.
- Worktree was clean at Phase 3B planning start before the accepted planning file was created.
- `bevy_ui_router_ecs` is still an empty shell with only core and view dependencies.
- `bevy_ui_router` already has an `ecs` feature and re-exports `bevy_ui_router_ecs` behind that feature.
- Core exposes `FrameApplyRequest`, `FrameApplyToken`, `FrameApplyOutcome`, `NavigationTransactionSnapshot`, `RouteFrameRecord`, `RouteFrameIdentity`, and `FrameCleanupPlan` as observation contracts.
- Core keeps `NavigationTransaction`, `NavigationTransactionId`, `FrameAllocator`, `RouteFrameScope::new`, `FrameReconciliationPlan::from_diff`, `FrameReconciliationPlan::from_first_navigation`, `NavigationTransaction::accept_apply_result`, and `NavigationTransaction::commit` crate-private.
- Phase 3B therefore must add a controlled core-owned runtime or bridge API before ECS can legally issue requests, register host/slot identity, report apply outcomes, or update active branch state.

---

## Canonical Invariants

These invariants are canonical for all tasks and reviews in Phase 3B. Task-local constraints refine them but do not replace them.

### Authority

- Do not expose public constructors for `RouterInstanceId`, `RouteHostId`, `RouteSlotId`, `RouteFrameId`, `RouteGeneration`, `RouteFrameIdentity`, `CleanupScope`, `FrameCleanupPlan`, `NavigationTransaction`, `NavigationTransactionId`, `FrameApplyToken`, or raw view protocol ids.
- Do not move `NavigationTransaction::accept_apply_result` or `NavigationTransaction::commit` into ECS, view, BSN, facade, platform, or adapter code.
- `RouterRuntime` or its accepted equivalent owns router, host, slot, frame allocation, pending transaction, active branch, apply result acceptance, active commit, and failed-navigation cleanup acknowledgement.
- ECS may call a core-owned public runtime method that performs stale checks inside core. ECS must not reconstruct stale checks from public `FrameApplyToken` fields.
- Scene-local observers may handle local UI behavior but cannot become route lifecycle authority.
- Adapter-facing code may affect Router only through public APIs. It must not forge generation, frame identity, cleanup scope, transaction token, or active-commit state.

### Lifecycle

- Pending scene output is not active UI.
- Renderer success does not create active UI. It can only create an unfinalized observation or prepared-inert ECS output before core acceptance.
- `ViewBuildOutcome::Built(plan)` is a renderer-local plan observation only.
- `ViewBuildOutcome::Failed(failure)` is recorded as a build failure candidate and is not active UI.
- `ViewBuildOutcome::Stale(failure)` is discarded and is not reported to core.
- `ViewApplyOutcome::Applied(partial)` is recorded as unfinalized partial output and is not active UI.
- `ViewApplyOutcome::Failed(failure)` is recorded as unfinalized failure and is not active UI.
- `ViewApplyOutcome::Stale(failure)` is discarded by the ECS bridge and is not reported to core as success or failure.
- Active UI requires, in order: core-issued `FrameApplyRequest`; request-bound renderer success; router-owned root under the expected slot; prepared-inert new root; inert replaced/leaving roots; `RouterRuntime::report_frame_apply`; `RouterRuntime::commit_ready_navigation`; ECS-minted `PostCommitActivation`; active marker and input/focus eligibility.

### Stale And Async Safety

- Do not hold `&mut World` across `await`.
- Exhaustive stale coverage belongs in core runtime tests where private test helpers can mutate tokens without weakening public API.
- Core stale token rejection must have no visible side effects across transaction/task identity, sequence, router, host, slot, route, frame, and generation mismatches.
- ECS tests must not forge tokens. ECS tests cover the public-path stale/error branch with a crate-local reporter stub or controlled test seam that returns `FrameApplyReportError::Stale` without exposing token mutation to production code or adapters.
- A stale apply result returns `FrameApplyReportError::Stale` and leaves `pending`, `active`, and failure cleanup unchanged.
- Renderer-local stale results must be discarded without core report, active UI, retained partial output, or app-owned host cleanup.

### Cleanup

- Router cleanup must target only router-owned subtrees identified by router, host, slot, frame, and generation.
- Cleanup must never despawn, replace, or clear the app-owned host entity.
- Cleanup must preserve app-owned host siblings under the same host.
- Resolve/apply failures clean only router-owned local state and do not touch app-owned hosts.
- Failed navigation remains pending until identity-scoped cleanup is completed and acknowledged through core.
- Cleanup must use explicit identity checks and stable sorted entity lists for every mutation path.
- Cleanup reports expose deterministic ordered mutation evidence, not counts only.

### Input And Focus

- Hidden, leaving, failed, retained, cached, cleanup-pending, cleanup-targeted, preparing, prepared-inert, and pre-commit roots must not keep pointer, keyboard, gamepad, or active focus ownership.
- Active roots accept pointer, keyboard, and gamepad input only after core commit produces a `PostCommitActivation` capability.
- Focus eligibility is enabled only through `PostCommitActivation`; preparing, prepared-inert, and pre-commit roots cannot keep or receive focus.
- Missing lifecycle markers are treated as inert and cannot become active or keep focus.
- Foreign root markers are treated as inert and cannot become active or keep focus.
- Invalid lifecycle or interaction marker combinations are treated as inert and cannot become active or keep focus.
- App-owned host focus is not cleared unless the focused entity is inside a router-owned subtree targeted by finalization.
- If retained/cached roots are introduced, they must be covered by the same input and focus isolation tests as hidden roots. If retained/cached roots are not introduced, tests must prove Phase 3B systems do not create retained/cached roots.

### Facade And Adapter Boundary

- Core must keep Bevy, BSN, platform, app shell, theme, copy, permission, and data-service dependencies out of `bevy_ui_router_core`.
- View must stay renderer-neutral. New reporting/session APIs may help adapters produce valid renderer observations, but must not grant active commit, cleanup, stale acceptance, generation advancement, or frame identity forging authority.
- ECS may depend on Bevy, core, and view. ECS must not depend on BSN, platform crates, app profile concepts, or a concrete host application.
- The facade default feature path must not pull Bevy dependencies.
- Bevy-facing API is available only through the `ecs` feature.
- `bevy_ui_router::core::runtime::RouterRuntime` is allowed as an explicit low-level core runtime path. It must not appear in facade preludes, ECS preludes, view APIs, BSN adapter code, or adapter-facing helper APIs.
- Bevy renderer adapters receive only ECS-minted request-bound reporters/sessions.

### Dependency Source

- Phase 3B is the first Bevy-dependent implementation slice.
- Resolve the project-approved Bevy source before ECS manifest edits.
- The current workspace manifest has no external Bevy dependency entries.
- Git history currently recovers the prior concrete source from `32b96e8:Cargo.toml`: `git = "https://github.com/shengyusi-SYS/bevy.git"`, `branch = "sys/bevy-0.19-router"`, `version = "0.19.0"`, `default-features = false`, features `ui`, `debug`, `bevy_dev_tools`, and `bevy_ui_widgets`.
- Use that recovered source as the candidate project-approved source unless a newer explicit decision document supersedes it.
- Do not choose a crates.io Bevy version or internet "latest" Bevy version as a substitute for the project-approved source.

### Verification Evidence

- Every Rust change is test-first. Record RED and GREEN evidence in `.superpowers/sdd/progress.md` while executing.
- A run that discovers 0 cleanup tests is a setup failure, not an acceptable RED result.
- A run that discovers 0 finalization tests is a setup failure, not an acceptable RED result.
- Final verification commands must pass, or every unavailable command must be reported with exact cause and no false success claim.
- Real Bevy visibility, pickability, and focus mapping is tested when the pinned Bevy source exposes stable components. If it cannot be tested, the final report marks that requirement unverified and Phase 3B does not claim acceptance for it.
- Final evidence scans must cover facade, BSN, and platform sources as well as core, view, and ECS.

---

## Phase 3B Flow Summary

1. Complete the entry gate and Bevy dependency-source gate.
2. Run the plan self-check.
3. Complete the pre-implementation review gate.
4. Add a core-owned runtime bridge that issues host/slot/frame requests and wraps transaction apply/commit authority without exporting that control surface through default preludes.
5. Add controlled request-bound view reporting sessions for non-fake adapter renderer observations.
6. Add ECS Bevy components, resources, plugin, and schedule labels.
7. Add ECS frame apply session foundations.
8. Add handoff-safe ECS cleanup.
9. Add handoff-safe ECS finalization.
10. Add full bridge integration that composes cleanup, finalization, core acceptance, and post-commit activation.
11. Wire facade `ecs` feature exports and boundary tests.
12. Run full Phase 3B verification and multi-agent review.
13. Record Phase 3B verification evidence and handoff to Phase 3C.

Task and commit boundaries are canonical in this rewrite. Do not use a separate global suggested commit list. Each task below owns exactly one implementation commit unless review requires splitting a task before execution begins.

---

## Gates Before Implementation Tasks

### Gate 0: Entry Gate And Dependency Source Gate

**Purpose:** Confirm Phase 3A completion evidence, freeze the Bevy dependency source, and record the Phase 3B execution start before any Phase 3B code changes.

**Files:**

- Read: `docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md`
- Read: `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md`
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
- Read: `Cargo.toml`
- Read: `crates/bevy_ui_router_ecs/Cargo.toml`
- Read/append execution ledger: `.superpowers/sdd/progress.md`

**Gate commands:**

```powershell
git log -1 --oneline
git status --short --branch
Select-String -LiteralPath 'docs\superpowers\plans\2026-07-08-phase-3a-view-contract-implementation-plan.md' -Pattern 'Phase 3A Verification Record' -Context 0,60
git show 32b96e8:Cargo.toml
rg -n "custom Bevy|Bevy 0.19|bevy_ecs|bevy_app|bevy_ui|git = .*bevy" docs Cargo.toml crates
```

**Expected evidence:**

- Latest commit is `3a29754 docs: record phase 3a view contract verification`, or a direct descendant with the same Phase 3A record preserved.
- Worktree is clean before Phase 3B code changes.
- Phase 3A record says verification PASS and includes the Phase 3B handoff statement.
- Bevy dependency source is resolved from git history and current docs, or no code changes begin until the user supplies the source.

**Dependency-source decision:**

- Use the recovered source from `32b96e8:Cargo.toml`: `git = "https://github.com/shengyusi-SYS/bevy.git"`, `branch = "sys/bevy-0.19-router"`, `version = "0.19.0"`, `default-features = false`, features `ui`, `debug`, `bevy_dev_tools`, and `bevy_ui_widgets`.
- If implementation finds a newer explicit decision document, use the newer decision and record the superseding source in the Phase 3B verification record.
- Do not choose a crates.io Bevy version or internet "latest" Bevy version as a substitute for the project-approved source.

**Execution ledger entry:**

Append to `.superpowers/sdd/progress.md`:

```markdown
## Phase 3B ECS Integration

Plan: docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md
Mode: Subagent-Driven on main with explicit user consent
Start commit: record the exact single-line output from `git log -1 --oneline`
Gate 0: complete (gate PASS; Phase 3A verification record present; Bevy dependency source gate resolved or awaiting explicit user source before ECS manifest edits)
```

**Commit boundary:** Gate 0 produces no implementation commit. If `.superpowers/sdd/progress.md` is tracked in the active workflow, include its ledger update in the first implementation commit that follows the gate; otherwise leave it as the execution ledger state and record its status in the final verification record.

### Gate 1: Plan Self-Check

**Purpose:** Run the structural self-check before review and implementation so placeholder text, trailing whitespace, and patch whitespace errors are caught while the plan is still easy to revise.

**Commands:**

```powershell
$zhPending = [string]([char]0x5f85) + [string]([char]0x5b9a)
$zhUnset = [string]([char]0x672a) + [string]([char]0x5b9a)
$zhAfter = [string]([char]0x7a0d) + [string]([char]0x540e)
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME',$zhPending,$zhUnset,$zhAfter,'implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3b-ecs-integration-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3b-ecs-integration-implementation-plan.md'
git diff --check
```

**Expected evidence:**

- No placeholder matches.
- No trailing whitespace.
- `git diff --check` exits successfully.

**Commit boundary:** Gate 1 produces no implementation commit.

### Gate 2: Review Gate Before Implementation

**Purpose:** Review the plan before Phase 3B implementation begins, with independent responsibility areas and lead-agent evaluation.

Before implementing Phase 3B, review this plan with independent agents using these responsibility directions:

- Architecture and lifecycle review: assess whether the plan keeps core as the only lifecycle/stale/active authority while giving ECS enough surface to execute Bevy hierarchy mutation, inert preparation, cleanup, post-commit activation, failure cleanup acknowledgement, and reporting.
- API and authority review: assess whether the proposed core runtime, view report session, ECS session, internal ECS capabilities, and facade feature exports have clear boundaries and do not expose raw constructors, cleanup authority, finalization authority, or adapter commit authority.
- Fact and source review: verify the plan against current source, Phase 3A record, `AGENTS.md`, roadmap docs, and current Cargo manifests. This review must explicitly check the Bevy dependency-source gate.
- Test and risk review: assess whether the planned tests meaningfully cover ownership, core-only stale matrix, public-path stale/error branches, reporter/session authority, input/focus isolation, query-order stability, facade defaults, failure cleanup acknowledgement, and dependency boundaries.

The lead agent must evaluate findings before accepting them. If findings share a root cause, revise the task structure or architecture. If the plan is structurally wrong, replace this plan through the same review loop before code implementation starts.

This planning iteration completed the review-and-rewrite loop before the planning commit. If a later edit materially changes architecture, task boundaries, authority surfaces, or verification scope, rerun Gate 2 before implementation continues.

**Commit boundary:** Gate 2 produces no implementation commit.

---

## Task 1: Add Core-Owned Runtime Bridge

**Purpose:** Add the controlled core-owned runtime bridge that lets ECS obtain host/slot/frame requests and report apply results without exposing transaction, id, stale, or active-commit authority outside core.

**Files:**

- Modify: `crates/bevy_ui_router_core/src/runtime.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Modify: `crates/bevy_ui_router_core/src/transaction.rs` only if existing private helpers need narrow internal adjustment.
- Modify: `crates/bevy_ui_router_core/tests/public_api.rs`
- Modify: `crates/bevy_ui_router_core/tests/dependency_boundary.rs`

**Interface Changes:**

- Produces explicit-path public core type: `RouterRuntime`.
- Consumes public inputs: `MatchedBranch`, `BranchDiff`, `FrameApplyToken`, `FrameApplyOutcome`.
- Produces observations: `RouterInstanceId`, `RouteHostId`, `RouteSlotId`, `RouteFrameScope`, `NavigationTransactionSnapshot`, `RouteFrameBranch`, `TransactionFailure`, `FrameCleanupPlan`.
- Keeps authority internals private: `NavigationTransaction`, `NavigationTransactionId`, `FrameAllocator`, raw id constructors, raw scope constructors, token constructors.

Target public explicit-path shape:

```rust
pub struct RouterRuntime {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    allocator: FrameAllocator,
    active: Option<RouteFrameBranch>,
    pending: Option<NavigationTransaction>,
}

pub enum BeginNavigationError {
    PendingNavigation,
    FailedNavigationNeedsCleanup,
}

pub enum FrameApplyReportError {
    Stale(StaleApplyResult),
    NoPendingNavigation,
    TerminalFailure,
}

pub enum FailureCleanupAckError {
    NoFailedNavigation,
    CleanupPlanMismatch,
}

impl RouterRuntime {
    pub fn new() -> Self;
    pub fn router(&self) -> RouterInstanceId;
    pub fn host(&self) -> RouteHostId;
    pub fn slot(&self) -> RouteSlotId;
    pub fn scope(&self) -> RouteFrameScope;
    pub fn active_branch(&self) -> Option<&RouteFrameBranch>;
    pub fn pending_snapshot(&self) -> Option<NavigationTransactionSnapshot>;
    pub fn begin_first_navigation(
        &mut self,
        target: &MatchedBranch,
    ) -> Result<NavigationTransactionSnapshot, BeginNavigationError>;
    pub fn begin_reconciled_navigation(
        &mut self,
        target: &MatchedBranch,
        diff: &BranchDiff,
    ) -> Result<NavigationTransactionSnapshot, BeginNavigationError>;
    pub fn report_frame_apply(
        &mut self,
        token: &FrameApplyToken,
        outcome: FrameApplyOutcome,
    ) -> Result<NavigationTransactionSnapshot, FrameApplyReportError>;
    pub fn commit_ready_navigation(&mut self) -> Result<&RouteFrameBranch, TransactionCommitError>;
    pub fn acknowledge_failed_navigation_cleanup(
        &mut self,
        completed: &[FrameCleanupPlan],
    ) -> Result<(), FailureCleanupAckError>;
}
```

**RED Tests:**

Create `crates/bevy_ui_router_core/src/runtime.rs` with tests before implementation, and add `pub mod runtime;` to `crates/bevy_ui_router_core/src/lib.rs` before running the RED command so Cargo compiles the new module:

- `runtime_issues_router_host_slot_scope_without_public_id_constructors`
- `runtime_begin_first_navigation_issues_frame_apply_requests`
- `runtime_instances_issue_distinct_router_host_slot_identities`
- `runtime_pending_navigation_conflict_returns_explicit_error`
- `runtime_accepts_apply_only_through_core_stale_check`
- `runtime_does_not_update_active_branch_before_explicit_commit`
- `runtime_commit_rechecks_active_baseline`
- `runtime_failure_exposes_router_owned_cleanup_without_touching_host`
- `runtime_stale_apply_result_has_no_visible_side_effects_for_transaction_router_host_slot_route_frame_generation_and_sequence`
- `runtime_failed_navigation_requires_cleanup_ack_before_next_navigation`
- `runtime_cleanup_ack_rejects_mismatched_cleanup_plans`

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core runtime::tests -- --nocapture
```

Expected RED:

- The tests fail because `RouterRuntime` does not exist.

**Implementation Constraints:**

- `RouterRuntime::new()` allocates router, host, and default slot ids inside core. It does not expose raw constructors.
- Every new `RouterRuntime` must receive distinct router, host, and slot ids. Tests must fail if constants or reused ids would create cleanup collisions.
- `begin_first_navigation` uses `FrameReconciliationPlan::from_first_navigation`.
- `begin_reconciled_navigation` uses `FrameReconciliationPlan::from_diff` and the runtime's current active branch as baseline.
- Starting a new navigation while another transaction is pending must return `BeginNavigationError::PendingNavigation` unless a separate explicit replacement API is designed and tested. It must not silently drop visible active state.
- Starting a new navigation while the previous navigation failed and its cleanup has not been acknowledged must return `BeginNavigationError::FailedNavigationNeedsCleanup`.
- `report_frame_apply` delegates to the private `NavigationTransaction::accept_apply_result`.
- `commit_ready_navigation` delegates to the private `NavigationTransaction::commit`, updates `active` only after success, and removes `pending` only after terminal success or terminal failure has been made observable.
- A stale apply result returns `FrameApplyReportError::Stale` and leaves `pending`, `active`, and failure cleanup unchanged.
- Exhaustive stale coverage for transaction/task identity, sequence, router, host, slot, route, frame, and generation belongs in core runtime tests where private test helpers can mutate tokens without weakening public API.
- `acknowledge_failed_navigation_cleanup` verifies that the completed cleanup plans match the failed transaction cleanup plans before clearing failed pending state. ECS can call it only after identity-scoped cleanup succeeds.
- Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod runtime;
```

- Keep existing prelude exports. Do not add `RouterRuntime`, `BeginNavigationError`, or runtime apply/commit methods to `bevy_ui_router_core::prelude` in Phase 3B.
- `bevy_ui_router::core::runtime::RouterRuntime` is allowed as an explicit low-level core runtime path for custom non-ECS integration authors, because the current facade re-exports the core crate as `bevy_ui_router::core`. That path is not an adapter-facing Bevy renderer API.

**GREEN Verification:**

Update `crates/bevy_ui_router_core/tests/public_api.rs` to assert `RouterRuntime` is explicit-path public low-level core runtime API.

Add or preserve compile-fail doctests proving:

- External crates cannot construct `RouterInstanceId`, `RouteHostId`, `RouteSlotId`, `RouteFrameId`, or `RouteGeneration`.
- External crates cannot construct `RouteFrameScope`.
- External crates cannot name or construct `NavigationTransaction` or `NavigationTransactionId`.
- External crates cannot construct `FrameApplyToken`.
- `bevy_ui_router::prelude::RouterRuntime` is not available through the default facade prelude.
- `bevy_ui_router::prelude::BeginNavigationError`, `FrameApplyReportError`, and `FailureCleanupAckError` are not available through the default facade prelude.
- `bevy_ui_router::core::runtime::RouterRuntime` remains an explicit low-level core path, and later facade/adapter boundary tests prove BSN and ECS public reporter APIs do not use it as renderer authority.

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core runtime::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary -- --nocapture
cargo fmt --all --check
```

Expected:

- Runtime tests pass.
- Core dependency-boundary tests still prove `bevy_ui_router_core` has no dependencies.
- Doctests preserve constructor privacy.
- `RouterRuntime` is explicit-path public low-level core runtime API, but raw transaction and id authorities remain private.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_core/src/runtime.rs crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/transaction.rs crates/bevy_ui_router_core/tests/public_api.rs crates/bevy_ui_router_core/tests/dependency_boundary.rs
git commit -m "feat(core): add controlled router runtime bridge"
```

Do not stage `crates/bevy_ui_router_core/src/transaction.rs` if it was not changed.

---

## Task 2: Add Controlled View Reporting Session

**Purpose:** Add a request-bound view reporting session so non-fake renderers can report observations without gaining lifecycle, cleanup, stale-acceptance, generation, or frame-identity authority.

**Files:**

- Modify: `crates/bevy_ui_router_view/src/contract.rs`
- Modify: `crates/bevy_ui_router_view/src/lib.rs`
- Modify: `crates/bevy_ui_router_view/tests/compile_fail_boundary.rs`
- Modify: `crates/bevy_ui_router_view/tests/authority_surface_boundary.rs`
- Modify: `crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs`

**Interface Changes:**

- Produces explicit-path public type: `ViewReportSession`.
- Produces explicit-path public type: `ViewReportedOutput`.
- May expose these through `bevy_ui_router_view` root and `bevy_ui_router::view` explicit paths.
- Does not add these to `bevy_ui_router_view::prelude` or facade prelude.
- The session must be request-bound. It may be public by explicit path, but it must be created from a core-issued `FrameApplyRequest`, not from a standalone descriptor.

Target public shape in `contract.rs`:

```rust
pub struct ViewReportSession {
    frame_request: FrameApplyRequest,
    issuer: ViewRendererIssuer,
    request_id: ViewRequestId,
    descriptor: RouteViewDescriptor,
    completed: bool,
}

pub struct ViewReportedOutput {
    outcome: ViewApplyOutcome,
}

impl ViewReportSession {
    pub fn from_frame_request(
        request: &FrameApplyRequest,
        descriptor: RouteViewDescriptor,
    ) -> Self;
    pub fn frame_request(&self) -> &FrameApplyRequest;
    pub fn build_request(&self) -> ViewBuildRequest;
    pub fn build_success(&mut self) -> ViewBuildOutcome;
    pub fn build_failed(&mut self, redacted_message: impl Into<String>) -> ViewBuildOutcome;
    pub fn applied(
        &mut self,
        disposal_hint: Option<impl Into<String>>,
    ) -> ViewReportedOutput;
    pub fn apply_failed(
        &mut self,
        redacted_message: impl Into<String>,
        disposal_hint: Option<impl Into<String>>,
    ) -> ViewApplyOutcome;
    pub fn stale(&mut self, redacted_message: impl Into<String>) -> ViewApplyOutcome;
}

impl ViewReportedOutput {
    pub fn outcome(&self) -> &ViewApplyOutcome;
}
```

**RED Tests:**

Add tests proving:

- A `ViewReportSession` can create a build request and render plan for a descriptor without exposing `ViewBuildRequest::new`.
- A `ViewReportSession` cannot be created without a `FrameApplyRequest`.
- Two sessions created from different `FrameApplyRequest` values produce distinct provenance and cannot be mixed by the ECS bridge.
- The session can report apply success and internally create `ViewPartialOutput`.
- The session can report build failure and apply failure while preserving Phase 3A error taxonomy.
- The session can report renderer-local stale result without partial output.
- The session records disposal hints without making `ViewDisposalHint::new` public.
- A completed request cannot be reported twice after build failure, apply success, apply failure, or stale report.

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view contract::tests::report_session -- --nocapture
```

Expected RED:

- Tests fail because the session API does not exist.

**Implementation Constraints:**

- The exact implementation may split method names between build and apply reporting, but it must keep raw constructors private.
- The session is a reporting capability, not lifecycle authority.
- It may allocate view-local request/output ids and disposal hints.
- It may store a clone of the immutable `FrameApplyRequest` so ECS can compare provenance, but it must not expose a mutable token or any method that accepts a token.
- It must not expose any method named `commit`, `accept`, `activate`, `set_active`, `make_active`, `mark_active`, `finalize`, `into_frame_apply`, `to_frame_apply`, `into_core_apply`, or `to_core_apply`.
- Stale reports carry `RouteErrorKind::StaleResult` and no `ViewPartialOutput`.
- ECS bridge tests in Task 5C must reject renderer observations whose session request does not equal the `EcsFrameApplySession` request.
- Replace the Phase 3A deny-list-only authority check with an allow-list plus forbidden authority fragments:
  - Add a structured allow-list of new public reporting types and methods.
  - Continue forbidding active commit, stale acceptance, token construction, generation advancement, cleanup construction, and lifecycle diagnostic construction outside core internals.
  - Update facade authority scan to include the view sources and the new allow-list.

**GREEN Verification:**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --lib -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test compile_fail_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test authority_surface_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary -- --nocapture
cargo fmt --all --check
```

Expected:

- View tests pass.
- Compile-fail fixtures prove raw constructors remain inaccessible.
- Authority tests prove the new session is reporting-only.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_view/src/contract.rs crates/bevy_ui_router_view/src/lib.rs crates/bevy_ui_router_view/tests/compile_fail_boundary.rs crates/bevy_ui_router_view/tests/authority_surface_boundary.rs crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs
git commit -m "feat(view): add controlled renderer report session"
```

---

## Task 3: Add ECS Components, Resources, Plugin, And Schedule Labels

**Purpose:** Establish the ECS crate's public observation types, crate-private authority state, plugin resource registration, schedule labels, and dependency boundary before bridge behavior is added.

**Files:**

- Modify: `crates/bevy_ui_router_ecs/Cargo.toml`
- Replace: `crates/bevy_ui_router_ecs/src/lib.rs`
- Create: `crates/bevy_ui_router_ecs/src/components.rs`
- Create: `crates/bevy_ui_router_ecs/src/plugin.rs`
- Create: `crates/bevy_ui_router_ecs/src/state.rs`
- Create: `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs`
- Create: `crates/bevy_ui_router_ecs/tests/public_api.rs`

**Interface Changes:**

- `RouteHost`
- `RouteSlot`
- `RouteFrameRoot`
- `RouteFrameRootLifecycleSnapshot`
- `RouteFrameInteractionSnapshot`
- `RouteFocusSnapshot`
- `RouterEcsState`
- `BevyUiRouterPlugin`
- `RouterScheduleSet`

Target component/resource shape:

```rust
pub struct RouteHost {
    router: RouterInstanceId,
    host: RouteHostId,
}

pub struct RouteSlot {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
}

pub struct RouteFrameRoot {
    identity: RouteFrameIdentity,
    branch_index: usize,
}

pub enum RouteFrameRootLifecycleSnapshot {
    Preparing,
    PreparedInert,
    Active,
    Leaving,
    Hidden,
    Failed,
    CleanupPending,
    Cached,
}

pub struct RouteFrameInteractionSnapshot {
    accepts_pointer: bool,
    accepts_keyboard: bool,
    accepts_gamepad: bool,
}

pub struct RouteFocusSnapshot {
    focused: Option<Entity>,
}

pub struct RouterEcsState {
    runtime: RouterRuntime,
}

pub(crate) struct RouteFrameRootLifecycleState {
    snapshot: RouteFrameRootLifecycleSnapshot,
}

pub(crate) struct RouteFrameInteractionBarrier {
    accepts_pointer: bool,
    accepts_keyboard: bool,
    accepts_gamepad: bool,
}

pub(crate) struct RouteFocusState {
    focused: Option<Entity>,
}
```

Target plugin and schedule labels:

```rust
pub struct BevyUiRouterPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum RouterScheduleSet {
    CollectNavigation,
    PlanFrames,
    BuildViews,
    ApplyViews,
    PrepareInert,
    ReportToCore,
    ActivateCommitted,
    Cleanup,
    AcknowledgeFailures,
}
```

**RED Tests:**

Only after Gate 0 has a concrete project-approved Bevy source, update `crates/bevy_ui_router_ecs/Cargo.toml`.

Create tests in `crates/bevy_ui_router_ecs/tests/public_api.rs`:

- `ecs_prelude_exposes_host_slot_frame_root_plugin_and_schedule_labels`
- `route_host_and_slot_observe_core_issued_identity`
- `frame_root_records_router_host_slot_route_frame_generation_and_branch_index`
- `route_host_does_not_provide_raw_id_constructors`

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test public_api -- --nocapture
```

Expected RED:

- Tests fail because the ECS types do not exist.

**Implementation Constraints:**

- The dependency set should be the smallest set needed for:
  - `App`/`Plugin` if plugin tests need Bevy app setup.
  - `World`, `Entity`, `Component`, `Resource`, query, command, and relationship APIs.
  - UI visibility/pickability/focus mapping only when available in the pinned Bevy source.
- `bevy_ui_router_ecs` must depend on `bevy_ui_router_core`, `bevy_ui_router_view`, and the minimal Bevy crates chosen from the project-approved source.
- `bevy_ui_router_ecs` must not depend on `bevy_ui_router_bsn`, `bevy_ui_router_platform`, or app-specific crates.
- `RouteHost` and `RouteSlot` constructors must consume ids from `RouterRuntime` or a `RouterEcsState` registration API. They must not accept raw numeric ids.
- `RouteFrameRoot` construction is crate-controlled in ECS and request/session-controlled. It is created from a `FrameApplyRequest` or an `EcsFrameApplySession` that was minted from that request. It must not offer a public constructor that accepts a raw observed `RouteFrameIdentity`, because a valid observed identity could otherwise be duplicated into a fake root.
- `RouteFrameRootLifecycleState` is the authoritative ECS lifecycle component and is crate-private. External crates must not be able to construct, insert, mutate, or set an authoritative `Active` lifecycle state.
- `RouteFrameRootLifecycleSnapshot` is public read-only observation and must not derive or be registered as a Bevy component/resource. Constructing a snapshot is not authority.
- `RouteFrameInteractionBarrier` is the authoritative router-owned input isolation component and is crate-private. It is updated by internal finalization systems from `RouteFrameRootLifecycleState`.
- `RouteFrameInteractionSnapshot` is public read-only observation and must not be accepted by systems as an authority input.
- `RouteFocusState` is a crate-private Bevy resource used by tests and internal finalization systems. `RouteFocusSnapshot` is public read-only observation.
- Public lifecycle, interaction, and focus snapshots are observation DTOs only. They must not be used as Bevy `Component` or `Resource` authority, and systems must not accept them as inputs that can mark a frame active, enable input, or retain focus.
- Finalization and activation systems must treat missing, foreign, or invalid lifecycle/interaction markers as inert. They must never infer active state from a missing marker.
- Keep fields private and expose read-only accessors.
- Keep cleanup and finalization helper functions out of facade and ECS preludes unless a narrow capability story is added with compile-fail tests.
- The plugin registers `RouterEcsState` and the crate-private `RouteFocusState` resource.
- The plugin configures schedule sets in a stable order: collect navigation, plan frames, build views, apply views, prepare inert roots and old roots, report to core, activate committed roots, cleanup inert leaving/failed roots, then acknowledge failure cleanup.
- The plugin must not add BSN systems.
- The plugin must not create app-owned host entities.

**GREEN Verification:**

`crates/bevy_ui_router_ecs/tests/dependency_boundary.rs` must prove:

- ECS depends on core and view.
- ECS may depend on the explicitly approved Bevy package set.
- ECS does not depend on BSN, platform, facade, or app profile crates.
- Core and view dependency-boundary tests still pass.
- Facade default feature path still does not pull Bevy dependencies.

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test public_api -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test dependency_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test dependency_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features facade_default_keeps_bevy_out_of_the_default_path -- --nocapture
cargo fmt --all --check
```

Expected:

- ECS public API and boundary tests pass.
- Default facade path remains Bevy-free.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_ecs/Cargo.toml crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/components.rs crates/bevy_ui_router_ecs/src/plugin.rs crates/bevy_ui_router_ecs/src/state.rs crates/bevy_ui_router_ecs/tests/dependency_boundary.rs crates/bevy_ui_router_ecs/tests/public_api.rs
git commit -m "feat(ecs): add router ECS components and plugin"
```

---

## Task 4: Implement ECS Frame Apply Session Foundations

**Purpose:** Add ECS-owned request-bound apply sessions that can record renderer observations locally without reporting active state, performing cleanup/finalization, or committing core state.

**Files:**

- Create: `crates/bevy_ui_router_ecs/src/bridge.rs`
- Modify: `crates/bevy_ui_router_ecs/src/state.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Create: `crates/bevy_ui_router_ecs/tests/apply_bridge.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/public_api.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs`

**Interface Changes:**

- `EcsFrameApplySession`
- `EcsFrameApplyReport`
- `EcsFrameApplyStatus`

Target bridge shape:

```rust
pub struct EcsFrameApplySession {
    request: FrameApplyRequest,
    report_session: ViewReportSession,
    finalized: bool,
}

pub enum EcsFrameApplyStatus {
    Pending,
    Finalized,
    ReportedToCore,
    StaleDiscarded,
    Failed,
}

pub struct EcsFrameApplyReport {
    status: EcsFrameApplyStatus,
}
```

**RED Tests:**

Create `crates/bevy_ui_router_ecs/tests/apply_bridge.rs`:

- `bridge_mints_session_from_core_frame_apply_request`
- `bridge_does_not_expose_frame_apply_token_mutation`
- `bridge_rejects_view_report_from_foreign_frame_request`
- `bridge_records_renderer_build_failure_without_partial_output`
- `bridge_records_renderer_apply_success_as_unfinalized_observation`
- `bridge_records_renderer_apply_failure_as_unfinalized_failure`
- `bridge_discards_renderer_local_stale_without_core_report`

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test apply_bridge -- --nocapture
```

Expected RED:

- Tests fail because bridge types and systems do not exist.

**Implementation Constraints:**

- `EcsFrameApplySession` has private fields and no public constructor. It is minted only by the controlled ECS bridge from an ECS-owned pending request table entry.
- The bridge rejects duplicate sessions for the same `FrameApplyRequest` unless the previous session reached a terminal discarded/failed/cleaned state.
- The bridge rejects a second `ViewReportSession` created from the same `FrameApplyRequest` when it is not associated with the ECS-owned pending session.
- The session exposes read-only `request()` and `frame()` accessors.
- It does not expose a mutable token, token constructor, or stale-check method.
- It can provide a `ViewBuildRequest` for renderer work through the controlled `ViewReportSession`.
- It records renderer observations locally until ECS finalization has completed.
- A view report from a `ViewReportSession` whose `frame_request()` does not equal the `EcsFrameApplySession` request is rejected before it can create visible output or reach core.
- Task 4 does not claim cleanup, finalization, active commit ordering, core stale rejection, or app-owned host preservation. Those GREEN checks depend on Tasks 5A, 5B, and 5C.

**GREEN Verification:**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test apply_bridge -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core runtime::tests -- --nocapture
cargo fmt --all --check
```

Expected:

- Bridge foundation tests pass.
- Core runtime tests still pass.
- No bridge foundation test marks active UI or calls core commit.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_ecs/src/bridge.rs crates/bevy_ui_router_ecs/src/state.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/tests/apply_bridge.rs crates/bevy_ui_router_ecs/tests/public_api.rs crates/bevy_ui_router_ecs/tests/dependency_boundary.rs
git commit -m "feat(ecs): add frame apply session"
```

---

## Task 5A: Implement Ownership Cleanup

**Purpose:** Add deterministic, identity-scoped cleanup for router-owned subtrees while preserving app-owned hosts and host siblings.

**Files:**

- Create: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/components.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Add module tests in: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify as needed for boundary evidence: `crates/bevy_ui_router_ecs/tests/public_api.rs`
- Modify as needed for boundary evidence: `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs`

**Interface Changes:**

```rust
pub(crate) fn cleanup_router_owned_subtree(
    world: &mut World,
    plan: &FrameCleanupPlan,
    capability: CleanupCapability,
) -> CleanupReport;

pub(crate) struct CleanupCapability;

pub struct CleanupReport {
    removed: Vec<Entity>,
    skipped_mismatches: Vec<Entity>,
}
```

**RED Tests:**

Before running the RED command, register the new module in `crates/bevy_ui_router_ecs/src/lib.rs`:

```rust
pub(crate) mod cleanup;
```

`cleanup.rs` module tests must prove:

- Cleanup removes only entities with matching `RouteFrameRoot` router, host, slot, frame, and generation.
- Cleanup does not despawn the app-owned `RouteHost` entity.
- Cleanup does not despawn app-owned sibling children under the same host.
- Cleanup does not remove another router's frame root.
- Cleanup does not remove another generation of the same frame id.
- Cleanup uses explicit identity checks and stable sorted entity lists for every mutation path.
- `CleanupReport` exposes ordered removed entity ids and ordered skipped entity ids so tests can prove ordering rather than counts only.
- Cleanup ordering key is `(router.raw(), host.raw(), slot.raw(), route.raw(), frame.raw(), generation.raw(), branch_index, entity.index())` or the closest equivalent stable `Entity` key exposed by the pinned Bevy source as the final tie-breaker. Use semantic identity before entity tie-breakers.

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs cleanup::tests -- --nocapture
```

Expected RED:

- Tests fail because cleanup systems do not exist.
- A run that discovers 0 cleanup tests is a setup failure, not an acceptable RED result.

**Implementation Constraints:**

- Use Bevy relationship/hierarchy APIs available from the pinned Bevy source.
- Resolve target roots by explicit `RouteFrameRoot` identity match.
- Traverse descendants only after the root identity check passes.
- Never call despawn on the app-owned host entity.
- Sort collected entities by a stable key before mutation for every cleanup path. This is a runtime rule, not a test-only convenience.
- Return ordered entity lists in the report so tests can assert deterministic cleanup order.
- `CleanupCapability` is crate-private and minted only by the ECS bridge from core cleanup plans. There is no public explicit path to invoke cleanup directly through `bevy_ui_router_ecs` or `bevy_ui_router::ecs`.

**GREEN Verification:**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs cleanup::tests -- --nocapture
cargo fmt --all --check
```

Expected:

- Cleanup tests pass and prove app-owned host integrity.
- Cleanup report assertions prove deterministic ordered mutation evidence.
- Cleanup leaves app-owned host and app-owned host siblings intact.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/components.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/tests/public_api.rs crates/bevy_ui_router_ecs/tests/dependency_boundary.rs
git commit -m "feat(ecs): add router-owned cleanup"
```

Do not stage `crates/bevy_ui_router_ecs/tests/public_api.rs` or `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs` if they were not changed.

---

## Task 5B: Implement Finalization And Input/Focus Isolation

**Purpose:** Add prepared-inert, inactive-finalization, and post-commit activation behavior that keeps roots inert until core commit and releases focus from inactive router-owned subtrees.

**Files:**

- Create: `crates/bevy_ui_router_ecs/src/finalize.rs`
- Modify: `crates/bevy_ui_router_ecs/src/components.rs`
- Modify: `crates/bevy_ui_router_ecs/src/plugin.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Add module tests in: `crates/bevy_ui_router_ecs/src/finalize.rs`
- Modify as needed for boundary evidence: `crates/bevy_ui_router_ecs/tests/public_api.rs`
- Modify as needed for boundary evidence: `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs`

**Interface Changes:**

```rust
pub(crate) fn prepare_inert_root(
    world: &mut World,
    root: Entity,
    capability: PrepareInertCapability,
) -> FinalizationReport;

pub(crate) fn activate_committed_root(
    world: &mut World,
    root: Entity,
    capability: PostCommitActivation,
) -> FinalizationReport;

pub(crate) fn finalize_inactive_root(
    world: &mut World,
    root: Entity,
    capability: InactiveFinalizationCapability,
) -> FinalizationReport;

pub(crate) struct PrepareInertCapability;
pub(crate) struct PostCommitActivation;
pub(crate) struct InactiveFinalizationCapability;
pub struct FinalizationReport;
```

**RED Tests:**

Before running the RED command, register the new module in `crates/bevy_ui_router_ecs/src/lib.rs`:

```rust
pub(crate) mod finalize;
```

`finalize.rs` module tests must prove:

- Preparing and prepared-inert roots reject pointer, keyboard, and gamepad input.
- Active roots accept pointer, keyboard, and gamepad input only after core commit produces a post-commit activation capability.
- Focus eligibility is enabled only through `PostCommitActivation`; preparing, prepared-inert, and pre-commit roots cannot keep or receive focus.
- Hidden, leaving, failed, cleanup-pending, and cleanup-targeted roots reject pointer, keyboard, or gamepad input.
- If Phase 3B introduces retained/cached roots, retained/cached roots do not accept pointer, keyboard, or gamepad input.
- If Phase 3B does not introduce retained/cached roots, tests prove no Phase 3B system creates a retained/cached root state.
- Missing lifecycle markers are treated as inert and cannot become active or keep focus.
- Foreign root markers are treated as inert and cannot become active or keep focus.
- Invalid lifecycle or interaction marker combinations are treated as inert and cannot become active or keep focus.
- If the focused entity belongs to hidden, leaving, failed, or cleanup-targeted subtree, focus is cleared.
- If the focused entity belongs to a retained/cached subtree introduced by Phase 3B, focus is cleared.
- If the focused entity is outside the subtree, focus is preserved.
- App-owned host focus is not cleared unless the focused entity is inside a router-owned subtree targeted by finalization.

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs finalize::tests -- --nocapture
```

Expected RED:

- Tests fail because finalization systems do not exist.
- A run that discovers 0 finalization tests is a setup failure, not an acceptable RED result.

**Implementation Constraints:**

- `prepare_inert_root` creates or confirms router-owned output, marks the private lifecycle component as prepared-inert, and keeps pointer, keyboard, gamepad, and focus disabled before core acceptance.
- `activate_committed_root` is called only after `RouterRuntime::commit_ready_navigation` succeeds. It marks the private lifecycle component as active and only then enables active input.
- `finalize_inactive_root` marks hidden, leaving, failed, cleanup-pending, or cached roots inert and clears focus when focus is inside their router-owned subtree.
- Focus release uses `RouteFocusState`.
- Capability structs are crate-private and minted by controlled ECS systems. There is no public explicit path to call these functions through `bevy_ui_router_ecs` or `bevy_ui_router::ecs`.
- If the pinned Bevy source exposes stable visibility, pickability, and focus components, add mapping systems from router-owned state to Bevy UI state in this task and test them.
- If the pinned Bevy source does not expose stable UI components, Phase 3B must not claim real Bevy pointer/keyboard/gamepad/focus isolation as verified. The implementation must either add tests against available Bevy UI components or mark the real Bevy UI mapping surface as unverified in the verification record and leave Phase 3B acceptance blocked for that requirement.

**GREEN Verification:**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs finalize::tests -- --nocapture
cargo fmt --all --check
```

Expected:

- Finalization tests pass and prove pre-commit roots are inert.
- `PostCommitActivation` enables input and focus eligibility only after core commit.
- Inactive roots release focus as required.
- Missing, foreign, and invalid markers remain inert.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_ecs/src/finalize.rs crates/bevy_ui_router_ecs/src/components.rs crates/bevy_ui_router_ecs/src/plugin.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/tests/public_api.rs crates/bevy_ui_router_ecs/tests/dependency_boundary.rs
git commit -m "feat(ecs): add frame finalization"
```

Do not stage `crates/bevy_ui_router_ecs/tests/public_api.rs` or `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs` if they were not changed.

---

## Task 5C: Integrate Cleanup, Finalization, And Core Apply Bridge

**Purpose:** Compose renderer observations, prepared-inert roots, core apply reporting, core commit, post-commit activation, stale discard, and failure cleanup acknowledgement into the full ECS bridge behavior.

**Files:**

- Modify: `crates/bevy_ui_router_ecs/src/bridge.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/finalize.rs`
- Modify: `crates/bevy_ui_router_ecs/src/components.rs`
- Modify: `crates/bevy_ui_router_ecs/src/plugin.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Modify/extend: `crates/bevy_ui_router_ecs/tests/apply_bridge.rs`

**Interface Changes:**

- Bridge systems may mint crate-private `CleanupCapability`, `PrepareInertCapability`, `PostCommitActivation`, and `InactiveFinalizationCapability`.
- Public or facade-visible `EcsFrameApplySession` still has no public constructor and does not expose methods that accept arbitrary `FrameApplyToken`, cleanup capability, finalization capability, or active commit capability.
- Production call sites for `RouterRuntime::report_frame_apply`, `RouterRuntime::commit_ready_navigation`, and `RouterRuntime::acknowledge_failed_navigation_cleanup` are limited to controlled ECS bridge files.

**RED Tests:**

Extend `crates/bevy_ui_router_ecs/tests/apply_bridge.rs` with:

- `renderer_success_is_not_active_until_ecs_finalization_and_core_commit`
- `renderer_build_failure_reports_core_failure_without_partial_output_or_host_cleanup`
- `renderer_apply_failure_cleans_partial_router_owned_output_and_preserves_host`
- `renderer_stale_result_is_discarded_without_core_commit_or_visible_output`
- `foreign_view_report_session_is_rejected_before_core_report`
- `duplicate_same_request_view_report_session_is_rejected_before_core_report`
- `core_report_error_branch_cleans_local_partial_output_without_active_ui_or_host_cleanup`
- `multiple_frame_requests_report_and_cleanup_in_branch_index_order`

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test apply_bridge -- --nocapture
```

Expected RED:

- New integration tests fail until the bridge composes cleanup, finalization, core acceptance, core commit, and failure acknowledgement.

**Implementation Constraints:**

- `ViewBuildOutcome::Failed(failure)` reports `FrameApplyOutcome::Failed(failure.error().clone())` through `RouterRuntime::report_frame_apply`, creates no active UI, carries no partial output, does not touch the app-owned host, performs any core-issued cleanup, and then calls `RouterRuntime::acknowledge_failed_navigation_cleanup`.
- `ViewApplyOutcome::Applied(partial)` becomes eligible for `FrameApplyOutcome::Applied` only after:
  - a router-owned frame root exists under the matching route slot,
  - the frame root marker identity matches router, host, slot, route, frame, and generation,
  - the new root has been prepared inert,
  - replaced/leaving roots have been finalized inert so they cannot receive input or retain focus.
- `ViewApplyOutcome::Failed(failure)` reports `FrameApplyOutcome::Failed(failure.error().clone())` to core, cleans any core-issued router-owned cleanup plans plus local partial output, and then acknowledges cleanup back to core.
- `ViewApplyOutcome::Stale(failure)` is discarded by the ECS bridge and is not reported to core as success or failure. It must not create active UI, must not create retained partial output, and must not clear app-owned host.
- If core rejects the token as stale, ECS must discard the result and perform any local router-owned cleanup associated with the abandoned partial output.
- When multiple requests exist, sort by `FrameApplyRequest::frame().branch_index()`, then by `FrameApplyToken::sequence()`. Do not depend on query iteration order. Spawn entities in reverse order in tests and assert report/finalization/cleanup order remains branch-index stable.
- Exhaustive forged-token stale mismatch coverage is core-only in Task 1. ECS tests must not forge tokens. ECS tests cover the public-path error branch by using a crate-local reporter stub or controlled test seam that returns `FrameApplyReportError::Stale` without exposing token mutation to production code or adapters.
- Active commit ordering tests must prove this exact order:
  1. Core issues `FrameApplyRequest`.
  2. Renderer reports success through the request-bound controlled session.
  3. ECS creates or confirms router-owned frame root under the expected slot.
  4. ECS prepares the new root inert and finalizes replaced/leaving roots inert.
  5. ECS submits `FrameApplyOutcome::Applied` to `RouterRuntime::report_frame_apply`.
  6. Only after all pending requests are ready does ECS call `RouterRuntime::commit_ready_navigation`.
  7. Only after core commit succeeds does ECS mint `PostCommitActivation`.
  8. Only with `PostCommitActivation` does ECS mark the frame root active and enable input/focus eligibility.
- Do not mark a frame root active after renderer success alone.

**GREEN Verification:**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs cleanup::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs finalize::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test apply_bridge -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core runtime::tests -- --nocapture
cargo fmt --all --check
```

Expected:

- Cleanup tests pass and prove app-owned host integrity.
- Finalization tests pass and prove pre-commit roots are inert, post-commit activation enables input only after core commit, and inactive roots release focus.
- Apply bridge integration tests pass after cleanup/finalization exists.
- Active UI changes only after core acceptance and commit.
- Failed navigation cleanup is acknowledged through core before next navigation can begin.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router_ecs/src/bridge.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/finalize.rs crates/bevy_ui_router_ecs/src/components.rs crates/bevy_ui_router_ecs/src/plugin.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/tests/apply_bridge.rs
git commit -m "feat(ecs): integrate apply bridge"
```

---

## Task 6: Wire Facade ECS Feature And Boundary Tests

**Purpose:** Expose intended ECS integration types through the facade `ecs` feature while keeping the default path Bevy-free and preventing lifecycle authority from leaking through preludes or adapter-facing APIs.

**Files:**

- Modify: `crates/bevy_ui_router/src/lib.rs`
- Modify: `crates/bevy_ui_router/tests/facade_features.rs`
- Modify: `crates/bevy_ui_router/tests/facade_compile_fail_boundary.rs`
- Modify: `crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs`

**Interface Changes:**

- Add `bevy_ui_router::ecs` only when the `ecs` feature is enabled.
- Add ECS common types to the facade prelude only when the `ecs` feature is enabled.
- Keep runtime apply/commit control out of default and ECS facade preludes.

Additive facade snippet:

```rust
#[cfg(feature = "ecs")]
pub use bevy_ui_router_ecs as ecs;

pub mod prelude {
    #[allow(unused_imports)]
    pub use bevy_ui_router_core::prelude::*;

    #[cfg(feature = "view")]
    #[allow(unused_imports)]
    pub use bevy_ui_router_view::prelude::*;

    #[cfg(feature = "ecs")]
    #[allow(unused_imports)]
    pub use bevy_ui_router_ecs::prelude::*;
}
```

If the actual facade prelude already uses a different local structure, keep that structure and make the ECS line an additive export rather than replacing existing exports. `bevy_ui_router::core::runtime::RouterRuntime` remains an explicit low-level core runtime path because the facade already re-exports the core crate, but it must not appear in facade preludes, ECS preludes, view APIs, BSN adapter code, or adapter-facing helper APIs.

**RED Tests:**

Add tests:

- `facade_ecs_module_exposes_plugin_host_slot_and_schedule_labels_with_ecs_feature`
- `facade_prelude_exposes_ecs_common_types_only_with_ecs_feature`
- `facade_default_path_does_not_expose_ecs_module`
- `facade_ecs_feature_does_not_expose_raw_commit_or_acceptance_authority`
- `facade_prelude_does_not_expose_router_runtime_apply_commit_control`

Run focused RED:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test facade_features -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary -- --nocapture
```

Expected RED:

- Tests fail until facade prelude and boundaries are updated.

**Implementation Constraints:**

Compile-fail fixtures must prove:

- `bevy_ui_router::ecs` is missing without the `ecs` feature.
- ECS feature does not expose `NavigationTransaction`.
- ECS feature does not expose token constructors.
- ECS feature does not expose raw id constructors.
- ECS feature does not expose adapter-facing active commit methods.
- Default and ECS facade preludes do not expose `RouterRuntime`, `BeginNavigationError`, `FrameApplyReportError`, `FailureCleanupAckError`, `report_frame_apply`, `commit_ready_navigation`, or `acknowledge_failed_navigation_cleanup`.
- ECS prelude and explicit `bevy_ui_router::ecs` paths do not expose `cleanup_router_owned_subtree`, `prepare_inert_root`, `activate_committed_root`, `finalize_inactive_root`, `CleanupCapability`, `PrepareInertCapability`, `PostCommitActivation`, or `InactiveFinalizationCapability`.
- Public `RouteFrameRoot` cannot be constructed from a public `RouteFrameIdentity`.
- Public `RouteFrameRootLifecycleSnapshot`, `RouteFrameInteractionSnapshot`, and `RouteFocusSnapshot` cannot be inserted as authoritative Bevy components/resources and cannot be used to mark a root active.
- The crate-private `RouteFrameRootLifecycleState`, `RouteFrameInteractionBarrier`, and `RouteFocusState` are not reachable through `bevy_ui_router_ecs` or `bevy_ui_router::ecs` explicit paths.
- Public `ViewReportSession` cannot be constructed with a standalone descriptor and must require a `FrameApplyRequest`.
- Public or facade-visible `EcsFrameApplySession` has no public constructor and does not expose methods that accept arbitrary `FrameApplyToken`, cleanup capability, finalization capability, or active commit capability.

Authority scan must:

- Allow definitions of `RouterRuntime::report_frame_apply`, `RouterRuntime::commit_ready_navigation`, and `RouterRuntime::acknowledge_failed_navigation_cleanup` only in core runtime source.
- Allow call sites for those runtime methods only in controlled ECS bridge source files and tests. A production call site in view, BSN, facade, platform, or adapter-facing helper code fails the scan.
- Allow ECS reporting/session names only in ECS source and facade compile-fail fixtures.
- Continue forbidding raw constructors and active mutation methods in view and adapter-facing facade surfaces.
- Back the source allow-list with compile-fail API path tests for `RouterRuntime`, `ViewReportSession`, `EcsFrameApplySession`, `RouteFrameRoot`, lifecycle snapshots, interaction snapshots, focus snapshots, internal lifecycle/barrier/focus state, cleanup/finalization capability paths, and ECS prelude exposure.

**GREEN Verification:**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test facade_features -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary -- --nocapture
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs
cargo fmt --all --check
```

Expected:

- Facade feature tests pass.
- Default facade path stays Bevy-free.
- ECS feature exposes the intended ECS integration surface.
- Authority scan is allow-list based for Phase 3B additions.

**Commit Boundary:**

```powershell
git add crates/bevy_ui_router/src/lib.rs crates/bevy_ui_router/tests/facade_features.rs crates/bevy_ui_router/tests/facade_compile_fail_boundary.rs crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs
git commit -m "feat(facade): expose ecs integration feature"
```

---

## Task 7: Phase 3B Verification Record

**Purpose:** Record exact final verification evidence, review results, dependency-source decision, unresolved risks, and Phase 3C handoff semantics in the accepted Phase 3B plan.

**Files:**

- Modify after lead acceptance: `docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md`
- Modify: `.superpowers/sdd/progress.md`

**Interface Changes:**

- No Rust API changes.
- Adds a `Phase 3B Verification Record` section to the accepted plan document.
- Adds or updates execution ledger evidence in `.superpowers/sdd/progress.md`.

**RED Tests:**

No new RED tests are required for Task 7 because it records evidence after implementation tasks complete. The entry condition is that every implementation task has already completed its RED/GREEN cycle and task commit.

**Implementation Constraints:**

- Do not claim acceptance until every required command has fresh output in the current verification run.
- If any command creates an untracked root `Cargo.lock`, inspect whether the workspace policy has changed. If it is only a generated artifact from verification and the repository intentionally remains lockfile-free, remove it with an approved cleanup command and record that action.
- If real Bevy visibility, pickability, or focus mapping cannot be tested because the pinned Bevy source does not expose stable components, mark that requirement unverified and do not claim Phase 3B acceptance for it.
- The lead agent must evaluate review findings before accepting them. If multiple findings share a root cause such as identity authority leakage, ECS active-commit authority, reporter overexposure, Bevy dependency-source uncertainty, or finalization modeled only as names without behavior, revise the architecture or split the work before isolated edits.

**GREEN Verification:**

Minimum commands:

```powershell
cargo fmt --all --check
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test facade_features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
```

Focused evidence checks:

```powershell
rg -n "NavigationTransaction|NavigationTransactionId|FrameAllocator|FrameApplyToken::|RouteFrameIdentity::new|CleanupScope::router_owned_subtree" crates\bevy_ui_router_ecs\src crates\bevy_ui_router_ecs\tests crates\bevy_ui_router_view\src crates\bevy_ui_router_view\tests crates\bevy_ui_router\src crates\bevy_ui_router\tests crates\bevy_ui_router_bsn\src crates\bevy_ui_router_platform\src
rg -n "RouteFrameRootLifecycleSnapshot|RouteFrameRootLifecycleState|RouteFrameInteractionSnapshot|RouteFrameInteractionBarrier|RouteFocusSnapshot|RouteFocusState|cleanup_router_owned_subtree|prepare_inert_root|activate_committed_root|finalize_inactive_root|PostCommitActivation|CleanupCapability" crates\bevy_ui_router_ecs\src crates\bevy_ui_router_ecs\tests crates\bevy_ui_router\src crates\bevy_ui_router\tests crates\bevy_ui_router_bsn\src crates\bevy_ui_router_platform\src
rg -n "ViewReportSession|EcsFrameApplySession|RouterRuntime|report_frame_apply|commit_ready_navigation|acknowledge_failed_navigation_cleanup" crates\bevy_ui_router_core\src crates\bevy_ui_router_core\tests crates\bevy_ui_router_view\src crates\bevy_ui_router_view\tests crates\bevy_ui_router_ecs\src crates\bevy_ui_router_ecs\tests crates\bevy_ui_router\src crates\bevy_ui_router\tests crates\bevy_ui_router_bsn\src crates\bevy_ui_router_platform\src
```

Expected focused evidence:

- ECS, view, and facade tests do not construct raw authority types.
- Cleanup, inert preparation, post-commit activation, and failure-ack symbols are present only in core runtime, controlled ECS internals, compile-fail fixture text, tests, authority allow-list text, or documentation.
- Core runtime defines `report_frame_apply`, `commit_ready_navigation`, and `acknowledge_failed_navigation_cleanup`; production call sites are limited to controlled ECS bridge files.
- Every match must be classified as one of: core implementation, controlled ECS implementation, compile-fail fixture text, tests, authority allow-list text, or documentation. Any production match outside the allowed file set fails this evidence check.

Whole-branch review:

- Architecture and lifecycle review: assess whether Phase 3B preserves core lifecycle authority, separates renderer success, prepared-inert ECS output, core acceptance, core active commit, post-commit activation, and failure cleanup acknowledgement; keeps pending scene out of active UI; and applies exhaustive stale checks only inside core.
- API and authority review: assess whether public API exposure is minimal, raw constructors remain private, facade feature gating is correct, and reporter/session APIs do not grant commit, stale acceptance, cleanup, or identity forging authority to adapters.
- Fact and source review: verify every claim in this document and the final verification record against current source, Phase 3A handoff evidence, `AGENTS.md`, and roadmap docs.
- Test and risk review: assess whether tests cover ownership cleanup, app-owned host integrity, input/focus isolation, stable ordering, dependency boundaries, facade boundaries, core stale rejection, and active commit ordering.

Append a `Phase 3B Verification Record` section to the accepted plan document with:

- implementation commits;
- exact command outputs and pass counts;
- dependency-source decision;
- real Bevy visibility/pickability/focus mapping result: tested, or blocked with exact unavailable API/source cause;
- review results;
- unresolved risks;
- Phase 3C handoff statement.

Required Phase 3C handoff statement:

Phase 3C may build the immediate BSN adapter only through the Phase 3B ECS public API and controlled reporter/session. BSN must not construct frame identity, generation, cleanup scope, transaction tokens, or active-commit state. BSN resolve/apply failures must be reported through the Phase 3B session and mapped to core/view errors without bypassing ECS cleanup/finalization.

**Commit Boundary:**

```powershell
git add docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md .superpowers/sdd/progress.md
git commit -m "docs: record phase 3b ecs integration verification"
```

Do not stage `.superpowers/sdd/progress.md` if the active workflow keeps it untracked or local-only.

---

## Canonical Verification And Acceptance Matrix

| Area | Required evidence | Acceptance semantics |
| --- | --- | --- |
| Phase 3A gate | Gate 0 commands show Phase 3A verification PASS and Phase 3B handoff preserved. | Phase 3B begins only after Phase 3A verification PASS is confirmed. |
| Dependency source | Gate 0 `git show` and `rg` evidence identify the project-approved Bevy source or block ECS manifest edits. | Bevy dependency source is explicitly resolved through git-history and current-doc checks before ECS manifest edits. |
| Core runtime authority | Task 1 runtime tests, public API tests, doctests, and dependency-boundary tests pass. | `RouterRuntime` or accepted equivalent owns router, host, slot, frame allocation, pending transaction, active branch, apply result acceptance, and active commit. |
| Navigation conflicts | Task 1 tests cover pending navigation and failed cleanup acknowledgement requirements. | Runtime begin-navigation conflicts return an explicit error or explicit replacement outcome; failed navigation remains pending until identity-scoped cleanup is completed and acknowledged through core. |
| Runtime identity uniqueness | Task 1 runtime test `runtime_instances_issue_distinct_router_host_slot_identities` passes. | Runtime instances issue distinct router, host, and slot identities. |
| Runtime prelude boundary | Task 1 public/boundary tests and Task 6 facade tests pass. | Runtime apply/commit control is not exposed through default or ECS facade preludes. |
| Core stale matrix | Task 1 stale tests pass across transaction/task identity, sequence, router, host, slot, route, frame, and generation. | Core stale token rejection has no visible side effects; ECS tests do not forge tokens. |
| View report session | Task 2 view tests, compile-fail fixtures, and authority scans pass. | Non-fake adapters can report renderer observations only through a controlled request-bound reporter/session minted from a core-issued `FrameApplyRequest`; raw Phase 3A view constructors remain private. |
| ECS dependency boundary | Task 3 dependency-boundary tests pass. | ECS depends only on core, view, and approved Bevy crates; default facade remains Bevy-free. |
| ECS identity components | Task 3 public API tests pass. | ECS observes router, host, slot, route, frame, generation, and branch index without public raw constructors or fake root authority. |
| Internal lifecycle state | Task 3 and Task 6 compile-fail tests pass. | Public snapshots are observation DTOs only; crate-private lifecycle, interaction, and focus state are not reachable through facade or ECS explicit public paths. |
| Bridge session foundation | Task 4 apply bridge foundation tests pass. | ECS uses core-issued `FrameApplyRequest` as the canonical handoff unit and does not expose token mutation or active commit authority. |
| Cleanup ownership | Task 5A cleanup tests pass with ordered evidence. | Resolve/apply failures clean only router-owned local state; app-owned hosts and host siblings survive; cleanup does not depend on query order. |
| Cleanup capability boundary | Task 5A tests and Task 6 compile-fail tests pass. | `cleanup_router_owned_subtree` and `CleanupCapability` are crate-private or capability-protected and unavailable through explicit facade paths. |
| Finalization input/focus isolation | Task 5B finalization tests pass. | Hidden/leaving/failed/cleanup-targeted and pre-commit router-owned subtrees reject pointer, keyboard, and gamepad input and cannot retain active focus. |
| Marker inertness | Task 5B finalization tests pass for missing, foreign, and invalid markers. | Missing, foreign, or invalid lifecycle/interaction markers are inert and cannot become active or keep focus. |
| Post-commit activation | Task 5B and Task 5C tests pass. | Post-commit activation is a separate ECS step after core commit succeeds; only that step can enable active input/focus eligibility. |
| Retained/cached roots | Task 5B tests either cover retained/cached roots or prove Phase 3B systems do not create them. | Retained/cached subtree runtime is either absent and tested absent, or present and covered by the same input/focus isolation tests. |
| Full bridge success path | Task 5C integration tests pass in the required active commit order. | Renderer success does not create active UI; active UI changes only after ECS finalization, core acceptance, core commit, and `PostCommitActivation`. |
| Full bridge failure path | Task 5C integration tests pass for build failure, apply failure, and core report stale/error branch. | Renderer build/apply failure reports failure without partial active UI or app-owned host cleanup, cleans local router-owned output, and acknowledges cleanup through core. |
| Renderer stale path | Task 5C integration tests pass for renderer-local stale discard. | Renderer stale result is discarded with no visible active state, retained partial output, core success/failure report, or app-owned host cleanup. |
| Stable ordering | Task 5A and Task 5C tests spawn reverse-order entities/requests and assert stable ordered evidence. | ECS systems do not rely on Bevy query iteration order. |
| Real Bevy UI mapping | Task 5B tests against pinned Bevy UI components, or Task 7 records exact unavailable API/source cause. | Real Bevy visibility, pickability, and focus mapping is tested when possible; otherwise Phase 3B does not claim acceptance for that requirement. |
| Facade feature boundary | Task 6 facade feature, compile-fail, authority-surface tests, and `cargo check -p bevy_ui_router --features ecs` pass. | Facade `ecs` feature exposes intended ECS integration types without exposing lifecycle authority; default facade path remains Bevy-free. |
| Final scans | Task 7 focused `rg` scans cover ECS, view, facade, BSN, and platform source/test paths. | Raw authority, cleanup/finalization capability, session, runtime, and commit methods appear only in allowed implementation, tests, fixtures, allow-list text, or docs. |
| Final command evidence | Task 7 minimum commands pass with exact outputs and pass counts, or failures are recorded with exact cause. | Final verification commands pass, or every unavailable command is reported with exact cause and no false success claim. |
| Review and handoff | Task 7 whole-branch review passes after lead-agent evaluation; verification record contains the required Phase 3C handoff statement. | Whole-branch multi-agent review passes after lead-agent evaluation and any required revisions; Phase 3C is constrained to Phase 3B ECS public API and controlled reporter/session. |

---

## Acceptance Criteria

Acceptance is the complete satisfaction of the Canonical Verification And Acceptance Matrix. If any matrix row lacks evidence, the final report must mark that row unavailable or failing with exact cause and must not claim Phase 3B acceptance.
