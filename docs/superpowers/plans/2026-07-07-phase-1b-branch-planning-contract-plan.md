# Phase 1B Branch Planning Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the data-only branch planning contract that Phase 2 needs: branch diff, raw params/search fingerprints, named slot placement, route metadata, and error-boundary declaration candidates.

**Architecture:** Phase 1B extends the Phase 1A matcher output inside `bevy_ui_router_core`. It produces data contracts for future frame planning, but it does not allocate frames, mutate ECS hierarchy, apply views, mark UI active, run async work, or choose renderer behavior.

**Tech Stack:** Rust 2024, workspace crates, zero runtime dependencies for `bevy_ui_router_core`, existing Cargo test and dependency-boundary checks.

**Current Repository Status:** Phase 1A is implemented, reviewed, and verified as of `8fc476a docs: record phase 1a matcher verification`. Phase 1B remains pending until this refreshed plan passes review and the user explicitly authorizes implementation.

## Global Constraints

- Phase 1B can execute only after Phase 1A implementation is complete, reviewed, committed, and verified; current evidence is recorded in `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`.
- If Phase 1A implemented names or semantics differ from `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`, stop and revise this plan before implementation.
- Router core remains renderer-agnostic and must not depend on Bevy ECS, BSN, platform APIs, application services, themes, permissions, data services, or application shell models.
- Phase 1B must not implement transaction runtime, async resolve/apply, frame lifecycle mutation, retained subtree behavior, focus behavior, ECS host/slot behavior, or BSN view application.
- A `MatchedBranch`, `BranchDiff`, or fingerprint is data-only planning input. None of them is active UI.
- Branch diff output must not contain Bevy `Entity`, frame root, frame generation, `RouteFrameId`, or host-owned cleanup authority.
- Named slot placement is declaration data, not ECS mount mutation.
- Error-boundary candidates are declaration metadata, not fallback rendering behavior.
- Raw search fingerprint policy is frozen in Phase 1B: no parsing, no percent-decoding, no key sorting, and no app payload participation.
- Segment reuse keys are lifecycle handoff data. They must include only route id, slot key, explicit key, and local params fingerprint.
- Route-tree provenance is a branch-level comparability gate. It must not participate in `RouteSegmentReuseKey` or `RouteBranchFingerprint`.
- All reuse keys and fingerprints must use typed length-prefixed structural encoding for every dynamic component. Separator-only concatenation is not valid.
- Diagnostic labels, route metadata, and error-boundary declaration candidates must not participate in segment reuse keys.
- Branch diff must fail closed when old and new branches come from different route-tree provenance.
- Matching and diff ordering must be deterministic and must not depend on Bevy query iteration order, filesystem order, hash iteration order, or accidental child insertion order beyond explicit route declaration order encoded by core.
- Route tree public `Debug`, `Eq`, and `PartialEq` behavior must not expose or make observable internal matcher inputs such as `children`, `declaration_index`, or provenance.
- The core manifest must keep dependency sections empty.
- Public API additions must be exported through `bevy_ui_router_core::prelude` only when they are stable Phase 1B contract surface.
- `lib.rs` and `prelude` snippets in tasks are additive examples unless a step explicitly says to remove or replace an existing export. Preserve existing Phase 0 / Phase 1A prelude exports while adding Phase 1B entries.
- Coverage and boundary-verification steps are not RED steps unless explicitly labeled RED. They may pass immediately when earlier tasks already implemented the behavior.
- Every filtered `cargo test` command in this plan is invalid evidence if it exits 0 with `running 0 tests`.
- Any test intended to be run with a `module::tests::test_name` filter must be inserted inside that module's existing `#[cfg(test)] mod tests` block, or the command in the plan must be updated to match the actual test path before it can be used as evidence.

---

## 1. Preconditions

Before executing this plan, verify:

- Phase 1A implementation and review-fix commits exist, ending with `8fc476a docs: record phase 1a matcher verification` or a later commit that preserves the same evidence.
- The worktree is clean.
- `crates/bevy_ui_router_core/src/pattern.rs` exists.
- `crates/bevy_ui_router_core/src/branch.rs` exists.
- `crates/bevy_ui_router_core/src/route_tree.rs` exists.
- `crates/bevy_ui_router_core/src/matcher.rs` exists.
- Phase 1A tests and boundary checks have passing evidence.

Run:

```powershell
git status --short
Test-Path crates\bevy_ui_router_core\src\pattern.rs
Test-Path crates\bevy_ui_router_core\src\branch.rs
Test-Path crates\bevy_ui_router_core\src\route_tree.rs
Test-Path crates\bevy_ui_router_core\src\matcher.rs
```

Expected: `git status --short` prints nothing, and every `Test-Path` command prints `True`.

If any precondition fails, do not execute Phase 1B. Return to Phase 1A implementation or revise this plan.

As of this plan refresh, the expected Phase 1A source files exist and the repository has Phase 1A verification evidence. Task 0 remains mandatory at execution time because the working tree and source state can change after planning.

---

## 2. Phase 1B Scope

Phase 1B completes the rest of the wider Phase 1 route contract that was deliberately deferred from Phase 1A:

- Branch-level raw params/search fingerprint.
- Per-route segment diff key for common-prefix branch diff.
- Named slot placement as declaration metadata.
- Route explicit key for remount/reuse decisions.
- Route metadata boundary without app-specific fields.
- Error-boundary declaration candidates as matched-branch metadata.
- Branch diff fixtures for sibling, child, params change, layout/provenance change, slot/provenance change, search change, and explicit key/provenance change.

Phase 1B intentionally does not include:

- Frame id allocation.
- Generation allocation.
- Transaction state.
- Active branch commit.
- Error boundary rendering.
- View build/apply.
- ECS hierarchy mutation.
- Retained subtree cache.
- Adapter integration.

---

## 3. File Structure

- Create `crates/bevy_ui_router_core/src/slot.rs`
  - Owns `RouteSlotKey` and named/default slot declaration keys.
- Create `crates/bevy_ui_router_core/src/meta.rs`
  - Owns `RouteMeta`, `RouteExplicitKey`, and `ErrorBoundaryKey`.
- Create `crates/bevy_ui_router_core/src/fingerprint.rs`
  - Owns `RouteParamsFingerprint`, `RouteSearchFingerprint`, `RouteSegmentReuseKey`, `RouteBranchFingerprint`, and `RouteBranchProvenance`.
- Create `crates/bevy_ui_router_core/src/branch_diff.rs`
  - Owns `BranchDiff`, `BranchDiffRoute`, and common-prefix comparison.
- Modify `crates/bevy_ui_router_core/src/route_tree.rs`
  - Add `RouteNodeOptions` and `route_with_options`.
- Modify `crates/bevy_ui_router_core/src/branch.rs`
  - Extend `MatchedRoute` and `MatchedBranch` with slot, explicit key, metadata, error-boundary candidate, and fingerprint accessors.
- Modify `crates/bevy_ui_router_core/src/matcher.rs`
  - Populate Phase 1B branch planning fields when producing `MatchedBranch`.
- Modify `crates/bevy_ui_router_core/src/diagnostics.rs`
  - Add branch-planning diagnostics with a structured planning payload and no active frame identity.
- Modify `crates/bevy_ui_router_core/src/error.rs`
  - Add only the route declaration error kind required for local declaration value validation if it is not already present.
- Modify `crates/bevy_ui_router_core/src/lib.rs`
  - Export new modules and stable Phase 1B prelude types.
- Modify `crates/bevy_ui_router_core/tests/dependency_boundary.rs`
  - Preserve zero-dependency and public constructor boundary checks.
- Modify `crates/bevy_ui_router/tests/facade_features.rs`
  - Verify facade re-exports Phase 1B stable core contracts under `--no-default-features`.

---

## 4. Public Contract

Phase 1B should leave these stable shapes available as public module APIs unless noted otherwise. Keep the default prelude smaller than the full module surface: prelude exports are for route declaration authoring and high-level branch diff use, not for every internal handoff/debug wrapper.

```rust
pub struct RouteSlotKey;

impl RouteSlotKey {
    pub fn default() -> Self;
    pub fn named(name: impl Into<String>) -> Result<Self, RouteError>;
    pub fn as_str(&self) -> &str;
    pub fn is_default(&self) -> bool;
}

pub struct RouteExplicitKey;

impl RouteExplicitKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RouteError>;
    pub fn as_str(&self) -> &str;
}

pub struct ErrorBoundaryKey;

impl ErrorBoundaryKey {
    pub fn named(name: impl Into<String>) -> Result<Self, RouteError>;
    pub fn as_str(&self) -> &str;
}

pub struct RouteMeta;

impl RouteMeta {
    pub fn new() -> Self;
    pub fn with_diagnostic_label(self, label: impl Into<String>) -> Result<Self, RouteError>;
    pub fn diagnostic_label(&self) -> Option<&str>;
}

pub struct RouteNodeOptions;

impl RouteNodeOptions {
    pub fn new() -> Self;
    pub fn with_slot(self, slot: RouteSlotKey) -> Self;
    pub fn with_explicit_key(self, key: RouteExplicitKey) -> Self;
    pub fn with_error_boundary(self, key: ErrorBoundaryKey) -> Self;
    pub fn with_meta(self, meta: RouteMeta) -> Self;
    pub fn slot(&self) -> &RouteSlotKey;
    pub fn explicit_key(&self) -> Option<&RouteExplicitKey>;
    pub fn error_boundary(&self) -> Option<&ErrorBoundaryKey>;
    pub fn meta(&self) -> &RouteMeta;
}

impl RouteTreeBuilder {
    pub fn route_with_options(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> Result<RouteId, RouteError>;
}

pub struct RouteParamsFingerprint;
pub struct RouteSearchFingerprint;
pub struct RouteSegmentReuseKey;
pub struct RouteBranchFingerprint;

impl RouteParamsFingerprint {
    pub fn as_str(&self) -> &str;
}

impl RouteSearchFingerprint {
    pub fn as_str(&self) -> &str;
}

impl RouteSegmentReuseKey {
    pub fn as_str(&self) -> &str;
}

impl RouteBranchFingerprint {
    pub fn as_str(&self) -> &str;
}

impl MatchedRoute {
    pub fn slot(&self) -> &RouteSlotKey;
    pub fn explicit_key(&self) -> Option<&RouteExplicitKey>;
    pub fn error_boundary(&self) -> Option<&ErrorBoundaryKey>;
    pub fn meta(&self) -> &RouteMeta;
    pub fn params_fingerprint(&self) -> &RouteParamsFingerprint;
    pub fn segment_reuse_key(&self) -> &RouteSegmentReuseKey;
}

impl MatchedBranch {
    pub fn search_fingerprint(&self) -> &RouteSearchFingerprint;
    pub fn branch_fingerprint(&self) -> &RouteBranchFingerprint;
    pub fn nearest_error_boundary_for(&self, route: RouteId) -> Option<&ErrorBoundaryKey>;
}

pub struct BranchDiff;

impl BranchDiff {
    pub fn compare(old: &MatchedBranch, new: &MatchedBranch) -> Self;
    pub fn common_prefix_len(&self) -> usize;
    pub fn leaving(&self) -> &[BranchDiffRoute];
    pub fn entering(&self) -> &[BranchDiffRoute];
    pub fn provenance_changed(&self) -> bool;
    pub fn search_changed(&self) -> bool;
    pub fn branch_fingerprint_changed(&self) -> bool;
}

pub struct BranchDiffRoute;

impl BranchDiffRoute {
    pub fn route(&self) -> RouteId;
    pub fn index(&self) -> usize;
    pub fn slot(&self) -> &RouteSlotKey;
}

pub struct BranchPlanningDiagnostic;

impl BranchPlanningDiagnostic {
    pub fn new(
        common_prefix_len: usize,
        leaving_len: usize,
        entering_len: usize,
        provenance_changed: bool,
        search_changed: bool,
        branch_fingerprint_changed: bool,
    ) -> Self;
    pub fn common_prefix_len(&self) -> usize;
    pub fn leaving_len(&self) -> usize;
    pub fn entering_len(&self) -> usize;
    pub fn provenance_changed(&self) -> bool;
    pub fn search_changed(&self) -> bool;
    pub fn branch_fingerprint_changed(&self) -> bool;
}
```

Public constructors for fingerprints remain core-internal. External code may read fingerprint strings for diagnostics and tests, but cannot construct a fake matched branch fingerprint through the public API. `RouteBranchProvenance` remains core-internal and is externally observable only through `BranchDiff::provenance_changed()`.

### 4.1 Prelude Surface Policy

Phase 1B prelude additions are limited to:

```rust
pub mod prelude {
    pub use crate::branch_diff::BranchDiff;
    pub use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    pub use crate::route_tree::RouteNodeOptions;
    pub use crate::slot::RouteSlotKey;
}
```

Do not add `RouteParamsFingerprint`, `RouteSearchFingerprint`, `RouteSegmentReuseKey`, `RouteBranchFingerprint`, `BranchDiffRoute`, `BranchPlanningDiagnostic`, or `RouteBranchProvenance` to the prelude in Phase 1B. They remain public module APIs where explicitly public, or core-internal where marked `pub(crate)`. This keeps branch-planning handoff/debug data from becoming the default app-facing lifecycle surface.

---

## 5. Semantics

### 5.1 Slot Keys

- `RouteSlotKey::default()` represents the unnamed outlet.
- `RouteSlotKey::named("details")` represents a declaration-level named outlet.
- Slot names must be non-empty ASCII strings containing only letters, digits, `_`, `-`, and `.`.
- Slot keys are not Bevy entities and are not ECS `RouteSlotId`.
- Slot keys participate in `RouteSegmentReuseKey`. If a future reviewed route-tree update path preserves provenance while changing a slot at the same route index, that change must break common-prefix reuse at that index.

### 5.2 Explicit Keys

- `RouteExplicitKey` is declaration data that can force remount/reuse boundaries.
- Explicit keys participate in `RouteSegmentReuseKey`.
- Explicit keys may contain separator-like characters. Reuse-key safety comes from typed length-prefixed encoding, not from delimiter restrictions.
- If a future reviewed route-tree update path preserves provenance while changing an explicit key at the same route index, that change must break common-prefix reuse at that route index.
- Explicit keys do not expose or alter `RouteFrameId` or `RouteGeneration`.

### 5.3 Raw Search Fingerprint

Phase 1B freezes this raw search policy:

- `RouteLocation::search() == None` fingerprints differently from `Some("")`.
- A leading `?` is not stripped by fingerprinting.
- Key order is meaningful: `a=1&b=2` differs from `b=2&a=1`.
- Repeated keys are meaningful: `a=1&a=2` differs from `a=2&a=1`.
- Empty value and missing value are meaningful raw strings: `a=` differs from `a`.
- Percent encoding is not normalized: `%7E` differs from `~`, and `%2f` differs from `%2F`.
- Fragment is not part of the search fingerprint.
- The current `RouteLocation` API has no app payload or extension field. Phase 1B verifies payload exclusion as an API boundary, not as a runtime fingerprint fixture. If a future payload/extension field is added, that change must add explicit fingerprint tests before the field can affect navigation identity.

### 5.4 Params Fingerprint

- Each matched route keeps local params from its own pattern.
- `RouteParamsFingerprint` is derived from route id plus local params in deterministic name order.
- Ancestor params do not silently appear in a leaf route's local params fingerprint.
- `RouteSegmentReuseKey` is derived from route id, slot key, explicit key, and local params fingerprint.
- `RouteBranchFingerprint` is derived from ordered segment reuse keys plus raw search fingerprint.
- Error-boundary declarations, route metadata, and diagnostic labels are branch metadata only. They must not participate in `RouteSegmentReuseKey` or `RouteBranchFingerprint`.

### 5.5 Branch Provenance

- `RouteBranchProvenance` is a core-internal source identity allocated by the `RouteTree` / `RouteMatcher` source that produced a `MatchedBranch`.
- It is not a structural fingerprint. Two independently built route trees must have different provenance even when they have identical routes, ids, slots, keys, params, metadata, and resulting fingerprint strings, unless a future reviewed route-tree update path explicitly preserves provenance.
- Phase 1B allocates provenance in `RouteTreeBuilder::build()` using a core-internal monotonic source id. `RouteTree::clone()` and `RouteMatcher::clone()` preserve the same source identity; independent `build()` calls must produce different source identities.
- `RouteBranchProvenance` type, constructors, string/debug access, and matched-branch accessor remain core-internal. It is not exported through `prelude`.
- `RouteBranchProvenance` is not a `RouteFrameId`, not a `RouteGeneration`, not a stale-result token, and not adapter-forgeable runtime authority.
- Two branches with different provenance must not share a common prefix, even if their route ids and reuse keys have the same raw values.
- This avoids treating tree-local `RouteId` values as globally stable identities.

### 5.6 Branch Diff

`BranchDiff::compare(old, new)` is data-only:

- It compares matched route chains by index.
- If branch provenance differs, `common_prefix_len()` is `0`, `provenance_changed()` is `true`, the old branch fully leaves, and the new branch fully enters.
- If raw search fingerprint differs, Phase 1B default policy also sets `common_prefix_len()` to `0`, `search_changed()` to `true`, the old branch fully leaves, and the new branch fully enters.
- A segment is reusable only when branch provenance matches, raw search fingerprint matches at the branch level, and route id, slot key, explicit key, and segment reuse key all match.
- Common prefix stops at the first non-reusable segment.
- `leaving()` returns old suffix routes from leaf to root order.
- `entering()` returns new suffix routes from root to leaf order.
- `search_changed()` reports raw search fingerprint difference. Under the Phase 1B default policy, a search difference prevents common-prefix reuse until a future reviewed per-route policy narrows that behavior.
- `branch_fingerprint_changed()` reports full branch fingerprint difference.
- Phase 1B has no provenance-preserving route-tree update path. Declaration changes such as slot or explicit-key changes are therefore covered as reuse-key inputs and as cross-provenance fail-closed cases, not as same-provenance hot-update diff cases.
- It never allocates frame ids, generations, cleanup scopes, or ECS entities.

### 5.7 Error-Boundary Candidates

- Error-boundary declaration is a route node option.
- `MatchedBranch::nearest_error_boundary_for(route)` searches ancestors in the matched route chain, including the route itself.
- The result is metadata for Phase 2 error handling only.
- Phase 1B does not build fallback UI and does not change history.

---

## 6. Coverage Matrix

| Requirement | Test Name | Command | Expected Evidence |
| --- | --- | --- | --- |
| Slot key declaration and validation | `slot::tests` | `cargo test -p bevy_ui_router_core slot::tests -- --nocapture` | Nonzero tests; pass after implementation |
| Metadata and declaration key validation | `meta::tests` | `cargo test -p bevy_ui_router_core meta::tests -- --nocapture` | Nonzero tests; pass after implementation |
| Route node options carry slot/key/boundary/meta | `route_tree::tests::route_options_attach_slot_key_boundary_and_meta_to_nodes` | `cargo test -p bevy_ui_router_core route_tree::tests::route_options_attach_slot_key_boundary_and_meta_to_nodes -- --nocapture` | Nonzero tests; pass after implementation |
| Default route node options | `route_tree::tests::route_without_options_uses_default_slot_and_empty_meta` | `cargo test -p bevy_ui_router_core route_tree::tests::route_without_options_uses_default_slot_and_empty_meta -- --nocapture` | Nonzero tests; pass after implementation |
| Params fingerprint structural encoding | `fingerprint::tests::params_fingerprint_uses_route_id_and_sorted_local_params` and `params_fingerprint_is_structural_when_values_contain_separators` | `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture` | Fingerprint strings are length-prefixed and deterministic |
| Raw search policy | `fingerprint::tests::raw_search_fingerprint_preserves_absence_empty_order_and_encoding` | `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture` | Absence/empty, leading `?`, ordering, repeated keys, empty/missing values, percent encoding, percent case, and fragment exclusion are covered |
| Segment reuse and branch fingerprint encoding | `fingerprint::tests::segment_reuse_key_and_branch_fingerprint_use_typed_structural_encoding` | `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture` | Slot, explicit key, params fingerprint, segment list, and search fingerprint are encoded with typed length prefixes |
| Matched branch carries planning metadata | `matcher::tests::matched_branch_carries_slot_metadata_boundaries_and_fingerprints` | `cargo test -p bevy_ui_router_core matcher::tests::matched_branch_carries_slot_metadata_boundaries_and_fingerprints -- --nocapture` | Metadata is readable but not part of segment reuse key or branch fingerprint |
| Local params stay local in fingerprints | `matcher::tests::matched_branch_fingerprints_do_not_merge_ancestor_params` | `cargo test -p bevy_ui_router_core matcher::tests::matched_branch_fingerprints_do_not_merge_ancestor_params -- --nocapture` | Leaf params fingerprint excludes ancestor captures |
| Metadata exclusion by equality | `matcher::tests::metadata_and_error_boundaries_do_not_change_reuse_or_branch_fingerprints` | `cargo test -p bevy_ui_router_core matcher::tests::metadata_and_error_boundaries_do_not_change_reuse_or_branch_fingerprints -- --nocapture` | Changing only metadata/boundary leaves reuse keys and branch fingerprints equal |
| Params change diff | `branch_diff::tests::diff_preserves_common_prefix_and_orders_suffixes` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Common prefix and suffix ordering are covered |
| Sibling change diff | `branch_diff::tests::sibling_route_change_preserves_only_shared_parent` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Only shared parent is preserved |
| Child/depth change diff | `branch_diff::tests::child_route_change_enters_new_suffix_from_root_to_leaf` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | New child suffix enters root-to-leaf |
| Layout/provenance change | `branch_diff::tests::layout_shape_change_uses_provenance_fail_closed` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Cross-provenance diff has common prefix zero |
| Structurally identical different trees | `branch_diff::tests::identical_independent_trees_fail_closed_by_provenance` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Identical independent route trees still fail closed |
| Slot/provenance change | `branch_diff::tests::different_tree_provenance_replaces_entire_branch_even_when_route_ids_match` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Cross-tree slot change fails closed |
| Explicit key/provenance change | `branch_diff::tests::explicit_key_change_in_different_tree_also_fails_closed` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Cross-tree explicit key change fails closed |
| Multi-element suffix ordering | `branch_diff::tests::multi_level_suffix_orders_leaving_leaf_to_root_and_entering_root_to_leaf` | `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture` | Multi-route leave and enter suffix order is explicit |
| Search change | `branch_diff::tests::search_change_fails_closed_by_raw_search_policy` | `cargo test -p bevy_ui_router_core branch_diff::tests::search_change_fails_closed_by_raw_search_policy -- --nocapture` | Search changes fully replace by default |
| Fragment-only change | `branch_diff::tests::fragment_only_change_does_not_change_search_or_branch_diff` | `cargo test -p bevy_ui_router_core branch_diff::tests::fragment_only_change_does_not_change_search_or_branch_diff -- --nocapture` | Fragment changes do not change search fingerprint, branch fingerprint, or diff suffixes |
| Nearest error-boundary metadata | `matcher::tests::nearest_error_boundary_uses_matched_ancestor_chain_only` | `cargo test -p bevy_ui_router_core matcher::tests::nearest_error_boundary_uses_matched_ancestor_chain_only -- --nocapture` | Nearest matched ancestor boundary is returned; off-branch routes return none |
| Branch-planning diagnostics | `diagnostics::tests::branch_planning_event_has_no_active_identity` | `cargo test -p bevy_ui_router_core diagnostics::tests::branch_planning_event_has_no_active_identity -- --nocapture` | Structured payload is present, `identity().is_none()`, redacted message is fixed, and no frame/generation text leaks |
| Non-prelude module exports | `public_api::non_prelude_branch_planning_support_types_are_public_module_api` | `cargo test -p bevy_ui_router_core --test public_api non_prelude_branch_planning_support_types_are_public_module_api -- --nocapture` | External integration test names fingerprint wrappers, `BranchDiffRoute`, and `diagnostics::BranchPlanningDiagnostic` through module paths without relying on prelude |
| Constructor and comparable-trait authority boundaries | core doctests | `cargo test -p bevy_ui_router_core --doc` | Nonzero doctest evidence; external construction of fingerprints, matched routes, and matched branches fails to compile; provenance is not exposed through public API; route-tree internals do not gain public `PartialEq` comparability |
| Default prelude/facade contract exports | `facade_prelude_exposes_branch_planning_contracts` | `cargo test -p bevy_ui_router --no-default-features facade_prelude_exposes_branch_planning_contracts -- --nocapture` | Positive external naming/import coverage for app-facing default prelude types; non-prelude branch planning support types are covered by public module-path integration tests |
| Core dependency boundary | `dependency_boundary` integration tests | `cargo test -p bevy_ui_router_core --test dependency_boundary -- --nocapture` | Core manifest remains dependency-free |

Every command in this table is invalid evidence if it exits 0 with `running 0 tests`.

---

## 7. Implementation Tasks

### Task 0: Confirm Phase 1A Gate

**Files:**
- Read: `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`
- Read: `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan-review-log.md`
- Read: `crates/bevy_ui_router_core/src/lib.rs`
- Read: `crates/bevy_ui_router_core/src/pattern.rs`
- Read: `crates/bevy_ui_router_core/src/branch.rs`
- Read: `crates/bevy_ui_router_core/src/route_tree.rs`
- Read: `crates/bevy_ui_router_core/src/matcher.rs`

**Interfaces:**
- Consumes: Phase 1A public API and verification evidence.
- Produces: authorization to execute Phase 1B tasks.

- [ ] **Step 1: Confirm files and worktree**

Run:

```powershell
git status --short
Test-Path crates\bevy_ui_router_core\src\pattern.rs
Test-Path crates\bevy_ui_router_core\src\branch.rs
Test-Path crates\bevy_ui_router_core\src\route_tree.rs
Test-Path crates\bevy_ui_router_core\src\matcher.rs
```

Expected: status prints nothing and every `Test-Path` prints `True`.

- [ ] **Step 2: Confirm Phase 1A verification evidence**

Run:

```powershell
git log --oneline -12
rg -n "pub struct RoutePattern|pub struct RouteTree|pub struct MatchedBranch|pub struct RouteMatcher" crates\bevy_ui_router_core\src
cargo test -p bevy_ui_router_core pattern::tests -- --nocapture
cargo test -p bevy_ui_router_core route_tree::tests -- --nocapture
cargo test -p bevy_ui_router_core matcher::tests -- --nocapture
cargo test -p bevy_ui_router_core branch::tests -- --nocapture
```

Expected: Phase 1A implementation evidence is present in source files, not only in planning docs. The log includes Phase 1A implementation, review-fix, and verification commits such as `dcd2837`, `b6d5c96`, and `8fc476a` or later equivalent commits. Each filtered test command reports nonzero tests and passes. If Phase 1A has not been implemented and verified, or any command exits 0 with `running 0 tests`, stop and report that Phase 1B is blocked.

- [ ] **Step 3: Commit**

No commit for this task unless the plan document records gate evidence during execution.

### Task 1: Slot And Route Metadata Declarations

**Files:**
- Create: `crates/bevy_ui_router_core/src/slot.rs`
- Create: `crates/bevy_ui_router_core/src/meta.rs`
- Modify: `crates/bevy_ui_router_core/src/error.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouteError`, `RouteErrorKind`, `RouteErrorSourceCategory`.
- Produces: `RouteSlotKey`, `RouteExplicitKey`, `ErrorBoundaryKey`, `RouteMeta`.

- [ ] **Step 1: Wire modules and write RED tests**

Add `pub mod slot;` and `pub mod meta;` to `crates/bevy_ui_router_core/src/lib.rs`, then create tests in `slot.rs` and `meta.rs`.

`crates/bevy_ui_router_core/src/slot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn slot_key_distinguishes_default_and_named_slots() {
        let default_slot = RouteSlotKey::default();
        let details = RouteSlotKey::named("details").unwrap();

        assert!(default_slot.is_default());
        assert_eq!(default_slot.as_str(), "");
        assert_eq!(details.as_str(), "details");
        assert!(!details.is_default());
    }

    #[test]
    fn slot_key_rejects_empty_or_unstable_names() {
        for name in ["", "two words", "with/slash"] {
            let error = RouteSlotKey::named(name).unwrap_err();
            assert_eq!(error.kind(), RouteErrorKind::InvalidRouteDeclaration);
            assert_eq!(error.source_category(), RouteErrorSourceCategory::RouterCore);
        }
    }
}
```

`crates/bevy_ui_router_core/src/meta.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn explicit_key_and_error_boundary_are_declaration_data() {
        let explicit = RouteExplicitKey::new("user-shell").unwrap();
        let boundary = ErrorBoundaryKey::named("root-boundary").unwrap();

        assert_eq!(explicit.as_str(), "user-shell");
        assert_eq!(boundary.as_str(), "root-boundary");
    }

    #[test]
    fn route_meta_exposes_only_generic_diagnostic_label() {
        let meta = RouteMeta::new()
            .with_diagnostic_label("settings route")
            .unwrap();

        assert_eq!(meta.diagnostic_label(), Some("settings route"));
    }

    #[test]
    fn declaration_keys_reject_empty_values() {
        for error in [
            RouteExplicitKey::new("").unwrap_err(),
            ErrorBoundaryKey::named("").unwrap_err(),
            RouteMeta::new().with_diagnostic_label("").unwrap_err(),
        ] {
            assert_eq!(error.kind(), RouteErrorKind::InvalidRouteDeclaration);
            assert_eq!(error.source_category(), RouteErrorSourceCategory::RouterCore);
        }
    }
}
```

Run: `cargo test -p bevy_ui_router_core slot::tests -- --nocapture`

Expected: compile failure or test failure because `RouteSlotKey` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid.

Run: `cargo test -p bevy_ui_router_core meta::tests -- --nocapture`

Expected: compile failure or test failure because metadata key types are not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid.

- [ ] **Step 2: Add declaration error kind**

Modify `crates/bevy_ui_router_core/src/error.rs` incrementally:

- Add `RouteErrorKind::InvalidRouteDeclaration` only if it is missing after Phase 1A.
- Do not replace the full enum in this task.
- Preserve every existing `RouteErrorKind` variant and derive from Phase 0 and Phase 1A.
- Do not add, remove, or reinterpret resolve/apply, stale-result, adapter, transaction, finalization, or cleanup error variants as part of Phase 1B.

Add constructor:

```rust
impl RouteError {
    pub(crate) fn invalid_route_declaration(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::InvalidRouteDeclaration,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }
}
```

- [ ] **Step 3: Implement slot keys**

Implement `crates/bevy_ui_router_core/src/slot.rs`:

```rust
use crate::error::RouteError;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSlotKey(String);

impl RouteSlotKey {
    pub fn default() -> Self {
        Self(String::new())
    }

    pub fn named(name: impl Into<String>) -> Result<Self, RouteError> {
        let name = name.into();
        validate_key(&name)?;
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_default(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for RouteSlotKey {
    fn default() -> Self {
        Self(String::new())
    }
}

fn validate_key(value: &str) -> Result<(), RouteError> {
    if value.is_empty() {
        return Err(RouteError::invalid_route_declaration("declaration key must not be empty"));
    }

    if !value
        .chars()
        .all(|ch| ch == '_' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric())
    {
        return Err(RouteError::invalid_route_declaration(
            "declaration key must be ascii alphanumeric, underscore, dash, or dot",
        ));
    }

    Ok(())
}
```

- [ ] **Step 4: Implement metadata keys**

Implement `crates/bevy_ui_router_core/src/meta.rs`:

```rust
use crate::error::RouteError;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteExplicitKey(String);

impl RouteExplicitKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RouteError> {
        let value = value.into();
        validate_non_empty(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ErrorBoundaryKey(String);

impl ErrorBoundaryKey {
    pub fn named(name: impl Into<String>) -> Result<Self, RouteError> {
        let name = name.into();
        validate_non_empty(&name)?;
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RouteMeta {
    diagnostic_label: Option<String>,
}

impl RouteMeta {
    pub fn new() -> Self {
        Self {
            diagnostic_label: None,
        }
    }

    pub fn with_diagnostic_label(mut self, label: impl Into<String>) -> Result<Self, RouteError> {
        let label = label.into();
        validate_non_empty(&label)?;
        self.diagnostic_label = Some(label);
        Ok(self)
    }

    pub fn diagnostic_label(&self) -> Option<&str> {
        self.diagnostic_label.as_deref()
    }
}

fn validate_non_empty(value: &str) -> Result<(), RouteError> {
    if value.is_empty() {
        return Err(RouteError::invalid_route_declaration("declaration value must not be empty"));
    }

    Ok(())
}
```

- [ ] **Step 5: Export stable types**

Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod meta;
pub mod slot;

pub mod prelude {
    pub use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
    pub use crate::slot::RouteSlotKey;
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p bevy_ui_router_core slot::tests -- --nocapture`

Expected: all slot tests pass.

Run: `cargo test -p bevy_ui_router_core meta::tests -- --nocapture`

Expected: all metadata tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/error.rs crates/bevy_ui_router_core/src/slot.rs crates/bevy_ui_router_core/src/meta.rs
git commit -m "feat(core): add route declaration planning keys"
```

### Task 2: Route Node Options

**Files:**
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`

**Interfaces:**
- Consumes: `RouteSlotKey`, `RouteExplicitKey`, `ErrorBoundaryKey`, `RouteMeta`, `RoutePattern`, `RouteId`.
- Produces: `RouteNodeOptions` and `RouteTreeBuilder::route_with_options`.

- [ ] **Step 1: Add RED route option tests**

Add tests inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
use crate::slot::RouteSlotKey;

#[test]
fn route_options_attach_slot_key_boundary_and_meta_to_nodes() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    let options = RouteNodeOptions::new()
        .with_slot(RouteSlotKey::named("details").unwrap())
        .with_explicit_key(RouteExplicitKey::new("detail-v1").unwrap())
        .with_error_boundary(ErrorBoundaryKey::named("detail-boundary").unwrap())
        .with_meta(RouteMeta::new().with_diagnostic_label("detail").unwrap());

    let detail = builder
        .route_with_options(root, RoutePattern::path("details/:id").unwrap(), options)
        .unwrap();
    let tree = builder.build().unwrap();
    let node = tree.node(detail).unwrap();

    assert_eq!(node.options().slot().as_str(), "details");
    assert_eq!(
        node.options().explicit_key().map(RouteExplicitKey::as_str),
        Some("detail-v1")
    );
    assert_eq!(
        node.options().error_boundary().map(ErrorBoundaryKey::as_str),
        Some("detail-boundary")
    );
    assert_eq!(node.options().meta().diagnostic_label(), Some("detail"));
}

#[test]
fn route_without_options_uses_default_slot_and_empty_meta() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    let child = builder.route(root, RoutePattern::path("settings").unwrap()).unwrap();
    let tree = builder.build().unwrap();
    let node = tree.node(child).unwrap();

    assert!(node.options().slot().is_default());
    assert!(node.options().explicit_key().is_none());
    assert!(node.options().error_boundary().is_none());
    assert!(node.options().meta().diagnostic_label().is_none());
}
```

Run: `cargo test -p bevy_ui_router_core route_tree::tests::route_options_attach_slot_key_boundary_and_meta_to_nodes -- --nocapture`

Expected: compile failure or test failure because `RouteNodeOptions` and `route_with_options` do not exist.

Run: `cargo test -p bevy_ui_router_core route_tree::tests::route_without_options_uses_default_slot_and_empty_meta -- --nocapture`

Expected: compile failure or test failure because `RouteNodeOptions` and `route_with_options` do not exist.

- [ ] **Step 2: Implement route options**

Modify `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
use crate::slot::RouteSlotKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteNodeOptions {
    slot: RouteSlotKey,
    explicit_key: Option<RouteExplicitKey>,
    error_boundary: Option<ErrorBoundaryKey>,
    meta: RouteMeta,
}

impl RouteNodeOptions {
    pub fn new() -> Self {
        Self {
            slot: RouteSlotKey::default(),
            explicit_key: None,
            error_boundary: None,
            meta: RouteMeta::new(),
        }
    }

    pub fn with_slot(mut self, slot: RouteSlotKey) -> Self {
        self.slot = slot;
        self
    }

    pub fn with_explicit_key(mut self, key: RouteExplicitKey) -> Self {
        self.explicit_key = Some(key);
        self
    }

    pub fn with_error_boundary(mut self, key: ErrorBoundaryKey) -> Self {
        self.error_boundary = Some(key);
        self
    }

    pub fn with_meta(mut self, meta: RouteMeta) -> Self {
        self.meta = meta;
        self
    }

    pub fn slot(&self) -> &RouteSlotKey {
        &self.slot
    }

    pub fn explicit_key(&self) -> Option<&RouteExplicitKey> {
        self.explicit_key.as_ref()
    }

    pub fn error_boundary(&self) -> Option<&ErrorBoundaryKey> {
        self.error_boundary.as_ref()
    }

    pub fn meta(&self) -> &RouteMeta {
        &self.meta
    }
}

impl Default for RouteNodeOptions {
    fn default() -> Self {
        Self::new()
    }
}
```

Extend `RouteNode` with an `options: RouteNodeOptions` field and public read accessor:

```rust
impl RouteNode {
    pub fn options(&self) -> &RouteNodeOptions {
        &self.options
    }
}
```

Preserve the Phase 1A route-tree debug/equality boundary:

- Do not reintroduce derived `Eq` or `PartialEq` on `RouteNode`, `RouteTree`, `RouteTreeBuilder`, or `RouteMatcher`.
- Keep manual `Debug` implementations for route tree types.
- If `RouteNode` debug output includes options, it must still omit `children`, `declaration_index`, and any future provenance internals.

Extend builder:

```rust
impl RouteTreeBuilder {
    pub fn route_with_options(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> Result<RouteId, RouteError> {
        self.route_inner(parent, pattern, options)
    }

    pub fn route(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
    ) -> Result<RouteId, RouteError> {
        self.route_inner(parent, pattern, RouteNodeOptions::new())
    }
}
```

Use a private `route_inner` helper so both paths preserve the same parent check and declaration order.

- [ ] **Step 3: Export `RouteNodeOptions`**

Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod prelude {
    pub use crate::route_tree::{RouteNodeOptions, RouteTree, RouteTreeBuilder};
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p bevy_ui_router_core route_tree::tests -- --nocapture`

Expected: all route tree tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/route_tree.rs
git commit -m "feat(core): add route node planning options"
```

### Task 3: Raw Fingerprint Contracts

**Files:**
- Create: `crates/bevy_ui_router_core/src/fingerprint.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouteId`, `RouteParams`, `RouteLocation`, `RouteSlotKey`, `RouteExplicitKey`, `ErrorBoundaryKey`, `RouteMeta`.
- Produces: `RouteParamsFingerprint`, `RouteSearchFingerprint`, `RouteSegmentReuseKey`, `RouteBranchFingerprint`, `RouteBranchProvenance`.

- [ ] **Step 1: Wire module and add RED fingerprint tests**

Add `pub mod fingerprint;` to `crates/bevy_ui_router_core/src/lib.rs`.

Create `crates/bevy_ui_router_core/src/fingerprint.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::RouteParams;
    use crate::ids::RouteId;
    use crate::location::RouteLocation;
    use crate::meta::RouteExplicitKey;
    use crate::slot::RouteSlotKey;

    #[test]
    fn params_fingerprint_uses_route_id_and_sorted_local_params() {
        let params = RouteParams::from_pairs([
            ("project".to_owned(), "alpha".to_owned()),
            ("document".to_owned(), "intro".to_owned()),
        ]);
        let fingerprint = RouteParamsFingerprint::from_route_params(RouteId::new(7), &params);

        assert_eq!(fingerprint.as_str(), "route:1:7|param:8:document=5:intro|param:7:project=5:alpha");
    }

    #[test]
    fn params_fingerprint_is_structural_when_values_contain_separators() {
        let params = RouteParams::from_pairs([
            ("a".to_owned(), "1;2".to_owned()),
            ("a_eq".to_owned(), "x=y".to_owned()),
        ]);
        let fingerprint = RouteParamsFingerprint::from_route_params(RouteId::new(3), &params);

        assert_eq!(fingerprint.as_str(), "route:1:3|param:1:a=3:1;2|param:4:a_eq=3:x=y");
    }

    #[test]
    fn raw_search_fingerprint_preserves_absence_empty_order_and_encoding() {
        let none = RouteSearchFingerprint::from_location(&RouteLocation::new("/items"));
        let empty = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search(""),
        );
        let leading_question = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("?a=1"),
        );
        let no_question = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=1"),
        );
        let ordered = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=1&b=2"),
        );
        let reversed = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("b=2&a=1"),
        );
        let encoded = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=%7E"),
        );
        let decoded = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=~"),
        );
        let lower_percent = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=%2f"),
        );
        let upper_percent = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("x=%2F"),
        );
        let repeated_a = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=1&a=2"),
        );
        let repeated_b = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a=2&a=1"),
        );
        let empty_value = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a="),
        );
        let missing_value = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items").with_search("a"),
        );
        let with_fragment = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items")
                .with_search("a=1")
                .with_fragment("ignored"),
        );

        assert_eq!(none.as_str(), "search:none");
        assert_eq!(empty.as_str(), "search:some:0:");
        assert_ne!(leading_question, no_question);
        assert_ne!(ordered, reversed);
        assert_ne!(encoded, decoded);
        assert_ne!(lower_percent, upper_percent);
        assert_ne!(repeated_a, repeated_b);
        assert_ne!(empty_value, missing_value);
        assert_eq!(no_question, with_fragment);
    }

    #[test]
    fn segment_reuse_key_and_branch_fingerprint_use_typed_structural_encoding() {
        let params = RouteParams::from_pairs([
            ("id".to_owned(), "4|2".to_owned()),
        ]);
        let params_fingerprint = RouteParamsFingerprint::from_route_params(RouteId::new(9), &params);
        let slot = RouteSlotKey::named("details").unwrap();
        let explicit_key = RouteExplicitKey::new("a|b=1").unwrap();
        let segment = RouteSegmentReuseKey::from_parts(
            RouteId::new(9),
            &slot,
            Some(&explicit_key),
            &params_fingerprint,
        );
        let search = RouteSearchFingerprint::from_location(
            &RouteLocation::new("/items/42").with_search("tab=a|b"),
        );
        let branch = RouteBranchFingerprint::from_parts([&segment], &search);

        assert_eq!(
            segment.as_str(),
            "segment|route:1:9|slot:7:details|key:5:a|b=1|params:26:route:1:9|param:2:id=3:4|2"
        );
        assert_eq!(
            branch.as_str(),
            "branch|segment:81:segment|route:1:9|slot:7:details|key:5:a|b=1|params:26:route:1:9|param:2:id=3:4|2|search:21:search:some:7:tab=a|b"
        );
    }
}
```

Run: `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture`

Expected: compile failure or test failure because fingerprint types are not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid.

- [ ] **Step 2: Implement fingerprint wrappers**

Implement `crates/bevy_ui_router_core/src/fingerprint.rs`:

```rust
use crate::branch::RouteParams;
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::meta::RouteExplicitKey;
use crate::slot::RouteSlotKey;

fn push_typed_field(raw: &mut String, field: &str, value: &str) {
    raw.push('|');
    raw.push_str(field);
    raw.push(':');
    raw.push_str(&value.len().to_string());
    raw.push(':');
    raw.push_str(value);
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteParamsFingerprint(String);

impl RouteParamsFingerprint {
    pub(crate) fn from_route_params(route: RouteId, params: &RouteParams) -> Self {
        let route_raw = route.raw().to_string();
        let mut raw = format!("route:{}:{route_raw}", route_raw.len());
        for (name, value) in params.iter() {
            raw.push_str("|param:");
            raw.push_str(&name.len().to_string());
            raw.push(':');
            raw.push_str(name);
            raw.push('=');
            raw.push_str(&value.len().to_string());
            raw.push(':');
            raw.push_str(value);
        }
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSearchFingerprint(String);

impl RouteSearchFingerprint {
    pub(crate) fn from_location(location: &RouteLocation) -> Self {
        match location.search() {
            Some(search) => Self(format!("search:some:{}:{search}", search.len())),
            None => Self("search:none".to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteSegmentReuseKey(String);

impl RouteSegmentReuseKey {
    pub(crate) fn from_parts(
        route: RouteId,
        slot: &RouteSlotKey,
        explicit_key: Option<&RouteExplicitKey>,
        params: &RouteParamsFingerprint,
    ) -> Self {
        let route_raw = route.raw().to_string();
        let mut raw = "segment".to_owned();
        push_typed_field(&mut raw, "route", &route_raw);
        push_typed_field(&mut raw, "slot", slot.as_str());
        match explicit_key {
            Some(key) => push_typed_field(&mut raw, "key", key.as_str()),
            None => push_typed_field(&mut raw, "key", ""),
        }
        push_typed_field(&mut raw, "params", params.as_str());
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteBranchFingerprint(String);

impl RouteBranchFingerprint {
    pub(crate) fn from_parts<'a>(
        segments: impl IntoIterator<Item = &'a RouteSegmentReuseKey>,
        search: &RouteSearchFingerprint,
    ) -> Self {
        let mut raw = "branch".to_owned();
        for segment in segments {
            push_typed_field(&mut raw, "segment", segment.as_str());
        }
        push_typed_field(&mut raw, "search", search.as_str());
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RouteBranchProvenance(String);

impl RouteBranchProvenance {
    pub(crate) fn next_tree_source() -> Self {
        static NEXT_TREE_SOURCE: AtomicU64 = AtomicU64::new(1);
        let raw = NEXT_TREE_SOURCE.fetch_add(1, Ordering::Relaxed);
        Self(format!("tree:{raw}"))
    }
}
```

Add the required imports to `fingerprint.rs`:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
```

Keep provenance type, debug/string access, and constructors `pub(crate)` so external crates cannot name, inspect, or forge core source identity. Public fingerprint constructors also remain `pub(crate)` so external crates cannot forge core-produced fingerprints.

- [ ] **Step 3: Expose fingerprint modules without prelude exports**

Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod fingerprint;
```

Do not export fingerprint wrappers through `bevy_ui_router_core::prelude` in Phase 1B. They are public read-only module APIs for diagnostics/tests, not default app-facing lifecycle surface.

- [ ] **Step 4: Run tests**

Run: `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture`

Expected: all fingerprint tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/fingerprint.rs
git commit -m "feat(core): add raw branch fingerprints"
```

### Task 4: Matched Branch Planning Metadata

**Files:**
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`
- Modify: `crates/bevy_ui_router_core/src/branch.rs`
- Modify: `crates/bevy_ui_router_core/src/matcher.rs`

**Interfaces:**
- Consumes: Phase 1A `MatchedRoute`, `MatchedBranch`, `RouteMatcher`, route node options, and fingerprint types.
- Produces: matched branch metadata and fingerprints.

- [ ] **Step 1: Add RED matched branch planning tests**

Add this test inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
#[test]
fn route_tree_provenance_is_internal_stable_for_clones_and_distinct_for_builds() {
    let tree = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap()
        .build()
        .unwrap();
    let cloned = tree.clone();
    let other = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap()
        .build()
        .unwrap();

    assert!(tree.provenance() == cloned.provenance());
    assert!(tree.provenance() != other.provenance());

    for output in [format!("{:?}", tree), format!("{:?}", cloned), format!("{:?}", other)] {
        assert!(!output.contains("provenance"));
        assert!(!output.contains("RouteBranchProvenance"));
    }
}
```

Add tests inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/matcher.rs`:

```rust
use crate::branch::MatchedBranch;
use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
use crate::route_tree::RouteNodeOptions;
use crate::slot::RouteSlotKey;

#[test]
fn matched_branch_carries_slot_metadata_boundaries_and_fingerprints() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    let layout_options = RouteNodeOptions::new()
        .with_error_boundary(ErrorBoundaryKey::named("root-boundary").unwrap())
        .with_meta(RouteMeta::new().with_diagnostic_label("layout").unwrap());
    let layout = builder
        .route_with_options(root, RoutePattern::pathless(), layout_options)
        .unwrap();
    let detail_options = RouteNodeOptions::new()
        .with_slot(RouteSlotKey::named("details").unwrap())
        .with_explicit_key(RouteExplicitKey::new("detail-v1").unwrap())
        .with_error_boundary(ErrorBoundaryKey::named("detail-boundary").unwrap())
        .with_meta(RouteMeta::new().with_diagnostic_label("detail-label").unwrap());
    builder
        .route_with_options(layout, RoutePattern::path("items/:id").unwrap(), detail_options)
        .unwrap();

    let branch = RouteMatcher::new(builder.build().unwrap())
        .match_location(&RouteLocation::new("/items/42").with_search("tab=files"))
        .unwrap();
    let detail = branch.leaf().unwrap();

    assert_eq!(detail.slot().as_str(), "details");
    assert_eq!(
        detail.explicit_key().map(RouteExplicitKey::as_str),
        Some("detail-v1")
    );
    assert_eq!(detail.meta().diagnostic_label(), Some("detail-label"));
    assert_eq!(branch.nearest_error_boundary_for(detail.route()).map(ErrorBoundaryKey::as_str), Some("detail-boundary"));
    assert!(detail.params_fingerprint().as_str().contains("param:2:id=2:42"));
    assert!(branch.search_fingerprint().as_str().contains("tab=files"));
    assert!(detail.segment_reuse_key().as_str().contains("detail-v1"));
    assert!(!detail.segment_reuse_key().as_str().contains("detail-label"));
    assert!(!detail.segment_reuse_key().as_str().contains("detail-boundary"));
    assert!(!branch.branch_fingerprint().as_str().contains("detail-label"));
    assert!(!branch.branch_fingerprint().as_str().contains("detail-boundary"));
}

#[test]
fn matched_branch_fingerprints_do_not_merge_ancestor_params() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    let project = builder
        .route(root, RoutePattern::path("projects/:project_id").unwrap())
        .unwrap();
    builder
        .route(project, RoutePattern::path("files/:file_id").unwrap())
        .unwrap();

    let branch = RouteMatcher::new(builder.build().unwrap())
        .match_location(&RouteLocation::new("/projects/alpha/files/readme"))
        .unwrap();
    let project_route = &branch.routes()[1];
    let leaf = branch.leaf().unwrap();

    assert_eq!(project_route.params().get("project_id"), Some("alpha"));
    assert_eq!(leaf.params().get("project_id"), None);
    assert!(project_route
        .params_fingerprint()
        .as_str()
        .contains("param:10:project_id=5:alpha"));
    assert!(leaf
        .params_fingerprint()
        .as_str()
        .contains("param:7:file_id=6:readme"));
    assert!(!leaf.params_fingerprint().as_str().contains("project_id"));
}

fn branch_with_metadata(label: &str, boundary: &str) -> MatchedBranch {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    let options = RouteNodeOptions::new()
        .with_slot(RouteSlotKey::named("details").unwrap())
        .with_explicit_key(RouteExplicitKey::new("detail-v1").unwrap())
        .with_error_boundary(ErrorBoundaryKey::named(boundary).unwrap())
        .with_meta(RouteMeta::new().with_diagnostic_label(label).unwrap());
    builder
        .route_with_options(root, RoutePattern::path("items/:id").unwrap(), options)
        .unwrap();

    RouteMatcher::new(builder.build().unwrap())
        .match_location(&RouteLocation::new("/items/42").with_search("tab=files"))
        .unwrap()
}

#[test]
fn metadata_and_error_boundaries_do_not_change_reuse_or_branch_fingerprints() {
    let first = branch_with_metadata("alpha-label", "alpha-boundary");
    let second = branch_with_metadata("beta-label", "beta-boundary");

    assert!(first.provenance() != second.provenance());
    assert_eq!(
        first.leaf().unwrap().segment_reuse_key(),
        second.leaf().unwrap().segment_reuse_key()
    );
    assert_eq!(first.branch_fingerprint(), second.branch_fingerprint());
}
```

Run: `cargo test -p bevy_ui_router_core matcher::tests::matched_branch_carries_slot_metadata_boundaries_and_fingerprints -- --nocapture`

Run: `cargo test -p bevy_ui_router_core matcher::tests::matched_branch_fingerprints_do_not_merge_ancestor_params -- --nocapture`

Run: `cargo test -p bevy_ui_router_core matcher::tests::metadata_and_error_boundaries_do_not_change_reuse_or_branch_fingerprints -- --nocapture`

Run: `cargo test -p bevy_ui_router_core route_tree::tests::route_tree_provenance_is_internal_stable_for_clones_and_distinct_for_builds -- --nocapture`

Expected: compile failure or test failure because matched branch planning fields are not implemented. If any command exits 0 with `running 0 tests`, the RED step is invalid.

- [ ] **Step 2: Extend matched route and branch data**

Modify `crates/bevy_ui_router_core/src/branch.rs`:

```rust
use crate::fingerprint::{
    RouteBranchFingerprint, RouteBranchProvenance, RouteParamsFingerprint,
    RouteSearchFingerprint, RouteSegmentReuseKey,
};
use crate::meta::{ErrorBoundaryKey, RouteExplicitKey, RouteMeta};
use crate::slot::RouteSlotKey;

pub struct MatchedRoute {
    route: RouteId,
    params: RouteParams,
    slot: RouteSlotKey,
    explicit_key: Option<RouteExplicitKey>,
    error_boundary: Option<ErrorBoundaryKey>,
    meta: RouteMeta,
    params_fingerprint: RouteParamsFingerprint,
    segment_reuse_key: RouteSegmentReuseKey,
}
```

Add accessors listed in Section 4. Keep constructors `pub(crate)` and update tests in `branch.rs` to construct matched routes through a core-internal helper.

Extend `MatchedBranch`:

```rust
#[derive(Clone)]
pub struct MatchedBranch {
    routes: Vec<MatchedRoute>,
    provenance: RouteBranchProvenance,
    search_fingerprint: RouteSearchFingerprint,
    branch_fingerprint: RouteBranchFingerprint,
}
```

Add a `pub(crate)` `provenance` accessor for internal diff comparison, plus public `search_fingerprint`, `branch_fingerprint`, and `nearest_error_boundary_for` accessors.

Do not derive `Debug`, `Eq`, or `PartialEq` for `MatchedBranch` after adding provenance. If `Debug` is needed, hand-write it and omit `provenance`. `BranchDiff::compare` is the public branch comparison contract; `MatchedBranch` equality must not become lifecycle or provenance authority.

- [ ] **Step 3: Add route-tree provenance storage**

Modify `crates/bevy_ui_router_core/src/route_tree.rs`:

- Add `provenance: RouteBranchProvenance` to `RouteTree`.
- Set it in `RouteTreeBuilder::build()` with `RouteBranchProvenance::next_tree_source()`.
- Add `pub(crate) fn provenance(&self) -> &RouteBranchProvenance`.
- Preserve `RouteTree::clone()` semantics so clones keep the same provenance.
- Keep manual `Debug` output free of `provenance`, `RouteBranchProvenance`, `children`, and `declaration_index`.

Do not store provenance in `RouteMatcher` separately. `RouteMatcher` reads it from its owned `RouteTree`.

- [ ] **Step 4: Populate metadata from matcher**

Modify `crates/bevy_ui_router_core/src/matcher.rs` so each `MatchedRoute` copies planning data from the matched `RouteNodeOptions`.

When building a `MatchedBranch`:

1. Copy one `RouteBranchProvenance` from `self.tree.provenance().clone()`. This is an opaque source identity, not a structural fingerprint of the route tree.
2. Compute `RouteSearchFingerprint::from_location(location)` once.
3. Compute each `RouteParamsFingerprint::from_route_params(route_id, params)`.
4. Compute each `RouteSegmentReuseKey` from route id, slot key, explicit key, and params fingerprint only.
5. Compute `RouteBranchFingerprint` from route segment reuse keys in matched order plus raw search fingerprint.
6. Store error-boundary key and metadata diagnostic label as matched branch metadata only; do not place them in reuse keys or branch fingerprint.

Use typed length-prefixed fields for every dynamic component. Separator-only strings are not valid because route params, explicit keys, and search strings may contain separator-like characters.

- [ ] **Step 5: Run focused tests**

Run: `cargo test -p bevy_ui_router_core matcher::tests -- --nocapture`

Expected: matcher tests pass.

Run: `cargo test -p bevy_ui_router_core branch::tests -- --nocapture`

Expected: branch tests pass.

Run: `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture`

Expected: fingerprint tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/src/branch.rs crates/bevy_ui_router_core/src/matcher.rs
git commit -m "feat(core): enrich matched branch planning data"
```

### Task 5: Branch Diff Contract

**Files:**
- Create: `crates/bevy_ui_router_core/src/branch_diff.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `MatchedBranch`, `MatchedRoute`, `RouteSlotKey`, route ids, segment fingerprints.
- Produces: `BranchDiff`, `BranchDiffRoute`.

- [ ] **Step 1: Wire module and add RED branch diff tests**

Add `pub mod branch_diff;` to `crates/bevy_ui_router_core/src/lib.rs`.

Create `crates/bevy_ui_router_core/src/branch_diff.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::RouteLocation;
    use crate::matcher::RouteMatcher;
    use crate::meta::RouteExplicitKey;
    use crate::pattern::RoutePattern;
    use crate::route_tree::{RouteNodeOptions, RouteTree};
    use crate::slot::RouteSlotKey;

    fn sample_items_matcher() -> RouteMatcher {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let root = builder.root_id().unwrap();
        let layout = builder.route(root, RoutePattern::path("items").unwrap()).unwrap();
        builder.route(layout, RoutePattern::path(":id").unwrap()).unwrap();
        RouteMatcher::new(builder.build().unwrap())
    }

    #[test]
    fn diff_preserves_common_prefix_and_orders_suffixes() {
        let matcher = sample_items_matcher();
        let old = matcher.match_location(&RouteLocation::new("/items/1")).unwrap();
        let new = matcher.match_location(&RouteLocation::new("/items/2")).unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 2);
        assert_eq!(diff.leaving().len(), 1);
        assert_eq!(diff.entering().len(), 1);
        assert!(diff.branch_fingerprint_changed());
    }

    #[test]
    fn sibling_route_change_preserves_only_shared_parent() {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let root = builder.root_id().unwrap();
        builder.route(root, RoutePattern::path("items").unwrap()).unwrap();
        builder.route(root, RoutePattern::path("settings").unwrap()).unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let old = matcher.match_location(&RouteLocation::new("/items")).unwrap();
        let new = matcher.match_location(&RouteLocation::new("/settings")).unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 1);
        assert_eq!(diff.leaving().len(), 1);
        assert_eq!(diff.entering().len(), 1);
    }

    #[test]
    fn child_route_change_enters_new_suffix_from_root_to_leaf() {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let root = builder.root_id().unwrap();
        let items = builder.route(root, RoutePattern::path("items").unwrap()).unwrap();
        builder.route(items, RoutePattern::path(":id").unwrap()).unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let old = matcher.match_location(&RouteLocation::new("/items")).unwrap();
        let new = matcher.match_location(&RouteLocation::new("/items/42")).unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 2);
        assert!(diff.leaving().is_empty());
        assert_eq!(diff.entering().len(), 1);
        assert_eq!(diff.entering()[0].index(), 2);
    }

    #[test]
    fn layout_shape_change_uses_provenance_fail_closed() {
        let mut old_builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let old_root = old_builder.root_id().unwrap();
        old_builder.route(old_root, RoutePattern::path("items").unwrap()).unwrap();
        let old = RouteMatcher::new(old_builder.build().unwrap())
            .match_location(&RouteLocation::new("/items"))
            .unwrap();

        let mut new_builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let new_root = new_builder.root_id().unwrap();
        let layout = new_builder.route(new_root, RoutePattern::pathless()).unwrap();
        new_builder.route(layout, RoutePattern::path("items").unwrap()).unwrap();
        let new = RouteMatcher::new(new_builder.build().unwrap())
            .match_location(&RouteLocation::new("/items"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);
        assert!(diff.provenance_changed());
        assert_eq!(diff.common_prefix_len(), 0);
    }

    #[test]
    fn different_tree_provenance_replaces_entire_branch_even_when_route_ids_match() {
        let mut old_builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let old_root = old_builder.root_id().unwrap();
        old_builder.route(old_root, RoutePattern::path("panel").unwrap()).unwrap();
        let old = RouteMatcher::new(old_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let mut new_builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let new_root = new_builder.root_id().unwrap();
        let options = RouteNodeOptions::new().with_slot(RouteSlotKey::named("side").unwrap());
        new_builder
            .route_with_options(new_root, RoutePattern::path("panel").unwrap(), options)
            .unwrap();
        let new = RouteMatcher::new(new_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);
        assert!(diff.provenance_changed());
        assert_eq!(diff.common_prefix_len(), 0);
        assert_eq!(diff.leaving().len(), old.routes().len());
        assert_eq!(diff.entering().len(), new.routes().len());
    }

    #[test]
    fn explicit_key_change_in_different_tree_also_fails_closed() {
        let mut old_builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let old_root = old_builder.root_id().unwrap();
        let old_options = RouteNodeOptions::new()
            .with_explicit_key(RouteExplicitKey::new("v1").unwrap());
        old_builder
            .route_with_options(old_root, RoutePattern::path("panel").unwrap(), old_options)
            .unwrap();
        let old = RouteMatcher::new(old_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let mut new_builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let new_root = new_builder.root_id().unwrap();
        let new_options = RouteNodeOptions::new()
            .with_explicit_key(RouteExplicitKey::new("v2").unwrap());
        new_builder
            .route_with_options(new_root, RoutePattern::path("panel").unwrap(), new_options)
            .unwrap();
        let new = RouteMatcher::new(new_builder.build().unwrap())
            .match_location(&RouteLocation::new("/panel"))
            .unwrap();

        let diff = BranchDiff::compare(&old, &new);
        assert!(diff.provenance_changed());
        assert_eq!(diff.common_prefix_len(), 0);
    }

    #[test]
    fn identical_independent_trees_fail_closed_by_provenance() {
        let old_matcher = sample_items_matcher();
        let new_matcher = sample_items_matcher();
        let old = old_matcher.match_location(&RouteLocation::new("/items/42")).unwrap();
        let new = new_matcher.match_location(&RouteLocation::new("/items/42")).unwrap();

        let diff = BranchDiff::compare(&old, &new);

        assert!(diff.provenance_changed());
        assert!(old.provenance() != new.provenance());
        assert_eq!(old.branch_fingerprint(), new.branch_fingerprint());
        assert_eq!(
            old.leaf().unwrap().segment_reuse_key(),
            new.leaf().unwrap().segment_reuse_key()
        );
        assert_eq!(diff.common_prefix_len(), 0);
        assert_eq!(diff.leaving().len(), old.routes().len());
        assert_eq!(diff.entering().len(), new.routes().len());
    }

    #[test]
    fn multi_level_suffix_orders_leaving_leaf_to_root_and_entering_root_to_leaf() {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
        let root = builder.root_id().unwrap();
        let items = builder.route(root, RoutePattern::path("items").unwrap()).unwrap();
        let id = builder.route(items, RoutePattern::path(":id").unwrap()).unwrap();
        let details = builder.route(id, RoutePattern::path("details").unwrap()).unwrap();
        builder.route(details, RoutePattern::path(":tab").unwrap()).unwrap();
        let summary = builder.route(id, RoutePattern::path("summary").unwrap()).unwrap();
        builder.route(summary, RoutePattern::path(":tab").unwrap()).unwrap();
        let matcher = RouteMatcher::new(builder.build().unwrap());

        let old = matcher
            .match_location(&RouteLocation::new("/items/1/details/files"))
            .unwrap();
        let new = matcher
            .match_location(&RouteLocation::new("/items/2/summary/activity"))
            .unwrap();
        let diff = BranchDiff::compare(&old, &new);

        assert_eq!(diff.common_prefix_len(), 2);
        let leaving_indices: Vec<_> = diff.leaving().iter().map(BranchDiffRoute::index).collect();
        let entering_indices: Vec<_> = diff.entering().iter().map(BranchDiffRoute::index).collect();
        assert_eq!(leaving_indices, vec![4, 3, 2]);
        assert_eq!(entering_indices, vec![2, 3, 4]);
    }
}
```

Run: `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture`

Expected: compile failure or test failure because `BranchDiff` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid.

- [ ] **Step 2: Implement diff route records**

Implement `crates/bevy_ui_router_core/src/branch_diff.rs`:

```rust
use crate::branch::{MatchedBranch, MatchedRoute};
use crate::ids::RouteId;
use crate::slot::RouteSlotKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchDiffRoute {
    route: RouteId,
    index: usize,
    slot: RouteSlotKey,
}

impl BranchDiffRoute {
    fn from_matched(index: usize, route: &MatchedRoute) -> Self {
        Self {
            route: route.route(),
            index,
            slot: route.slot().clone(),
        }
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn slot(&self) -> &RouteSlotKey {
        &self.slot
    }
}
```

- [ ] **Step 3: Implement common-prefix diff**

Implement:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchDiff {
    common_prefix_len: usize,
    leaving: Vec<BranchDiffRoute>,
    entering: Vec<BranchDiffRoute>,
    provenance_changed: bool,
    search_changed: bool,
    branch_fingerprint_changed: bool,
}

impl BranchDiff {
    pub fn compare(old: &MatchedBranch, new: &MatchedBranch) -> Self {
        let provenance_changed = old.provenance() != new.provenance();
        let search_changed = old.search_fingerprint() != new.search_fingerprint();
        let common_prefix_len = if provenance_changed || search_changed {
            0
        } else {
            old.routes()
                .iter()
                .zip(new.routes().iter())
                .take_while(|(old_route, new_route)| reusable_segment(old_route, new_route))
                .count()
        };

        let leaving = old.routes()[common_prefix_len..]
            .iter()
            .enumerate()
            .rev()
            .map(|(offset, route)| BranchDiffRoute::from_matched(common_prefix_len + offset, route))
            .collect();
        let entering = new.routes()[common_prefix_len..]
            .iter()
            .enumerate()
            .map(|(offset, route)| BranchDiffRoute::from_matched(common_prefix_len + offset, route))
            .collect();

        Self {
            common_prefix_len,
            leaving,
            entering,
            provenance_changed,
            search_changed,
            branch_fingerprint_changed: old.branch_fingerprint() != new.branch_fingerprint(),
        }
    }

    pub const fn common_prefix_len(&self) -> usize {
        self.common_prefix_len
    }

    pub fn leaving(&self) -> &[BranchDiffRoute] {
        &self.leaving
    }

    pub fn entering(&self) -> &[BranchDiffRoute] {
        &self.entering
    }

    pub const fn provenance_changed(&self) -> bool {
        self.provenance_changed
    }

    pub const fn search_changed(&self) -> bool {
        self.search_changed
    }

    pub const fn branch_fingerprint_changed(&self) -> bool {
        self.branch_fingerprint_changed
    }
}

fn reusable_segment(old: &MatchedRoute, new: &MatchedRoute) -> bool {
    old.route() == new.route()
        && old.slot() == new.slot()
        && old.explicit_key() == new.explicit_key()
        && old.segment_reuse_key() == new.segment_reuse_key()
}
```

- [ ] **Step 4: Export branch diff module and default prelude type**

Modify `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod branch_diff;

pub mod prelude {
    pub use crate::branch_diff::BranchDiff;
}
```

Keep `BranchDiffRoute` available through `bevy_ui_router_core::branch_diff::BranchDiffRoute`, not through the prelude.

- [ ] **Step 5: Run branch diff tests**

Run: `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture`

Expected: all branch diff tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/branch_diff.rs
git commit -m "feat(core): add branch diff contract"
```

### Task 6: Search And Error-Boundary Fixtures

**Files:**
- Modify: `crates/bevy_ui_router_core/src/branch_diff.rs`
- Modify: `crates/bevy_ui_router_core/src/matcher.rs`

**Interfaces:**
- Consumes: branch diff, fingerprints, error-boundary metadata.
- Produces: coverage for search change, nearest boundary lookup, and data-only guarantees.

- [ ] **Step 1: Add search-change fixture**

Add this test inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/branch_diff.rs`:

```rust
#[test]
fn search_change_fails_closed_by_raw_search_policy() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    builder.route(root, RoutePattern::path("items").unwrap()).unwrap();
    let tree = builder.build().unwrap();
    let matcher = RouteMatcher::new(tree);

    let first = matcher
        .match_location(&RouteLocation::new("/items").with_search("a=1&a=2"))
        .unwrap();
    let second = matcher
        .match_location(&RouteLocation::new("/items").with_search("a=2&a=1"))
        .unwrap();
    let diff = BranchDiff::compare(&first, &second);

    assert!(diff.search_changed());
    assert!(diff.branch_fingerprint_changed());
    assert_eq!(diff.common_prefix_len(), 0);
    assert_eq!(diff.leaving().len(), first.routes().len());
    assert_eq!(diff.entering().len(), second.routes().len());
}
```

Run: `cargo test -p bevy_ui_router_core branch_diff::tests::search_change_fails_closed_by_raw_search_policy -- --nocapture`

Expected: test passes if Task 5 already implemented search tracking and search-change fail-closed behavior. If it fails, fix fingerprint propagation or branch diff policy. If the command exits 0 with `running 0 tests`, the step is invalid.

- [ ] **Step 2: Add fragment-only branch fixture**

Add this test inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/branch_diff.rs`:

```rust
#[test]
fn fragment_only_change_does_not_change_search_or_branch_diff() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    builder.route(root, RoutePattern::path("items").unwrap()).unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());

    let first = matcher
        .match_location(&RouteLocation::new("/items").with_search("a=1"))
        .unwrap();
    let second = matcher
        .match_location(
            &RouteLocation::new("/items")
                .with_search("a=1")
                .with_fragment("ignored"),
        )
        .unwrap();
    let diff = BranchDiff::compare(&first, &second);

    assert_eq!(first.search_fingerprint(), second.search_fingerprint());
    assert_eq!(first.branch_fingerprint(), second.branch_fingerprint());
    assert!(!diff.search_changed());
    assert!(!diff.branch_fingerprint_changed());
    assert_eq!(diff.common_prefix_len(), first.routes().len());
    assert!(diff.leaving().is_empty());
    assert!(diff.entering().is_empty());
}
```

Run: `cargo test -p bevy_ui_router_core branch_diff::tests::fragment_only_change_does_not_change_search_or_branch_diff -- --nocapture`

Expected: test passes if fragment exclusion is propagated through matched-branch fingerprints and branch diff. If it fails, fix fingerprint propagation. If the command exits 0 with `running 0 tests`, the step is invalid.

- [ ] **Step 3: Add nearest error-boundary fixture**

Add this test inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/matcher.rs`:

```rust
use crate::ids::RouteId;
use crate::meta::ErrorBoundaryKey;
use crate::route_tree::RouteNodeOptions;

#[test]
fn nearest_error_boundary_uses_matched_ancestor_chain_only() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/").unwrap()).unwrap();
    let root = builder.root_id().unwrap();
    let root_layout = builder
        .route_with_options(
            root,
            RoutePattern::pathless(),
            RouteNodeOptions::new().with_error_boundary(ErrorBoundaryKey::named("root").unwrap()),
        )
        .unwrap();
    let section = builder
        .route_with_options(
            root_layout,
            RoutePattern::path("settings").unwrap(),
            RouteNodeOptions::new().with_error_boundary(ErrorBoundaryKey::named("settings").unwrap()),
        )
        .unwrap();
    let leaf = builder.route(section, RoutePattern::path("profile").unwrap()).unwrap();
    let billing = builder
        .route_with_options(
            root_layout,
            RoutePattern::path("billing").unwrap(),
            RouteNodeOptions::new().with_error_boundary(ErrorBoundaryKey::named("billing").unwrap()),
        )
        .unwrap();

    let branch = RouteMatcher::new(builder.build().unwrap())
        .match_location(&RouteLocation::new("/settings/profile"))
        .unwrap();

    assert_eq!(
        branch.nearest_error_boundary_for(leaf).map(ErrorBoundaryKey::as_str),
        Some("settings")
    );
    assert_eq!(
        branch.nearest_error_boundary_for(root_layout).map(ErrorBoundaryKey::as_str),
        Some("root")
    );
    assert_eq!(branch.nearest_error_boundary_for(billing), None);
    assert_eq!(branch.nearest_error_boundary_for(RouteId::new(999)), None);
}
```

Run: `cargo test -p bevy_ui_router_core matcher::tests::nearest_error_boundary_uses_matched_ancestor_chain_only -- --nocapture`

Expected: test passes after nearest-boundary metadata is implemented.

- [ ] **Step 4: Add compile-time data-only guard**

Add this test to `crates/bevy_ui_router_core/tests/dependency_boundary.rs`:

```rust
#[test]
fn branch_planning_manifest_stays_dependency_free() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "Phase 1B branch planning must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}
```

Run: `cargo test -p bevy_ui_router_core --test dependency_boundary -- --nocapture`

Expected: dependency boundary tests pass.

- [ ] **Step 5: Run focused coverage**

Run: `cargo test -p bevy_ui_router_core branch_diff::tests -- --nocapture`

Expected: branch diff tests pass.

Run: `cargo test -p bevy_ui_router_core matcher::tests -- --nocapture`

Expected: matcher tests pass.

Run: `cargo test -p bevy_ui_router_core fingerprint::tests -- --nocapture`

Expected: fingerprint tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_ui_router_core/src/branch_diff.rs crates/bevy_ui_router_core/src/matcher.rs crates/bevy_ui_router_core/tests/dependency_boundary.rs
git commit -m "test(core): cover branch planning boundaries"
```

### Task 7: Diagnostics And Public Boundary

**Files:**
- Modify: `crates/bevy_ui_router_core/src/diagnostics.rs`
- Modify: `crates/bevy_ui_router_core/src/fingerprint.rs`
- Modify: `crates/bevy_ui_router_core/src/branch.rs`
- Modify: `crates/bevy_ui_router_core/src/branch_diff.rs`
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`
- Create: `crates/bevy_ui_router_core/tests/public_api.rs`
- Modify: `crates/bevy_ui_router/tests/facade_features.rs`

**Interfaces:**
- Consumes: `BranchDiff`, fingerprints, route ids, slot keys.
- Produces: diagnostics and facade coverage without exposing forgeable active identity.

- [ ] **Step 1: Add RED diagnostics tests**

Add this test inside the existing `#[cfg(test)] mod tests` block in `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
#[test]
fn branch_planning_event_has_no_active_identity() {
    let payload = BranchPlanningDiagnostic::new(
        2,
        1,
        1,
        true,
        false,
        true,
    );
    let event = DiagnosticEvent::branch_planned(payload.clone());

    assert_eq!(event.kind(), DiagnosticEventKind::BranchPlanned);
    assert!(event.identity().is_none());
    assert_eq!(event.branch_planning(), Some(&payload));
    assert_eq!(event.branch_planning().unwrap().common_prefix_len(), 2);
    assert_eq!(event.branch_planning().unwrap().leaving_len(), 1);
    assert_eq!(event.branch_planning().unwrap().entering_len(), 1);
    assert!(event.branch_planning().unwrap().provenance_changed());
    assert!(!event.branch_planning().unwrap().search_changed());
    assert!(event.branch_planning().unwrap().branch_fingerprint_changed());
    assert_eq!(event.redacted_message(), "branch planned");
    assert!(!event.redacted_message().contains("frame"));
    assert!(!event.redacted_message().contains("generation"));
}
```

Run: `cargo test -p bevy_ui_router_core diagnostics::tests::branch_planning_event_has_no_active_identity -- --nocapture`

Expected: compile failure or test failure because `BranchPlanned` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid.

- [ ] **Step 2: Implement diagnostic event**

Modify `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
// Add this variant to the existing DiagnosticEventKind enum.
// Preserve every existing variant and derive.
BranchPlanned,

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPlanningDiagnostic {
    common_prefix_len: usize,
    leaving_len: usize,
    entering_len: usize,
    provenance_changed: bool,
    search_changed: bool,
    branch_fingerprint_changed: bool,
}

impl BranchPlanningDiagnostic {
    pub const fn new(
        common_prefix_len: usize,
        leaving_len: usize,
        entering_len: usize,
        provenance_changed: bool,
        search_changed: bool,
        branch_fingerprint_changed: bool,
    ) -> Self {
        Self {
            common_prefix_len,
            leaving_len,
            entering_len,
            provenance_changed,
            search_changed,
            branch_fingerprint_changed,
        }
    }

    pub const fn common_prefix_len(&self) -> usize {
        self.common_prefix_len
    }

    pub const fn leaving_len(&self) -> usize {
        self.leaving_len
    }

    pub const fn entering_len(&self) -> usize {
        self.entering_len
    }

    pub const fn provenance_changed(&self) -> bool {
        self.provenance_changed
    }

    pub const fn search_changed(&self) -> bool {
        self.search_changed
    }

    pub const fn branch_fingerprint_changed(&self) -> bool {
        self.branch_fingerprint_changed
    }
}

// Add this field to the existing DiagnosticEvent struct.
// Preserve every existing field, derive, and accessor.
branch_planning: Option<BranchPlanningDiagnostic>,

impl DiagnosticEvent {
    pub(crate) fn branch_planned(branch_planning: BranchPlanningDiagnostic) -> Self {
        Self {
            kind: DiagnosticEventKind::BranchPlanned,
            identity: None,
            branch_planning: Some(branch_planning),
            redacted_message: "branch planned".to_owned(),
        }
    }

    pub fn branch_planning(&self) -> Option<&BranchPlanningDiagnostic> {
        self.branch_planning.as_ref()
    }
}
```

Existing diagnostic constructors must set `branch_planning: None`. Phase 1B must not add a free-form public constructor that lets callers smuggle frame, generation, host, cleanup, resolve/apply, or adapter semantics into a `BranchPlanned` diagnostic.

- [ ] **Step 3: Add external module-path positive boundary test**

Create `crates/bevy_ui_router_core/tests/public_api.rs`:

```rust
#[test]
fn non_prelude_branch_planning_support_types_are_public_module_api() {
    use bevy_ui_router_core::branch_diff::BranchDiffRoute;
    use bevy_ui_router_core::diagnostics::BranchPlanningDiagnostic;
    use bevy_ui_router_core::fingerprint::{
        RouteBranchFingerprint, RouteParamsFingerprint, RouteSearchFingerprint,
        RouteSegmentReuseKey,
    };

    let diagnostic = BranchPlanningDiagnostic::new(0, 0, 0, false, false, false);
    assert_eq!(diagnostic.common_prefix_len(), 0);

    fn assert_public<T>() {}
    assert_public::<BranchDiffRoute>();
    assert_public::<RouteBranchFingerprint>();
    assert_public::<RouteParamsFingerprint>();
    assert_public::<RouteSearchFingerprint>();
    assert_public::<RouteSegmentReuseKey>();
}
```

Run: `cargo test -p bevy_ui_router_core --test public_api non_prelude_branch_planning_support_types_are_public_module_api -- --nocapture`

Expected: the test passes and reports one nonzero integration test. If it fails because a type is not public through its module path, fix the module export rather than adding the type to prelude.

- [ ] **Step 4: Add public boundary doctests**

Do not modify `crates/bevy_ui_router_core/src/lib.rs` for `BranchPlanningDiagnostic` unless implementation-time inspection proves `pub mod diagnostics;` is absent. Current Phase 1A exposes the diagnostics module, so `BranchPlanningDiagnostic` is named through `bevy_ui_router_core::diagnostics::BranchPlanningDiagnostic`, not through the prelude. Do not add or restate `DiagnosticIdentity`, `DiagnosticSnapshot`, `DiagnosticEvent`, or `DiagnosticEventKind` as Phase 1B export work. Preserve their existing exports and derives from earlier phases.

Add these doctests to `crates/bevy_ui_router_core/src/fingerprint.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::fingerprint::{
//!     RouteBranchFingerprint, RouteSearchFingerprint, RouteSegmentReuseKey,
//! };
//!
//! fn forge(segment: &RouteSegmentReuseKey, search: &RouteSearchFingerprint) {
//!     let _ = RouteBranchFingerprint::from_parts([segment], search);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::fingerprint::{RouteParamsFingerprint, RouteSegmentReuseKey};
//! use bevy_ui_router_core::prelude::{RouteExplicitKey, RouteId, RouteSlotKey};
//!
//! fn forge(
//!     route: RouteId,
//!     slot: &RouteSlotKey,
//!     key: &RouteExplicitKey,
//!     params: &RouteParamsFingerprint,
//! ) {
//!     let _ = RouteSegmentReuseKey::from_parts(route, slot, Some(key), params);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteBranchProvenance;
//!
//! let _ = core::mem::size_of::<RouteBranchProvenance>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::fingerprint::RouteBranchProvenance;
//!
//! let _ = core::mem::size_of::<RouteBranchProvenance>();
//! ```
//! 
//! ```compile_fail
//! use bevy_ui_router_core::fingerprint::RouteSearchFingerprint;
//! use bevy_ui_router_core::prelude::RouteLocation;
//!
//! let location = RouteLocation::new("/items");
//! let _ = RouteSearchFingerprint::from_location(&location);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::fingerprint::RouteParamsFingerprint;
//! use bevy_ui_router_core::prelude::{RouteId, RouteParams};
//!
//! fn forge(route: RouteId, params: &RouteParams) {
//!     let _ = RouteParamsFingerprint::from_route_params(route, params);
//! }
//! ```
```

Add this doctest to `crates/bevy_ui_router_core/src/branch_diff.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::BranchDiffRoute;
//!
//! let _ = core::mem::size_of::<BranchDiffRoute>();
//! ```
```

Add this doctest to `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::BranchPlanningDiagnostic;
//!
//! let _ = BranchPlanningDiagnostic::new(0, 0, 0, false, false, false);
//! ```
```

Add these doctests to `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::route_tree::RouteNode;
//!
//! fn requires_partial_eq<T: PartialEq>() {}
//!
//! requires_partial_eq::<RouteNode>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::route_tree::RouteTree;
//!
//! fn requires_partial_eq<T: PartialEq>() {}
//!
//! requires_partial_eq::<RouteTree>();
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::route_tree::RouteTreeBuilder;
//!
//! fn requires_partial_eq<T: PartialEq>() {}
//!
//! requires_partial_eq::<RouteTreeBuilder>();
//! ```
```

Add this doctest to `crates/bevy_ui_router_core/src/branch.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{MatchedBranch, MatchedRoute};
//!
//! let _ = MatchedBranch::new(Vec::<MatchedRoute>::new());
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::fingerprint::{RouteParamsFingerprint, RouteSegmentReuseKey};
//! use bevy_ui_router_core::prelude::{
//!     ErrorBoundaryKey, MatchedRoute, RouteExplicitKey, RouteId, RouteMeta,
//!     RouteParams, RouteSlotKey,
//! };
//!
//! fn forge(
//!     route: RouteId,
//!     params: RouteParams,
//!     slot: RouteSlotKey,
//!     explicit_key: Option<RouteExplicitKey>,
//!     error_boundary: Option<ErrorBoundaryKey>,
//!     meta: RouteMeta,
//!     params_fingerprint: RouteParamsFingerprint,
//!     segment_reuse_key: RouteSegmentReuseKey,
//! ) {
//!     let _ = MatchedRoute::new(
//!         route,
//!         params,
//!         slot,
//!         explicit_key,
//!         error_boundary,
//!         meta,
//!         params_fingerprint,
//!         segment_reuse_key,
//!     );
//! }
//! ```
```

If the internal `MatchedRoute::new` signature changes during implementation, update this compile-fail snippet to call the actual internal constructor signature with function parameters for all otherwise-unconstructible values. Do not leave stale arity snippets as boundary evidence.

Run: `cargo test -p bevy_ui_router_core --doc`

Expected: doctests pass, with nonzero doctest evidence and the compile-fail constructor boundary checks exercised.

- [ ] **Step 5: Add facade boundary test**

Add to `crates/bevy_ui_router/tests/facade_features.rs`:

```rust
#[test]
fn facade_prelude_exposes_branch_planning_contracts() {
    use bevy_ui_router::prelude::{
        BranchDiff, ErrorBoundaryKey, MatchedBranch, MatchedRoute, RouteExplicitKey,
        RouteId, RouteLocation, RouteMeta, RouteNodeOptions, RouteParams, RouteSlotKey,
    };

    let _slot = RouteSlotKey::default();
    let _key = RouteExplicitKey::new("stable").unwrap();
    let _boundary = ErrorBoundaryKey::named("root").unwrap();
    let _location = RouteLocation::new("/items");
    let _meta = RouteMeta::new();
    let _options = RouteNodeOptions::new();

    fn assert_types<T>() {}
    assert_types::<BranchDiff>();
    assert_types::<MatchedBranch>();
    assert_types::<MatchedRoute>();
    assert_types::<RouteId>();
    assert_types::<RouteLocation>();
    assert_types::<RouteParams>();
}
```

Run: `cargo test -p bevy_ui_router --no-default-features facade_prelude_exposes_branch_planning_contracts -- --nocapture`

Expected: the test may pass immediately if facade re-export is already correct. If it fails because stable core contracts are missing, fix `crates/bevy_ui_router_core/src/lib.rs` or the facade prelude.

- [ ] **Step 6: Run diagnostics and public boundary checks**

Run: `cargo test -p bevy_ui_router_core diagnostics::tests -- --nocapture`

Expected: diagnostics tests pass.

Run: `cargo test -p bevy_ui_router_core --test public_api -- --nocapture`

Expected: public API module-path tests pass.

Run: `cargo test -p bevy_ui_router_core --doc`

Expected: doctests pass and compile-fail boundary checks are exercised.

Run: `cargo test -p bevy_ui_router --no-default-features`

Expected: facade tests pass without default features.

- [ ] **Step 7: Commit**

```bash
git add crates/bevy_ui_router_core/src/diagnostics.rs crates/bevy_ui_router_core/src/fingerprint.rs crates/bevy_ui_router_core/src/branch.rs crates/bevy_ui_router_core/src/branch_diff.rs crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/tests/public_api.rs crates/bevy_ui_router/tests/facade_features.rs
git commit -m "test: expose branch planning contracts"
```

### Task 8: Final Phase 1B Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Interfaces:**
- Consumes: all Phase 1B changes.
- Produces: final verification evidence and Phase 2 handoff.

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all --check`

Expected: exit code 0.

- [ ] **Step 2: Run core tests**

Run: `cargo test -p bevy_ui_router_core`

Expected: exit code 0.

- [ ] **Step 3: Run core doctests**

Run: `cargo test -p bevy_ui_router_core --doc`

Expected: exit code 0.

- [ ] **Step 4: Run public module-path tests**

Run: `cargo test -p bevy_ui_router_core --test public_api`

Expected: exit code 0.

- [ ] **Step 5: Run dependency boundary tests**

Run: `cargo test -p bevy_ui_router_core --test dependency_boundary`

Expected: exit code 0.

- [ ] **Step 6: Run facade checks**

Run: `cargo test -p bevy_ui_router --no-default-features`

Expected: exit code 0.

Run: `cargo check -p bevy_ui_router --no-default-features --features platform`

Expected: exit code 0.

Run: `cargo check -p bevy_ui_router --no-default-features --features bsn`

Expected: exit code 0.

- [ ] **Step 7: Confirm Phase 2 handoff conditions**

Run:

```powershell
rg -n "BranchDiff|RouteBranchFingerprint|RouteSearchFingerprint|RouteSlotKey|ErrorBoundaryKey" crates\bevy_ui_router_core\src
git status --short
```

Expected: branch planning contracts exist and worktree has only intentional Phase 1B files modified or staged.

- [ ] **Step 8: Commit verification notes**

```bash
git add docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md
git commit -m "docs: record phase 1b branch planning verification"
```

---

## 8. Review Gate Before Implementation

Before implementing Phase 1B, review this plan with independent agents using these responsibility directions:

- Architecture and module-boundary review: verify Phase 1B remains core data-only and does not leak frame lifecycle, ECS, adapter, BSN, platform, or app assumptions.
- API and lifecycle review: verify branch diff and fingerprints cannot become active UI authority, cannot forge frame/generation identity, and are sufficient handoff data for Phase 2.
- Fact and source review: verify claims against current repository files, Phase 1A plan/review log, and existing roadmap docs.
- Test and risk review: verify every semantic requirement has executable coverage and every command is meaningful for touched areas.

The lead agent must evaluate findings before accepting them. If multiple findings share a root cause such as over-broad scope, unstable fingerprint semantics, or lifecycle leakage, revise the plan architecture instead of applying isolated edits.

---

## 9. Acceptance Criteria

- The final reviewed plan clearly states that Phase 1B depends on completed Phase 1A implementation and must re-check that gate at execution time.
- The final reviewed plan clearly states that the current repository has Phase 1A implementation and verification evidence, and that Phase 1B execution is blocked only if the execution-time gate fails or the refreshed plan has not passed review.
- The plan covers branch diff, raw params/search fingerprint, named slot placement, route metadata, and error-boundary declaration candidates.
- The plan has no frame id allocation, generation allocation, active branch commit, transaction runtime, async resolve/apply, ECS mutation, retained subtree, focus, or BSN adapter tasks.
- `MatchedBranch`, `BranchDiff`, and fingerprints remain data-only planning inputs.
- Segment reuse keys do not include diagnostic labels, route metadata, or error-boundary declarations.
- Segment reuse keys and branch fingerprints use typed length-prefixed structural encoding and do not rely on separator-only concatenation.
- Branch diff fails closed when branch provenance differs.
- Branch diff fails closed when raw search fingerprint differs under the Phase 1B default policy.
- Branch provenance is a core-internal source identity, not a structural fingerprint; structurally identical independent route trees still fail closed.
- Raw search fingerprint policy is frozen and tested for absence vs empty, leading question mark preservation, ordering, repeated keys, empty/missing values, percent encoding and percent-case preservation, and fragment exclusion.
- App payload exclusion is documented as an API boundary because current `RouteLocation` has no payload/extension field; future payload/extension fields require explicit fingerprint tests before they can affect navigation identity.
- Branch diff tests cover sibling, child/depth, params change, layout/provenance change, structurally identical cross-provenance change, slot/provenance change, explicit-key/provenance change, search change, fragment-only neutrality, and multi-element suffix ordering.
- Matched-branch tests directly prove leaf fingerprints use only local params, metadata/boundary changes do not alter reuse keys or branch fingerprints, and independent identical trees can have equal fingerprints while still failing closed by provenance.
- Error-boundary tests prove nearest boundary metadata lookup for the matched ancestor chain and include off-branch / unknown-route negative coverage; rendering fallback UI and history mutation remain out of scope and are not introduced by Phase 1B APIs.
- Public fingerprint, matched-route, and matched-branch constructors remain core-internal and have compile-fail boundary coverage; provenance is not exposed through any public API.
- `RouteNode`, `RouteTree`, and `RouteTreeBuilder` do not gain public `PartialEq` comparability as a side effect of adding provenance or route planning options, and this has compile-fail boundary coverage.
- Default app-facing contract types have positive prelude/facade import coverage, while non-prelude branch planning support types have positive integration-test module-path coverage, so compile-fail constructor tests cannot pass merely because required public module types are missing.
- `BranchDiffRoute`, branch fingerprint wrappers, and `BranchPlanningDiagnostic` remain out of the default prelude unless a future reviewed API decision expands that surface.
- Branch-planning diagnostics use a structured payload and fixed redacted message, with no active frame identity and no free-form path for frame/generation/host/cleanup semantics.
- Phase 1B adds only local route declaration validation errors if missing; it preserves existing Phase 0 / Phase 1A error variants and does not add or reinterpret resolve/apply, stale-result, adapter, transaction, finalization, or cleanup errors.
- The plan distinguishes RED evidence from coverage and boundary-verification steps.
- The plan lists concrete verification commands and requires command output before success claims.

---

## 10. Phase 1B Verification Record

Recorded on 2026-07-08 after Task 1 through Task 7 implementation and task-scoped review.

Final implementation commits:

- `780cd67 feat(core): add route declaration planning keys`
- `6eee798 feat(core): add route node planning options`
- `9098786 feat(core): add raw branch fingerprints`
- `8d21453 feat(core): enrich matched branch planning data`
- `2a95c70 feat(core): add branch diff contract`
- `c066251 test(core): cover branch search diff policy`
- `35a1069 test(core): cover branch planning boundaries`
- `1c4a686 test(core): add branch planning dependency boundary guard`
- `23ab674 test: expose branch planning contracts`

Final verification commands were run with `--config "build.rustc-wrapper = ''"` for Cargo commands to avoid local `sccache` sandbox access failures:

- `cargo fmt --all --check`
  - Result: passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core`
  - Result: passed.
  - Evidence: 62 core unit tests, 4 `dependency_boundary` integration tests, 1 `public_api` integration test, and 20 core doctests passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --doc`
  - Result: passed.
  - Evidence: 20 doctests passed, including compile-fail public-boundary checks.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api`
  - Result: passed.
  - Evidence: 1 integration test passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary`
  - Result: passed.
  - Evidence: 4 integration tests passed.
- `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features`
  - Result: passed.
  - Evidence: 4 facade integration tests passed.
- `cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features platform`
  - Result: passed.
- `cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features bsn`
  - Result: passed.
- `rg -n "BranchDiff|RouteBranchFingerprint|RouteSearchFingerprint|RouteSlotKey|ErrorBoundaryKey" crates\bevy_ui_router_core\src`
  - Result: found the Phase 1B branch-planning contracts in core source files.
- `git status --short`
  - Result before writing this verification record: clean after removing the generated untracked `Cargo.lock`.

Phase 2 handoff status:

- Branch diff, raw params/search fingerprints, route declaration slots, explicit keys, route metadata, error-boundary candidates, diagnostics, public module-path coverage, facade prelude coverage, dependency boundary checks, and constructor/provenance compile-fail boundaries are present.
- Phase 1B remains data-only. It does not allocate frames or generations, mark active UI, mutate ECS hierarchy, run async resolve/apply, retain hidden subtrees, manage focus, or introduce platform/BSN/app-shell assumptions into core.
