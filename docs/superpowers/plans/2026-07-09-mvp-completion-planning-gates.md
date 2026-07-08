# MVP Completion Planning Gates

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:writing-plans` when producing each detailed phase plan. Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` only when an accepted implementation plan explicitly authorizes execution.

**Goal:** Complete planning coverage from the accepted Phase 3B plan to MVP completion without guessing Phase 3C or Phase 3D implementation details before their prerequisite evidence exists.

**Architecture:** MVP completion remains a staged pipeline. Phase 3B proves the controlled core/ECS handoff. Phase 3C may add the immediate BSN adapter only through the Phase 3B public ECS API and controlled reporter/session. Phase 3D may add the neutral minimal nested routes example only after Phase 3C gate evidence proves the BSN adapter boundary.

**Tech Stack:** Rust 2024, existing Cargo workspace, `bevy_ui_router_core`, `bevy_ui_router_view`, `bevy_ui_router_ecs`, `bevy_ui_router_bsn`, `bevy_ui_router_platform`, facade feature gates, custom project-approved Bevy source, `cargo fmt`, `cargo test`, `cargo check`, compile-fail boundary tests, authority scans, multi-agent review.

---

## Current State

- Current branch: `main`.
- Latest planning commit before this document: `06c467f docs: plan phase 3b ecs integration`.
- Accepted Phase 3B plan: `docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md`.
- Phase 3A implementation and verification record are already committed.
- Phase 3B implementation has not been executed in this planning step.
- During review of this document, this file itself is the only expected untracked worktree item. Every phase execution gate below still requires a clean worktree or an explicit unrelated-change note.
- Detailed Phase 3C and Phase 3D implementation plans must not be written as final executable plans until the required prior gate evidence exists.
- This document supplements the existing Phase 0-3 execution flow. It does not rename, supersede, or narrow that execution flow unless a later commit explicitly updates both documents.

---

## Source Documents

Every remaining MVP planning step must read these documents before drafting:

- `AGENTS.md`
- `docs/01-BSN-scene-authoring-boundaries.md`
- `docs/02-Web-router-patterns-for-Bevy.md`
- `docs/03-Core-architecture-and-MVP.md`
- `docs/04-Risks-tests-and-handoff.md`
- `docs/05-Non-MVP-capabilities-in-Bevy.md`
- `docs/superpowers/specs/2026-07-07-project-decisions-design.md`
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-review-log.md`
- `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md`
- `docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md`

Each detailed phase plan must additionally read the implementation result and gate evidence from the immediately preceding phase:

- Phase 3C plan reads Phase 3B implementation result and Phase 3B gate evidence.
- Phase 3D plan reads Phase 3C implementation result and Phase 3C gate evidence.
- MVP completion review reads Phase 0 through Phase 3D gate evidence.

This sequencing is a hard planning boundary. It prevents Phase 3C and Phase 3D plans from inventing file layout, API names, test commands, or unresolved-risk decisions that Phase 3B or Phase 3C implementation has not yet proven.

---

## MVP Module Boundaries

### Core

`bevy_ui_router_core` owns route matching data, branch planning data, frame identity, generation, navigation transaction state, stale checks, active commit authority, cleanup acknowledgement, and diagnostics contracts.

Core must not depend on Bevy, ECS, BSN, platform APIs, theme, copy, permissions, data services, app shell policy, or concrete host applications.

### View

`bevy_ui_router_view` owns renderer-neutral view descriptors, renderer outcomes, fake/test renderer support, and request-bound reporting contracts introduced by Phase 3A and Phase 3B.

View contracts may help adapters report renderer observations. They must not expose active commit, cleanup authority, stale acceptance, generation advancement, frame identity forging, or app-owned host mutation authority.

### ECS

`bevy_ui_router_ecs` owns Bevy components, resources, schedule labels, router-owned hierarchy mutation, cleanup execution, finalization, input/focus isolation, and the controlled bridge back to core.

ECS may call public core runtime methods that keep stale checks and active commit authority inside core. ECS must not depend on BSN, platform crates, facade crates, app profiles, or specific applications.

### BSN

`bevy_ui_router_bsn` owns only the immediate code-driven BSN adapter path for MVP.

BSN must use Phase 3B ECS public API and controlled reporter/session. It must not construct frame identity, generation, cleanup scope, transaction tokens, active commit state, app-owned hosts, or router-owned cleanup plans.

BSN failures map to router/view errors and controlled cleanup flows. An immediate BSN scene output is never active UI until the Phase 3B core/ECS sequence accepts, commits, and activates it. Queued or asset-driven BSN scene apply remains future-only and must not be implemented by Phase 3C unless a later accepted plan changes MVP scope.

### Platform

`bevy_ui_router_platform` remains a thin adapter-glue boundary for router-neutral platform intents and capability snapshots.

Platform helpers are not required for Phase 3C or Phase 3D unless an accepted plan identifies a specific public API boundary that can be tested without app or OS assumptions.

### Facade And Examples

`bevy_ui_router` exposes feature-gated module and prelude surfaces without moving authority from the owning crates.

Examples prove integration behavior. They must use neutral route/page names, avoid app-specific services, and assert router behavior rather than relying on visual inspection alone.

---

## Canonical MVP Invariants

- Router only manages router-owned subtrees and never destroys, replaces, or rebuilds app-owned hosts.
- Pending scene output is not active UI.
- Active UI requires core-issued request identity, renderer success, ECS prepared-inert output, core stale acceptance, core active commit, and ECS post-commit activation.
- No code may hold `&mut World` across `await`.
- Async or deferred results must pass stale checks before apply; stale results must produce no visible state mutation.
- Resolve/apply failure cleanup touches only router-owned local state and is reported through explicit error channels.
- Hidden, retained, cached, leaving, failed, cleanup-pending, cleanup-targeted, preparing, prepared-inert, and pre-commit roots must not keep input or active focus.
- Stable ordering must use explicit identity keys, indices, or sorting rules, never Bevy query iteration order.
- Scene-local observers may handle local UI behavior but cannot become lifecycle authority.
- Adapters may affect Router only through public APIs and cannot forge frame identity, generation, cleanup scope, transaction tokens, or active commit state.
- External declarations, loaders, asset files, or incremental update mechanisms can be integration inputs but cannot become routing state source.
- Verification claims require observed command output.

---

## Remaining MVP Planning Pipeline

### Stage A: Execute Accepted Phase 3B Plan

**Input:** `docs/superpowers/plans/2026-07-07-phase-3b-ecs-integration-implementation-plan.md`

**Purpose:** Build and verify the core/ECS bridge required before BSN adapter planning can become concrete.

**Required outcome:**

- Phase 3B verification record appended to the accepted Phase 3B plan.
- The appended record contains these exact record markers:
  - `Phase 3B verification status: PASS`
  - `Phase 3B implementation commit:`
  - `Phase 3B review result: PASS`
  - `Phase 3C handoff statement:`
- Phase 3B review passes after lead-agent evaluation.
- Phase 3C handoff statement is present and still says BSN may use only Phase 3B ECS public API and controlled reporter/session.
- Worktree is clean.
- Final Phase 3B implementation commit exists.

**Blocks Phase 3C planning if:**

- Phase 3B does not prove real Bevy UI input/focus mapping, unless a later accepted plan explicitly narrows MVP scope before Phase 3C planning begins.
- Phase 3B cannot expose a controlled reporter/session for non-fake adapters.
- Phase 3B leaves core stale acceptance or active commit authority outside core.
- Phase 3B final evidence lacks exact command output or review result.
- Any Phase 3B item is recorded as unavailable or blocked and the user has not approved a reviewed MVP scope change. Exact blocked causes are required for honesty, but they do not satisfy the gate.

### Stage B: Create Phase 3C Immediate BSN Adapter Plan

**Target file:** `docs/superpowers/plans/2026-07-07-phase-3c-bsn-adapter-implementation-plan.md`

**Purpose:** Produce the executable implementation plan for the immediate code-driven BSN adapter after Phase 3B proves the ECS bridge API.

**Plan must cover:**

- BSN dependency and feature gate source decision.
- Immediate code-driven BSN scene apply.
- Route view binding from renderer-neutral descriptor to BSN adapter input.
- BSN build/resolve/apply success mapping into Phase 3B reporter/session.
- BSN build/resolve/apply failure mapping into route view/apply errors.
- Partial output and disposal hints reported through controlled Phase 3B APIs.
- Stale renderer results discarded without core report or active UI.
- Proof that BSN uses only adapter-facing public ECS/view APIs and controlled reporter/session paths. Direct BSN use of `RouterRuntime`, core active commit, cleanup acknowledgement, token acceptance, cleanup/finalization capabilities, or stale-acceptance authority is forbidden even if a low-level public core path exists.
- Compile-fail/API-boundary tests plus authority-surface scans or allow-lists proving BSN cannot construct frame identity, generation, cleanup scope, transaction tokens, or active commit state.
- Cargo verification for `bevy_ui_router_bsn` and facade `bsn` feature.
- Exact unverified or blocked handling if the custom Bevy/BSN API is unavailable.

**Pre-plan gate commands:**

```powershell
git log -1 --oneline
git status --short --branch
rg -n "^## Phase 3B Verification Record$|^Phase 3B verification status: PASS$|^Phase 3B implementation commit:|^Phase 3B review result: PASS$|^Phase 3C handoff statement:" docs\superpowers\plans\2026-07-07-phase-3b-ecs-integration-implementation-plan.md
rg -n "bevy_ui_router_bsn|bsn|BSN|queue_spawn_scene|Scene" Cargo.toml crates docs
```

Expected: the first `rg` command finds only the appended Phase 3B verification record and its exact marker lines, not Task 7 instructions or acceptance-matrix prose. The `git status --short --branch` output must show a clean worktree or explicitly named unrelated changes.

**Plan self-check commands:**

```powershell
$zhPending = [string]([char]0x5f85) + [string]([char]0x5b9a)
$zhUnset = [string]([char]0x672a) + [string]([char]0x5b9a)
$zhLater = [string]([char]0x7a0d) + [string]([char]0x540e)
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME',$zhPending,$zhUnset,$zhLater,'implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3c-bsn-adapter-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3c-bsn-adapter-implementation-plan.md'
git diff --check
```

Expected: placeholder scan and trailing whitespace scan return no matches; `rg` exits with code 1 for those no-match scans, and that no-match exit is the expected passing result. `git diff --check` exits with code 0.

**Review directions:**

- Architecture and lifecycle review: verify BSN cannot bypass Phase 3B core/ECS lifecycle and cannot make pending scene output active UI.
- API and authority review: verify BSN uses only adapter-facing public APIs and cannot forge identity, generation, cleanup scope, transaction token, cleanup/finalization capability, core runtime commit/report authority, or active commit.
- Fact and source review: verify every BSN and Bevy API claim against current source, Phase 3B verification evidence, `AGENTS.md`, and roadmap docs.
- Test and risk review: verify the planned tests can distinguish BSN build failure, apply failure, stale result, partial output cleanup, facade feature boundary, and dependency boundary.

The lead agent must evaluate review findings for shared root causes before accepting any suggested edits. If the root cause is Phase 3B API insufficiency, do not patch around it in BSN; return to the Phase 3B design or implementation boundary.

After review passes, ask independent agents whether the plan needs structural cleanup or semantic rewrite. If yes, create a separate rewrite candidate, validate it against the accepted content, replace only after lead-agent acceptance, then commit.

**Planning commit:**

```powershell
git add docs/superpowers/plans/2026-07-07-phase-3c-bsn-adapter-implementation-plan.md
git commit -m "docs: plan phase 3c bsn adapter"
```

### Stage C: Execute Phase 3C Plan

**Input:** Accepted Phase 3C implementation plan.

**Purpose:** Build the immediate BSN adapter only after its detailed plan passes review.

**Minimum verification baseline:**

```powershell
cargo fmt --all --check
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_bsn
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_surface_boundary
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
```

If package or feature names differ after Phase 3B, the accepted Phase 3C plan must name the equivalent commands and explain the mapping.

Unavailable BSN or Bevy APIs block Phase 3C completion and Phase 3D planning unless a later accepted plan explicitly changes MVP scope. Recording the exact unavailable source or API cause is required, but it is not acceptance evidence.

**Required outcome:**

- Phase 3C verification record appended to the accepted Phase 3C plan.
- The appended record contains these exact record markers:
  - `Phase 3C verification status: PASS`
  - `Phase 3C implementation commit:`
  - `Phase 3C review result: PASS`
  - `Phase 3D handoff statement:`
- Phase 3C review passes after lead-agent evaluation.
- Worktree is clean or unrelated changes are explicitly named.
- Final Phase 3C implementation commit exists.

**Blocks Phase 3D planning if:**

- BSN cannot be tested.
- BSN errors do not map to router/view errors.
- BSN can reach private authority or active commit paths.
- BSN applies visible active UI without Phase 3B core/ECS acceptance and activation.
- Phase 3C final evidence lacks exact command output, exit codes, or review result.
- Any Phase 3C item is recorded as unavailable or blocked and the user has not approved a reviewed MVP scope change. Exact blocked causes are required for honesty, but they do not satisfy the gate.

### Stage D: Create Phase 3D Minimal Nested Routes Example Plan

**Target file:** `docs/superpowers/plans/2026-07-07-phase-3d-minimal-example-implementation-plan.md`

**Purpose:** Produce the executable implementation plan for the neutral MVP example after Phase 3C proves the BSN adapter boundary.

**Plan must cover:**

- Neutral nested routes with static, param, index, pathless layout, and named slot coverage if already supported by Phase 1/3 APIs.
- Example target ownership. Default to `crates/bevy_ui_router/examples/minimal_nested_routes.rs` so commands can run from the current virtual workspace through `-p bevy_ui_router`; a different package is allowed only if the Phase 3D plan explains why it provides stronger evidence and updates every command.
  This reconciles the execution-flow document's older root-level `examples/minimal_nested_routes.rs` placeholder with the current virtual workspace package layout; it does not rename the Phase 3D plan file or supersede the execution flow.
- Fake/test renderer path assertion.
- Immediate BSN adapter path assertion.
- Active branch assertion.
- Frame root marker and generation assertion through public read-only diagnostics or snapshot APIs.
- Diagnostics assertion for router, host, slot, route id chain, active branch, frame identity, generation, and error path where applicable. The example must not construct internal ECS markers, lifecycle state, generation authority, or facade-exported lifecycle authority to make assertions pass.
- Facade feature combinations required to run the example.
- Non-visual, deterministic, self-terminating verification through tests or explicit runtime assertions.
- Clear separation between example code and app-specific shell, theme, permissions, or data services.

**Pre-plan gate commands:**

```powershell
git log -1 --oneline
git status --short --branch
rg -n "^## Phase 3C Verification Record$|^Phase 3C verification status: PASS$|^Phase 3C implementation commit:|^Phase 3C review result: PASS$|^Phase 3D handoff statement:" docs\superpowers\plans\2026-07-07-phase-3c-bsn-adapter-implementation-plan.md
rg -n "examples|minimal_nested_routes|RouteTree|RouteHost|RouteSlot|bevy_ui_router_bsn" Cargo.toml crates docs
```

Expected: the first `rg` command finds only the appended Phase 3C verification record and its exact marker lines, not planning instructions. The `git status --short --branch` output must show a clean worktree or explicitly named unrelated changes.

**Plan self-check commands:**

```powershell
$zhPending = [string]([char]0x5f85) + [string]([char]0x5b9a)
$zhUnset = [string]([char]0x672a) + [string]([char]0x5b9a)
$zhLater = [string]([char]0x7a0d) + [string]([char]0x540e)
$terms = @('T'+'BD','TO'+'DO','FIX'+'ME',$zhPending,$zhUnset,$zhLater,'implement'+' later','fill'+' in')
$pattern = $terms -join '|'
rg -n $pattern 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3d-minimal-example-implementation-plan.md'
rg -n "[ \t]+$" 'C:\projects\bevy_ui_router\docs\superpowers\plans\2026-07-07-phase-3d-minimal-example-implementation-plan.md'
git diff --check
```

Expected: placeholder scan and trailing whitespace scan return no matches; `rg` exits with code 1 for those no-match scans, and that no-match exit is the expected passing result. `git diff --check` exits with code 0.

**Review directions:**

- Architecture and lifecycle review: verify the example demonstrates the intended Router lifecycle without becoming a source of lifecycle authority.
- API and facade review: verify the example uses public MVP observation and facade APIs, intended feature gates, and no internal marker construction or lifecycle authority.
- Fact and source review: verify all example claims against Phase 3C evidence, current public API, and roadmap docs.
- Test and risk review: verify assertions cover active branch, frame markers, generation, diagnostics, fake renderer path, BSN adapter path, and failure reporting where applicable.

The lead agent must evaluate review findings for shared root causes before accepting edits. If the root cause is missing Phase 3C adapter capability or Phase 3B ECS public API insufficiency, return to the owning phase instead of hiding the gap in the example.

After review passes, ask independent agents whether the plan needs structural cleanup or semantic rewrite. If yes, create a separate rewrite candidate, validate it against the accepted content, replace only after lead-agent acceptance, then commit.

**Planning commit:**

```powershell
git add docs/superpowers/plans/2026-07-07-phase-3d-minimal-example-implementation-plan.md
git commit -m "docs: plan phase 3d minimal example"
```

### Stage E: Execute Phase 3D Plan

**Input:** Accepted Phase 3D implementation plan.

**Purpose:** Build the neutral minimal nested routes example and prove MVP integration behavior.

**Minimum verification baseline:**

```powershell
cargo fmt --all --check
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn --example minimal_nested_routes
cargo run --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn --example minimal_nested_routes -- --assert-and-exit
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
```

If the example is implemented as an integration test rather than a runnable example, the accepted Phase 3D plan must name the exact package, target, feature flags, and equivalent command, then explain why it provides stronger evidence than a runtime example.

The runtime path must be non-visual, deterministic, and self-terminating. A command that opens a window, waits for manual input, depends on visual inspection, or lacks an assertion-and-exit mode is not valid MVP verification.

Unavailable BSN or Bevy APIs block Phase 3D completion and MVP completion unless a later accepted plan explicitly changes MVP scope. Recording the exact unavailable source or API cause is required, but it is not acceptance evidence.

**Required outcome:**

- Phase 3D verification record appended to the accepted Phase 3D plan.
- The appended record contains these exact record markers:
  - `Phase 3D verification status: PASS`
  - `Phase 3D implementation commit:`
  - `Phase 3D review result: PASS`
  - `MVP completion handoff statement:`
- Phase 3D review passes after lead-agent evaluation.
- Worktree is clean or unrelated changes are explicitly named.
- Final Phase 3D implementation commit exists.

**Blocks MVP completion if:**

- The example uses app-specific business, brand, data service, permission, shell, or theme assumptions.
- The example lacks assertions for active branch, frame identity, generation, and diagnostics.
- The BSN path cannot run.
- The example bypasses public facade/core/view/ECS/BSN APIs.
- Phase 3D final evidence lacks exact command output, exit codes, or review result.
- Any Phase 3D item is recorded as unavailable or blocked and the user has not approved a reviewed MVP scope change. Exact blocked causes are required for honesty, but they do not satisfy the gate.

### Stage F: MVP Completion Review

**Purpose:** Determine whether Phase 0 through Phase 3D satisfy the MVP, and explicitly leave Phase 4+ as non-MVP unless the user approves a new planning target.

**Required evidence:**

- Phase 0 verification record and commit evidence.
- Phase 1A and Phase 1B verification records and commit evidence.
- Phase 2 verification record and commit evidence.
- Phase 3A verification record and commit evidence.
- Phase 3B verification record and commit evidence.
- Phase 3C verification record and commit evidence.
- Phase 3D verification record and commit evidence.
- Worktree clean or unrelated changes explicitly identified.
- Final authority scans prove no adapter, facade, BSN, platform, or example code can forge frame identity, generation, cleanup scope, transaction tokens, or active commit state.

**Completion review commands:**

```powershell
git log --oneline -20
git status --short --branch
cargo fmt --all --check
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_view
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_bsn
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features bsn
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features platform
cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
```

**Final authority scan commands:**

```powershell
rg -n "RouterRuntime|report_frame_apply|commit_ready_navigation|acknowledge_failed_navigation_cleanup|NavigationTransaction|NavigationTransactionId|FrameAllocator|FrameApplyToken::|RouteFrameIdentity::new|RouteGeneration|RouteGeneration::initial|RouteGeneration::next|RouterInstanceId::|RouteHostId::|RouteSlotId::|RouteFrameId::|RouteFrameScope::new|CleanupScope::router_owned_subtree|FrameCleanupPlan::" crates\bevy_ui_router_core crates\bevy_ui_router_view crates\bevy_ui_router_ecs crates\bevy_ui_router_bsn crates\bevy_ui_router_platform crates\bevy_ui_router
rg -n "RouteFrameRootLifecycleState|RouteFrameInteractionBarrier|RouteFocusState|cleanup_router_owned_subtree|prepare_inert_root|activate_committed_root|finalize_inactive_root|PostCommitActivation|CleanupCapability|PrepareInertCapability|InactiveFinalizationCapability" crates\bevy_ui_router_ecs crates\bevy_ui_router_bsn crates\bevy_ui_router_platform crates\bevy_ui_router
rg -n "ViewReportSession|EcsFrameApplySession|FrameApplyRequest|FrameApplyOutcome|ViewBuildOutcome|ViewApplyOutcome" crates\bevy_ui_router_view crates\bevy_ui_router_ecs crates\bevy_ui_router_bsn crates\bevy_ui_router
```

Expected: every match is classified as one of core implementation, controlled ECS implementation, public read-only observation API, compile-fail fixture text, tests, examples using public observation APIs, authority allow-list text, or documentation. A production match in BSN, platform, facade, or examples that calls core runtime commit/report/cleanup acknowledgement directly, constructs frame/generation/token/cleanup authority, or reaches ECS cleanup/finalization capabilities fails the scan.

If any command creates an untracked root `Cargo.lock`, inspect whether the workspace policy changed. If it is only a generated verification artifact in a lockfile-free workspace, remove it with approved cleanup and record that action.

Unavailable command output, missing authority-scan classification, unavailable BSN/Bevy APIs, or any unverified MVP matrix row blocks MVP completion unless a later accepted plan explicitly changes MVP scope before the completion review.

**MVP review directions:**

- Architecture review: verify MVP keeps core generic, Bevy/BSN/platform isolated, and Router ownership boundaries intact.
- API review: verify builder-first public API, raw location/search MVP boundary, facade features, and prelude surfaces match accepted decisions.
- Fact review: verify every completion claim against current source, all phase records, current manifests, and `AGENTS.md`.
- Test review: verify minimum tests cover matcher, branch diff, transaction/stale, error boundary, fake renderer, ECS cleanup/finalization, BSN adapter, facade boundaries, and minimal example assertions.

**Completion record target:**

Create or append an MVP completion record only after all evidence is available:

```powershell
git add docs/superpowers/plans/2026-07-09-mvp-completion-planning-gates.md
git commit -m "docs: record mvp completion review"
```

The completion record must distinguish:

- accepted MVP behavior;
- failed or unavailable verification;
- non-MVP Phase 4+ roadmap items;
- risks requiring future planning;
- whether the next target is Phase 4 planning, release/CI hardening, API stabilization, or external review.

---

## MVP Acceptance Matrix

| Area | Required evidence | MVP acceptance semantics |
| --- | --- | --- |
| Core independence | Phase 0-3D dependency tests and final scans. | Core has no Bevy, BSN, platform, app shell, theme, permission, copy, or data-service dependency. |
| Builder-first route API | Phase 1 records and facade tests. | MVP exposes hand-written builder-first route declarations with raw route/search boundary. |
| Matching and branch diff | Phase 1A/1B tests. | Static, param, catch-all, index, nested, pathless, named slot, and branch diff behavior pass accepted tests. |
| Frame lifecycle | Phase 2 tests and Phase 3B bridge tests. | Frame identity, generation, transaction, stale rejection, error boundary, and diagnostics are traceable. |
| View contract | Phase 3A tests. | Renderer-neutral fake path covers build/apply success, failure, stale result, and provenance. |
| ECS bridge | Phase 3B verification record. | ECS owns Bevy hierarchy mutation while core owns stale acceptance and active commit. |
| Cleanup and focus | Phase 3B cleanup/finalization tests. | Router-owned cleanup cannot touch app-owned host; inactive subtrees cannot keep input or focus. |
| BSN adapter | Phase 3C verification record. | Immediate BSN adapter uses only public APIs and maps success/failure through Phase 3B reporter/session. |
| Minimal example | Phase 3D verification record. | Neutral nested routes example asserts active branch, frame markers, generation, diagnostics, fake path, and BSN path. |
| Facade features | Phase 3B-3D facade tests and final checks. | Default facade remains Bevy-free; `ecs`, `bsn`, and `platform` are explicit opt-in surfaces. |
| Authority boundary | Compile-fail tests and final authority scans. | No adapter, facade, BSN, platform, or example code can forge identity, generation, cleanup scope, transaction token, or active commit state. |
| Verification honesty | Phase records include exact commands, exit codes, and output summaries. | No skipped or unavailable item is reported as passing. |

MVP completion requires every matrix row to have direct evidence. Missing, indirect, or unverified evidence keeps MVP completion open.

---

## Non-MVP Boundaries

The following remain Phase 4 or later unless the user approves a new reviewed plan:

- typed route public API;
- typed search public API;
- loader, guard, action, revalidation, pending async guard, and redirect-loop runtime;
- asset-driven BSN route scenes;
- queued scene apply, asset-driven scene apply, and any queued/asset future-token contract;
- devtools;
- platform-specific back or deep-link packages;
- retained subtree cache runtime beyond input/focus isolation guarantees required by MVP;
- transition runtime;
- app-specific shell, theme, copy, permissions, data services, or integration profile.

---

## Commit Strategy

- Commit this planning-gates document before Phase 3B execution continues:

```powershell
git add docs/superpowers/plans/2026-07-09-mvp-completion-planning-gates.md
git commit -m "docs: plan mvp completion gates"
```

- Commit the accepted Phase 3B plan separately from implementation commits.
- Execute Phase 3B using the task-level commits defined in the accepted Phase 3B plan.
- Commit the Phase 3C planning document separately before Phase 3C implementation.
- Execute Phase 3C with task-level commits defined by the accepted Phase 3C plan.
- Commit the Phase 3D planning document separately before Phase 3D implementation.
- Execute Phase 3D with task-level commits defined by the accepted Phase 3D plan.
- Commit the final MVP completion record only after all Phase 0-3D evidence is available and reviewed.

Do not combine planning, implementation, and completion-record commits unless a later accepted plan explicitly explains why the split would be misleading.
