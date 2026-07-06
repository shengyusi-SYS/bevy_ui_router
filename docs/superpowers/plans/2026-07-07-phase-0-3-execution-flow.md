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
- Phase 3 plan: view adapter, ECS integration, immediate BSN adapter, minimal example.

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
- Commit at phase checkpoints and at meaningful sub-phase boundaries.
- If a phase reveals a design contradiction, pause implementation and update the planning docs before continuing.
- Adapter work may only use public APIs; it must not construct or forge frame identity, generation, or commit tokens.
- Verification claims require fresh command output.

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

## 6. Task Flow

### Task 1: Create Phase 0 Detailed Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-0-contract-freeze-implementation-plan.md`
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-review-log.md`

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
```

Expected: no matches.

- [ ] **Step 4: Ask for execution mode**

Offer:

```text
Phase 0 plan is ready. Execution options:
1. Subagent-Driven: one worker per bounded task, with review between tasks.
2. Inline Execution: execute in this session with checkpoints.
```

Expected: user chooses an execution mode before implementation starts.

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

Expected: each task follows failing test, implementation, passing test, commit or checkpoint.

- [ ] **Step 3: Run Phase 0 verification**

Run the commands required by the Phase 0 plan. The minimum baseline is:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo check -p bevy_ui_router --no-default-features
```

Expected: all commands pass, or every skipped/failed command is explained with evidence.

- [ ] **Step 4: Phase 0 review gate**

Confirm:

- core has no Bevy dependency;
- facade default does not pull Bevy unless a documented decision changed it;
- diagnostics can express router, host, slot, frame, generation;
- no BSN or app-specific concepts entered core.

Expected: Phase 0 can be marked complete only after this gate passes.

### Task 3: Create Phase 1 Detailed Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-1-route-tree-branch-model-implementation-plan.md`
- Read: Phase 0 implementation result
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`

- [ ] **Step 1: Confirm Phase 0 completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 0 commit exists and worktree is clean.

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
```

Expected: no matches.

### Task 4: Execute Phase 1

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-1-route-tree-branch-model-implementation-plan.md`
- Modify: files listed by the Phase 1 plan

- [ ] **Step 1: Execute Phase 1 task-by-task**

Expected: route declaration and matcher behavior are built through contract tests first.

- [ ] **Step 2: Run Phase 1 verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo check -p bevy_ui_router --no-default-features
```

Expected: matcher, branch diff, and fingerprint tests pass.

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
- Read: Phase 1 implementation result
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`

- [ ] **Step 1: Confirm Phase 1 completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 1 commit exists and worktree is clean.

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
```

Expected: no matches.

### Task 6: Execute Phase 2

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-2-frame-error-transaction-implementation-plan.md`
- Modify: files listed by the Phase 2 plan

- [ ] **Step 1: Execute Phase 2 task-by-task**

Expected: frame and transaction behavior are built through failing tests before implementation.

- [ ] **Step 2: Run Phase 2 verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo check -p bevy_ui_router --no-default-features
```

Expected: lifecycle, transaction, stale rejection, and error boundary tests pass.

- [ ] **Step 3: Phase 2 review gate**

Confirm:

- stale results cannot produce visible state changes;
- old active branch survives failed navigation until replacement commits;
- failed transaction cleanup only touches its own partial output;
- adapter cannot construct or forge frame id/generation.

Expected: Phase 3 planning starts only after this gate passes.

### Task 7: Create Phase 3 Detailed Plan

**Files:**
- Create: `docs/superpowers/plans/2026-07-07-phase-3-view-ecs-bsn-implementation-plan.md`
- Read: Phase 2 implementation result
- Read: `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`

- [ ] **Step 1: Confirm Phase 2 completion evidence**

Run:

```powershell
git log -1 --oneline
git status --short
```

Expected: Phase 2 commit exists and worktree is clean.

- [ ] **Step 2: Write the Phase 3 plan**

Create a detailed plan split into:

- Phase 3A: view contract and fake/test renderer;
- Phase 3B: ECS host, slot, frame root marker, schedule sets, cleanup and finalization;
- Phase 3C: immediate BSN adapter;
- Phase 3D: minimal nested routes example.

- [ ] **Step 3: Phase 3 plan self-review**

Run:

```powershell
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME','待'+'定','未'+'定','稍'+'后','implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3-view-ecs-bsn-implementation-plan.md'
```

Expected: no matches.

### Task 8: Execute Phase 3

**Files:**
- Follow: `docs/superpowers/plans/2026-07-07-phase-3-view-ecs-bsn-implementation-plan.md`
- Modify: files listed by the Phase 3 plan

- [ ] **Step 1: Execute Phase 3A**

Expected: fake renderer proves build/apply success, failure cleanup, and active commit ordering.

- [ ] **Step 2: Execute Phase 3B**

Expected: ECS ownership cleanup cannot touch app-owned host; focus/pickability/visibility finalization is testable.

- [ ] **Step 3: Execute Phase 3C**

Expected: immediate BSN adapter maps BSN failures into Router errors and uses public APIs only.

- [ ] **Step 4: Execute Phase 3D**

Expected: minimal nested routes example asserts active branch, frame markers, generation, and diagnostics.

- [ ] **Step 5: Run Phase 3 verification**

Minimum baseline:

```powershell
cargo fmt --all --check
cargo test -p bevy_ui_router_core
cargo test -p bevy_ui_router_view
cargo test -p bevy_ui_router_ecs --features ecs
cargo check -p bevy_ui_router --features bsn
```

Expected: all commands pass, or any environment-limited command is documented with exact failure output and reason.

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

## 7. Subagent Strategy

Use subagents only after a phase plan defines disjoint ownership.

Recommended split:

- Phase 0: one worker for workspace/facade, one worker for core ids/location/history, one worker for diagnostics/tests.
- Phase 1: one worker for route tree builder, one worker for matcher, one worker for branch diff/fingerprint tests.
- Phase 2: one worker for frame/generation, one worker for transaction/stale gates, one worker for error boundary/fake harness.
- Phase 3: one worker each for 3A, 3B, 3C, and 3D.

Each worker must return:

- files changed;
- tests added;
- commands run;
- risks or deviations;
- whether the worker touched only its assigned files.

The lead agent must review diffs and run phase verification before accepting worker output.

---

## 8. Commit Strategy

Use small commits:

- one commit for each detailed phase plan;
- one or more commits for each phase implementation, grouped by stable behavior;
- one commit for docs updates after each phase gate;
- no mixed commits that combine unrelated phase work.

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

## 9. Verification Discipline

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
