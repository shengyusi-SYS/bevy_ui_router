# Root Task 3 Report

## Commit

Baseline: `0532589` (`feat(core): define root navigation state model`).

First follow-up: `9f9caa2` (`fix(core): seal root navigation state model`).

Second follow-up: `fix(core): bind confirmed host provenance` (this change).

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

## GREEN Evidence

`cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core` passed with:

- 129 unit tests
- 11 isolated consumer compile-fail tests
- 6 dependency-boundary tests
- 7 public API tests
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

## Concerns

- `NavigationRequestSnapshot` remains absent from Core by contract; Task 7 owns its public ECS
  projection.
- Admission settlement and `RouterNavigationState` remain crate-private foundations for Task 4;
  no public host lease or settlement API was introduced.
- No authoritative documentation was modified by this follow-up.
