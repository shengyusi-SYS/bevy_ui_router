# Root Task 4 Report

## Result

Implemented the disconnected, Core-private `RouterEngine` first-push authority and logical cleanup token protocol. The legacy `runtime.rs` and `transaction.rs` path remains unchanged and solely integrated.

The new private engine owns install-time matching, sealed host confirmation, monotonic request queueing, full `RootTaskIdentity`, report and commit checks, private push history intent, logical publication, explicit cleanup completion, and FIFO retention of the most recent 64 terminal records. Private lookup uses bounded issued/evicted watermarks to distinguish retained, evicted, and unknown requests.

## TDD Evidence

RED command:

```text
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core engine
```

The original Task 4 RED failed as intended with unresolved engine/cleanup protocol types. The review-fix RED then failed with 25 compile errors for the deliberately missing atomic reservation, presealed ticket cleanup, unit-closure commit, receipt ledger, generation fail-closed, and bounded watermark APIs.

Focused GREEN: 19 engine tests and 1 allocator reservation test passed. They cover confirmed-host admission rejection, five-counter atomic reservation failure, internal canonical matching, one-running ordered queueing, independent identity-field rejection, automatic post-closure publication, exact old-frame manifests, zero/nonzero cleanup receipt ledgers, missing/duplicate/foreign/foreign-batch/out-of-order rejection, generation overflow settlement, match-failure queue handoff, trace order, digest batch/order sensitivity, and bounded 64-record retention facts.

Full Core GREEN:

```text
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
```

Result: 215 passed across 156 library tests, 27 integration tests, and 32 doctests; 0 failed.

Workspace static GREEN:

```text
cargo check --locked --config "build.rustc-wrapper = ''" --workspace --all-targets --all-features
```

Result: all six workspace crates checked successfully.

## Owned Files

- `crates/bevy_ui_router_core/src/engine.rs`
- `crates/bevy_ui_router_core/src/cleanup.rs`
- `crates/bevy_ui_router_core/src/history.rs`
- `crates/bevy_ui_router_core/src/ids.rs`
- `crates/bevy_ui_router_core/src/lib.rs`
- `crates/bevy_ui_router_core/src/trace.rs`
- `.superpowers/sdd/root-task-4-report.md`

## Boundary And Concerns

- Task 4 proves logical preflight and the non-fallible Core publication capability only. It does not prove a Bevy `World` preflight or physical atomic switch; Task 9 consumes this boundary after ECS preflight.
- The host fixture uses only the sealed Core issuer/token/grant/confirmation protocol. It intentionally does not simulate Bevy topology or inert staging validation.
- Every cleanup batch, including count zero, requires one exact completion. Zero batches append one payload-free `CleanupSettled`; nonzero batches append one disposition-bearing event per exact ordered receipt. Task 4 tests use simulated `Removed` receipts; Task 9/10 supply real physical results.
- The engine, identities, report sessions, commit tickets, cleanup types, retention, and lookup facts are private and disconnected. Task 9 owns cross-crate integration and public snapshot projection.
- Match failure now settles its known request and advances the queue, but root fallback and full match/build/apply recovery remain assigned to Task 10.

Review-fix commit subject: `fix(core): seal root engine publication`.
