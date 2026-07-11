# Root Push Implementation Plan

**Status:** Root push `Plan PASS`

**Target baseline:** Phase 3B.1 architecture `Plan PASS` at commit `203f5ba`

**Stage contract:** `docs/root-push-contract-profile.md`

**Acceptance entry:** `crates/bevy_ui_router/tests/public_navigation_journeys.rs` (`root_push` cases)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the legacy manual apply prototype with one facade-driven, single-root-frame `push` that proceeds through real `App::update`, creates real Bevy UI, atomically publishes presentation/history, and settles through an identity-bound cleanup completion.

**Architecture:** `RouterEngine` becomes the only logical navigation authority; ECS owns every Bevy `Entity`, registry record, mutation plan, receipt, and physical validation. The immediate driver produces only a session-local data plan. Root push establishes the complete host-to-snapshot vertical path and honest repeated single-root `push`; it does not implement child mount, nested common-prefix reconciliation, replace/back, or full authority recovery.

**Tech Stack:** Rust 2024, Bevy 0.19 from exact revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`, Cargo resolver 3, `syn` 2 with `full`, `cargo-nextest`.

## Global Constraints

- All Bevy git dependencies use exact `rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7"`; the workspace `Cargo.lock` is tracked and every post-lock gate uses `--locked`.
- `bevy_ui_router_core` remains Bevy-free and is the sole logical authority. ECS is the sole `Entity`, component, relationship, registry, physical preflight, and receipt authority.
- The Router may mutate or remove only router-owned entities. It never inserts components into, replaces, despawns, or reorders existing children of the app-owned host except for the exact authorized top-slot membership edge.
- No production path exposes raw `Entity`, an authority marker constructor, a commit/mount/cleanup capability, a generic `Bundle`, a World callback, or a mutable registry handle through the facade.
- Every router-created entity is empty-allocated, staging-registered, and made inert before any reentrant Bevy mutation. Pending UI is hidden, pointer/keyboard/gamepad blocked, non-focused, and uses `Pickable::IGNORE`.
- The successful switch is one exclusive, preflighted, non-fallible mutation: isolate leaving, activate entering, replace registry/gate state, then publish core presentation/history. First push has no leaving root; fragment remount and changed-root repeated push each have one exact leaving frame closure.
- A first push remains `Running(PostCommitCleanup)` until its sealed zero-obligation cleanup batch is completed. Duplicate, stale, foreign, or malformed reports/completions never change World, registry, ledger, presentation, history, request phase, or terminal state. They may append one deduplicated structured disposition diagnostic; an exact completion replay returns `DuplicateRejected` without adding another event.
- Root push implements one normal root frame per attempt, repeatable single-root push, root fragment remount, and mandatory router fallback. Child outlet mount, nested frames, nested common-prefix preservation, replace/back, full host-loss recovery, quarantine reaping, BSN, loaders, transitions, and cache remain out of scope.
- Every production behavior follows RED -> verified failure -> minimal GREEN -> focused regression -> commit. Configuration-only edits are preceded by a failing manifest/feature test.
- Authoritative sources are `docs/target-architecture.md`, `docs/authority-transition-matrix.md`, `docs/facade-contract-fixture.md`, `docs/facade-export-allowlist.md`, `docs/facade-associated-api-golden.txt`, and `docs/mvp-acceptance-journeys.md`. `docs/root-push-contract-profile.md` selects the exact stage-reachable subset without changing the final MVP contract.

## Derived Root Push Authority Protocol

This section is an implementation projection of Target Architecture sections 6-10 and Authority Transition Matrix sections 1-7. Those documents remain authoritative; this plan may order their work but may not redefine their lifecycle semantics.

Every root build path uses one internal immutable identity tuple:

```text
RootTaskIdentity = WorldDomain + RouterInstanceId + HostVersion(host_id, epoch)
                 + RouteSlotId(top_slot) + NavigationRequestId + TransactionId
                 + FrameTaskId + RouteFrameId + RouteGeneration + FrameOrigin
```

`FrameTaskId` and `TransactionId` are core-private non-reusable IDs. `ViewRequestId` is a read-only projection for the driver and never replaces `FrameTaskId`. The engine frame task, ECS report session, accepted `ContentPlan`, staging registry records, commit preflight, sealed publication operation, cleanup logical root, and diagnostics all carry or verify the same tuple. Any mismatch is stale/invalid before visible mutation.

Normal root and fallback are separate attempts in one request. A normal match/build/apply failure cancels the push history intent and, when recovery is allowed, makes `RouterEngine` issue a fresh `RouterFallback` frame ID/task/generation against the already confirmed top slot. Fallback success commits `ActivePresentation::Error` while history remains unchanged; fallback failure cleans only fallback staging output and leaves the prior presentation unchanged, or `None` on first navigation, before the request settles as `Failed(RecoveryFailed)`.

Commit has two phases. Read-only preflight validates the complete tuple, current engine state, ECS closure, gate baseline, and publication payload, then produces `Ready(CommitMutationPlan)` containing a single-use internal publication operation. Every rejectable check occurs before the first physical write. Executing the plan performs only pre-resolved in-place ECS mutations and the already-authorized, non-fallible core publication operation; publication cannot return an error after ECS activation begins.

New authority code may coexist in source with the legacy prototype only while it is private and disconnected from every plugin/system/public path. A dedicated workspace cutover task migrates all callers, wires the new system chain, removes legacy exports/integration, and proves by compile gates that exactly one authority can publish active/history.

Repeated single-root branches use these exact terminal rules:

- Equivalent canonical path/search/fragment and branch: no entering or leaving frame, zero cleanup obligations, but the push history intent still appends the canonical location. The engine publishes the unchanged presentation, appends `CommitPublished`, enters `PostCommitCleanup`, completes the zero batch, and terminates `Succeeded`.
- Fragment-only change: preserve frame ID, advance generation, build one entering generation, isolate the complete prior-generation frame closure, and issue cleanup obligations for exactly every ECS registry record in that old closure and no others. Commit appends the new canonical location to push history; terminal success waits for all old-closure receipts.
- Any other accepted single-root target: allocate a new frame ID/generation, build one entering root, isolate the complete old frame closure, and issue cleanup obligations for exactly every registry record in that old closure and no others. Commit appends the target to push history; terminal success waits for all receipts.

For both nonzero branches, the cleanup manifest count equals the deterministic cardinality of the exact old frame closure captured by preflight; it is not a hard-coded entity count because driver output cardinality is data-dependent. Trace order is `CommitPublished -> PostCommitCleanup -> zero or more disposition receipts -> CleanupSettled -> RequestTerminal`.

## Root Push Exit Evidence

- A slice-specific facade consumer installs a router, admits one app-owned Node host, pushes one single-node route, observes a real active `SolidRect`, one history entry, ordered trace, and `Succeeded` within 16 `App::update` calls.
- The same public method can be called again: equivalent input emits no view effect, fragment-only input preserves frame ID and advances generation, and a different single-root target atomically replaces the old root, records cleanup, and appends push history.
- The same journey proves the host sentinel and pre-existing child identities/order remain unchanged except for exact top-slot insertion.
- Core tests prove canonical location, sealed host authority, first-frame identity, stale rejection, atomic publication, push history, and zero-obligation cleanup gating.
- ECS private tests prove full Node ancestry, required-component closure, inert staging, real layout/visibility/picking, registry uniqueness, post-write validation, and failure-plan World zero-write.
- Facade gates prove the frozen feature graph, Root push contract profile, forbidden authority paths, automatic/explicit plugin installation, and isolated consumer compilation. Full allowlist/golden closure remains an MVP-gate obligation.
- `cargo fmt --all --check`, targeted tests, `cargo nextest run --locked --workspace --all-features`, metadata/tree gates, and an independent factual review all pass with captured output.

## File Map

### Core owner

- `crates/bevy_ui_router_core/src/view_key.rs`: validated `RouteViewKey`.
- `crates/bevy_ui_router_core/src/request.rs`: request IDs, status, phase, failure, lookup-neutral snapshot data.
- `crates/bevy_ui_router_core/src/presentation.rs`: active presentation and frame snapshots.
- `crates/bevy_ui_router_core/src/view_input.rs`: renderer-neutral route-local input and frame origin projection.
- `crates/bevy_ui_router_core/src/trace.rs`: append-only navigation trace types.
- `crates/bevy_ui_router_core/src/admission.rs`: sealed host admission request/grant/confirmation protocol.
- `crates/bevy_ui_router_core/src/cleanup.rs`: sealed logical cleanup batch identity and completion protocol.
- `crates/bevy_ui_router_core/src/engine.rs`: queue, matcher, host authority, first-frame task, commit, history, retention.
- Existing `location.rs`, `ids.rs`, `meta.rs`, `route_tree.rs`, `history.rs`, `frame.rs`, and `lib.rs` are migrated to the new model.
- Legacy `runtime.rs` and `transaction.rs` are removed after all callers move; they must never coexist as a second integrated authority.

### View owner

- `crates/bevy_ui_router_view/src/registry.rs`: `ViewDriverKey`, descriptor, and deterministic registry.
- `crates/bevy_ui_router_view/src/effect.rs`: `ViewRequestId` and `ViewBuildEffect` over core-owned `RouteViewInput`.
- `contract.rs`, `testing.rs`, and `lib.rs` are replaced or reduced so old partial-output/disposal authority is not public.

### ECS owner

- `crates/bevy_ui_router_ecs/src/app_ext.rs`: plugin installation, driver registration, router installation.
- `host.rs`: host candidate observation and confirmed inert top-slot admission.
- `registry.rs`: unique entity ownership/seal/manifest records, physical classification, and private quarantine-table foundation.
- `content.rs`: `SolidRect`, `ContentColor`, session-local `ContentPlan`, and `FrameContentWriter`.
- `driver.rs`: immediate driver object store, report session, and report validation.
- `navigation.rs`: public enqueue resource and ECS-to-engine command intake.
- `presentation.rs`: shell/content realization, final preflight, and atomic switch.
- `input.rs`: production `RouterInputGate`, focus release, and picking values.
- `audit.rs`: minimal read-only test-support projection over registry facts; internal Node/manifest details remain private tests.
- `cleanup.rs`: root-push foreground ledger and zero-obligation completion; later tasks extend its physical dispositions.
- `snapshots.rs`: public immutable snapshots and request retention projection.
- `plugin.rs`, `components.rs`, `state.rs`, `bridge.rs`, `finalize.rs`, and `lib.rs` migrate to or are replaced by these owners.

### Facade and tests

- `crates/bevy_ui_router/src/lib.rs`: curated modules and prelude only.
- `crates/bevy_ui_router/tests/public_navigation_journeys.rs`: fixed facade journey target; Root push adds its stage-owned cases here.
- `crates/bevy_ui_router/tests/default_facade_features.rs`: Root-active default consumer target.
- `crates/bevy_ui_router/tests/public_navigation_journeys.rs`: Root-active public journey target.
- `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`: Root-active audit target.
- `crates/bevy_ui_router/tests/ecs_facade_features.rs`: Root-active ECS feature target.
- `crates/bevy_ui_router/tests/bsn_facade_features.rs`: Root-active empty-BSN target.
- `crates/bevy_ui_router/tests/test_support_features.rs`: Root-active test-support target.
- `crates/bevy_ui_router/tests/facade_export_allowlist.rs`: Root-profile positive/source/golden gate.
- `crates/bevy_ui_router/tests/facade_authority_compile_fail.rs`: Root-profile negative consumer gate.
- `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`: shared isolated consumer catalogue and lock harness.
- `scripts/verify-authority-regressions.ps1`: catalogue/registered/passed set equality gate.
- `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`: private physical and hostile regressions.
- `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`: `syn` catalogue completeness gate.

---

### Task 1: Lock Dependencies And Feature Graph

**Files:**
- Modify: `crates/bevy_ui_router/Cargo.toml`
- Modify: `crates/bevy_ui_router_ecs/Cargo.toml`
- Modify: every workspace manifest containing a Bevy git dependency
- Modify: `crates/bevy_ui_router/tests/facade_features.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs`
- Add: `Cargo.lock`

**Interfaces:**
- Produces: the exact facade feature graph and locked Bevy dependency graph used by every later task.

- [ ] **Step 1: Make manifest tests express the frozen graph**

Add assertions for the exact facade feature map, removal of `view-test-support`, ECS `test-support`, exact Bevy revision, `bevy_ui/bevy_picking`, facade direct `bevy_app/bevy_ecs/bevy_ui/syn` dev dependencies, and ECS direct `bevy_internal/syn` dev dependencies.

- [ ] **Step 2: Run the focused tests and verify RED**

Run: `cargo test -p bevy_ui_router --test facade_features`

Run: `cargo test -p bevy_ui_router_ecs --test dependency_boundary`

Expected: failures name the current branch dependency, missing feature edges, missing direct dependencies, and obsolete `view-test-support`.

- [ ] **Step 3: Apply the exact manifest definitions**

Use this facade feature graph verbatim:

```toml
[features]
default = ["view"]
view = ["dep:bevy_ui_router_view"]
ecs = ["view", "dep:bevy_ui_router_ecs"]
ecs-test-support = ["ecs", "bevy_ui_router_ecs/test-support"]
test-support = ["ecs-test-support"]
bsn = ["ecs", "dep:bevy_ui_router_bsn"]
platform = ["dep:bevy_ui_router_platform"]
devtools = []
```

Every Bevy dependency uses the exact revision from Global Constraints. `bevy_ui` enables only its required `bevy_picking` integration in normal ECS dependencies. `bevy_internal` is ECS dev-only with `bevy_window`, `bevy_camera`, `bevy_ui`, and `bevy_picking`.

- [ ] **Step 4: Regenerate and inspect the lock**

Run: `cargo generate-lockfile`

Run: `cargo metadata --locked --config "build.rustc-wrapper = ''" --format-version 1 --no-deps`

Run: `cargo tree --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs -e features -i bevy_ui`

Run: `cargo tree --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --target all -e features,dev -i bevy_internal`

Expected: all Bevy git packages resolve to `61f78f7...`; `bevy_ui/bevy_picking` is present; `bevy_ui/ghost_nodes` is absent; dev-only `bevy_internal` has exactly the allowed feature set.

- [ ] **Step 5: Verify GREEN and commit**

Run: `cargo test --locked -p bevy_ui_router --test facade_features`

Run: `cargo test --locked -p bevy_ui_router_ecs --test dependency_boundary`

Commit: `build: lock root push dependency graph`

### Task 2: Canonical Location And Route View Bindings

**Files:**
- Create: `crates/bevy_ui_router_core/src/view_key.rs`
- Modify: `crates/bevy_ui_router_core/src/location.rs`
- Modify: `crates/bevy_ui_router_core/src/meta.rs`
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Test: owner module unit tests and `crates/bevy_ui_router_core/tests/public_api.rs`

**Interfaces:**
- Produces: `RouteViewKey`, canonical `RouteLocation`, normal/error route bindings consumed by view and ECS installation.

- [ ] **Step 1: Write canonicalization RED tests**

```rust
#[test]
fn canonicalizes_slash_only_path_differences() {
    let location = RouteLocation::new("items//1/")
        .with_search("?tab=raw")
        .with_fragment("details");
    assert_eq!(location.path(), "/items/1");
    assert_eq!(location.search(), Some("?tab=raw"));
    assert_eq!(location.fragment(), Some("details"));
    assert_eq!(RouteLocation::new("///").path(), "/");
}
```

Add tests that invalid/empty `RouteViewKey` values fail, duplicate route bindings are deterministic, and a node can hold normal view plus boundary/error view without exposing a mutable compiled manifest.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_core canonicalizes_slash_only_path_differences`

Run: `cargo test --locked -p bevy_ui_router_core route_tree::tests`

Expected: slash folding and target binding APIs are missing.

- [ ] **Step 3: Implement the minimal pure-data model**

`RouteLocation::new` canonicalizes immediately. `RouteNodeOptions::with_view` is public for Root profile assembly. Child-route builder methods and error-boundary builder methods remain crate-private until their owning slice; internal core/ECS tests still exercise compiled binding validation. Compiled binding access remains crate-internal.

- [ ] **Step 4: Verify GREEN and compatibility**

Run: `cargo test --locked -p bevy_ui_router_core`

Expected: all matcher/tree tests pass with canonical input.

- [ ] **Step 5: Commit**

Commit: `feat(core): add canonical route view model`

### Task 3: Request, Presentation, Trace, And Admission Types

**Files:**
- Create: `crates/bevy_ui_router_core/src/request.rs`
- Create: `crates/bevy_ui_router_core/src/presentation.rs`
- Create: `crates/bevy_ui_router_core/src/view_input.rs`
- Create: `crates/bevy_ui_router_core/src/trace.rs`
- Create: `crates/bevy_ui_router_core/src/admission.rs`
- Modify: `crates/bevy_ui_router_core/src/ids.rs`
- Modify: `crates/bevy_ui_router_core/src/frame.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Produces: non-forgeable router/request/frame/task identities, orthogonal availability/request/presentation states, the Core request-record foundation later projected by ECS as `NavigationRequestSnapshot`, core-owned `RouteViewInput`, host admission protocol, and append-only trace DTOs. The final public `NavigationRequestSnapshot` remains ECS-owned as frozen by Target Architecture and the facade golden.

- [ ] **Step 1: Write type-state RED tests**

Test that a fresh engine-facing state is `AwaitingHost + None`, no host/slot ID exists before a confirmed grant, request IDs bind domain/router/sequence, and trace events preserve append order even within one update.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_core`

Expected: modules/types do not exist.

- [ ] **Step 3: Implement exact read-only states**

Implement `RouterAvailability`, `RequestStatus`, `NavigationPhase`, `ActivePresentation`, Core-private request-record state, `NavigationTraceEvent`, `FrameOrigin`, `RouteViewInput`, and sealed admission token/grant/confirmation. `RouterHandle` and `NavigationRequestId` have no raw constructor/accessor; all identity constructors and allocators remain crate-private. Read-only `raw()` access on the other facade IDs follows the associated-API golden exactly and does not grant construction authority.

- [ ] **Step 4: Add compile-boundary assertions**

Assert read-only DTO traits and prove facade-facing code cannot construct admission tokens, host IDs, slot IDs, frame IDs, task IDs, or generation values.

- [ ] **Step 5: Verify GREEN and commit**

Run: `cargo test --locked -p bevy_ui_router_core`

Commit: `feat(core): define root navigation state model`

### Task 4: RouterEngine First-Push Authority

**Files:**
- Create: `crates/bevy_ui_router_core/src/engine.rs`
- Create: `crates/bevy_ui_router_core/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_core/src/history.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Keep temporarily private and disconnected until Task 9 cutover: `crates/bevy_ui_router_core/src/runtime.rs`
- Keep temporarily private and disconnected until Task 9 cutover: `crates/bevy_ui_router_core/src/transaction.rs`

**Interfaces:**
- Consumes: Task 2 route bindings and Task 3 state types.
- Produces: install-time compiled matcher, host admission, queued repeatable `push`, one root frame task per attempt, single-root diff/remount decision, stale report disposition, sealed commit ticket, push history intent, cleanup batch token, and retained snapshots.

- [ ] **Step 1: Write engine RED tests in lifecycle order**

Tests cover: no-host enqueue rejection without request ID; confirmed host then canonical push; one running request; one `MatchedRoute` root task; independent mismatch rejection for every field of `RootTaskIdentity`; report success without publication; preflighted single-use publication; history unchanged before commit and one entry after commit; equivalent repeated push has zero frame task; fragment-only repeated push preserves frame ID and advances generation; changed single-root push issues one leaving/entering replacement; cleanup completion gates success; duplicate/foreign cleanup completion is lifecycle zero-write.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_core engine`

Expected: `RouterEngine` and cleanup protocol are absent.

- [ ] **Step 3: Implement the minimal engine**

The engine accepts `RouteLocation`, performs matching internally, and owns all logical transitions. It never accepts an externally supplied `MatchedBranch` as navigation authority. With no active frame it issues one normal root task; with an active single root it applies final semantics: canonical path/search/fragment equivalence produces no frame task, fragment-only change preserves frame ID and advances generation, and every other accepted target replaces the whole root. Report acceptance and commit preflight verify every field of `RootTaskIdentity`, including top slot and frame task; the accepted plan cannot be rebound to another `ViewRequestId` projection.

- [ ] **Step 4: Implement zero-obligation terminal gating**

After `CommitPublished`, issue a cleanup batch with count zero, append `PostCommitCleanup`, accept one exact completion, append `CleanupSettled`, then append terminal `Succeeded`. Replays do not alter trace/history/presentation.

- [ ] **Step 5: Keep the new engine disconnected until workspace cutover**

Make the legacy runtime/transaction surface private now, but do not wire `RouterEngine` into the existing ECS plugin in this task. Tests may construct the engine only through core-private fixtures. Task 9 performs the one workspace cutover; no intermediate commit has two integrated publishers.

- [ ] **Step 6: Verify and commit**

Run: `cargo test --locked -p bevy_ui_router_core`

Commit: `feat(core): build router engine authority`

### Task 5: Renderer-Neutral View Registry And Effects

**Files:**
- Create: `crates/bevy_ui_router_view/src/registry.rs`
- Create: `crates/bevy_ui_router_view/src/effect.rs`
- Modify: `crates/bevy_ui_router_view/src/lib.rs`
- Remove or rewrite: `crates/bevy_ui_router_view/src/contract.rs`
- Remove facade forwarding of: `crates/bevy_ui_router_view/src/testing.rs`

**Interfaces:**
- Consumes: core `RouteViewKey`, canonical location, matched route, frame origin, and navigation failure.
- Produces: `ViewDriverKey`, `RouteViewDescriptor`, `RouteViewRegistry`, `ViewRequestId`, and `ViewBuildEffect`; no World or acceptance capability.

- [ ] **Step 1: Write RED tests for keys, registry, and immutable effect input**

Cover invalid keys, duplicate registry entries, stable lookup, route-local params/search/fragment, frame origin, and absence of generation/Entity/commit accessors.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_view`

- [ ] **Step 3: Implement pure data contracts and remove old authority-shaped reports**

The view crate remains dependency-limited to core. Move immediate ECS report/session ownership out of view. Do not retain aliases for `ViewBindingKey`, `ViewReportSession`, partial output, disposal hints, or fake facade renderers.

- [ ] **Step 4: Verify dependency and compile boundaries**

Run: `cargo test --locked -p bevy_ui_router_view`

Commit: `feat(view): define renderer-neutral view effects`

### Task 6: Data-Only Content Plan And Immediate Driver

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/content.rs`
- Create: `crates/bevy_ui_router_ecs/src/driver.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Replace callers in: `crates/bevy_ui_router_ecs/src/bridge.rs`

**Interfaces:**
- Consumes: `ViewBuildEffect`.
- Produces: sealed `SolidRect`, `ContentColor`, session-local `ContentEntity`, `FrameContentWriter`, `EcsViewDriver`, `EcsViewReportSession`, and `ViewBuildReport`.

- [ ] **Step 1: Write content-plan RED tests**

Cover zero dimensions, session-local handle ownership, self-edge, duplicate edge/parent, cycle, duplicate outlet, foreign-session outlet, stable topology, and terminal report single use. Assert driver return has not changed a Bevy `World` or registry fingerprint.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_ecs`

- [ ] **Step 3: Implement the closed data model**

`FrameContentWriter` owns only a mutable in-memory plan. `spawn_empty`, `spawn_solid_rect`, `add_child`, private `declare_outlet`, and `despawn` validate handles and graph structure. `SolidRect` stores validated nonzero pixel dimensions and four `u8` sRGBA channels. No generic component or callback entry exists. For Root profile, `declare_outlet` remains crate-private while its plan validation is unit-tested; a direct type re-export must not leak excluded inherent methods.

- [ ] **Step 4: Implement same-crate report authority**

Only ECS constructs `EcsViewReportSession`; consumers can call `applied`, `build_failed`, or `apply_failed` once. The report carries kind/message only; the accepted plan remains ECS-private.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --locked -p bevy_ui_router_ecs`

Commit: `feat(ecs): add closed content planning protocol`

### Task 7: Plugin, App Extension, Navigation, And Snapshots

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/app_ext.rs`
- Create: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Create: `crates/bevy_ui_router_ecs/src/snapshots.rs`
- Modify: `crates/bevy_ui_router_ecs/src/plugin.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`

**Interfaces:**
- Consumes: engine, view registry, and driver store.
- Produces: `BevyUiRouterPlugin: Default + Plugin`, `AppRouterExt`, `RouterConfig`, `RouterHandle`, `RouterNavigation`, `RouterSnapshots`, public schedule/input labels, install marker, and resources ready for the Task 9 workspace cutover.

- [ ] **Step 1: Write plugin and install RED tests**

Cover automatic install, explicit preinstall then extension use, duplicate driver rejection, synchronous config validation, missing normal/error/fallback/driver binding errors, initial `AwaitingHost`, and no router-created host entity.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_ecs`

- [ ] **Step 3: Define installation and scheduling contracts without wiring lifecycle systems**

Define the exact `RouterScheduleSet` and `RouterInputSet` variants and an internal install marker. Automatic and explicit plugin installation create each public resource/driver/router store once. Do not register placeholder lifecycle systems before host/registry/presentation functions exist; Task 9 wires the complete chain in one cutover.

- [ ] **Step 4: Implement public command/snapshot resources**

Commands enqueue with exact World/router provenance and are consumed only in `CollectNavigation`. Snapshots publish after lifecycle work and retain the engine's request records without exposing mutation capability.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --locked -p bevy_ui_router_ecs`

Commit: `feat(ecs): define root navigation facade resources`

### Task 8: Host Admission And Entity Registry Foundation

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/host.rs`
- Create: `crates/bevy_ui_router_ecs/src/registry.rs`
- Modify: `crates/bevy_ui_router_ecs/src/components.rs`
- Modify: `crates/bevy_ui_router_ecs/src/plugin.rs`

**Interfaces:**
- Consumes: `RouterHostRegistration` candidates and engine admission protocol.
- Produces: one confirmed app-host mapping, one inert router-owned top slot, `EntityOwner`, ownership seal, component manifest, candidate revision latch, inbound-parent/cycle classification, and a private quarantine lease table ready to receive staging residuals.

- [ ] **Step 1: Write host-admission RED tests**

Cover no host, unique Node host, non-Node host, non-Node/cyclic ancestry, competing candidates, extra candidate after confirmation, app host sentinel, existing child identity/order, post-write observer drift, and candidate-revision latch.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_ecs`

- [ ] **Step 3: Implement read-only candidate classification**

Never repair app topology. Preflight the exact host generation and full actual Node ancestry. Allocate logical host/epoch/top-slot only after preflight succeeds.

- [ ] **Step 4: Implement inert staging and confirmation**

Empty-allocate the top slot, staging-register identity/owner/seal/manifest, insert full Node + visibility/input/picking barrier, attach only its authorized host edge, post-write validate, then confirm. On rejection, keep IDs burned and settle only the staging residual.

If a protected parent/component/foreign descendant prevents safe removal of a staging residual, transfer only its immutable exact owned set into the independent inert quarantine table. Do not implement background reaping behavior in this task.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --locked -p bevy_ui_router_ecs`

Commit: `feat(ecs): admit app hosts without taking ownership`

### Task 9: Root Frame Realization, Atomic Commit, And Workspace Cutover

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/presentation.rs`
- Create: `crates/bevy_ui_router_ecs/src/input.rs`
- Create: `crates/bevy_ui_router_ecs/src/audit.rs`
- Rewrite foundation in: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/registry.rs`
- Modify: `crates/bevy_ui_router_ecs/src/plugin.rs`
- Replace integrated use of: `state.rs`, `finalize.rs`, and `bridge.rs`
- Modify for minimal cutover: `crates/bevy_ui_router/src/lib.rs`
- Add minimal authority RED cases to: `crates/bevy_ui_router/tests/facade_authority_compile_fail.rs`
- Remove after caller migration: `crates/bevy_ui_router_core/src/runtime.rs`
- Remove after caller migration: `crates/bevy_ui_router_core/src/transaction.rs`

**Interfaces:**
- Consumes: root frame task, accepted content plan, confirmed top slot.
- Produces: inert shell/content target/output entities, required component closure, `CommitMutationPlan`, active `SolidRect`, updated input gate, core commit publication, zero-obligation/normal-leaving cleanup settlement, and a minimal read-only authority audit projection under `test-support`.

- [ ] **Step 1: Write realization RED tests**

Test stable allocation/relationship order; `spawn_empty` becomes transparent 100% Node; `SolidRect` becomes Node + `BackgroundColor`; every entity has the frozen recursive manifest and prebuilt `Pickable::IGNORE`; accepted plan post-write drift is rejected and locally settled.

- [ ] **Step 2: Write atomic-switch RED tests**

Before commit, presentation/history remain `None`/empty and every output is inert. A failed preflight leaves them unchanged. A successful preflight activates the root and primitive, swaps registry/gate/pickable values, and publishes engine presentation/history in the same exclusive mutation.

- [ ] **Step 3: Verify RED**

Run: `cargo test --locked -p bevy_ui_router_ecs`

- [ ] **Step 4: Implement realization and immutable preflight plan**

Resolve every entity, owner, seal, generation, manifest, ancestry, parent edge, barrier, gate baseline, and engine publication payload before mutation. Preflight consumes the rejectable commit ticket and places a single-use, non-fallible publication operation into `CommitMutationPlan`. The mutation block performs no lookup, allocation, insert/remove, relationship write, hook, fallible validation, or fallible engine call.

- [ ] **Step 5: Add locked-Bevy physical regression**

Install one `bevy_internal::DefaultPlugins` set with the exact allowed features, configure the existing single `PrimaryWindow`, spawn one `Camera2d`, run UI propagation, and assert nonzero layout, effective visibility, and native picking behavior. Do not add `UiPickingPlugin` manually or spawn a second primary window.

- [ ] **Step 6: Publish the minimal read-only audit**

Under ECS `test-support`, expose opaque host/top-slot/frame/content mappings, lifecycle, primitive kind, visibility/pickable intent, and owned entity state. Do not expose raw Entity, component values, structural `spawn_empty` nodes, app ancestors, mutation methods, or registry handles.

- [ ] **Step 7: Perform the single workspace authority cutover**

First add minimal compile-fail cases proving the current whole-crate facade exposes legacy active/history authority and deferred Root-profile methods; run them to observe RED. Then implement the minimum new cleanup ledger needed for zero-obligation first push and exact normal-leaving roots, plus the no-op reaper interface. Wire `ObserveAuthority -> CollectNavigation -> DriveNavigation -> CommitPresentation -> SettleCleanup -> ReapQuarantines -> PublishSnapshots` in `Update` and `RouterInputSet::Refresh -> Ready` in `PreUpdate`. `Refresh` reads the now-existing registry/quarantine/ancestry facts and only narrows permission. Migrate every ECS/view/facade caller to the new engine/content/cleanup protocol, replace whole-crate facade reexports with the minimal Root-profile wrappers, remove legacy runtime/transaction integration and exports, then delete the legacy source/tests. The workspace compile gate fails if any old active/history publication entrypoint remains reachable.

- [ ] **Step 8: Verify and commit**

Run: `cargo test --locked -p bevy_ui_router_ecs`

Run: `cargo test --locked -p bevy_ui_router --all-features --test facade_authority_compile_fail cutover`

Run: `cargo check --locked --workspace --all-targets --all-features`

Commit: `feat(ecs): atomically commit a root frame`

### Task 10: Root Cleanup, Fallback, Diagnostics, And Slice Journey

**Files:**
- Extend: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs`
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/request.rs`
- Modify: `crates/bevy_ui_router_core/src/trace.rs`
- Add root case to: `crates/bevy_ui_router/tests/public_navigation_journeys.rs`
- Add: focused ECS private authority regressions

**Interfaces:**
- Consumes: post-commit cleanup token, root build/apply failures, stale reports, and engine-issued `RouterFallback` tasks.
- Produces: settled first push, immutable obligation manifests, versioned cleanup attempts, mandatory router fallback for root route failures, structured trace/diagnostics, and a public root-only end-to-end proof.

- [ ] **Step 1: Write terminal-gate and stale RED tests**

Assert commit does not imply terminal success; zero-obligation completion does. Duplicate/foreign completion and stale view report do not change registry, presentation, history, request, or current trace except the allowed disposition diagnostic.

- [ ] **Step 2: Write root failure RED tests**

Cover root match/build/apply failure, mandatory fallback success/failure, partial plan discard with World zero-write, first-navigation `None` preservation before fallback commit, cancelled history intent, and explicit failed request status. Do not add child mount failure cases.

- [ ] **Step 3: Implement foreground settlement foundation**

Bind the ECS ledger to the engine token/digest/count and preserve the immutable obligation identity model even though first push normally has zero leaving obligations. Every nonempty staging/failure batch freezes exact obligation identity but reclassifies current World evidence in a monotonically versioned attempt. A mutation-triggered drift supersedes the attempt, preserves only validated receipts, and retries remaining obligations from fresh evidence. Protected parent/component/foreign descendant outcomes transfer exact inert residuals to the private quarantine table; full background reaping remains out of scope.

- [ ] **Step 4: Implement the root-only facade journey**

Use the Task 9 Root-profile facade wrappers so the journey cannot import whole owner crates or deferred methods. On normal root failure, the engine cancels history intent and issues a fresh `RouterFallback` task with a new frame/task/generation and the same confirmed top slot. Fallback success uses the same preflight/publication protocol to commit `ActivePresentation::Error`; fallback failure settles its own staging output and leaves presentation/history unchanged.

The root cases in `public_navigation_journeys.rs` use only `bevy_ui_router::prelude::*`, register an immediate driver, install single-node routes and fallback, create an app-owned Node host with a sentinel and existing child, and settle each request within 16 updates. They prove first push, equivalent repeated push, fragment-only remount, and changed-root replacement; assertions cover history, ordered commit/cleanup/terminal trace, old-root isolation/removal, real active `SolidRect`, untouched host-owned state, `ActivePresentation::matched() == None` for match failure, and the original immutable fingerprint for post-match build/apply recovery.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --locked -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys root_push`

Run: `cargo test --locked -p bevy_ui_router_ecs`

Commit: `feat: complete root push vertical slice`

### Task 11: Curated Facade And Closed-World Contract Gates

**Files:**
- Rewrite: `crates/bevy_ui_router/src/lib.rs`
- Rewrite or replace: existing facade boundary tests
- Create: the Root-active fixed consumer targets listed in Step 5; later slices add cases/targets owned by their profile without renaming existing files
- Create: `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`
- Create: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`
- Create: `scripts/verify-authority-regressions.ps1`

**Interfaces:**
- Consumes: completed owner APIs.
- Produces: Root profile-exact facade modules/prelude, positive compile coverage, negative authority coverage, associated API subset completeness, and executable authority catalogue completeness. The full target contract remains pending.

- [ ] **Step 1: Write RED gates against the frozen allowlist and golden**

Parse Markdown tables and the line-oriented golden with dedicated deterministic parsers, parse Cargo manifests with a TOML parser, and use `syn` only for Rust source structure/signatures. Apply the exact exclusions in `docs/root-push-contract-profile.md`; require every remaining module/path/prelude symbol and associated item to exist, reject every unexpected public symbol/item, compute the public signature type-graph closure, and generate feature-specific forbidden consumers with expected rustc error categories. A zero-case catalogue is a failure.

The owner-source gate also proves every profile-excluded inherent method is absent or non-public in its owner crate. Facade `pub use` cannot hide inherent methods, so excluded methods may not be implemented as public owner methods before their slice.

- [ ] **Step 2: Verify RED**

Run: `cargo test --locked -p bevy_ui_router --all-features --test facade_export_allowlist`

Run: `cargo test --locked -p bevy_ui_router --all-features --test facade_authority_compile_fail`

Expected: the minimal cutover already rejects old authority paths, while incomplete positive/source/golden/profile coverage and any remaining extra Root-profile item keep the full closed-world gates RED.

- [ ] **Step 3: Complete the curated wrappers against the closed-world gates**

Refine the Task 9 wrappers until they export every and only Root profile row. Keep default Bevy-free. Under `ecs`, expose the complete public Root push facade without exposing `RouterEngine`, deferred methods/symbols, or ECS private authority. `bsn` remains an empty reserved module for this phase.

- [ ] **Step 4: Implement the fixed isolated consumer harness**

Generate cases only under `target/facade-contract-cases`, give every case its own `[workspace]` and lock, use canonical facade path, and run `cargo generate-lockfile --offline`, `cargo tree --locked --offline`, and `cargo check --locked --offline` against the shared target directory.

Include independent `--no-default-features --features platform` and `--no-default-features --features devtools` generated consumers with their own locks. The platform case imports and uses the router-neutral back reexports; the devtools case is checked against the closed-world source/consumer catalogue and must add no public symbol.

- [ ] **Step 5: Verify the exact Root push consumer matrix**

Run each command separately:

```text
cargo test --locked -p bevy_ui_router --test default_facade_features
cargo test --locked -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys root_push
cargo test --locked -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit root_push
cargo test --locked -p bevy_ui_router --no-default-features --features ecs --test ecs_facade_features
cargo test --locked -p bevy_ui_router --no-default-features --features bsn --test bsn_facade_features
cargo test --locked -p bevy_ui_router --no-default-features --features test-support --test test_support_features
cargo test --locked -p bevy_ui_router --all-features --test facade_export_allowlist
cargo test --locked -p bevy_ui_router --all-features --test facade_authority_compile_fail
cargo check --locked -p bevy_ui_router --no-default-features --features platform
cargo check --locked -p bevy_ui_router --no-default-features --features devtools
```

The full `public_navigation_input_isolation` target and non-Root cases in the fixed targets remain pending until their owning slices. Passing the eight test-target commands plus the two isolated feature checks proves only the Root profile; the generated harness additionally records the independent platform/devtools consumer locks.

- [ ] **Step 6: Verify authority catalogue execution and commit**

`scripts/verify-authority-regressions.ps1` must compare three exact sets/counts: catalogue entries parsed from `authority_regression_catalogue.rs`, tests actually registered by the compiled ECS test binary, and tests that actually passed. It fails on missing, extra, duplicate, filtered-out, or zero cases.

The script internally runs `cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib authority_regression_tests:: -- --list`, compares the registered names, then runs the same package/lib/filter without `--list` and parses the final passed/failed/ignored/measured counts.

Run: `powershell -ExecutionPolicy Bypass -File scripts/verify-authority-regressions.ps1`

Run: `cargo test --locked -p bevy_ui_router --all-features`

Run: `cargo test --locked -p bevy_ui_router_ecs --all-features`

Commit: `test: enforce closed root push facade`

### Task 12: Root Push Completion Gate

**Files:**
- Modify: `docs/current-architecture.md`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-07-11-root-push-review-log.md`

**Interfaces:**
- Consumes: all prior commits and their test evidence.
- Produces: `Task PASS` for Root push only; does not claim `Journey PASS` for nested MVP Journey A or `MVP PASS`.

- [ ] **Step 1: Run formatting and static gates**

Run: `cargo fmt --all --check`

Run: `cargo check --locked --config "build.rustc-wrapper = ''" --workspace --all-targets --all-features`

- [ ] **Step 2: Run targeted and workspace tests**

Run: every GREEN/final command recorded in Tasks 1-11. Do not rerun commands whose stated purpose is to verify RED failure.

Run: `cargo nextest run --locked --workspace --all-features`

Run: `cargo test --locked --config "build.rustc-wrapper = ''" --workspace --all-features`

- [ ] **Step 3: Run dependency and consumer evidence**

Run: `cargo metadata --locked --config "build.rustc-wrapper = ''" --format-version 1 --no-deps`

Run: `cargo tree --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs -e features -i bevy_ui`

Run: `cargo tree --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --target all -e features,dev -i bevy_internal`

Run: `powershell -ExecutionPolicy Bypass -File scripts/verify-authority-regressions.ps1`

Confirm exact revision, picking integration, absence of `ghost_nodes`, tracked lock, nonempty positive/negative consumer cases, Root profile closure, and catalogue/binary/passing-set equality.

- [ ] **Step 4: Dispatch independent reviews**

Use separate `gpt-5.6-sol` high-reasoning reviewers for factual Bevy/manifest verification, authority/lifecycle correctness, and facade capability closure; use `gpt-5.6-terra` high-reasoning for implementation/test adequacy. Each returns `PASS` or evidence-backed findings. Record model, role, result, finding disposition, and re-review result in the review log; all four roles must return PASS after fixes.

- [ ] **Step 5: Update facts and commit**

Update Current Architecture with implemented/integrated/facade-reachable/demonstrated evidence. Mark only the Roadmap Root push slice complete. Record commands, outputs, review findings, dispositions, and commit range in the review log.

Commit: `docs: record root push task pass`
