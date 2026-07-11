# Root Task 3 Report

## Commit

Baseline: `0532589` (`feat(core): define root navigation state model`).

Follow-up: `fix(core): seal root navigation state model` (this change).

## RED Evidence

The follow-up regression suite was added before its implementation. The Core test command failed
with the expected missing lifecycle/error APIs, including sealed admission settlement,
`IdExhaustion`, exact host confirmation, request status transitions, and guarded trace append.
Consumer fixtures also failed until admission protocol names became unreachable and
`RouteViewInput` lost its unlisted traits.

## GREEN Evidence

`cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core` passed with:

- 125 unit tests
- 10 isolated consumer compile-fail tests
- 6 dependency-boundary tests
- 6 public API tests
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

## Concerns

- `NavigationRequestSnapshot` remains absent from Core by contract; Task 7 owns its public ECS
  projection.
- Admission settlement and `RouterNavigationState` remain crate-private foundations for Task 4;
  no public host lease or settlement API was introduced.
- No authoritative documentation was modified by this follow-up.
