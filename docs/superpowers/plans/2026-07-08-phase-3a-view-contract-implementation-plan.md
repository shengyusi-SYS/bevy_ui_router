# Phase 3A View Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a renderer-agnostic view contract and feature-gated fake renderer without adding public constructors, mutation paths, or commit paths that would let adapters forge core lifecycle authority.

**Architecture:** Phase 3A defines the renderer-side lifecycle boundary. Default prelude/facade exposure stays limited to user-facing view binding declarations, while explicit crate paths expose observable build/apply records whose constructors remain crate-private. `ViewRequestId`, `ViewOutputId`, `ViewApplyOutcome::Applied`, and `ViewPartialOutput` are renderer-local, pre-finalization observations; Phase 3B must pair them with a core-issued `FrameApplyRequest` and submit the result back to core so core-owned stale checks remain authoritative. The fake renderer lives behind `test-support`; it proves build success, build failure, apply success, apply failure with partial output, stale build result, stale apply result, deterministic correlation ids, and fake-issued plan provenance without creating active UI or mutating ECS.

**Tech Stack:** Rust 2024, existing Cargo workspace, `bevy_ui_router_view` depending only on `bevy_ui_router_core`, feature-gated facade re-exports, `cargo fmt`, `cargo test`, dependency-boundary tests.

## Global Constraints

- Phase 3A can execute only after Phase 2 implementation, whole-branch review, final verification, and documentation commit `bd9c625` or later equivalent evidence.
- Router core remains renderer-agnostic and must not depend on Bevy ECS, BSN, platform APIs, application services, themes, permissions, data services, or application shell models.
- `bevy_ui_router_view` may depend on `bevy_ui_router_core`; it must not depend on Bevy ECS, BSN, platform crates, facade crate, or app profile concepts.
- `bevy_ui_router_core` dependency sections must remain empty.
- View contracts must not expose `NavigationTransaction`, `NavigationTransactionId`, `FrameAllocator`, `FrameApplyToken` construction, `RouteFrameIdentity` construction, `RouteGeneration::next`, cleanup-scope construction, lifecycle diagnostic construction, or commit authority.
- Existing Phase 2 public observation types such as `FrameApplyToken`, `RouteFrameIdentity`, `RouteGeneration`, `FrameCleanupPlan`, and lifecycle snapshots may remain visible through core and facade preludes; Phase 3A must not add new ways to construct, mutate, accept, finalize, or commit them.
- View request ids and output ids are renderer-local correlation ids only. They are not router identity, frame identity, generation, task identity, stale-check authority, or commit authority.
- View request ids, output ids, build requests, render plans, partial outputs, and view failures are observable protocol records in Phase 3A; their constructors must remain crate-private.
- Phase 3A fake/test-support code may mint renderer-local observation records inside the view crate. Non-fake adapters in later phases must report renderer results through a controlled reporter/session capability minted by the core/ECS bridge from the core-issued `FrameApplyRequest`; they must not require raw protocol constructors to become public.
- Build output is not partial UI. `ViewPartialOutput` can only be created by apply-path code, including the feature-gated fake renderer.
- Stale renderer results never carry `ViewPartialOutput`; only apply failure may report partial output for router-owned cleanup planning.
- `ViewApplyOutcome::Applied` and `ViewPartialOutput` are strictly pre-finalization and pre-commit observations. They cannot create active UI unless a later trusted bridge pairs them with the exact core-issued `FrameApplyRequest`, translates them into a core apply outcome, and submits that outcome back to core for the final stale check.
- Phase 3A does not create Bevy entities, mutate ECS hierarchy, apply BSN scenes, hide retained subtrees, release focus, run async tasks, or call platform adapters.
- Fake renderer output is test support for view build/apply semantics; it is not active UI.
- Active UI still comes only from core-owned transaction commit after identity checks; Phase 3A must not introduce an alternate active-frame path.
- View build, apply, and stale failures map to `RouteErrorSourceCategory::RendererContract`.
- View build failures use `RouteErrorKind::ViewBuildFailed`; view apply failures use `RouteErrorKind::ViewApplyFailed`; stale renderer results use `RouteErrorKind::StaleResult`.
- Public module coverage may expose stable view protocol records by explicit path. Default view/facade preludes expose only `ViewBindingKey` and `RouteViewDescriptor`; protocol observation records stay out of default preludes.
- Public module/prelude/facade coverage must not expose fake renderer unless the explicit test-support feature is enabled.
- Compile-fail boundary evidence that gates implementation must assert stderr fragments naming the intended missing/private API. Rustdoc `compile_fail` examples are allowed only as supplemental smoke coverage.
- Every filtered GREEN or final `cargo test` command is invalid evidence if no intended target harness runs nonzero tests.
- RED compile-fail evidence is valid only when the failure names the intentionally missing or private API for that task, not when the failure is a missing module, wrong feature, missing dependency, or wrong test target.
- Use `cargo test --config "build.rustc-wrapper = ''" ...` and `cargo check --config "build.rustc-wrapper = ''" ...` for Cargo verification in this workspace.

---

## File Structure

- Modify `crates/bevy_ui_router_view/Cargo.toml`
  - Adds `default = []` and `test-support = []`.
- Create `crates/bevy_ui_router_view/src/contract.rs`
  - Owns renderer-neutral view binding, descriptor, renderer-local request/output correlation ids, build request, render plan, partial output, disposal hint, failure report, build outcome, and apply outcome.
- Create `crates/bevy_ui_router_view/src/testing.rs`
  - Owns feature-gated `FakeViewRenderer` and `FakeViewRendererMode`.
- Create `crates/bevy_ui_router_view/tests/compile_fail_boundary.rs`
  - Runs temporary crates and asserts stderr fragments for private constructors, narrow prelude exposure, fake-renderer root non-exposure, and missing commit/finalize/accept authority.
- Create `crates/bevy_ui_router_view/tests/authority_surface_boundary.rs`
  - Audits public source surface for active commit, accept, finalize, activate, or frame-apply conversion verbs until a later controlled reporter/session API is deliberately introduced.
- Modify `crates/bevy_ui_router_view/src/lib.rs`
  - Re-exports stable contract types from crate root; default prelude exposes only `ViewBindingKey` and `RouteViewDescriptor`; re-exports fake renderer only under `test-support`.
- Create `crates/bevy_ui_router_view/tests/dependency_boundary.rs`
  - Proves the view crate depends only on core, rejects Bevy/BSN package-renaming and dependency-subtable drift, and keeps the fake renderer feature dependency-free.
- Modify `crates/bevy_ui_router/Cargo.toml`
  - Adds `view-test-support` facade feature and keeps the view dependency default-features disabled.
- Modify `crates/bevy_ui_router/src/lib.rs`
  - Adds a default-feature compile-fail guard proving fake renderer is not in the facade prelude without `view-test-support`.
- Modify `crates/bevy_ui_router/tests/facade_features.rs`
  - Proves default facade prelude exposes only user-facing view binding declarations, explicit facade view paths expose protocol observations, no-default remains core-only, and fake renderer exposure is explicit.
- Create `crates/bevy_ui_router/tests/facade_compile_fail_boundary.rs`
  - Runs temporary crates and asserts stderr fragments for facade default/test-support negative authority and exposure boundaries.
- Create `crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs`
  - Audits facade source surface for accidental active commit, accept, finalize, activate, or frame-apply conversion APIs.
- Modify `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md`
  - Updates Phase 3A plan references to this file.
- Modify `docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md`
  - Normalizes Phase 2 final evidence fields required by the execution-flow gate model.
- Modify `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
  - Aligns Phase 3A/3B boundary wording so active commit ordering belongs to Phase 3B ECS/core integration.
- Modify `docs/superpowers/specs/2026-07-07-project-decisions-design.md`
  - Aligns Phase 3 aggregate decision wording with the Phase 3A/3B split.
- Modify `docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md`
  - Records final Phase 3A verification after implementation.

---

## Lifecycle Boundary And Phase 3B Handoff

Phase 3A freezes the renderer observation contract only. It does not finalize ECS output and does not commit active UI.

Canonical Phase 3B handoff:

1. Core owns the navigation transaction and issues a `FrameApplyRequest` for the frame that may receive renderer output.
2. The ECS bridge may ask the view layer to build and apply a `ViewRenderPlan`, producing `ViewApplyOutcome`.
3. Non-fake adapters may report renderer success, failure, partial output, and disposal hints only through a controlled reporter/session capability minted by the ECS bridge from the `FrameApplyRequest`; raw protocol constructors remain crate-private.
4. `ViewApplyOutcome::Applied` and `ViewPartialOutput` remain renderer observations until the ECS bridge finalizes the router-owned subtree.
5. The ECS bridge must translate the finalized renderer result into core's `FrameApplyOutcome` paired with the exact `FrameApplyToken` from the `FrameApplyRequest`.
6. Core remains the only authority that accepts or rejects that apply result. Phase 3B must use a public or trusted core API that performs the transaction/token stale check inside core; it must not reconstruct the check from public token fields.
7. Active UI may change only after core accepts the apply result and commits the transaction. A stale or failed result must not create visible active state.

If Phase 3B needs a public bridge or reporter API that does not yet exist, it must add that API with token/request construction still controlled by core and with an explicit allow-list replacing the Phase 3A authority-surface deny-list. It must not move transaction identity, generation advancement, stale acceptance, or active-branch commit authority into view, ECS, BSN, facade, or adapter code.

---

### Task 0: Phase 3A Entry Gate And Flow Reference

**Files:**
- Read: `docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md`
- Read: `.superpowers/sdd/progress.md`
- Modify: `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md`

**Interfaces:**
- Consumes: Phase 2 verification record and Phase 0-3 flow document.
- Produces: go/no-go evidence for Task 1 and corrected Phase 3A plan path.

- [ ] **Step 1: Confirm Phase 2 completion evidence**

Run:

```powershell
git log --oneline --grep "record phase 2 frame transaction verification" --all
git status --short
Select-String -LiteralPath 'docs\superpowers\plans\2026-07-07-phase-2-frame-error-transaction-implementation-plan.md' -Pattern 'Phase 2 Verification Record','Final Phase 2 evidence commit','exit code 0','Architecture/API/lifecycle re-review reported','Fact/evidence re-review is now reconciled as PASS','Phase 2 gate PASS'
```

Expected:

- `bd9c625 docs: record phase 2 frame transaction verification` or later equivalent evidence appears.
- `git status --short` has no unrelated source changes. Intentional in-flight planning document changes are allowed during this planning gate.
- The Phase 2 plan contains a `Phase 2 Verification Record`, records final evidence commit SHA, records verification exit code 0 evidence, records architecture re-review outcome, records reconciled fact/evidence PASS evidence, and explicitly authorizes Phase 3A.
- If Cargo verification generates an untracked root `Cargo.lock`, remove that generated root file before recording final gate status and note the cleanup in Step 4 evidence. Do not remove tracked lockfiles or source artifacts.

- [ ] **Step 2: Run Phase 2 smoke verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests::commit_rejects_remounted_current_active_branch_with_same_fingerprint -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features facade_prelude_exposes_phase_2_observation_contracts -- --nocapture
```

Expected:

- The transaction stale-baseline regression runs 1 test and passes.
- The facade Phase 2 prelude test runs 1 test and passes.

- [ ] **Step 3: Confirm Phase 0-3 flow reference and scope wording**

Confirm every Phase 3A plan reference in `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md` points to:

```text
docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md
```

Also confirm Phase 3A wording in that flow document describes renderer contract, failure cleanup reporting, stale-result modeling, and negative authority boundaries; it must not require Phase 3A to prove active commit ordering.

Run:

```powershell
$flowPath = 'docs\superpowers\plans\2026-07-07-phase-0-3-execution-flow.md'
Select-String -LiteralPath $flowPath -Pattern 'phase-3a-view-contract-implementation-plan'
$flow = Get-Content -LiteralPath $flowPath
$startMatch = Select-String -LiteralPath $flowPath -Pattern '^### Task 7A:' | Select-Object -First 1
$endMatch = Select-String -LiteralPath $flowPath -Pattern '^### Task 7B:' | Select-Object -First 1
if ($null -eq $startMatch -or $null -eq $endMatch) { throw 'Phase 3A flow anchors not found' }
$start = $startMatch.LineNumber - 1
$end = $endMatch.LineNumber - 2
$phase3aSection = $flow[$start..$end]
$phase3aSection | Select-String -Pattern 'active commit ordering'
```

Expected:

- Every match points to `2026-07-08-phase-3a-view-contract-implementation-plan.md`.
- `active commit ordering` has no matches in the Phase 3A section.

- [ ] **Step 4: Record gate result in this plan**

Append a `Task 0 Gate Evidence` subsection to this plan with the command outcomes from Step 1 through Step 3.

Expected:

- The subsection states `Task 0 gate PASS` with the exact nonzero counts from Step 2 and the corrected Phase 3A scope wording.
- If a generated root `Cargo.lock` was removed, the subsection names that cleanup before the final `git status --short` evidence.

- [ ] **Step 5: Commit**

Run:

```powershell
git add docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md
git commit -m "docs: update phase 3a plan reference"
```

#### Task 0 Gate Evidence

Task 0 gate PASS, recorded on 2026-07-08.

- `git log --oneline --grep "record phase 2 frame transaction verification" --all` returned `bd9c625 docs: record phase 2 frame transaction verification`.
- `Select-String` against the Phase 2 plan returned `Phase 2 Verification Record`, `Final Phase 2 evidence commit`, `exit code 0`, `Architecture/API/lifecycle re-review reported`, `Fact/evidence re-review is now reconciled as PASS`, and `Phase 2 gate PASS`.
- A local untracked root `Cargo.lock` generated during smoke verification was removed before recording final status; this was an untracked local artifact, not durable repo evidence.
- At initial Task 0 gate time after that local cleanup, `git status --short` showed only intentional planning changes: `M docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md` and `?? docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md`. Subsequent review-driven planning updates intentionally added the readiness brief, Phase 2 evidence record, roadmap, and decisions documents to the current planning diff.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core transaction::tests::commit_rejects_remounted_current_active_branch_with_same_fingerprint -- --nocapture` ran the target regression and passed with `1 passed; 0 failed`.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features facade_prelude_exposes_phase_2_observation_contracts -- --nocapture` ran the target facade observation test and passed with `1 passed; 0 failed`.
- `Select-String` against the Phase 0-3 flow showed Phase 3A references point to `2026-07-08-phase-3a-view-contract-implementation-plan.md`.
- Phase 0-3 flow Phase 3A wording now requires renderer-agnostic contracts, fake renderer build/apply success and failure paths, stale-result modeling, failure cleanup reporting, and negative boundary checks proving Phase 3A does not expose active UI or commit authority.
- `active commit ordering` is now scoped to Phase 3B in the Phase 0-3 flow; it is not listed as a Phase 3A requirement after the scope wording update.

---

### Task 1: Renderer-Neutral Contract Boundary

**Files:**
- Modify: `crates/bevy_ui_router_view/src/lib.rs`
- Create: `crates/bevy_ui_router_view/src/contract.rs`

**Interfaces:**
- Consumes: `bevy_ui_router_core::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory}`.
- Produces:
  - `ViewBindingKey`
  - `RouteViewDescriptor`
  - `ViewRequestId`
  - `ViewOutputId`
  - crate-private `ViewRendererIssuer`
  - `ViewBuildRequest`
  - `ViewRenderPlan`
  - `ViewDisposalHint`
  - `ViewPartialOutput`
  - `ViewFailure`
  - `ViewBuildOutcome`
  - `ViewApplyOutcome`

- [ ] **Step 1: Add RED contract tests**

Create `crates/bevy_ui_router_view/src/contract.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn descriptor_and_build_request_preserve_binding_and_request_id() {
        let request_id = ViewRequestId::from_raw(7);
        let issuer = ViewRendererIssuer::from_raw(1);
        let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("layout"));
        let request = ViewBuildRequest::new(issuer, request_id, descriptor.clone());

        assert_eq!(request.issuer(), issuer);
        assert_eq!(request.request_id(), request_id);
        assert_eq!(request.descriptor(), &descriptor);
        assert_eq!(request.descriptor().binding().as_str(), "layout");
    }

    #[test]
    fn build_plan_is_not_partial_output() {
        let request = ViewBuildRequest::new(
            ViewRendererIssuer::from_raw(1),
            ViewRequestId::from_raw(9),
            RouteViewDescriptor::new(ViewBindingKey::new("leaf")),
        );
        let plan = ViewRenderPlan::new(&request);
        let ViewRenderPlan {
            issuer,
            request_id,
            descriptor,
        } = &plan;

        assert_eq!(*issuer, request.issuer());
        assert_eq!(*request_id, request.request_id());
        assert_eq!(descriptor, request.descriptor());
        assert_eq!(plan.request_id(), request.request_id());
        assert_eq!(plan.descriptor(), request.descriptor());
    }

    #[test]
    fn apply_failure_carries_apply_created_partial_output_and_cleanup_hint() {
        let request_id = ViewRequestId::from_raw(11);
        let output = ViewPartialOutput::new(
            request_id,
            ViewOutputId::from_raw(3),
            ViewBindingKey::new("leaf"),
            Some(ViewDisposalHint::new("fake-output-3")),
        );
        let failure =
            ViewFailure::apply_failed(request_id, "apply failed", Some(output.clone()));

        assert_eq!(failure.request_id(), request_id);
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert_eq!(failure.partial_output(), Some(&output));
        assert_eq!(
            failure.partial_output().unwrap().disposal_hint().unwrap().as_str(),
            "fake-output-3"
        );
    }

    #[test]
    fn build_and_stale_failures_do_not_invent_partial_output() {
        let request_id = ViewRequestId::from_raw(13);
        let build = ViewFailure::build_failed(request_id, "build failed");
        let stale = ViewFailure::stale_result(request_id, "stale renderer result");

        assert_eq!(build.error().kind(), RouteErrorKind::ViewBuildFailed);
        assert_eq!(
            build.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(build.partial_output().is_none());
        assert_eq!(stale.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            stale.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(stale.partial_output().is_none());
    }
}
```

Expose the module temporarily from `crates/bevy_ui_router_view/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

mod contract;

pub use contract::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBindingKey, ViewBuildOutcome, ViewBuildRequest,
    ViewDisposalHint, ViewFailure, ViewOutputId, ViewPartialOutput, ViewRenderPlan, ViewRequestId,
};

pub mod prelude {
    pub use crate::{RouteViewDescriptor, ViewBindingKey};
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view contract::tests -- --nocapture
```

Expected RED:

- The command fails during test build because the new contract types and methods are not implemented.
- The failure names the intended missing contract symbols or methods from this task. A failure caused by a missing module, wrong feature, missing dependency, or wrong package is not valid RED evidence.

- [ ] **Step 2: Implement the stable contract types**

Replace `crates/bevy_ui_router_view/src/contract.rs` with:

```rust
use bevy_ui_router_core::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ViewBindingKey(String);

impl ViewBindingKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewRequestId(u64);

impl ViewRequestId {
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ViewOutputId(u64);

impl ViewOutputId {
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ViewRendererIssuer(u64);

impl ViewRendererIssuer {
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteViewDescriptor {
    binding: ViewBindingKey,
}

impl RouteViewDescriptor {
    pub fn new(binding: ViewBindingKey) -> Self {
        Self { binding }
    }

    pub fn binding(&self) -> &ViewBindingKey {
        &self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewBuildRequest {
    issuer: ViewRendererIssuer,
    request_id: ViewRequestId,
    descriptor: RouteViewDescriptor,
}

impl ViewBuildRequest {
    pub(crate) fn new(
        issuer: ViewRendererIssuer,
        request_id: ViewRequestId,
        descriptor: RouteViewDescriptor,
    ) -> Self {
        Self {
            issuer,
            request_id,
            descriptor,
        }
    }

    pub(crate) const fn issuer(&self) -> ViewRendererIssuer {
        self.issuer
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn descriptor(&self) -> &RouteViewDescriptor {
        &self.descriptor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRenderPlan {
    issuer: ViewRendererIssuer,
    request_id: ViewRequestId,
    descriptor: RouteViewDescriptor,
}

impl ViewRenderPlan {
    pub(crate) fn new(request: &ViewBuildRequest) -> Self {
        Self {
            issuer: request.issuer(),
            request_id: request.request_id(),
            descriptor: request.descriptor().clone(),
        }
    }

    pub(crate) const fn issuer(&self) -> ViewRendererIssuer {
        self.issuer
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn descriptor(&self) -> &RouteViewDescriptor {
        &self.descriptor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewDisposalHint(String);

impl ViewDisposalHint {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewPartialOutput {
    request_id: ViewRequestId,
    output_id: ViewOutputId,
    binding: ViewBindingKey,
    disposal_hint: Option<ViewDisposalHint>,
}

impl ViewPartialOutput {
    pub(crate) fn new(
        request_id: ViewRequestId,
        output_id: ViewOutputId,
        binding: ViewBindingKey,
        disposal_hint: Option<ViewDisposalHint>,
    ) -> Self {
        Self {
            request_id,
            output_id,
            binding,
            disposal_hint,
        }
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub const fn output_id(&self) -> ViewOutputId {
        self.output_id
    }

    pub fn binding(&self) -> &ViewBindingKey {
        &self.binding
    }

    pub fn disposal_hint(&self) -> Option<&ViewDisposalHint> {
        self.disposal_hint.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFailure {
    request_id: ViewRequestId,
    error: RouteError,
    partial_output: Option<ViewPartialOutput>,
}

impl ViewFailure {
    pub(crate) fn build_failed(
        request_id: ViewRequestId,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            request_id,
            error: RouteError::new(
                RouteErrorKind::ViewBuildFailed,
                RouteErrorSourceCategory::RendererContract,
                redacted_message,
            ),
            partial_output: None,
        }
    }

    pub(crate) fn apply_failed(
        request_id: ViewRequestId,
        redacted_message: impl Into<String>,
        partial_output: Option<ViewPartialOutput>,
    ) -> Self {
        Self {
            request_id,
            error: RouteError::new(
                RouteErrorKind::ViewApplyFailed,
                RouteErrorSourceCategory::RendererContract,
                redacted_message,
            ),
            partial_output,
        }
    }

    pub(crate) fn stale_result(
        request_id: ViewRequestId,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            request_id,
            error: RouteError::new(
                RouteErrorKind::StaleResult,
                RouteErrorSourceCategory::RendererContract,
                redacted_message,
            ),
            partial_output: None,
        }
    }

    pub const fn request_id(&self) -> ViewRequestId {
        self.request_id
    }

    pub fn error(&self) -> &RouteError {
        &self.error
    }

    pub fn partial_output(&self) -> Option<&ViewPartialOutput> {
        self.partial_output.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewBuildOutcome {
    Built(ViewRenderPlan),
    Failed(ViewFailure),
    Stale(ViewFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewApplyOutcome {
    Applied(ViewPartialOutput),
    Failed(ViewFailure),
    Stale(ViewFailure),
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn descriptor_and_build_request_preserve_binding_and_request_id() {
        let request_id = ViewRequestId::from_raw(7);
        let issuer = ViewRendererIssuer::from_raw(1);
        let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("layout"));
        let request = ViewBuildRequest::new(issuer, request_id, descriptor.clone());

        assert_eq!(request.issuer(), issuer);
        assert_eq!(request.request_id(), request_id);
        assert_eq!(request.descriptor(), &descriptor);
        assert_eq!(request.descriptor().binding().as_str(), "layout");
    }

    #[test]
    fn build_plan_is_not_partial_output() {
        let request = ViewBuildRequest::new(
            ViewRendererIssuer::from_raw(1),
            ViewRequestId::from_raw(9),
            RouteViewDescriptor::new(ViewBindingKey::new("leaf")),
        );
        let plan = ViewRenderPlan::new(&request);
        let ViewRenderPlan {
            issuer,
            request_id,
            descriptor,
        } = &plan;

        assert_eq!(*issuer, request.issuer());
        assert_eq!(*request_id, request.request_id());
        assert_eq!(descriptor, request.descriptor());
        assert_eq!(plan.request_id(), request.request_id());
        assert_eq!(plan.descriptor(), request.descriptor());
    }

    #[test]
    fn apply_failure_carries_apply_created_partial_output_and_cleanup_hint() {
        let request_id = ViewRequestId::from_raw(11);
        let output = ViewPartialOutput::new(
            request_id,
            ViewOutputId::from_raw(3),
            ViewBindingKey::new("leaf"),
            Some(ViewDisposalHint::new("fake-output-3")),
        );
        let failure =
            ViewFailure::apply_failed(request_id, "apply failed", Some(output.clone()));

        assert_eq!(failure.request_id(), request_id);
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert_eq!(failure.partial_output(), Some(&output));
        assert_eq!(
            failure.partial_output().unwrap().disposal_hint().unwrap().as_str(),
            "fake-output-3"
        );
    }

    #[test]
    fn build_and_stale_failures_do_not_invent_partial_output() {
        let request_id = ViewRequestId::from_raw(13);
        let build = ViewFailure::build_failed(request_id, "build failed");
        let stale = ViewFailure::stale_result(request_id, "stale renderer result");

        assert_eq!(build.error().kind(), RouteErrorKind::ViewBuildFailed);
        assert_eq!(
            build.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(build.partial_output().is_none());
        assert_eq!(stale.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            stale.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(stale.partial_output().is_none());
    }
}
```

The crate-private constructors are intentional. Phase 3A exposes observation and test-support behavior, not a public way to fabricate renderer results that a later ECS bridge might mistake for trusted output.

- [ ] **Step 3: Add compile-fail visibility guard**

Replace `crates/bevy_ui_router_view/src/lib.rs` with:

```rust
#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "test-support"),
    doc = r#"
```compile_fail
use bevy_ui_router_view::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router_view::prelude::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router_view::prelude::FakeViewRendererMode;
```

```compile_fail
use bevy_ui_router_view::test_support::FakeViewRenderer;
```
"#
)]
#![cfg_attr(
    feature = "test-support",
    doc = r#"
```compile_fail
use bevy_ui_router_view::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
renderer.commit();
```

```compile_fail
use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode};
use bevy_ui_router_core::prelude::FrameApplyToken;

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let token: FrameApplyToken = unimplemented!();
renderer.accept_apply_token(token);
```

```compile_fail
use bevy_ui_router_view::prelude::{FakeViewRenderer, FakeViewRendererMode};
use bevy_ui_router_core::prelude::FrameApplyRequest;

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let request: FrameApplyRequest = unimplemented!();
renderer.finalize(request);
```

```compile_fail
use bevy_ui_router_view::prelude::ViewCommitAuthority;
```

```compile_fail
use bevy_ui_router_view::test_support::ViewFinalizeAuthority;
```
"#
)]

//! ```compile_fail
//! use bevy_ui_router_view::prelude::ViewBuildRequest;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::ViewApplyOutcome;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewRequestId;
//!
//! let _ = ViewRequestId::from_raw(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewBuildRequest;
//!
//! let _ = ViewBuildRequest::new;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewOutputId;
//!
//! let _ = ViewOutputId::from_raw(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewBuildRequest, ViewRenderPlan};
//!
//! let request: &ViewBuildRequest = unimplemented!();
//! let _ = ViewRenderPlan::new(request);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::ViewDisposalHint;
//!
//! let _ = ViewDisposalHint::new("fake-output");
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{
//!     ViewBindingKey, ViewOutputId, ViewPartialOutput, ViewRequestId,
//! };
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let output_id: ViewOutputId = unimplemented!();
//! let _ = ViewPartialOutput::new(request_id, output_id, ViewBindingKey::new("leaf"), None);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewFailure, ViewRequestId};
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let _ = ViewFailure::build_failed(request_id, "build failed");
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewFailure, ViewRequestId};
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let _ = ViewFailure::apply_failed(request_id, "apply failed", None);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::{ViewFailure, ViewRequestId};
//!
//! let request_id: ViewRequestId = unimplemented!();
//! let _ = ViewFailure::stale_result(request_id, "stale result");
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::FrameApplyToken;
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_view::prelude::NavigationTransaction;
//! ```
//!
mod contract;

#[cfg(feature = "test-support")]
mod testing;

pub use contract::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBindingKey, ViewBuildOutcome, ViewBuildRequest,
    ViewDisposalHint, ViewFailure, ViewOutputId, ViewPartialOutput, ViewRenderPlan, ViewRequestId,
};

#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::testing::{FakeViewRenderer, FakeViewRendererMode};
}

pub mod prelude {
    pub use crate::{RouteViewDescriptor, ViewBindingKey};

    #[cfg(feature = "test-support")]
    pub use crate::test_support::{FakeViewRenderer, FakeViewRendererMode};
}
```

Create `crates/bevy_ui_router_view/tests/compile_fail_boundary.rs`:

```rust
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
    fs::write(root.join("src/main.rs"), fixture.code)
        .expect("fixture source should be written");

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
        expected_stderr: &["error[E0624]", "associated function `build_failed` is private"],
    });
}

#[test]
fn private_view_failure_apply_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_view_failure_apply_constructor",
        code: "use bevy_ui_router_view::ViewFailure; fn main() { let _ = ViewFailure::apply_failed; }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `apply_failed` is private"],
    });
}

#[test]
fn private_view_failure_stale_constructor_names_private_function() {
    assert_compile_fails(Fixture {
        name: "private_view_failure_stale_constructor",
        code: "use bevy_ui_router_view::ViewFailure; fn main() { let _ = ViewFailure::stale_result; }",
        features: &[],
        expected_stderr: &["error[E0624]", "associated function `stale_result` is private"],
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
        expected_stderr: &["error[E0432]", "no `ViewFinalizeAuthority` in `test_support`"],
    });
}
```

Create `crates/bevy_ui_router_view/tests/authority_surface_boundary.rs`:

```rust
fn assert_forbidden_public_fragments_absent(file: &str, source: &str) {
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
            "{file} exposes forbidden Phase 3A authority surface fragment `{fragment}`"
        );
    }
}

#[test]
fn view_crate_does_not_expose_active_commit_or_acceptance_surface() {
    assert_forbidden_public_fragments_absent("src/lib.rs", include_str!("../src/lib.rs"));
    assert_forbidden_public_fragments_absent("src/contract.rs", include_str!("../src/contract.rs"));
}
```

If Phase 3B later introduces a controlled reporter/session API, that phase must replace this deny-list with an explicit allow-list proving the new API cannot construct or accept core lifecycle authority.

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view contract::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test compile_fail_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test authority_surface_boundary -- --nocapture
cargo fmt --check
```

Expected:

- `contract::tests` runs 4 tests and passes.
- Default view doctests run the compile-fail examples for protocol records staying out of the default prelude, private constructors, missing core authority re-exports, and default fake-renderer non-exposure, then pass.
- Default view compile-fail boundary harness runs 11 tests and asserts stderr fragments for the intended private/missing symbols.
- View authority-surface boundary harness runs 1 test and rejects accidental public commit, accept, finalize, activate, or frame-apply conversion APIs.
- Formatting passes.

- [ ] **Step 4: Commit**

Run:

```powershell
git add crates/bevy_ui_router_view/src/lib.rs crates/bevy_ui_router_view/src/contract.rs crates/bevy_ui_router_view/tests/compile_fail_boundary.rs crates/bevy_ui_router_view/tests/authority_surface_boundary.rs
git commit -m "feat(view): add renderer-neutral view contract"
```

---

### Task 2: Feature-Gated Fake Renderer Harness

**Files:**
- Modify: `crates/bevy_ui_router_view/Cargo.toml`
- Modify: `crates/bevy_ui_router_view/src/lib.rs`
- Create: `crates/bevy_ui_router_view/src/testing.rs`
- Modify: `crates/bevy_ui_router_view/tests/authority_surface_boundary.rs`

**Interfaces:**
- Consumes:
  - `RouteViewDescriptor`
  - `ViewApplyOutcome`
  - `ViewBindingKey`
  - `ViewBuildOutcome`
  - `ViewBuildRequest`
  - `ViewDisposalHint`
  - `ViewFailure`
  - `ViewOutputId`
  - `ViewPartialOutput`
  - `ViewRenderPlan`
  - `ViewRequestId`
- Produces:
  - `test-support` feature
  - `FakeViewRenderer`
  - `FakeViewRendererMode`

- [ ] **Step 1: Add the `test-support` feature and RED fake renderer tests**

Replace `crates/bevy_ui_router_view/Cargo.toml` with:

```toml
[package]
name = "bevy_ui_router_view"
version.workspace = true
edition.workspace = true

[features]
default = []
test-support = []

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
```

Create `crates/bevy_ui_router_view/src/testing.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ViewApplyOutcome, ViewBuildOutcome};
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn fake_renderer_allocates_request_ids_deterministically() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let first = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("first")));
        let second = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("second")));

        assert_eq!(first.request_id().raw(), 1);
        assert_eq!(second.request_id().raw(), 2);
    }

    #[test]
    fn fake_renderer_build_success_does_not_create_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };

        assert_eq!(plan.request_id(), request.request_id());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_rejects_request_from_another_renderer_without_partial_output() {
        let mut source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let ViewBuildOutcome::Stale(failure) = renderer.build(&request) else {
            panic!("foreign request should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_apply_success_creates_and_records_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };

        let ViewApplyOutcome::Applied(output) = renderer.apply(plan) else {
            panic!("success mode should apply");
        };

        assert_eq!(output.request_id(), request.request_id());
        assert_eq!(output.output_id().raw(), 1);
        assert_eq!(output.binding().as_str(), "leaf");
        assert_eq!(output.disposal_hint().unwrap().as_str(), "fake-output-1");
        assert_eq!(renderer.partial_outputs(), &[output]);
    }

    #[test]
    fn fake_renderer_rejects_already_consumed_plan_without_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };
        let consumed = plan.clone();
        let ViewApplyOutcome::Applied(_output) = renderer.apply(plan) else {
            panic!("success mode should apply");
        };

        let ViewApplyOutcome::Stale(failure) = renderer.apply(consumed) else {
            panic!("consumed plan should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert_eq!(renderer.partial_outputs().len(), 1);
    }

    #[test]
    fn fake_renderer_rejects_plan_from_another_renderer_without_partial_output() {
        let mut source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = source.build(&request) else {
            panic!("source renderer should build");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("foreign plan should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_build_failure_without_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildFailure);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Failed(failure) = renderer.build(&request) else {
            panic!("build failure mode should fail build");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewBuildFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_build_failure_mode_rejects_apply_without_partial_output() {
        let mut plan_source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = plan_source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = plan_source.build(&request) else {
            panic!("success mode should provide a forced plan");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildFailure);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("build failure mode should not apply a forced plan");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_apply_failure_with_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::ApplyFailure);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("apply failure mode should build before failing apply");
        };

        let ViewApplyOutcome::Failed(failure) = renderer.apply(plan) else {
            panic!("apply failure mode should fail apply");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert_eq!(
            failure.partial_output().map(ViewPartialOutput::output_id),
            Some(ViewOutputId::from_raw(1))
        );
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_stale_build_result_without_visible_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildStaleResult);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Stale(failure) = renderer.build(&request) else {
            panic!("stale mode should report stale build result");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_build_stale_mode_rejects_apply_without_partial_output() {
        let mut plan_source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = plan_source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = plan_source.build(&request) else {
            panic!("success mode should provide a forced plan");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildStaleResult);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("build stale mode should not apply a forced plan");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_stale_apply_result_without_visible_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::ApplyStaleResult);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("apply stale mode should build before reporting stale apply");
        };

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("apply stale mode should report stale apply result");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }
}
```

Enable the module in `crates/bevy_ui_router_view/src/lib.rs` if Task 1 did not already do it:

```rust
#[cfg(feature = "test-support")]
mod testing;

#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::testing::{FakeViewRenderer, FakeViewRendererMode};
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support testing::tests -- --nocapture
```

Expected RED:

- The command fails during test build because `FakeViewRenderer`, `FakeViewRendererMode`, or their methods are not implemented.
- The failure names the intended missing fake-renderer symbols or methods from this task. A failure caused by a missing feature, missing module, missing dependency, or wrong package is not valid RED evidence.

- [ ] **Step 2: Implement the fake renderer**

Replace `crates/bevy_ui_router_view/src/testing.rs` with:

```rust
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    RouteViewDescriptor, ViewApplyOutcome, ViewBindingKey, ViewBuildOutcome, ViewBuildRequest,
    ViewDisposalHint, ViewFailure, ViewOutputId, ViewPartialOutput, ViewRenderPlan,
    ViewRendererIssuer, ViewRequestId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeViewRendererMode {
    Success,
    BuildFailure,
    ApplyFailure,
    BuildStaleResult,
    ApplyStaleResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeViewRenderer {
    issuer: ViewRendererIssuer,
    mode: FakeViewRendererMode,
    next_request: u64,
    next_output: u64,
    pending_plans: BTreeSet<ViewRequestId>,
    partial_outputs: Vec<ViewPartialOutput>,
}

impl FakeViewRenderer {
    pub fn new(mode: FakeViewRendererMode) -> Self {
        Self {
            issuer: next_renderer_issuer(),
            mode,
            next_request: 1,
            next_output: 1,
            pending_plans: BTreeSet::new(),
            partial_outputs: Vec::new(),
        }
    }

    pub fn request(&mut self, descriptor: RouteViewDescriptor) -> ViewBuildRequest {
        let request_id = ViewRequestId::from_raw(self.next_request);
        self.next_request += 1;
        ViewBuildRequest::new(self.issuer, request_id, descriptor)
    }

    pub fn build(&mut self, request: &ViewBuildRequest) -> ViewBuildOutcome {
        if request.issuer() != self.issuer {
            return ViewBuildOutcome::Stale(ViewFailure::stale_result(
                request.request_id(),
                "request was not issued by this fake renderer",
            ));
        }

        match self.mode {
            FakeViewRendererMode::BuildFailure => ViewBuildOutcome::Failed(
                ViewFailure::build_failed(request.request_id(), "view build failed"),
            ),
            FakeViewRendererMode::BuildStaleResult => ViewBuildOutcome::Stale(
                ViewFailure::stale_result(request.request_id(), "stale renderer result"),
            ),
            FakeViewRendererMode::Success
            | FakeViewRendererMode::ApplyFailure
            | FakeViewRendererMode::ApplyStaleResult => {
                self.pending_plans.insert(request.request_id());
                ViewBuildOutcome::Built(ViewRenderPlan::new(request))
            }
        }
    }

    pub fn apply(&mut self, plan: ViewRenderPlan) -> ViewApplyOutcome {
        if plan.issuer() != self.issuer || !self.pending_plans.remove(&plan.request_id()) {
            return ViewApplyOutcome::Stale(ViewFailure::stale_result(
                plan.request_id(),
                "render plan was not issued by this fake renderer",
            ));
        }

        match self.mode {
            FakeViewRendererMode::BuildFailure | FakeViewRendererMode::BuildStaleResult => {
                ViewApplyOutcome::Stale(ViewFailure::stale_result(
                    plan.request_id(),
                    "apply called without successful build",
                ))
            }
            FakeViewRendererMode::ApplyStaleResult => ViewApplyOutcome::Stale(
                ViewFailure::stale_result(plan.request_id(), "stale renderer result"),
            ),
            FakeViewRendererMode::ApplyFailure => {
                let output = self.next_partial_output(&plan);
                ViewApplyOutcome::Failed(ViewFailure::apply_failed(
                    plan.request_id(),
                    "view apply failed",
                    Some(output),
                ))
            }
            FakeViewRendererMode::Success => {
                let output = self.next_partial_output(&plan);
                self.partial_outputs.push(output.clone());
                ViewApplyOutcome::Applied(output)
            }
        }
    }

    pub fn partial_outputs(&self) -> &[ViewPartialOutput] {
        &self.partial_outputs
    }

    fn next_partial_output(&mut self, plan: &ViewRenderPlan) -> ViewPartialOutput {
        let output_id = ViewOutputId::from_raw(self.next_output);
        self.next_output += 1;
        ViewPartialOutput::new(
            plan.request_id(),
            output_id,
            plan.descriptor().binding().clone(),
            Some(ViewDisposalHint::new(format!(
                "fake-output-{}",
                output_id.raw()
            ))),
        )
    }
}

fn next_renderer_issuer() -> ViewRendererIssuer {
    static NEXT_RENDERER: AtomicU64 = AtomicU64::new(1);
    ViewRendererIssuer::from_raw(NEXT_RENDERER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ViewApplyOutcome, ViewBuildOutcome};
    use bevy_ui_router_core::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn fake_renderer_allocates_request_ids_deterministically() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let first = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("first")));
        let second = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("second")));

        assert_eq!(first.request_id().raw(), 1);
        assert_eq!(second.request_id().raw(), 2);
    }

    #[test]
    fn fake_renderer_build_success_does_not_create_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };

        assert_eq!(plan.request_id(), request.request_id());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_rejects_request_from_another_renderer_without_partial_output() {
        let mut source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let ViewBuildOutcome::Stale(failure) = renderer.build(&request) else {
            panic!("foreign request should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_apply_success_creates_and_records_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };

        let ViewApplyOutcome::Applied(output) = renderer.apply(plan) else {
            panic!("success mode should apply");
        };

        assert_eq!(output.request_id(), request.request_id());
        assert_eq!(output.output_id().raw(), 1);
        assert_eq!(output.binding().as_str(), "leaf");
        assert_eq!(output.disposal_hint().unwrap().as_str(), "fake-output-1");
        assert_eq!(renderer.partial_outputs(), &[output]);
    }

    #[test]
    fn fake_renderer_rejects_already_consumed_plan_without_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("success mode should build");
        };
        let consumed = plan.clone();
        let ViewApplyOutcome::Applied(_output) = renderer.apply(plan) else {
            panic!("success mode should apply");
        };

        let ViewApplyOutcome::Stale(failure) = renderer.apply(consumed) else {
            panic!("consumed plan should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert_eq!(renderer.partial_outputs().len(), 1);
    }

    #[test]
    fn fake_renderer_rejects_plan_from_another_renderer_without_partial_output() {
        let mut source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = source.build(&request) else {
            panic!("source renderer should build");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("foreign plan should be rejected as stale");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_build_failure_without_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildFailure);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Failed(failure) = renderer.build(&request) else {
            panic!("build failure mode should fail build");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewBuildFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_build_failure_mode_rejects_apply_without_partial_output() {
        let mut plan_source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = plan_source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = plan_source.build(&request) else {
            panic!("success mode should provide a forced plan");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildFailure);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("build failure mode should not apply a forced plan");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_apply_failure_with_partial_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::ApplyFailure);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("apply failure mode should build before failing apply");
        };

        let ViewApplyOutcome::Failed(failure) = renderer.apply(plan) else {
            panic!("apply failure mode should fail apply");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::ViewApplyFailed);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert_eq!(
            failure.partial_output().map(ViewPartialOutput::output_id),
            Some(ViewOutputId::from_raw(1))
        );
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_stale_build_result_without_visible_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildStaleResult);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));

        let ViewBuildOutcome::Stale(failure) = renderer.build(&request) else {
            panic!("stale mode should report stale build result");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_build_stale_mode_rejects_apply_without_partial_output() {
        let mut plan_source = FakeViewRenderer::new(FakeViewRendererMode::Success);
        let request = plan_source.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = plan_source.build(&request) else {
            panic!("success mode should provide a forced plan");
        };
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::BuildStaleResult);

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("build stale mode should not apply a forced plan");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }

    #[test]
    fn fake_renderer_reports_stale_apply_result_without_visible_output() {
        let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::ApplyStaleResult);
        let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
        let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
            panic!("apply stale mode should build before reporting stale apply");
        };

        let ViewApplyOutcome::Stale(failure) = renderer.apply(plan) else {
            panic!("apply stale mode should report stale apply result");
        };

        assert_eq!(failure.request_id(), request.request_id());
        assert_eq!(failure.error().kind(), RouteErrorKind::StaleResult);
        assert_eq!(
            failure.error().source_category(),
            RouteErrorSourceCategory::RendererContract
        );
        assert!(failure.partial_output().is_none());
        assert!(renderer.partial_outputs().is_empty());
    }
}
```

- [ ] **Step 3: Run GREEN verification**

Update `crates/bevy_ui_router_view/tests/authority_surface_boundary.rs` so the existing test also scans the fake renderer source now that Task 2 has created it:

```rust
#[test]
fn view_crate_does_not_expose_active_commit_or_acceptance_surface() {
    assert_forbidden_public_fragments_absent("src/lib.rs", include_str!("../src/lib.rs"));
    assert_forbidden_public_fragments_absent("src/contract.rs", include_str!("../src/contract.rs"));
    assert_forbidden_public_fragments_absent("src/testing.rs", include_str!("../src/testing.rs"));
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support testing::tests -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --lib -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --lib -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --test compile_fail_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --test authority_surface_boundary -- --nocapture
cargo fmt --check
```

Expected:

- `testing::tests` runs 12 tests and passes.
- Default view lib tests run the 4 contract tests and pass.
- Test-support view lib tests run 16 unit tests and pass.
- Test-support view doctests prove fake renderer exposure still does not provide commit, token-acceptance, finalize, or named authority-trait paths.
- Test-support view compile-fail boundary harness runs 17 tests and asserts stderr fragments for private constructors, root non-exposure, and missing commit/token/finalize/named authority symbols.
- Test-support view authority-surface boundary harness runs 1 test and scans `src/testing.rs` after it exists.
- Formatting passes.

- [ ] **Step 4: Commit**

Run:

```powershell
git add crates/bevy_ui_router_view/Cargo.toml crates/bevy_ui_router_view/src/lib.rs crates/bevy_ui_router_view/src/testing.rs crates/bevy_ui_router_view/tests/authority_surface_boundary.rs
git commit -m "test(view): add feature-gated fake renderer"
```

---

### Task 3: Dependency And Facade Feature Coverage

**Files:**
- Create: `crates/bevy_ui_router_view/tests/dependency_boundary.rs`
- Modify: `crates/bevy_ui_router/Cargo.toml`
- Modify: `crates/bevy_ui_router/src/lib.rs`
- Modify: `crates/bevy_ui_router/tests/facade_features.rs`

**Interfaces:**
- Consumes: Phase 3A view public/prelude types and `test-support` feature.
- Produces: dependency boundary, default facade view contract coverage, explicit fake-renderer facade coverage, and no-default facade safety.

- [ ] **Step 1: Add dependency-boundary test**

Create `crates/bevy_ui_router_view/tests/dependency_boundary.rs`:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn section_name(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']')
}

fn dependency_section_kind(section: &str) -> Option<&str> {
    let section = if let Some(target_tail) = section.strip_prefix("target.") {
        let (_, remainder) = target_tail.split_once('.')?;
        remainder
    } else {
        section
    };

    ["dependencies", "dev-dependencies", "build-dependencies"]
        .into_iter()
        .find(|kind| *kind == section || section.starts_with(&format!("{kind}.")))
}

fn dependency_subtable_alias(section: &str) -> Option<&str> {
    let kind = dependency_section_kind(section)?;
    section
        .strip_prefix("target.")
        .and_then(|tail| tail.split_once('.').map(|(_, remainder)| remainder))
        .unwrap_or(section)
        .strip_prefix(kind)?
        .strip_prefix('.')
}

fn dependency_name_is_forbidden(name: &str) -> bool {
    name == "bsn" || name.starts_with("bevy") && !name.starts_with("bevy_ui_router")
}

fn quoted_string(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(&value[..end])
}

fn package_rename(key: &str, value: &str) -> Option<String> {
    let value = value.split('#').next().unwrap_or(value).trim();

    if key == "package" {
        return quoted_string(value).map(str::to_owned);
    }

    let inline_table = value.strip_prefix('{')?.trim_end_matches('}').trim();
    for entry in inline_table.split(',') {
        let Some((field, field_value)) = entry.split_once('=') else {
            continue;
        };

        if field.trim() == "package" {
            return quoted_string(field_value).map(str::to_owned);
        }
    }

    None
}

fn dependency_entries(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut entries = Vec::new();

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);
            continue;
        }

        if !current_section.is_some_and(|section| dependency_section_kind(section).is_some()) {
            continue;
        }

        let Some((name, _value)) = line.split_once('=') else {
            continue;
        };
        entries.push(name.trim().to_owned());
    }

    entries
}

fn dependency_violations(manifest: &str) -> Vec<String> {
    let mut current_section = None;
    let mut violations = Vec::new();

    for (line_number, raw_line) in manifest.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);

            if let Some(alias) = dependency_subtable_alias(section) {
                if dependency_name_is_forbidden(alias) {
                    violations.push(format!(
                        "line {}: forbidden dependency subtable `{}`",
                        line_number + 1,
                        section
                    ));
                }
            }
            continue;
        }

        if !current_section.is_some_and(|section| dependency_section_kind(section).is_some()) {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        if dependency_subtable_alias(current_section.expect("checked above")).is_none()
            && dependency_name_is_forbidden(key)
        {
            violations.push(format!(
                "line {}: forbidden dependency key `{}`",
                line_number + 1,
                key
            ));
        }

        if let Some(package) = package_rename(key, value) {
            if package == "bsn" {
                violations.push(format!(
                    "line {}: forbidden package rename to `bsn`",
                    line_number + 1
                ));
            } else if package.starts_with("bevy") && !package.starts_with("bevy_ui_router") {
                violations.push(format!(
                    "line {}: forbidden package rename to Bevy-family crate",
                    line_number + 1
                ));
            }
        }
    }

    violations
}

fn feature_entries(manifest: &str) -> BTreeMap<String, String> {
    let mut current_section = None;
    let mut features = BTreeMap::new();

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            current_section = Some(section);
            continue;
        }

        if current_section == Some("features") {
            let (name, value) = line
                .split_once('=')
                .expect("feature entries should contain '='");
            features.insert(name.trim().to_owned(), value.trim().to_owned());
        }
    }

    features
}

fn dependency_default_features_disabled(manifest: &str, dependency: &str) -> bool {
    let mut in_dependency_subtable = false;

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(section) = section_name(line) {
            in_dependency_subtable = dependency_subtable_alias(section) == Some(dependency);
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == dependency {
                return value.contains("default-features = false")
                    || value.contains("default-features=false");
            }

            if in_dependency_subtable && key == "default-features" {
                return value == "false";
            }
        }
    }

    false
}

#[test]
fn view_manifest_depends_only_on_core_and_keeps_test_support_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("view manifest should be readable");

    assert_eq!(dependency_entries(&manifest), vec!["bevy_ui_router_core"]);
    assert!(dependency_violations(&manifest).is_empty());
    assert_eq!(
        feature_entries(&manifest),
        BTreeMap::from([
            ("default".to_owned(), "[]".to_owned()),
            ("test-support".to_owned(), "[]".to_owned()),
        ])
    );
}

#[test]
fn dependency_subtables_and_package_renames_are_checked() {
    let manifest = r#"
[dependencies]
render_inline = { package = "bevy_render", version = "1" }
core_inline = { package = "bevy_ui_router_core", path = "../core" }

[dependencies.bsn]
version = "1"

[target.'cfg(target_os = "linux")'.dev-dependencies.bevy_input]
version = "1"

[target.'cfg(target_os = "windows")'.build-dependencies.integration]
package = "bsn"
"#;

    assert_eq!(
        dependency_violations(manifest),
        vec![
            "line 3: forbidden package rename to Bevy-family crate",
            "line 6: forbidden dependency subtable `dependencies.bsn`",
            "line 9: forbidden dependency subtable `target.'cfg(target_os = \"linux\")'.dev-dependencies.bevy_input`",
            "line 13: forbidden package rename to `bsn`",
        ]
    );
}

#[test]
fn dependency_default_features_helper_accepts_inline_and_subtable_forms() {
    let inline = r#"
[dependencies]
bevy_ui_router_view = { path = "../view", default-features = false }
"#;
    let subtable = r#"
[dependencies.bevy_ui_router_view]
path = "../view"
default-features = false
"#;

    assert!(dependency_default_features_disabled(
        inline,
        "bevy_ui_router_view"
    ));
    assert!(dependency_default_features_disabled(
        subtable,
        "bevy_ui_router_view"
    ));
}

#[test]
fn facade_depends_on_view_with_default_features_disabled() {
    let facade_manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("view crate should have a workspace parent")
        .join("bevy_ui_router")
        .join("Cargo.toml");
    let manifest = fs::read_to_string(facade_manifest_path)
        .expect("facade manifest should be readable");

    assert!(dependency_default_features_disabled(
        &manifest,
        "bevy_ui_router_view"
    ));
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test dependency_boundary -- --nocapture
```

Expected:

- `dependency_boundary` runs 4 tests and passes.

- [ ] **Step 2: Add explicit facade test-support feature**

Replace `crates/bevy_ui_router/Cargo.toml` with:

```toml
[package]
name = "bevy_ui_router"
version.workspace = true
edition.workspace = true

[features]
default = ["view"]
view = ["dep:bevy_ui_router_view"]
view-test-support = ["view", "bevy_ui_router_view/test-support"]
ecs = ["dep:bevy_ui_router_ecs"]
bsn = ["ecs", "dep:bevy_ui_router_bsn"]
platform = ["dep:bevy_ui_router_platform"]
devtools = []

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
bevy_ui_router_view = { path = "../bevy_ui_router_view", optional = true, default-features = false }
bevy_ui_router_ecs = { path = "../bevy_ui_router_ecs", optional = true }
bevy_ui_router_bsn = { path = "../bevy_ui_router_bsn", optional = true }
bevy_ui_router_platform = { path = "../bevy_ui_router_platform", optional = true }
```

- [ ] **Step 3: Add facade negative and positive feature tests**

Replace `crates/bevy_ui_router/src/lib.rs` with:

```rust
#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "view-test-support"),
    doc = r#"
```compile_fail
use bevy_ui_router::prelude::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router::prelude::FakeViewRendererMode;
```

```compile_fail
use bevy_ui_router::view::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router::view::test_support::FakeViewRenderer;
```

```compile_fail
use bevy_ui_router::view::prelude::FakeViewRendererMode;
```

```compile_fail
use bevy_ui_router::prelude::ViewBuildRequest;
```
"#
)]
#![cfg_attr(
    feature = "view-test-support",
    doc = r#"
```compile_fail
use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
renderer.commit();
```

```compile_fail
use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode, FrameApplyToken};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let token: FrameApplyToken = unimplemented!();
renderer.accept_apply_token(token);
```

```compile_fail
use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode, FrameApplyRequest};

let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
let request: FrameApplyRequest = unimplemented!();
renderer.finalize(request);
```

```compile_fail
use bevy_ui_router::prelude::ViewCommitAuthority;
```

```compile_fail
use bevy_ui_router::view::test_support::ViewFinalizeAuthority;
```
"#
)]

pub use bevy_ui_router_core as core;

#[cfg(feature = "view")]
pub use bevy_ui_router_view as view;

#[cfg(feature = "ecs")]
pub use bevy_ui_router_ecs as ecs;

#[cfg(feature = "bsn")]
pub use bevy_ui_router_bsn as bsn;

#[cfg(feature = "platform")]
pub use bevy_ui_router_platform as platform;

pub mod prelude {
    #[allow(unused_imports)]
    pub use bevy_ui_router_core::prelude::*;

    #[cfg(feature = "view")]
    #[allow(unused_imports)]
    pub use bevy_ui_router_view::prelude::*;
}
```

Create `crates/bevy_ui_router/tests/facade_compile_fail_boundary.rs`:

```rust
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
    fs::write(root.join("src/main.rs"), fixture.code)
        .expect("fixture source should be written");

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
fn test_support_fake_renderer_view_root_still_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_not_view_root",
        code: "use bevy_ui_router::view::FakeViewRenderer; fn main() {}",
        default_features: false,
        features: &["view-test-support"],
        expected_stderr: &["error[E0432]", "no `FakeViewRenderer` in the root"],
    });
}

#[test]
fn test_support_fake_renderer_commit_names_missing_method() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_has_no_commit",
        code: "use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode}; fn main() { let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success); renderer.commit(); }",
        default_features: false,
        features: &["view-test-support"],
        expected_stderr: &["error[E0599]", "no method named `commit` found"],
    });
}

#[test]
fn test_support_fake_renderer_finalize_names_missing_method() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_has_no_finalize",
        code: "use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode, FrameApplyRequest}; fn main() { let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success); let request: FrameApplyRequest = unreachable!(); renderer.finalize(request); }",
        default_features: false,
        features: &["view-test-support"],
        expected_stderr: &["error[E0599]", "no method named `finalize` found"],
    });
}

#[test]
fn test_support_fake_renderer_token_accept_names_missing_method() {
    assert_compile_fails(Fixture {
        name: "test_support_fake_renderer_has_no_token_accept",
        code: "use bevy_ui_router::prelude::{FakeViewRenderer, FakeViewRendererMode, FrameApplyToken}; fn main() { let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success); let token: FrameApplyToken = unreachable!(); renderer.accept_apply_token(token); }",
        default_features: false,
        features: &["view-test-support"],
        expected_stderr: &["error[E0599]", "no method named `accept_apply_token` found"],
    });
}

#[test]
fn test_support_commit_authority_trait_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "test_support_no_named_commit_authority",
        code: "use bevy_ui_router::prelude::ViewCommitAuthority; fn main() {}",
        default_features: false,
        features: &["view-test-support"],
        expected_stderr: &["error[E0432]", "no `ViewCommitAuthority` in `prelude`"],
    });
}

#[test]
fn test_support_finalize_authority_trait_names_missing_symbol() {
    assert_compile_fails(Fixture {
        name: "test_support_no_named_finalize_authority",
        code: "use bevy_ui_router::view::test_support::ViewFinalizeAuthority; fn main() {}",
        default_features: false,
        features: &["view-test-support"],
        expected_stderr: &["error[E0432]", "no `ViewFinalizeAuthority` in `test_support`"],
    });
}
```

Create `crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs`:

```rust
#[test]
fn facade_does_not_expose_active_commit_or_acceptance_surface() {
    let source = include_str!("../src/lib.rs");
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
```

If Phase 3B later adds a controlled reporter/session API at the facade layer, that phase must replace this deny-list with an explicit allow-list proving the new API is not active commit or stale-acceptance authority.

Update the `expected_features` map in `crates/bevy_ui_router/tests/facade_features.rs` so it includes:

```rust
(
    "view-test-support".to_owned(),
    "[\"view\", \"bevy_ui_router_view/test-support\"]".to_owned(),
),
```

Append these tests to `crates/bevy_ui_router/tests/facade_features.rs`:

```rust
#[cfg(feature = "view")]
#[test]
fn facade_default_prelude_exposes_phase_3a_view_binding_contracts() {
    use bevy_ui_router::prelude::{RouteViewDescriptor, ViewBindingKey};

    fn assert_public<T>() {}
    assert_public::<RouteViewDescriptor>();
    assert_public::<ViewBindingKey>();

    let descriptor = RouteViewDescriptor::new(ViewBindingKey::new("leaf"));
    assert_eq!(descriptor.binding().as_str(), "leaf");
}

#[cfg(feature = "view")]
#[test]
fn facade_view_module_exposes_phase_3a_protocol_observations_by_explicit_path() {
    use bevy_ui_router::view::{
        ViewApplyOutcome, ViewBuildOutcome, ViewBuildRequest, ViewDisposalHint, ViewFailure,
        ViewOutputId, ViewPartialOutput, ViewRenderPlan, ViewRequestId,
    };

    fn assert_public<T>() {}
    assert_public::<ViewApplyOutcome>();
    assert_public::<ViewBuildOutcome>();
    assert_public::<ViewBuildRequest>();
    assert_public::<ViewDisposalHint>();
    assert_public::<ViewFailure>();
    assert_public::<ViewOutputId>();
    assert_public::<ViewPartialOutput>();
    assert_public::<ViewRenderPlan>();
    assert_public::<ViewRequestId>();
}

#[cfg(feature = "view-test-support")]
#[test]
fn facade_test_support_prelude_exposes_phase_3a_fake_renderer() {
    use bevy_ui_router::prelude::{
        FakeViewRenderer, FakeViewRendererMode, RouteViewDescriptor, ViewBindingKey,
    };
    use bevy_ui_router::view::{ViewApplyOutcome, ViewBuildOutcome};

    let mut renderer = FakeViewRenderer::new(FakeViewRendererMode::Success);
    let request = renderer.request(RouteViewDescriptor::new(ViewBindingKey::new("leaf")));
    let ViewBuildOutcome::Built(plan) = renderer.build(&request) else {
        panic!("success mode should build");
    };
    let ViewApplyOutcome::Applied(output) = renderer.apply(plan) else {
        panic!("success mode should apply");
    };

    assert_eq!(output.request_id(), request.request_id());
    assert_eq!(output.binding().as_str(), "leaf");
}
```

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features view-test-support --doc
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features platform
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features facade_default_prelude_exposes_phase_3a_view_binding_contracts -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features facade_view_module_exposes_phase_3a_protocol_observations_by_explicit_path -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features view-test-support --test facade_features facade_test_support_prelude_exposes_phase_3a_fake_renderer -- --nocapture
```

Expected:

- No-default facade tests compile and run only core-compatible tests.
- Facade compile-fail boundary harness runs 9 tests and asserts stderr fragments for no-default, default, and `view-test-support` negative boundaries.
- Facade authority-surface boundary harness runs 1 test and rejects accidental public commit, accept, finalize, activate, or frame-apply conversion APIs.
- Default facade doctests prove `FakeViewRenderer` and `FakeViewRendererMode` are not exposed from prelude, root view, view prelude, or `test_support` paths without `view-test-support`, and protocol observation records are not in the default facade prelude.
- View-test-support facade doctests prove fake renderer exposure still does not provide commit, token-acceptance, finalize, or named authority-trait paths.
- Facade optional feature checks compile `ecs`, `bsn`, `platform`, and `--all-features` combinations after the view re-export changes.
- Default facade focused binding test runs 1 Phase 3A user-facing view contract test and passes.
- Default facade explicit-path protocol observation test runs 1 test and passes.
- `view-test-support` facade focused test runs 1 fake-renderer test and passes.

- [ ] **Step 4: Run GREEN verification**

Run:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test dependency_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features view-test-support --doc
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features platform
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features facade_default_prelude_exposes_phase_3a_view_binding_contracts -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features facade_view_module_exposes_phase_3a_protocol_observations_by_explicit_path -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features view-test-support --test facade_features facade_test_support_prelude_exposes_phase_3a_fake_renderer -- --nocapture
cargo fmt --check
```

Expected:

- View dependency-boundary integration test runs 4 tests and passes.
- No-default facade test target runs its core-compatible tests and passes.
- Facade compile-fail boundary harness runs 9 tests and asserts exact stderr fragments for negative boundaries.
- Facade authority-surface boundary harness runs 1 test and passes.
- Default facade doctests pass and include the fake-renderer non-exposure and default-prelude protocol non-exposure compile-fail guards.
- View-test-support facade doctests pass and include no commit, token-acceptance, finalize, or named authority-trait compile-fail guards.
- Facade optional feature checks pass for `ecs`, `bsn`, `platform`, and `--all-features`.
- Default facade Phase 3A binding contract test runs 1 test and passes.
- Default facade explicit-path protocol observation test runs 1 test and passes.
- View-test-support facade fake-renderer test runs 1 test and passes.
- Formatting passes.

- [ ] **Step 5: Commit**

Run:

```powershell
git add crates/bevy_ui_router_view/tests/dependency_boundary.rs crates/bevy_ui_router/Cargo.toml crates/bevy_ui_router/src/lib.rs crates/bevy_ui_router/tests/facade_features.rs crates/bevy_ui_router/tests/facade_compile_fail_boundary.rs crates/bevy_ui_router/tests/facade_authority_surface_boundary.rs
git commit -m "test: expose view contract through gated facade features"
```

---

### Task 4: Final Phase 3A Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md`

**Interfaces:**
- Consumes: all Phase 3A changes.
- Produces: final Phase 3A verification evidence and Phase 3B handoff.

- [ ] **Step 1: Run final verification**

Run:

```powershell
cargo fmt --all --check
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --lib
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --lib
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test compile_fail_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --features test-support --test compile_fail_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test authority_surface_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view --test dependency_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --test facade_features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --doc
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features view-test-support --doc
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features platform
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features facade_default_prelude_exposes_phase_3a_view_binding_contracts -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features facade_view_module_exposes_phase_3a_protocol_observations_by_explicit_path -- --nocapture
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features view-test-support --test facade_features facade_test_support_prelude_exposes_phase_3a_fake_renderer -- --nocapture
```

Expected:

- Every command exits 0.
- Record exact nonzero counts for default view lib tests, test-support view lib tests, default view doctests, test-support view doctests, view compile-fail boundary tests, test-support view compile-fail boundary tests, view authority-surface tests, view dependency-boundary tests, core tests, facade no-default tests, facade compile-fail boundary tests, facade authority-surface tests, default facade doctests, view-test-support facade doctests, facade optional feature checks, facade default binding focused test, facade explicit-path protocol focused test, and facade test-support focused test.

- [ ] **Step 2: Confirm Phase 3B handoff conditions**

Run:

```powershell
rg -n "FakeViewRenderer|ViewBuildRequest|ViewApplyOutcome|RouteViewDescriptor|ViewPartialOutput|ViewRenderPlan|ViewDisposalHint" crates\bevy_ui_router_view crates\bevy_ui_router\tests
rg -n "ViewRequestId::from_raw|ViewOutputId::from_raw|ViewBuildRequest::new|ViewRenderPlan::new|ViewDisposalHint::new|ViewPartialOutput::new|ViewFailure::" crates\bevy_ui_router_view\src crates\bevy_ui_router_view\tests crates\bevy_ui_router\tests
git status --short
```

Expected:

- Phase 3A view contract symbols exist in `bevy_ui_router_view`.
- Constructor uses for request ids, output ids, build requests, render plans, disposal hints, partial outputs, and failures are confined to `bevy_ui_router_view` crate internals and tests. This `rg` check is supplemental audit evidence; compile-fail boundary harnesses are the authority for public visibility failures.
- `ViewRequestId` and `ViewOutputId` remain renderer-local correlation ids only; no test or public API treats them as router identity, frame identity, generation, stale-check authority, or commit authority.
- `ViewApplyOutcome::Applied` and `ViewPartialOutput` remain pre-finalization, pre-commit observations.
- The `Phase 3A Verification Record` must include a Phase 3B handoff statement saying: Phase 3B uses core-issued `FrameApplyRequest` as the canonical handoff unit; non-fake adapters report renderer observations only through a controlled reporter/session minted from that request; the bridge may translate finalized renderer output into core's `FrameApplyOutcome` paired with the request token, but final sequence/router/host/slot/route/frame/generation/transaction stale checks stay inside core.
- The worktree has only intentional Phase 3A documentation changes before final commit.

- [ ] **Step 3: Whole-branch review**

Dispatch independent reviewers with these responsibility directions:

- Architecture and lifecycle review: verify Phase 3A keeps build output separate from apply-created partial output, forbids partial output on stale results, marks applied output as pre-finalization/pre-commit, treats renderer ids as local correlation only, uses core-issued `FrameApplyRequest` plus a controlled reporter/session as the Phase 3B handoff path for non-fake adapters, and does not introduce active UI or core commit authority.
- API and authority review: verify public view/facade APIs may expose stable observation contracts by explicit path, including existing Phase 2 observation types, but default preludes stay narrow and do not expose new constructors, mutation methods, accept/finalize paths, lifecycle diagnostic construction, cleanup construction, or commit authority.
- Fact and source review: verify claims against current source, Phase 2 verification evidence, AGENTS.md, and roadmap docs.
- Test and risk review: verify every Phase 3A semantic requirement has executable coverage and every command is meaningful for touched areas.

The lead agent must evaluate findings before accepting them. If multiple findings share a root cause such as authority leakage, build/apply collapse, or feature-gating drift, revise the implementation or documentation architecture instead of applying isolated edits.

- [ ] **Step 4: Write verification record**

Append a `Phase 3A Verification Record` section to this plan with:

- implementation commits;
- command outputs summarized with exact pass counts;
- whole-branch review result;
- unresolved risks;
- Phase 3B handoff statement.

- [ ] **Step 5: Commit verification record**

Run:

```powershell
git add docs/superpowers/plans/2026-07-08-phase-3a-view-contract-implementation-plan.md
git commit -m "docs: record phase 3a view contract verification"
```

---

## Review Gate Before Implementation

Before implementing Phase 3A, review this plan with independent agents using these responsibility directions:

- Architecture and lifecycle review: verify Phase 3A keeps view contracts renderer-neutral, separates build plans from apply-created partial output, forbids partial output on stale results, marks applied output as pre-finalization/pre-commit, treats renderer ids as local correlation only, uses core-issued `FrameApplyRequest` plus a controlled reporter/session as the Phase 3B handoff path for non-fake adapters, and does not introduce active UI or core commit authority.
- API and authority review: verify public view/facade APIs may expose stable observation contracts by explicit path, including existing Phase 2 observation types, but default preludes stay narrow and do not expose new constructors, mutation methods, accept/finalize paths, lifecycle diagnostic construction, cleanup construction, or commit authority.
- Fact and source review: verify claims against current source, Phase 2 verification evidence, AGENTS.md, and roadmap docs.
- Test and risk review: verify every Phase 3A semantic requirement has executable coverage and every command is meaningful for touched areas.

The lead agent must evaluate findings before accepting them. If multiple findings share a root cause such as view-layer authority leakage, build/apply boundary collapse, fake-renderer overexposure, or feature-matrix drift, revise the plan architecture instead of applying isolated edits.

After the plan passes review, ask independent agents whether the document needs structural cleanup or semantic rewrite. If cleanup is needed, create a rewritten candidate, validate it against the accepted content, replace this file only after lead-agent acceptance, then commit the accepted planning documents.

---

## Acceptance Criteria

- Phase 3A starts only after Phase 2 is complete, reviewed, and verified.
- Phase 0-3 execution flow points to this Phase 3A plan file.
- `bevy_ui_router_view` depends only on `bevy_ui_router_core`.
- `bevy_ui_router_view` does not depend on Bevy, BSN, platform crates, facade crate, or app profile concepts.
- View contract types model descriptor, build request, render plan, apply-created partial output, disposal hint, build failure, apply failure, stale result, and apply success.
- Default view and facade preludes expose only the user-facing `ViewBindingKey` and `RouteViewDescriptor` view declaration types.
- Protocol observation records such as build request, render plan, outcomes, failures, ids, disposal hints, and partial output remain public only by explicit view crate or facade `view` module paths.
- `ViewRenderPlan` does not contain `ViewPartialOutput`.
- `ViewPartialOutput` contains request id, output id, binding key, and optional renderer-local disposal hint.
- `ViewRequestId` and `ViewOutputId` are renderer-local correlation ids only, not router/frame/generation/task identity or authority.
- View build failures use `RouteErrorKind::ViewBuildFailed` with `RouteErrorSourceCategory::RendererContract`.
- View apply failures use `RouteErrorKind::ViewApplyFailed` with `RouteErrorSourceCategory::RendererContract`.
- View stale results use `RouteErrorKind::StaleResult` with `RouteErrorSourceCategory::RendererContract` and cannot carry `ViewPartialOutput`.
- Fake renderer covers deterministic request ids, build success, build failure, apply success, apply failure with partial output, stale build result, and stale apply result.
- Fake renderer tracks renderer-issued request/plan provenance and rejects foreign or already-consumed plans without creating partial output.
- Fake renderer rejects forced apply calls in build-failure or build-stale modes without creating partial output.
- Apply failure reports apply-created partial output; build failure, stale build result, and stale apply result report no partial output.
- Fake renderer output is not active UI; `ViewApplyOutcome::Applied` and `ViewPartialOutput` are pre-finalization and pre-commit observations only.
- Phase 3B handoff uses core-issued `FrameApplyRequest` as the canonical handoff unit; non-fake adapters report renderer observations through a controlled reporter/session minted from that request; final stale acceptance remains inside core.
- Phase 3A does not define or export public fake-renderer methods or authority traits that can accept core apply tokens, finalize ECS output, or commit a core active branch.
- Fake renderer is exposed only when `test-support` or facade `view-test-support` is enabled.
- Phase 3A does not expose `NavigationTransaction`, `NavigationTransactionId`, `FrameAllocator`, frame identity construction, generation advancement, apply-token construction, cleanup-scope construction, lifecycle diagnostic construction, accept/finalize paths, or commit authority. Existing public observation types remain observation-only.
- Public constructors for request ids, output ids, build requests, render plans, partial outputs, and view failures remain crate-private.
- Facade default prelude exposes only stable Phase 3A user-facing view binding declarations under the default `view` feature.
- Facade `view` module explicit paths expose stable Phase 3A protocol observation records without putting them in the default prelude.
- Facade no-default checks remain passing without requiring view exports.
- Facade fake-renderer exposure is covered by `view-test-support`.
- Final verification record lists commands, pass counts, review result, unresolved risks, and Phase 3B handoff.
