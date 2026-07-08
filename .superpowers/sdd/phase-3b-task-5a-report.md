# Phase 3B Task 5A Report

## Status

Completed.

## Commit SHA

`23c0517e309fdce85c6a86453761844a31be5fe2`

## Changed Files

- `crates/bevy_ui_router_ecs/src/cleanup.rs`
- `crates/bevy_ui_router_ecs/src/lib.rs`
- `crates/bevy_ui_router_ecs/tests/public_api.rs`

## RED Command / Output Summary

Command:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs cleanup::tests -- --nocapture
```

Observed RED phases:

1. Initial cleanup test RED failed in `cleanup::tests::cleanup_removes_only_matching_subtree_and_preserves_host_and_siblings` because `cleanup_router_owned_subtree` still contained `todo!("Task 5A cleanup implementation")`.
2. Boundary/accessor RED failed after tightening the tests because `CleanupReport` did not yet expose `removed(&self) -> &[Entity]` or `skipped_mismatches(&self) -> &[Entity]`.

Representative failure summary:

- `no method named 'removed' found for struct 'CleanupReport'`
- `no method named 'skipped_mismatches' found for struct 'CleanupReport'`

## GREEN Command / Output Summaries

### 1. Focused cleanup tests

Command:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs cleanup::tests -- --nocapture
```

Result:

- `2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Cleanup tests proved:
  - matching `RouteFrameRoot` identity is required before traversal
  - app-owned host survives cleanup
  - app-owned sibling children survive cleanup
  - foreign router / other frame roots are skipped
  - ordered removed/skipped evidence is observable through `CleanupReport` accessors

### 2. Formatting check

Command:

```powershell
cargo fmt --all --check
```

Result:

- Exit code `0`
- No formatting diffs after the final import-order adjustment

### 3. Public API / boundary evidence

Command:

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test public_api -- --nocapture
```

Result:

- `11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Added boundary evidence proved:
  - `cleanup` stays `pub(crate)` in ECS root
  - prelude does not expose cleanup authority
  - `CleanupCapability` and `cleanup_router_owned_subtree` are not publicly re-exported
  - `CleanupReport` carries public read-only ordered evidence accessors

## Self-Review Notes

- Cleanup remains crate-private and cannot be invoked through the public ECS root/prelude API.
- `CleanupReport` now exposes read-only ordered slices without exposing mutation authority.
- Root matching checks router, host, slot, route, frame, and generation explicitly before descendant traversal.
- Mutation target ordering uses semantic frame identity first and `Entity::index()` as the stable Bevy tie-breaker.
- Cleanup never despawns the app-owned host directly; it only despawns matching router-owned frame roots.
- For the pinned Bevy branch, cleanup relies on `bevy_ecs::hierarchy::Children` traversal plus `EntityWorldMut::despawn()`. I verified locally in the pinned Bevy source that `Children` is declared with `#[relationship_target(relationship = ChildOf, linked_spawn)]`, which is why despawning the matching frame root recursively removes its descendants on this branch.

## Concerns

- `Cargo.lock` was generated during Cargo resolution in this workspace and remains untracked; it was not staged or committed because it is outside the Task 5A commit boundary.

## Follow-up Fix Evidence

### Review Fix Scope

- Added a narrow `bevy_ui_router_core` `test-support` feature and helper module at `bevy_ui_router_core::test_support`.
- Kept the helper out of `bevy_ui_router_core::prelude`.
- Enabled the helper only through `bevy_ui_router_ecs` dev-dependency scope for tests.
- Added runtime cleanup coverage for:
  - same frame id / different generation surviving cleanup
  - deterministic ordering of `removed()` under reverse discovery order
  - deterministic ordering of `skipped_mismatches()` under reverse discovery order

### Follow-up GREEN Commands

#### 1. Focused cleanup tests

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs cleanup::tests -- --nocapture
```

Result:

- `4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- New runtime proofs:
  - `cleanup_skips_same_frame_with_different_generation_from_core_test_support`
  - `cleanup_orders_removed_and_skipped_reports_deterministically_at_runtime`

#### 2. ECS public API boundary

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test public_api -- --nocapture
```

Result:

- `11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Boundary evidence now also checks the core `test-support` module stays feature-gated and out of the core prelude.

#### 3. ECS dependency boundary

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test dependency_boundary -- --nocapture
```

Result:

- `1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Verified ECS uses the core `test-support` helper only via dev-dependency scope.

#### 4. Core dependency boundary

```powershell
cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test dependency_boundary -- --nocapture
```

Result:

- `6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Verified the core manifest stayed dependency-clean after adding the `test-support` feature.

#### 5. Formatting check

```powershell
cargo fmt --all --check
```

Result:

- Exit code `1`
- Remaining formatting diffs are outside the approved write set:
  - `crates/bevy_ui_router_core/src/diagnostics.rs`
  - `crates/bevy_ui_router_core/src/matcher.rs`
  - `crates/bevy_ui_router_core/src/runtime.rs`
- I did not reformat those files because Task 5A + unblocker scope did not authorize editing them, and the workspace already showed them as modified.

### Follow-up Self-Review Notes

- The core helper clones a core-issued `FrameApplyRequest` with the same frame id and the next generation, without introducing any raw constructor or default-path public forging API.
- The helper is available only when the `test-support` feature is enabled.
- ECS cleanup authority remains crate-private.
- `CleanupReport` still exposes only read-only ordered slices.
