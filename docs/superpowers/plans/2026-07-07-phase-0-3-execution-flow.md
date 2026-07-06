# Bevy UI Router Phase 0-3 Execution Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the reviewed architecture roadmap into a controlled Phase 0-3 execution process with clear phase plans, review gates, verification, and commit boundaries.

**Architecture:** Use a two-layer execution model: this document controls the overall process, while each phase gets its own detailed implementation plan before code changes begin. Phase 0-3 must run sequentially because each phase freezes contracts consumed by the next phase; within a phase, independent work may be delegated only after the phase plan defines disjoint file ownership.

**Tech Stack:** Rust 2024, Cargo workspace, Bevy 0.19 custom git branch for Bevy-dependent crates, Markdown planning docs, Git, `cargo fmt`, `cargo check`, `cargo test`.

---

## 1. Scope Check

The reviewed roadmap covers multiple subsystems: workspace structure, core route model, lifecycle state, renderer contract, ECS integration, BSN adapter, diagnostics, and tests. This is too broad for one monolithic implementation plan.

Execution must be split into phase-specific plans:

- Phase 0 plan: contract freeze and workspace/core baseline.
- Phase 1 plan: route tree, matcher, branch model.
- Phase 2 plan: frame state, transaction, error boundary.
- Phase 3A plan: view contract and fake/test renderer.
- Phase 3B plan: ECS integration, host/slot/frame root, cleanup and finalization.
- Phase 3C plan: immediate BSN adapter.
- Phase 3D plan: minimal nested routes example.

This document is the process and task-flow plan. It does not authorize code implementation by itself.

---

## 2. Source Documents

Implementation workers must read these files before writing a phase plan or code:

- `AGENTS.md`
- `docs/superpowers/specs/2026-07-07-project-decisions-design.md`
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-review-log.md`
- `docs/01-BSN-scene-authoring-boundaries.md`
- `docs/02-Web-router-patterns-for-Bevy.md`
- `docs/03-Core-architecture-and-MVP.md`
- `docs/04-Risks-tests-and-handoff.md`
- `docs/05-Non-MVP-capabilities-in-Bevy.md`

---

## 3. Execution Rules

- Do not start implementation code until the current phase has a detailed implementation plan.
- Do not execute Phase 1 before Phase 0 is complete and verified.
- Do not execute Phase 2 before Phase 1 is complete and verified.
- Do not execute Phase 3 before Phase 2 is complete and verified.
- Keep core independent from Bevy, BSN, platform APIs, and app-specific concepts.
- Use TDD for behavior contracts: failing test, minimal implementation, passing test, commit.
- Commit at phase gates and meaningful sub-phase gates. A checkpoint may be used for local progress notes, but it cannot replace a commit at a phase or sub-phase gate.
- If a phase reveals a design contradiction, pause implementation and update the planning docs before continuing. Execution may resume only after the revised plan passes the same review flow that found the contradiction.
- Adapter work may only use public APIs; it must not construct or forge frame identity, generation, or commit tokens.
- Verification claims require fresh command output.
- Required verification command failures block phase completion. A command that cannot run because of environment limits marks the gate as unverified or blocked unless the user explicitly accepts the risk in writing.
- Every detailed phase plan must pass review before implementation starts. A self-review scan is not enough to authorize implementation.

---

## 4. Planned File Ownership

The exact file list is frozen by each phase plan. The expected target structure is:

```text
Cargo.toml
crates/bevy_ui_router_core/Cargo.toml
crates/bevy_ui_router_core/src/lib.rs
crates/bevy_ui_router_core/src/location.rs
crates/bevy_ui_router_core/src/ids.rs
crates/bevy_ui_router_core/src/history.rs
crates/bevy_ui_router_core/src/diagnostics.rs
crates/bevy_ui_router_core/src/route_tree.rs
crates/bevy_ui_router_core/src/matcher.rs
crates/bevy_ui_router_core/src/branch.rs
crates/bevy_ui_router_core/src/frame.rs
crates/bevy_ui_router_core/src/error.rs
crates/bevy_ui_router_view/Cargo.toml
crates/bevy_ui_router_view/src/lib.rs
crates/bevy_ui_router_ecs/Cargo.toml
crates/bevy_ui_router_ecs/src/lib.rs
crates/bevy_ui_router_bsn/Cargo.toml
crates/bevy_ui_router_bsn/src/lib.rs
crates/bevy_ui_router_platform/Cargo.toml
crates/bevy_ui_router_platform/src/lib.rs
crates/bevy_ui_router/Cargo.toml
crates/bevy_ui_router/src/lib.rs
examples/minimal_nested_routes.rs
```

If implementation discovers a better layout, update the active phase plan first and record the reason.

---

## 5. Phase Gate Summary

| Phase | Purpose | Blocking Gate |
| --- | --- | --- |
| Phase 0 | Workspace, core isolation, raw location, ids, memory history, diagnostics baseline | core compiles without Bevy; facade default does not pull Bevy; identity chain is testable |
| Phase 1 | Route tree, matcher, branch diff, stable ordering | matcher and branch diff contract tests pass; raw fingerprint rules are frozen |
| Phase 2 | Frame state, transaction, stale gates, error boundary | stale result cannot mutate visible state; old active survives failed transaction; adapter cannot forge identity |
| Phase 3A | View contract and fake renderer | fake renderer proves build/apply success and failure paths |
| Phase 3B | ECS host/slot/frame root and cleanup | router-owned cleanup cannot touch app-owned host |
| Phase 3C | Immediate BSN adapter | BSN errors map to Router errors; adapter uses public APIs only |
| Phase 3D | Minimal nested routes example | example asserts active branch, frame markers, generation, diagnostics |

---

## 6. Gate Evidence Model

Every detailed plan and execution gate must produce auditable evidence.

### 6.1 Plan Acceptance Gate

Before implementation starts for any phase or sub-phase:

- the detailed plan exists;
- the plan has passed placeholder, trailing whitespace, and `git diff --check` checks;
- the plan has been reviewed against the roadmap and review log;
- any requested split or refinement has been applied;
- the user has confirmed the execution mode or continuation path.

### 6.2 TDD Evidence Ledger

Each implementation task must record:

- task id and files changed;
- RED command and exit code;
- RED output summary proving the test failed for the expected reason;
- GREEN command and exit code;
- GREEN output summary proving the target behavior passed;
- commit SHA or explicit reason the task is still before a commit boundary;
- any skipped command and whether that makes the gate blocked.

Tests added after implementation do not count as TDD evidence unless the RED failure was observed and recorded before the implementation change.

### 6.3 Phase Gate Evidence

Each phase or sub-phase gate must record:

- final commit SHA;
- verification commands and exit codes;
- review gate checklist result;
- unresolved risks;
- user-accepted exceptions, if any;
- next phase or sub-phase authorized by the lead.

`git log -1 --oneline` and `git status --short` are not enough by themselves. They only support the evidence record; they do not replace it.

### 6.4 Failed Or Unavailable Verification

Required command outcomes:

- pass: the gate may proceed if all checklist items also pass;
- fail: the gate is blocked;
- unavailable because target crate does not exist yet: the active plan must identify the task that creates it and the later step that runs the command;
- unavailable because of environment or network limits: the gate is unverified or blocked unless the user explicitly accepts the risk.

---

## 7. Task Flow

### Task 0: Create Implementation Readiness Brief

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-implementation-readiness-brief.md`
- Read: all source documents listed in Section 2

- [ ] **Step 1: Create the readiness brief**

The brief must contain these implementation-start inputs:

- Public API draft scope for Phase 0.
- State machine diagram scope for Phase 0-2.
- Error taxonomy finalization target.
- Adapter responsibility table scope.
- Test harness outline.
- Minimal examples outline.
- Open decisions that Phase 0 must freeze.

- [ ] **Step 2: Run document checks**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-implementation-readiness-brief.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-implementation-readiness-brief.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 3: Readiness review gate**

The readiness brief must be reviewed before Task 1 begins.

Expected: the reviewer confirms the brief does not open new implementation scope and only prepares Phase 0 planning inputs.

### Task 1: Create Phase 0 Detailed Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-0-contract-freeze-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: `docs/superpowers/plans/2026-07-07-implementation-readiness-brief.md`

- [ ] **Step 1: Re-read the planning baseline**

Run:

```powershell
Get-Content -Encoding UTF8 -LiteralPath 'C:\projects\bevy_ui_router\docs\superpowers\specs\2026-07-07-project-architecture-roadmap-plan.md'
```

Expected: the file states it is an approved planning baseline and lists Phase 0 scope.

- [ ] **Step 2: Write the Phase 0 plan**

Create `docs/superpowers/plans/2026-07-07-phase-0-contract-freeze-implementation-plan.md` with the required implementation-plan header and tasks for:

- workspace migration;
- core crate creation;
- facade crate creation;
- core id types;
- raw `RouteLocation`;
- memory history;
- diagnostics baseline;
- dependency boundary tests;
- pending overwrite and missing host/slot diagnostics fixtures;
- final Phase 0 verification.

- [ ] **Step 3: Self-review the Phase 0 plan**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-0-contract-freeze-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-0-contract-freeze-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 0 plan acceptance gate**

Review the Phase 0 plan against:

- the roadmap Phase 0 scope;
- the readiness brief;
- the review log must-fix decisions;
- the evidence model in Section 6.

Expected: Phase 0 implementation does not begin until this plan acceptance gate passes.

- [ ] **Step 5: Ask for execution mode**

Offer:

```text
Phase 0 plan is ready. Execution options:
1. Subagent-Driven: one worker per bounded task, with review between tasks.
2. Inline Execution: execute in this session with checkpoints.
```

Expected: user chooses an execution mode after the Phase 0 plan is accepted and before implementation starts.

### Task 2: Execute Phase 0

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-0-contract-freeze-implementation-plan.md`
- Modify: files listed by the Phase 0 plan

- [ ] **Step 1: Start from a clean worktree**

Run:

```powershell
git status --short
```

Expected: no unrelated changes, or unrelated changes are explicitly noted and ignored.

- [ ] **Step 2: Execute the Phase 0 plan task-by-task**

Use `superpowers:subagent-driven-development` or `superpowers:executing-plans`.

Expected: each task records RED evidence, implementation summary, GREEN evidence, and commit boundary status in the TDD evidence ledger.

- [ ] **Step 3: Run Phase 0 verification**

Run the commands required by the Phase 0 plan. The minimum baseline is:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo check -p bevy_ui_router --no-default-features
```

Expected: all required commands pass. Failed commands block Phase 0 completion. Environment-limited commands mark the gate unverified or blocked unless the user explicitly accepts the risk in writing.

- [ ] **Step 4: Phase 0 review gate**

Confirm:

- core has no Bevy dependency;
- facade default does not pull Bevy unless a documented decision changed it;
- diagnostics can express router, host, slot, frame, generation;
- no BSN or app-specific concepts entered core.

Expected: Phase 0 can be marked complete only after this gate passes and Phase 0 gate evidence is recorded.

### Task 3: Create Phase 1 Detailed Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-1-route-tree-branch-model-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: Phase 0 implementation result and Phase 0 gate evidence

- [ ] **Step 1: Confirm Phase 0 completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 0 commit exists and worktree is clean.

Also confirm Phase 0 gate evidence records final verification commands, exit codes, review checklist, unresolved risks, and authorized continuation to Phase 1.

- [ ] **Step 2: Write the Phase 1 plan**

Create a detailed plan for:

- builder-first `RouteTree`;
- route ids and declaration order;
- static, param, catch-all, index, nested, pathless routes;
- named slots;
- error boundary declaration;
- `MatchedBranch`;
- raw params/search fingerprint;
- common-prefix branch diff;
- stable ordering tests.

- [ ] **Step 3: Phase 1 plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-1-route-tree-branch-model-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-1-route-tree-branch-model-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 1 plan acceptance gate**

Review the Phase 1 plan against Phase 0 gate evidence, roadmap Phase 1 scope, and Section 6 evidence requirements.

Expected: Phase 1 implementation does not begin until this plan acceptance gate passes.

### Task 4: Execute Phase 1

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-1-route-tree-branch-model-implementation-plan.md`
- Modify: files listed by the Phase 1 plan

- [ ] **Step 1: Execute Phase 1 task-by-task**

Expected: route declaration and matcher behavior are built through contract tests first, with RED and GREEN evidence recorded.

- [ ] **Step 2: Run Phase 1 verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo check -p bevy_ui_router --no-default-features
```

Expected: matcher, branch diff, and fingerprint tests pass.

Failed required commands block Phase 1 completion. Environment-limited commands mark the gate unverified or blocked unless the user explicitly accepts the risk in writing.

- [ ] **Step 3: Phase 1 review gate**

Confirm:

- matching order is deterministic;
- branch diff preserves common prefix;
- matched branch is not treated as active rendered UI;
- app payload does not silently affect fingerprint.

Expected: Phase 2 planning starts only after this gate passes.

### Task 5: Create Phase 2 Detailed Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: Phase 1 implementation result and Phase 1 gate evidence

- [ ] **Step 1: Confirm Phase 1 completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 1 commit exists and worktree is clean.

Also confirm Phase 1 gate evidence records matcher, branch diff, fingerprint, stable ordering verification, unresolved risks, and authorized continuation to Phase 2.

- [ ] **Step 2: Write the Phase 2 plan**

Create a detailed plan for:

- `RouteFrameId`;
- `RouteGeneration`;
- frame records;
- frame id reuse rules;
- generation increment rules;
- navigation transaction;
- pre-apply stale gate;
- pre-commit identity recheck;
- nearest error boundary resolution;
- fake apply-result harness;
- adapter identity visibility or compile-fail tests.

- [ ] **Step 3: Phase 2 plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-2-frame-error-transaction-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-2-frame-error-transaction-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 2 plan acceptance gate**

Review the Phase 2 plan against Phase 1 gate evidence, roadmap Phase 2 scope, and Section 6 evidence requirements.

Expected: Phase 2 implementation does not begin until this plan acceptance gate passes.

### Task 6: Execute Phase 2

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md`
- Modify: files listed by the Phase 2 plan

- [ ] **Step 1: Execute Phase 2 task-by-task**

Expected: frame and transaction behavior are built through failing tests before implementation, with RED and GREEN evidence recorded.

- [ ] **Step 2: Run Phase 2 verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo check -p bevy_ui_router --no-default-features
```

Expected: lifecycle, transaction, stale rejection, and error boundary tests pass.

Failed required commands block Phase 2 completion. Environment-limited commands mark the gate unverified or blocked unless the user explicitly accepts the risk in writing.

- [ ] **Step 3: Phase 2 review gate**

Confirm:

- stale results cannot produce visible state changes;
- old active branch survives failed navigation until replacement commits;
- failed transaction cleanup only touches its own partial output;
- adapter cannot construct or forge frame id/generation.

Expected: Phase 3A planning starts only after this gate passes and Phase 2 gate evidence is recorded.

### Task 7A: Create Phase 3A View Contract Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-3a-view-contract-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: Phase 2 implementation result and Phase 2 gate evidence

- [ ] **Step 1: Confirm Phase 2 completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 2 commit exists and worktree is clean.

Also confirm Phase 2 gate evidence records frame lifecycle, transaction, stale gate, error boundary verification, unresolved risks, and authorized continuation to Phase 3A.

- [ ] **Step 2: Write the Phase 3A plan**

Create a detailed plan for:

- Phase 3A: view contract and fake/test renderer;
- renderer-agnostic view descriptor/result contracts;
- fake renderer build/apply success and failure paths;
- failure cleanup reporting;
- active commit ordering tests.

- [ ] **Step 3: Phase 3A plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3a-view-contract-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3a-view-contract-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 3A plan acceptance gate**

Review the Phase 3A plan against Phase 2 gate evidence, roadmap Phase 3A scope, and Section 6 evidence requirements.

Expected: Phase 3A implementation does not begin until this plan acceptance gate passes.

### Task 8A: Execute Phase 3A

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-3a-view-contract-implementation-plan.md`
- Modify: files listed by the Phase 3A plan

- [ ] **Step 1: Execute Phase 3A**

Expected: fake renderer proves build/apply success, failure cleanup, and active commit ordering, with RED and GREEN evidence recorded.

- [ ] **Step 2: Run Phase 3A verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_view
cargo test -p bevy_ui_router_core
```

Expected: all required commands pass. Failed commands block Phase 3A completion.

- [ ] **Step 3: Phase 3A review gate**

Confirm:

- fake renderer tests cover success and failure paths;
- adapter-facing contracts do not expose forgeable identity;
- active commit ordering is tested.

Expected: Phase 3B planning starts only after this gate passes and Phase 3A gate evidence is recorded.

### Task 7B: Create Phase 3B ECS Integration Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: Phase 3A implementation result and Phase 3A gate evidence

- [ ] **Step 1: Confirm Phase 3A completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 3A commit exists and worktree is clean.

Also confirm Phase 3A gate evidence authorizes continuation to Phase 3B.

- [ ] **Step 2: Write the Phase 3B plan**

Create a detailed plan for:

- ECS plugin setup;
- `RouteHost`;
- `RouteSlot`;
- frame root marker;
- schedule sets;
- controlled cleanup API;
- visibility, pickability, and focus finalization;
- ownership tests proving app-owned host is not touched.

- [ ] **Step 3: Phase 3B plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3b-ecs-integration-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3b-ecs-integration-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 3B plan acceptance gate**

Review the Phase 3B plan against Phase 3A gate evidence, roadmap Phase 3B scope, and Section 6 evidence requirements.

Expected: Phase 3B implementation does not begin until this plan acceptance gate passes.

### Task 8B: Execute Phase 3B

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md`
- Modify: files listed by the Phase 3B plan

- [ ] **Step 1: Execute Phase 3B task-by-task**

Expected: ECS ownership, cleanup, and finalization behavior are built through tests first, with RED and GREEN evidence recorded.

- [ ] **Step 2: Run Phase 3B verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_ecs --features ecs
```

If Phase 0 or Phase 3B freezes a different feature name, run the equivalent ECS integration test command recorded in the Phase 3B plan.

Expected: all required commands pass. Failed commands block Phase 3B completion.

- [ ] **Step 3: Phase 3B review gate**

Confirm:

- router-owned cleanup does not touch app-owned host;
- frame root markers include router, host, slot, frame, generation, and route identity;
- focus, pickability, and visibility finalization has tests.

Expected: Phase 3C planning starts only after this gate passes and Phase 3B gate evidence is recorded.

### Task 7C: Create Phase 3C Immediate BSN Adapter Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-3c-bsn-adapter-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: Phase 3B implementation result and Phase 3B gate evidence

- [ ] **Step 1: Confirm Phase 3B completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 3B commit exists and worktree is clean.

Also confirm Phase 3B gate evidence authorizes continuation to Phase 3C.

- [ ] **Step 2: Write the Phase 3C plan**

Create a detailed plan for:

- immediate code-driven BSN adapter;
- BSN resolve/apply error mapping;
- partial output reporting;
- controlled cleanup API use;
- proof that adapter only uses public Router/ECS APIs.

- [ ] **Step 3: Phase 3C plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3c-bsn-adapter-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3c-bsn-adapter-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 3C plan acceptance gate**

Review the Phase 3C plan against Phase 3B gate evidence, roadmap Phase 3C scope, and Section 6 evidence requirements.

Expected: Phase 3C implementation does not begin until this plan acceptance gate passes.

### Task 8C: Execute Phase 3C

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-3c-bsn-adapter-implementation-plan.md`
- Modify: files listed by the Phase 3C plan

- [ ] **Step 1: Execute Phase 3C task-by-task**

Expected: immediate BSN adapter behavior is built through tests or integration checks first, with RED and GREEN evidence recorded. If RED/GREEN evidence is not feasible because of BSN API or environment limits, record the exact reason and mark the item unverified or blocked under Section 6.4 unless the user explicitly accepts the risk in writing.

- [ ] **Step 2: Run Phase 3C verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo check -p bevy_ui_router --features bsn
cargo test -p bevy_ui_router_bsn --features bsn
```

If Phase 0 or Phase 3C freezes different feature names or package names, run the equivalent BSN adapter check and test commands recorded in the Phase 3C plan.

Expected: all required commands pass. Failed commands block Phase 3C completion. If BSN cannot be tested because of missing upstream/custom API availability, the gate is unverified or blocked unless the user explicitly accepts the risk in writing.

- [ ] **Step 3: Phase 3C review gate**

Confirm:

- BSN failures map to Router errors;
- adapter does not own matching, history, lifecycle, cleanup authority, or platform policy;
- adapter only uses public APIs.

Expected: Phase 3D planning starts only after this gate passes and Phase 3C gate evidence is recorded.

### Task 7D: Create Phase 3D Minimal Example Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-3d-minimal-example-implementation-plan.md`
- Read: all source documents listed in Section 2
- Read: Phase 3C implementation result and Phase 3C gate evidence

- [ ] **Step 1: Confirm Phase 3C completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 3C commit exists and worktree is clean.

Also confirm Phase 3C gate evidence authorizes continuation to Phase 3D.

- [ ] **Step 2: Write the Phase 3D plan**

Create a detailed plan for:

- neutral minimal nested routes example;
- fake renderer path assertion;
- BSN adapter path assertion;
- active branch assertion;
- frame marker assertion;
- generation assertion;
- diagnostics assertion.

- [ ] **Step 3: Phase 3D plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3d-minimal-example-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3d-minimal-example-implementation-plan.md'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

- [ ] **Step 4: Phase 3D plan acceptance gate**

Review the Phase 3D plan against Phase 3C gate evidence, roadmap Phase 3D scope, and Section 6 evidence requirements.

Expected: Phase 3D implementation does not begin until this plan acceptance gate passes.

### Task 8D: Execute Phase 3D

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-3d-minimal-example-implementation-plan.md`
- Modify: files listed by the Phase 3D plan

- [ ] **Step 1: Execute Phase 3D task-by-task**

Expected: minimal nested routes example behavior is built with assertions for active branch, frame markers, generation, and diagnostics.

- [ ] **Step 2: Run Phase 3D verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test --example minimal_nested_routes --features bsn
cargo run --example minimal_nested_routes --features bsn
```

If the example is implemented as an integration test instead of a runnable example, run the equivalent command recorded in the Phase 3D plan.

Expected: all required commands pass. Failed commands block Phase 3D completion.

- [ ] **Step 3: Phase 3D review gate**

Confirm:

- example uses neutral route/page names;
- example does not introduce app-specific assumptions;
- fake renderer and BSN adapter paths are covered or explicitly separated with evidence;
- diagnostics and frame marker assertions are present.

Expected: Phase 0-3 completion review starts only after this gate passes and Phase 3D gate evidence is recorded.

### Task 9: Phase 0-3 Completion Review

**Files:**
- Read: all Phase 0-3 plan files
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
- Modify: release or handoff notes only if requested

- [ ] **Step 1: Verify phase evidence**

Run:

```powershell
git log --oneline -10
git status --short
```

Expected: all Phase 0-3 commits are visible and worktree is clean.

Also review every Phase 0, Phase 1, Phase 2, Phase 3A, Phase 3B, Phase 3C, and Phase 3D gate evidence artifact. Each artifact must include final commit SHA, verification commands and exit codes, review checklist result, unresolved risks, and authorization for the next phase or sub-phase.

- [ ] **Step 2: Produce completion summary**

The summary must include:

- implemented phases;
- verification commands and outcomes;
- skipped verification with reasons;
- remaining Phase 4-6 roadmap items;
- known risks, especially custom Bevy git dependency and upstream BSN migration.

- [ ] **Step 3: Decide next planning target**

Choose one:

- write Phase 4 loader/guard/revalidate plan;
- stabilize Phase 0-3 API and examples;
- prepare release/CI plan;
- pause for external review.

Expected: no Phase 4 implementation begins without a new detailed plan.

---

## 8. Subagent Strategy

Use subagents only after a phase plan defines disjoint ownership.

Recommended split:

- Phase 0: one worker for workspace/facade, one worker for core ids/location/history, one worker for diagnostics/tests.
- Phase 1: one worker for route tree builder, one worker for matcher, one worker for branch diff/fingerprint tests.
- Phase 2: one worker for frame/generation, one worker for transaction/stale gates, one worker for error boundary/fake harness.
- Phase 3: one worker each for 3A, 3B, 3C, and 3D after each sub-phase plan is accepted.

Each worker must return:

- files changed;
- tests added;
- RED command, exit code, and output summary;
- GREEN command, exit code, and output summary;
- full verification commands, exit codes, and output summaries;
- commit SHA for the accepted task or explicit note that the task has not reached a commit boundary;
- failed commands, if any, clearly marked as blocking;
- skipped commands and whether they make the gate unverified or blocked;
- risks or deviations;
- whether the worker touched only its assigned files.

The lead agent must review diffs and run phase verification before accepting worker output.

---

## 9. Commit Strategy

Use small commits:

- one commit for each detailed phase plan;
- one or more commits for each phase implementation, grouped by stable behavior;
- one commit for docs updates after each phase gate;
- no mixed commits that combine unrelated phase work.

Checkpoint rules:

- A checkpoint is a local progress marker or lead-agent review point.
- A checkpoint may not replace a commit at a phase gate or sub-phase gate.
- A checkpoint may not be used to claim a phase is complete.
- If work pauses at a checkpoint, the final response must say which verification remains unfinished.

Suggested commit messages:

```text
docs: plan phase 0 contract freeze
chore: scaffold router workspace
feat(core): add route location and ids
feat(core): add memory history
feat(core): add diagnostics baseline
docs: record phase 0 completion
```

---

## 10. Verification Discipline

Before any completion claim, run the relevant verification commands and read the output.

Minimum document checks for every plan document:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern '<plan-file>'
rg -n "[ \t]+$" '<plan-file>'
git diff --check
```

Expected: no placeholder matches, no trailing whitespace, and `git diff --check` exits successfully.

Minimum Rust checks are phase-specific and listed above. If a command cannot run because the workspace or crate does not exist yet, the active phase plan must say which task creates it and which later step runs the command.

Required verification failures block completion. Explanations are useful for diagnosis, but they do not convert a failed command into a passed gate. Environment-limited verification must be reported as unverified or blocked unless the user explicitly accepts the risk in writing.
