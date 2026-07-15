# Task 4 Cleanup Authority Layering Design

**Date:** 2026-07-15

**Status:** Pending written-spec review

## 1. Goal

Close Root Push Task 4 with the cleanup authority boundary required by the
Authority Transition Matrix before the private `RouterEngine` is connected to
ECS.

Core owns logical lifecycle authority: request identity, ordered logical cleanup
roots, batch identity, commit publication, the outstanding cleanup gate, and the
request terminal transition. ECS owns all physical cleanup truth: exact Bevy
`Entity` obligations, immutable manifests, attempt revisions, World evidence,
ledgers, receipts, terminal dispositions, and quarantine transfer.

This correction remains Core-private and disconnected. It does not add an ECS
implementation or a second active/history publisher.

## 2. Authoritative Inputs

The design follows:

- `AGENTS.md`
- `docs/authority-transition-matrix.md`, especially Cleanup Obligation Ledger
- `docs/target-architecture.md`
- `docs/root-push-contract-profile.md`
- `docs/facade-contract-fixture.md`
- `docs/facade-associated-api-golden.txt`
- Root Push Task 4, Task 9, and Task 10 in
  `docs/superpowers/plans/2026-07-11-root-push-implementation-plan.md`

When those sources describe the final integrated system, this Task 4 correction
implements only the request-bound post-commit success subset. Task 9 and Task 10
remain responsible for physical ECS integration, failure cleanup, fallback,
authority settlement, quarantine, and public snapshot projection.

## 3. Problem

The current private protocol seals a `Vec<RouteFrameVersion>` as a cleanup
manifest, creates one Core obligation per logical frame version, constructs
receipts in Core, and verifies their order and disposition before allowing the
request to become terminal.

That makes a logical root serve as an Entity obligation, manifest entry, ledger
entry, and receipt identity. The final architecture instead requires a single
logical root to expand during ECS preflight into a data-dependent exact Entity
closure. Core cannot know the Entity cardinality, reclassify current World
evidence, validate mutation hooks, or issue a physical terminal disposition.

Leaving the current model in place would force Task 9 to replace the cleanup
protocol while also wiring ECS, switching facade callers, publishing snapshots,
and deleting the legacy authority path. Task 4 therefore closes only after its
private cleanup model has the final ownership direction.

## 4. Scope

### 4.1 In scope

- Request-bound normal root commit cleanup.
- Exact validation of the current `RootTaskIdentity`.
- Deterministically ordered logical cleanup roots.
- The reserved `CleanupBatchId` for the running request.
- ECS-supplied manifest digest and obligation count as opaque physical facts.
- A Core-sealed batch token binding the logical and physical correlation facts.
- A tracked sealed-commit state before mutation.
- A single outstanding cleanup token after publication.
- A sealed ECS terminal-ledger attestation accepted exactly once.
- An aggregate Core `CleanupSettled(None)` lifecycle marker.
- Zero-write rejection and focused Core-private tests.

### 4.2 Out of scope

- Bevy `Entity`, registry record, owner, component, relationship, or World data.
- ECS cleanup manifests, attempts, ledgers, receipts, dispositions, or leases.
- Pending-output failure cleanup, fallback, quarantine, reaping, or recovery.
- Host or authority settlement identity and continuation.
- ECS systems, facade wrappers, public snapshot merging, or caller migration.
- Changes to the legacy `runtime.rs` or `transaction.rs` path.
- New public exports or facade-visible cleanup authority.

The Task 4 types and method names must not be described as the universal cleanup
protocol. `RootTaskIdentity` is valid for this request-bound commit slice; it is
not a substitute for the request-or-settlement scope required by later slices.

## 5. Authority Boundary

### 5.1 Core-owned facts

Core owns and verifies:

- the complete immutable `RootTaskIdentity`;
- the current lifecycle stage;
- the reserved `CleanupBatchId`;
- the expected ordered logical roots;
- the association between those roots and the supplied manifest digest/count;
- the unique sealed token and its current outstanding status;
- single-use completion consumption;
- history/presentation publication and the request continuation.

Core may retain the logical roots or a deterministic binding of them. It must
not contain an Entity-equivalent obligation identity or infer an Entity count
from the number of logical roots.

### 5.2 ECS-owned facts

ECS exclusively owns:

- deterministic expansion from logical roots to exact owned Entity records;
- immutable Entity manifest ordering and digest calculation;
- obligation cardinality;
- current World evidence and attempt revision;
- exact-once receipt settlement;
- terminal disposition truth;
- quarantine transfer and background lease state;
- terminal-ledger digest calculation.

ECS must compare the returned token's manifest digest/count with the exact local
manifest used to establish the unique ledger. ECS may not treat the token as a
replacement for that comparison.

## 6. Core Data Model

The implementation may refine private names, but it must preserve the following
roles.

### 6.1 Current work

`CurrentWork` retains these cleanup facts before token issuance:

```text
cleanup_batch: CleanupBatchId
cleanup_roots: ordered Vec<RouteFrameVersion>
```

Removing the presealed token must not discard the batch reserved by the Task 3
allocator.

### 6.2 Commit ticket

`CommitTicket` carries only the single-use commit authority and exact task
identity needed to correlate it with `CurrentWork`. It does not expose a
presealed cleanup token or physical manifest assertion.

### 6.3 Manifest seal input

After physical preflight, the integration supplies a private typed value with:

```text
logical_roots: ordered logical roots submitted for verification
manifest_digest: CleanupManifestDigest
obligation_count: usize
```

`CleanupManifestDigest` is an opaque correlation digest. Core does not decode it
or recompute it from World data.

For the Task 4 post-commit success subset:

- first push and equivalent push have zero logical roots and zero obligations;
- fragment remount and changed-root replacement have the exact old logical root
  and a nonzero obligation count;
- zero roots with a nonzero count, or nonzero roots with a zero count, is an
  invalid seal for this subset;
- one logical root may correspond to more than one physical obligation.

### 6.4 Cleanup batch token

The sealed token binds:

```text
RootTaskIdentity
+ CleanupBatchId
+ deterministic logical-root binding
+ CleanupManifestDigest
+ obligation_count
```

The token remains opaque outside the narrow Core/ECS integration path. Its
binding digest is domain-separated from the manifest digest and completion
binding so values cannot be substituted across roles.

### 6.5 Completion attestation

The ECS terminal-ledger attestation contains:

```text
original CleanupBatchToken
+ CleanupCompletionDigest
+ settled_count
+ domain-separated structural binding
```

The integration constructor is available only after the ECS ledger has reached
its terminal state. Core verifies exact token equality, settled count equality,
the canonical zero-count representation, and the structural binding before its
first state write.

This verification is correlation and replay protection, not an independent
proof that ECS receipts are truthful. Manifest coverage, exact-once settlement,
terminal disposition, and terminal-ledger digest correctness remain ECS
authority.

## 7. Lifecycle And Data Flow

### 7.1 Commit sealing

The request reaches `ReadyToCommit` with its identity, reserved batch, and
expected logical roots already stored by Core.

The commit sealing operation performs every rejectable Core check before any
physical mutation:

1. Verify the current stage is `ReadyToCommit`.
2. Verify every `RootTaskIdentity` field.
3. Verify the ticket is the current single-use ticket.
4. Verify exact ordered logical-root equality.
5. Verify the Task 4 root/count coherence rules.
6. Seal the token from the retained batch and supplied digest/count.
7. Atomically change Core state to `CommitSealed { token }`.
8. Return a single-use sealed commit capability exposing only read-only token
   correlation facts to the integration.

There must be no interval in which a token exists but Core tracks neither the
sealed commit nor the outstanding cleanup batch.

### 7.2 ECS ledger establishment and publication

Task 4 tests use a Core-private fixture for the integration side. Task 9 will:

1. Compare the token digest/count with the already frozen local Entity manifest.
2. Establish exactly one ECS ledger keyed by the token.
3. Place the single-use sealed publication capability in the immutable commit
   mutation plan.
4. Execute the already preflighted, non-fallible switch mutation.
5. Consume the publication capability in the same exclusive operation.

On the normal return path, publication performs no lookup, allocation, World
validation, or fallible engine operation. It publishes presentation and private
history intent, appends `CommitPublished`, moves the request to
`PostCommitCleanup`, and changes Core state from `CommitSealed { token }` to
`PostCommitCleanup { outstanding: token }`.

Task 4 does not define physical preflight failure or sealed-commit abandonment;
Task 9/10 must add the appropriate failure continuation before the integration
can leave a sealed commit unconsumed.

### 7.3 Completion

ECS submits a terminal-ledger attestation only after all exact obligations in
its unique ledger have settled once. Core accepts it only when:

- the request is in `PostCommitCleanup`;
- its token exactly equals the current outstanding token;
- the settled count equals the token obligation count;
- the structural binding is valid;
- the zero-count representation is canonical when the count is zero.

One accepted Task 4 post-commit completion appends exactly one aggregate
`CleanupSettled(None)`, transitions the request to `Succeeded`, appends
`RequestTerminal` last, clears the outstanding batch, and advances the queue.
Replay after consumption is zero-write.

Zero-obligation cleanup is still a real batch. Task 9 must submit its canonical
empty-ledger attestation in the same cleanup stage; commit alone never implies
terminal success.

## 8. Trace And Snapshot Projection

Task 4's `CleanupSettled(None)` means only that the current batch is completely
settled. It is not an Entity receipt and it does not report a physical
disposition.

The final public trace still requires zero or more ordered disposition receipt
events before the aggregate marker:

```text
CommitPublished
RequestPhaseChanged(PostCommitCleanup)
CleanupSettled(Some(disposition)) repeated in stable obligation order
CleanupSettled(None) aggregate batch marker
RequestTerminal
```

Task 9/10 must project the `Some(disposition)` events from the ECS ledger without
giving Core Entity receipt authority. The projection is frozen when the
foreground ledger closes and retained at least until the corresponding request
record is evicted. Reaper progress may update diagnostics/audit but cannot
rewrite a terminal request's frozen receipt projection.

The existing public `CleanupDisposition`, `cleanup_disposition()` accessor, and
`CleanupSummary::last_disposition()` contract remain unchanged. Task 4 neither
deletes them nor fabricates physical values for them.

## 9. Rejection And Error Semantics

Every rejected seal or completion is lifecycle zero-write. In particular it
does not change allocator state, request records, trace, presentation, history,
queue order, retained records, or current cleanup state, and it never invokes
the mutation closure.

Private errors distinguish at least:

- stale stage, ticket, identity, token, batch, or replay;
- invalid logical-root or manifest metadata correlation;
- invalid completion attestation structure.

The implementation may preserve a narrower private error enum if tests prove
all required zero-write classifications and no public consumer observes the
difference.

No check that can return an error may run after the physical mutation begins.
The publication step on the valid path is non-fallible.

## 10. Required Tests

Tests are written RED before production changes and cover:

1. `CommitTicket` no longer contains or exposes a presealed cleanup token.
2. `CurrentWork` retains the allocator-issued batch and ordered logical roots.
3. Every independent `RootTaskIdentity` mismatch rejects before mutation.
4. Missing, extra, foreign, duplicate, and reordered logical roots reject before
   mutation.
5. Manifest digest/count are bound into the exact token and cannot be exchanged
   between batches.
6. First/equivalent push accepts zero roots with canonical zero count.
7. Fragment/replacement accepts the exact old logical root with nonzero count.
8. Task 4 root/count incoherence rejects before mutation.
9. A sealed token is tracked in `CommitSealed` before publication.
10. Duplicate or foreign sealed commit capability is zero-write.
11. Successful publication remains `Running(PostCommitCleanup)`.
12. Valid nonzero completion requires no Core receipt list.
13. Wrong token, completion digest binding, settled count, zero representation,
    or batch rejects before state mutation.
14. Exact completion succeeds once; replay and foreign completion are zero-write.
15. Every accepted batch appends exactly one aggregate `CleanupSettled(None)` and
    `RequestTerminal` remains last.
16. Allocator, presentation, history, request, trace, queue, and retention
    snapshots remain identical around every reject path.

The Task 4 implementation gate runs:

```text
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core engine
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
cargo check --locked --config "build.rustc-wrapper = ''" --workspace --all-targets --all-features
cargo fmt --all --check
git diff --check
```

No verification is reported as passing without successful command output.

## 11. File And Commit Boundaries

The implementation/TDD change owns only:

- `crates/bevy_ui_router_core/src/engine.rs`
- `crates/bevy_ui_router_core/src/cleanup.rs`

It does not modify ECS, facade, View, `runtime.rs`, `transaction.rs`, allocator
semantics, public trace types, the Root Push implementation plan, or the Task 4
brief.

After the implementation gate and both independent reviews pass, a separate
evidence/status change updates only:

- `.superpowers/sdd/root-task-4-report.md`
- `docs/current-architecture.md`
- `.superpowers/sdd/progress.md`

The report must remove claims that Core owns a receipt ledger. The progress file
changes Task 4 to complete only after a factual contract review and a lifecycle
quality review both pass. The current-architecture document records the private,
disconnected engine honestly and continues to identify the legacy runtime as the
only integrated publisher until Task 9.

Shared-workspace commits use exact file lists and a single index owner. No broad
staging command is allowed.

## 12. Review Gates

Task 4 closes only after:

1. Focused RED/GREEN evidence exists for the corrected protocol.
2. Full Core tests, workspace check, format check, and diff check pass.
3. A factual authority review confirms Core contains no Entity-equivalent
   obligation, receipt, attempt, or ledger authority.
4. A lifecycle quality review confirms sealing, publication, completion,
   zero-write rejection, trace order, and queue continuation.
5. The implementation report records the corrected evidence and limitations.
6. `docs/current-architecture.md` and `.superpowers/sdd/progress.md` are updated
   only after the preceding gates pass.

Task 5 may begin only after this corrected Task 4 closure is recorded.
