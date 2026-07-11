# Root Task 4 Report

## Result

Implemented the disconnected, Core-private `RouterEngine` first-push authority and logical cleanup token protocol. The legacy `runtime.rs` and `transaction.rs` path remains unchanged and solely integrated.

The new private engine owns install-time matching, sealed host confirmation, monotonic request queueing, full `RootTaskIdentity`, report and commit checks, private push history intent, logical publication, explicit cleanup completion, and FIFO retention of the most recent 64 terminal records. Private lookup preserves the facts needed to distinguish retained, evicted, and unknown requests.

## TDD Evidence

RED command:

```text
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core engine
```

The first run failed as intended with unresolved `RouterEngine`, `EngineAction`, `ReportError`, and `CommitError` imports and a missing private `cleanup` module. No production engine or cleanup implementation existed at that point.

Focused GREEN: 9 engine tests passed. They cover no-host zero-allocation rejection, sealed host confirmation, internal canonical matching, one-running ordered queueing, independent identity-field rejection at report/commit/cleanup boundaries, publication gating, repeated-root semantics, explicit zero/nonzero cleanup completion and replay, trace terminal order, and private 64-record retention facts.

Full Core GREEN:

```text
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
```

Result: 204 passed across 145 library tests, 27 integration tests, and 32 doctests; 0 failed.

Workspace static GREEN:

```text
cargo check --locked --config "build.rustc-wrapper = ''" --workspace --all-targets --all-features
```

Result: all six workspace crates checked successfully.

## Owned Files

- `crates/bevy_ui_router_core/src/engine.rs`
- `crates/bevy_ui_router_core/src/cleanup.rs`
- `crates/bevy_ui_router_core/src/history.rs`
- `crates/bevy_ui_router_core/src/lib.rs`
- `.superpowers/sdd/root-task-4-report.md`

## Boundary And Concerns

- Task 4 proves logical preflight and the non-fallible Core publication capability only. It does not prove a Bevy `World` preflight or physical atomic switch; Task 9 consumes this boundary after ECS preflight.
- The host fixture uses only the sealed Core issuer/token/grant/confirmation protocol. It intentionally does not simulate Bevy topology or inert staging validation.
- Every cleanup batch, including count zero, requires one exact completion. A batch appends exactly one payload-free `CleanupSettled`; physical receipt and disposition details remain Task 10 work.
- The engine, identities, report sessions, commit tickets, cleanup types, retention, and lookup facts are private and disconnected. Task 9 owns cross-crate integration and public snapshot projection.
- Root fallback and full match/build/apply failure recovery remain assigned to Task 10.

Requested commit subject: `feat(core): build router engine authority`.
