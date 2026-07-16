# Nested Push Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** reviewed, ready for execution

**Stage contract:** `docs/nested-push-contract-profile.md`

**Planning baseline:** `1ccbcd7`

**Target version:** Phase 3B.1 `Plan PASS`

**Acceptance sources:** `docs/target-architecture.md`、`docs/authority-transition-matrix.md`、`docs/facade-contract-fixture.md`、`docs/facade-export-allowlist.md`、`docs/facade-associated-api-golden.txt`、`docs/mvp-acceptance-journeys.md`

**Goal:** 从公开 facade `push("/items/1")` 建立真实 parent outlet、sealed child mount、逐层 inert apply 与完整 branch 原子 publication，并保持 Root 已交付语义

**Architecture:** Core `RouterEngine` 继续独占逻辑 branch、frame/task/slot ID、mount token/grant 和唯一 publication；ECS 独占 exact Entity、registry、ownership seal、物理 outlet、child slot、preflight、input/focus 与 cleanup receipt。Nested 首先支持 Journey A，branch-changing 与 nested fragment-only navigation 安全替换整支 branch，common-prefix、History 与 Recovery 保持后续切片

**Tech Stack:** Rust 2024、Bevy `0.19.0` exact git revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`、Cargo、nextest、PowerShell、`syn` contract gates

## Global Constraints

- `RouterEngine` 是唯一 lifecycle publisher，禁止新增 nested runtime 或 ECS publication authority
- Core 只拥有逻辑 branch、ID、token、digest 与 publication，ECS 独占 Entity manifest、ledger 与 receipt
- admission revision、frame/task/mount identity 必须绑定显式 router domain、host epoch、transaction、request 与 sequence
- `MountRequestToken` 不含 slot ID，只有 ECS exact preflight 成功后 Core 才分配 child slot ID
- seal、slot/frame/task/authority ID 一经消费不得复用，失败 grant 保留永久 ID gap
- Router-created Entity 必须 empty allocate → staging registry → inert write → relationship write → post-write validation
- 每次可能触发 hook/observer 的 write 后复验 exact Entity、owner、seal、manifest、lifecycle 与 parent/outlet mapping
- pending、leaving、quarantine 与 authority-invalid subtree 必须隔离 pointer、keyboard、gamepad 与 focus
- traversal 遇到 child slot 后停止，不能越过 child authority root
- atomic switch 只允许预解析的不可失败原地 mutation；成功切换的 leaving branch 只能在 `PostCommitCleanup` 做 despawn/relationship cleanup，precommit residual 则必须在 `CleanupPending|Recovering` 中经 Core-sealed ledger 收敛后才 terminal
- cleanup terminal receipt 必须早于 `RequestTerminal`，任何清理不得修改或销毁 app-owned host 或 foreign Entity
- Root equivalent、fragment generation、changed-root、fallback、cleanup 与 app-owned host zero-write 语义不得回退
- Nested 不宣称 Journey B、common-prefix、replace/back、route-local recovery、host-loss recovery、Authority closure 或 MVP PASS
- 不持有 `&mut World` 跨 `await`，不依赖 query iteration order，不启用 `ghost_nodes`
- 所有 Cargo 命令使用 `--locked --config "build.rustc-wrapper = ''"`，workspace 重验证使用单 build job
- 共享工作区只有主代理负责 index 与提交，子代理不得运行 `git add` 或 `git commit`
- 每次精确 `git add -- <files>` 后，主代理将 `git diff --cached --name-only` 与该任务文件清单逐项比对并运行 `git diff --cached --check`，出现职责外文件立即停止提交

## Execution Precondition

Task 1 开始前，本计划必须已通过独立计划复审并由主代理单独提交；执行者先运行 `git ls-files --error-unmatch docs/superpowers/plans/2026-07-16-nested-push-implementation-plan.md`，未跟踪则停止，不能把计划文件延迟到 Task 9 首次纳入版本控制

---

### Task 1: Accepted outlet plan retention

**Files:**
- Modify: `crates/bevy_ui_router_ecs/src/content.rs`

**Interfaces:**
- Consumes: existing crate-private `RouteTreeBuilder::{route, route_with_options}` and `FrameContentWriter::declare_outlet`
- Produces: private stable `AcceptedOutlet { local_index, key }` retained by `AcceptedContentPlan`
- Preserves: Root-stage public exclusions plus duplicate outlet、foreign session、self-edge、duplicate parent/edge、cycle and driver-return World-zero semantics

- [ ] **Step 1: Write the accepted outlet RED**

Add a content unit test that declares two outlets in reverse declaration order relative to topology and asserts stable `(local_index, key)` order

```rust
assert_eq!(
    accepted
        .outlets()
        .iter()
        .map(|outlet| (outlet.local_index(), outlet.key().clone()))
        .collect::<Vec<_>>(),
    vec![(root.index, first), (child.index, second)],
);
```

Keep the existing duplicate、foreign-session、cycle and zero-write tests unchanged

- [ ] **Step 2: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib content::tests::
```

Expected: fail because `AcceptedContentPlan::outlets` does not exist and accepted plans discard declarations

- [ ] **Step 3: Implement the final accepted-plan shape**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedOutlet {
    local_index: usize,
    key: RouteSlotKey,
}

impl AcceptedOutlet {
    pub(crate) const fn local_index(&self) -> usize { self.local_index }
    pub(crate) const fn key(&self) -> &RouteSlotKey { &self.key }
}

pub(crate) struct AcceptedContentPlan {
    entries: Vec<ContentRealizationEntry>,
    relationships: Vec<(usize, usize)>,
    roots: Vec<usize>,
    outlets: Vec<AcceptedOutlet>,
}
```

`ContentPlan::accept` maps every live outlet handle to its local index without consulting or mutating `World`, then stores outlets in stable `(local_index, key)` order

- [ ] **Step 4: Verify GREEN and Root compatibility**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test compile_fail_boundary
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib content::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys
```

Expected: all commands exit 0, nested declaration methods remain excluded from public consumers and existing Root journeys remain green

- [ ] **Step 5: Review and commit**

Review gate: outlet plan remains data-only, stable ordering is explicit and no public surface changes before the Task 8 atomic profile switch

Root stages only the file above and commits

```powershell
git add -- crates/bevy_ui_router_ecs/src/content.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ecs): retain accepted outlet plans"
```

---

### Task 2: Core sealed child-mount protocol

**Files:**
- Create: `crates/bevy_ui_router_core/src/mount.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Modify: `crates/bevy_ui_router_core/src/ids.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`

**Interfaces:**
- Consumes: `CoreIdAllocator` as the only slot allocator
- Produces: private `MountRequestToken`、`MountGrant`、`MountReceiptAttestation`、`MountRegistrationPermit`、`MountRegistrationCompletion`、`MountConfirmation`、`MountIssuer` and `MountError`
- Produces: reject/cancel/grant/accept-receipt/complete-registration/reject-after-mutation single-use transitions

- [ ] **Step 1: Write mount protocol RED tests**

Add direct tests named

```rust
mount_request_has_no_slot_until_exact_resolution
mount_request_reject_cancel_accept_receipt_complete_registration_and_replay_are_single_use
mount_grant_rejection_burns_slot_id
mount_identity_rejects_foreign_router_host_request_transaction_sequence_parent_outlet_or_stage
mount_receipt_and_registration_reject_wrong_digest_foreign_permit_and_duplicate_or_stale_completion
mount_registration_permit_cannot_issue_child_before_exact_completion
```

The first test snapshots `CoreIdAllocator`, issues a request, proves the slot counter is unchanged, grants after resolution, then proves exactly one slot ID was consumed

The replay test clones internal test tokens before their first terminal transition and requires every replay to return `MountError::Stale` without changing allocator or issuer state

- [ ] **Step 2: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib mount::tests::
```

Expected: compile failure because module and types do not exist

- [ ] **Step 3: Implement the sealed protocol**

```rust
pub(crate) struct MountRequestToken { identity: MountIdentity, binding: u64 }
pub(crate) struct MountGrant { identity: MountIdentity, slot: RouteSlotId, binding: u64 }
pub(crate) struct MountReceiptDigest(u64);
pub(crate) struct MountRegistrationDigest(u64);
pub(crate) struct MountReceiptAttestation {
    identity: MountIdentity,
    slot: RouteSlotId,
    digest: MountReceiptDigest,
    binding: u64,
}
pub(crate) struct MountRegistrationPermit {
    identity: MountIdentity,
    slot: RouteSlotId,
    receipt_digest: MountReceiptDigest,
    binding: u64,
}
pub(crate) struct MountRegistrationCompletion {
    identity: MountIdentity,
    slot: RouteSlotId,
    receipt_digest: MountReceiptDigest,
    registration_digest: MountRegistrationDigest,
    binding: u64,
}
pub(crate) struct MountConfirmation {
    identity: MountIdentity,
    slot: RouteSlotId,
    receipt_digest: MountReceiptDigest,
    registration_digest: MountRegistrationDigest,
    binding: u64,
}

pub(crate) struct MountIdentity {
    router: RouterIdentity,
    host: HostVersion,
    request: NavigationRequestId,
    transaction: TransactionId,
    sequence: u64,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
}

pub(crate) enum OutstandingMount {
    Requested(MountIdentity),
    Granted { identity: MountIdentity, slot: RouteSlotId },
    Confirming {
        identity: MountIdentity,
        slot: RouteSlotId,
        receipt_digest: MountReceiptDigest,
    },
}

pub(crate) struct MountIssuer {
    router: RouterIdentity,
    outstanding: Option<OutstandingMount>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MountError {
    Foreign,
    Stale,
    WrongPhase,
    Exhausted,
}
```

`MountIssuer::issue` obtains its monotonic mount sequence from a new counter inside the existing `CoreIdAllocator`; no local counter or second allocator is allowed

`MountIssuer::grant` checks the exact outstanding request before calling `CoreIdAllocator::allocate_slot`

`integration::MountGrant::attest_receipt(digest)` consumes the integration grant wrapper and produces `MountReceiptAttestation`; `MountIssuer::accept_receipt` validates it, moves the outstanding state from `Granted` to `Confirming` and returns a one-use `MountRegistrationPermit` without clearing authority or allowing a child action

ECS uses the opaque integration permit for the provisional→registered CAS; only after exact post-CAS validation does consuming `integration::MountRegistrationPermit::complete(registration_digest)` create the private `MountRegistrationCompletion`; `MountIssuer::complete_registration` validates it, clears `Confirming` and returns `MountConfirmation`, which is the first state that can authorize the child task

Each binding includes the artifact kind (`Requested`, `Granted`, `Receipt`, `RegistrationPermit`, `RegistrationCompletion` or `Confirmed`) so the distinct private types encode and validate the expected protocol stage in addition to router/host/request/transaction/sequence/parent/outlet identity

`reject` and `cancel` consume only requested state; grant-bound or registration-permit-bound `reject_after_mutation` handles drift from `Granted` staging or `Confirming` CAS respectively and leaves physical settlement to the sealed failure cleanup; only `complete_registration` returns the non-forgeable final confirmation

Do not export these types from `prelude` or facade

- [ ] **Step 4: Verify GREEN and identity non-reuse**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib mount::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib ids::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib integration::tests::mount_
```

Expected: all mount and allocator tests pass with a visible permanent slot gap after rejected grant

- [ ] **Step 5: Review and commit**

Review gate: no slot ID before resolution, no second allocator, every token transition exact and terminal

```powershell
git add -- crates/bevy_ui_router_core/src/mount.rs crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/ids.rs crates/bevy_ui_router_core/src/integration.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(core): add sealed child mount protocol"
```

---

### Task 3: Core root-to-leaf branch scheduling

**Files:**
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_core/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_core/src/frame.rs`
- Modify: `crates/bevy_ui_router_core/src/ids.rs`
- Modify: `crates/bevy_ui_router_core/src/trace.rs`
- Modify: `crates/bevy_ui_router_view/src/lib.rs`
- Modify: `crates/bevy_ui_router_view/src/effect.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs` (workspace compatibility holder only)
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs` (depth-one test helper exhaustiveness only)

**Interfaces:**
- Consumes: sealed mount protocol from Task 2
- Produces: hidden integration `FrameTask`、`MountRequest`、`MountGrant` and `FrameMountTarget`
- Produces: sealed `AppliedFrameReport` carrying the consumed task identity and stable logical outlet keys
- Produces: `EngineAction::{FrameTask, MountRequest, Commit}`
- Produces: full root-to-leaf `ActivePresentation::frames()` on one publication
- Preserves: workspace compilation by retaining an unhandled sealed `MountRequest` in one private ECS holder; Task 6 remains the only slice that consumes it and implements physical mount staging

- [ ] **Step 1: Write happy-path engine RED**

Build a two-level route using layout view plus `default` child slot and assert this action order

```text
FrameTask(parent, TopSlot)
MountRequest(parent, default)
FrameTask(child, ChildOutlet)
Commit
```

After receipt acceptance and while the registration permit is outstanding, assert `engine.action().is_none()` and the child frame task has not been issued; only exact registration completion may produce the final mount confirmation

After publication assert `active.frames() == [parent_version, child_version]` and history contains one requested location

Assert the exact normal trace subsequence

```text
FrameTaskIssued(parent) -> ShellPrepared(parent) -> ViewApplied(parent)
-> OutletRegistered(parent) -> MountRequested -> MountResolved -> MountGranted
-> FrameTaskIssued(child) -> ShellPrepared(child) -> ViewApplied(child)
-> FrameActivated(parent) -> FrameActivated(child) -> CommitPublished
```

- [ ] **Step 2: Write repeated nested classification RED**

```rust
equivalent_nested_push_is_zero_entering
branch_changing_nested_push_replaces_the_whole_branch
nested_fragment_push_replaces_the_whole_branch_until_reconciliation
depth_one_fragment_push_preserves_frame_and_advances_generation
queued_push_matches_once_and_reuses_the_stored_branch
match_failure_queues_without_branch_reservation_then_reserves_fallback_attempt
branch_reservation_exhaustion_is_complete_zero_change
```

The nested replacement tests require fresh parent/child frame IDs and cleanup roots in child-to-parent order without claiming common-prefix reuse

- [ ] **Step 3: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::nested_
```

Expected: fail because engine emits one leaf-only `RootFrameTask`

- [ ] **Step 4: Generalize task identity and actions**

```rust
struct FrameTaskIdentity {
    router: RouterIdentity,
    host: RouteHostId,
    host_epoch: u64,
    request: NavigationRequestId,
    transaction: TransactionId,
    frame_task: FrameTaskId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    slot: RouteSlotId,
    origin: FrameOrigin,
    branch_index: usize,
}

pub enum FrameMountTarget {
    TopSlot { host: HostVersion, slot: RouteSlotId },
    ChildOutlet { parent: RouteFrameVersion, outlet: RouteSlotKey },
}

struct ReservedFrameAttempt {
    frame_task: FrameTaskId,
    frame: RouteFrameId,
}

struct ReservedBranchAttempt {
    request: NavigationRequestId,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
    frames: Vec<ReservedFrameAttempt>,
}

enum QueuedMatchOutcome {
    Matched(MatchedBranch),
    MatchFailed(NavigationFailure),
}

struct QueuedPush {
    request: NavigationRequestId,
    location: RouteLocation,
    outcome: QueuedMatchOutcome,
    reservation: Option<ReservedBranchAttempt>,
}

struct AppliedFrameReport {
    identity: FrameTaskIdentity,
    outlets: Vec<RouteSlotKey>,
}
```

Keep `ViewReportIdentity` opaque and private-field only

`ViewRequestId` uses the frame-task sequence rather than navigation request sequence so parent and child effects in one request are distinct; it remains observational and never becomes authority

- [ ] **Step 5: Implement branch scheduling**

`push` uses one complete enqueue flow

1. normalize and match the immutable route tree exactly once before consuming any ID
2. for a matched branch, call `CoreIdAllocator::reserve_navigation_branch(branch_len)`; it checked-adds request, transaction, cleanup batch, every frame and every frame-task counter before changing any of them, then returns the single request identity plus `ReservedBranchAttempt`
3. queue the stored `MatchedBranch` plus `ReservedBranchAttempt`; later collection never rematches or reallocates the request
4. for match failure, use one request-only reservation and queue `MatchFailed`; when processed, the mandatory Root fallback uses the existing fresh-attempt reservation bound to that same request

If any matched-branch counter is exhausted, no queue entry is created and the complete allocator/engine snapshot remains unchanged, including the request counter

Add allocation tests proving length/counter exhaustion is complete zero-change and any successful reservation is permanently consumed even when mount resolution later fails

Remove the dormant `FrameAllocator`; branch/frame helpers consume IDs reserved by the engine's single `CoreIdAllocator`, and tests use the same allocator rather than a second counter

For each branch index

1. construct the frame only after its physical slot is known
2. issue parent `FrameTask`
3. consume the task into one `AppliedFrameReport` carrying the exact private identity and stable sorted logical outlet keys, then accept it once
4. issue `MountRequestToken` for the next route slot
5. after exact registration completion and final mount confirmation issue the child `FrameTask`
6. after leaf report issue one `CommitTicket`

`FrameTask::into_applied_report(outlet_keys)` consumes the non-forgeable task after ECS has validated the driver's `ViewReportIdentity`; Core rejects duplicate/foreign/unsorted outlet observations, then appends `ShellPrepared`, `ViewApplied` and one exact `OutletRegistered` per key without receiving any Entity claim

Mount resolve/grant/registration-completion methods append their exact frame/outlet/slot events; publication appends root-to-leaf `FrameActivated` then one `CommitPublished`

`precompute_publication` writes the complete frame vector, one history intent and one trace

- [ ] **Step 6: Verify GREEN and Root engine suite**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib ids::tests::branch_reservation_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --test public_api
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_view
cargo check --locked --workspace --all-targets --all-features -j 1 --config "build.rustc-wrapper = ''"
```

Expected: full Core engine and View suites pass and every workspace consumer remains exhaustive

- [ ] **Step 7: Review and commit**

Review gate: child task cannot exist before exact registered-slot completion and final mount confirmation, active publication contains one ordered branch, Root semantics stay green

```powershell
git add -- crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/src/cleanup.rs crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/ids.rs crates/bevy_ui_router_core/src/trace.rs crates/bevy_ui_router_view/src/lib.rs crates/bevy_ui_router_view/src/effect.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/snapshots.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(core): schedule nested frame branches"
```

---

### Task 4: Core nested failure cleanup and terminal ordering

**Files:**
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_core/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_core/src/trace.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs` (failure capability holder only)
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs` (depth-one test helper exhaustiveness only)

**Interfaces:**
- Produces: unsealed logical `FailureCleanupRequest` followed by the existing digest/count-bound `CleanupBatchToken`
- Produces: `reject_mount`、`reject_mount_after_mutation` and child frame failure transitions
- Produces: `EngineAction::FailureCleanup` only when an accepted frame cannot issue its next mount capability; ECS retains it without starting physical cleanup until Task 6
- Produces: `Running(CleanupPending|Recovering) -> CleanupSettled -> Failed -> RequestTerminal`
- Preserves: Root mandatory RouterFallback for match/root build/root apply failure

- [ ] **Step 1: Write terminal-order RED tests**

```rust
missing_outlet_waits_for_cleanup_before_mount_failure_terminal
invalid_outlet_waits_for_cleanup_before_mount_failure_terminal
rejected_after_mutation_burns_slot_and_waits_for_cleanup
child_build_and_apply_failure_wait_for_cleanup
parent_authority_loss_recovers_before_authority_lost_terminal
root_apply_failure_still_issues_router_fallback
```

For each pending case assert no `RequestTerminal` before cleanup completion and assert the final two semantic events are `CleanupSettled` then `RequestTerminal`

Also require `MissingOutlet` and `InvalidOutlet` to emit `MountRejected` with the exact `MountRejectionKind`, rejected-after-mutation to emit `MountRejectedAfterMutation`, and parent authority loss to emit `MountRejected(ParentAuthorityLost)` without fallback

- [ ] **Step 2: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::missing_outlet_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::child_build_
```

Expected: fail because the root-only engine has no mount failure ticket or nested cleanup state

- [ ] **Step 3: Implement failure tickets and sealed cleanup**

```rust
enum PendingFailureKind {
    Apply,
    Build,
    Mount(MountRejectionKind),
    AuthorityLost,
}

struct FailureCleanupRequest {
    identity: FrameTaskIdentity,
    logical_roots: Vec<RouteFrameVersion>,
    failure: NavigationFailure,
}
```

The failure transition first returns `FailureCleanupRequest` carrying the exact router/host epoch/request/transaction/frame-task/sequence/stage identity, failure and logical roots but no Entity or manifest claim

The outlet-aware report path accepts any stable sorted unique observation and still issues the matched `MountRequest` when its required key is absent, so ECS can consume the token as `MissingOutlet`; the legacy leaf-only `report_applied` path remains distinct and reissues the exact nested parent task with `Invalid`

Failure cleanup logical roots contain only the already accepted applied-frame prefix in child-to-parent order; an issued but unapplied frame is not a cleanup root, while ECS may bind additional slot or partial-realization obligations into the manifest digest and count

ECS freezes its exact physical obligations and returns the existing integration `CleanupManifest { logical_roots, manifest_digest, obligation_count }`; `prepare_failure_cleanup` verifies the exact request and logical roots, then seals the existing `CleanupBatchToken` with its reserved cleanup batch ID, digest and count

Core validates the final existing `CleanupCompletion` digest/count before terminal publication; exact Entity obligations, attempts and receipts remain ECS-only and no weaker parallel cleanup authority is introduced

`MissingOutlet`、`InvalidOutlet` and rejected-after-mutation map to `NavigationFailureKind::Mount`

child build/apply map to `Build`/`Apply`

parent authority loss maps to `AuthorityLost` and `NavigationPhase::Recovering`

All other precommit nested failure uses `CleanupPending`

Only `complete_cleanup` may append `CleanupSettled`, transition terminal and append the last `RequestTerminal`

- [ ] **Step 4: Verify GREEN and fallback preservation**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib integration::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib -j 1
cargo check --locked --workspace --all-targets --all-features -j 1 --config "build.rustc-wrapper = ''"
```

Expected: all Core failure, trace, cleanup and Root fallback tests pass and every workspace consumer retains the new failure capability exhaustively

- [ ] **Step 5: Review and commit**

Review gate: no terminal-before-cleanup path, no nested fallback authority, existing Root fallback remains

```powershell
git add -- crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/src/cleanup.rs crates/bevy_ui_router_core/src/trace.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/snapshots.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(core): settle nested failures before terminal"
```

---

### Task 5: ECS exact outlet realization and child-slot staging

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/mount.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Modify: `crates/bevy_ui_router_ecs/src/components.rs`
- Modify: `crates/bevy_ui_router_ecs/src/registry.rs`
- Modify: `crates/bevy_ui_router_ecs/src/presentation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/content.rs`

**Interfaces:**
- Consumes: `AcceptedOutlet` and Core `MountGrant`
- Produces: private `RealizedOutlet` and `FrameMountBinding`
- Produces: private `MountedChildSlot` with exact claim and sealed parent/outlet identity
- Produces: ECS-only `MountReceipt` attesting the exact staged slot and anchor; Core receives only its opaque digest plus logical grant identity
- Produces: registry-first `stage_child_slot`、`confirm_child_slot` and sealed-ledger-only `settle_rejected_child_slot`

- [ ] **Step 1: Write realization and registry RED**

```rust
accepted_outlet_maps_to_exact_realized_output
child_frame_shell_uses_confirmed_child_slot_not_top_slot
child_slot_is_provisionally_registered_before_any_observer_capable_write
child_slot_barrier_drift_is_rejected_and_inert
child_slot_node_closure_drift_is_rejected_and_inert
child_slot_authority_drift_is_rejected_and_inert
child_slot_relationship_drift_is_rejected_and_inert
child_slot_remains_provisional_before_registration_permit
child_slot_is_registered_inert_after_permit_until_exact_completion_and_confirmation
child_slot_seal_and_slot_id_are_not_reused_after_failure
```

Install separate observers that despawn, reparent, remove the barrier or add a foreign component after each barrier, Node-required-closure, private-authority and relationship write batch

- [ ] **Step 2: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib mount::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib presentation::tests::child_
```

Expected: compile failure because child-slot production staging and generic mount binding do not exist

- [ ] **Step 3: Generalize provisional registry owner**

```rust
enum ProvisionalOwner {
    HostTopSlot(ProvisionalTopSlotOwner),
    ChildSlot {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot: RouteSlotId,
    },
}

enum RegistryOwner {
    Committed(EntityOwner),
    Provisional(ProvisionalOwner),
}
```

Keep owner uniqueness, seal consumption and compare-and-swap claims unchanged

- [ ] **Step 4: Implement mount binding and child-slot staging**

```rust
struct FrameMountBinding {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    kind: FrameMountBindingKind,
}

enum FrameMountBindingKind {
    HostTop,
    ChildOutlet {
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        anchor: Entity,
        anchor_claim: ExactRegistryClaim,
    },
}

struct MountReceipt {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    anchor: Entity,
    anchor_claim: ExactRegistryClaim,
    digest: u64,
}
```

`RealizedFrame` stores this binding instead of `top_slot/top_slot_claim`

`MountedChildSlot` stores slot Entity、exact claim、parent frame、outlet key、anchor Entity and anchor claim

The child slot carries `Visibility::Hidden`, deny-all `RouterInputBarrier`, `Pickable::IGNORE` and a transparent 100% `Node` before relationship insertion; it cannot acquire focus before branch publication

Staging order is exact and individually testable

1. empty Entity allocation
2. provisional registry record with owner/seal/lifecycle/initial manifest validation
3. inert barrier batch → flush → exact Entity/owner/seal/manifest/lifecycle/anchor validation
4. transparent 100% Node required-closure batch → flush → the same exact validation
5. private authority batch → flush → the same exact validation
6. `ChildOf(anchor)` relationship write → flush → exact validation including the unique anchor edge
7. produce `MountReceipt` while the owner remains provisional and inert
8. pass only its digest through the exact Core grant to obtain `MountRegistrationPermit`; Core stays `Confirming` and emits no child action
9. `confirm_child_slot` validates the unchanged receipt/claims, uses the permit for the provisional→registered CAS, flushes and exact-revalidates the registered inert `ChildSlot`
10. return permit-bound `MountRegistrationCompletion` with the final registration digest
11. Core validates completion, emits `MountConfirmation` and only then may issue the child frame task

Drift before the CAS keeps the slot provisional/inert; drift or stale completion after the CAS keeps it registered but inert and transfers the residual into Task 6 `FailedPreparedBranch`; `settle_rejected_child_slot` executes only as a Core-sealed exact ledger obligation, never directly from the staging error branch, and no child task exists

- [ ] **Step 5: Verify GREEN and locked Node contract**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib mount::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib presentation::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib registry::tests::
```

Expected: exact staging, drift rejection and all existing Root physical tests pass

- [ ] **Step 6: Review and commit**

Review gate: registry-first is physically ordered, every observer-capable write is followed by exact validation, app-owned host is untouched

```powershell
git add -- crates/bevy_ui_router_ecs/src/mount.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/components.rs crates/bevy_ui_router_ecs/src/registry.rs crates/bevy_ui_router_ecs/src/presentation.rs crates/bevy_ui_router_ecs/src/content.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ecs): stage exact child outlet slots"
```

---

### Task 6: ECS branch transaction and sequential apply

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/branch_transaction.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/driver.rs`
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`

**Interfaces:**
- Consumes: Core actions and ECS mount staging
- Produces: root-to-leaf `RealizedBranch { frames, child_slots }` wrapped by `PreparedBranch`
- Produces: sequential parent report → mount resolution → child report → pending commit
- Preserves: one driver call per exact task and opaque report validation before first World/registry write

- [ ] **Step 1: Write branch transaction RED**

Add an internal two-level fixture and assert

```rust
assert_eq!(observed_views, vec![layout_view, leaf_view]);
assert_eq!(entry.pending_branch().frames().len(), 2);
assert_eq!(entry.pending_branch().child_slots().len(), 1);
assert!(entry.active_branch().is_empty());
```

Insert a schedule probe between parent apply and child apply to prove both parent frame and child slot remain hidden、gate-denied、`Pickable::IGNORE` and unfocused

- [ ] **Step 2: Write stale and failure RED**

Cover foreign/duplicate parent report, stale mount request, missing outlet, invalid anchor, grant drift and child report mismatch

Every case snapshots World and registry before the first unauthorized operation and requires zero change or sealed residual cleanup according to its phase

For missing/invalid outlet, rejected-after-mutation and child build/apply failure, assert the partial prepared branch moves to one `FailedPreparedBranch`, its exact manifest orders partial child frame → child slot → parent frames, the old active branch/history remain unchanged, and no terminal status appears before every receipt plus Core completion

- [ ] **Step 3: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib cleanup::tests::failed_prepared_branch_
```

Expected: fail because `RouterEntry` and `PendingCommit` hold one frame and drive blocks after parent realization

- [ ] **Step 4: Implement branch transaction orchestration**

```rust
struct RealizedBranch {
    frames: Vec<RealizedFrame>,
    child_slots: Vec<MountedChildSlot>,
}

struct PreparedBranch {
    realized: RealizedBranch,
}

impl RealizedBranch {
    fn frames(&self) -> &[RealizedFrame] { &self.frames }
    fn child_slots(&self) -> &[MountedChildSlot] { &self.child_slots }
}

impl PreparedBranch {
    fn frames(&self) -> &[RealizedFrame] { self.realized.frames() }
    fn child_slots(&self) -> &[MountedChildSlot] { self.realized.child_slots() }
    fn into_realized(self) -> RealizedBranch { self.realized }
}

struct PendingCommit {
    ticket: Option<CommitTicket>,
    branch: PreparedBranch,
}

struct FailedPreparedBranch {
    realized: RealizedBranch,
}

struct PendingFailureCleanup {
    request: FailureCleanupRequest,
    branch: FailedPreparedBranch,
    state: FailureCleanupState,
}

enum FailureCleanupState {
    ManifestPending,
    Prepared(CleanupLedger),
}

struct RouterEntry {
    handle: RouterHandle,
    engine: RouterEngine,
    views: RouteViewRegistry,
    host: Option<HostBinding>,
    host_settlement: Option<ProvisionalHostSettlement>,
    pending_commit: Option<PendingCommit>,
    active: Option<RealizedBranch>,
    cleanup_ledger: Option<CleanupLedger>,
    failure_cleanup: Option<PendingFailureCleanup>,
    cleanup_observation: RouterCleanupObservation,
    requests: Vec<NavigationRequestId>,
}
```

`active` is the only ECS active branch, `pending_commit.branch` is the only entering branch, `failure_cleanup.branch` is the only failed-entering owner and the cleanup ledger is the only post-publication leaving owner; each transition moves the whole `RealizedBranch` without cloning authority state and no second resource mirrors it

Drive one action at a time

1. validate opaque report identity before World/registry write
2. realize the frame against its exact `FrameMountBinding`
3. record realized outlets
4. report applied to Core
5. resolve the next `MountRequest` against the exact parent frame
6. stage the provisional child slot, accept its receipt, consume the registration permit, validate exact registered completion and only then obtain the child action
7. continue until Core emits one commit ticket

On any precommit failure

1. consume the partial `PreparedBranch` into `FailedPreparedBranch`
2. freeze exact Entity obligations from stored frame/output/child-slot claims in leaf-to-root order
3. pass the manifest digest/count and matching logical roots to Core `prepare_failure_cleanup`
4. execute only the sealed exact ledger while the failed branch stays inert
5. submit each ECS receipt and the completion digest/count to Core
6. remove `PendingFailureCleanup` only after Core publishes `CleanupSettled` and the terminal status

- [ ] **Step 5: Verify GREEN and Root driver compatibility**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib cleanup::tests::failed_prepared_branch_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::production_report_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys
```

Expected: nested internal flow and all existing Root journeys pass

- [ ] **Step 6: Review and commit**

Review gate: no child effect before registered-slot completion, no second publisher, stale identity rejected before first write

```powershell
git add -- crates/bevy_ui_router_ecs/src/branch_transaction.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/driver.rs crates/bevy_ui_router_ecs/src/snapshots.rs crates/bevy_ui_router_ecs/src/cleanup.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ecs): drive nested branch transactions"
```

---

### Task 7: Frame-local atomic commit, input, focus and cleanup

**Files:**
- Create: `crates/bevy_ui_router_ecs/src/frame_local.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/presentation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/input.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/registry.rs`

**Interfaces:**
- Consumes: ordered `PreparedBranch`, exact `FrameMountBinding` and Core commit/cleanup tickets
- Produces: frame-local traversal that stops at an exact child-slot boundary
- Produces: one in-place atomic publication plan with leaving leaf-to-root and entering root-to-leaf ordering
- Produces: branch cleanup ordering `child frame -> child slot -> parent frame`
- Preserves: cleanup never mutates or despawns the app-owned host

- [ ] **Step 1: Write frame-local physical traversal RED**

Create a two-level fixture with decorative descendants and one registered child slot, then assert the parent frame traversal includes the exact child-slot boundary but excludes every descendant owned by the child frame

```rust
assert_eq!(
    collect_frame_local_entities(&world, parent_frame, parent_root, &registry).unwrap(),
    vec![parent_root, parent_decoration, child_slot],
);
assert_eq!(
    collect_frame_local_entities(&world, child_frame, child_root, &registry).unwrap(),
    vec![child_root, child_button],
);
```

Add a corrupted relationship cycle fixture and require a closed error before any presentation, input or cleanup mutation

- [ ] **Step 2: Write atomic switch and focus RED**

Build a visible old branch and an inert prepared branch, then require one commit call to leave no intermediate externally visible state

```rust
assert_eq!(commit_trace, vec![
    CommitTrace::GateLeaving(child_old),
    CommitTrace::GateLeaving(parent_old),
    CommitTrace::PublishEntering(parent_new),
    CommitTrace::PublishEntering(child_new),
    CommitTrace::FocusCleared(old_focus),
]);
assert_branch_interactive(&world, &[parent_new, child_new]);
assert_branch_inert(&world, &[child_old, parent_old]);
assert_eq!(world.resource::<InputFocus>().get(), None);
```

Cover stale focus owner and focus inside a child slot; no leaving or lost-authority subtree may retain active focus and Nested introduces no automatic destination-focus policy

Install test probes immediately before and after `RouterScheduleSet::CommitPresentation`; each probe captures visibility, `Pickable`, `RouterInputGate`, `InputFocus`, registry active branch and public Core snapshot, then assert the pair is exactly old/old followed by new/new with no mixed branch observable in the same `App::update`

Add branch-changing and fragment-only nested pushes and assert both use full-branch replacement in this slice

```rust
assert_ne!(old.frames(), replacement.frames());
assert_ne!(old.child_slots(), replacement.child_slots());
assert_eq!(cleanup_receipts, old.branch_obligations_leaf_to_root());
assert_eq!(history.entries().last(), Some(&successful_location));
```

Failed replacement must leave the old active branch and history unchanged

- [ ] **Step 3: Write branch cleanup and protected-host RED**

```rust
assert_eq!(cleanup_trace, vec![
    CleanupTrace::Frame(child_frame),
    CleanupTrace::ChildSlot(child_slot),
    CleanupTrace::Frame(parent_frame),
]);
assert!(world.get_entity(app_host).is_ok());
assert_eq!(world.get::<AppHostMarker>(app_host), Some(&host_marker));
```

Add exact-claim drift at each cleanup unit and require quarantine or closed failure without crossing into a foreign entity, app host or sibling router

Add `nested_node_ancestry_runs_real_layout_and_picking_pipeline` using the locked `bevy_internal::DefaultPlugins`; reuse one `PrimaryWindow`, create one `Camera2d` and do not register `UiPickingPlugin` twice; assert every host → top slot → parent shell → content → outlet anchor → child slot → child shell edge is a real Node ancestry and effective visibility reaches both primitives after commit; raw `PointerHits` may contain backend candidates, but final hover/target events must reach only committed active content and never pending/leaving content

- [ ] **Step 4: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib frame_local::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_commit_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_replacement_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_fragment_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib cleanup::tests::nested_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib presentation::tests::nested_node_ancestry_runs_real_layout_and_picking_pipeline -- --exact
```

Expected: fail because physical traversal crosses the whole root subtree and cleanup/presentation still model one frame

- [ ] **Step 5: Implement stable frame-local traversal and atomic branch mutation**

```rust
pub(crate) enum FrameLocalError {
    MissingRoot(Entity),
    OwnershipDrift(Entity),
    RelationshipCycle(Entity),
    ForeignChildSlot(Entity),
}

pub(crate) fn collect_frame_local_entities(
    world: &World,
    frame: RouteFrameVersion,
    root: Entity,
    registry: &RouteEntityRegistry,
) -> Result<Vec<Entity>, FrameLocalError>;
```

Traversal sorts siblings by stable registry identity, includes the exact child-slot boundary in the parent frame and never descends through it

Build `CommitMutationPlan` entirely from immutable snapshots, validate every exact claim and relationship first, then apply only in-place presentation, input and focus changes after all validation succeeds

Build `FrozenCleanupManifest::from_leaving_branch` as ordered exact Entity units; this leaving cleanup starts only after successful publication and consumes child frames before their child slots and parent frames, while Task 6's distinct `from_failed_prepared_branch` remains the precommit-failure path

- [ ] **Step 6: Verify GREEN and native picking behavior**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib frame_local::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_commit_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_replacement_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::nested_fragment_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib cleanup::tests::nested_
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib input::tests::
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib presentation::tests::nested_node_ancestry_runs_real_layout_and_picking_pipeline -- --exact
```

Expected: frame-local traversal, old/old-or-new/new atomic branch publication, focus release, native picking isolation and ordered cleanup pass

- [ ] **Step 7: Review and commit**

Review gate: validation is complete before mutation, input/focus cannot cross a child boundary, successful-switch leaving cleanup is post-publication and the app-owned host is unchanged

```powershell
git add -- crates/bevy_ui_router_ecs/src/frame_local.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/presentation.rs crates/bevy_ui_router_ecs/src/input.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/registry.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ecs): commit nested branches frame locally"
```

---

### Task 8: Opaque outlet audit, public Journey A and Nested authority gates

**Files:**
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`
- Modify: `crates/bevy_ui_router_core/tests/public_api.rs`
- Modify: `crates/bevy_ui_router_core/tests/compile_fail_boundary.rs`
- Modify: `crates/bevy_ui_router_ecs/src/content.rs`
- Modify: `crates/bevy_ui_router_ecs/src/audit.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Modify: `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`
- Modify: `crates/bevy_ui_router/src/lib.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_journeys.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`
- Modify: `crates/bevy_ui_router/tests/root_navigation_surface.rs`
- Modify: `crates/bevy_ui_router/tests/facade_export_allowlist.rs`
- Modify: `crates/bevy_ui_router/tests/facade_authority_compile_fail.rs`
- Modify: `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`
- Test: `scripts/verify-authority-regressions.ps1`

**Interfaces:**
- Consumes: committed nested branch, exact child-slot bindings and frame-local audit traversal
- Produces: atomic public unlock of `RouteTreeBuilder::{route, route_with_options}` and `FrameContentWriter::declare_outlet` with the Nested facade profile switch
- Produces: opaque `AuthorityOutletMapping` tied by its snapshot to router/host and directly to parent frame, outlet key, anchor and optional child slot
- Produces: Nested stage gate with Root 19 plus exactly three Nested authority catalogue cases

- [ ] **Step 1: Write public facade surface RED**

Use only `bevy_ui_router::prelude::*` to declare `/items/1`, attach the layout's default outlet and submit a push; do not import owner crates or private integration types

```rust
let mut routes = RouteTree::builder()
    .root_with_options(
        RoutePattern::pathless(),
        RouteNodeOptions::new().with_view(layout_view.clone()),
    )?;
let layout_route = routes.root_id().unwrap();
routes.route_with_options(
    layout_route,
    RoutePattern::path("items/:id")?,
    RouteNodeOptions::new()
        .with_view(leaf_view.clone())
        .with_slot(RouteSlotKey::default()),
)?;
let routes = routes.build()?;

let anchor = content.spawn_empty()?;
content.declare_outlet(anchor, RouteSlotKey::default())?;

let request = app
    .world_mut()
    .resource_mut::<RouterNavigation>()
    .push(router, RouteLocation::new("/items/1"))?;
```

Expected compile failure: nested route/outlet facade methods are absent or private

- [ ] **Step 2: Write public Journey A RED**

Drive one public push to `/items/1` and assert the existing public snapshots, effect records and host fingerprint

```rust
let request_id = push_and_settle(
    &mut fixture.app,
    fixture.router,
    RouteLocation::new("/items/1"),
    None,
    None,
);
let request = request_snapshot(&fixture.app, fixture.router, request_id);
let router = router_snapshot(&fixture.app, fixture.router);
let effects = fixture.control.effects();

assert_eq!(
    effects.iter().map(|effect| effect.view.clone()).collect::<Vec<_>>(),
    vec![fixture.layout.clone(), fixture.leaf.clone()],
);
assert_eq!(router.active().frames().len(), 2);
assert_eq!(router.history().entries(), &[RouteLocation::new("/items/1")]);
assert_eq!(request.status(), &RequestStatus::Succeeded);
assert_host_intact(&fixture.app, &fixture.host_fingerprint);
```

From `request.trace()`, require the full profile subsequence with exact frame/outlet/slot accessors: parent `FrameTaskIssued` → `ShellPrepared` → `ViewApplied` → `OutletRegistered` → `MountRequested` → `MountResolved` → `MountGranted` → child `FrameTaskIssued` → `ShellPrepared` → `ViewApplied` → parent then child `FrameActivated` → `CommitPublished` → `RequestPhaseChanged(PostCommitCleanup)` → aggregate `CleanupSettled` → final `RequestTerminal`

The public Journey observes visibility/pickability at stable boundaries; Task 7's private schedule, gate and real-picking regressions prove pending/leaving pointer, keyboard, gamepad and focus isolation without unlocking the Authority-closure `RouterInputProbes` surface

Assert the driver effect order is layout then leaf, the child effect occurs only after outlet registration and mount grant, both active primitives are visible and active-pickable after commit, and the app-owned host components plus existing child identities and relative order are unchanged

Record the public `RouteViewInput` for both effects and assert the leaf receives location `/items/1`, route param `id = "1"`, no fragment and its matched-route origin; this proves the layered apply transports the real match rather than only invoking two view keys

- [ ] **Step 3: Write opaque authority audit RED**

```rust
assert_eq!(audit.frames().len(), 2);
assert_eq!(audit.slots().len(), 2);
assert_eq!(audit.outlets().len(), 1);
assert_eq!(audit.slots()[0].kind(), AuthoritySlotKind::HostTop);
assert_eq!(audit.slots()[1].kind(), AuthoritySlotKind::ChildOutlet);
assert_ne!(audit.frames()[0].root(), audit.frames()[1].root());
assert_eq!(audit.frames()[0].lifecycle(), AuthorityFrameLifecycle::Active);
assert_eq!(audit.frames()[1].lifecycle(), AuthorityFrameLifecycle::Active);
assert_eq!(audit.frames()[0].slot_entity(), audit.slots()[0].entity());
assert_eq!(audit.frames()[1].slot_entity(), audit.slots()[1].entity());
let outlet = &audit.outlets()[0];
assert_eq!(outlet.parent(), audit.frames()[0].frame());
assert_eq!(outlet.key(), &RouteSlotKey::default());
assert_eq!(outlet.child_slot(), Some(audit.slots()[1].entity()));
assert_eq!(
    audit.owned_entity_state(outlet.anchor()),
    AuditEntityState::PresentOwned,
);
assert!(audit.orphan_slots().is_empty());
```

Require the outlet DTO to expose opaque identities and lifecycle facts only; it must not expose writable Entity handles or authority constructors

- [ ] **Step 4: Register the exact Nested authority catalogue additions**

Keep the script-readable catalogue as one flat 22-string literal, appending only the three Nested names after the locked Root 19 entries

```rust
pub const AUTHORITY_REGRESSION_CATALOGUE: &[&str] = &[
    "cleanup_observer_reentry_supersedes_attempt",
    "cleanup_receipt_replay_is_rejected",
    "commit_gate_switch_is_same_frame_atomic",
    "foreign_component_quarantine_avoids_hooks",
    "foreign_descendant_quarantines_owned_ancestor",
    "host_children_change_only_top_slot_membership",
    "host_grant_observer_drift_rejects_after_mutation",
    "host_ui_topology_requires_all_node_ancestry",
    "pending_cycle_rejects_without_touching_active",
    "persistent_admission_observer_latches_retry",
    "plan_relationship_errors_are_world_zero",
    "post_commit_cleanup_drift_supersedes_attempt",
    "post_commit_cleanup_waits_for_receipt",
    "protected_host_is_never_despawned",
    "protected_parent_quarantine_preserves_relationship",
    "solid_rect_required_components_match_locked_bevy",
    "solid_rect_runs_real_layout_and_picking_pipeline",
    "stale_report_identity_is_zero_write",
    "structural_outputs_have_locked_manifest_and_topology",
    "mount_grant_observer_drift_rejects_after_mutation",
    "mount_token_replay_is_zero_write",
    "slot_lifecycle_has_no_orphans",
];
```

Update the catalogue target to parse the Root and Nested profile tables, validate their union against the final MVP vocabulary and require exact equality with the Rust constant/direct test set; keep `scripts/verify-authority-regressions.ps1` as the single binary-registration and actual-pass gate over the resulting 22-item constant

Switch the facade closed-world tests from the Root profile to `docs/nested-push-contract-profile.md`, require its exact 12 remaining exclusions, and prove the five unlocked APIs compile while History, Recovery and Authority-closure APIs remain rejected

- [ ] **Step 5: Verify RED**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys nested_push_journey_a -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit nested_outlet_authority_is_opaque_and_exact -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue
& .\scripts\verify-authority-regressions.ps1
```

Expected: facade/audit gaps and the three absent Nested regressions fail

- [ ] **Step 6: Implement the public path and final audit DTO**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityOutletMapping {
    parent: RouteFrameVersion,
    key: RouteSlotKey,
    anchor: AuditEntityId,
    child_slot: Option<AuditEntityId>,
}

impl AuthorityOutletMapping {
    pub fn parent(&self) -> &RouteFrameVersion { &self.parent }
    pub fn key(&self) -> &RouteSlotKey { &self.key }
    pub fn anchor(&self) -> AuditEntityId { self.anchor }
    pub fn child_slot(&self) -> Option<AuditEntityId> { self.child_slot }
}

impl RouterAuthorityAuditSnapshot {
    pub fn outlets(&self) -> &[AuthorityOutletMapping] { &self.outlets }
}
```

Change only the three frozen declaration methods from crate-private to public, remove their matching compile-fail exclusions, add their exact callable signature assertions, then switch the closed-world facade tests to the Nested profile in the same commit

Match the frozen source signature exactly, including `FrameContentWriter::declare_outlet(&mut self, anchor: ContentEntity, key: RouteSlotKey)` parameter names; do not merely change visibility on the current `entity/slot` spelling

Add `outlets: Vec<AuthorityOutletMapping>` to `RouterAuthorityAuditSnapshot` and populate it from exact registry snapshots only after verifying router, host epoch, parent frame, outlet key, anchor claim, child-slot claim and lifecycle seal; sort mappings by parent frame version then outlet key, keep the anchor ID stable while its exact Entity generation remains owned, and expose no Entity conversion or authority constructor

Implement the three direct `#[test]` regressions in `authority_regression_tests.rs`

1. `mount_grant_observer_drift_rejects_after_mutation` injects drift after a granted slot mutation, requires `RejectedAfterMutation`, a burned slot/seal ID, sealed provisional settlement and unchanged app host/old active branch
2. `mount_token_replay_is_zero_write` snapshots World and registry, replays stale/foreign/duplicate request, grant, receipt, registration permit, registration completion and final confirmation artifacts, then proves zero physical or registry change and no allocation by observing the next successful slot identity; stale permits must fail closed before provisional→registered CAS, and stale/foreign/duplicate permits or completions must never authorize a child action; Task 2 independently snapshots the private `CoreIdAllocator`
3. `slot_lifecycle_has_no_orphans` covers successful commit, every precommit failure class and terminal cleanup, requiring no provisional record, no orphan child slot and no unreceipted obligation at stable state

- [ ] **Step 7: Verify GREEN and Root compatibility**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test root_navigation_surface
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue
& .\scripts\verify-authority-regressions.ps1
```

Expected: public Journey A, opaque audit, negative facade contract, Root regressions and all 22 cumulative authority cases pass

- [ ] **Step 8: Review and commit**

Review gate: public callers cannot mint identity, bypass the driver, select active frames or mutate audit entities; the Root profile remains locked at 19

```powershell
git add -- crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/tests/public_api.rs crates/bevy_ui_router_core/tests/compile_fail_boundary.rs crates/bevy_ui_router_ecs/src/content.rs crates/bevy_ui_router_ecs/src/audit.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/src/lib.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/root_navigation_surface.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs crates/bevy_ui_router/tests/support/facade_contract_cases.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "test: prove nested push public authority"
```

---

### Task 9: Nested completion gate, factual review and documentation

**Files:**
- Modify: `docs/current-architecture.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/phase-3b1-architecture-review-log.md`
- Modify: `docs/superpowers/plans/2026-07-16-nested-push-implementation-plan.md`

**Interfaces:**
- Consumes: Tasks 1-8 commits and their fresh verification output
- Produces: factual Nested PASS record with exact commands, counts, commit range and reviewer outcomes
- Preserves: Journey B, common-prefix reuse, final nested fragment semantics, replace/back, Recovery and MVP remain explicitly incomplete

- [ ] **Step 1: Run focused Nested gates**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_view
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-features
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail
& .\scripts\verify-authority-regressions.ps1
```

Expected: every command exits 0, the catalogue target proves Root 19 plus Nested 3 and the authority script reports exactly 22 registered/passed cases with no missing or duplicate name

- [ ] **Step 2: Run the full workspace gate serially**

```powershell
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features -j 1 --config "build.rustc-wrapper = ''"
cargo nextest run --locked --workspace --all-features --build-jobs 1 --config "build.rustc-wrapper = ''"
cargo test --locked --workspace --all-features -j 1 --config "build.rustc-wrapper = ''" --doc
git diff --check
```

Expected: format, static checks, every workspace test, doctest and diff check exit 0; record actual counts instead of copying earlier Root counts

- [ ] **Step 3: Run four independent read-only reviews**

Dispatch separate reviewers for specification coverage, Core lifecycle protocol, ECS ownership/physical behavior and public facade/test evidence

Each reviewer reports `PASS` or findings classified `Critical`, `Important` or `Minor`, with exact file and line evidence; no reviewer stages, commits or edits files

Resolve all Critical and Important findings, rerun every affected focused gate and repeat the responsible review until PASS

- [ ] **Step 4: Update factual architecture and progress records**

Record only verified production behavior and fresh command evidence

```powershell
git rev-parse 1ccbcd7
git rev-parse HEAD
git log --oneline 1ccbcd7..HEAD
git status --short
git ls-files --error-unmatch docs/superpowers/plans/2026-07-16-nested-push-implementation-plan.md
```

Expected: the planning baseline, exact implementation range and tracked plan resolve successfully; only the intended documentation files may become dirty during this step

```markdown
- Nested Push: PASS
- Proven: real parent outlet, sealed mount, root-to-leaf apply, atomic full-branch publication, frame-local input/focus and exact cleanup
- Authority catalogue: Root 19 + Nested 3 = 22
- Still pending: Reconciliation Journey B, common-prefix preservation, replace/back, Recovery, Authority closure and final MVP gate
```

Mark this plan `Status: Complete` only after all focused gates, full gates and reviews pass

- [ ] **Step 5: Verify documentation and commit**

```powershell
git diff --check
rg -n "Nested Push|Journey B|common-prefix|Authority catalogue|MVP" docs/current-architecture.md docs/roadmap.md docs/phase-3b1-architecture-review-log.md docs/superpowers/plans/2026-07-16-nested-push-implementation-plan.md
git add -- docs/current-architecture.md docs/roadmap.md docs/phase-3b1-architecture-review-log.md docs/superpowers/plans/2026-07-16-nested-push-implementation-plan.md
git diff --cached --name-only
git diff --cached --check
git commit -m "docs: record nested push task pass"
git status --short
```

Expected: documentation consistently distinguishes Nested PASS from incomplete Reconciliation and MVP work, the final commit succeeds and the worktree is clean

---

## Plan self-review checklist

- [x] Every requirement in `docs/nested-push-contract-profile.md` maps to Tasks 1-9
- [x] Root Push fallback, depth-one fragment behavior and Root authority 19 remain locked regressions
- [x] Mount identity binds router, host epoch, parent frame, outlet key, transaction and single-use sequence
- [x] Core owns logical lifecycle only; ECS exclusively owns exact Entity manifests, ledgers, receipts and physical cleanup
- [x] Registry-first ordering and post-write exact validation cover every observer-capable staging write
- [x] Full-branch publication is atomic; successful leaving cleanup is post-publication, while precommit residual cleanup uses a distinct Core-sealed failure ledger before terminal
- [x] Frame-local traversal stops at child-slot boundaries and never reaches app-owned host or sibling routers
- [x] Nested fragment-only and branch-changing navigation use safe whole-branch replacement until Reconciliation
- [x] Journey B, common-prefix preservation, replace/back, Recovery, Authority closure and MVP are not claimed
- [x] Every task has a RED command, GREEN command, independent review gate and exact commit boundary
