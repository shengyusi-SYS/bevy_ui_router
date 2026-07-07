# Phase 2 Frame Error Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the core-only frame lifecycle and navigation transaction contract that keeps requested location, matched branch, pending apply results, and active rendered branch separate.

**Architecture:** Phase 2 stays inside `bevy_ui_router_core` and models runtime frame identity as data, not Bevy entities. A `RouteFrameScope` carries router/host/runtime-slot context independently from the active branch, so first navigation does not masquerade as an empty active UI. Core-owned transactions consume Phase 1B `MatchedBranch` and `BranchDiff`, stage entering/leaving frame records, accept fake apply-result harness outputs only through identity-checked tokens, and expose read-only snapshots instead of public commit authority.

**Tech Stack:** Rust 2024, zero runtime dependencies for `bevy_ui_router_core`, existing Cargo workspace, `cargo fmt`, `cargo test`, compile-fail doctests, and facade no-default checks.

## Global Constraints

- Phase 2 can execute only after Phase 1B implementation, review, final verification, and documentation commit `bf2dff4` or later equivalent evidence.
- Router core remains renderer-agnostic and must not depend on Bevy ECS, BSN, platform APIs, application services, themes, permissions, data services, or application shell models.
- Phase 2 must not create Bevy entities, mutate ECS hierarchy, apply views, hide retained subtrees, release focus, run async tasks, or call BSN/platform adapters.
- Requested location, matched branch, pending transaction, and active rendered branch are distinct states.
- Pending apply results are not active UI.
- A frame becomes active only after its apply token is accepted, the current active branch still matches the transaction baseline, and commit succeeds.
- Stale apply results must be discarded before visible state changes. Stale checks must cover router, host, slot, frame identity, generation, transaction identity, and apply token sequence.
- Failed transactions must preserve the old active branch and produce cleanup scopes only for router-owned partial output.
- Cleanup plans must never include app-owned host cleanup authority.
- Error-boundary resolution uses the nearest matched ancestor boundary from Phase 1B metadata. Phase 2 records fallback targets but does not render fallback UI.
- Frame identity and generation must be observable but not externally forgeable.
- Adapters may observe transaction and frame data through public APIs, but they cannot construct frame ids, generations, transaction ids, frame records, apply tokens, cleanup scopes, lifecycle diagnostic payloads, or commit authority.
- `NavigationTransaction` is a core-internal authority object. Public APIs expose observation snapshots and apply requests only.
- Core dependency sections must remain empty.
- Every filtered `cargo test` command is invalid evidence if no intended target harness runs nonzero tests.
- Use `cargo test --config "build.rustc-wrapper = ''" ...` and `cargo check --config "build.rustc-wrapper = ''" ...` for Cargo verification in this workspace.

---

## 1. Preconditions

Before executing Phase 2, verify:

- `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md` contains a Phase 1B verification record.
- `crates/bevy_ui_router_core/src/branch_diff.rs` exists.
- `crates/bevy_ui_router_core/src/fingerprint.rs` exists.
- `crates/bevy_ui_router_core/src/matcher.rs` exists.
- `crates/bevy_ui_router_core/tests/public_api.rs` exists.
- The worktree is clean.

Run:

```powershell
git log --oneline -12
git status --short
Test-Path docs\superpowers\plans\2026-07-07-phase-1b-branch-planning-contract-plan.md
Test-Path crates\bevy_ui_router_core\src\branch_diff.rs
Test-Path crates\bevy_ui_router_core\src\fingerprint.rs
Test-Path crates\bevy_ui_router_core\src\matcher.rs
Test-Path crates\bevy_ui_router_core\tests\public_api.rs
```

Expected: the log includes `bf2dff4 docs: record phase 1b branch planning verification` or a later equivalent verification commit, `git status --short` prints nothing, and every `Test-Path` command prints `True`.

If any precondition fails, do not execute Phase 2. Repair Phase 1B evidence or revise this plan first.

---

## 2. Phase 2 Scope

Phase 2 implements:

- core frame identity and status records;
- active frame branch snapshots;
- common-prefix frame preservation and entering/leaving frame planning;
- transaction ids, apply tokens, and pre-apply stale rejection;
- pre-commit active-branch recheck;
- transaction failure records and router-owned cleanup scopes;
- nearest error-boundary failure targeting;
- frame lifecycle diagnostics;
- public boundary tests proving adapters cannot forge frame or transaction authority.

Phase 2 intentionally does not implement:

- Bevy `Entity`, `Component`, `World`, `Commands`, or schedule behavior;
- renderer/view descriptor or fake renderer traits;
- a real renderer adapter. Phase 2 may include a core-only fake apply-result harness in tests to prove transaction behavior without introducing view contracts;
- BSN scene apply;
- focus, pickability, visibility, retained subtree, or transition runtime behavior;
- async task spawning or polling;
- platform back/deep-link adapters;
- app-specific metadata or error payloads.

---

## 3. File Structure

- Create `crates/bevy_ui_router_core/src/frame.rs`
  - Owns `RouteFrameScope`, `RouteFrameStatus`, `RouteFrameIdentity`, `RouteFrameRecord`, `RouteFrameBranch`, `FrameAllocator`, `FrameReconciliationPlan`, and `FrameCleanupPlan`.
- Create `crates/bevy_ui_router_core/src/transaction.rs`
  - Owns core-internal `NavigationTransaction`, `NavigationTransactionId`, `FrameApplyToken`, `FrameApplyRequest`, `FrameApplyOutcome`, `NavigationTransactionSnapshot`, `NavigationTransactionState`, `AcceptedFrameApply`, `StaleApplyResult`, `TransactionCommitError`, `TransactionFailure`, and test-only fake apply-result harness helpers.
- Modify `crates/bevy_ui_router_core/src/diagnostics.rs`
  - Adds structured frame lifecycle diagnostics without public event-authoring authority.
- Modify `crates/bevy_ui_router_core/src/error.rs`
  - Adds crate-visible constructors for stale result, view apply failure, cleanup failure, and `FinalizeFailed` if absent.
- Modify `crates/bevy_ui_router_core/src/lib.rs`
  - Exports new modules and a small stable prelude surface.
- Modify `crates/bevy_ui_router_core/tests/public_api.rs`
  - Adds positive public module-path tests and compile-fail boundary support where integration tests are better than doctests.
- Modify `crates/bevy_ui_router_core/tests/dependency_boundary.rs`
  - Keeps core dependency sections empty with a Phase 2-named guard.
- Modify `crates/bevy_ui_router/tests/facade_features.rs`
  - Verifies facade no-default prelude exposes stable Phase 2 observation contracts without exposing constructors.

---

## 4. Public Contract

Phase 2 should leave these stable public observation APIs available. Constructors and mutating methods marked `pub(crate)` are intentionally listed so implementers know not to make them public.

```rust
pub struct RouteFrameScope;

impl RouteFrameScope {
    pub(crate) fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
    ) -> Self;

    pub fn router(&self) -> RouterInstanceId;
    pub fn host(&self) -> RouteHostId;
    pub fn slot(&self) -> RouteSlotId;
}

pub enum RouteFrameStatus {
    Matching,
    Guarding,
    Redirecting,
    Building,
    ResolvingView,
    ApplyingView,
    Entering,
    Active,
    Pausing,
    Leaving,
    Cached,
    Error,
}

pub struct RouteFrameIdentity;

impl RouteFrameIdentity {
    pub(crate) fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        route: RouteId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> Self;

    pub fn router(&self) -> RouterInstanceId;
    pub fn host(&self) -> RouteHostId;
    pub fn slot(&self) -> RouteSlotId;
    pub fn route(&self) -> RouteId;
    pub fn frame(&self) -> RouteFrameId;
    pub fn generation(&self) -> RouteGeneration;
}

pub struct RouteFrameRecord;

impl RouteFrameRecord {
    pub(crate) fn new(
        identity: RouteFrameIdentity,
        branch_index: usize,
        slot_key: RouteSlotKey,
        segment_reuse_key: RouteSegmentReuseKey,
        status: RouteFrameStatus,
    ) -> Self;

    pub fn identity(&self) -> &RouteFrameIdentity;
    pub fn branch_index(&self) -> usize;
    pub fn slot_key(&self) -> &RouteSlotKey;
    pub fn segment_reuse_key(&self) -> &RouteSegmentReuseKey;
    pub fn status(&self) -> RouteFrameStatus;
}

pub struct RouteFrameBranch;

impl RouteFrameBranch {
    pub fn frames(&self) -> &[RouteFrameRecord];
    pub fn branch_fingerprint(&self) -> &RouteBranchFingerprint;
}

pub struct FrameReconciliationPlan;

impl FrameReconciliationPlan {
    pub fn preserved(&self) -> &[RouteFrameRecord];
    pub fn leaving(&self) -> &[RouteFrameRecord];
    pub fn entering(&self) -> &[RouteFrameRecord];
    pub fn baseline_fingerprint(&self) -> Option<&RouteBranchFingerprint>;
    pub fn target_fingerprint(&self) -> &RouteBranchFingerprint;
}

pub struct FrameCleanupPlan;

impl FrameCleanupPlan {
    pub fn identity(&self) -> &RouteFrameIdentity;
    pub fn scope(&self) -> CleanupScope;
}

pub struct FrameLifecycleDiagnostic;

impl FrameLifecycleDiagnostic {
    pub fn identity(&self) -> &RouteFrameIdentity;
    pub fn status(&self) -> RouteFrameStatus;
}

pub enum NavigationTransactionState {
    PendingApply,
    ReadyToCommit,
    Committed,
    Failed,
}

pub struct FrameApplyToken;

impl FrameApplyToken {
    pub fn sequence(&self) -> u64;
    pub fn router(&self) -> RouterInstanceId;
    pub fn host(&self) -> RouteHostId;
    pub fn slot(&self) -> RouteSlotId;
    pub fn route(&self) -> RouteId;
    pub fn frame(&self) -> RouteFrameId;
    pub fn generation(&self) -> RouteGeneration;
}

pub struct FrameApplyRequest;

impl FrameApplyRequest {
    pub fn token(&self) -> &FrameApplyToken;
    pub fn frame(&self) -> &RouteFrameRecord;
}

pub enum FrameApplyOutcome {
    Applied,
    Failed(RouteError),
}

pub enum StaleApplyReason {
    TransactionMismatch,
    SequenceMismatch,
    RouterMismatch,
    HostMismatch,
    SlotMismatch,
    RouteMismatch,
    FrameMismatch,
    GenerationMismatch,
}

pub struct StaleApplyResult;

impl StaleApplyResult {
    pub fn reason(&self) -> StaleApplyReason;
}

pub enum TransactionCommitErrorKind {
    ChangedCurrentBranch,
    PendingApply,
    Failed,
}

pub struct TransactionCommitError;

impl TransactionCommitError {
    pub fn kind(&self) -> TransactionCommitErrorKind;
}

pub struct TransactionFailure;

impl TransactionFailure {
    pub fn failing_route(&self) -> RouteId;
    pub fn error(&self) -> &RouteError;
    pub fn nearest_error_boundary(&self) -> Option<&ErrorBoundaryKey>;
    pub fn cleanup(&self) -> &[FrameCleanupPlan];
}

pub struct NavigationTransactionSnapshot;

impl NavigationTransactionSnapshot {
    pub fn state(&self) -> NavigationTransactionState;
    pub fn requests(&self) -> &[FrameApplyRequest];
    pub fn failure(&self) -> Option<&TransactionFailure>;
}

pub(crate) struct NavigationTransaction;

impl NavigationTransaction {
    pub(crate) fn new(plan: FrameReconciliationPlan) -> Self;
    pub(crate) fn snapshot(&self) -> NavigationTransactionSnapshot;
    pub(crate) fn accept_apply_result(
        &mut self,
        token: &FrameApplyToken,
        outcome: FrameApplyOutcome,
    ) -> Result<(), StaleApplyResult>;
    pub(crate) fn commit(
        self,
        current_active: Option<&RouteFrameBranch>,
    ) -> Result<RouteFrameBranch, TransactionCommitError>;
}
```

Phase 2 prelude additions are limited to:

```rust
pub mod prelude {
    pub use crate::frame::{
        FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameIdentity,
        RouteFrameRecord, RouteFrameScope, RouteFrameStatus,
    };
    pub use crate::transaction::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, NavigationTransactionSnapshot,
        NavigationTransactionState, StaleApplyReason, StaleApplyResult, TransactionCommitError,
        TransactionCommitErrorKind, TransactionFailure,
    };
}
```

Do not add `NavigationTransaction`, `NavigationTransactionId`, `FrameAllocator`, or any constructor-only authority type to the prelude.

---

## 5. Semantics

### 5.1 Frame Identity

- `RouteFrameScope` is the core data identity for one router/host/runtime-slot surface. It exists even when there is no active rendered branch yet.
- `RouteFrameIdentity` is the core data identity for one runtime route frame.
- It contains router, host, runtime slot, route id, frame id, and generation.
- Phase 2 uses a runtime `RouteSlotId` as the host/root slot scope and separately records the declaration-level `RouteSlotKey` on `RouteFrameRecord`.
- Phase 3B may map `RouteSlotKey` to Bevy `RouteSlot` entities, but Phase 2 must not introduce ECS slot behavior.
- External code can observe scope and identity fields but cannot construct them.

### 5.2 Frame Reuse

- Common-prefix records preserve frame id and generation.
- Leaving records are copied from the old active suffix in leaf-to-root order.
- Entering records are allocated in root-to-leaf suffix order and start as `Building`.
- First navigation uses `active: None`, preserves no frames, leaves no frames, and enters the full matched branch within an explicit `RouteFrameScope`.
- Search or provenance fail-closed diffs enter the full new branch and leave the full old branch.
- Generation advances only through core-controlled retry or replacement paths, not through adapter input.

### 5.3 Transaction

- A core-internal `NavigationTransaction` owns one reconciliation plan. Public code observes it through `NavigationTransactionSnapshot`.
- Each entering frame receives exactly one apply token for the current generation and transaction sequence.
- `FrameApplyOutcome::Applied` moves that staged frame to an applied/entering-ready internal state.
- `FrameApplyOutcome::Failed(error)` records a transaction failure, produces cleanup plans for staged partial output, and prevents commit.
- `commit(current_active)` rechecks that the optional current active branch equals the transaction baseline. `None` is valid only when the transaction baseline was also `None`.
- If the active branch changed after the transaction was created, commit returns a stale/changed-current error and does not produce a new active branch.
- Public APIs do not expose `accept_apply_result` or `commit`. Later adapters must submit outcomes through a router-owned public API, not by mutating transactions directly.

### 5.4 Stale Results

An apply result is stale when any of these do not match the pending transaction:

- router id;
- host id;
- slot id;
- route id;
- frame id;
- generation;
- transaction id;
- apply sequence.

Stale results return `StaleApplyResult` and do not change transaction state, active frame branch, diagnostics active identity, or cleanup plans.

### 5.5 Error Boundary

- Apply failure records the failing frame identity, failing route id, error kind, and nearest error-boundary key from the matched branch.
- If no matched ancestor boundary exists, Phase 2 records router-level fallback intent as `None`.
- Phase 2 does not render fallback UI and does not replace history.

### 5.6 Cleanup

- Cleanup plans target only records created by the failed transaction.
- Cleanup plans use `CleanupScope::router_owned_subtree`.
- Cleanup plans must report `excludes_app_owned_host() == true`.
- Failed cleanup is represented as `RouteErrorKind::CleanupFailed` and does not authorize host cleanup.

---

## 6. Coverage Matrix

| Requirement | Test Name | Command | Expected Evidence |
| --- | --- | --- | --- |
| Frame identity/status records are observable but constructors remain core-only | `frame::tests` and doctests | `cargo test -p bevy_ui_router_core frame::tests -- --nocapture`; `cargo test -p bevy_ui_router_core --doc` | Nonzero tests and compile-fail constructor boundary |
| First navigation has no active branch but still has explicit scope | `frame::tests::first_navigation_enters_full_branch_without_active_snapshot` | `cargo test -p bevy_ui_router_core frame::tests::first_navigation_enters_full_branch_without_active_snapshot -- --nocapture` | Nonzero test pass |
| Common-prefix frame ids are preserved and suffix frames are allocated | `frame::tests::reconciliation_preserves_common_prefix_and_allocates_entering_suffix` | `cargo test -p bevy_ui_router_core frame::tests::reconciliation_preserves_common_prefix_and_allocates_entering_suffix -- --nocapture` | Nonzero test pass |
| Search/provenance fail-closed diff replaces full frame branch | `frame::tests::fail_closed_diff_replaces_entire_frame_branch` | `cargo test -p bevy_ui_router_core frame::tests::fail_closed_diff_replaces_entire_frame_branch -- --nocapture` | Nonzero test pass |
| Apply token identity check rejects every stale dimension without side effects | `transaction::tests::all_apply_token_identity_mismatches_are_stale_without_side_effects` | `cargo test -p bevy_ui_router_core transaction::tests::all_apply_token_identity_mismatches_are_stale_without_side_effects -- --nocapture` | Nonzero test pass |
| Commit rechecks active branch baseline | `transaction::tests::commit_rejects_changed_current_active_branch` | `cargo test -p bevy_ui_router_core transaction::tests::commit_rejects_changed_current_active_branch -- --nocapture` | Nonzero test pass |
| Failed transaction preserves old active branch and creates router-owned cleanup only | `transaction::tests::apply_failure_preserves_old_active_and_limits_cleanup_scope` | `cargo test -p bevy_ui_router_core transaction::tests::apply_failure_preserves_old_active_and_limits_cleanup_scope -- --nocapture` | Nonzero test pass |
| Multi-frame failure cleans attempted/accepted partial output only | `transaction::tests::multi_frame_failure_cleans_only_attempted_transaction_output` | `cargo test -p bevy_ui_router_core transaction::tests::multi_frame_failure_cleans_only_attempted_transaction_output -- --nocapture` | Nonzero test pass |
| Nearest error boundary and router fallback are recorded | `transaction::tests::failure_prefers_nearest_nested_error_boundary`; `transaction::tests::failure_without_boundary_records_router_fallback` | `cargo test -p bevy_ui_router_core transaction::tests -- --nocapture` | Nonzero test pass |
| Core-only fake apply-result harness models build/apply success/failure and partial output without a renderer | `transaction::tests::fake_apply_harness_models_build_apply_and_partial_output_paths` | `cargo test -p bevy_ui_router_core transaction::tests::fake_apply_harness_models_build_apply_and_partial_output_paths -- --nocapture` | Nonzero test pass |
| Diagnostics describe frame lifecycle without public event authority | `diagnostics::tests::frame_lifecycle_diagnostic_records_identity_and_status` | `cargo test -p bevy_ui_router_core diagnostics::tests::frame_lifecycle_diagnostic_records_identity_and_status -- --nocapture` | Nonzero test pass |
| Public API/facade expose observation contracts but not constructors | `public_api` and facade tests | `cargo test -p bevy_ui_router_core --test public_api -- --nocapture`; `cargo test -p bevy_ui_router --no-default-features` | Nonzero integration tests pass |
| Core remains dependency-free | `dependency_boundary` | `cargo test -p bevy_ui_router_core --test dependency_boundary -- --nocapture` | Nonzero tests pass |

---

## 7. Tasks

### Task 0: Phase 2 Entry Gate

**Files:**
- Read: `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`
- Read: `.superpowers/sdd/progress.md`

**Interfaces:**
- Consumes: Phase 1B branch planning contracts.
- Produces: permission to start Phase 2 tasks.

- [ ] **Step 1: Verify Phase 1B completion evidence**

Run:

```powershell
git log --oneline -12
git status --short
Select-String -Path docs\superpowers\plans\2026-07-07-phase-1b-branch-planning-contract-plan.md -Pattern "Phase 1B Verification Record"
Select-String -Path .superpowers\sdd\progress.md -Pattern "Phase 1B Branch Planning Contract","Whole-branch review"
```

Expected: log includes `bf2dff4`, status is clean, the plan has a verification record, and the ledger has a Phase 1B whole-branch review entry.

- [ ] **Step 2: Run carry-forward smoke checks**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core branch_diff::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core matcher::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core fingerprint::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api -- --nocapture
```

Expected: the module commands report nonzero branch diff, matcher, and fingerprint tests; the integration command reports 1 public API integration test.

- [ ] **Step 3: Record the gate**

Append to `.superpowers/sdd/progress.md`:

```markdown
## Phase 2 Frame Error Transaction

Plan: docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md
Mode: Subagent-Driven on main with explicit user consent
Start commit: <current HEAD>
Task 0: complete (gate PASS; Phase 1B verification record and branch-planning smoke checks present)
```

Do not commit `.superpowers/` files.

### Task 1: Frame Identity And Status Records

**Files:**
- Create: `crates/bevy_ui_router_core/src/frame.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouterInstanceId`, `RouteHostId`, `RouteSlotId`, `RouteId`, `RouteFrameId`, `RouteGeneration`, `RouteSlotKey`, `RouteSegmentReuseKey`, `CleanupScope`.
- Produces: `RouteFrameScope`, `RouteFrameStatus`, `RouteFrameIdentity`, `RouteFrameRecord`, `FrameCleanupPlan`.

- [ ] **Step 1: Add RED frame identity tests**

Create `crates/bevy_ui_router_core/src/frame.rs` with these tests first:

```rust
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::slot::RouteSlotKey;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::RouteParams;
    use crate::fingerprint::{RouteParamsFingerprint, RouteSegmentReuseKey};

    fn segment(route: RouteId) -> RouteSegmentReuseKey {
        let params = RouteParams::new();
        let params_fingerprint = RouteParamsFingerprint::from_route_params(route, &params);
        RouteSegmentReuseKey::from_parts(
            route,
            &RouteSlotKey::default(),
            None,
            &params_fingerprint,
        )
    }

    #[test]
    fn frame_scope_records_router_host_and_runtime_slot_without_active_branch() {
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );

        assert_eq!(scope.router().raw(), 1);
        assert_eq!(scope.host().raw(), 2);
        assert_eq!(scope.slot().raw(), 3);
    }

    #[test]
    fn frame_identity_records_router_host_slot_route_frame_and_generation() {
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteId::new(4),
            RouteFrameId::new(5),
            RouteGeneration::initial(),
        );

        assert_eq!(identity.router().raw(), 1);
        assert_eq!(identity.host().raw(), 2);
        assert_eq!(identity.slot().raw(), 3);
        assert_eq!(identity.route().raw(), 4);
        assert_eq!(identity.frame().raw(), 5);
        assert_eq!(identity.generation().raw(), 0);
    }

    #[test]
    fn frame_record_tracks_status_slot_key_and_segment_reuse_key() {
        let route = RouteId::new(7);
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            route,
            RouteFrameId::new(8),
            RouteGeneration::initial(),
        );
        let record = RouteFrameRecord::new(
            identity,
            2,
            RouteSlotKey::named("detail").unwrap(),
            segment(route),
            RouteFrameStatus::Building,
        );

        assert_eq!(record.identity(), &identity);
        assert_eq!(record.branch_index(), 2);
        assert_eq!(record.slot_key().as_str(), "detail");
        assert_eq!(record.status(), RouteFrameStatus::Building);
        assert_eq!(record.segment_reuse_key().as_str(), segment(route).as_str());
    }

    #[test]
    fn cleanup_plan_targets_only_router_owned_subtree() {
        let identity = RouteFrameIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteId::new(4),
            RouteFrameId::new(5),
            RouteGeneration::initial(),
        );
        let cleanup = FrameCleanupPlan::router_owned_partial_output(identity);

        assert_eq!(cleanup.identity(), &identity);
        assert!(cleanup.scope().excludes_app_owned_host());
        assert_eq!(cleanup.scope().frame(), RouteFrameId::new(5));
    }
}
```

Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod frame;

pub mod prelude {
    pub use crate::frame::{
        FrameCleanupPlan, RouteFrameIdentity, RouteFrameRecord, RouteFrameScope,
        RouteFrameStatus,
    };
}
```

Preserve all existing module declarations and prelude exports.

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core frame::tests -- --nocapture
```

Expected RED: compile failure because the frame types are not implemented. If the command exits 0 with no intended tests, fix placement before implementation.

- [ ] **Step 2: Implement frame identity and cleanup records**

Implement `crates/bevy_ui_router_core/src/frame.rs`:

```rust
use crate::fingerprint::{RouteBranchFingerprint, RouteSegmentReuseKey};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::ownership::CleanupScope;
use crate::slot::RouteSlotKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFrameScope {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
}

impl RouteFrameScope {
    pub(crate) const fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
    ) -> Self {
        Self { router, host, slot }
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }

    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteFrameStatus {
    Matching,
    Guarding,
    Redirecting,
    Building,
    ResolvingView,
    ApplyingView,
    Entering,
    Active,
    Pausing,
    Leaving,
    Cached,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteFrameIdentity {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    route: RouteId,
    frame: RouteFrameId,
    generation: RouteGeneration,
}

impl RouteFrameIdentity {
    pub(crate) const fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        route: RouteId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            route,
            frame,
            generation,
        }
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }

    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(&self) -> RouteGeneration {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteFrameRecord {
    identity: RouteFrameIdentity,
    branch_index: usize,
    slot_key: RouteSlotKey,
    segment_reuse_key: RouteSegmentReuseKey,
    status: RouteFrameStatus,
}

impl RouteFrameRecord {
    pub(crate) fn new(
        identity: RouteFrameIdentity,
        branch_index: usize,
        slot_key: RouteSlotKey,
        segment_reuse_key: RouteSegmentReuseKey,
        status: RouteFrameStatus,
    ) -> Self {
        Self {
            identity,
            branch_index,
            slot_key,
            segment_reuse_key,
            status,
        }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn branch_index(&self) -> usize {
        self.branch_index
    }

    pub const fn slot_key(&self) -> &RouteSlotKey {
        &self.slot_key
    }

    pub const fn segment_reuse_key(&self) -> &RouteSegmentReuseKey {
        &self.segment_reuse_key
    }

    pub const fn status(&self) -> RouteFrameStatus {
        self.status
    }

    pub(crate) fn with_status(&self, status: RouteFrameStatus) -> Self {
        let mut next = self.clone();
        next.status = status;
        next
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteFrameBranch {
    frames: Vec<RouteFrameRecord>,
    branch_fingerprint: RouteBranchFingerprint,
}

impl RouteFrameBranch {
    pub(crate) fn new(
        frames: Vec<RouteFrameRecord>,
        branch_fingerprint: RouteBranchFingerprint,
    ) -> Self {
        Self {
            frames,
            branch_fingerprint,
        }
    }

    pub fn frames(&self) -> &[RouteFrameRecord] {
        &self.frames
    }

    pub const fn branch_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.branch_fingerprint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameCleanupPlan {
    identity: RouteFrameIdentity,
    scope: CleanupScope,
}

impl FrameCleanupPlan {
    pub(crate) fn router_owned_partial_output(identity: RouteFrameIdentity) -> Self {
        Self {
            scope: CleanupScope::router_owned_subtree(
                identity.router(),
                identity.host(),
                identity.slot(),
                identity.frame(),
                identity.generation(),
            ),
            identity,
        }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn scope(&self) -> CleanupScope {
        self.scope
    }
}
```

Keep the RED tests in the same file. If `const fn` cannot call an accessor because of current Rust const restrictions, make only that accessor non-const and record the reason in the task report.

- [ ] **Step 3: Run GREEN verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core frame::tests -- --nocapture
cargo fmt --check
```

Expected: frame tests pass with 4 nonzero tests; formatting passes.

- [ ] **Step 4: Commit**

```powershell
git add crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/lib.rs
git commit -m "feat(core): add route frame records"
```

### Task 2: Frame Branch Reconciliation

**Files:**
- Modify: `crates/bevy_ui_router_core/src/frame.rs`

**Interfaces:**
- Consumes: `MatchedBranch`, `BranchDiff`, `RouteFrameBranch`.
- Produces: `FrameAllocator` and `FrameReconciliationPlan`.

- [ ] **Step 1: Add RED reconciliation tests**

Add these tests inside `crates/bevy_ui_router_core/src/frame.rs`:

```rust
#[test]
fn first_navigation_enters_full_branch_without_active_snapshot() {
    let matcher = sample_items_matcher();
    let target = matcher
        .match_location(&RouteLocation::new("/items/1"))
        .unwrap();
    let mut allocator = FrameAllocator::new();
    let scope = RouteFrameScope::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
    );

    let plan = FrameReconciliationPlan::from_first_navigation(scope, &target, &mut allocator);

    assert!(plan.baseline_fingerprint().is_none());
    assert!(plan.preserved().is_empty());
    assert!(plan.leaving().is_empty());
    assert_eq!(plan.entering().len(), target.routes().len());
    assert_eq!(plan.entering()[0].identity().router(), RouterInstanceId::new(1));
    assert_eq!(plan.entering()[0].status(), RouteFrameStatus::Building);
}

#[test]
fn reconciliation_preserves_common_prefix_and_allocates_entering_suffix() {
    let matcher = sample_items_matcher();
    let old_branch = matcher
        .match_location(&RouteLocation::new("/items/1"))
        .unwrap();
    let new_branch = matcher
        .match_location(&RouteLocation::new("/items/2"))
        .unwrap();
    let diff = BranchDiff::compare(&old_branch, &new_branch);
    let mut allocator = FrameAllocator::new();
    let scope = RouteFrameScope::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
    );
    let active = RouteFrameBranch::from_matched_branch(
        scope,
        &old_branch,
        &mut allocator,
    );

    let plan = FrameReconciliationPlan::from_diff(
        scope,
        Some(&active),
        &new_branch,
        &diff,
        &mut allocator,
    );

    assert_eq!(plan.preserved().len(), 2);
    assert_eq!(plan.leaving().len(), 1);
    assert_eq!(plan.entering().len(), 1);
    assert_eq!(
        plan.preserved()[0].identity().frame(),
        active.frames()[0].identity().frame()
    );
    assert_ne!(
        plan.entering()[0].identity().frame(),
        active.frames()[2].identity().frame()
    );
    assert_eq!(plan.entering()[0].status(), RouteFrameStatus::Building);
}

#[test]
fn fail_closed_diff_replaces_entire_frame_branch() {
    let old_matcher = sample_items_matcher();
    let new_matcher = sample_items_matcher();
    let old_branch = old_matcher
        .match_location(&RouteLocation::new("/items/1"))
        .unwrap();
    let new_branch = new_matcher
        .match_location(&RouteLocation::new("/items/1"))
        .unwrap();
    let diff = BranchDiff::compare(&old_branch, &new_branch);
    let mut allocator = FrameAllocator::new();
    let scope = RouteFrameScope::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
    );
    let active = RouteFrameBranch::from_matched_branch(
        scope,
        &old_branch,
        &mut allocator,
    );

    let plan = FrameReconciliationPlan::from_diff(
        scope,
        Some(&active),
        &new_branch,
        &diff,
        &mut allocator,
    );

    assert!(diff.provenance_changed());
    assert!(plan.preserved().is_empty());
    assert_eq!(plan.leaving().len(), active.frames().len());
    assert_eq!(plan.entering().len(), new_branch.routes().len());
}
```

Add these helper imports and helper function inside the test module:

```rust
use crate::branch_diff::BranchDiff;
use crate::location::RouteLocation;
use crate::matcher::RouteMatcher;
use crate::pattern::RoutePattern;
use crate::route_tree::RouteTree;

fn sample_items_matcher() -> RouteMatcher {
    let mut builder = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap();
    let root = builder.root_id().unwrap();
    let items = builder
        .route(root, RoutePattern::path("items").unwrap())
        .unwrap();
    builder
        .route(items, RoutePattern::path(":id").unwrap())
        .unwrap();
    RouteMatcher::new(builder.build().unwrap())
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core frame::tests::reconciliation_preserves_common_prefix_and_allocates_entering_suffix -- --nocapture
```

Expected RED: compile failure because `FrameAllocator` and `FrameReconciliationPlan` are not implemented.

- [ ] **Step 2: Implement allocator and reconciliation plan**

Add to `crates/bevy_ui_router_core/src/frame.rs`:

```rust
use crate::branch::MatchedBranch;
use crate::branch_diff::BranchDiff;

#[derive(Debug)]
pub(crate) struct FrameAllocator {
    next_frame: u64,
}

impl FrameAllocator {
    pub(crate) const fn new() -> Self {
        Self { next_frame: 1 }
    }

    fn allocate(&mut self) -> RouteFrameId {
        let frame = RouteFrameId::new(self.next_frame);
        self.next_frame += 1;
        frame
    }
}

impl RouteFrameBranch {
    pub(crate) fn from_matched_branch(
        scope: RouteFrameScope,
        branch: &MatchedBranch,
        allocator: &mut FrameAllocator,
    ) -> Self {
        let frames = branch
            .routes()
            .iter()
            .enumerate()
            .map(|(index, matched)| {
                let identity = RouteFrameIdentity::new(
                    scope.router(),
                    scope.host(),
                    scope.slot(),
                    matched.route(),
                    allocator.allocate(),
                    RouteGeneration::initial(),
                );
                RouteFrameRecord::new(
                    identity,
                    index,
                    matched.slot().clone(),
                    matched.segment_reuse_key().clone(),
                    RouteFrameStatus::Active,
                )
            })
            .collect();
        Self::new(frames, branch.branch_fingerprint().clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameReconciliationPlan {
    preserved: Vec<RouteFrameRecord>,
    leaving: Vec<RouteFrameRecord>,
    entering: Vec<RouteFrameRecord>,
    baseline_fingerprint: Option<RouteBranchFingerprint>,
    target_fingerprint: RouteBranchFingerprint,
}

impl FrameReconciliationPlan {
    pub(crate) fn from_diff(
        scope: RouteFrameScope,
        active: Option<&RouteFrameBranch>,
        target: &MatchedBranch,
        diff: &BranchDiff,
        allocator: &mut FrameAllocator,
    ) -> Self {
        let common = diff.common_prefix_len();
        let active_frames = active.map(RouteFrameBranch::frames).unwrap_or(&[]);
        let preserved = active_frames[..common].to_vec();
        let leaving = active_frames[common..]
            .iter()
            .rev()
            .map(|record| record.with_status(RouteFrameStatus::Leaving))
            .collect();
        let entering = target.routes()[common..]
            .iter()
            .enumerate()
            .map(|(offset, matched)| {
                let branch_index = common + offset;
                let identity = RouteFrameIdentity::new(
                    scope.router(),
                    scope.host(),
                    scope.slot(),
                    matched.route(),
                    allocator.allocate(),
                    RouteGeneration::initial(),
                );
                RouteFrameRecord::new(
                    identity,
                    branch_index,
                    matched.slot().clone(),
                    matched.segment_reuse_key().clone(),
                    RouteFrameStatus::Building,
                )
            })
            .collect();

        Self {
            preserved,
            leaving,
            entering,
            baseline_fingerprint: active.map(|branch| branch.branch_fingerprint().clone()),
            target_fingerprint: target.branch_fingerprint().clone(),
        }
    }

    pub(crate) fn from_first_navigation(
        scope: RouteFrameScope,
        target: &MatchedBranch,
        allocator: &mut FrameAllocator,
    ) -> Self {
        let entering = target
            .routes()
            .iter()
            .enumerate()
            .map(|(index, matched)| {
                let identity = RouteFrameIdentity::new(
                    scope.router(),
                    scope.host(),
                    scope.slot(),
                    matched.route(),
                    allocator.allocate(),
                    RouteGeneration::initial(),
                );
                RouteFrameRecord::new(
                    identity,
                    index,
                    matched.slot().clone(),
                    matched.segment_reuse_key().clone(),
                    RouteFrameStatus::Building,
                )
            })
            .collect();
        Self {
            preserved: Vec::new(),
            leaving: Vec::new(),
            entering,
            baseline_fingerprint: None,
            target_fingerprint: target.branch_fingerprint().clone(),
        }
    }

    pub fn preserved(&self) -> &[RouteFrameRecord] {
        &self.preserved
    }

    pub fn leaving(&self) -> &[RouteFrameRecord] {
        &self.leaving
    }

    pub fn entering(&self) -> &[RouteFrameRecord] {
        &self.entering
    }

    pub const fn baseline_fingerprint(&self) -> Option<&RouteBranchFingerprint> {
        self.baseline_fingerprint.as_ref()
    }

    pub const fn target_fingerprint(&self) -> &RouteBranchFingerprint {
        &self.target_fingerprint
    }
}
```

Do not add an empty active-branch sentinel. First navigation must use `from_first_navigation(scope, target, allocator)` with `baseline_fingerprint() == None`.

- [ ] **Step 3: Run GREEN verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core frame::tests -- --nocapture
cargo fmt --check
```

Expected: frame tests pass with at least 6 nonzero tests; formatting passes.

- [ ] **Step 4: Commit**

```powershell
git add crates/bevy_ui_router_core/src/frame.rs
git commit -m "feat(core): plan frame reconciliation"
```

### Task 3: Navigation Transaction And Stale Apply Gate

**Files:**
- Create: `crates/bevy_ui_router_core/src/transaction.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `FrameReconciliationPlan`, `RouteFrameBranch`, `RouteFrameRecord`.
- Produces: core-internal `NavigationTransaction`, public `NavigationTransactionSnapshot`, public `FrameApplyRequest`, public `FrameApplyToken`, stale apply rejection, and commit baseline recheck.

- [ ] **Step 1: Add RED transaction tests**

Create `crates/bevy_ui_router_core/src/transaction.rs` with these tests first:

```rust
use crate::frame::{FrameReconciliationPlan, RouteFrameBranch};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch_diff::BranchDiff;
    use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    use crate::frame::{FrameAllocator, RouteFrameScope};
    use crate::ids::{RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId};
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::pattern::RoutePattern;
    use crate::route_tree::RouteTree;

    fn branches() -> (RouteFrameBranch, FrameReconciliationPlan) {
        let mut builder = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap();
        let root = builder.root_id().unwrap();
        let items = builder
            .route(root, RoutePattern::path("items").unwrap())
            .unwrap();
        builder
            .route(items, RoutePattern::path(":id").unwrap())
            .unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());
        let old_branch = matcher
            .match_location(&RouteLocation::new("/items/1"))
            .unwrap();
        let new_branch = matcher
            .match_location(&RouteLocation::new("/items/2"))
            .unwrap();
        let diff = BranchDiff::compare(&old_branch, &new_branch);
        let mut allocator = FrameAllocator::new();
        let scope = RouteFrameScope::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
        );
        let active = RouteFrameBranch::from_matched_branch(
            scope,
            &old_branch,
            &mut allocator,
        );
        let plan = FrameReconciliationPlan::from_diff(
            scope,
            Some(&active),
            &new_branch,
            &diff,
            &mut allocator,
        );
        (active, plan)
    }

    #[test]
    fn all_apply_token_identity_mismatches_are_stale_without_side_effects() {
        let (_active, plan) = branches();
        for (mut stale, expected) in stale_token_cases(&NavigationTransaction::new(plan.clone())) {
            let mut transaction = NavigationTransaction::new(plan.clone());

            let result = transaction.accept_apply_result(&stale, FrameApplyOutcome::Applied);

            assert_eq!(result.unwrap_err().reason(), expected);
            assert_eq!(transaction.snapshot().state(), NavigationTransactionState::PendingApply);
            assert_eq!(transaction.accepted_count(), 0);
            assert!(transaction.snapshot().failure().is_none());
        }
    }

    #[test]
    fn commit_rejects_changed_current_active_branch() {
        let (_active, plan) = branches();
        let mut transaction = NavigationTransaction::new(plan.clone());
        let token = transaction.snapshot().requests()[0].token().clone();
        transaction
            .accept_apply_result(&token, FrameApplyOutcome::Applied)
            .unwrap();
        let changed_active = RouteFrameBranch::new(Vec::new(), plan.target_fingerprint().clone());

        let result = transaction.commit(Some(&changed_active));

        assert_eq!(result.unwrap_err().kind(), TransactionCommitErrorKind::ChangedCurrentBranch);
    }

    #[test]
    fn fake_apply_harness_models_build_apply_and_partial_output_paths() {
        let (_active, plan) = branches();

        let mut success = NavigationTransaction::new(plan.clone());
        let built =
            FakeApplyHarness::build_success(success.snapshot().requests()[0].token());
        assert_eq!(built.frame(), success.snapshot().requests()[0].token().frame());
        let (token, outcome) = FakeApplyHarness::apply_success(built);
        success.accept_apply_result(&token, outcome).unwrap();
        assert_eq!(success.snapshot().state(), NavigationTransactionState::ReadyToCommit);

        let mut build_failure = NavigationTransaction::new(plan.clone());
        let (token, outcome) = FakeApplyHarness::build_failure(
            build_failure.snapshot().requests()[0].token(),
            RouteError::new(
                RouteErrorKind::ViewBuildFailed,
                RouteErrorSourceCategory::RendererContract,
                "view build failed",
            ),
        );
        build_failure.accept_apply_result(&token, outcome).unwrap();
        assert_eq!(
            build_failure.snapshot().state(),
            NavigationTransactionState::Failed
        );

        let mut apply_failure = NavigationTransaction::new(plan);
        let built =
            FakeApplyHarness::build_success(apply_failure.snapshot().requests()[0].token());
        let (token, outcome) = FakeApplyHarness::apply_failure(
            built,
            RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "view apply failed",
            ),
        );
        apply_failure.accept_apply_result(&token, outcome).unwrap();
        assert_eq!(
            apply_failure.snapshot().state(),
            NavigationTransactionState::Failed
        );
    }

    fn stale_token_cases(transaction: &NavigationTransaction) -> Vec<(FrameApplyToken, StaleApplyReason)> {
        let base = transaction.snapshot().requests()[0].token().clone();
        let mut cases = Vec::new();

        let mut token = base.clone();
        token.testing_set_transaction_for_stale_check();
        cases.push((token, StaleApplyReason::TransactionMismatch));

        let mut token = base.clone();
        token.testing_set_sequence(999);
        cases.push((token, StaleApplyReason::SequenceMismatch));

        let mut token = base.clone();
        token.testing_set_router(RouterInstanceId::new(9));
        cases.push((token, StaleApplyReason::RouterMismatch));

        let mut token = base.clone();
        token.testing_set_host(RouteHostId::new(9));
        cases.push((token, StaleApplyReason::HostMismatch));

        let mut token = base.clone();
        token.testing_set_slot(RouteSlotId::new(9));
        cases.push((token, StaleApplyReason::SlotMismatch));

        let mut token = base.clone();
        token.testing_set_route(RouteId::new(9));
        cases.push((token, StaleApplyReason::RouteMismatch));

        let mut token = base.clone();
        token.testing_set_frame(RouteFrameId::new(9));
        cases.push((token, StaleApplyReason::FrameMismatch));

        let mut token = base;
        token.testing_set_generation(RouteGeneration::initial().next());
        cases.push((token, StaleApplyReason::GenerationMismatch));

        cases
    }
}
```

Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod transaction;

pub mod prelude {
    pub use crate::transaction::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, NavigationTransactionSnapshot,
        NavigationTransactionState, StaleApplyReason, StaleApplyResult,
        TransactionCommitError, TransactionCommitErrorKind, TransactionFailure,
    };
}
```

Preserve all existing exports.

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests -- --nocapture
```

Expected RED: compile failure because transaction types are not implemented.

- [ ] **Step 2: Implement transaction token and stale gate**

Implement `crates/bevy_ui_router_core/src/transaction.rs`:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::RouteError;
use crate::frame::{
    FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameRecord,
    RouteFrameStatus,
};
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationTransactionState {
    PendingApply,
    ReadyToCommit,
    Committed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NavigationTransactionId(u64);

impl NavigationTransactionId {
    fn next() -> Self {
        static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameApplyToken {
    transaction: NavigationTransactionId,
    sequence: u64,
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    route: RouteId,
    frame: RouteFrameId,
    generation: RouteGeneration,
}

impl FrameApplyToken {
    fn from_record(transaction: NavigationTransactionId, sequence: u64, record: &RouteFrameRecord) -> Self {
        let identity = record.identity();
        Self {
            transaction,
            sequence,
            router: identity.router(),
            host: identity.host(),
            slot: identity.slot(),
            route: identity.route(),
            frame: identity.frame(),
            generation: identity.generation(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn router(&self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(&self) -> RouteHostId {
        self.host
    }

    pub const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn frame(&self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(&self) -> RouteGeneration {
        self.generation
    }

    #[cfg(test)]
    pub(crate) fn testing_set_generation(&mut self, generation: RouteGeneration) {
        self.generation = generation;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_transaction_for_stale_check(&mut self) {
        self.transaction = NavigationTransactionId(self.transaction.0 + 1);
    }

    #[cfg(test)]
    pub(crate) fn testing_set_sequence(&mut self, sequence: u64) {
        self.sequence = sequence;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_router(&mut self, router: RouterInstanceId) {
        self.router = router;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_host(&mut self, host: RouteHostId) {
        self.host = host;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_slot(&mut self, slot: RouteSlotId) {
        self.slot = slot;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_route(&mut self, route: RouteId) {
        self.route = route;
    }

    #[cfg(test)]
    pub(crate) fn testing_set_frame(&mut self, frame: RouteFrameId) {
        self.frame = frame;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameApplyRequest {
    token: FrameApplyToken,
    frame: RouteFrameRecord,
}

impl FrameApplyRequest {
    pub fn token(&self) -> &FrameApplyToken {
        &self.token
    }

    pub fn frame(&self) -> &RouteFrameRecord {
        &self.frame
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameApplyOutcome {
    Applied,
    Failed(RouteError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaleApplyReason {
    TransactionMismatch,
    SequenceMismatch,
    RouterMismatch,
    HostMismatch,
    SlotMismatch,
    RouteMismatch,
    FrameMismatch,
    GenerationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleApplyResult {
    reason: StaleApplyReason,
}

impl StaleApplyResult {
    pub const fn reason(&self) -> StaleApplyReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionCommitErrorKind {
    ChangedCurrentBranch,
    PendingApply,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionCommitError {
    kind: TransactionCommitErrorKind,
}

impl TransactionCommitError {
    pub const fn kind(&self) -> TransactionCommitErrorKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionFailure {
    cleanup: Vec<FrameCleanupPlan>,
}

impl TransactionFailure {
    pub fn cleanup(&self) -> &[FrameCleanupPlan] {
        &self.cleanup
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationTransactionSnapshot {
    state: NavigationTransactionState,
    requests: Vec<FrameApplyRequest>,
    failure: Option<TransactionFailure>,
}

impl NavigationTransactionSnapshot {
    pub const fn state(&self) -> NavigationTransactionState {
        self.state
    }

    pub fn requests(&self) -> &[FrameApplyRequest] {
        &self.requests
    }

    pub fn failure(&self) -> Option<&TransactionFailure> {
        self.failure.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NavigationTransaction {
    id: NavigationTransactionId,
    plan: FrameReconciliationPlan,
    state: NavigationTransactionState,
    requests: Vec<FrameApplyRequest>,
    accepted: Vec<RouteFrameId>,
    failure: Option<TransactionFailure>,
}

impl NavigationTransaction {
    pub(crate) fn new(plan: FrameReconciliationPlan) -> Self {
        let id = NavigationTransactionId::next();
        let requests = plan
            .entering()
            .iter()
            .enumerate()
            .map(|(index, record)| FrameApplyRequest {
                token: FrameApplyToken::from_record(id, index as u64, record),
                frame: record.clone(),
            })
            .collect();
        Self {
            id,
            plan,
            state: NavigationTransactionState::PendingApply,
            requests,
            accepted: Vec::new(),
            failure: None,
        }
    }

    pub(crate) fn snapshot(&self) -> NavigationTransactionSnapshot {
        NavigationTransactionSnapshot {
            state: self.state,
            requests: self.requests.clone(),
            failure: self.failure.clone(),
        }
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted.len()
    }

    pub(crate) fn accept_apply_result(
        &mut self,
        token: &FrameApplyToken,
        outcome: FrameApplyOutcome,
    ) -> Result<(), StaleApplyResult> {
        let expected = self
            .requests
            .iter()
            .map(FrameApplyRequest::token)
            .find(|candidate| candidate.sequence == token.sequence)
            .ok_or(StaleApplyResult {
                reason: StaleApplyReason::SequenceMismatch,
            })?;
        check_token(expected, token)?;

        match outcome {
            FrameApplyOutcome::Applied => {
                self.accepted.push(token.frame());
                if self.accepted.len() == self.requests.len() {
                    self.state = NavigationTransactionState::ReadyToCommit;
                }
            }
            FrameApplyOutcome::Failed(_) => {
                self.state = NavigationTransactionState::Failed;
            }
        }
        Ok(())
    }

    pub(crate) fn commit(
        mut self,
        current_active: Option<&RouteFrameBranch>,
    ) -> Result<RouteFrameBranch, TransactionCommitError> {
        if self.state == NavigationTransactionState::Failed {
            return Err(TransactionCommitError {
                kind: TransactionCommitErrorKind::Failed,
            });
        }
        if self.state != NavigationTransactionState::ReadyToCommit {
            return Err(TransactionCommitError {
                kind: TransactionCommitErrorKind::PendingApply,
            });
        }
        let current_fingerprint = current_active.map(RouteFrameBranch::branch_fingerprint);
        if current_fingerprint != self.plan.baseline_fingerprint() {
            return Err(TransactionCommitError {
                kind: TransactionCommitErrorKind::ChangedCurrentBranch,
            });
        }
        let frames = self
            .plan
            .preserved()
            .iter()
            .cloned()
            .chain(
                self.plan
                    .entering()
                    .iter()
                    .map(|record| record.with_status(RouteFrameStatus::Active)),
            )
            .collect();
        self.state = NavigationTransactionState::Committed;
        Ok(RouteFrameBranch::new(
            frames,
            self.plan.target_fingerprint().clone(),
        ))
    }
}

fn check_token(expected: &FrameApplyToken, actual: &FrameApplyToken) -> Result<(), StaleApplyResult> {
    let reason = if expected.transaction != actual.transaction {
        Some(StaleApplyReason::TransactionMismatch)
    } else if expected.router != actual.router {
        Some(StaleApplyReason::RouterMismatch)
    } else if expected.host != actual.host {
        Some(StaleApplyReason::HostMismatch)
    } else if expected.slot != actual.slot {
        Some(StaleApplyReason::SlotMismatch)
    } else if expected.route != actual.route {
        Some(StaleApplyReason::RouteMismatch)
    } else if expected.frame != actual.frame {
        Some(StaleApplyReason::FrameMismatch)
    } else if expected.generation != actual.generation {
        Some(StaleApplyReason::GenerationMismatch)
    } else {
        None
    };

    match reason {
        Some(reason) => Err(StaleApplyResult { reason }),
        None => Ok(()),
    }
}

#[cfg(test)]
pub(crate) struct FakeApplyHarness;

#[cfg(test)]
pub(crate) struct FakePartialOutput {
    token: FrameApplyToken,
    frame: RouteFrameId,
}

#[cfg(test)]
impl FakePartialOutput {
    pub(crate) const fn frame(&self) -> RouteFrameId {
        self.frame
    }
}

#[cfg(test)]
impl FakeApplyHarness {
    pub(crate) fn build_success(token: &FrameApplyToken) -> FakePartialOutput {
        FakePartialOutput {
            token: token.clone(),
            frame: token.frame(),
        }
    }

    pub(crate) fn build_failure(
        token: &FrameApplyToken,
        error: RouteError,
    ) -> (FrameApplyToken, FrameApplyOutcome) {
        (token.clone(), FrameApplyOutcome::Failed(error))
    }

    pub(crate) fn apply_success(output: FakePartialOutput) -> (FrameApplyToken, FrameApplyOutcome) {
        (output.token, FrameApplyOutcome::Applied)
    }

    pub(crate) fn apply_failure(
        output: FakePartialOutput,
        error: RouteError,
    ) -> (FrameApplyToken, FrameApplyOutcome) {
        (output.token, FrameApplyOutcome::Failed(error))
    }
}
```

If `RouteFrameBranch::new`, `FrameAllocator::new`, or `FrameAllocator::allocate` are still private to `frame.rs`, adjust their visibility to `pub(crate)` so transaction tests can build fixtures without exposing them publicly.

- [ ] **Step 3: Run GREEN verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests -- --nocapture
cargo fmt --check
```

Expected: transaction tests pass with at least 3 nonzero tests; formatting passes.

- [ ] **Step 4: Commit**

```powershell
git add crates/bevy_ui_router_core/src/transaction.rs crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/lib.rs
git commit -m "feat(core): add navigation transaction stale gate"
```

### Task 4: Failure Cleanup And Error Boundary Targeting

**Files:**
- Modify: `crates/bevy_ui_router_core/src/transaction.rs`
- Modify: `crates/bevy_ui_router_core/src/error.rs`

**Interfaces:**
- Consumes: `FrameApplyOutcome::Failed`, `MatchedBranch::nearest_error_boundary_for`, `FrameCleanupPlan`.
- Produces: `TransactionFailure` with nearest boundary and cleanup plans.

- [ ] **Step 1: Add RED failure tests**

Add these tests inside `crates/bevy_ui_router_core/src/transaction.rs`:

```rust
#[test]
fn apply_failure_preserves_old_active_and_limits_cleanup_scope() {
    let (active, plan) = branches_with_error_boundary();
    let mut transaction = NavigationTransaction::new(plan);
    let token = transaction.snapshot().requests()[0].token().clone();

    transaction
        .accept_apply_result(
            &token,
            FrameApplyOutcome::Failed(RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "view apply failed",
            )),
        )
        .unwrap();

    assert_eq!(transaction.snapshot().state(), NavigationTransactionState::Failed);
    let failure = transaction.snapshot().failure().unwrap();
    assert_eq!(failure.failing_route(), token.route());
    assert_eq!(failure.cleanup().len(), 1);
    assert!(failure.cleanup()[0].scope().excludes_app_owned_host());
    assert_eq!(active.frames()[0].status(), RouteFrameStatus::Active);
}

#[test]
fn failure_prefers_nearest_nested_error_boundary() {
    let (_active, plan) = branches_with_error_boundary();
    let mut transaction = NavigationTransaction::new(plan);
    let token = transaction.snapshot().requests()[0].token().clone();

    transaction
        .accept_apply_result(
            &token,
            FrameApplyOutcome::Failed(RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "view apply failed",
            )),
        )
        .unwrap();

    assert_eq!(
        transaction
            .snapshot()
            .failure()
            .unwrap()
            .nearest_error_boundary()
            .map(ErrorBoundaryKey::as_str),
        Some("items")
    );
}

#[test]
fn failure_without_boundary_records_router_fallback() {
    let (_active, plan) = branches_without_error_boundary();
    let mut transaction = NavigationTransaction::new(plan);
    let token = transaction.snapshot().requests()[0].token().clone();

    transaction
        .accept_apply_result(
            &token,
            FrameApplyOutcome::Failed(RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "view apply failed",
            )),
        )
        .unwrap();

    assert!(transaction
        .snapshot()
        .failure()
        .unwrap()
        .nearest_error_boundary()
        .is_none());
}

#[test]
fn multi_frame_failure_cleans_only_attempted_transaction_output() {
    let (_active, plan) = branches_with_three_entering_frames();
    let mut transaction = NavigationTransaction::new(plan);
    let first = transaction.snapshot().requests()[0].token().clone();
    let second = transaction.snapshot().requests()[1].token().clone();

    transaction
        .accept_apply_result(&first, FrameApplyOutcome::Applied)
        .unwrap();
    transaction
        .accept_apply_result(
            &second,
            FrameApplyOutcome::Failed(RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                "view apply failed",
            )),
        )
        .unwrap();

    let cleanup = transaction.snapshot().failure().unwrap().cleanup();
    assert_eq!(cleanup.len(), 2);
    assert_eq!(cleanup[0].identity().frame(), first.frame());
    assert_eq!(cleanup[1].identity().frame(), second.frame());
    assert!(cleanup.iter().all(|plan| plan.scope().excludes_app_owned_host()));
}
```

Add a helper fixture in the same test module:

```rust
use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
use crate::frame::RouteFrameScope;
use crate::ids::{RouteHostId, RouteSlotId, RouterInstanceId};
use crate::meta::ErrorBoundaryKey;
use crate::route_tree::RouteNodeOptions;

fn branches_with_error_boundary() -> (RouteFrameBranch, FrameReconciliationPlan) {
    let mut builder = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap();
    let root = builder.root_id().unwrap();
    let section = builder
        .route_with_options(
            root,
            RoutePattern::path("section").unwrap(),
            RouteNodeOptions::new().with_error_boundary(ErrorBoundaryKey::named("section").unwrap()),
        )
        .unwrap();
    let items = builder
        .route_with_options(
            section,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new().with_error_boundary(ErrorBoundaryKey::named("items").unwrap()),
        )
        .unwrap();
    builder
        .route(items, RoutePattern::path(":id").unwrap())
        .unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());
    let old_branch = matcher
        .match_location(&RouteLocation::new("/section/items/1"))
        .unwrap();
    let new_branch = matcher
        .match_location(&RouteLocation::new("/section/items/2"))
        .unwrap();
    let diff = BranchDiff::compare(&old_branch, &new_branch);
    let mut allocator = FrameAllocator::new();
    let scope = RouteFrameScope::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
    );
    let active = RouteFrameBranch::from_matched_branch(
        scope,
        &old_branch,
        &mut allocator,
    );
    let plan = FrameReconciliationPlan::from_diff(
        scope,
        Some(&active),
        &new_branch,
        &diff,
        &mut allocator,
    )
        .with_error_boundaries_from(&new_branch);
    (active, plan)
}

fn branches_without_error_boundary() -> (RouteFrameBranch, FrameReconciliationPlan) {
    let mut builder = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap();
    let root = builder.root_id().unwrap();
    let items = builder
        .route(root, RoutePattern::path("items").unwrap())
        .unwrap();
    builder
        .route(items, RoutePattern::path(":id").unwrap())
        .unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());
    let old_branch = matcher
        .match_location(&RouteLocation::new("/items/1"))
        .unwrap();
    let new_branch = matcher
        .match_location(&RouteLocation::new("/items/2"))
        .unwrap();
    let diff = BranchDiff::compare(&old_branch, &new_branch);
    let mut allocator = FrameAllocator::new();
    let scope = RouteFrameScope::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
    );
    let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &mut allocator);
    let plan = FrameReconciliationPlan::from_diff(
        scope,
        Some(&active),
        &new_branch,
        &diff,
        &mut allocator,
    )
    .with_error_boundaries_from(&new_branch);
    (active, plan)
}

fn branches_with_three_entering_frames() -> (RouteFrameBranch, FrameReconciliationPlan) {
    let mut builder = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap();
    let root = builder.root_id().unwrap();
    let section = builder
        .route(root, RoutePattern::path("section").unwrap())
        .unwrap();
    let items = builder
        .route(section, RoutePattern::path("items").unwrap())
        .unwrap();
    builder
        .route(items, RoutePattern::path(":id").unwrap())
        .unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());
    let old_branch = matcher.match_location(&RouteLocation::new("/")).unwrap();
    let new_branch = matcher
        .match_location(&RouteLocation::new("/section/items/1"))
        .unwrap();
    let diff = BranchDiff::compare(&old_branch, &new_branch);
    let mut allocator = FrameAllocator::new();
    let scope = RouteFrameScope::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
    );
    let active = RouteFrameBranch::from_matched_branch(scope, &old_branch, &mut allocator);
    let plan = FrameReconciliationPlan::from_diff(
        scope,
        Some(&active),
        &new_branch,
        &diff,
        &mut allocator,
    )
    .with_error_boundaries_from(&new_branch);
    assert_eq!(plan.entering().len(), 3);
    (active, plan)
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests::apply_failure_preserves_old_active_and_limits_cleanup_scope -- --nocapture
```

Expected RED: compile failure because extended `TransactionFailure` fields, cleanup construction, and boundary attachment are not implemented.

- [ ] **Step 2: Implement failure records and boundary attachment**

Add to `FrameReconciliationPlan` in `frame.rs`:

```rust
use crate::meta::ErrorBoundaryKey;
use std::collections::BTreeMap;

error_boundaries: BTreeMap<RouteId, Option<ErrorBoundaryKey>>,

pub(crate) fn with_error_boundaries_from(mut self, branch: &MatchedBranch) -> Self {
    self.error_boundaries = branch
        .routes()
        .iter()
        .map(|route| {
            (
                route.route(),
                branch.nearest_error_boundary_for(route.route()).cloned(),
            )
        })
        .collect();
    self
}

pub(crate) fn nearest_error_boundary_for(&self, route: RouteId) -> Option<&ErrorBoundaryKey> {
    self.error_boundaries
        .get(&route)
        .and_then(Option::as_ref)
}
```

Initialize `error_boundaries` to `BTreeMap::new()` in `FrameReconciliationPlan::from_diff`.

Replace the provisional Task 3 `TransactionFailure` in `transaction.rs` with:

```rust
use crate::frame::FrameCleanupPlan;
use crate::meta::ErrorBoundaryKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionFailure {
    failing_route: RouteId,
    error: RouteError,
    nearest_error_boundary: Option<ErrorBoundaryKey>,
    cleanup: Vec<FrameCleanupPlan>,
}

impl TransactionFailure {
    pub const fn failing_route(&self) -> RouteId {
        self.failing_route
    }

    pub fn error(&self) -> &RouteError {
        &self.error
    }

    pub fn nearest_error_boundary(&self) -> Option<&ErrorBoundaryKey> {
        self.nearest_error_boundary.as_ref()
    }

    pub fn cleanup(&self) -> &[FrameCleanupPlan] {
        &self.cleanup
    }
}
```

Keep `failure: Option<TransactionFailure>` on core-internal `NavigationTransaction`, initialize it to `None`, and expose it only through `NavigationTransactionSnapshot::failure()`.

In `FrameApplyOutcome::Failed(error)` handling:

```rust
let failed_record = self
    .plan
    .entering()
    .iter()
    .find(|record| record.identity().frame() == token.frame())
    .expect("token was checked against entering records");
let mut cleanup = self
    .accepted
    .iter()
    .filter_map(|frame| {
        self.plan
            .entering()
            .iter()
            .find(|record| record.identity().frame() == *frame)
    })
    .map(|record| FrameCleanupPlan::router_owned_partial_output(*record.identity()))
    .collect::<Vec<_>>();
cleanup.push(FrameCleanupPlan::router_owned_partial_output(*failed_record.identity()));
self.failure = Some(TransactionFailure {
    failing_route: token.route(),
    nearest_error_boundary: self
        .plan
        .nearest_error_boundary_for(token.route())
        .cloned(),
    cleanup,
    error,
});
self.state = NavigationTransactionState::Failed;
```

Cleanup must include only frames whose apply was accepted by this transaction plus the failed frame being handled. It must not include unattempted staged frames, preserved active frames, leaving frames, or the app-owned host.

- [ ] **Step 3: Add crate-visible error constructors if required**

If tests or implementation need constructor helpers, add these to `RouteError` in `crates/bevy_ui_router_core/src/error.rs`:

```rust
pub(crate) fn stale_result(redacted_message: impl Into<String>) -> Self {
    Self::new(
        RouteErrorKind::StaleResult,
        RouteErrorSourceCategory::RouterCore,
        redacted_message,
    )
}

pub(crate) fn view_apply_failed(redacted_message: impl Into<String>) -> Self {
    Self::new(
        RouteErrorKind::ViewApplyFailed,
        RouteErrorSourceCategory::RendererContract,
        redacted_message,
    )
}

pub(crate) fn cleanup_failed(redacted_message: impl Into<String>) -> Self {
    Self::new(
        RouteErrorKind::CleanupFailed,
        RouteErrorSourceCategory::RouterCore,
        redacted_message,
    )
}

pub(crate) fn finalize_failed(redacted_message: impl Into<String>) -> Self {
    Self::new(
        RouteErrorKind::FinalizeFailed,
        RouteErrorSourceCategory::RouterCore,
        redacted_message,
    )
}
```

Do not add app-specific error variants.

- [ ] **Step 4: Run GREEN verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core frame::tests -- --nocapture
cargo fmt --check
```

Expected: transaction and frame tests pass with nonzero test counts; formatting passes.

- [ ] **Step 5: Commit**

```powershell
git add crates/bevy_ui_router_core/src/transaction.rs crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/error.rs
git commit -m "feat(core): record transaction failure cleanup"
```

### Task 5: Frame Lifecycle Diagnostics

**Files:**
- Modify: `crates/bevy_ui_router_core/src/diagnostics.rs`

**Interfaces:**
- Consumes: `RouteFrameIdentity`, `RouteFrameStatus`.
- Produces: `FrameLifecycleDiagnostic` and internal event constructor.

- [ ] **Step 1: Add RED diagnostic tests**

Add this test inside `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
#[test]
fn frame_lifecycle_diagnostic_records_identity_and_status() {
    let identity = RouteFrameIdentity::new(
        RouterInstanceId::new(1),
        RouteHostId::new(2),
        RouteSlotId::new(3),
        RouteId::new(4),
        RouteFrameId::new(5),
        RouteGeneration::initial(),
    );
    let payload = FrameLifecycleDiagnostic::new(identity, RouteFrameStatus::ApplyingView);
    let event = DiagnosticEvent::frame_lifecycle(payload.clone());

    assert_eq!(event.kind(), DiagnosticEventKind::FrameLifecycle);
    assert_eq!(event.identity().map(DiagnosticIdentity::frame), Some(RouteFrameId::new(5)));
    assert_eq!(event.frame_lifecycle(), Some(&payload));
    assert_eq!(event.redacted_message(), "frame lifecycle changed");
    assert!(!event.redacted_message().contains("/"));
}
```

Add imports inside the test module:

```rust
use crate::frame::{RouteFrameIdentity, RouteFrameStatus};
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core diagnostics::tests::frame_lifecycle_diagnostic_records_identity_and_status -- --nocapture
```

Expected RED: compile failure because frame lifecycle diagnostics are not implemented.

- [ ] **Step 2: Implement frame lifecycle diagnostics**

Add to `diagnostics.rs`:

```rust
use crate::frame::{RouteFrameIdentity, RouteFrameStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameLifecycleDiagnostic {
    identity: RouteFrameIdentity,
    status: RouteFrameStatus,
}

impl FrameLifecycleDiagnostic {
    pub(crate) const fn new(identity: RouteFrameIdentity, status: RouteFrameStatus) -> Self {
        Self { identity, status }
    }

    pub const fn identity(&self) -> &RouteFrameIdentity {
        &self.identity
    }

    pub const fn status(&self) -> RouteFrameStatus {
        self.status
    }
}
```

Add `FrameLifecycle` to `DiagnosticEventKind`, add `frame_lifecycle: Option<FrameLifecycleDiagnostic>` to `DiagnosticEvent`, set it to `None` in every existing constructor, and add:

```rust
pub(crate) fn frame_lifecycle(frame_lifecycle: FrameLifecycleDiagnostic) -> Self {
    let identity = frame_lifecycle.identity();
    Self {
        kind: DiagnosticEventKind::FrameLifecycle,
        identity: Some(DiagnosticIdentity::new(
            identity.router(),
            identity.host(),
            identity.slot(),
            identity.frame(),
            identity.generation(),
            Some(identity.route()),
        )),
        branch_planning: None,
        frame_lifecycle: Some(frame_lifecycle),
        redacted_message: "frame lifecycle changed".to_owned(),
    }
}

pub fn frame_lifecycle(&self) -> Option<&FrameLifecycleDiagnostic> {
    self.frame_lifecycle.as_ref()
}
```

The constructor stays crate-visible so public adapters can observe lifecycle diagnostics but cannot author them as route-lifecycle authority.

- [ ] **Step 3: Run GREEN verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core diagnostics::tests -- --nocapture
cargo fmt --check
```

Expected: diagnostics tests pass with nonzero count; formatting passes.

- [ ] **Step 4: Commit**

```powershell
git add crates/bevy_ui_router_core/src/diagnostics.rs
git commit -m "feat(core): add frame lifecycle diagnostics"
```

### Task 6: Core Authority Boundary Coverage

**Files:**
- Modify: `crates/bevy_ui_router_core/src/frame.rs`
- Modify: `crates/bevy_ui_router_core/src/transaction.rs`
- Modify: `crates/bevy_ui_router_core/src/diagnostics.rs`
- Modify: `crates/bevy_ui_router_core/src/ids.rs`
- Modify: `crates/bevy_ui_router_core/src/ownership.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Modify: `crates/bevy_ui_router_core/tests/public_api.rs`

**Interfaces:**
- Consumes: Phase 2 public module APIs and internal authority types.
- Produces: compile-fail authority boundaries and positive public observation coverage.

- [ ] **Step 1: Add compile-fail doctests for authority boundaries**

Add to the top of `crates/bevy_ui_router_core/src/frame.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     RouteFrameId, RouteFrameIdentity, RouteGeneration, RouteHostId, RouteId,
//!     RouteSlotId, RouterInstanceId,
//! };
//!
//! fn forge(
//!     router: RouterInstanceId,
//!     host: RouteHostId,
//!     slot: RouteSlotId,
//!     route: RouteId,
//!     frame: RouteFrameId,
//!     generation: RouteGeneration,
//! ) {
//!     let _ = RouteFrameIdentity::new(router, host, slot, route, frame, generation);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::frame::FrameAllocator;
//!
//! let _ = FrameAllocator::new();
//! ```
```

Add to the top of `crates/bevy_ui_router_core/src/transaction.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::transaction::NavigationTransaction;
//!
//! let _ = core::mem::size_of::<NavigationTransaction>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::transaction::NavigationTransactionId;
//!
//! let _ = core::mem::size_of::<NavigationTransactionId>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::FrameApplyToken;
//!
//! let _ = FrameApplyToken {
//!     transaction: unreachable!(),
//!     sequence: 0,
//!     router: unreachable!(),
//!     host: unreachable!(),
//!     slot: unreachable!(),
//!     route: unreachable!(),
//!     frame: unreachable!(),
//!     generation: unreachable!(),
//! };
//! ```
```

The token snippet must fail because fields are private, not because the public type is missing. If field names change, update the snippet to use the actual private fields.

Add to the top of `crates/bevy_ui_router_core/src/ids.rs` if equivalent coverage is not already present:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteGeneration;
//!
//! let generation = RouteGeneration::initial();
//! let _ = generation.next();
//! ```
```

This makes generation authority explicit in the Phase 2 boundary task even though `ids.rs` already has earlier compile-fail coverage for private id constructors.

Add to the top of `crates/bevy_ui_router_core/src/ownership.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     CleanupScope, RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId,
//!     RouterInstanceId,
//! };
//!
//! fn forge(
//!     router: RouterInstanceId,
//!     host: RouteHostId,
//!     slot: RouteSlotId,
//!     frame: RouteFrameId,
//!     generation: RouteGeneration,
//! ) {
//!     let _ = CleanupScope::router_owned_subtree(router, host, slot, frame, generation);
//! }
//! ```
```

Add to the top of `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::diagnostics::FrameLifecycleDiagnostic;
//! use bevy_ui_router_core::prelude::{RouteFrameIdentity, RouteFrameStatus};
//!
//! fn forge(identity: RouteFrameIdentity) {
//!     let _ = FrameLifecycleDiagnostic::new(identity, RouteFrameStatus::Active);
//! }
//! ```
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --doc
```

Expected: doctests pass with nonzero compile-fail coverage for frame identity, frame allocator, generation advancement, navigation transaction, transaction id, apply token fields, cleanup scope, and lifecycle diagnostic constructor.

- [ ] **Step 2: Add positive public API tests**

Append to `crates/bevy_ui_router_core/tests/public_api.rs`:

```rust
#[test]
fn phase_2_frame_transaction_types_are_public_observation_api() {
    use bevy_ui_router_core::diagnostics::FrameLifecycleDiagnostic;
    use bevy_ui_router_core::frame::{
        FrameCleanupPlan, FrameReconciliationPlan, RouteFrameBranch, RouteFrameIdentity,
        RouteFrameRecord, RouteFrameScope, RouteFrameStatus,
    };
    use bevy_ui_router_core::transaction::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, NavigationTransactionSnapshot,
        NavigationTransactionState, StaleApplyReason, StaleApplyResult, TransactionCommitError,
        TransactionCommitErrorKind, TransactionFailure,
    };

    fn assert_public<T>() {}
    assert_public::<FrameCleanupPlan>();
    assert_public::<FrameReconciliationPlan>();
    assert_public::<RouteFrameBranch>();
    assert_public::<RouteFrameIdentity>();
    assert_public::<RouteFrameRecord>();
    assert_public::<RouteFrameScope>();
    assert_public::<FrameLifecycleDiagnostic>();
    assert_public::<FrameApplyOutcome>();
    assert_public::<FrameApplyRequest>();
    assert_public::<FrameApplyToken>();
    assert_public::<NavigationTransactionSnapshot>();
    assert_public::<NavigationTransactionState>();
    assert_public::<StaleApplyReason>();
    assert_public::<StaleApplyResult>();
    assert_public::<TransactionCommitError>();
    assert_public::<TransactionCommitErrorKind>();
    assert_public::<TransactionFailure>();
    assert_eq!(RouteFrameStatus::ApplyingView, RouteFrameStatus::ApplyingView);
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api phase_2_frame_transaction_types_are_public_observation_api -- --nocapture
```

Expected: one nonzero integration test passes without importing `NavigationTransaction`, `NavigationTransactionId`, `FrameAllocator`, or constructor-only authority.

- [ ] **Step 3: Run core authority boundary verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api -- --nocapture
cargo fmt --check
```

Expected: doctests and public API tests pass with intended nonzero counts; formatting passes.

- [ ] **Step 4: Commit**

```powershell
git add crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/transaction.rs crates/bevy_ui_router_core/src/diagnostics.rs crates/bevy_ui_router_core/src/ids.rs crates/bevy_ui_router_core/src/ownership.rs crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/tests/public_api.rs
git commit -m "test(core): lock frame transaction authority boundaries"
```

### Task 7: Facade And Dependency Boundary Coverage

**Files:**
- Modify: `crates/bevy_ui_router_core/tests/dependency_boundary.rs`
- Modify: `crates/bevy_ui_router/tests/facade_features.rs`

**Interfaces:**
- Consumes: Phase 2 public observation contracts already exposed by core.
- Produces: dependency guard and facade no-default import coverage.

- [ ] **Step 1: Add dependency boundary guard**

Append to `crates/bevy_ui_router_core/tests/dependency_boundary.rs`:

```rust
#[test]
fn phase_2_frame_transaction_manifest_stays_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "Phase 2 frame and transaction work must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary -- --nocapture
```

Expected: dependency-boundary tests pass with nonzero count.

- [ ] **Step 2: Add facade prelude test**

Append to `crates/bevy_ui_router/tests/facade_features.rs`:

```rust
#[test]
fn facade_prelude_exposes_phase_2_observation_contracts() {
    use bevy_ui_router::prelude::{
        FrameApplyOutcome, FrameApplyRequest, FrameApplyToken, FrameCleanupPlan,
        FrameReconciliationPlan, NavigationTransactionSnapshot, NavigationTransactionState,
        RouteFrameBranch, RouteFrameIdentity, RouteFrameRecord, RouteFrameScope,
        RouteFrameStatus, StaleApplyReason, StaleApplyResult, TransactionCommitError,
        TransactionCommitErrorKind, TransactionFailure,
    };

    fn assert_public<T>() {}
    assert_public::<FrameApplyOutcome>();
    assert_public::<FrameApplyRequest>();
    assert_public::<FrameApplyToken>();
    assert_public::<FrameCleanupPlan>();
    assert_public::<FrameReconciliationPlan>();
    assert_public::<NavigationTransactionSnapshot>();
    assert_public::<NavigationTransactionState>();
    assert_public::<RouteFrameBranch>();
    assert_public::<RouteFrameIdentity>();
    assert_public::<RouteFrameRecord>();
    assert_public::<RouteFrameScope>();
    assert_public::<StaleApplyReason>();
    assert_public::<StaleApplyResult>();
    assert_public::<TransactionCommitError>();
    assert_public::<TransactionCommitErrorKind>();
    assert_public::<TransactionFailure>();
    assert_eq!(RouteFrameStatus::Active, RouteFrameStatus::Active);
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features facade_prelude_exposes_phase_2_observation_contracts -- --nocapture
```

Expected: one nonzero facade test passes and the facade does not expose `NavigationTransaction`.

- [ ] **Step 3: Run facade and dependency boundary verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features
cargo fmt --check
```

Expected: all commands pass with intended nonzero integration counts where applicable.

- [ ] **Step 4: Commit**

```powershell
git add crates/bevy_ui_router_core/tests/dependency_boundary.rs crates/bevy_ui_router/tests/facade_features.rs
git commit -m "test: expose frame transaction facade contracts"
```

### Task 8: Final Phase 2 Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md`

**Interfaces:**
- Consumes: all Phase 2 changes.
- Produces: final verification evidence and Phase 3A handoff.

- [ ] **Step 1: Run final verification**

Run:

```powershell
cargo fmt --all --check
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features
```

Expected: every command exits 0. Record exact nonzero test counts for unit, integration, facade, and doctest commands.

- [ ] **Step 2: Confirm Phase 3A handoff conditions**

Run:

```powershell
rg -n "RouteFrameStatus|NavigationTransaction|FrameApplyToken|StaleApplyResult|TransactionFailure" crates\bevy_ui_router_core\src
git status --short
```

Expected: frame/transaction contracts exist and the worktree has only intentional Phase 2 documentation changes before final commit.

- [ ] **Step 3: Write verification record**

Append a `Phase 2 Verification Record` section to this plan with:

- final implementation commits;
- command outputs summarized with exact pass counts;
- whole-branch review result;
- unresolved risks;
- Phase 3A handoff statement.

- [ ] **Step 4: Commit verification record**

```powershell
git add docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md
git commit -m "docs: record phase 2 frame transaction verification"
```

---

## 8. Review Gate Before Implementation

Before implementing Phase 2, review this plan with independent agents using these responsibility directions:

- Architecture and lifecycle review: verify Phase 2 separates requested, matched, pending, and active states, and does not leak renderer/ECS/app assumptions into core.
- API and authority review: verify adapters cannot forge frame identity, generation, transaction id, apply token, cleanup scope, or commit authority.
- Fact and source review: verify claims against current source, Phase 1B verification evidence, AGENTS.md, and roadmap docs.
- Test and risk review: verify every Phase 2 semantic requirement has executable coverage and every command is meaningful for touched areas.

The lead agent must evaluate findings before accepting them. If multiple findings share a root cause such as lifecycle leakage, public authority overexposure, or insufficient stale-check identity, revise the plan architecture instead of applying isolated edits.

---

## 9. Acceptance Criteria

- The plan starts only after Phase 1B is complete, reviewed, and verified.
- Core remains dependency-free and does not import Bevy, BSN, platform, app service, theme, permission, or shell APIs.
- Frame identity records router, host, slot, route, frame id, and generation.
- Frame identity and generation are observable but not externally constructible.
- Common-prefix frame records preserve frame id and generation.
- Changed suffix frames leave leaf-to-root and enter root-to-leaf.
- Search or provenance fail-closed branch diff replaces the full frame branch.
- Pending transaction state is distinct from active rendered branch.
- Apply tokens carry router, host, slot, route, frame, generation, transaction identity, and sequence.
- Stale apply results are discarded before visible state changes.
- Commit rechecks the current active branch baseline before producing a new active branch.
- Failed transaction preserves the old active branch.
- Failed transaction cleanup plans target only router-owned partial output and explicitly exclude app-owned host.
- Error-boundary failure records use nearest matched ancestor boundary metadata.
- Phase 2 records fallback intent but does not render fallback UI.
- Frame lifecycle diagnostics use structured payloads and cannot be authored by external adapters as lifecycle authority.
- Public module/prelude/facade coverage proves observation APIs are available.
- Compile-fail doctests prove frame, generation, transaction, token, cleanup-scope, and lifecycle-diagnostic authority cannot be forged through public APIs.
- Dependency-boundary tests prove core remains dependency-free.
- Final verification record lists commands, pass counts, review result, and Phase 3A handoff.

---

## 10. Phase 2 Verification Record

Recorded on 2026-07-08 after implementation and whole-branch review.

Final Phase 2 evidence commit: `bd9c625` `docs: record phase 2 frame transaction verification`.

### Implementation Commits

- `fbb99ca` `feat(core): add route frame records`
- `d4b129b` `fix(core): add frame status cloning`
- `3d03937` `feat(core): plan frame reconciliation`
- `7f2606b` `fix(core): fail closed on stale frame diffs`
- `63fff7f` `feat(core): add navigation transaction stale gate`
- `97b6105` `fix(core): make transaction failure cleanup precise`
- `43d7f54` `feat(core): record transaction failure cleanup`
- `df5baca` `fix(core): populate plan error boundaries in constructors`
- `0b400ed` `feat(core): add frame lifecycle diagnostics`
- `3a7492c` `fix(core): expose frame lifecycle diagnostics as inherent API`
- `50803ec` `docs: record phase 2 task 5 review fix`
- `b9add4f` `test(core): lock frame transaction authority boundaries`
- `66925db` `test: expose frame transaction facade contracts`
- `676778a` `fix: expose frame planning observations through prelude`
- `964321c` `fix(core): recheck frame baseline before commit`
- `c9d17d8` `docs: update phase 2 final review fix report`

### Final Verification Commands

- `cargo fmt --all --check`
  - Result: passed with exit code 0.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core`
  - Result: passed with exit code 0.
  - Counts: 87 unit tests, 5 `dependency_boundary` integration tests, 2 `public_api` integration tests, and 28 doctests passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --doc`
  - Result: passed with exit code 0.
  - Count: 28 doctests passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api`
  - Result: passed with exit code 0.
  - Count: 2 integration tests passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary`
  - Result: passed with exit code 0.
  - Count: 5 integration tests passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features`
  - Result: passed with exit code 0.
  - Counts: 5 `facade_features` integration tests passed; facade lib and doctest harnesses each ran 0 tests.

### Handoff Checks

- `rg -n "RouteFrameStatus|NavigationTransaction|FrameApplyToken|StaleApplyResult|TransactionFailure" crates\bevy_ui_router_core\src`
  - Result: matched the Phase 2 frame, transaction, diagnostics, and prelude contracts in core source.
- `git status --short`
  - Result before writing this record: no output after removing Cargo's generated root `Cargo.lock` verification artifact.

### Whole-Branch Review Result

- Architecture/API/lifecycle review initially returned `With fixes` because commit rechecked the active branch baseline only by `RouteBranchFingerprint`.
- The blocker was fixed by `964321c`, which stores a private active-branch baseline snapshot and compares branch fingerprint plus frame count, router, host, slot, route, frame id, generation, and branch index before commit can publish a new active branch.
- Architecture/API/lifecycle re-review reported `Critical fixed: Yes` and `Ready to proceed: Yes`.
- Initial fact/evidence review had a `With fixes` finding because this final verification record and final full-phase pass counts were not yet present in the reviewed range. This section records the missing evidence.
- Fact/evidence re-review is now reconciled as PASS with the ledger update applied; the missing final evidence is present in this record.

### Unresolved Risks

- Public API and facade observation coverage proves type exposure and authority boundaries, but remains mostly import/type-smoke coverage rather than behavioral observation coverage.
- `RouteError` crate-visible helper constructors described in Task 4 were conditional on implementation need. The implementation uses `RouteError::new` directly, so unused helpers were not added.
- Hidden/retained subtree input isolation and focus release remain future work because Phase 2 does not implement retained subtrees or Bevy ECS mutation.

### Next Phase Authorization

Phase 2 gate PASS. Phase 3A planning is authorized because final verification commands exited 0, whole-branch review blockers were fixed, fact/evidence review is reconciled, and unresolved risks are documented above.

### Phase 3A Handoff

Phase 2 leaves a core-only frame and transaction contract ready for the next planning phase. Phase 3A can build on:

- observable frame scope, identity, status, branch, reconciliation plan, cleanup plan, and lifecycle diagnostics;
- internal navigation transaction authority with apply tokens, stale token checks, failure records, router-owned cleanup, nearest error-boundary metadata, and active-baseline commit recheck;
- compile-fail coverage for non-forgeable frame, generation, transaction, token, cleanup, and diagnostic authority;
- public module, core prelude, and facade prelude observation coverage without exposing `NavigationTransaction` or other commit authority.
