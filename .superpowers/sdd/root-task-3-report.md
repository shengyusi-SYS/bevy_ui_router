# Root Task 3 Report

## Commit

Shared-index race: `4cceb772fc09b706e4244801f60b00c6ec0c71ad`
(`docs: correct root state contract`) captured this complete Task 3 implementation together with
the concurrent documentation correction. Per the follow-up instruction, no subsequent Git write
operation was used to split, amend, or replace that commit. The requested
`feat(core): define root navigation state model` commit was therefore not created.

## RED Evidence

The initial Core test command failed before implementation with the expected missing-module and
missing-allocator errors: `E0583` for `admission`, `presentation`, `request`, `trace`, and
`view_input`, plus `E0432` for `CoreIdAllocator`.

The corrected `ActivePresentation::matched()` contract was also tested RED first. Its focused test
failed with `E0599` until the read-only accessor was added.

## GREEN Evidence

`cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core` passed with:

- 114 unit and integration tests
- 9 isolated consumer compile-fail tests
- 6 dependency-boundary tests
- 5 public API tests
- 1 consumer state test
- 31 doctests

`cargo check --locked --config "build.rustc-wrapper = ''" --workspace --all-targets`,
`cargo fmt --all --check`, and `git diff --check` also passed.

## Files

- Added Core admission, presentation, request-record, trace, and view-input modules.
- Extended Core IDs and frame versions/origins.
- Added focused state and isolated consumer non-forgeability tests.

## Concerns

- `NavigationRequestSnapshot` remains absent from Core by contract; Task 7 owns its public ECS
  projection.
- Documentation revisions owned by another contributor were unintentionally captured by the
  shared-index race in `4cceb77`; no docs were edited as part of this follow-up.
