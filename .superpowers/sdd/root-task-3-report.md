# Root Task 3 Report

## Commit

Baseline: `0532589` (`feat(core): define root navigation state model`).

First follow-up: `9f9caa2` (`fix(core): seal root navigation state model`).

Second follow-up: `61c90d9` (`fix(core): bind confirmed host provenance`).

Third follow-up: `61cc14f` (`fix(core): bind world router provenance`).

Final mechanical follow-up: `fix(core): narrow identity dead code guards` (this change).

## RED Evidence

The follow-up regression suite was added before its implementation. The Core test command failed
with the expected missing lifecycle/error APIs, including sealed admission settlement,
`IdExhaustion`, exact host confirmation, request status transitions, and guarded trace append.
Consumer fixtures also failed until admission protocol names became unreachable and
`RouteViewInput` lost its unlisted traits.

The second follow-up started with focused Core tests that failed with `E0599` for the missing
`AdmissionError::AuthorityExists` and crate-private `HostVersion` identity accessors, plus `E0061`
because `RouterNavigationState::new` did not accept a router identity. After the provenance fix,
the new public-API quality regression remained RED until broad dead-code allowances were replaced
with item-scoped, test-aware allowances.

The third follow-up added cross-World regressions before implementation. With two allocators whose
first public router IDs were both `1`, four focused tests failed because request ownership accepted
the foreign router, host and frame versions compared equal, and router state accepted the foreign
confirmation. A second RED failed on the missing `AdmissionError::ForeignRouter` until host/slot
allocation also verified the token's exact World-bound router identity before mutation.
During final takeover review, a focused source-boundary RED also caught the remaining private
`CoreIdAllocator::allocate_router` path that discarded the World domain. Removing that unused path
made exact `RouterIdentity` allocation the only router-allocation route.

The final mechanical follow-up extended the Task 3 foundation dead-code guard to `ids.rs`. Its
focused test failed on plain item allowances and the blanket `CoreIdAllocator` impl allowance,
then passed after suppression was narrowed to concrete test-aware types and methods.

## GREEN Evidence

`cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core` passed with:

- 136 unit tests
- 12 isolated consumer compile-fail tests
- 6 dependency-boundary tests
- 8 public API tests
- 1 consumer state test
- 31 doctests

`cargo check --locked --config "build.rustc-wrapper = ''" --workspace --all-targets`,
`cargo fmt --all --check`, and `git diff --check` also passed.

## Files

- Sealed admission as a single-outstanding token/grant lifecycle with checked issuer, sequence,
  epoch, host, and slot allocation.
- Preserved exact confirmed host/top-slot authority and rejected stale or replayed confirmations
  without state advancement.
- Closed request and trace terminal transitions, aligned public associated items with the golden,
  and tightened isolated consumer non-forgeability tests.
- Bound `RouterNavigationState` to one router identity and reject same-issuer confirmations for a
  different router without mutation.
- Kept confirmed authority single-owner until a future explicit settlement, added crate-private
  `HostVersion` identity access for Task 4, and proved it can construct `RouteFrameVersion`.
- Replaced source-string positive API checks with compiler-checked function pointers and trait
  bounds; source checks remain only for negative `const`, derive, and allow-scope constraints.
- Added a Core-private `RouterIdentity` that binds World domain and public router ID, and carried it
  through request ownership, admission, confirmed authority, and route frame versions.
- Preserved the golden raw router projection while making cross-World request, host, frame, and
  confirmation identities unequal and non-interchangeable.
- Removed the allocator's raw-only router allocation path and retained regressions for both
  cross-World same-raw-router mismatches and same-World different-router mismatches.
- Removed the blanket `CoreIdAllocator` impl dead-code allowance and covered `ids.rs` with the same
  Task 3 item-scoped, test-aware guard used by the other private foundations.

## Concerns

- `NavigationRequestSnapshot` remains absent from Core by contract; Task 7 owns its public ECS
  projection.
- Admission settlement and `RouterNavigationState` remain crate-private foundations for Task 4;
  no public host lease or settlement API was introduced.
- The existing live legacy runtime remains the sole publisher until the planned Task 9 caller
  migration/removal/integration cutover; Task 3 adds no adapter, alias, shim, or new caller.
- No authoritative documentation was modified by this follow-up.
