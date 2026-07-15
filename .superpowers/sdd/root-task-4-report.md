# Root Task 4 Report

## Result

Task 4 is complete. The disconnected, Core-private `RouterEngine` owns internal
matching, ordered request execution, logical presentation/history publication,
and the request-bound cleanup gate. The legacy `runtime.rs` and
`transaction.rs` path remains unchanged and is still the only integrated
publisher through Task 8.

The cleanup boundary now matches the authority matrix: Core retains only the
ordered logical roots, allocator-issued batch identity, opaque ECS manifest
digest/count, sealed correlation token, and aggregate completion attestation.
Exact Entity manifests, attempts, ledgers, receipts, terminal dispositions, and
quarantine truth remain exclusively ECS-owned.

## TDD And Verification Evidence

The corrected RED command was:

```text
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core engine
```

It failed with 37 compile errors for the deliberately absent manifest seal,
opaque digest types, `CommitSealed`, `SealedCommit`, `seal_commit`,
`publish_commit`, and retained logical cleanup roots.

The first lifecycle review then exposed a seal/publish queue regression. Its
focused RED ran 23 engine tests: 22 passed and the new seal-A/enqueue-B/publish-A
case failed because publication replaced the request-record deque with a stale
snapshot. The fix pre-resolves the current record slot and preserves records,
queue entries, and allocator state accepted after sealing.

Final GREEN evidence:

- focused engine tests: 23 passed, 0 failed;
- full Core suite: 160 unit tests plus all boundary, integration, and doctests
  passed;
- workspace all-target/all-feature check passed;
- `cargo fmt --all --check` passed;
- `git diff --check` exited 0.

## Lifecycle Boundary

- `CommitTicket` carries only the exact root-task identity.
- `seal_commit` verifies stage, every identity field, exact ordered logical
  roots, and root/count coherence before tracking `CommitSealed`.
- Publication validates and resolves its target record before the mutation
  closure. After the closure it performs only precomputed field moves and a
  fixed-slot assignment.
- Completion validates the exact outstanding token, settled count, canonical
  zero representation, and structural binding before its first write.
- One accepted completion appends exactly one aggregate
  `CleanupSettled(None)`, then succeeds the request with `RequestTerminal` last.
  Replay and foreign/malformed inputs are zero-write.

## Independent Reviews

- Factual authority review: PASS for spec compliance and code quality; no
  Critical, Important, or Minor findings.
- Lifecycle quality review: initially CHANGES REQUIRED for stale record-deque
  replacement; after commit `666bb1f`, re-review PASS for spec compliance and
  code quality with no remaining findings.

## Commits And Remaining Scope

- `323e4ee` `fix(core): separate cleanup authority layers`
- `666bb1f` `fix(core): preserve sealed request queue`

Task 4 does not prove Bevy World preflight, Entity ledger truth, physical atomic
switching, fallback, quarantine, or facade integration. Tasks 5-8 add private,
disconnected foundations; Task 9 performs the single authority cutover.
