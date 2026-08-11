# Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` to implement this plan task-by-task. Use `test-driven-development` before each behavior change, `systematic-debugging` for unexpected failures, `receiving-code-review` before applying review feedback, and `verification-before-completion` before every PASS or commit claim.

**Status:** Independently reviewed; ready for Task 1 canonical/profile activation. Runtime implementation has not started.

**Review:** PASS — Core/ECS/Public Critical 0, Important 0, Minor 0 on semantic snapshot `F5BEC9529B412A4038BC2F5AF5FDDD96427F6FA8FA1A70F694FD4A5E63E43916`.

**Baseline:** `9e4d34f`

**Spec:** `docs/superpowers/specs/2026-08-12-recovery-design.md`

**Goal:** Deliver boundary-bound error presentation, pre/post-commit authority settlement, stable unavailable and source-bound re-admission, and sealed stale-output cleanup without rolling back committed History or touching app-owned hosts. Close Recovery at six global active contract deltas and `27/27/27`, while leaving the actual old-lease reaper and full Journey G to Authority closure.

**Architecture:** R1 retains the failed physical branch but moves an independently frozen continuation into Core before cleanup. R2 transfers immutable normal ledger `L`, disjoint authority ledger `A`, and retired-host edge proof `H` into one compound owner. R3 binds a private non-`Clone` permit to Core availability source and exact ECS candidate revision; every hook-capable write is a `write -> flush -> full refreeze` micro-step.

**Tech Stack:** Rust 2024, Bevy ECS/UI, Markdown machine profiles, facade compile-contract harness, PowerShell authority verifier, Cargo/nextest.

## Global constraints

- Core owns request, presentation, history, availability, diagnostics, and terminal decisions; ECS owns exact Entity partitions, registry claims, allowed writes, ledgers, receipts, and host-edge mutation.
- Capabilities, leases, batches, tickets, and continuations move or consume once. Projections only validate; mismatch returns the whole owner.
- Register Router-owned entities before component/relationship writes. After every hook-capable write, flush and rebuild/refreeze target/protected closures; drift stops later writes.
- Publication is last and assignment-only. All allocation, sorting, reservation, owner composition, queries, and fallible validation finish first.
- Post-commit failure never rolls back presentation/history: the committed request stays `Succeeded` and receives the same severe anomaly diagnostic as the router.
- R2 never extends/copies `L`, creates a second normal batch, or models `H` as an Entity receipt. Settled `L` indices validate frozen terminal evidence only; unsettled indices are freshly classified.
- `H` must detach or prove `AlreadyAbsent`, flush, and refreeze before `A` attempts the old top-slot obligation.
- Re-admission binds Core availability incarnation/source to ECS registration incarnation/revision/topology/latch. Identity exhaustion is permanently ineligible; prior provisional cleanup must be terminal.
- A stale artifact without its unique sealed owner is lifecycle/World zero-write. With it, cleanup is limited to exact old output and cannot obtain a current continuation.
- Recovery excludes the background reaper, round-robin admission, multi-router closure, public quarantine APIs, and `RouterInputProbes`.
- Root alone runs Cargo/rustc and owns index/commits. One runtime writer at a time. Never touch untracked `review-history-*` files.

| Stage | Exclusions | History active | Recovery active / planned | Recovery authority active | Global active | Catalogue |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Activation | 10 | 3 | 0 / 3 | 0 | 3 | 24 |
| R1 | 8 | 3 | 1 / 2 | 1 | 4 | 25 |
| R2 | 8 | 3 | 2 / 1 | 2 | 5 | 26 |
| R3 / R4 | 8 | 3 | 3 / 0 | 3 | 6 | 27 |

## Preflight

- [ ] Confirm baseline, tracked cleanliness, spec tracking, and unique profile/journey headings.

```powershell
git rev-parse HEAD
git show --stat --oneline 9e4d34f
git status --short
git diff --check
git diff --cached --check
git ls-files --error-unmatch docs/superpowers/specs/2026-08-12-recovery-design.md
rg -n "^### 10\.[123] " docs/superpowers/specs/2026-08-12-recovery-design.md
rg -n "^### Journey (D|G)" docs/mvp-acceptance-journeys.md
```

Expected: tracked worktree/index clean, only known user WIP untracked, three profile headings, one Journey D and one Journey G heading.

---

### Task 1: Activate the Recovery canonical/profile contract without runtime semantics

**Files:** `docs/README.md`, `docs/superpowers/README.md`, `docs/target-architecture.md`, `docs/authority-transition-matrix.md`, `docs/mvp-acceptance-journeys.md`, `docs/roadmap.md`, `docs/superpowers/specs/2026-08-12-recovery-design.md`, `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`, `crates/bevy_ui_router/tests/facade_export_allowlist.rs`, `crates/bevy_ui_router/tests/facade_authority_compile_fail.rs`, `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`.

**Target:** exclusions `10`; History active deltas `3`; Recovery planned deltas `3`; Recovery active authority `0`; catalogue `24`.

- [ ] Add strict REDs: `recovery_profile_has_exact_activation_machine_state`, `active_stage_profile_preserves_history_union_at_recovery_activation`, `recovery_profile_keys_are_disjoint`, `recovery_profile_rejects_invalid_10_1_rows`, `recovery_profile_rejects_invalid_10_2_rows`, `recovery_profile_rejects_invalid_10_3_rows`, `recovery_profile_requires_exact_unique_section_tables`, `recovery_authority_parser_requires_exact_unique_first_table`, `recovery_authority_parser_rejects_invalid_rows`, and `recovery_planned_rows_do_not_enter_the_runtime_catalogue`.

Negative tables must reject duplicate/misordered headings, wrong first-table header/separator, empty/duplicate rows, unknown state/slice/item/test, noncanonical paths, missing/duplicate direct tests, `#[ignore]`, and extra `cfg/cfg_attr`.

- [ ] Fix the existing false-negative exclusion consumer before trusting it:

```rust
fn probe(options: RouteNodeOptions, boundary: ErrorBoundaryKey, view: RouteViewKey) {
    let _ = options.with_error_view(boundary, view);
}
```

It must fail because the two-argument method is private, never because of arity.

- [ ] Run structural REDs; each filter must execute nonzero tests.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist recovery_profile_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist active_stage_profile_preserves_history_union_at_recovery_activation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue recovery_ -j 1
```

Expected RED: no strict Recovery parser/composed stage profile exists.

- [ ] Implement support-only `parse_history_contract_profile`, `parse_recovery_contract_profile`, and `stage_contract_profile`. Use Recovery `10.1` as current exclusions, preserve all three active History deltas, append all Recovery rows in order, validate planned rows, expose only active authority rows, and reject cross-stage duplicate keys. Add no facade export.

- [ ] Synchronize canonical active-stage facts: replace matrix “merge/transfer existing ledger” language with `L+A+H`; record `A0` and every fallible binding before final-fallible `retire_current`; callback installs the ECS compound; admission is source-bound and confirmation-last; identity exhaustion is permanent. Register this spec/plan as the active pair, keep History completed, and leave actual old-lease reap/full Journey G to Authority closure. Do not claim Recovery/MVP PASS.

- [ ] Run GREEN gates.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-targets --all-features -j 1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-targets -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all exit `0`; verifier prints `authority regression verification passed: catalogue=24 registered=24 passed=24`.

- [ ] Obtain canonical/profile review and commit the exact Task 1 set. This plan is committed before execution and is absent from the activation commit.

```powershell
git add -- docs/README.md docs/superpowers/README.md docs/target-architecture.md docs/authority-transition-matrix.md docs/mvp-acceptance-journeys.md docs/roadmap.md docs/superpowers/specs/2026-08-12-recovery-design.md crates/bevy_ui_router/tests/support/facade_contract_cases.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "test: activate recovery stage profile"
```

Expected staged inventory is exactly the eleven listed paths and excludes this plan plus all user WIP.

---

### Task 2: Deliver R1 boundary-bound error presentation and mount recovery

**Files:** `crates/bevy_ui_router_core/src/route_tree.rs`, `crates/bevy_ui_router_core/src/branch.rs`, `crates/bevy_ui_router_core/src/matcher.rs`, `crates/bevy_ui_router_core/src/frame.rs`, `crates/bevy_ui_router_core/src/engine.rs`, `crates/bevy_ui_router_core/src/integration.rs`, `crates/bevy_ui_router_core/tests/public_api.rs`, `crates/bevy_ui_router_ecs/src/app_ext.rs`, `crates/bevy_ui_router_ecs/src/mount.rs`, `crates/bevy_ui_router_ecs/src/presentation.rs`, `crates/bevy_ui_router_ecs/src/cleanup.rs`, `crates/bevy_ui_router_ecs/src/registry.rs`, `crates/bevy_ui_router_ecs/src/navigation.rs`, `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`, `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`, `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`, `crates/bevy_ui_router/tests/public_navigation_journeys.rs`, `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`, `crates/bevy_ui_router/tests/facade_export_allowlist.rs`, `crates/bevy_ui_router/tests/facade_authority_compile_fail.rs`, `docs/facade-contract-fixture.md`, `docs/facade-associated-api-golden.txt`, `docs/mvp-acceptance-journeys.md`, `docs/superpowers/specs/2026-08-12-recovery-design.md`.

**Target:** exclusions `8`; global active deltas `4`; catalogue `25`.

- [ ] Add and run the crate-private Core RED `nested_apply_failure_selects_bound_error_view_before_router_fallback` through the existing internal tree builder before changing public visibility.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib nested_apply_failure_selects_bound_error_view_before_router_fallback -j 1
```

Expected RED: one test executes and observes global RouterFallback instead of the nearest bound view.

- [ ] Freeze the exact public signatures, but keep both methods private until the private Core implementation below is GREEN:

```rust
pub fn with_error_boundary(self, boundary: ErrorBoundaryKey) -> Self;
pub fn with_error_view(self, boundary: ErrorBoundaryKey, view: RouteViewKey) -> Self;
```

Tests: `nested_apply_failure_selects_bound_error_view_before_router_fallback`, `root_zero_residual_failure_uses_canonical_zero_failure_ledger`, `failure_cleanup_zero_terminal_consumes_typed_physical_before_publication`, `failure_cleanup_zero_bind_mismatch_retains_exact_evidence_and_redrives_zero`, `failure_cleanup_completion_receiver_mismatch_returns_whole_owner_and_calls_zero`, `recovery_attempt_failure_cleans_residual_before_recovery_failed_terminal`, `recovery_r1_journey_d_uses_bound_error_views_for_failure_matrix`, `recovery_r1_install_matrix_is_synchronous_and_world_zero`, `recovery_r1_journey_d_recovery_failure_cleans_all_attempts_and_queue_continues`, `recovery_r1_error_presentation_authority_maps_exact_frame`, `error_outlet_role_classification_is_preserved`, and `error_plan_failure_is_world_and_registry_zero_before_recovery`.

Also add `navigation::tests::failed_output_despawn_hook_protected_drift_stops_before_receipt_and_next_write`; its real despawn hook changes one protected relationship/component after the first target write, and it must prove no receipt for that write and no later mutation.

The public fixture uses distinct normal/error/fallback keys and production drivers for success, build/apply failure, missing/duplicate/foreign outlet, and relationship cycle. Test seams alter only real driver inputs.

The synchronous install matrix is exact: bound normal+error views succeed; boundary without error view is `MissingErrorView`; missing normal view takes `MissingRouteView` precedence; unknown normal/error key is `UnknownRouteView`; unregistered normal, error, or fallback driver is `UnknownViewDriver`; absent fallback is `MissingFallbackView`. Every error compares router count, host, request inventory, snapshot, and World before/after and proves no router/request/entity was created.

- [ ] Add the remaining owner/physical/public tests, but do not run public Journey D until the private implementation is GREEN and the methods have been exposed.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib failure_cleanup_zero_terminal_consumes_typed_physical_before_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib failure_cleanup_zero_bind_mismatch_retains_exact_evidence_and_redrives_zero -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::failed_output_despawn_hook_protected_drift_stops_before_receipt_and_next_write -- --exact
```

Expected owner/physical RED: Core cannot yet consume the zero physical owner before terminal publication; the hook drift still permits a receipt or later write. Zero executed tests is failure.

- [ ] Freeze nearest-boundary binding in matcher/tree as `(boundary_route, branch_index, ErrorBoundaryKey, RouteViewKey, compiled binding)` and add consuming Core ownership:

```rust
struct FailureCleanupRequest {
    receiver: FailureCleanupReceiverProjection,
    failure: NavigationFailure,
    continuation: FailureCleanupContinuationOwner,
}

struct FailureCleanupReceiverProjection {
    router: RouterIdentity,
    identity: FrameTaskIdentity,
    expected: PendingFailureKind,
    logical_roots: Vec<RouteFrameVersion>,
    recovery: FailureRecoveryTargetProjection,
}

enum FailureRecoveryTargetProjection {
    BoundError {
        boundary_route: RouteId,
        boundary: ErrorBoundaryKey,
        view: RouteViewKey,
        preserved_prefix_len: usize,
        binding_digest: u64,
    },
    RouterFallback { view: RouteViewKey },
    Terminal,
}

struct CleanupBatchProjection {
    identity: RootTaskIdentity,
    batch: CleanupBatchId,
    logical_roots_binding: u64,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
    binding: u64,
}

struct FailureCleanupPreparedProjection {
    receiver: FailureCleanupReceiverProjection,
    batch: CleanupBatchProjection,
    manifest_digest: CleanupManifestDigest,
}

struct FailureCleanupCompletionProjection {
    prepared: FailureCleanupPreparedProjection,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
}

struct FailureCleanupReceiverMismatchProjection {
    expected: FailureCleanupReceiverProjection,
    observed: Option<FailureCleanupReceiverProjection>,
}

struct FailureCleanupCompletionReceiverMismatchProjection {
    expected: FailureCleanupCompletionProjection,
    observed: Option<FailureCleanupCompletionProjection>,
}

struct FailureCleanupCompletionFactsProjection {
    batch: CleanupBatchProjection,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
}

struct FailureCleanupCompletionFactsMismatchProjection {
    expected: FailureCleanupCompletionFactsProjection,
    observed: FailureCleanupCompletionFactsProjection,
}

struct FailureCleanupManifestContradictionProjection {
    receiver: FailureCleanupReceiverProjection,
    expected_manifest_digest: CleanupManifestDigest,
    observed_manifest_digest: CleanupManifestDigest,
    expected_obligation_count: usize,
    observed_obligation_count: usize,
}

struct CoreFailureCleanupBatchOwner {
    token: CleanupBatchToken,
    receiver: FailureCleanupPreparedProjection,
}

enum FailureCleanupContinuationOwner {
    Recover(FrozenFailureRecoveryTarget),
    TerminalOriginal,
    TerminalRecovery(RecoveryFailureEvidence),
    AuthoritySettlement,
}

struct RecoveryFailureEvidence {
    source: FailureCleanupReceiverProjection,
    failed_attempt: FailureRecoveryAttemptKind,
    failure: NavigationFailure,
}

enum FrozenFailureRecoveryTarget {
    BoundError {
        boundary_route: RouteId,
        boundary: ErrorBoundaryKey,
        view: RouteViewKey,
        preserved_prefix_len: usize,
        compiled_binding: FrozenErrorBindingProjection,
    },
    RouterFallback { view: RouteViewKey },
}

struct FrozenErrorBindingProjection {
    boundary_route: RouteId,
    boundary: ErrorBoundaryKey,
    view: RouteViewKey,
    tree_revision: u64,
    binding_digest: u64,
}

struct FailureCleanupCompletionOwner {
    receiver: FailureCleanupCompletionProjection,
    continuation: FailureCleanupContinuationOwner,
    prepared: PreparedFailureContinuation,
}

enum PreparedFailureContinuation {
    RecoveryAction {
        attempt_kind: FailureRecoveryAttemptKind,
        action: PreparedRecoveryAction,
    },
    Terminal(PreparedFailureTerminal),
}

struct PreparedFailureTerminal {
    projection: FailureCleanupTerminalProjection,
    trace: Vec<NavigationTraceEvent>,
}

struct PreparedRecoveryAction {
    identity: RootTaskIdentity,
    target: FailureRecoveryTargetProjection,
    reserved_attempt: ReservedBranchAttempt,
    prepared_trace: Vec<NavigationTraceEvent>,
}

struct FailureContinuationEvidence {
    identity: RootTaskIdentity,
    attempt_kind: FailureRecoveryAttemptKind,
    next_phase: NavigationPhase,
}

struct FailureCleanupTerminalProjection {
    identity: RootTaskIdentity,
    status: RequestStatus,
    failure: NavigationFailure,
    trace_digest: u64,
}

enum FailureRecoveryAttemptKind {
    BoundError { boundary_route: RouteId, view: RouteViewKey },
    RouterFallback { view: RouteViewKey },
}

enum FailureCleanupPrepareOutcome {
    Prepared(CoreFailureCleanupBatchOwner),
    Mismatch {
        request: FailureCleanupRequest,
        evidence: FailureCleanupReceiverMismatchProjection,
    },
    Permanent {
        request: FailureCleanupRequest,
        evidence: FailureCleanupManifestContradictionProjection,
    },
}

struct FailureCleanupPermanentHolder<Physical> {
    request: FailureCleanupRequest,
    physical: Physical,
    evidence: FailureCleanupManifestContradictionProjection,
}

struct FailureCleanupZeroPhysicalProjection {
    receiver: FailureCleanupReceiverProjection,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
}

struct FailureCleanupZeroPhysicalMismatchProjection {
    source: FailureCleanupReceiverProjection,
    expected: FailureCleanupZeroPhysicalProjection,
    observed: FailureCleanupZeroPhysicalProjection,
}

struct PreparedFailureCleanupZeroPhysical<Physical> {
    physical: Physical,
    proof: FailureCleanupZeroPhysicalProjection,
}

struct FailureCleanupZeroTerminalOwner {
    receiver: FailureCleanupReceiverProjection,
    terminal: PreparedFailureTerminal,
}

enum FailureCleanupPermanentCoreOutcome<Physical, PreparedZero> {
    AuthoritySettlement {
        physical: Physical,
        owner: TargetAuthoritySettlementOwner,
    },
    ProvenZero {
        zero: PreparedZero,
        owner: FailureCleanupZeroTerminalOwner,
    },
    ZeroMismatch {
        holder: FailureCleanupPermanentHolder<Physical>,
        evidence: FailureCleanupZeroPhysicalMismatchProjection,
    },
    Mismatch(FailureCleanupPermanentHolder<Physical>),
}

enum FailureCleanupZeroTerminalOutcome<PreparedZero> {
    Terminal(FailureCleanupTerminalProjection),
    Mismatch {
        zero: PreparedZero,
        owner: FailureCleanupZeroTerminalOwner,
        evidence: FailureCleanupReceiverMismatchProjection,
    },
}

enum FailureCleanupCompletionPreparationOutcome {
    Prepared(FailureCleanupCompletionOwner),
    Mismatch {
        completion: CleanupCompletion,
        evidence: FailureCleanupCompletionReceiverMismatchProjection,
    },
    FactsMismatch {
        completion: CleanupCompletion,
        evidence: FailureCleanupCompletionFactsMismatchProjection,
    },
}

enum FailureCleanupCompletionCoreOutcome {
    Continued(FailureContinuationEvidence),
    Terminal(FailureCleanupTerminalProjection),
    Mismatch {
        owner: FailureCleanupCompletionOwner,
        evidence: FailureCleanupCompletionReceiverMismatchProjection,
    },
}

fn prepare_failure_cleanup_retaining(
    &mut self,
    request: FailureCleanupRequest,
    seal: CleanupManifestSeal,
) -> FailureCleanupPrepareOutcome;

fn prepare_failure_cleanup_completion(
    &mut self,
    completion: CleanupCompletion,
) -> FailureCleanupCompletionPreparationOutcome;

fn classify_failure_cleanup_permanent<Physical, PreparedZero, BindZero>(
    &mut self,
    holder: FailureCleanupPermanentHolder<Physical>,
    bind_zero: BindZero,
) -> FailureCleanupPermanentCoreOutcome<Physical, PreparedZero>
where
    BindZero: FnOnce(
        Physical,
        &FailureCleanupZeroPhysicalProjection,
    ) -> Result<
        PreparedZero,
        (Physical, FailureCleanupZeroPhysicalMismatchProjection),
    >;

fn complete_failure_cleanup_zero_terminal<PreparedZero, ConsumeZero>(
    &mut self,
    owner: FailureCleanupZeroTerminalOwner,
    zero: PreparedZero,
    consume_zero_and_clear_holder: ConsumeZero,
) -> FailureCleanupZeroTerminalOutcome<PreparedZero>
where
    ConsumeZero: FnOnce(PreparedZero);

fn complete_prepared_failure_cleanup_with_ledger<F>(
    &mut self,
    owner: FailureCleanupCompletionOwner,
    commit_ledger_and_clear_holder: F,
) -> FailureCleanupCompletionCoreOutcome
where
    F: FnOnce();
```

Core stages are `FailureCleanupManifestPending { expected: FailureCleanupReceiverProjection } -> FailureCleanupPrepared { expected: FailureCleanupPreparedProjection, continuation: FailureCleanupContinuationOwner } -> FailureCleanupCompleting { expected: FailureCleanupCompletionProjection }`. Before install, ECS guards `Option<PendingFailedBranch>`, then moves it apart into `request: FailureCleanupRequest` and `physical: FailedPreparedBranch`. Core mismatch returns the request so ECS can reassemble the original whole owner. Success installs only `PreparedFailureCleanupInstallation { physical, manifest, partition, reserved, core: CoreFailureCleanupBatchOwner }`; it must not retain or reconstruct the already-consumed request/continuation. The holder then transitions `Installed -> Completing { ledger, core_completion } -> Inert { ledger, returned_core_completion, evidence } | cleared`. `Permanent` is consuming and bounded: ECS immediately pairs the returned request, original physical owner, and contradiction in `FailureCleanupPermanentHolder`. Every nonzero residual moves into the existing authority-settlement owner. Canonical zero is accepted only through the sole production `bind_zero` callback, which consumes the exact physical owner and binds receiver + typed manifest digest + `obligation_count == 0` into a non-`Clone` prepared-zero owner. A zero-bind mismatch returns the same physical owner plus exact source/expected/observed projection; ECS reconstructs the whole permanent holder and stores that evidence in a stable inert state, with callback/terminal count zero on every later tick. The final terminal bridge exact-refreezes Core, invokes `consume_zero_and_clear_holder` once, and only then assignment-publishes `RecoveryFailed`; mismatch calls the callback zero times and returns both zero owner and terminal owner. No terminal outcome carries an unconsumed `Physical`. Foreign mismatch remains stable inert with a zero-redrive test. `FailureCleanupRequest`, batch/completion/zero owners, and continuation are non-`Clone`/non-`Copy`. `RouteFailureContinuationOwner` remains History-specific. Root zero-residual, nested, and fallback failure all enter one canonical ledger path. Post-install mismatch retains the Core stage/ECS ledger pair. Completion preparation freezes/reserves recovery action or terminal before callback; exact callback only clears the ECS holder; the tail only assigns prepared state.

- [ ] Implement ECS role partition and prepared publication around `navigation.rs::PendingFailedBranch`, `navigation.rs::settle_cleanup_system`, `cleanup.rs::CleanupLedger::settle`, and the exact retirement seam in `registry.rs`:

```rust
struct FrozenFailureCleanupPartition {
    source: FrozenFailureCleanupSource,
    targets: Vec<FrozenFailureCleanupTarget>,
    protected: Vec<FrozenProtectedEntity>,
    current_next: Vec<FrozenLifecycleTransition>,
    allowed_writes: Vec<FailureCleanupAllowedWrite>,
}
```

Targets are only failed suffix/residual obligations. Protected contains preserved active, non-target current, host/app-owned/foreign, and other authority/quarantine/stale closures. Freeze current and next independently; `next=None` remains validation-only. Plan failures are World/registry/mount zero-write; missing outlet emits `MountRequested -> MountRejected(MissingOutlet)`; ownership registration precedes each error-frame write; each write flushes/refreezes; suffix cleanup receipts finish before the one terminal event.

- [ ] Run the private Core test again and require GREEN, then prove the exact two-argument public consumer still fails only on visibility before exposure.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib nested_apply_failure_selects_bound_error_view_before_router_fallback -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
```

Expected: Core is GREEN; compile-fail passes with the corrected two-argument probe while both canonical exclusions remain.

- [ ] Expose the two runtime builders without changing the machine profile/exclusions, then run the public Journey D RED:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r1_journey_d_uses_bound_error_views_for_failure_matrix -- --exact
```

Expected: one test executes and fails on missing bound-error lifecycle behavior, not compilation. The transient candidate intentionally leaves the contract row planned and the compile-fail gate RED until post-review activation.

- [ ] Close the public contract: nearest boundary retains only frames strictly above its index and appends fresh `FrameOrigin::RouteError(boundary_route)`; match failure has `matched=None` and one RouterFallback frame; requested/matched/failure remain immutable; history/cursor do not advance; old active survives until error commit; Core/audit frames match; host manifest/children/sentinels are exact; recovery failure cleans residuals, returns `Failed(RecoveryFailed)`, and permits the next request; no lifecycle owner/orphan/inert root remains.

- [ ] Keep the R1 row planned and both exclusions unchanged; run semantic GREEN and closed History regressions before activation review:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib nested_apply_failure_selects_bound_error_view_before_router_fallback -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib root_zero_residual_failure_uses_canonical_zero_failure_ledger -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib failure_cleanup_zero_terminal_consumes_typed_physical_before_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib failure_cleanup_zero_bind_mismatch_retains_exact_evidence_and_redrives_zero -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib failure_cleanup_completion_receiver_mismatch_returns_whole_owner_and_calls_zero -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib recovery_attempt_failure_cleans_residual_before_recovery_failed_terminal -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r1_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit recovery_r1_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib error_outlet_role_classification_is_preserved -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib error_plan_failure_is_world_and_registry_zero_before_recovery -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::failed_output_despawn_hook_protected_drift_stops_before_receipt_and_next_write -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib nested_missing_outlet_rejects_before_slot_or_registry_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys root_push_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_ -j 1
cargo nextest run --locked -p bevy_ui_router_core --config "build.rustc-wrapper = ''" -j 1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-targets -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all listed semantic tests pass. The machine profile is deliberately still activation-state `10 / 24`; facade compile-fail/profile gates are deferred because the candidate public methods now exist.

- [ ] Obtain Core/ECS/public semantic review. Only after all three report no Critical/Important findings, activate exactly the R1 delta/authority row, remove exactly the two builder exclusions, and run the machine/profile gates:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
```

Expected: all pass; exclusions are exactly `8`; verifier prints `catalogue=25 registered=25 passed=25`.

- [ ] Commit the exact R1 set. `docs/facade-contract-fixture.md` and `docs/facade-associated-api-golden.txt` are verification-only because their frozen signatures already match.

```powershell
git add -- crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/src/branch.rs crates/bevy_ui_router_core/src/matcher.rs crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/tests/public_api.rs crates/bevy_ui_router_ecs/src/app_ext.rs crates/bevy_ui_router_ecs/src/mount.rs crates/bevy_ui_router_ecs/src/presentation.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/registry.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/tests/support/facade_contract_cases.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs docs/mvp-acceptance-journeys.md docs/superpowers/specs/2026-08-12-recovery-design.md
git diff --cached --name-only
git diff --cached --check
git commit -m "feat: add boundary-bound error recovery"
```

Expected staged inventory is exactly the twenty-two listed paths; fixture/golden and user WIP are absent.

---

### Task 3: Deliver R2 pre/post-commit authority settlement as `L + A + H`

**Files:** `crates/bevy_ui_router_core/src/ids.rs`, `crates/bevy_ui_router_core/src/diagnostics.rs`, `crates/bevy_ui_router_core/src/engine.rs`, `crates/bevy_ui_router_core/src/integration.rs`, `crates/bevy_ui_router_core/src/presentation.rs`, `crates/bevy_ui_router_ecs/src/cleanup.rs`, `crates/bevy_ui_router_ecs/src/navigation.rs`, `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`, `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`, `crates/bevy_ui_router/tests/public_navigation_journeys.rs`, `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`, `crates/bevy_ui_router/tests/facade_export_allowlist.rs`, `docs/facade-contract-fixture.md`, `docs/facade-associated-api-golden.txt`, `docs/mvp-acceptance-journeys.md`, `docs/superpowers/specs/2026-08-12-recovery-design.md`.

**Target:** global active deltas `5`; catalogue `26`.

- [ ] Add real scheduled REDs: `post_commit_host_loss_enters_single_compound_without_rolling_back_history`, `post_commit_host_loss_canonical_zero_l_transfers_disjoint_a_and_h`, `post_commit_host_loss_unattempted_original_ledger_preserves_batch_and_owner`, `post_commit_host_loss_partial_live_attempt_preserves_receipts_and_reclassifies_only_residual`, `post_commit_host_loss_physical_settled_core_pending_retains_leaving_owner`, `post_commit_host_loss_wrong_authority_batch_is_zero_write_and_returns_whole_compound`, `post_commit_host_loss_first_authority_write_hook_drift_stops_before_receipt_and_later_writes`, Core `post_commit_compound_completion_preparation_returns_whole_inputs_on_mismatch`, table-driven public `recovery_r2_characterization_precommit_host_loss_closes_foreground_owners`, `recovery_r2_journey_g_post_commit_registration_removal_preserves_committed_history`, `recovery_r2_journey_g_post_commit_host_despawn_preserves_committed_history`, audit `recovery_r2_authority_audit_closes_l_a_h_and_preserves_host_order`, and catalogue `post_commit_host_loss_preserves_committed_history`. The completion-preparation mismatch RED must separately drift L completion, A digest/count, and H expected/physical facts; each row must return `CompletionFacts { expected, observed }` for that exact field. It also inspects the ECS holder: the same `physical`, Core batch, and exact L/A/H inputs are retained in `CompletionPreparationInert`, its callback count is zero, and the next tick performs no World write or redrive.

The public R2 matrix has exactly five rows. Its three pre-commit rows are distinct production inputs, not only phase labels: first navigation pending plus host despawn; an existing active presentation plus a new request and host despawn; and an existing active presentation plus registration removal while the app host and sentinel remain. Inject them respectively at real `Apply`, `ReadyToCommit`, and `CommitSealed` boundaries and assert `HostLost`, no committed-history advance, no surviving request/foreground owner, exact host/sentinel preservation when the host remains, and no fabricated active presentation. The two post-commit rows are registration removal and host despawn after `CommitPresentation`; both retain the newly committed history and successful request while closing `L + A + H` before stable unavailable. Together these are the exact five Journey G Recovery rows.

Inject loss with `.after(RouterScheduleSet::CommitPresentation).before(RouterScheduleSet::SettleCleanup)`. Detect at the start of `settle_cleanup_system`, before normal ledger observe/attempt/complete; the ordinary authority observer has already missed this same-update window. Expected first RED: current code completes/stalls ordinary `PostCommitCleanup` and never installs a compound while history is already committed.

Run the first scheduled/physical REDs immediately after adding them:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::post_commit_host_loss_canonical_zero_l_transfers_disjoint_a_and_h -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::post_commit_host_loss_first_authority_write_hook_drift_stops_before_receipt_and_later_writes -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib post_commit_compound_completion_preparation_returns_whole_inputs_on_mismatch -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r2_characterization_precommit_host_loss_closes_foreground_owners -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r2_journey_g_post_commit_registration_removal_preserves_committed_history -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r2_journey_g_post_commit_host_despawn_preserves_committed_history -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit recovery_r2_authority_audit_closes_l_a_h_and_preserves_host_order -- --exact
```

Expected first failures: no compound holder exists and ordinary cleanup proceeds; the hook case permits a later write or receipt; every public test executes and observes rollback/stall/missing `L+A+H` terminal evidence. Zero executed tests is failure.

- [ ] Freeze immutable `L` and `A0`:

```rust
enum FrozenOriginalCleanupIndex {
    Unsettled { obligation: CleanupObligation, evidence: CleanupEvidence },
    Settled {
        entity: Entity,
        receipt: FrozenCleanupReceiptIdentity,
        terminal: FrozenCleanupTerminalProjection,
    },
}

struct FrozenOriginalCleanupLedgerTransfer {
    batch: CleanupBatchProjection,
    owner: OriginalCleanupPhysicalOwnerProjection,
    indices: Vec<FrozenOriginalCleanupIndex>,
    immutable_entities: Vec<Entity>,
    ledger_domain: u64,
    receipts: Vec<Option<FrozenCleanupReceiptIdentity>>,
    next_attempt: CleanupAttemptRevision,
    issued_attempt: Option<FrozenCleanupAttempt>,
    completion_digest: Option<u64>,
}

struct PreparedPostCommitAuthorityLedger {
    manifest: FrozenPostCommitAuthorityManifest,
    reserved: PreparedCleanupLedger,
    partition: FrozenPostCommitAuthorityPartition,
}

struct FrozenPostCommitAuthorityPartition {
    targets: Vec<FrozenPostCommitAuthorityTarget>,
    protected_non_target_current: Vec<FrozenProtectedEntity>,
    protected_preserved: Vec<FrozenProtectedEntity>,
    protected_app_foreign: Vec<FrozenProtectedEntity>,
    protected_other_holders: Vec<FrozenProtectedEntity>,
    original_entities: Vec<Entity>,
    union_coverage: Vec<Entity>,
    competing_same_source_holder: Option<FrozenCompetingHolder>,
}

struct FrozenPostCommitAuthorityTarget {
    entity: Entity,
    claim: ExactRegistryClaim,
    current: FrozenEntityAuthorityProjection,
    next: Option<FrozenEntityAuthorityProjection>,
    manifest: ComponentManifest,
    top_slot_dependency: Option<FrozenHostEdgeDependency>,
}

struct PreparedAuthorityLedgerBinding {
    prepared: PreparedPostCommitAuthorityLedger,
    batch: CleanupBatchProjection,
}

struct FrozenRetiredHostEdge {
    host: Entity,
    host_version: HostVersion,
    registration_incarnation: HostRegistrationIncarnation,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
    host_non_children_manifest: ComponentManifest,
    host_parent: Option<Entity>,
    host_ancestry: Vec<Entity>,
    children_before: Vec<Entity>,
    children_after: Vec<Entity>,
}
```

The `L` Entity set includes settled indices. Settled entries validate receipt identity plus frozen terminal only; unsettled entries are freshly classified. A live issued attempt is transferred with World zero writes: preserve accepted receipts, burn/supersede its old revision, and issue a strictly higher revision only for residual indices; never continue the old attempt. The partial-live-attempt RED must first fail if a settled index is re-observed/reissued or the next revision is not higher. Do not release the leaving owner until dedicated normal/compound Core completion. Prove `L ∩ A0 = ∅` by exact Entity-generation before validating each `A0` claim (including registry owner/seal/lifecycle), manifest, and independent current/next projection. `A0` covers committed active, old top-slot, and same-transaction old duties; target and every protected closure are pairwise disjoint; `L ∪ A0` exactly covers authorized old duties. Independent quarantine/stale/other-request closures stay protected. Unknown inbound relationship, cycle, or same-source competing holder is permanent before the first write. Implement around `CleanupLedger::{release_settled_owner,attempt,settle}` and use `settle_target_preparation_permanent` only as the transition-callback pattern, never its role proof.

- [ ] Add dedicated Core handoff rather than widening the precommit/generic paths:

```rust
struct CorePostCommitAuthorityTransferOwner {
    receiver: CorePostCommitCleanupReceiverProjection,
    cause: PostCommitAuthorityCause,
}

enum CorePostCommitCleanupStageProjection {
    PostCommitCleanup,
    AuthorityAccepted,
    AuthorityCleanup,
}

struct FrozenCommittedHistoryProjection {
    entries: Vec<RouteLocation>,
    cursor: Option<usize>,
    digest: u64,
}

struct FrozenActiveBranchProjection {
    identity: RootTaskIdentity,
    presentation: ActivePresentation,
    frames: Vec<RouteFrameVersion>,
}

struct CorePostCommitCleanupReceiverProjection {
    router: RouterIdentity,
    request: NavigationRequestId,
    identity: RootTaskIdentity,
    normal_batch: CleanupBatchProjection,
    current_host: HostVersion,
    current_top_slot: RouteSlotId,
    committed_history: FrozenCommittedHistoryProjection,
    active: FrozenActiveBranchProjection,
    stage: CorePostCommitCleanupStageProjection,
}

struct CorePostCommitCleanupObservedProjection {
    receiver: Option<CorePostCommitCleanupReceiverProjection>,
    retired_host: Option<RetiredHostAuthorityProjection>,
    competing_cleanup: Option<CoreCompetingCleanupOwnerProjection>,
}

enum CoreCompetingCleanupOwnerStage {
    Prepared,
    Active,
    Completing,
    Inert,
}

struct CoreCompetingCleanupOwnerProjection {
    identity: RootTaskIdentity,
    batch: CleanupBatchProjection,
    stage: CoreCompetingCleanupOwnerStage,
}

struct CorePostCommitCompoundProjection {
    receiver: CorePostCommitCleanupReceiverProjection,
    authority_batch: CleanupBatchProjection,
    retired_host: RetiredHostAuthorityProjection,
}

struct CorePostCommitReceiverMismatchProjection {
    expected: CorePostCommitCleanupReceiverProjection,
    observed: CorePostCommitCleanupObservedProjection,
}

struct CoreNormalCleanupCompletionFacts {
    batch: CleanupBatchProjection,
    manifest_digest: CleanupManifestDigest,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
}

struct CorePostCommitCompoundCompletionFactsProjection {
    normal: CoreNormalCleanupCompletionFacts,
    authority: CoreAuthorityCleanupCompletionFacts,
    host_edge: CoreRetiredHostEdgeCompletionFacts,
}

enum CorePostCommitCompoundMismatchEvidence {
    Receiver {
        expected: CorePostCommitCompoundProjection,
        observed: Option<CorePostCommitCompoundProjection>,
    },
    CompletionFacts {
        expected: CorePostCommitCompoundCompletionFactsProjection,
        observed: CorePostCommitCompoundCompletionFactsProjection,
    },
}

enum PostCommitAuthorityCause {
    HostRegistrationLost,
    HostEntityMissing,
    RequiredResourcesLost,
}

struct CoreRetiredHostAuthorityExpectedProjection {
    receiver: CorePostCommitCleanupReceiverProjection,
    retired_host: RetiredHostAuthorityProjection,
}

struct CoreRetiredHostAuthorityTerminalEvidence {
    expected: CoreRetiredHostAuthorityExpectedProjection,
    consumed: RetiredHostAuthorityProjection,
}

struct CoreRetiredHostEdgeSettlementOwner {
    authority: RetiredHostAuthority,
    expected: CoreRetiredHostAuthorityExpectedProjection,
}

struct CoreAuthorityCleanupCompletionFacts {
    batch: CleanupBatchProjection,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
}

struct CoreRetiredHostEdgeCompletionFacts {
    expected: CoreRetiredHostAuthorityExpectedProjection,
    physical_digest: u64,
}

struct CorePostCommitAuthorityBeginRetryEvidence {
    expected: CorePostCommitCleanupReceiverProjection,
    observed: CorePostCommitCleanupObservedProjection,
    missing_capacity: CorePostCommitCapacityKind,
}

struct CorePostCommitAuthorityBeginPermanentEvidence {
    expected: CorePostCommitCleanupReceiverProjection,
    observed: CorePostCommitCleanupObservedProjection,
    contradiction: CorePostCommitAuthorityContradiction,
}

enum CorePostCommitCapacityKind {
    CleanupBatchId,
    PreparedTrace,
    PreparedTerminal,
}

enum CorePostCommitAuthorityContradiction {
    ForeignReceiver,
    CompetingSameSourceOwner,
    RetiredHostMismatch,
    HistoryOrActiveMismatch,
}

struct CorePreparedPostCommitAuthorityTransfer {
    transfer: CorePostCommitAuthorityTransferOwner,
    host: HostLease,
    authority_batch: CleanupBatchToken,
    authority_manifest: CleanupManifestSeal,
}

struct CorePostCommitAuthorityPreparationSource {
    transfer: CorePostCommitAuthorityTransferOwner,
    host: HostLease,
}

struct CorePostCommitCompoundBatchOwner {
    receiver: CorePostCommitCompoundProjection,
    authority_batch: CleanupBatchToken,
    host_edge: CoreRetiredHostEdgeSettlementOwner,
}

struct CorePostCommitCompoundCompletionOwner {
    batch: CorePostCommitCompoundBatchOwner,
    normal: CleanupCompletion,
    authority: CoreAuthorityCleanupCompletionFacts,
    host_edge: CoreRetiredHostEdgeCompletionFacts,
}

struct PostCommitCompoundCompletionInputs {
    normal: CleanupCompletion,
    authority: CoreAuthorityCleanupCompletionFacts,
    host_edge: CoreRetiredHostEdgeCompletionFacts,
}

enum PostCommitCompoundCompletionPreparationOutcome {
    Prepared(CorePostCommitCompoundCompletionOwner),
    Mismatch {
        batch: CorePostCommitCompoundBatchOwner,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: CorePostCommitCompoundMismatchEvidence,
    },
}

struct PreparedPostCommitTerminalPlan {
    history_before: FrozenCommittedHistoryProjection,
    recovering_record: NavigationTraceEvent,
    aggregate_cleanup: NavigationTraceEvent,
    diagnostic: DiagnosticEvent,
    request_terminal: NavigationTraceEvent,
    queued_terminals: Vec<NavigationTraceEvent>,
    final_state: RouterAvailability,
}

struct CorePostCommitTerminalProjection {
    router: RouterIdentity,
    request: NavigationRequestId,
    history: FrozenCommittedHistoryProjection,
    availability: RouterAvailability,
    trace_digest: u64,
}

enum PostCommitAuthorityAcceptanceOutcome {
    Accepted(CorePostCommitAuthorityTransferOwner),
    Mismatch {
        expected: CorePostCommitCleanupReceiverProjection,
        observed: CorePostCommitCleanupObservedProjection,
    },
}

enum PostCommitAuthorityBeginCoreOutcome<Unbound, Bound, BindError> {
    Begun,
    BindingMismatch {
        owner: CorePreparedPostCommitAuthorityTransfer,
        unbound: Unbound,
        evidence: BindError,
    },
    ReceiverMismatch {
        owner: CorePreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: CorePostCommitReceiverMismatchProjection,
    },
    Retryable {
        owner: CorePreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: CorePostCommitAuthorityBeginRetryEvidence,
    },
    Permanent {
        owner: CorePreparedPostCommitAuthorityTransfer,
        bound: Bound,
        evidence: CorePostCommitAuthorityBeginPermanentEvidence,
    },
}

enum PostCommitCompoundCompletionCoreOutcome<PhysicalTerminal> {
    Terminal {
        terminal: CorePostCommitTerminalProjection,
        physical: PhysicalTerminal,
        retired_host: CoreRetiredHostAuthorityTerminalEvidence,
    },
    Mismatch {
        owner: CorePostCommitCompoundCompletionOwner,
        evidence: CorePostCommitCompoundMismatchEvidence,
    },
}

enum PostCommitAuthorityPreparationOutcome {
    Prepared {
        owner: CorePreparedPostCommitAuthorityTransfer,
        batch: CleanupBatchProjection,
    },
    Mismatch {
        source: CorePostCommitAuthorityPreparationSource,
        evidence: CorePostCommitReceiverMismatchProjection,
    },
    Retryable {
        source: CorePostCommitAuthorityPreparationSource,
        evidence: CorePostCommitAuthorityBeginRetryEvidence,
    },
    Permanent {
        source: CorePostCommitAuthorityPreparationSource,
        evidence: CorePostCommitAuthorityBeginPermanentEvidence,
    },
}

fn accept_post_commit_authority_transfer(
    &mut self,
    normal: &CleanupBatchToken,
    cause: PostCommitAuthorityCause,
) -> PostCommitAuthorityAcceptanceOutcome;

fn prepare_post_commit_authority_transfer(
    &mut self,
    source: CorePostCommitAuthorityPreparationSource,
    manifest: CleanupManifestSeal,
) -> PostCommitAuthorityPreparationOutcome;

fn bind_prevalidated_authority_ledger(
    prepared: PreparedPostCommitAuthorityLedger,
    batch: &CleanupBatchProjection,
) -> Result<PreparedAuthorityLedgerBinding, PreparedAuthorityLedgerBindingMismatch>;

fn begin_post_commit_authority_transfer<Unbound, Bound, BindError, Bind, Install>(
    &mut self,
    issuer: &mut HostAdmissionIssuer,
    owner: CorePreparedPostCommitAuthorityTransfer,
    unbound: Unbound,
    bind: Bind,
    install_compound: Install,
) -> PostCommitAuthorityBeginCoreOutcome<Unbound, Bound, BindError>
where
    Bind: FnOnce(
        Unbound,
        &CleanupBatchProjection,
    ) -> Result<Bound, (Unbound, BindError)>,
    Install: FnOnce(Bound, CorePostCommitCompoundBatchOwner);

fn complete_post_commit_authority_compound<PhysicalTerminal, F>(
    &mut self,
    owner: CorePostCommitCompoundCompletionOwner,
    clear_physical_holder: F,
) -> PostCommitCompoundCompletionCoreOutcome<PhysicalTerminal>
where F: FnOnce(CoreRetiredHostAuthorityExpectedProjection) -> PhysicalTerminal;

fn prepare_post_commit_compound_completion(
    &mut self,
    batch: CorePostCommitCompoundBatchOwner,
    inputs: PostCommitCompoundCompletionInputs,
) -> PostCommitCompoundCompletionPreparationOutcome;
```

Use `CoreIdAllocator::reserve_cleanup_batch() -> Result<CleanupBatchId, IdExhaustion>` and never `reserve_attempt`. Core preparation stores the actual new `CleanupBatchToken` in `CorePreparedPostCommitAuthorityTransfer` and returns only its borrowed projection for ECS binding. The exact order is accept receiver; ECS moves the exact `HostLease` out of `HostBinding`; Core preparation consumes it and the new token; final begin invokes the sole production `bind` closure with an immutable projection of that stored token. That closure consumes only the unbound ECS source and returns a bound installation or the original unbound source plus typed evidence. Core never depends on ECS types, and no `Unbound` value can reach the remaining receiver checks or `retire_current(owner.host)`, which is the final fallible operation. The install callback receives only `Bound` plus a `CorePostCommitCompoundBatchOwner` created by moving the stored token, never by projection reconstruction. Binding mismatch returns the token/lease-bearing Core owner and unchanged unbound source; every later failure returns the Core owner plus unchanged bound installation. At terminal, `prepare_post_commit_compound_completion` is the unique producer of the private completion owner: it consumes the batch plus exact `L/A/H` facts and returns all inputs whole on mismatch. Only its `Prepared` result may call the final completion bridge; completion mismatch calls the physical callback zero times and returns the whole owner.

- [ ] Store separate owners:

```rust
struct PostCommitCompoundOwner {
    core: CorePostCommitCompoundBatchOwner,
    physical: PostCommitCompoundPhysicalOwner,
}

struct PostCommitCompoundPhysicalOwner {
    original: CleanupLedger,
    authority: CleanupLedger,
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
}

struct PostCommitCompoundSourceProjection {
    receiver: CorePostCommitCleanupReceiverProjection,
    original_ledger_domain: u64,
    original_batch: CleanupBatchProjection,
    authority_batch: CleanupBatchProjection,
    host: Entity,
    host_version: HostVersion,
    registration_incarnation: HostRegistrationIncarnation,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
}

struct PreparedEcsPostCommitCompound<AuthorityState> {
    original: CleanupLedger,
    original_freeze: FrozenOriginalCleanupLedgerTransfer,
    authority: AuthorityState,
    active: RealizedBranch,
    host_parts: PreparedRetiredHostParts,
    edge: FrozenRetiredHostEdge,
    source: PostCommitCompoundSourceProjection,
}

type UnboundPreparedEcsPostCommitCompound =
    PreparedEcsPostCommitCompound<PreparedPostCommitAuthorityLedger>;
type BoundPreparedEcsPostCommitCompound =
    PreparedEcsPostCommitCompound<PreparedAuthorityLedgerBinding>;

struct PreparedRetiredHostParts {
    physical: HostBindingPhysicalOwner,
    candidate: ValidatedHostCandidate,
    top_slot_claim: ExactRegistryClaim,
    expected_retirement: RetiredHostProjection,
}

impl PreparedRetiredHostParts {
    fn restore(self, lease: HostLease) -> HostBinding;
    fn commit_retired(self) -> RetiredHostBinding;
}

enum PreparedPostCommitCompoundSource {
    Unbound(UnboundPreparedEcsPostCommitCompound),
    Bound(BoundPreparedEcsPostCommitCompound),
}

enum PostCommitCompoundHolder {
    PreparedInert {
        source: PreparedPostCommitCompoundSource,
        core: PreparedPostCommitCoreOwner,
        evidence: PostCommitCompoundPreparationEvidence,
    },
    Active(PostCommitCompoundOwner),
    CompletionPreparationInert {
        physical: PostCommitCompoundPhysicalOwner,
        batch: CorePostCommitCompoundBatchOwner,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: CorePostCommitCompoundMismatchEvidence,
    },
    Completing {
        physical: PostCommitCompoundPhysicalOwner,
        core_completion: CorePostCommitCompoundCompletionOwner,
    },
    Inert {
        physical: PostCommitCompoundPhysicalOwner,
        returned_core_completion: Option<CorePostCommitCompoundCompletionOwner>,
        evidence: PostCommitCompoundMismatchEvidence,
    },
}

enum PreparedPostCommitCoreOwner {
    Preparation(CorePostCommitAuthorityPreparationSource),
    Prepared(CorePreparedPostCommitAuthorityTransfer),
}

struct FrozenRetiredHostEdgeTerminal {
    edge: FrozenRetiredHostEdge,
    children_after: Vec<Entity>,
    terminal_digest: u64,
}

enum RetiredTopSlotDisposition {
    Removed { claim: ExactRegistryClaim },
    Quarantined { claim: ExactRegistryClaim },
    AlreadyAbsent { claim: ExactRegistryClaim },
}

enum HostEdgeSettlementProof {
    Detached(FrozenRetiredHostEdgeTerminal),
    AlreadyAbsent(FrozenRetiredHostEdgeTerminal),
}

struct RetiredHostPhysicalTerminalEvidence {
    old_host: Entity,
    old_host_version: HostVersion,
    old_registration_incarnation: HostRegistrationIncarnation,
    old_top_slot: Entity,
    old_top_slot_claim: ExactRegistryClaim,
    edge: HostEdgeSettlementProof,
    top_slot: RetiredTopSlotDisposition,
}

struct SettledRetiredHostEvidence {
    old_host: Entity,
    old_host_version: HostVersion,
    old_registration_incarnation: HostRegistrationIncarnation,
    old_top_slot: Entity,
    old_top_slot_claim: ExactRegistryClaim,
    edge: HostEdgeSettlementProof,
    top_slot: RetiredTopSlotDisposition,
}

fn combine_prevalidated_retired_host_terminal(
    physical: RetiredHostPhysicalTerminalEvidence,
    core: CoreRetiredHostAuthorityTerminalEvidence,
) -> SettledRetiredHostEvidence;
```

Core stages are `PostCommitCleanup -> PostCommitAuthorityAccepted { expected } -> PostCommitAuthorityCleanup { expected, retired_host, terminal_plan }`. The sole production bind bridge consumes `UnboundPreparedEcsPostCommitCompound` and can produce only `BoundPreparedEcsPostCommitCompound`; the generic Core begin API never receives a state-erased enum. The callback moves `CorePostCommitCompoundBatchOwner` into the ECS holder; it is returned only through the dedicated completion bridge and is never cloned or reconstructed. Pre-callback ECS failure stores the exact `Unbound` or `Bound` source plus the matching lease-bearing preparation/prepared Core owner in `PreparedInert`; an explicitly defined restore transition may instead consume both and reconstruct the original `HostBinding` whole source before storing inert. Completion preparation first destructures `PostCommitCompoundOwner` into `physical + batch`, then constructs the exact `PostCommitCompoundCompletionInputs` from L/A/H terminal facts. If `prepare_post_commit_compound_completion` mismatches, `CompletionPreparationInert` stores all three returned linear values plus typed evidence; it cannot fall back to `Active`, call the completion callback, or redrive physical writes. Only `Prepared` may move `physical` into `Completing` beside the newly created completion owner. Later completion mismatch stores returned Core completion beside physical owner; exact completion callback consumes physical once. The callback consumes `RetiredHostBinding + H + top-slot disposition` and returns `RetiredHostPhysicalTerminalEvidence`; Core consumes its private `RetiredHostAuthority` and returns `CoreRetiredHostAuthorityTerminalEvidence`. The assignment-only tail combines those non-authorizing values and stores `RouterEntry::settled_retired_host_evidence` for R3. No crate names both private owners and no stage holds the same Core batch twice. `H` authorizes only old-host to old-slot `ChildOf`, freezes host non-`Children` manifest/ancestry/other child order, and is not a receipt.

- [ ] Settle residual `L`, then `A` non-top leaf-to-root, then `H`, then `A` top-slot, then jointly consume all three. Every write uses prewrite refreeze, one write, flush, full refreeze. Despawn uses `despawn -> flush -> PhysicalAbsent refreeze -> exact registry retirement + receipt -> ReceiptCommitted refreeze`; hook drift cannot mint the current receipt or permit later writes.

- [ ] Freeze and publish the exact trace: `CommitPublished -> PostCommitCleanup -> prior L receipts -> Recovering -> remaining L -> A non-top -> H -> A top -> aggregate CleanupSettled -> severe diagnostic -> RequestTerminal`. `PreparedPostCommitTerminalPlan` stores one diagnostic identity used by both request/router projections, committed-history-before, aggregate, queue terminals, and final unavailable state. Exact compound callback clears physical holder; the assignment-only tail installs aggregate, diagnostic, and terminal in that order. Committed request stays `Succeeded`, queued foreground becomes `Failed(HostLost)`, active `None`, history unchanged, and availability `Unavailable(HostLost)`. Preserve app host/sentinels/other children/order; host despawn uses typed `AlreadyAbsent`; no duplicate `L` receipt or outstanding owner.

- [ ] Keep the R2 delta/catalogue row planned and run semantic GREEN plus closed History/R1 regressions:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r2_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit recovery_r2_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib post_commit_authority_transfer_receiver_mismatch_returns_whole_owner_and_calls_zero -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib post_commit_host_retirement_is_last_fallible_before_callback -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib post_commit_compound_completion_preparation_returns_whole_inputs_on_mismatch -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib post_commit_compound_completion_requires_exact_l_a_h_and_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib post_commit_host_loss_terminal_keeps_committed_history_and_succeeded_request -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib post_commit_host_loss_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib authority_regression_tests::post_commit_host_loss_preserves_committed_history -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib precommit_authority_takeover_preserves_history_until_settlement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib precommit_host_binding_loss_enters_authority_settlement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib authority_regression_tests::post_commit_cleanup_waits_for_receipt -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib authority_regression_tests::post_commit_cleanup_drift_supersedes_attempt -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_ -j 1
cargo nextest run --locked -p bevy_ui_router_core --config "build.rustc-wrapper = ''" -j 1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --all-targets -j 1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-targets -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all listed semantic tests pass while the machine profile/catalogue remains at R1 state `8 / 25`.

- [ ] Obtain Core publication/ECS ledger/public Journey G semantic review. Only after all three report no Critical/Important findings, activate exactly the R2 delta/catalogue row and run:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
```

Expected: all pass; verifier prints `catalogue=26 registered=26 passed=26`.

- [ ] Commit the exact R2 set.

```powershell
git add -- crates/bevy_ui_router_core/src/ids.rs crates/bevy_ui_router_core/src/diagnostics.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/src/presentation.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs docs/mvp-acceptance-journeys.md docs/superpowers/specs/2026-08-12-recovery-design.md
git diff --cached --name-only
git diff --cached --check
git commit -m "feat: add post-commit compound settlement"
```

Expected staged inventory is exactly the fourteen listed paths; fixture/golden, verification-only compile-fail files, and user WIP are absent.

---

### Task 4: Deliver R3 source-bound re-admission and sealed stale-output cleanup

**Files:** `crates/bevy_ui_router_core/src/admission.rs`, `crates/bevy_ui_router_core/src/presentation.rs`, `crates/bevy_ui_router_core/src/engine.rs`, `crates/bevy_ui_router_core/src/integration.rs`, `crates/bevy_ui_router_ecs/src/host.rs`, `crates/bevy_ui_router_ecs/src/cleanup.rs`, `crates/bevy_ui_router_ecs/src/navigation.rs`, `crates/bevy_ui_router_ecs/src/test_support.rs`, `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`, `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`, `crates/bevy_ui_router/tests/public_navigation_journeys.rs`, `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`, `crates/bevy_ui_router/tests/facade_export_allowlist.rs`, `docs/facade-contract-fixture.md`, `docs/facade-associated-api-golden.txt`, `docs/mvp-acceptance-journeys.md`, `docs/superpowers/specs/2026-08-12-recovery-design.md`.

**Target:** global active deltas `6`; catalogue `27`.

- [ ] Add real REDs: `host_loss_terminal_publishes_stable_unavailable_host_lost`, `host_admission_source_permit_reopens_only_changed_retryable_unavailable`, `host_admission_unavailable_cause_is_derived_from_exact_observation`, `host_unavailable_rejection_clears_issuer_before_publication`, `host_unavailable_issuer_failure_returns_typed_observed_evidence`, `host_admission_request_and_grant_errors_retain_candidate_owner`, `host_admission_request_sequence_exhaustion_is_permanent_and_exact`, `host_admission_grant_identity_exhaustion_is_permanent_and_exact`, `preallocation_grant_rejection_consumes_candidate_before_unavailable`, `host_confirmation_receiver_mismatch_returns_whole_owner_and_calls_zero`, `host_confirmation_preparation_mismatch_enters_provisional_settlement`, `host_confirmation_issuer_rejection_is_last_fallible_and_returns_whole_owner`, `host_confirmation_unwind_returns_grant_and_installation`, `host_confirmation_unwind_mismatch_retains_whole_owner_inert`, `host_confirmation_callback_installs_binding_before_ready_publication`, `candidate_revision_exhaustion_is_bounded_world_zero`, `staging_first_write_registration_drift_stops_before_node_and_host_edge`, `provisional_top_slot_settlement_terminal_precedes_unavailable_publication`, `invalid_host_topology_publishes_unavailable_without_staging`, `same_host_readmission_waits_for_retired_edge_and_old_slot_disposition`, `sealed_stale_output_cleanup_is_disjoint_from_new_epoch`, `ownerless_old_report_after_readmission_is_zero_write`, `recovery_r3_unavailable_reason_and_enqueue_mapping_are_source_bound`, `recovery_r3_journey_g_re_admission_uses_fresh_epoch_and_explicit_restore`, `recovery_r3_journey_g_same_host_readmission_detaches_old_top_slot_before_new_epoch`, `recovery_scenario_e_sealed_stale_output_cleanup_preserves_current_epoch`, `recovery_scenario_e_ownerless_old_artifact_is_public_snapshot_zero_write`, audit `recovery_r3_same_host_and_stale_epoch_authority_audit_is_exact`, and catalogue `host_reacceptance_detaches_old_top_slot`.

Public tests settle real loss, prove `HostUnavailable` creates no request, change a real registration revision, re-admit, and restore only after explicit replace. `recovery_r3_unavailable_reason_and_enqueue_mapping_are_source_bound` is an exact table for initial no-candidate (`AwaitingHost`, not unavailable), conflict, invalid non-`Node` ancestry, cyclic ancestry, and repaired topology on the same registration; it asserts the exact public reason, zero staging for invalid rows, no request on enqueue, and exactly one fresh revision/Ready transition after repair. `recovery_scenario_e_ownerless_old_artifact_is_public_snapshot_zero_write` replays an ownerless old artifact after new Ready and compares the complete facade-visible request/active/history/host snapshot before/after, allowing at most one report-disposition diagnostic and no cleanup/terminal. Scenario E sealed cleanup duplicates/delays only an opaque report artifact through the following doc-hidden ECS test-support control; unique physical authority is never cloned or exposed:

`provisional_top_slot_settlement_terminal_precedes_unavailable_publication` also serves as a structural owner gate: provisional cleanup remains batchless, no projection-to-`CleanupBatchToken`/`CleanupCompletion` reconstruction or `Clone` path exists, the physical completion-facts owner is unique from settled ledger through terminal callback, and both issuer failure and refreeze mismatch retain it whole with publication/callback count zero.

`host_admission_request_and_grant_errors_retain_candidate_owner` is table-driven over every production `AdmissionError`: it asserts the exact operation/disposition mapping above, exact expected/observed issuer and optional counter, and whole candidate retention. The ownerless `OutstandingExists` row must retain exact availability/Core-pending/issuer projections in `OperationEvidenceInert`, emit no terminal, and keep cause-bind/callback/publication/redrive counts zero. Malformed `Exhausted`/counter or non-exhaustion/counter pairs have the same zero-write inert boundary.

```rust
#[doc(hidden)]
pub struct TestingDelayedDriverReportReplay {
    router: RouterHandle,
    control: u64,
}

#[doc(hidden)]
pub enum TestingDelayedDriverReportReplayError {
    UnknownRouter,
    AlreadyArmed,
    NothingCaptured,
    ForeignControl,
}

#[doc(hidden)]
pub fn testing_arm_next_applied_driver_report_replay(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
) -> Result<TestingDelayedDriverReportReplay, TestingDelayedDriverReportReplayError>;

#[doc(hidden)]
pub fn testing_release_applied_driver_report_replay(
    navigation: &mut RouterNavigation,
    control: TestingDelayedDriverReportReplay,
) -> Result<(), TestingDelayedDriverReportReplayError>;
```

`bevy_ui_router_ecs::test_support` implements and exports the control under the existing `test-support` feature; the facade does not re-export it or add any symbol. In `public_navigation_journeys.rs`, the direct import, control helper, and both `recovery_scenario_e_*` tests are each guarded by `#[cfg(feature = "test-support")]`, matching the existing authority-audit test-support precedent and preserving the target's canonical `ecs`-only build. The control has private fields and never contains the artifact or owner. Arming changes only the real report-delivery timing: test support copies the next `AppliedDriverReportArtifact` into a private resource and delays that delivery while production retains the unique pending/task owner. After real host loss, fresh re-admission, and a new explicit replace, release moves the stored artifact through the normal applied-report consumer. The public test asserts the old sealed owner alone enters/finishes independent stale cleanup, current request/active/history/new host/registry/audit/queue remain exact, no fallback or current terminal is emitted, and a second/foreign control is rejected with zero writes. Short-circuiting stale classification or directly injecting a terminal/receipt must make the test fail.

- [ ] Replace panic/retry ambiguity with typed source authority:

```rust
enum HostCandidateRevisionObservation {
    Current(HostCandidateObservationSource),
    Fresh(HostCandidateObservationSource),
    RetryableCapacity(HostCandidateObservationSource),
    IdentityExhausted {
        source: HostCandidateObservationSource,
        attempted_counter: u64,
    },
}

enum HostCandidateObservationSource {
    None {
        router: RouterIdentity,
        revision: HostCandidateRevision,
        set_fingerprint: CandidateSetFingerprint,
    },
    Conflict {
        router: RouterIdentity,
        candidates: Vec<FrozenHostCandidateIdentity>,
        revision: HostCandidateRevision,
        set_fingerprint: CandidateSetFingerprint,
    },
    Unique {
        router: RouterIdentity,
        registration_incarnation: HostRegistrationIncarnation,
        revision: HostCandidateRevision,
        fingerprint: CandidateFingerprint,
        topology: HostCandidateTopologyFingerprint,
    },
}

enum HostAdmissionUnavailableEvidence {
    Candidate(HostCandidateRevisionObservation),
    Operation(HostAdmissionOperationFailureEvidence),
    Confirmation(HostAdmissionConfirmationFailureEvidence),
    AuthorityLoss(LifecyclePreflightEvidence),
    Preallocation(ProvisionalPreallocationRejection),
    PostAllocation(ProvisionalTopSlotSettlementEvidence),
}

enum HostAdmissionUnavailableEvidenceKind {
    NoCandidate,
    Conflict,
    InvalidTopology,
    HostLost,
    AuthorityEstablishmentFailed,
    RegistryInvariantViolation,
    IssuerOperationFailed,
    ConfirmationFailed,
    PreallocationRejected,
    PostAllocationRejected,
}

struct HostAdmissionUnavailableEvidenceProjection {
    source: HostAdmissionSourceProjection,
    kind: HostAdmissionUnavailableEvidenceKind,
    candidate_revision: Option<u64>,
    expected_digest: u64,
    observed_digest: u64,
    exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    operation_failure: Option<HostAdmissionOperationFailureProjection>,
}

struct PreparedHostAdmissionUnavailableCause<Evidence> {
    core: HostAdmissionUnavailableCause,
    evidence: Evidence,
}

enum HostAdmissionUnavailableCauseBindOutcome<Evidence> {
    Bound(PreparedHostAdmissionUnavailableCause<Evidence>),
    Mismatch {
        source: HostAdmissionSourceProjection,
        evidence: Evidence,
        projection: HostAdmissionUnavailableEvidenceProjection,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
}

fn bind_host_admission_unavailable_cause<Evidence>(
    source: HostAdmissionSourceProjection,
    projection: &HostAdmissionUnavailableEvidenceProjection,
    evidence: Evidence,
) -> HostAdmissionUnavailableCauseBindOutcome<Evidence>;

enum HostAdmissionSourceKind {
    InitialAwaiting,
    RetryableHostLost,
    RetryableHostConflict,
    RetryableInvalidHostUiTopology,
    RetryableAuthorityEstablishmentFailed,
    PermanentIdentityExhausted,
    PermanentRegistryInvariantViolation,
}

struct AvailabilityIncarnation(u64);

struct HostAdmissionSourcePermitKey {
    router: RouterIdentity,
    availability_incarnation: AvailabilityIncarnation,
    sequence: u64,
    binding: u64,
}

struct HostAdmissionSourceProjection {
    router: RouterIdentity,
    availability_incarnation: AvailabilityIncarnation,
    kind: HostAdmissionSourceKind,
    public_reason: Option<RouterUnavailableReason>,
    host: Option<HostVersion>,
    top_slot: Option<RouteSlotId>,
}

struct HostAdmissionSourcePermit {
    source: HostAdmissionSourceProjection,
    key: HostAdmissionSourcePermitKey,
}

struct HostAdmissionSourceIneligibleProjection {
    expected: HostAdmissionSourceProjection,
    observed: Option<HostAdmissionSourceProjection>,
}

struct HostGrantProjection {
    source: HostAdmissionSourceProjection,
    permit_key: HostAdmissionSourcePermitKey,
    host: HostVersion,
    top_slot: RouteSlotId,
}

struct SourceBoundHostAdmissionToken {
    permit: HostAdmissionSourcePermit,
    token: HostAdmissionToken,
}

struct SourceBoundHostGrant {
    source: SourceBoundHostAdmissionToken,
    grant: HostGrant,
}

enum CorePendingHostAdmissionStage {
    Permit,
    Token,
    Grant,
    Confirming,
}

struct CorePendingHostAdmissionProjection {
    source: HostAdmissionSourceProjection,
    stage: CorePendingHostAdmissionStage,
    permit_key: Option<HostAdmissionSourcePermitKey>,
    host: Option<HostVersion>,
    top_slot: Option<RouteSlotId>,
}

struct HostConfirmationReceiverProjection {
    source: HostAdmissionSourceProjection,
    grant: HostGrantProjection,
    pending: CorePendingHostAdmissionProjection,
}

struct HostConfirmationReceiverMismatchProjection {
    expected: HostConfirmationReceiverProjection,
    observed: Option<HostConfirmationReceiverProjection>,
}

struct HostIssuerGrantIdentityProjection {
    router: RouterIdentity,
    sequence: u64,
    host: HostVersion,
    top_slot: RouteSlotId,
}

enum HostAdmissionIssuerOutstandingProjection {
    Requested { router: RouterIdentity, sequence: u64 },
    Granted(HostIssuerGrantIdentityProjection),
}

struct HostAdmissionIssuerProjection {
    issuer_fingerprint: u64,
    next_sequence: u64,
    next_epoch: u64,
    outstanding: Option<HostAdmissionIssuerOutstandingProjection>,
    current: Option<HostIssuerGrantIdentityProjection>,
}

enum HostAdmissionOperation {
    Request,
    Grant,
}

enum HostAdmissionOperationFailureDisposition {
    PermanentAuthorityConflict,
    PermanentForeignOrStale,
    PermanentIdentityExhausted,
}

enum HostAdmissionOperationExhaustionExpectation {
    Forbidden,
    RequiredOneOf(Vec<HostAdmissionAllocatorCounter>),
}

enum HostAdmissionOperationFailureContradiction {
    OutstandingWithoutReturnedOwner {
        observed: HostAdmissionIssuerOutstandingProjection,
    },
    ExhaustionPairMismatch,
}

enum HostAdmissionAllocatorCounter {
    CandidateRevision,
    RequestSequence,
    GrantEpoch,
    HostVersion,
    TopSlot,
}

struct HostAdmissionAllocatorExhaustionEvidence {
    counter: HostAdmissionAllocatorCounter,
    current: u64,
    attempted: u64,
}

struct HostAdmissionOperationFailureProjection {
    source: HostAdmissionSourceProjection,
    operation: HostAdmissionOperation,
    error: AdmissionError,
    disposition: HostAdmissionOperationFailureDisposition,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
    exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
}

struct HostAdmissionOperationFailureEvidence {
    projection: HostAdmissionOperationFailureProjection,
}

struct HostAdmissionOperationFailureEvidenceMismatch {
    source: HostAdmissionSourceProjection,
    operation: HostAdmissionOperation,
    error: AdmissionError,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
    expected_exhaustion: HostAdmissionOperationExhaustionExpectation,
    observed_exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
    contradiction: HostAdmissionOperationFailureContradiction,
}

enum HostAdmissionOperationFailureFreezeOutcome {
    Frozen(HostAdmissionOperationFailureEvidence),
    Mismatch(HostAdmissionOperationFailureEvidenceMismatch),
}

enum HostAdmissionConfirmationFailureEvidence {
    Receiver(HostConfirmationReceiverMismatchProjection),
    Issuer(HostConfirmationIssuerMismatchProjection),
}

struct HostConfirmationIssuerMismatchProjection {
    receiver: HostConfirmationReceiverProjection,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
}

struct PreparedHostReadyPublication {
    source: HostAdmissionSourceProjection,
    grant: HostGrantProjection,
    availability: RouterAvailability,
    trace: Vec<HostTraceEvent>,
}

struct HostAdmissionReadyProjection {
    source: HostAdmissionSourceProjection,
    host: HostVersion,
    top_slot: RouteSlotId,
    availability: RouterAvailability,
}

struct PreparedHostAdmissionSource {
    permit: HostAdmissionSourcePermit,
    candidate: ValidatedHostCandidate,
    registration_incarnation: HostRegistrationIncarnation,
    revision: HostCandidateRevision,
    fingerprint: CandidateFingerprint,
    latch: PreparedHostCandidateLatch,
}

enum HostAdmissionSourcePermitOutcome {
    Issued(HostAdmissionSourcePermit),
    Ineligible {
        source: HostAdmissionSourceProjection,
        evidence: HostAdmissionSourceIneligibleProjection,
    },
    Outstanding { source: HostAdmissionSourceProjection },
}

enum CorePendingHostAdmission {
    Permit(CorePendingHostAdmissionProjection),
    Token(CorePendingHostAdmissionProjection),
    Grant(CorePendingHostAdmissionProjection),
    Confirming(HostConfirmationReceiverProjection),
}

enum HostAdmissionPermanence {
    RetryableAfterCandidateChange,
    PermanentForRouter,
}

enum HostAdmissionUnavailableDisposition {
    Awaiting,
    Unavailable {
        reason: RouterUnavailableReason,
        permanence: HostAdmissionPermanence,
    },
}

struct HostAdmissionUnavailableCause {
    source: HostAdmissionSourceProjection,
    evidence: HostAdmissionUnavailableEvidenceProjection,
    disposition: HostAdmissionUnavailableDisposition,
    cause_digest: u64,
}

struct HostAdmissionUnavailableTerminalProjection {
    source: HostAdmissionSourceProjection,
    cause_digest: u64,
    reason: RouterUnavailableReason,
    permanence: HostAdmissionPermanence,
}

struct HostAdmissionUnavailableMismatchProjection {
    expected_source: HostAdmissionSourceProjection,
    observed_pending: Option<CorePendingHostAdmissionProjection>,
}

enum HostAdmissionUnavailableIssuerOperation {
    CancelToken,
    RejectGrant,
}

struct HostAdmissionUnavailableIssuerMismatchProjection {
    receiver: HostAdmissionUnavailableMismatchProjection,
    operation: HostAdmissionUnavailableIssuerOperation,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
}

enum HostAdmissionUnavailableFailureEvidence {
    Receiver(HostAdmissionUnavailableMismatchProjection),
    Issuer(HostAdmissionUnavailableIssuerMismatchProjection),
}

enum HostAdmissionUnavailablePreStagingOwner {
    Permit(HostAdmissionSourcePermit),
    Token(SourceBoundHostAdmissionToken),
}

struct HostAdmissionUnavailablePreStagingSource<Installation, Evidence> {
    prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    owner: HostAdmissionUnavailablePreStagingOwner,
    installation: Installation,
}

struct HostAdmissionUnavailableGrantedSource<Installation, Evidence> {
    prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    owner: CoreHostConfirmationSource<Installation>,
}

enum HostAdmissionUnavailablePreStagingOutcome<Installation, Evidence, PhysicalTerminal> {
    Published {
        terminal: HostAdmissionUnavailableTerminalProjection,
        physical: PhysicalTerminal,
    },
    Mismatch {
        source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

enum HostAdmissionUnavailableGrantedOutcome<Installation, Evidence, PhysicalTerminal> {
    Published {
        terminal: HostAdmissionUnavailableTerminalProjection,
        physical: PhysicalTerminal,
    },
    Mismatch {
        source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

enum HostAdmissionUnavailableObservedOutcome<Evidence> {
    Published(HostAdmissionUnavailableTerminalProjection),
    Awaiting {
        source: HostAdmissionSourceProjection,
        evidence: Evidence,
    },
    Mismatch {
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        evidence: HostAdmissionUnavailableFailureEvidence,
    },
}

fn complete_pre_staging_host_admission_unavailable<Installation, Evidence, PhysicalTerminal, Consume>(
    &mut self,
    issuer: &mut HostAdmissionIssuer,
    source: HostAdmissionUnavailablePreStagingSource<Installation, Evidence>,
    consume_installation_and_clear_holder: Consume,
) -> HostAdmissionUnavailablePreStagingOutcome<Installation, Evidence, PhysicalTerminal>
where
    Consume: FnOnce(Installation) -> PhysicalTerminal;

fn complete_granted_host_admission_unavailable<Installation, Evidence, PhysicalTerminal, Consume>(
    &mut self,
    issuer: &mut HostAdmissionIssuer,
    source: HostAdmissionUnavailableGrantedSource<Installation, Evidence>,
    consume_physical_terminal: Consume,
) -> HostAdmissionUnavailableGrantedOutcome<Installation, Evidence, PhysicalTerminal>
where
    Consume: FnOnce(Installation) -> PhysicalTerminal;

fn complete_observed_host_admission_unavailable<Evidence, Clear>(
    &mut self,
    prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
    clear_holder: Clear,
) -> HostAdmissionUnavailableObservedOutcome<Evidence>
where
    Clear: FnOnce();
```

ECS derives a non-authorizing `HostAdmissionUnavailableEvidenceProjection` from the whole typed `HostAdmissionUnavailableEvidence`, then moves the original evidence through the generic `Evidence` parameter. `bind_host_admission_unavailable_cause` is the only Core cause producer: it exact-matches the projection to the Core source and either returns the original evidence on mismatch or seals `core cause + original evidence` in the private non-`Clone` `PreparedHostAdmissionUnavailableCause`. Pre-staging, observed, and granted sources carry that wrapper until publication consumes it; `Awaiting` and every mismatch return the evidence whole. No caller can choose reason or permanence independently. The only permit producer is `issue_host_admission_source_permit(&mut self) -> HostAdmissionSourcePermitOutcome`; every `CorePendingHostAdmission` variant stores the complete stage projection, including permit key and grant host/top-slot when applicable. The three `complete_*_host_admission_unavailable` functions are the only unavailable publishers. They derive reason and permanence from the bound cause. Observed `InitialAwaiting + NoCandidate` returns `Awaiting` without publication, while a retryable-unavailable source with no candidate preserves its existing typed unavailable reason. Identity exhaustion consumes the whole `Candidate(IdentityExhausted { source, attempted_counter })` through the observed path and is permanently `AuthorityEstablishmentFailed`; it never degrades to `HostAdmissionSourcePermitOutcome::Ineligible`. Registry invariant violation is permanent for the router. Conflict/invalid topology/host loss are retryable only after an exact candidate-source change.

`freeze_host_admission_operation_failure` is the sole producer of operation evidence and derives, rather than accepts, disposition. `OutstandingExists` without the exact existing outstanding owner always returns `Mismatch(OutstandingWithoutReturnedOwner { observed })`; the returned Permit/Token and outside candidate enter `OperationEvidenceInert`, while availability, Core pending projection, issuer projection, and public trace remain exact and no terminal is emitted. This Recovery plan has no API that returns that old owner, so this branch never publishes unavailable; a future owner-returning path may publish only after consuming the exact outstanding identity. Normal frozen mappings are `AuthorityExists -> PermanentAuthorityConflict`, `ForeignRouter | Stale -> PermanentForeignOrStale`, and valid `Exhausted -> PermanentIdentityExhausted`. The exhaustion matrix is exact: Request accepts only `RequestSequence`; Grant accepts only `GrantEpoch | HostVersion | TopSlot`; `CandidateRevision` exists only on the observed candidate path; every non-`Exhausted` error requires no exhaustion payload. Invalid error/counter pairs return `Mismatch(ExhaustionPairMismatch)` with expected/observed exhaustion; `AdmissionOwnerError` returns that outcome beside the whole Permit/Token, ECS pairs it with the outside candidate in `OperationEvidenceInert`, and no cause binding, retry, callback, publication, terminal, or redrive occurs. Only `Frozen` may become `HostAdmissionUnavailableEvidence::Operation`. The bound cause exact-matches operation, original `AdmissionError`, disposition, expected/observed issuer, and optional counter before deriving public reason/permanence; operation failures never map to `RetryableAfterCandidateChange`.

Pre-staging and granted owner paths exact-match Core pending state and the issuer projection before mutation. Receiver drift returns `HostAdmissionUnavailableFailureEvidence::Receiver`; token cancellation or grant rejection is the final fallible issuer operation, and its failure returns the whole prepared cause+owner plus `Issuer { operation, expected_issuer, observed_issuer }` while calling no callback. A `Confirming` owner must first pass the unwind bridge below and become `CoreHostConfirmationSource<Installation>`. If a provisional Entity exists, ECS retains its settlement owner until terminal, then passes that terminal installation to the granted publisher; only after issuer rejection succeeds does the infallible callback consume/clear it, followed by assignment-only unavailable publication. No public unavailable state appears before physical cleanup. No helper may accept revision alone or inject a classification. Test exhaustion through the real counter boundary.

- [ ] Make issue/grant/confirm owner-returning and confirmation-last:

```rust
struct CorePreparedHostConfirmation<Installation> {
    grant: SourceBoundHostGrant,
    installation: Installation,
    receiver: HostConfirmationReceiverProjection,
    ready: PreparedHostReadyPublication,
}

struct CoreHostConfirmationSource<Installation> {
    grant: SourceBoundHostGrant,
    installation: Installation,
}

struct RejectedHostConfirmationPreparation<Installation> {
    grant: SourceBoundHostGrant,
    installation: Installation,
    evidence: HostConfirmationReceiverMismatchProjection,
}

impl<Installation> RejectedHostConfirmationPreparation<Installation> {
    fn into_parts(
        self,
    ) -> (
        SourceBoundHostGrant,
        Installation,
        HostConfirmationReceiverMismatchProjection,
    );
}

fn bind_granted_host_admission_source<Installation>(
    grant: SourceBoundHostGrant,
    installation: Installation,
) -> CoreHostConfirmationSource<Installation>;

fn consume_host_confirmation_preparation_mismatch<Installation>(
    owner: CoreHostConfirmationSource<Installation>,
    evidence: HostConfirmationReceiverMismatchProjection,
) -> RejectedHostConfirmationPreparation<Installation>;

enum HostAdmissionConfirmationCoreOutcome<Installation, Installed> {
    Ready {
        projection: HostAdmissionReadyProjection,
        installed: Installed,
    },
    ReceiverMismatch {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
    IssuerRejected {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationIssuerMismatchProjection,
    },
}

enum HostConfirmationUnwindOutcome<Installation> {
    Grant(CoreHostConfirmationSource<Installation>),
    Mismatch {
        owner: CorePreparedHostConfirmation<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
}

enum HostConfirmationPreparationOutcome<Installation> {
    Prepared(CorePreparedHostConfirmation<Installation>),
    ReceiverMismatch {
        owner: CoreHostConfirmationSource<Installation>,
        evidence: HostConfirmationReceiverMismatchProjection,
    },
}

struct AdmissionOwnerError<Owner> {
    owner: Owner,
    evidence: HostAdmissionOperationFailureFreezeOutcome,
}

fn freeze_host_admission_operation_failure(
    source: HostAdmissionSourceProjection,
    operation: HostAdmissionOperation,
    error: AdmissionError,
    expected_issuer: HostAdmissionIssuerProjection,
    observed_issuer: HostAdmissionIssuerProjection,
    exhaustion: Option<HostAdmissionAllocatorExhaustionEvidence>,
) -> HostAdmissionOperationFailureFreezeOutcome;

fn request_host_admission(
    &mut self,
    issuer: &mut HostAdmissionIssuer,
    permit: HostAdmissionSourcePermit,
) -> Result<SourceBoundHostAdmissionToken, AdmissionOwnerError<HostAdmissionSourcePermit>>;

fn grant_host_admission(
    &mut self,
    issuer: &mut HostAdmissionIssuer,
    token: SourceBoundHostAdmissionToken,
) -> Result<SourceBoundHostGrant, AdmissionOwnerError<SourceBoundHostAdmissionToken>>;

fn prepare_host_confirmation<Installation>(
    &mut self,
    owner: CoreHostConfirmationSource<Installation>,
) -> HostConfirmationPreparationOutcome<Installation>;

fn unwind_host_confirmation<Installation>(
    &mut self,
    owner: CorePreparedHostConfirmation<Installation>,
) -> HostConfirmationUnwindOutcome<Installation>;

fn confirm_host_admission_with_binding<Installation, Installed, F>(
    &mut self,
    issuer: &mut HostAdmissionIssuer,
    owner: CorePreparedHostConfirmation<Installation>,
    install_binding: F,
) -> HostAdmissionConfirmationCoreOutcome<Installation, Installed>
where F: FnOnce(Installation, HostLease) -> Installed;
```

`SourceBoundHostAdmissionToken` owns the consumed permit beside the private issuer token; `SourceBoundHostGrant` owns that whole source beside the private issuer grant. The underlying admission module does not depend on engine/ECS source types, and no sibling permit exists after issue. Remove token/grant `Clone/Copy` and confirmation `expect`. Confirmation source/prepared owner contains only the source-bound grant plus installation. A preparation mismatch leaves Core in `Grant` and returns `CoreHostConfirmationSource` directly. Receiver/issuer failure after preparation returns the complete confirming owner; `unwind_host_confirmation` exact-matches it, assignment-restores Core to `Grant`, and returns `grant + installation` for physical settlement and the granted unavailable consumer. Prepare Core Ready records, trace capacity, and ECS binding first; issuer confirmation is final fallible. On success the callback consumes installation plus `HostLease`, installs `HostBinding`, and returns non-authorizing installed evidence in `Ready`; only then does Core assignment-publish `ReadyNoActive`. No success or failure drops `Installation`.

- [ ] Make `StagedProvisionalTopSlot` non-`Clone` and include Entity/claim/owner/manifest/candidate plus `FrozenProvisionalStagingPartition { slot_target, allowed_writes, candidate_host_non_children_manifest, candidate_host_parent, candidate_host_ancestry, candidate_other_children, protected_foreign, protected_other_holders }`.

```rust
struct ProvisionalTopSlotPhysicalOwner {
    staged: StagedProvisionalTopSlot,
    candidate: PreparedEcsHostCandidateSource,
    partition: FrozenProvisionalStagingPartition,
}

enum ProvisionalTopSlotSettlementStage {
    OwnershipRegistered,
    BundleWritten,
    NodeWritten,
    HostEdgeWritten,
    ProbeWritten,
}

struct ProvisionalTopSlotSettlementEvidence {
    source: HostCandidateObservationSource,
    stage: ProvisionalTopSlotSettlementStage,
    expected_candidate_digest: u64,
    observed_candidate_digest: u64,
}

struct ProvisionalStageSource {
    grant: SourceBoundHostGrant,
    candidate: PreparedEcsHostCandidateSource,
}

struct ProvisionalPreallocationRejection {
    source: HostCandidateObservationSource,
    expected_candidate_digest: u64,
    observed_candidate_digest: u64,
}

struct PreparedEcsHostCandidateSource {
    candidate: ValidatedHostCandidate,
    registration_incarnation: HostRegistrationIncarnation,
    revision: HostCandidateRevision,
    fingerprint: CandidateFingerprint,
    latch: PreparedHostCandidateLatch,
}

struct PreparedProvisionalHostInstallation {
    staged: StagedProvisionalTopSlot,
    candidate: PreparedEcsHostCandidateSource,
    partition: FrozenProvisionalStagingPartition,
}

impl PreparedProvisionalHostInstallation {
    fn into_physical(self) -> ProvisionalTopSlotPhysicalOwner;
}

struct ProvisionalTopSlotPhysicalCompletionFacts {
    grant: HostGrantProjection,
    source: HostAdmissionUnavailableEvidenceProjection,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
    settled_count: usize,
    receipt_digest: u64,
    terminal_digest: u64,
}

struct ProvisionalTopSlotTerminalInstallation {
    physical: ProvisionalTopSlotPhysicalOwner,
    facts: ProvisionalTopSlotPhysicalCompletionFacts,
}

struct PreparedEcsHostCandidateTerminalEvidence {
    registration_incarnation: HostRegistrationIncarnation,
    revision: HostCandidateRevision,
    fingerprint: CandidateFingerprint,
    latch_digest: u64,
}

struct ProvisionalTopSlotPhysicalTerminalEvidence {
    grant: HostGrantProjection,
    source: HostAdmissionUnavailableEvidenceProjection,
    terminal_digest: u64,
}

enum HostAdmissionUnavailableGrantedInstallation {
    Preallocation(PreparedEcsHostCandidateSource),
    Provisional(ProvisionalTopSlotTerminalInstallation),
}

enum HostAdmissionUnavailablePhysicalTerminal {
    Preallocation(PreparedEcsHostCandidateTerminalEvidence),
    Provisional(ProvisionalTopSlotPhysicalTerminalEvidence),
}

struct ProvisionalTopSlotTerminalMismatchProjection {
    expected_grant: HostGrantProjection,
    observed_grant: HostGrantProjection,
    expected_source: HostAdmissionUnavailableEvidenceProjection,
    observed_source: HostAdmissionUnavailableEvidenceProjection,
    manifest_digest: CleanupManifestDigest,
    expected_obligation_count: usize,
    observed_obligation_count: usize,
    expected_settled_count: usize,
    observed_settled_count: usize,
    expected_receipt_digest: u64,
    observed_receipt_digest: u64,
}

enum ProvisionalTopSlotTerminalPreparationOutcome {
    Prepared(ProvisionalTopSlotTerminalInstallation),
    Mismatch {
        ledger: CleanupLedger,
        evidence: ProvisionalTopSlotTerminalMismatchProjection,
    },
}

fn prepare_provisional_top_slot_terminal(
    ledger: CleanupLedger,
    grant: &HostGrantProjection,
    source: &HostAdmissionUnavailableEvidenceProjection,
) -> ProvisionalTopSlotTerminalPreparationOutcome;

struct ProvisionalTopSlotSettlementMismatchProjection {
    source: HostAdmissionUnavailableEvidenceProjection,
    stage: ProvisionalTopSlotSettlementStage,
    expected_partition_digest: u64,
    observed_partition_digest: u64,
}

enum ProvisionalTopSlotSettlementInertEvidence {
    Settlement(ProvisionalTopSlotSettlementMismatchProjection),
    TerminalPreparation(ProvisionalTopSlotTerminalMismatchProjection),
    Publication(HostAdmissionUnavailableFailureEvidence),
}

enum ProvisionalTopSlotSettlementInertSource<Evidence> {
    Prepared {
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        grant: SourceBoundHostGrant,
        physical: ProvisionalTopSlotPhysicalOwner,
        reserved: PreparedCleanupLedger,
    },
    Active {
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        grant: SourceBoundHostGrant,
        ledger: CleanupLedger,
    },
    Terminal(
        HostAdmissionUnavailableGrantedSource<
            ProvisionalTopSlotTerminalInstallation,
            Evidence,
        >,
    ),
}

enum ProvisionalTopSlotSettlementHolder<Evidence> {
    CauseBindInert {
        grant: SourceBoundHostGrant,
        physical: ProvisionalTopSlotPhysicalOwner,
        evidence: Evidence,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
    Prepared {
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        grant: SourceBoundHostGrant,
        physical: ProvisionalTopSlotPhysicalOwner,
        reserved: PreparedCleanupLedger,
    },
    Active {
        prepared: PreparedHostAdmissionUnavailableCause<Evidence>,
        grant: SourceBoundHostGrant,
        ledger: CleanupLedger,
    },
    Terminal(
        HostAdmissionUnavailableGrantedSource<
            ProvisionalTopSlotTerminalInstallation,
            Evidence,
        >,
    ),
    Inert {
        source: ProvisionalTopSlotSettlementInertSource<Evidence>,
        evidence: ProvisionalTopSlotSettlementInertEvidence,
    },
}

enum ProvisionalStageOutcome {
    Staged {
        source: ProvisionalStageSource,
        staged: StagedProvisionalTopSlot,
    },
    Rejected {
        source: ProvisionalStageSource,
        evidence: ProvisionalPreallocationRejection,
    },
    Settlement {
        grant: SourceBoundHostGrant,
        physical: ProvisionalTopSlotPhysicalOwner,
        evidence: ProvisionalTopSlotSettlementEvidence,
    },
}

enum HostAdmissionFailureInertSource {
    Observed {
        prepared: PreparedHostAdmissionUnavailableCause<HostAdmissionUnavailableEvidence>,
    },
    PreStaging {
        source: HostAdmissionUnavailablePreStagingSource<
            PreparedEcsHostCandidateSource,
            HostAdmissionUnavailableEvidence,
        >,
    },
    Granted {
        source: HostAdmissionUnavailableGrantedSource<
            HostAdmissionUnavailableGrantedInstallation,
            HostAdmissionUnavailableEvidence,
        >,
    },
    Confirming {
        owner: CorePreparedHostConfirmation<PreparedProvisionalHostInstallation>,
        evidence: HostAdmissionUnavailableEvidence,
    },
}

enum HostAdmissionFailureInertEvidence {
    CauseBind(HostAdmissionSourceIneligibleProjection),
    Unavailable(HostAdmissionUnavailableFailureEvidence),
    ConfirmationUnwind(HostConfirmationReceiverMismatchProjection),
}

enum HostAdmissionFailureHolder {
    OperationEvidenceInert {
        owner: HostAdmissionUnavailablePreStagingOwner,
        candidate: PreparedEcsHostCandidateSource,
        evidence: HostAdmissionOperationFailureEvidenceMismatch,
    },
    CauseBindObservedInert {
        source: HostAdmissionSourceProjection,
        evidence: HostAdmissionUnavailableEvidence,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
    CauseBindPreStagingInert {
        owner: HostAdmissionUnavailablePreStagingOwner,
        candidate: PreparedEcsHostCandidateSource,
        evidence: HostAdmissionUnavailableEvidence,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
    CauseBindGrantedInert {
        grant: SourceBoundHostGrant,
        installation: HostAdmissionUnavailableGrantedInstallation,
        evidence: HostAdmissionUnavailableEvidence,
        mismatch: HostAdmissionSourceIneligibleProjection,
    },
    Observed(
        PreparedHostAdmissionUnavailableCause<HostAdmissionUnavailableEvidence>,
    ),
    PreStaging(
        HostAdmissionUnavailablePreStagingSource<
            PreparedEcsHostCandidateSource,
            HostAdmissionUnavailableEvidence,
        >,
    ),
    Granted(
        HostAdmissionUnavailableGrantedSource<
            HostAdmissionUnavailableGrantedInstallation,
            HostAdmissionUnavailableEvidence,
        >,
    ),
    Confirming {
        owner: CorePreparedHostConfirmation<PreparedProvisionalHostInstallation>,
        evidence: HostAdmissionUnavailableEvidence,
    },
    Provisional(
        ProvisionalTopSlotSettlementHolder<HostAdmissionUnavailableEvidence>,
    ),
    Inert {
        source: HostAdmissionFailureInertSource,
        evidence: HostAdmissionFailureInertEvidence,
    },
}
```

Add exactly one `RouterEntry::pending_host_admission_failure: Option<HostAdmissionFailureHolder>`; no parallel candidate, grant, confirmation, or provisional-failure slot may own the same lineage. `PreparedHostAdmissionSource::into_parts()` keeps `PreparedEcsHostCandidateSource` beside the Core call: request error re-pairs returned Permit + candidate, and grant error re-pairs returned Token + candidate. Their exact `HostAdmissionOperationFailureEvidence` becomes the sole `HostAdmissionUnavailableEvidence::Operation`, then the bound `PreStaging` source retains owner + candidate until its callback consumes/clears the candidate after any required token cancellation and before unavailable publication.

`bind_granted_host_admission_source` is the only consuming producer of `CoreHostConfirmationSource`: staged success uses `PreparedProvisionalHostInstallation`; preallocation rejection uses `HostAdmissionUnavailableGrantedInstallation::Preallocation(candidate)` and enters `Granted`; post-allocation terminal uses its Provisional installation. Grant rejection is final fallible, and only its success permits the infallible installation-consuming callback and assignment-only unavailable publication. `HostConfirmationPreparationOutcome::ReceiverMismatch` is a synchronous pre-Confirming branch: `consume_host_confirmation_preparation_mismatch` consumes exactly that returned source + evidence, the evidence becomes the sole `HostAdmissionUnavailableEvidence::Confirmation(Receiver(_))`, and `PreparedProvisionalHostInstallation::into_physical` consumes staged+candidate+partition once before binding the cause and entering `HostAdmissionFailureHolder::Provisional`. It never calls unwind or enters `Granted`, and provisional cleanup terminal must precede unavailable publication. A later `CorePreparedHostConfirmation` failure enters `Confirming` whole. Exact unwind returns the same grant + installation for provisional settlement; unwind mismatch enters typed `Inert` with the complete confirming owner and original evidence. Cause-bind, receiver, issuer, preparation, and unwind mismatch each have a holder variant that retains every outside owner and calls callback/publication/redrive zero times. Observed completion also invokes its holder-clearing callback before publication. The holder is empty only after a terminal callback has consumed candidate/provisional installation, or after successful Ready publication has consumed the confirmation installation.

`PreparedHostAdmissionSource::into_parts()` moves the permit into Core issue/grant lineage and returns the non-`Clone` `PreparedEcsHostCandidateSource`; neither half is reconstructed. `StagedProvisionalTopSlot` carries only the frozen candidate projection, not the candidate owner. Every outcome returns the whole `grant + candidate` source exactly once: success combines `staged + candidate + partition` into `PreparedProvisionalHostInstallation` beside the grant, pre-allocation rejection returns the source unchanged, and post-allocation failure returns unchanged Core grant, evidence-free `ProvisionalTopSlotPhysicalOwner`, and the sole `ProvisionalTopSlotSettlementEvidence` as three distinct values. The evidence is consumed into `PreparedHostAdmissionUnavailableCause`; it is never cloned into the physical owner. A post-preparation confirmation failure first uses `unwind_host_confirmation`, then consumes the returned `PreparedProvisionalHostInstallation` into the same `grant + physical + evidence` shape; the pre-Confirming preparation mismatch follows the direct path above and never calls unwind. Neither path duplicates staged/candidate/evidence ownership.

The exact post-allocation flow is `CauseBindInert | Prepared -> Active -> Terminal -> published | Inert`. The holder keeps the original typed unavailable evidence, grant, and physical owner throughout cleanup. Activation moves physical into a dedicated batchless cleanup ledger. `prepare_provisional_top_slot_terminal` is the only consuming terminal producer: it exact-refreezes the grant and borrowed prepared-evidence projection, requires every reserved obligation to have one exact receipt, extracts the same physical owner from the settled ledger, and seals it beside private non-`Clone` `ProvisionalTopSlotPhysicalCompletionFacts`. This path never issues or reconstructs a Core `CleanupBatchToken`/`CleanupCompletion`; projection values are validation-only. Mismatch returns the whole ledger and exact expected/observed obligation, settled, and receipt-digest evidence. Only then may `Terminal` combine the grant with that installation as `CoreHostConfirmationSource` and enter the granted unavailable publisher. Receiver/issuer mismatch returns the complete terminal source and is stored in `Inert`; `ProvisionalTopSlotSettlementInertEvidence` preserves the distinct Settlement, TerminalPreparation, or Publication cause without retagging. Every mismatch retains the matching Prepared/Active/Terminal source and performs zero callback, publication, later World write, or redrive. Exact publication invokes the sole callback that consumes/clears the terminal installation and its facts. No callback or public unavailable state occurs before physical terminal.

Register ownership first; write bundle, `Node`, `ChildOf`, and probe separately, flushing and rebuilding the complete candidate/target/protected partition after each. Any failure after allocation returns `Settlement` and can only enter independent cleanup; `stage_granted_top_slot` may not call `remove_exact + despawn` directly. Implement around `host.rs::{HostCandidateRevisionState::observe,stage_provisional_top_slot,stage_granted_top_slot,activate_granted_top_slot}` and `navigation.rs::{observe_authority_system,apply_applied_driver_report_to_branch,stale_driver_report_owner,RouterEntry}`.

- [ ] Consume the R2-produced `SettledRetiredHostEvidence` for same-host eligibility. Require the same exact host and a fresh registration incarnation before issuing the source permit. New admission allocates fresh host version/top-slot Entity/claim/seal, retains history, leaves active `None`, and replays nothing until explicit replace. Old quarantine may remain protected; do not reap it. The stored evidence is non-authorizing and cannot detach, mutate, or reconstruct a lease.

- [ ] Add independent stale-output holder:

```rust
#[derive(Clone)]
struct AppliedDriverReportArtifact {
    identity: ViewReportIdentity,
    kind: ViewBuildReportKind,
    message: Option<String>,
    plan: ContentPlan,
}

struct SealedOldOutputSourceOwner {
    source: OldOutputSourceIdentity,
    pending: PendingCommit,
    task: RootFrameTask,
}

struct SealedOldOutputOwner {
    source: OldOutputSourceIdentity,
    physical: FailedPreparedBranch,
}

struct OldOutputSourceIdentity {
    router: RouterIdentity,
    host_version: HostVersion,
    frame: RouteFrameVersion,
    generation: RouteGeneration,
    transaction: TransactionId,
    sequence: u64,
    task: RootTaskIdentity,
}

struct FrozenStaleOutputPartition {
    targets: Vec<FrozenStaleOutputTarget>,
    protected_current_epoch: Vec<FrozenProtectedEntity>,
    protected_app_foreign: Vec<FrozenProtectedEntity>,
    protected_old_output_non_target: Vec<FrozenProtectedEntity>,
    protected_other_holders: Vec<FrozenProtectedEntity>,
    allowed_writes: Vec<StaleOutputAllowedWrite>,
}

struct FrozenStaleOutputTarget {
    source: OldOutputSourceIdentity,
    claim: ExactRegistryClaim,
    current: FrozenEntityAuthorityProjection,
    next: Option<FrozenEntityAuthorityProjection>,
}

enum StaleOutputLedgerHolder {
    Prepared {
        owner: SealedOldOutputOwner,
        ledger: PreparedCleanupLedger,
    },
    Active(CleanupLedger),
    Inert { ledger: CleanupLedger, evidence: StaleOutputMismatchEvidence },
}

struct PendingStaleOutputCleanup {
    report: AppliedDriverReportArtifact,
    partition: FrozenStaleOutputPartition,
    ledger: StaleOutputLedgerHolder,
}
```

`AppliedDriverReportArtifact` is a non-authorizing production value created only by consuming the real `ViewBuildReport` beside its `ContentPlan`; its private fields preserve the exact report identity/kind/message/plan. Cloning it cannot create a task, branch, ticket, continuation, or physical owner; only test-support requests such a clone to simulate replay. `SealedOldOutputSourceOwner` and `SealedOldOutputOwner` are non-`Clone`. Add `RouterEntry::pending_stale_output_cleanup: Option<PendingStaleOutputCleanup>`. Route through the real applied-report consumer while retaining `pending + task` whole until it classifies `StaleDiscarded` with World/lifecycle zero writes; the consuming transition then produces `physical: FailedPreparedBranch` and installs the unique owner only in `StaleOutputLedgerHolder::Prepared`. Activation moves that owner into `CleanupLedger::owner`, leaving no duplicate external owner. Targets and every protected set are pairwise disjoint and their union exactly covers the complete observed relationship closure; app-owned/foreign and old-output non-target extras remain legal protected facts, not a closed-world rejection. Clean only sealed old output with exact-once receipts using the same purpose-compatible per-write refreeze executor; protect current/new request, active, history, host, registry, snapshot, audit, queue, app-owned/foreign, old-output non-target, and other holders. Mismatch destructures the holder and returns report+owner; ownerless stale report builds no ledger/terminal and borrows no current continuation. `stale_output_despawn_hook_protected_drift_stops_before_receipt` mutates a real app-owned/foreign protected relationship/component after the first target write and proves no receipt for that write and no later mutation.

- [ ] Run the first R3 REDs immediately after adding them:

First add only the doc-hidden ECS test-support control shape so the integration target compiles; it may delay/release the real artifact but must not implement stale classification, cleanup, terminal, or receipt behavior.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_loss_terminal_publishes_stable_unavailable_host_lost -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_unavailable_cause_is_derived_from_exact_observation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_unavailable_rejection_clears_issuer_before_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_unavailable_issuer_failure_returns_typed_observed_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_request_sequence_exhaustion_is_permanent_and_exact -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_grant_identity_exhaustion_is_permanent_and_exact -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_confirmation_unwind_returns_grant_and_installation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::host_admission_request_and_grant_errors_retain_candidate_owner -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::preallocation_grant_rejection_consumes_candidate_before_unavailable -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::host_confirmation_preparation_mismatch_enters_provisional_settlement -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::host_confirmation_unwind_mismatch_retains_whole_owner_inert -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib host::tests::candidate_revision_exhaustion_is_bounded_world_zero -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib host::tests::staging_first_write_registration_drift_stops_before_node_and_host_edge -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::provisional_top_slot_settlement_terminal_precedes_unavailable_publication -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::invalid_host_topology_publishes_unavailable_without_staging -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::same_host_readmission_waits_for_retired_edge_and_old_slot_disposition -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::sealed_stale_output_cleanup_is_disjoint_from_new_epoch -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::ownerless_old_report_after_readmission_is_zero_write -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::stale_output_despawn_hook_protected_drift_stops_before_receipt -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r3_unavailable_reason_and_enqueue_mapping_are_source_bound -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r3_journey_g_re_admission_uses_fresh_epoch_and_explicit_restore -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r3_journey_g_same_host_readmission_detaches_old_top_slot_before_new_epoch -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_scenario_e_sealed_stale_output_cleanup_preserves_current_epoch -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_scenario_e_ownerless_old_artifact_is_public_snapshot_zero_write -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit recovery_r3_same_host_and_stale_epoch_authority_audit_is_exact -- --exact
```

Expected first failures: typed unavailable mapping/fresh repair is absent; revision/request-sequence/grant-identity exhaustion panics or loses exact evidence; request/grant error drops the outside candidate, preallocation has no grant-bound terminal consumer, preparation mismatch lacks the direct provisional-settlement transition, and unwind mismatch has no whole-owner inert holder; staging performs Node/edge writes before observing drift; same-host and stale outputs lack independent terminal evidence. Every public/audit test executes and fails on its named semantic assertion rather than compilation. Zero executed tests is failure.

- [ ] Keep the R3 delta/catalogue row planned and run semantic GREEN plus closed History/R1/R2 regressions:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_r3_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys recovery_scenario_e_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit recovery_r3_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_loss_terminal_publishes_stable_unavailable_host_lost -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_source_permit_reopens_only_changed_retryable_unavailable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_unavailable_cause_is_derived_from_exact_observation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_unavailable_rejection_clears_issuer_before_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_unavailable_issuer_failure_returns_typed_observed_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_request_sequence_exhaustion_is_permanent_and_exact -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_admission_grant_identity_exhaustion_is_permanent_and_exact -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_confirmation_receiver_mismatch_returns_whole_owner_and_calls_zero -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_confirmation_issuer_rejection_is_last_fallible_and_returns_whole_owner -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_confirmation_unwind_returns_grant_and_installation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib host_admission_request_and_grant_errors_retain_candidate_owner -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib preallocation_grant_rejection_consumes_candidate_before_unavailable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib host_confirmation_preparation_mismatch_enters_provisional_settlement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib host_confirmation_unwind_mismatch_retains_whole_owner_inert -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib host_confirmation_callback_installs_binding_before_ready_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib candidate_revision_exhaustion_is_bounded_world_zero -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib staging_first_write_registration_drift_stops_before_node_and_host_edge -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib provisional_top_slot_settlement_terminal_precedes_unavailable_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib same_host_readmission_waits_for_retired_edge_and_old_slot_disposition -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib sealed_stale_output_cleanup_is_disjoint_from_new_epoch -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib ownerless_old_report_after_readmission_is_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib authority_regression_tests::host_reacceptance_detaches_old_top_slot -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib reconciliation_authority_loss_app_despawned_host_settles_and_readmits_without_replay -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib stale_back_report_receipt_and_publication_are_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib admission::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib integration::tests:: -j 1
cargo nextest run --locked -p bevy_ui_router_core --config "build.rustc-wrapper = ''" -j 1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --all-targets -j 1
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-targets -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all listed semantic tests pass while the machine profile/catalogue remains at R2 state `8 / 26`.

- [ ] Obtain Core source/epoch, ECS staging/stale, and public semantic review. Only after all three report no Critical/Important findings, activate exactly the R3 delta/catalogue row and run:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
```

Expected: all pass; verifier prints `authority regression verification passed: catalogue=27 registered=27 passed=27`.

- [ ] Commit the exact R3 set.

```powershell
git add -- crates/bevy_ui_router_core/src/admission.rs crates/bevy_ui_router_core/src/presentation.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_ecs/src/host.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/test_support.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs docs/mvp-acceptance-journeys.md docs/superpowers/specs/2026-08-12-recovery-design.md
git diff --cached --name-only
git diff --cached --check
git commit -m "feat: add fresh-epoch host readmission"
```

Expected staged inventory is exactly the fifteen listed paths; facade `src/lib.rs`, fixture/golden, verification-only compile-fail files, and user WIP are absent.

---

### Task 5: Run the Recovery completion gate and close facts only

**Candidate:** Tasks 1–4 committed; tracked worktree/index clean; no facts document changed before verification.

- [ ] Run the same committed candidate through all gates:

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
cargo nextest run --locked --workspace --all-features --config "build.rustc-wrapper = ''" -j 1
cargo test --locked --workspace --doc --config "build.rustc-wrapper = ''" -j 1
cargo check --locked --workspace --all-targets --all-features --config "build.rustc-wrapper = ''" -j 1
cargo fmt --all -- --check
git diff --check
git diff --cached --check
git diff --exit-code
git diff --cached --exit-code
git status --short
```

Record actual run identities/counts; do not prewrite nextest/doctest totals. Only authority `27/27/27` is frozen. `git status --short` may show the existing user `review-history-*` files; call only the tracked worktree/index clean.

- [ ] Prove evidence files are tracked and have no trailing whitespace.

```powershell
git ls-files --error-unmatch -- docs/superpowers/specs/2026-07-20-history-design.md docs/superpowers/specs/2026-08-12-recovery-design.md docs/superpowers/plans/2026-08-12-recovery-implementation-plan.md docs/README.md docs/superpowers/README.md docs/current-architecture.md docs/roadmap.md docs/implementation-goals.md docs/target-architecture.md docs/authority-transition-matrix.md docs/mvp-acceptance-journeys.md docs/facade-export-allowlist.md docs/facade-contract-fixture.md docs/facade-associated-api-golden.txt crates/bevy_ui_router/tests/support/facade_contract_cases.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs scripts/verify-authority-regressions.ps1
rg -n "[ \t]+$" docs/superpowers/specs/2026-07-20-history-design.md docs/superpowers/specs/2026-08-12-recovery-design.md docs/superpowers/plans/2026-08-12-recovery-implementation-plan.md docs/README.md docs/superpowers/README.md docs/current-architecture.md docs/roadmap.md docs/implementation-goals.md docs/target-architecture.md docs/authority-transition-matrix.md docs/mvp-acceptance-journeys.md docs/facade-export-allowlist.md docs/facade-contract-fixture.md docs/facade-associated-api-golden.txt crates/bevy_ui_router/tests/support/facade_contract_cases.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs scripts/verify-authority-regressions.ps1
```

Expected: tracking exits `0`; `rg` has no matches.

- [ ] Obtain three independent final reviews: Core lifecycle/owner conservation, ECS physical authority/hook writes, and public/canonical profile. Close all Critical/Important findings before facts update.

- [ ] Update facts only in `docs/current-architecture.md`, `docs/roadmap.md`, `docs/README.md`, `docs/authority-transition-matrix.md`, `docs/implementation-goals.md`, `docs/superpowers/README.md`, `.superpowers/sdd/progress.md`, Recovery spec, and this plan. Record actual commit range/run identities, six active deltas, and `27/27/27`.

Claims are limited to Journey D complete, Scenario E complete, and Journey G’s Recovery-owned foreground/readmission rows complete. Keep actual old-lease reap, full Journey G, Authority closure, and MVP PASS explicitly incomplete; set `Now` to Authority closure.

- [ ] Re-run exact facts/profile gates, then stage and commit the facts-only set.

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
rg -n "[ \t]+$" docs/current-architecture.md docs/roadmap.md docs/README.md docs/authority-transition-matrix.md docs/implementation-goals.md docs/superpowers/README.md .superpowers/sdd/progress.md docs/superpowers/specs/2026-08-12-recovery-design.md docs/superpowers/plans/2026-08-12-recovery-implementation-plan.md
git diff --check
git add -- docs/current-architecture.md docs/roadmap.md docs/README.md docs/authority-transition-matrix.md docs/implementation-goals.md docs/superpowers/README.md docs/superpowers/specs/2026-08-12-recovery-design.md docs/superpowers/plans/2026-08-12-recovery-implementation-plan.md
git add -f -- .superpowers/sdd/progress.md
git diff --cached --name-only
git diff --cached --check
git commit -m "docs: close recovery stage"
git diff --exit-code
git diff --cached --exit-code
git status --short
```

Expected staged inventory is exactly the nine facts files; verifier remains `27/27/27`; post-commit tracked diffs are empty and status lists only the pre-existing user WIP.

## Execution handoff

Execute Tasks 1–5 in order. Each semantic task uses one writer, a real RED before production code, focused GREEN before affected/public gates, three-way review before commit, and a clean tracked candidate before the next task. Do not start Authority closure in this plan.
