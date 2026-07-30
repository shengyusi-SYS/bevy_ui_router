# History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking
>
> Use `test-driven-development` for every production semantic change and `verification-before-completion` before every PASS or commit claim

**Status:** Approved, independent plan review PASS 2026-07-20

**Review:** no open Critical or Important findings

**Planning baseline:** `be25dbe7ed087a5b80dd020a942223838a563032`

**Stage spec:** `docs/superpowers/specs/2026-07-20-history-design.md`

**Goal:** 在唯一 Core history publisher 下交付 `replace`、晚绑定相对 `back`、exact history identity、manifest-neutral back reservation、pure no-target settlement 与完整 History 公共证据

**Architecture:** Core 独占 canonical history、request ordering、exact source/destination 与单次 publication capability，ECS 独占 exact Entity preflight、cleanup capacity、manifest、ledger 与 receipt，facade 只提交命令并投影只读快照

`replace` 先建立共享 history capability，`back` 在 enqueue 只持有 published admission stamp，成为 running 后从 Core live history 解析一次 exact target，所有 history 变化只在现有 atomic publication tail 与 presentation 同时发生

**Tech Stack:** Rust 2024、Bevy ECS/UI、workspace facade contract harness、markdown machine profile、PowerShell authority verification

## Global Constraints

- 权威顺序固定为 `AGENTS.md → target-architecture → authority-transition-matrix → facade fixture/allowlist/golden → journeys → current facts → roadmap`
- Core 是唯一 logical history、active、request terminal publisher
- ECS 不保存 cursor 或可重建 Core owner 的 history 副本
- `MemoryHistory` 与 compatibility DTO 保持 detached、router-neutral
- 每个 router history incarnation、entry identity 与 revision 私有且不可复用
- 每个成功 `push | replace | back` 都推进 revision，包括内容等价的 replace/back
- push/replace target 在 enqueue 时 canonicalize，exact source 在 request 成为 running 时冻结
- back enqueue 只使用上一轮 published admission stamp，running 时只解析一次 live target
- `PreparedPublication` 前 exact refreeze，source drift 永久 fail closed，不得 retry 或静默 retarget
- history capability 私有、non-`Clone`、单次消费，snapshot/fingerprint/projection 不能重建
- back acceptance 先持有 ECS manifest-neutral capacity owner，再申请 Core request identity
- no-target 必须使用 exact proven-empty projection 与 canonical zero manifest，不得伪造 frame/task identity
- retry 只允许 `RetryableCapacity | UnfinalizedBoundaryFacts`，并返还完整原 owner 与 evidence
- protected host、current、retained、slot-only 与 non-target closure 保持 zero write
- publication 是最后语义步骤，atomic tail 不得包含 query、allocation、validation 或 hook/observer-triggering write
- precommit authority takeover 保持 history，settlement 后才清空失效 active
- postcommit compound settlement、host re-admission、background reaper、multi-router 与 input probes 不属于 History
- 每片先真实 RED，再 focused → affected crate → public target 验证
- Root 是唯一 Cargo/rustc、index 与 commit 负责人，子代理不得暂存或提交
- 只使用精确文件列表 `git add -- <files>`
- 不在同一提交混合机械拆分与 lifecycle 语义修改
- 文档优先使用换行、列表和表格自然分隔，不使用不必要的中文句号

## File Responsibility Map

| File | Responsibility |
| --- | --- |
| `crates/bevy_ui_router_core/src/history.rs` | detached `MemoryHistory` 与 public `HistorySnapshot` |
| `crates/bevy_ui_router_core/src/history_authority.rs` | private canonical incarnation、entry、revision、source/destination 与 publication capability |
| `crates/bevy_ui_router_core/src/ids.rs` | existing atomic request/transaction/cleanup/frame identity reservation and burn semantics |
| `crates/bevy_ui_router_core/src/request.rs` | optional resolved target 与 single-assignment rule |
| `crates/bevy_ui_router_core/src/engine.rs` | command queue、running owner transition、failure、seal 与 publication orchestration |
| `crates/bevy_ui_router_core/src/integration.rs` | opaque ECS bridge，不进入 facade re-export |
| `crates/bevy_ui_router_ecs/src/cleanup.rs` | manifest-neutral capacity owner、proven-empty、exact ledger/receipt |
| `crates/bevy_ui_router_ecs/src/navigation.rs` | facade enqueue、admission cache、physical preflight 与 Core/ECS handoff |
| `crates/bevy_ui_router_ecs/src/snapshots.rs` | public optional requested projection |
| `crates/bevy_ui_router/tests/public_navigation_journeys.rs` | C1–C3、F1–F3 与既有 A/B/D 真实 `App::update` 证据 |
| `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs` | facade-reachable exact owner、protected partition 与 receipt 证据 |
| `crates/bevy_ui_router/tests/support/facade_contract_cases.rs` | History stage profile、golden、negative contract parser |
| `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs` | Root 20 + Nested 3 + active History additions 的唯一 union |

## Execution Ownership

| Slice | Writer | Independent reviews before commit |
| --- | --- | --- |
| Profile and canonical docs | one documentation writer | canonical facts reviewer |
| Replace | one Core/ECS lifecycle writer | Core authority + public contract |
| Back execution | one Core/ECS lifecycle writer | Core authority + ECS physical authority |
| Back failure | one Core/ECS lifecycle writer | Core authority + ECS physical authority + public contract |
| Completion | Root only | three responsibility-separated final reviews |

并行只允许只读审查与不重叠的测试设计，不并行修改 shared lifecycle runtime

## Preflight

- [ ] **Step 1: Confirm the approved baseline and clean workspace**

```powershell
git rev-parse HEAD
git status --short
git show --stat --oneline be25dbe7ed087a5b80dd020a942223838a563032
git diff --check
```

Expected

- `HEAD` includes `be25dbe`
- design spec is committed
- worktree has no unowned changes
- diff check exits `0`

- [ ] **Step 2: Record single-writer and single-verifier ownership in the active task update**

Expected

- exactly one runtime writer
- Root is the only Cargo/rustc, index and commit owner
- reviewers are read-only

---

### Task 1: Activate the History machine profile without runtime changes

**Files**

- Modify: `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`
- Modify: `crates/bevy_ui_router/tests/facade_export_allowlist.rs`
- Modify: `crates/bevy_ui_router/tests/facade_authority_compile_fail.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`

**Interfaces**

- Consumes: exact tables under spec sections `10.1`、`10.2`、`10.3`
- Produces: strict History exclusions, contract-delta state and active authority union parsers
- Preserves: 12 exclusions、0 active deltas、Root 20 + Nested 3 = 23 authority tests

- [ ] **Step 1: Write parser RED tests before changing production harness callers**

Add focused tests with these exact assertions

```rust
#[test]
fn history_profile_has_exact_initial_machine_state() {
    let profile = history_contract_profile();
    assert_eq!(profile.exclusions.len(), 12);
    assert_eq!(profile.contract_deltas.len(), 3);
    assert!(profile.contract_deltas.iter().all(|row| row.state == "planned"));
    assert_eq!(profile.authority_additions.len(), 1);
    assert!(profile.authority_additions.iter().all(|row| row.state == "planned"));
}

#[test]
fn history_profile_keys_are_disjoint() {
    let profile = history_contract_profile();
    profile.assert_disjoint_keys();
}
```

Run

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist history_profile_ -j 1
```

Expected: FAIL because `history_contract_profile` and staged row validation do not exist

- [ ] **Step 2: Implement the strict profile model**

Use this internal shape in `facade_contract_cases.rs`

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedProfileRow {
    pub item: String,
    pub owning_slice: String,
    pub state: String,
    pub claim_boundary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryContractProfile {
    pub exclusions: Vec<TableRow>,
    pub contract_deltas: Vec<StagedProfileRow>,
    pub authority_additions: Vec<StagedProfileRow>,
}

pub fn history_contract_profile() -> HistoryContractProfile {
    let source = read_workspace("docs/superpowers/specs/2026-07-20-history-design.md");
    HistoryContractProfile::parse(&source)
}
```

Parser requirements

- require exact headings and exact column headers
- accept only `planned | active`
- reject empty tables, duplicate rows, duplicate item keys and unknown owning slices
- require canonical symbol or associated-item paths in exclusions
- require all three key sets to be pairwise disjoint
- active contract deltas must already exist in fixture, golden and owning public evidence
- active History authority rows must belong to final MVP vocabulary and not overlap Root/Nested

- [ ] **Step 3: Switch all facade profile consumers from Nested to History**

Replace direct reads of `docs/nested-push-contract-profile.md` in the three facade harness paths with

```rust
let profile = history_contract_profile();
let exclusions = profile.exclusions;
```

Do not change golden, fixture, runtime source or exclusions state in this task

- [ ] **Step 4: Extend the authority catalogue expected union**

Use the active History suffix only

```rust
let history_names = parse_history_authority_rows(&history_profile)
    .into_iter()
    .filter_map(|row| (row.state == "active").then_some(row.test_function))
    .collect::<Vec<_>>();

let mut expected = root_names.clone();
expected.extend(nested_names.clone());
expected.extend(history_names.clone());
assert_eq!(AUTHORITY_REGRESSION_CATALOGUE, expected);
```

Keep `AUTHORITY_REGRESSION_CATALOGUE` at 23 in this task

- [ ] **Step 5: Run the green profile gates**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit `0`, exclusions remain 12, active History rows remain empty, catalogue remains 23

- [ ] **Step 6: Review and commit the profile harness**

```powershell
git diff -- crates/bevy_ui_router/tests/support/facade_contract_cases.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs
git add -- crates/bevy_ui_router/tests/support/facade_contract_cases.rs crates/bevy_ui_router/tests/facade_export_allowlist.rs crates/bevy_ui_router/tests/facade_authority_compile_fail.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs
git diff --cached --name-only
git commit -m "test: activate history stage profile"
```

Expected staged set: exactly the four listed files

---

### Task 2: Amend canonical History contracts before runtime

**Files**

- Modify: `docs/README.md`
- Modify: `docs/superpowers/README.md`
- Modify: `docs/target-architecture.md`
- Modify: `docs/authority-transition-matrix.md`
- Modify: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`
- Modify: `docs/facade-contract-fixture.md`
- Modify: `docs/mvp-acceptance-journeys.md`

**Interfaces**

- Consumes: approved History design sections `1–7` and `11`
- Produces: one canonical command/target/identity/cleanup contract for all runtime tasks
- Preserves: current architecture, roadmap, progress and golden as current facts until owning slices pass

- [ ] **Step 1: Add canonical facts with no implementation claims**

Required amendments

```text
target-architecture
- immutable command and resolved target are separate
- push/replace requested is Some at enqueue
- back requested is None until one-time running resolution
- private incarnation, entry identity, revision and non-Clone capability
- manifest-neutral back capacity reservation and proven-empty projection
- IdentityExhausted and exact enqueue precedence

authority-transition-matrix
- no-target proven-empty and canonical zero manifest
- cleanup scope and continuation in separate columns
- precommit authority precedence and postcommit History/Recovery boundary
- pre-proof no-target authority takeover publishes only authority-specific `Running(Recovering) + None`, never normal CleanupPending
- typed no-target Retry is completely internal/public zero-write and returns original candidate/reservation/evidence to its named producer
- requested Some/None terminal exception
- IdentityExhausted has no request and no public state change

facade-contract-fixture
- NavigationRequestSnapshot::requested() -> Option<&RouteLocation>
- NavigationEnqueueErrorKind::IdentityExhausted
- replace/back observation semantics without new authority API

mvp-acceptance-journeys
- C1 requested Some
- C2 resolved failure remains Some
- C3 first-success second request None at no-target terminal
- C3 first-failure second request resolves Some
- allocator-derived IdentityExhausted and enqueue precedence
```

- [ ] **Step 2: Register the only active History profile**

Update both README files so that

- the History spec is the only active stage profile during implementation
- older specs/plans are evidence-only
- machine profile tables point to sections `10.1–10.3`
- no second roadmap or duplicated normative rules are introduced

- [ ] **Step 3: Run documentation/profile validation**

```powershell
rg -n "History|requested|IdentityExhausted|ProvenEmptyProjection|active stage profile" docs/README.md docs/superpowers/README.md docs/target-architecture.md docs/authority-transition-matrix.md docs/facade-contract-fixture.md docs/mvp-acceptance-journeys.md
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue history_no_target_matrix_rows_are_canonical -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
git diff --check
```

Expected: all gates remain green because golden and staged rows are not activated yet

- [ ] **Step 4: Independent canonical review**

Reviewer must answer

- Does every new normative statement derive from the approved History spec
- Is command distinct from resolved target
- Is Core still the only history publisher
- Are History and Recovery boundaries explicit
- Are current facts left untouched

Expected: no Critical or Important findings

- [ ] **Step 5: Commit canonical amendments**

```powershell
git add -- docs/README.md docs/superpowers/README.md docs/target-architecture.md docs/authority-transition-matrix.md docs/facade-contract-fixture.md docs/mvp-acceptance-journeys.md crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs
git diff --cached --name-only
git commit -m "docs: amend canonical history contracts"
```

Expected staged set: exactly the six listed documentation files plus the canonical static gate

---

### Task 3: Deliver the Replace vertical slice

**Files**

- Create: `crates/bevy_ui_router_core/src/history_authority.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Modify: `crates/bevy_ui_router_core/src/history.rs`
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_journeys.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`
- Modify: `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`
- Modify: `docs/facade-associated-api-golden.txt`
- Modify: `docs/superpowers/specs/2026-07-20-history-design.md`

**Interfaces**

- Consumes: existing reconciliation projection, `PreparedPublication`, cleanup settlement and public snapshots
- Produces: private `CanonicalHistory`, exact source binding, non-Clone history capability, `RouterNavigation::replace`, precise `IdentityExhausted`
- Preserves: `MemoryHistory` public behavior and push journey semantics

- [ ] **Step 1: Create the public RED from the owning profile rows**

Make these staged contract changes together

- remove only `bevy_ui_router::ecs::RouterNavigation::replace` from section `10.1`
- set `NavigationEnqueueErrorKind::IdentityExhausted` to `active` in section `10.2`
- add `IdentityExhausted` to the golden enum
- add a real facade journey calling `RouterNavigation::replace`
- add an ECS consumer test that calls public `RouterNavigation::replace` after changing only the real Core allocator predicate input

The first public test must assert

```rust
let request = app
    .world_mut()
    .resource_mut::<RouterNavigation>()
    .replace(router, RouteLocation::new("/items/2"))
    .expect("replace must enqueue");

let terminal = drive_request_to_terminal(&mut app, router, request);
assert_eq!(terminal.status(), &NavigationRequestStatus::Succeeded);
assert_eq!(router_snapshot(&app, router).history().entries(), [RouteLocation::new("/items/2")]);
assert_eq!(router_snapshot(&app, router).history().cursor(), Some(0));
```

Add the named exhaustion RED in `navigation.rs`

```rust
#[test]
fn replace_facade_identity_exhaustion_comes_from_real_allocator_predicate() {
    for counter in TestingReservationCounter::ALL {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "replace-exhaustion");
        app.update();
        {
            let mut navigation = app.world_mut().resource_mut::<RouterNavigation>();
            navigation.entries[0]
                .engine
                .testing_exhaust_reservation_counter(counter);
        }
        let world_before = authority_world_fingerprint(app.world());
        let snapshots_before = app
            .world()
            .resource::<RouterSnapshots>()
            .get(router)
            .expect("the installed router has a published snapshot")
            .clone();
        let requests_before = app
            .world()
            .resource::<RouterNavigation>()
            .entries[0]
            .requests
            .len();

        let error = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/items/2"))
            .expect_err("the real allocator predicate is exhausted");

        assert_eq!(error.kind(), NavigationEnqueueErrorKind::IdentityExhausted);
        assert_eq!(
            app.world()
                .resource::<RouterNavigation>()
                .entries[0]
                .requests
                .len(),
            requests_before,
        );
        assert_eq!(authority_world_fingerprint(app.world()), world_before);
        assert_eq!(
            app.world()
                .resource::<RouterSnapshots>()
                .get(router)
                .expect("the router snapshot remains published"),
            &snapshots_before,
        );
    }
}
```

`TestingReservationCounter` and the exhaustion method stay in doc-hidden Core integration under `test-support`

They are not re-exported by the facade and can only change request/transaction/frame-task/frame/cleanup-batch counter inputs

Run

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_replace_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib replace_facade_identity_exhaustion_comes_from_real_allocator_predicate -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
```

Expected: FAIL because facade replace and precise enqueue error mapping do not exist

- [ ] **Step 2: Write Core authority RED tests**

Add exact tests in `history_authority.rs`

| Test | Setup | Required assertion |
| --- | --- | --- |
| `equivalent_replace_advances_revision_without_changing_public_shape` | publish `/items/1`, freeze and publish replace `/items/1` | entries/cursor unchanged, revision changes to replace request, entry identity changes once |
| `replace_at_middle_preserves_forward_entries_and_replaces_entry_identity` | publish A/B/C, back to B, replace B2 | entries become A/B2/C, cursor stays 1, only middle entry identity changes |
| `source_revision_aba_is_rejected_before_publication` | freeze replace at revision R1, publish another change, restore equal locations | prepare returns `HistoryPreparationError::Source`, frozen capability cannot publish |
| `foreign_publication_evidence_probe_is_zero_write` | alter only the observed router/request fields of a non-authorizing sealed-commit evidence projection | the production validation predicate rejects before mutation and the live capability remains unique |
| `current_work_clone_does_not_clone_history_capability` | clone work during child-mount preparation while a capability is live in the engine | work clones remain capability-free and engine retains exactly one owner |
| `consumed_publication_replay_proof_is_zero_write` | consume a real prepared publication, derive its non-authorizing evidence proof, probe the shared validation predicate again | replay is rejected before mutation and no capability can be reconstructed |

Add engine tests

| Test | Setup | Required assertion |
| --- | --- | --- |
| `replace_failure_and_fallback_never_publish_history` | active route then replace whose build and fallback branches are driven to terminal | old entries/cursor/revision remain exact |
| `replace_publication_updates_active_and_history_once` | seal replace and inspect before/after `publish_prepared` | before has old active/history, after has new active/history and one commit trace |
| `replace_identity_exhaustion_is_complete_zero_change` | exhaust each real reservation counter before replace | returns `IdExhausted`, no record/queue/id/public projection change |

Run

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib history_authority::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::replace_ -j 1
```

Expected: FAIL because exact history authority and replace queue command do not exist

- [ ] **Step 3: Implement the private canonical history unit**

Use this exact ownership shape, keep all names crate-private and do not re-export them

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryIncarnation(RouterIdentity);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryEntryId {
    incarnation: HistoryIncarnation,
    creator: NavigationRequestId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryRevision {
    incarnation: HistoryIncarnation,
    committed_by: Option<NavigationRequestId>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FrozenHistorySource {
    incarnation: HistoryIncarnation,
    revision: HistoryRevision,
    cursor: Option<usize>,
    entry_ids: Vec<HistoryEntryId>,
    locations: Vec<RouteLocation>,
}

pub(crate) struct CanonicalHistory {
    projection: MemoryHistory,
    entry_ids: Vec<HistoryEntryId>,
    incarnation: HistoryIncarnation,
    revision: HistoryRevision,
}

pub(crate) enum HistoryEffect {
    Push(RouteLocation),
    Replace(RouteLocation),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NormalAttemptIdentity {
    host: RouteHostId,
    host_epoch: u64,
    slot: RouteSlotId,
    root_frame_task: FrameTaskId,
    root_frame: RouteFrameId,
    generation: RouteGeneration,
    origin: FrameOrigin,
    branch_index: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HistoryExecutionBinding {
    router: RouterIdentity,
    request: NavigationRequestId,
    normal_attempt: NormalAttemptIdentity,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoryExecutionBindingProjection {
    router: RouterIdentity,
    request: NavigationRequestId,
    normal_attempt: NormalAttemptIdentityProjection,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalAttemptIdentityProjection {
    host: RouteHostId,
    host_epoch: u64,
    slot: RouteSlotId,
    root_frame_task: FrameTaskId,
    root_frame: RouteFrameId,
    generation: RouteGeneration,
    origin: FrameOrigin,
    branch_index: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FrozenHistoryDestination {
    Push {
        target: RouteLocation,
    },
    Replace {
        target: RouteLocation,
        replaced: Option<HistoryEntryId>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedPresentationBinding {
    execution: HistoryExecutionBindingProjection,
    presentation: ActivePresentation,
    matched: RouteBranchFingerprint,
    target_frames: Vec<RouteFrameVersion>,
    cleanup: CleanupBatchToken,
}

pub(crate) struct HistoryCommitCapability {
    execution: HistoryExecutionBinding,
    source: FrozenHistorySource,
    destination: FrozenHistoryDestination,
}

pub(crate) struct PreparedHistoryPublication {
    execution: HistoryExecutionBinding,
    source: FrozenHistorySource,
    destination: FrozenHistoryDestination,
    presentation: PreparedPresentationBinding,
    next: CanonicalHistory,
}

pub(crate) struct HistorySourceDrift {
    capability: HistoryCommitCapability,
    observed: FrozenHistorySource,
}

pub(crate) struct HistoryBindingDrift {
    capability: HistoryCommitCapability,
    observed: HistoryExecutionBindingProjection,
}

pub(crate) enum HistoryPreparationError {
    Source(HistorySourceDrift),
    Binding(HistoryBindingDrift),
}
```

Required methods

```rust
impl CanonicalHistory {
    pub(crate) fn new(router: RouterIdentity) -> Self;
    pub(crate) fn projection(&self) -> &MemoryHistory;
    pub(crate) fn snapshot(&self) -> HistorySnapshot;
    pub(crate) fn freeze(
        &self,
        execution: HistoryExecutionBinding,
        effect: HistoryEffect,
    ) -> HistoryCommitCapability;
    pub(crate) fn prepare(
        &self,
        capability: HistoryCommitCapability,
        presentation: PreparedPresentationBinding,
    ) -> Result<PreparedHistoryPublication, HistoryPreparationError>;
    pub(crate) fn publish(&mut self, prepared: PreparedHistoryPublication);
}
```

Implementation rules

- `FrozenHistorySource` binds incarnation, revision, cursor and exact source entry identity
- `HistoryExecutionBinding` binds exact router, request, normal root task, transaction and cleanup batch
- destination binds intent and exact replaced entry, back extends it with exact target entry/cursor in Task 4
- `HistoryCommitCapability` is the only owner of the non-Clone `HistoryExecutionBinding`
- `PreparedPresentationBinding` carries only the immutable non-authorizing `HistoryExecutionBindingProjection` minted from the same sealed logical presentation and cleanup token
- `CanonicalHistory::prepare` exact-compares that projection with the capability owner, then moves the one owned binding into `PreparedHistoryPublication`
- binding or source drift returns the exact capability in a typed preparation error and enters the same bounded `AuthorityLost` settlement
- `PreparedHistoryPublication` stores the owned binding once; its nested presentation retains only the projection
- replace-empty creates the first entry with creator equal to request
- replace-current preserves forward tail and assigns a fresh entry identity
- every successful effect sets revision `committed_by = Some(request)`
- only `PreparedHistoryPublication` moves a new canonical state into the engine
- capability, owned execution binding and prepared publication do not implement `Clone`; only the non-authorizing binding projection may clone
- remove lifecycle `HistoryIntent` from detached `history.rs`

- [ ] **Step 4: Generalize Core queue and publication for Push and Replace**

Use a command-bearing queue

```rust
enum NavigationCommand {
    Push(RouteLocation),
    Replace(RouteLocation),
}

struct QueuedNavigation {
    request: NavigationRequestId,
    command: NavigationCommand,
    outcome: QueuedMatchOutcome,
    reservation: Option<ReservedBranchAttempt>,
}
```

Required flow

- canonicalize and match target at enqueue
- freeze exact history source and execution binding when queued request becomes running
- store the single `HistoryCommitCapability` in `RouterEngine::current_history_capability`, outside cloneable `CurrentWork`
- fallback and precommit failure drop the capability and preserve canonical history
- `prepare_confirmed_child` continues to clone only the physical/logical work projection and never the history owner
- `seal_commit` takes the capability exactly once, creates `PreparedPresentationBinding` from the same root task and cleanup token, then calls `CanonicalHistory::prepare`
- `PublicationPlan` and `SealedCommit` become move-only and own `PreparedHistoryPublication`
- source drift enters existing bounded `AuthorityLost` settlement
- existing atomic mutation closure runs before `CanonicalHistory::publish`
- public snapshot is projected from `CanonicalHistory::projection`
- engine test snapshots record only a non-authorizing `HistoryCapabilitySnapshot`, never clone the owner
- factor sealed-commit validation through a projection-only `SealedCommitEvidence` predicate used by production preparation and test probes
- replace both `SealedCommit::testing_duplicate` and `SealedCommit::testing_with_identity` with evidence probes that contain no `PublicationPlan`, cleanup owner or history capability
- derive duplicate/replay evidence only after real consumption, while foreign probes alter observed fields on a cloned evidence projection
- `PreparedHistoryPublication`, `PublicationPlan` and `SealedCommit` remain non-Clone in production and tests

The seam boundary is exact

- `SealedCommitEvidence` contains only immutable router/request/task/transaction/batch/source-revision projections
- production `prepare_publication` validates the live owner through the same evidence predicate before moving it
- a test-only publish wrapper consumes the real prepared publication and returns `ConsumedPublicationEvidence`
- replay and foreign probes accept only cloned/altered evidence, return classification, and have no path to the mutation closure
- removing or bypassing the production predicate must make the evidence-probe tests fail

Expose only these integration methods

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryExecutionBindingView {
    inner: HistoryExecutionBindingProjection,
}

pub fn push(&mut self, location: RouteLocation) -> Result<NavigationRequestId, IntegrationError>;
pub fn replace(&mut self, location: RouteLocation) -> Result<NavigationRequestId, IntegrationError>;
```

Each `*View` is an opaque immutable projection with private fields and no public constructor

It can validate exact binding but cannot mint or publish a capability

- [ ] **Step 5: Add facade replace and precise exhaustion mapping**

Add the public method

```rust
pub fn replace(
    &mut self,
    router: RouterHandle,
    location: RouteLocation,
) -> Result<NavigationRequestId, NavigationEnqueueError>;
```

Add and use the exact variant

```rust
pub enum NavigationEnqueueErrorKind {
    UnknownRouter,
    HostUnavailable,
    AuthorityRecovering,
    NoBackEntry,
    IdentityExhausted,
}
```

Mapping requirements

- `IntegrationError::Exhausted → IdentityExhausted`
- exhaustion produces no request id and no public snapshot change
- exhaustion never maps to `AuthorityRecovering`
- the test seam changes the real allocator counter only, never the returned classification
- the ECS test invokes the public facade method and production mapper, it does not call a private classification helper

Test-support-only Core integration seam

```rust
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestingReservationCounter {
    Request,
    Transaction,
    FrameTask,
    CandidateFrame,
    CleanupBatch,
}

#[cfg(any(test, feature = "test-support"))]
impl TestingReservationCounter {
    pub const ALL: [Self; 5] = [
        Self::Request,
        Self::Transaction,
        Self::FrameTask,
        Self::CandidateFrame,
        Self::CleanupBatch,
    ];
}

#[cfg(any(test, feature = "test-support"))]
impl RouterEngine {
    pub fn testing_exhaust_reservation_counter(
        &mut self,
        counter: TestingReservationCounter,
    );
}
```

The facade allowlist must continue to prove these items are not re-exported

- [ ] **Step 6: Close Replace physical and public matrix**

Add or extend tests for

- F1 canonical equivalent replace, zero Entity/view/generation/registry/cleanup mutation, revision advances
- F2 fragment-only replace, deepest normal leaf remount only
- F3 canonical path normalization while search and fragment remain exact
- nested suffix replace preserves prefix and exact child slot identity
- retained-empty and slot-only use Entity-local role/binding, not counts
- match/build/apply/mount failure and fallback keep old history
- stale report/publication/cleanup replay keeps current history and request unchanged
- exact leaving receipt and protected partition remain disjoint

Named public tests

| Test | Required evidence |
| --- | --- |
| `history_replace_f1_equivalent_is_physical_zero_write` | success request and revision advance with unchanged Entity set, effects, generation, registry, cleanup and history shape |
| `history_replace_f2_fragment_remounts_only_deepest_leaf` | preserved ancestors/slots, same leaf frame id, generation +1, exact old generation receipt, replaced fragment entry |
| `history_replace_f3_canonical_path_preserves_search_and_fragment` | canonical path in requested/active/history with exact original search and fragment |
| `history_replace_failure_preserves_published_history` | original failure terminal, fallback evidence, old entries/cursor/revision and no failed target entry |

- [ ] **Step 7: Verify Replace in layers**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib history_authority::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::replace_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib replace_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib replace_facade_identity_exhaustion_comes_from_real_allocator_predicate -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_replace_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit history_replace_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all focused, public and facade gates exit `0`

- [ ] **Step 8: Independent Replace review and commit**

Reviewers verify

- no second history publisher
- no cloned capability or owner reconstruction
- equivalent replace advances revision but is physically zero-write
- failure and stale paths preserve history
- exact Entity cleanup remains ECS-only

Commit only after no Critical or Important findings

```powershell
git add -- crates/bevy_ui_router_core/src/history_authority.rs crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/history.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/support/facade_contract_cases.rs docs/facade-associated-api-golden.txt docs/superpowers/specs/2026-07-20-history-design.md
git diff --cached --name-only
git commit -m "feat: add exact replace history flow"
```

---

### Task 4: Deliver the Back execution vertical slice

**Files**

- Modify: `crates/bevy_ui_router_core/src/history_authority.rs`
- Modify: `crates/bevy_ui_router_core/src/ids.rs`
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`
- Modify: `crates/bevy_ui_router_core/src/matcher.rs`
- Modify: `crates/bevy_ui_router_core/src/request.rs`
- Modify: `crates/bevy_ui_router_core/src/trace.rs`
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_core/src/test_support.rs`
- Modify: `crates/bevy_ui_router_ecs/Cargo.toml`
- Modify: `crates/bevy_ui_router_ecs/src/app_ext.rs`
- Create: `crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Create: `crates/bevy_ui_router_ecs/src/test_support.rs`
- Modify: `crates/bevy_ui_router_ecs/src/branch_transaction.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/dependency_boundary.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_journeys.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`
- Modify: `crates/bevy_ui_router/tests/support/facade_contract_cases.rs`
- Modify: `docs/facade-associated-api-golden.txt`
- Modify: `docs/superpowers/specs/2026-07-20-history-design.md`

**Interfaces**

- Consumes: canonical history capability from Task 3 and existing ECS cleanup ledger
- Produces: optional requested target, published admission stamp, linear request reservation, manifest-neutral ECS capacity owner, late-bound back, proven-empty zero settlement and public `back`
- Preserves: no target authority from snapshot, no fake frame/task identity, no fallback for `BackTargetUnavailable`

- [ ] **Step 0: Recheck the canonical no-target visibility rows before any runtime RED**

Run the exact static/canonical gate before Step 1 or any Back runtime test is added

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue history_no_target_matrix_rows_are_canonical -j 1
rg -n -F '只可公开 authority-specific `Running(Recovering) + None`' docs/authority-transition-matrix.md
rg -n -F '完全 internal 且 public zero-write' docs/authority-transition-matrix.md
```

Expected: the named gate exits `0`; both canonical rows are present, and the gate rejects either old visible Retry or normal no-target CleanupPending wording

- [ ] **Step 1: Extract the shared lifecycle evidence boundary before Back changes**

Move the existing `RequiredResourceSet`、`LifecyclePreflightSource`、`LifecyclePreflightExpectation`、`LifecyclePreflightObserved` and `LifecyclePreflightEvidence` definitions from `navigation.rs` into `lifecycle_preparation.rs` without changing variants or predicates

- make the moved types and every constructor/accessor used by sibling modules `pub(crate)`
- keep all fields private and preserve `const` where it exists
- update `navigation.rs` imports explicitly; `cleanup.rs` imports the shared types only when Step 8 begins using them
- do not re-export the types from the crate or facade
- never name `LifecyclePreflightEvidence` from Core source、`RouterEngine` inherent methods or Core-owned integration outcomes. Core receiver mismatch instead returns the single opaque non-authorizing `PreparationReceiverMismatchView`; ECS keeps its physical evidence paired only with ECS owners/diagnostics

Expose the module only as crate-private from `lib.rs`

Keep this commit purely mechanical

Do not add `LifecyclePreparationOutcome` or retry evidence until the Step 7 compile scaffold, where they can bind a Core-issued opaque boundary scope rather than naming private `RouterIdentity` or `TransactionId` types from ECS

Run the existing preflight and authority tests before adding any optional-request, admission, capacity or Back RED

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib reconciliation_preflight -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib root_fallback_ -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit `0`

Have one read-only reviewer confirm the relocation changes no enum variants, predicates or caller behavior, then commit it separately before writing any Back RED

```powershell
git add -- crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/navigation.rs
git diff --cached --name-only
git commit -m "refactor: share lifecycle preparation evidence"
```

- [ ] **Step 2: Write optional request projection RED**

Change the owning golden line and section `10.2` state to active, then add these assertions

```rust
assert_eq!(queued_back.requested(), None);
assert_eq!(running_back.requested(), Some(&RouteLocation::new("/items/1")));
assert_eq!(terminal_no_target.requested(), None);
```

Run

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib request::tests:: -j 1
```

Expected: FAIL because request target is non-optional and immutable from construction

- [ ] **Step 3: Implement single-assignment resolved target**

Use this exact field delta

```diff
- requested: RouteLocation,
+ requested: Option<RouteLocation>,
```

Add these constructors and methods

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RequestedTargetAlreadyResolved;

impl NavigationRequestRecord {
    pub(crate) fn with_resolved_target(
        id: NavigationRequestId,
        requested: RouteLocation,
    ) -> Self;

    pub(crate) fn unresolved_back(id: NavigationRequestId) -> Self;

    pub(crate) fn resolve_requested_once(
        &mut self,
        requested: RouteLocation,
    ) -> Result<(), RequestedTargetAlreadyResolved>;

    pub fn requested(&self) -> Option<&RouteLocation>;
}
```

Rules

- push/replace use `with_resolved_target`
- back starts with `unresolved_back`
- resolution happens once before Matching
- failure after resolution preserves `Some(target)`
- no-target terminal preserves `None`

- [ ] **Step 4: Write admission and Core reservation RED tests**

Add tests

| Test | Required setup and assertion |
| --- | --- |
| `no_back_entry_precedes_identity_exhaustion` | publish `can_back=false`, exhaust request counter, call back, assert `NoBackEntry`、no request、no capacity/Core allocator reach and unchanged queue/public state |
| `admission_stamp_cannot_authorize_or_reconstruct_target` | admit from positive stamp, advance live history before running, assert target comes from new live cursor and differs from stamp preview |
| `back_reserves_installed_maximum_branch_depth` | install real depth-1/depth-3 branches, reserve the immutable maximum, run a shallow target, assert only the exact prefix is consumed and every unused frame/task identity remains burned and cannot be reused |
| `back_between_duplicate_locations_uses_entry_identity` | publish equal locations as distinct entries, drive back through full prepared publication, assert exact destination entry/cursor/revision, preserved ordered entry identities, one presentation/history publication and replay zero-write |

Negative capability gates

| Gate | Required evidence |
| --- | --- |
| accepted view compile-fail fixture | with the real type resolvable, attempts to construct、clone、read raw identities or convert `BackAcceptedReservationView` into execution authority fail for the intended privacy/non-Clone reason |
| facade/profile gate | accepted view、branch projection、world seal and execution owners remain absent from facade exports and machine profile additions |
| static ownership review | fields confirm pre-resolution lineage contains no host/slot/frame/task/generation、normal attempt、destination、Entity or manifest |

Expected assertions

- unknown router wins first
- unavailable router wins before history eligibility
- `NoBackEntry` wins before Core identity exhaustion
- a positive stamp can admit but only live Core history chooses target
- unused Core branch identities are never reused
- accepted lineage, running execution authority and later physical disposition remain distinct owners

Obtain a semantic RED before Step 5

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib admission_stamp_cannot_authorize_or_reconstruct_target -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_reserves_installed_maximum_branch_depth -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_between_duplicate_locations_uses_entry_identity -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib no_back_entry_precedes_identity_exhaustion -j 1
```

Missing symbols do not count as the semantic RED

If the exact canonical signatures are absent, introduce only their opaque type/signature shape and a fail-closed zero-write path, confirm the focused test compiles with `cargo test ... --no-run`, then rerun it

Expected: each focused test executable exits nonzero on its named production assertion, not on compilation、test setup or injected classification

Register the negative fixtures now, but count them only after Step 6 makes the real type resolvable; missing symbols do not prove negative capability. Capacity issuer、growth、observer and Core failure order RED/GREEN belong to the ordered Step 6 micro-slices below

- [ ] **Step 5: Implement the world seal and typed Core admission**

Use opaque Core integration types not re-exported by facade

```rust
#[doc(hidden)]
pub struct HistoryAdmissionWorldSeal {
    inner: CoreIssuedWorldSeal,
}

pub struct HistoryAdmissionStamp {
    world: CoreIssuedWorldSealProjection,
    router: RouterIdentity,
    incarnation: HistoryIncarnation,
    published_revision: HistoryRevision,
    can_back: bool,
}

pub struct BackAdmissionToken {
    verified_stamp: HistoryAdmissionStamp,
    live_revision_at_prepare: HistoryRevision,
}

pub enum PrepareBackAdmissionErrorKind {
    ForeignStamp,
    AuthorityRecovering,
    NoBackEntry,
}

impl RouterEngine {
    pub fn claim_history_admission_world(
        &mut self,
    ) -> Result<HistoryAdmissionWorldSeal, IntegrationError>;
    pub fn history_admission_stamp(
        &self,
        world: &HistoryAdmissionWorldSeal,
    ) -> Result<HistoryAdmissionStamp, PrepareBackAdmissionErrorKind>;
    pub fn prepare_back_admission(
        &self,
        current_world: &HistoryAdmissionWorldSeal,
        stamp: &HistoryAdmissionStamp,
    ) -> Result<BackAdmissionToken, PrepareBackAdmissionErrorKind>;
}
```

Rules

- Core issues one non-Clone `HistoryAdmissionWorldSeal` when an installed `RouterEngine` is claimed by its ECS `RouterEntry`
- the seal binds a non-reusable Core identity plus router/history incarnation and has no public constructor, raw integer conversion, clone or snapshot projection
- Core stores the issued seal projection and rejects a second claim or a foreign seal
- ECS stores the seal only beside the exact `RouterEntry`, then returns it by reference at publish and prepare boundaries
- facade and direct integration consumers cannot mint, alter or reconstruct the seal from `RouterNavigation::domain` or a stamp
- ECS caches only stamps emitted during `PublishSnapshots`
- `prepare_back_admission` validates availability and exact world/router/incarnation provenance without allocating any identity
- a same-provenance stamp remains admissible when its published revision lags Core live revision
- `prepare_back_admission` records the current live revision in the token but does not compare it for equality with `published_revision`
- cross-world/router/incarnation mismatch returns `ForeignStamp`, availability drift returns `AuthorityRecovering`, both create no request and never choose target
- `can_back=false` returns `NoBackEntry` without a token or request
- `BackAdmissionToken` is opaque and non-Clone, preserving verified stamp provenance plus prepare-time live revision
- the stamp and token are not stored in queued/running work and never choose a target

- [ ] **Step 6: Implement admission and queued accepted-reservation safe stop**

Define only the opaque ECS capacity responsibilities before wiring the facade flow. `Free | Outstanding | Leased` are semantic cell states; the table、cell and journal containers remain implementation-private

```rust
struct BackCapacityPrivateSeal;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationCapacityDomain(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationReleaseGeneration(u64);

pub(crate) struct EcsCleanupCapacityReservation {
    permit: BackCapacityPermit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct BackCapacityOwnerId(u64);

pub(crate) struct BackCapacityPermit {
    /* exact entry domain、persistent cell、owner and generation */
    private: BackCapacityPrivateSeal,
}
pub(crate) struct ManifestNeutralLedgerSlot {
    /* move-only exact lease for one fully initialized persistent cell */
    private: BackCapacityPrivateSeal,
}
pub(crate) struct BackCapacityDomainIssuer {
    /* non-reusable domain allocator only */
    private: BackCapacityPrivateSeal,
}
impl BackCapacityDomainIssuer {
    pub(crate) const fn new() -> Self;
    pub(crate) fn issue(&mut self) -> Result<BackCapacityIssuer, BackCapacityReserveError>;
}

pub(crate) struct BackCapacityIssuer {
    /* exact domain/owner/generation allocator、entry-local persistent cells and exact Outstanding|Leased live registry */
    private: BackCapacityPrivateSeal,
}

pub(crate) enum BackCapacityReserveError { DomainExhausted, OwnerExhausted, LedgerSlotUnavailable }
pub(crate) enum BackCapacityIssuerInstallError { AlreadyInstalled, Reserve(BackCapacityReserveError) }
pub(crate) enum EnqueueRejected { CoreTokenDrift, CoreAllocatorFailure(EnqueueBackErrorKind) }
pub(crate) enum BackCapacityRetireDisposition { Target, Contradiction, AuthorityLost, Permanent }
pub(crate) enum BackCapacityRetireError { ForeignDomain, StaleGeneration, ConsumedReplay }
pub(crate) struct BackCapacityRetirementEvidence { /* exact non-authorizing retired identity */ }
enum BackCapacityLiveState { Outstanding, Leased }
struct BackCapacityLiveRecord { /* exact domain/cell/owner/generation + BackCapacityLiveState */ }
enum BackCapacityReuse { Reusable, PermanentlyRetired }
struct BackCapacityCell { /* Free | Outstanding | Leased lifecycle + orthogonal BackCapacityReuse marker */ }
enum PreparedRetiredCell {
    ReusableFree { next_generation: PreparationReleaseGeneration },
    PermanentlyRetired { generation: PreparationReleaseGeneration },
}
struct PreparedRetirementEvidencePayload { /* move-only exact retired identity/disposition */ }
struct PreparedNoTargetPermitPayload { /* move-only exact leased successor facts */ }
struct PreparedCanonicalZeroSlotPayload { /* move-only exact slot successor facts */ }
pub(crate) struct PreparedBackCapacityRetirementEvidence {
    payload: PreparedRetirementEvidencePayload,
}
pub(crate) struct PreparedNoTargetCapacityPermit {
    payload: PreparedNoTargetPermitPayload,
}
pub(crate) struct PreparedManifestNeutralLedgerSlot {
    payload: PreparedCanonicalZeroSlotPayload,
}
enum BackCapacityCommitTransition<'issuer> {
    OutstandingRetire {
        live: std::collections::hash_map::OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        next: PreparedRetiredCell,
    },
    OutstandingToLeased {
        live: std::collections::hash_map::OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        prepared_live: BackCapacityLiveRecord,
    },
    LeasedRetire {
        live: std::collections::hash_map::OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        next: PreparedRetiredCell,
    },
    LeasedToCanonicalZeroSlot {
        live: std::collections::hash_map::OccupiedEntry<'issuer, BackCapacityOwnerId, BackCapacityLiveRecord>,
        cell: &'issuer mut BackCapacityCell,
        prepared_slot_cell: BackCapacityCell,
    },
}
pub(crate) struct ExactBackCapacityCommitGuard<'issuer, 'owner, Owner: ?Sized> {
    transition: BackCapacityCommitTransition<'issuer>,
    owner: &'owner Owner,
}
impl<Owner: ?Sized> ExactBackCapacityCommitGuard<'_, '_, Owner> {
    pub(crate) fn commit(self); /* match prepared transition; exact OccupiedEntry update/remove + cell assignment only */
}
impl PreparedBackCapacityRetirementEvidence {
    pub(crate) fn into_evidence_after_exact_receiver(self) -> BackCapacityRetirementEvidence;
}
impl PreparedNoTargetCapacityPermit {
    pub(crate) fn into_permit_after_exact_receiver(
        self,
        stale_bound: BoundBackCapacityReservation,
    ) -> NoTargetCapacityPermit;
}
impl PreparedManifestNeutralLedgerSlot {
    pub(crate) fn into_slot_after_exact_receiver(
        self,
        stale_permit: NoTargetCapacityPermit,
    ) -> ManifestNeutralLedgerSlot;
}
pub(crate) enum BackCapacityRetireOutcome<Owner> { Retired(BackCapacityRetirementEvidence), Rejected { owner: Owner, rejection: EnqueueRejected, error: BackCapacityRetireError } }
pub(crate) struct NoTargetCapacityPermit { capacity: EcsCleanupCapacityReservation }

struct BackCapacityEnqueueRetireResidual {
    owner: EcsCleanupCapacityReservation,
    rejection: EnqueueRejected,
    retire_error: BackCapacityRetireError,
}

impl BackCapacityIssuer {
    pub(crate) fn reserve(&mut self) -> Result<EcsCleanupCapacityReservation, BackCapacityReserveError>;
    pub(crate) fn retire_unbound(&mut self, reservation: EcsCleanupCapacityReservation, rejected: EnqueueRejected) -> BackCapacityRetireOutcome<EcsCleanupCapacityReservation>;
    pub(crate) fn preflight_bound_retirement<'issuer, 'bound>(
        &'issuer mut self,
        bound: &'bound BoundBackCapacityReservation,
        disposition: BackCapacityRetireDisposition,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'bound, BoundBackCapacityReservation>,
            PreparedBackCapacityRetirementEvidence,
        ),
        BackCapacityRetireError,
    >;
    pub(crate) fn preflight_no_target_transfer<'issuer, 'bound>(
        &'issuer mut self,
        bound: &'bound BoundBackCapacityReservation,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'bound, BoundBackCapacityReservation>,
            PreparedNoTargetCapacityPermit,
        ),
        BackCapacityRetireError,
    >;
    pub(crate) fn preflight_no_target_retirement<'issuer, 'permit>(
        &'issuer mut self,
        permit: &'permit NoTargetCapacityPermit,
        disposition: BackCapacityRetireDisposition,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'permit, NoTargetCapacityPermit>,
            PreparedBackCapacityRetirementEvidence,
        ),
        BackCapacityRetireError,
    >;
    pub(crate) fn preflight_back_canonical_zero<'issuer, 'permit>(
        &'issuer mut self,
        permit: &'permit NoTargetCapacityPermit,
        projection: &ProvenEmptyProjection,
    ) -> Result<
        (
            ExactBackCapacityCommitGuard<'issuer, 'permit, NoTargetCapacityPermit>,
            PreparedManifestNeutralLedgerSlot,
        ),
        BackCapacityRetireError,
    >;
}

pub(crate) struct BoundBackCapacityReservation {
    capacity: EcsCleanupCapacityReservation,
    accepted: BackAcceptedReservationView,
}

pub struct BackAcceptedReservationView {
    inner: BackAcceptedReservationProjection,
}

pub(crate) struct BackAcceptedReservationProjection {
    world: CoreIssuedWorldSealProjection,
    router: RouterIdentity,
    history: HistoryIncarnation,
    request: NavigationRequestId,
    transaction: TransactionId,
    cleanup_batch: CleanupBatchId,
    branch: CoreAcceptedBranchReservationProjection,
}

// bevy_ui_router_core::test_support::back_capacity_order; representation is non-normative.
struct BackCapacityOrderObserverSeal;
struct BackCapacityProducerSeal;

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub struct BackCapacityOrderObserver {
    /* read-only journal handle */
    private: BackCapacityOrderObserverSeal,
}
#[cfg(any(test, feature = "test-support"))]
pub struct BackCapacityOrderSnapshot {
    /* immutable ordered projection */
    private: BackCapacityOrderObserverSeal,
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct BackCapacityIssuerProducer { private: BackCapacityProducerSeal }
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct BackCapacityCoreProducer { private: BackCapacityProducerSeal }
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct BackCapacityFacadeProducer { private: BackCapacityProducerSeal }
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackCapacityIssuerEvent { Issued, Retired, TransferredNoTarget, BoundCanonicalZero }
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackCapacityOrderEvent { EcsIssuer(BackCapacityIssuerEvent), CoreAllocatorAttempt, CoreAllocatorFailure, FacadeTypedReturn }
#[cfg(any(test, feature = "test-support"))]
impl BackCapacityOrderObserver {
    pub fn snapshot(&self) -> BackCapacityOrderSnapshot;
    pub fn observes_same_journal(&self, other: &Self) -> bool;
}
#[cfg(any(test, feature = "test-support"))]
impl BackCapacityOrderSnapshot {
    pub fn events(&self) -> &[(u64, BackCapacityOrderEvent)];
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn issue_back_capacity_order_channels() -> (BackCapacityOrderObserver, BackCapacityIssuerProducer, BackCapacityCoreProducer, BackCapacityFacadeProducer);
#[cfg(any(test, feature = "test-support"))]
impl BackCapacityIssuerProducer { pub fn record(&mut self, event: BackCapacityIssuerEvent); }
#[cfg(any(test, feature = "test-support"))]
impl BackCapacityCoreProducer { pub fn allocator_attempt(&mut self); pub fn allocator_failure(&mut self); }
#[cfg(any(test, feature = "test-support"))]
impl BackCapacityFacadeProducer { pub fn typed_return(&mut self); }

// Production: every exact RouterEntry owns one issuer signed by the World domain issuer.
struct RouterEntry {
    /* existing fields + one exact-entry private slot; no parallel table */
    back_capacity_enqueue_retire_residual: Option<BackCapacityEnqueueRetireResidual>,
}
impl RouterEntry {
    pub(crate) fn install_back_capacity_issuer(
        &mut self,
        domain: &mut BackCapacityDomainIssuer,
    ) -> Result<(), BackCapacityIssuerInstallError>;
}

// RouterNavigation is the unique World Resource. It owns the domain issuer, never a per-entry issuer.
pub struct RouterNavigation {
    /* existing navigation state + one BackCapacityDomainIssuer; each exact entry owns its issued BackCapacityIssuer */
}
impl RouterNavigation {
    pub(crate) fn new() -> Self; // initializes BackCapacityDomainIssuer::new()
}

// Test support: each exact RouterEntry boundary consumes its producer exactly once; no installer returns it.
#[cfg(any(test, feature = "test-support"))]
impl RouterEngine { #[doc(hidden)] pub fn install_back_capacity_core_producer(&mut self, producer: BackCapacityCoreProducer); }
#[cfg(any(test, feature = "test-support"))]
impl RouterEntry {
    pub(crate) fn install_back_capacity_test_producers(
        &mut self,
        issuer: BackCapacityIssuerProducer,
        facade: BackCapacityFacadeProducer,
    );
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub enum BackCapacityOrderInstallError { UnknownRouter, ForeignRouter, AlreadyInstalled }
// ECS owner test-support module; not a RouterNavigation associated API or facade re-export.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub fn testing_install_back_capacity_order_observer(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
) -> Result<BackCapacityOrderObserver, BackCapacityOrderInstallError>;

pub enum EnqueueBackErrorKind {
    AuthorityRecovering,
    Exhausted,
}

pub struct AcceptedBackReservation {
    request: NavigationRequestId,
    accepted: BackAcceptedReservationView,
}

impl AcceptedBackReservation {
    pub fn request(&self) -> NavigationRequestId;
    pub fn into_parts(self) -> (NavigationRequestId, BackAcceptedReservationView);
}
```

- `BackCapacityDomainIssuer::issue` is the only non-reusable domain constructor and `reserve` is the only permit constructor; their raw fields have no sibling constructor or accessor
- issuer installation signs only the exact entry domain and creates no fixed pool. Each issuer retains an entry-local persistent cell table with no hidden concurrency limit; cells transition in place through `Free | Outstanding | Leased`, while permit、slot and ledger owners carry only the exact linear cell lease
- `reserve` reuses a `Free` cell or performs grow-on-demand. Growth must complete every fallible reservation for one fully initialized vacant cell and its exact outstanding registry entry before owner/generation mint. A real growth failure returns `LedgerSlotUnavailable` before Core allocator reach, with no mint and no semantic state write
- after mint, bind、exact retire、no-target transfer and canonical-zero perform no allocation. Retire burns the owner and returns the same cell lifecycle to `Free`; `ReusableFree` advances generation，while `PermanentlyRetired` preserves the final generation and sets the orthogonal private non-reusable marker. `reserve` selects only reusable `Free` cells，so a generation with no successor is permanently retired without adding a fourth lifecycle state and later reserve grows a fresh cell. No-target keeps the same cell `Leased`
- `retire_unbound(..., EnqueueRejected)` handles Core token drift/allocator failure and preserves the original reservation on foreign/stale/replay; its success burns the identity before facade return
- if `retire_unbound` theoretically returns `Rejected { owner, rejection, error }`
  - return the original `EnqueueRejected` in the outcome without requiring `Copy` or caller-side storage/reconstruction
  - move that owner、the original `EnqueueRejected` and retire error into the exact entry's single private fail-closed residual
  - never panic、drop the owner or map this residual path to ordinary exhaustion
  - reuse the exact-entry fail-closed gate's `HostUnavailable` typed return for the current and every later Back while the slot is occupied
  - keep `IdentityExhausted` for a normal Core allocator failure whose exact retire succeeds
- bound retirement accepts only target/contradiction/authority/permanent disposition; `EnqueueRejected` remains the separate argument of `retire_unbound`. The private live registry records exact identity plus `Outstanding | Leased` state. Bound retirement、no-target transfer、no-target retirement and canonical-zero bind each take `&mut issuer` plus `&owner` and either return `Err` before constructing any guard/successor, or return a lifetime-bound `ExactBackCapacityCommitGuard { transition, owner: &owner }` together with one private non-authorizing prepared successor. `BackCapacityCommitTransition` holds the exact `OccupiedEntry`、exact cell `&mut` and all precomputed next data as `OutstandingRetire | OutstandingToLeased | LeasedRetire | LeasedToCanonicalZeroSlot`
- dropping an uncommitted guard is zero-write. `commit(self)` only matches its prepared transition: retirement variants remove the exact occupied live entry、set lifecycle state to `Free` and apply precomputed `ReusableFree { next_generation } | PermanentlyRetired { generation }` reuse metadata；transfer retains that same live entry and updates it plus the cell in place from `Outstanding` to `Leased`；canonical-zero removes the exact `Leased` live entry and moves the exact cell lease into the prepared slot state. It performs no owner/key lookup、key-based remove、Vec index/query、allocation、validation、`expect`、`unwrap`、panic or unsafe；the only permitted removal is the already-held `OccupiedEntry`
- issuer preflight `Err` means no guard/successor exists，Core is not called and the helper directly returns typed `CapacityRejected` with the complete owner. After successful preflight, a Core receiver mismatch calls the callback zero times、drops the uncommitted guard and prepared successor、ends their borrows and returns typed `ReceiverMismatch` with the still-complete owner. An exact Core outcome proves the callback consumed `guard.commit()` once. Retirement then drops the stale owner and seals prepared evidence；transfer moves the stale `BoundBackCapacityReservation` into `PreparedNoTargetCapacityPermit::into_permit_after_exact_receiver`；canonical-zero moves the stale `NoTargetCapacityPermit` into `PreparedManifestNeutralLedgerSlot::into_slot_after_exact_receiver`. Prepared successors retain their move-only payload and are non-authorizing before the exact outcome；neither guard nor successor enters an outcome、field、queue or later helper
- all retire/transfer rejection variants return the original move-only owner; foreign domain、stale generation and replay cannot become `()`/bool, while success returns only non-authorizing retirement evidence and permanently burns the identity
- `RouterNavigation` is already the unique ECS World Resource, so it owns the single persistent `back_capacity_domain_issuer` initialized by `RouterNavigation::new`. It owns no per-entry issuer field. During router install, construct the entire `RouterEntry` locally, ask the domain issuer to sign/install its non-reconstructible `BackCapacityIssuer`, and push the entry only after every install step succeeds. Failure maps `BackCapacityIssuerInstallError::Reserve` to existing `IntegrationError::Exhausted` and impossible local-entry `AlreadyInstalled` to existing `IntegrationError::Invalid`; it leaves no ghost entry or partially installed entry
- `install_back_capacity_issuer` first checks the exact entry's issuer slot. `AlreadyInstalled` returns before `BackCapacityDomainIssuer::issue`, so a second install cannot consume a domain identity、replace the issuer or alter its outstanding registry; only a vacant slot may map `issue` failure to `BackCapacityIssuerInstallError::Reserve`. This production error is separate from test-only journal-channel `BackCapacityOrderInstallError::AlreadyInstalled`
- under `test-support`, only the ECS owner test-support free function installs the observer and the public audit calls that owner function directly; it is not a `RouterNavigation` associated API, is not facade-re-exported and does not change the facade associated-API golden. `BackCapacityOrderObserver::observes_same_journal` performs only opaque identity equality such as `Arc::ptr_eq`; it exposes no raw journal/domain identity or producer and cannot record events or authorize lifecycle work. Tests receive only observer/error, while Core defines only layer-neutral channels and never depends on ECS cleanup types
- set the ECS feature exactly to `test-support = ["bevy_ui_router_core/test-support"]`, and update the existing dependency-boundary gate to require that exact forwarding edge in this micro-slice. Core channel definitions and `issue_back_capacity_order_channels` live in `crates/bevy_ui_router_core/src/test_support.rs`; ECS only installs their sealed producers. `app_ext.rs` adds exactly one crate-private `RouterHandle` domain predicate so the installer can distinguish a legitimately obtained foreign handle from same-domain unknown without exposing raw domain/router values, a public constructor or a forged-handle path. No plugin or facade change is required

```toml
# crates/bevy_ui_router_ecs/Cargo.toml
[features]
default = []
test-support = ["bevy_ui_router_core/test-support"]
```

Implement Step 6 as these ordered RED → GREEN micro-slices; do not batch later capacity semantics into the first passing implementation

1. **Exact domain install**
   - RED `back_capacity_issuer_second_install_is_zero_write`
   - install one vacant production issuer on an exact entry, attempt a second install and assert `AlreadyInstalled` before `BackCapacityDomainIssuer::issue`
   - GREEN only after the installed issuer is not replaced、no domain identity is consumed and the next different entry receives the immediately next domain; this micro-slice does not reserve an outstanding owner

2. **Growth、reclaim and ABA**
   - RED `back_capacity_growth_failure_is_pre_mint_zero_write`
   - its only fault input passes `usize::MAX` as the additional capacity to the production backing storage's real `try_reserve_exact`, which must return `CapacityOverflow`; it cannot rely on OOM、allocator policy or inject `LedgerSlotUnavailable`、`IdentityExhausted` or any classification
   - the test must fail if that production `try_reserve_exact` predicate is removed or bypassed
   - assert `IdentityExhausted`、no owner/generation mint、no new cell/outstanding entry、no Core allocator reach and unchanged request/queue/public/World/issuer semantic state
   - RED `back_capacity_reclaimed_cell_advances_generation_and_rejects_aba`
   - reserve and exact-retire one real cell, reserve again and assert the same reusable cell has a new owner/generation; stale owner/generation/evidence cannot retire、transfer or authorize the current owner and every rejection returns the current move-only owner
   - also drive the real no-successor predicate: a generation-exhausted cell is never reused and the next successful reserve grows a different cell
   - GREEN only after growth preflights every fallible capacity before mint and reclaim/generation transitions are allocation-free
   - after those growth/reclaim gates are GREEN, add `back_capacity_issuer_second_install_preserves_outstanding` using one real reserved owner; assert a second install leaves the exact cell、outstanding owner and generation unchanged

3. **Read-only cross-crate observer install**
   - this micro-slice installs empty channels only; both independent snapshots stay empty and record none of `Issued | Retired | TransferredNoTarget | BoundCanonicalZero | CoreAllocatorAttempt | CoreAllocatorFailure | FacadeTypedReturn`. All real order-event production remains owned by micro-slice 4
   - RED `back_capacity_order_observer_installs_across_crates_read_only` in the public audit installs observers for two exact routers in one App, asserts observer A vs its clone is same-journal、A vs independently installed B is different-journal、both snapshots are independently empty、the second install returns exact `AlreadyInstalled`, and a legitimately obtained foreign handle from a second App is rejected with zero write
   - RED `back_capacity_order_observer_same_domain_unknown_is_zero_write` stays in ECS unit coverage under `test-support` and uses the existing private handle fixture to exercise the same owner free function; assert the Core factory closure call count remains zero and the exact entry plus all producer slots remain unchanged. Do not add a public forged-handle constructor or a `cfg(test)` installer alternative
   - the sole installer order is: (1) reject foreign domain; (2) resolve the exact same-domain entry or reject unknown; (3) preflight Core、issuer and facade producer slots all vacant; (4) reject any occupied slot as `AlreadyInstalled`; (5) call the Core factory exactly once; (6) infallibly move all three sealed producers into that same exact entry; (7) return the observer
   - GREEN only when no validation、allocation or fallible installer follows factory creation, each producer is sealed and single-install, tests receive only observer/error, every rejection is zero-write with no partial journal installation, and the existing dependency-boundary gate requires `test-support = ["bevy_ui_router_core/test-support"]`

4. **Real Core allocator failure order**
   - RED `back_capacity_precedes_core_reservation_failure_evidence`
   - this is the first micro-slice that records real order events; it must not weaken the empty-channel evidence owned by micro-slice 3
   - record production events only at these boundaries
     - `Issued` after successful mint
     - `CoreAllocatorAttempt` after exact admission refreeze and immediately before the real `reserve_navigation_branch`
     - `CoreAllocatorFailure` only when that reserve returns `Err`
     - `Retired` after exact burn、generation advance and cell reclaim succeed
     - `FacadeTypedReturn` at the outermost typed return after retirement
   - RED `back_capacity_retire_rejection_owner_is_retained_fail_closed`
     - the seam changes only the production retire predicate's real input fact and cannot inject `Rejected`、classification or typed return
     - assert the residual retains the exact owner、original `EnqueueRejected` and retire error
     - assert the current Back and every later Back return `HostUnavailable` while the owner remains present、unchanged and never replaced or cleared
   - RED Core companion `back_core_allocator_failure_is_post_refreeze_private_zero_write`
     - for every `TestingReservationCounter::ALL` value, use a fresh engine and fresh observer
     - after applying the real counter seam, freeze the production-semantic full private snapshot including allocator、queue、records、history、state/current/action/mount and prove allocator exhaustion is zero-write across the enqueue call
     - exclude the test-support observer journal from that snapshot so journal evidence cannot hide production state mutation
     - prove stale/token drift is also zero-write and records neither allocator attempt nor allocator failure
     - do not add a Core private snapshot accessor to ECS
   - for every `TestingReservationCounter::ALL` value
     - use a fresh App and fresh observer
     - assert the exact sequence `Issued → CoreAllocatorAttempt → CoreAllocatorFailure → Retired → FacadeTypedReturn`
     - assert the exact minted owner is burned and the reclaimed cell generation advances
     - assert ECS request/bound/pending collections remain empty and public snapshot、World fingerprint remain unchanged
   - GREEN permits only the named real counter-exhaustion seam and real retire-predicate fact seam; each changes only its production predicate input and never injects `Rejected`、classification or typed return, and neither adds a second publisher、public API or later accepted/resolution behavior

5. **Queued accepted-reservation safe stop**
   - wire the accepted view and single Core queue only after micro-slices 1–4 are GREEN
   - stop at the queued unresolved owner boundary described below
   - first strengthen the existing assertions, then run the gates as GREEN evidence-strengthening only; this does not fabricate a production RED or alter production semantics
   - Core evidence companion `back_resolution_action_blocks_next_collect`: before the first `collect`, the single private queue linearly holds the complete `ReservedBranchAttempt`, and its request remains `Queued` with `requested=None`、`matched=None` and an empty trace, with no action/current/history/presentation publication. After `collect`, retain the existing proof that the one `BackResolution` action blocks the next collect without consuming or overwriting the queued Back
   - ECS evidence companion `accepted_back_reserves_ecs_capacity_before_core_request_identity`: after a successful enqueue, the shared order journal proves exactly `Issued → CoreAllocatorAttempt`, with no `CoreAllocatorFailure`、`Retired` or `FacadeTypedReturn`; the exact entry retains its matching `BoundBackCapacityReservation` beside the exact queued request and persistent `Outstanding` cell; queued request projections remain `requested=None`、`matched=None` and empty trace, with no pending resolution or cleanup and zero public router/history/presentation writes
   - keep the accepted-view negative/static contract: it has no target、normal attempt、`Entity` or manifest authority, proved by the existing static/negative gates rather than a new production accessor
   - these are Step 6 evidence-strengthening gates only: they do not alter production semantics or add a production seam、second publisher or later resolution behavior

- acceptance infallibly combines the ECS owner with the view returned by `AcceptedBackReservation::into_parts` into `BoundBackCapacityReservation`; at resolution, Core Matched returns the same accepted view beside one verified branch owner, so ECS recomposes that view with the original capacity into the same bound reservation. Mismatch returns raw view + raw handoff so ECS likewise reconstructs the original bound reservation
- `BackAcceptedReservationProjection` is independent from `HistoryExecutionBindingProjection` and binds only exact Core world seal projection、router/history incarnation、request、transaction、cleanup batch 与 opaque accepted branch-reservation identity/pool
- `BackAcceptedReservationView` has private fields, no public constructor and no method that can mint/reconstruct a Core owner or execution binding
- ECS consumes `AcceptedBackReservation::into_parts`, moves the view into capacity binding, then returns the copied request id to facade only after that move completes
- `request()` is read-only convenience and never authorizes binding or publication
- the accepted view binds no normal attempt、destination、eventual host/slot/frame/task/generation or branch selector
- the accepted view carries no authority to consume the branch pool、resolve a target or construct any later owner
- `AcceptedBackReservation` exposes only non-authorizing accepted lineage

The micro-slice 3 sequence above is the only observer installer path; there is no inherent or facade installer alternative. Micro-slice 4 may record through the already-installed exact-entry slots but cannot change observer identity or installation semantics

Add installed immutable maximum depth without exposing it through facade

```rust
impl RouteTree {
    pub(crate) fn max_branch_depth(&self) -> usize;
}

impl RouteMatcher {
    pub(crate) fn max_branch_depth(&self) -> usize {
        self.tree.max_branch_depth()
    }
}
```

`RouteTree::max_branch_depth` walks declared parent/child relationships from the root, counts the root as depth 1 and fails construction if the immutable tree cannot yield a finite nonzero maximum

Add the Core reservation method only after maximum depth exists

```rust
impl RouterEngine {
    pub fn enqueue_back(
        &mut self,
        admission: BackAdmissionToken,
    ) -> Result<AcceptedBackReservation, EnqueueBackErrorKind>;
}
```

- `enqueue_back` consumes and exact-refreezes current availability/incarnation/live revision against the token before reserving Core identities
- token drift returns `AuthorityRecovering` with no Core request
- Core allocator exhaustion consumes the token and returns `Exhausted`

Upgrade the Task 3 queue without creating a second queue

```rust
enum QueuedReservation {
    Branch(ReservedBranchAttempt),
    RequestOnly(NavigationRequestId),
}

enum NavigationCommand {
    Push(RouteLocation),
    Replace(RouteLocation),
    Back,
}

struct QueuedNavigation {
    request: NavigationRequestId,
    command: NavigationCommand,
    outcome: Option<QueuedMatchOutcome>,
    reservation: QueuedReservation,
}
```

Back enqueue order is exact

1. validate router identity and availability
2. call Core `prepare_back_admission` to validate the published stamp and receive a non-Clone token
3. return `NoBackEntry` without a token or allocation when `can_back=false`
4. reserve ECS manifest-neutral capacity
5. call Core `enqueue_back(token)`, which uses existing `reserve_navigation_branch(router, matcher.max_branch_depth())`
6. bind ECS capacity to the returned accepted reservation view by an infallible move

If step 4 growth or owner/generation preflight fails, return `IdentityExhausted` before mint and before any Core allocator attempt, with exact zero semantic state change

If step 5 fails, burn the ECS reservation identity and release its capacity with zero request/public state

- allocator exhaustion maps to `IdentityExhausted`
- prepare-to-enqueue token drift maps to `AuthorityRecovering`

Step 6 product is exact

- Core retains the full `ReservedBranchAttempt` owner in the single queue
- ECS retains `BoundBackCapacityReservation { capacity, accepted }` beside the exact queued request while its persistent cell remains exact `Outstanding`
- request remains queued、unresolved and `requested=None`
- accepted view binding is the only Core→ECS handoff

Step 6 must stop here

- do not consume the accepted owner or any reserved prefix
- do not call live history preview、match a back target or freeze a destination
- do not create target、no-target、contradiction、history capability、continuation or physical-classification owner
- do not publish running phase、rejection、cleanup、terminal、presentation or history changes

Run only Step 6-owned gates

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_issuer_second_install_is_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_growth_failure_is_pre_mint_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_reclaimed_cell_advances_generation_and_rejects_aba -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_issuer_second_install_preserves_outstanding -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --features test-support --lib back_capacity_order_observer_same_domain_unknown_is_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit back_capacity_order_observer_installs_across_crates_read_only -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --features test-support --lib back_capacity_precedes_core_reservation_failure_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --features test-support --lib back_capacity_retire_rejection_owner_is_retained_fail_closed -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --features test-support --lib back_core_allocator_failure_is_post_refreeze_private_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --features test-support --lib back_resolution_action_blocks_next_collect -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --features test-support --lib accepted_back_reserves_ecs_capacity_before_core_request_identity -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib no_back_entry_precedes_identity_exhaustion -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test dependency_boundary ecs_manifest_locks_the_dependency_boundary -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
```

Expected: all fourteen gates exit `0`; admission-target、exact live verify、max-depth prefix/unused burn、duplicate-entry publication and every resolution/physical test remain RED for Step 8

- [ ] **Step 7: Scaffold the complete typed boundary and write RED tests**

Before tests, add fail-closed zero-write bodies for these complete integration signatures; owners are non-Clone, scope/evidence are non-authorizing

```rust
pub(crate) struct BackDestination { source: FrozenHistorySource, target_entry: HistoryEntryId, target_cursor: usize, target_location: RouteLocation }
pub(crate) struct BackPreviewContradiction { expected: FrozenHistorySource, observed: FrozenHistorySource }

pub(crate) enum BackPreview {
    Target(BackDestination),
    NoTarget(FrozenHistorySource),
    Contradiction(BackPreviewContradiction),
}

impl CanonicalHistory {
    pub(crate) fn preview_back(&self) -> BackPreview;
    pub(crate) fn freeze_back(&self, execution: HistoryExecutionBinding, destination: BackDestination)
        -> Result<HistoryCommitCapability, HistorySourceDrift>;
}

pub struct BackResolutionScopeView { inner: BackResolutionScopeProjection }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackResolutionEvidence { inner: BackResolutionEvidenceProjection }
#[doc(hidden)]
pub enum BackResolutionEvidenceClassification { Current, Foreign, ConsumedReplay }
pub(crate) enum BackResolutionRecordState {
    Current(BackResolutionEvidence),
    Consumed(BackResolutionEvidence),
}
pub(crate) struct NavigationRequestRecord {
    // existing fields
    back_resolution: Option<BackResolutionRecordState>,
}
pub struct BackTargetExecutionOwner { inner: CoreBackTargetExecutionOwner }
pub struct BackTargetPermanentOwner { inner: CoreBackTargetPermanentOwner }
pub struct BackNoTargetCandidate { inner: CoreBackNoTargetCandidate /* raw resolution handoff only */ }
pub struct VerifiedBackTargetExecutionOwner { inner: CoreVerifiedBackTargetExecutionOwner }
pub struct VerifiedBackNoTargetCandidate { inner: CoreVerifiedBackNoTargetCandidate /* sole opaque unresolved lifecycle lineage owner */ }
pub struct VerifiedBackResolutionContradiction { inner: CoreVerifiedBackResolutionContradiction }
#[doc(hidden)]
pub struct BackNoTargetSourceDriftOwner { inner: CoreBackNoTargetSourceDriftOwner }
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackNoTargetSourceDriftEvidence { inner: CoreBackNoTargetSourceDriftProjection }
pub struct BackResolutionContradiction { inner: CoreBackResolutionContradiction }
pub struct BackContradictionTerminalEvidence { inner: CoreBackContradictionTerminalProjection }

pub enum BackResolutionOwnerHandoff {
    Target(BackTargetExecutionOwner),
    NoTarget(BackNoTargetCandidate),
    Contradiction(BackResolutionContradiction),
}

pub enum VerifiedBackResolutionOwnerHandoff {
    Target(VerifiedBackTargetExecutionOwner),
    NoTarget(VerifiedBackNoTargetCandidate),
    Contradiction(VerifiedBackResolutionContradiction),
}
pub struct VerifiedBackResolutionHandoff {
    accepted: BackAcceptedReservationView,
    handoff: VerifiedBackResolutionOwnerHandoff,
}
pub enum BackResolutionVerifyOutcome {
    Matched(VerifiedBackResolutionHandoff),
    Mismatch { accepted: BackAcceptedReservationView, handoff: BackResolutionOwnerHandoff, evidence: BackResolutionEvidence },
}
impl VerifiedBackResolutionHandoff {
    pub fn into_parts(self) -> (BackAcceptedReservationView, VerifiedBackResolutionOwnerHandoff);
}

impl BackResolutionOwnerHandoff {
    pub fn scope(&self) -> BackResolutionScopeView;
    pub fn evidence(&self) -> BackResolutionEvidence;
}

pub enum EngineAction {
    // existing variants
    BackResolution(BackResolutionOwnerHandoff),
}

#[doc(hidden)]
pub enum BackTargetResumeAction {
    FrameTask(RootFrameTask),
    MountRequest(MountRequest),
    Commit(CommitTicket),
}
pub struct BackTargetResume { /* action + existing HistoryExecutionBindingView + continuation */ }
#[doc(hidden)]
pub enum BackTargetStart {
    Ready(BackTargetResume),
    FailureCleanup(FailureCleanupRequest),
}
pub enum BackTargetCoreResumeOutcome {
    Resumed(BackTargetStart),
    Permanent(BackTargetPermanentOwner),
    Mismatch { owner: VerifiedBackTargetExecutionOwner, evidence: BackResolutionEvidence },
}
pub struct BackTargetPermanentTerminalEvidence { inner: CoreBackTargetPermanentTerminalProjection }
#[doc(hidden)]
pub enum BackNoTargetSourceRefreezeOutcome {
    Unchanged(VerifiedBackNoTargetCandidate),
    Drift(BackNoTargetSourceDriftOwner),
    Mismatch { candidate: VerifiedBackNoTargetCandidate, evidence: BackResolutionEvidence },
}
impl BackNoTargetSourceDriftOwner {
    pub fn evidence(&self) -> BackNoTargetSourceDriftEvidence;
}
#[doc(hidden)]
pub struct BackNoTargetSourceDriftTerminalEvidence { inner: CoreBackNoTargetSourceDriftTerminalProjection }
#[doc(hidden)]
pub enum BackNoTargetSourceDriftSettlementOutcome {
    Settled(BackNoTargetSourceDriftTerminalEvidence),
    Mismatch { owner: BackNoTargetSourceDriftOwner, evidence: BackNoTargetSourceDriftEvidence },
}
pub enum BackContradictionCoreOutcome {
    Failed(BackContradictionTerminalEvidence),
    Mismatch { owner: VerifiedBackResolutionContradiction, evidence: BackResolutionEvidence },
}
pub enum BackTargetPermanentSettlementOutcome {
    Settled(BackTargetPermanentTerminalEvidence),
    Mismatch { owner: BackTargetPermanentOwner, evidence: BackResolutionEvidence },
}
impl BackTargetResume {
    pub fn into_parts(self)
        -> (BackTargetResumeAction, HistoryExecutionBindingView, LifecycleContinuationCapacity);
}

impl RouterEngine {
    pub fn verify_back_resolution_handoff(&mut self, accepted: BackAcceptedReservationView, handoff: BackResolutionOwnerHandoff) -> BackResolutionVerifyOutcome;
    #[doc(hidden)]
    pub fn classify_back_resolution_evidence(
        &self,
        evidence: &BackResolutionEvidence,
    ) -> BackResolutionEvidenceClassification;
    #[doc(hidden)]
    pub fn resume_back_target_with_retirement(
        &mut self,
        owner: VerifiedBackTargetExecutionOwner,
        commit_retirement: impl FnOnce(),
    ) -> BackTargetCoreResumeOutcome;
    pub fn settle_back_target_permanent_exact(
        &mut self,
        owner: BackTargetPermanentOwner,
    ) -> BackTargetPermanentSettlementOutcome;
    #[doc(hidden)]
    pub fn refreeze_back_no_target_source_with_transfer(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
        commit_transfer: impl FnOnce(),
    ) -> BackNoTargetSourceRefreezeOutcome;
    #[doc(hidden)]
    pub fn settle_back_no_target_source_drift_with_retirement(
        &mut self,
        owner: BackNoTargetSourceDriftOwner,
        commit_retirement: impl FnOnce(),
    ) -> BackNoTargetSourceDriftSettlementOutcome;
    #[doc(hidden)]
    pub fn fail_back_resolution_contradiction_with_retirement(
        &mut self,
        owner: VerifiedBackResolutionContradiction,
        commit_retirement: impl FnOnce(),
    ) -> BackContradictionCoreOutcome;
}
```

`BackDestination` and `BackPreviewContradiction` keep the exact source/entry/cursor/location fields from §2.2 and §5.3. These Core-private types may name `FrozenHistorySource`; ECS cannot import that type or inspect candidate internals. `verify_back_resolution_handoff` consumes accepted lineage plus the raw handoff and on success returns `VerifiedBackResolutionHandoff`; its only split yields the same accepted view plus exactly one branch-specific verified owner, never a raw `BackResolutionOwnerHandoff`. Every later Core consumer accepts only the corresponding verified branch owner. Mismatch alone returns the raw accepted view、raw handoff and evidence so ECS can restore the untouched capacity binding

`NavigationRequestRecord` 固定保存 `back_resolution: Option<BackResolutionRecordState>`，queued Back 在 raw handoff 生成前为 `None`，非 Back 始终为 `None`，生成 exact observed evidence 后且 action 对外可见前安装 `Some(Current(evidence))`，仅 exact verify `Matched` 原位改为 `Some(Consumed(evidence))`，不新增 collection 或消费后 allocation

`classify_back_resolution_evidence` 是该事实的唯一 classifier，retained path 先比较 supplied evidence 与 stored evidence exact equality，再按状态返回 `Current | ConsumedReplay`。foreign mismatch 前保存的 observed-form evidence 仍为 `Current`，Core mismatch diagnostic evidence 是 non-observed-form 并返回 `Foreign`。record eviction 后只有 exact opaque scope、`evicted_through` 与 `highest_issued` 共同匹配才返回 `ConsumedReplay`，`highest_issued` 单独不足

`Current` record 不得 terminal 或 evict，只有 `Consumed` 可进入 terminal retention/eviction

Target resume and contradiction terminal each accept a caller-provided locally infallible `FnOnce`. Issuer preflight `Err` creates no guard/successor、does not call Core and returns `CapacityRejected` with the complete bound owner. Successful preflight yields a lifetime-bound guard plus private prepared retirement evidence while the complete bound owner remains outside the closure. Core first checks the target receiver identity; `Mismatch` returns the verified owner before preparation and calls the closure zero times, so dropping the uncommitted guard/evidence is zero-write and ECS returns `ReceiverMismatch` after recomposing the untouched bound owner. For an exact target receiver, Core refreezes history source、host/top-slot and active/child-slot baseline, then completes matcher、reconciliation、first-action and mount-issue preparation into private `PreparedReady | PreparedFailureCleanup | PreparedPermanent`. Only after preparation succeeds as one typed branch does Core call the closure exactly once；that closure only moves `guard.commit()`. After the exact outcome returns, ECS drops the now-stale bound owner and converts the prepared non-authorizing evidence into `BackCapacityRetirementEvidence`. Same-engine source/host/baseline drift and generation/depth contradiction are private typed Permanent/AuthorityLost, not Mismatch. Contradiction terminal retains the same zero/one callback rule. No guard、prepared successor or closure is returned or stored

`BackTargetStart` separates `Ready(BackTargetResume)` from `FailureCleanup(FailureCleanupRequest)`. `BackTargetResume` linearly carries one `BackTargetResumeAction` beside the existing execution binding and continuation. Real reconciliation may return `FrameTask` or a nested `MountRequest`; legitimate duplicate/equivalent or pure-truncate zero-entering reconciliation returns the real `CommitTicket` without synthesizing a task. Initial Ready `MountRequest` remains whole in `ResumeBlocked::Start` with the generic mount field empty until its target action slice exists. Mount issue exhaustion returns the independent FailureCleanup branch and never enters the normal resume action enum. These doc-hidden types are not reexported by the facade

Every callback-bearing `*_with_*` method is a `#[doc(hidden)]` trusted ECS integration API and is absent from facade reexports. This follows the existing `ValidatedCommit::publish(FnOnce())` boundary: the callback is not authority; the verified/prepared Core owner is authority. Correctness relies on one audited ECS callsite whose conditional callback captures only one lifetime-bound guard and calls `guard.commit()` once. The static gate rejects any owner-by-value capture、owner/key lookup、key-based remove、Vec index/query、allocation、validation、`expect`、`unwrap`、panic、unsafe or hook/observer-triggering write in that callback. It separately audits operation-specific transition behavior：Outstanding/Leased retire consumes the already-held live entry、returns lifecycle to `Free` and applies `Reusable | PermanentlyRetired` metadata；transfer retains that same entry and updates it to `Leased`；canonical-zero consumes the `Leased` entry and moves its exact cell lease into the slot. Retirement evidence seals only after stale-owner drop，while transfer/canonical-zero successor conversion must consume the stale owner by value in the exact outer outcome. Rust visibility/types are not claimed to defeat a malicious direct Core caller; the supported facade/adapter surface cannot obtain the verified owners or name/call these methods

No-target transfer/refreeze uses the same guard/prepared-successor pattern. Issuer preflight `Err` creates no guard/successor、does not call Core and returns `CapacityRejected` with the complete bound owner. After successful preflight, Core exact-matches receiver + verified candidate before preparation；`Mismatch` returns the verified candidate, calls the closure zero times, drops the uncommitted transfer guard/successor and returns `ReceiverMismatch` with the borrowed bound reservation untouched. The exact receiver first rebuilds and exact-refreezes the source into private `PreparedUnchanged | PreparedDrift`；only after that branch is complete does it call `guard.commit()` once，and the Core tail installs or moves the prepared branch after the callback. The exact outer outcome then moves the stale bound into `PreparedNoTargetCapacityPermit::into_permit_after_exact_receiver` to construct `NoTargetCapacityPermit` beside the unchanged candidate or opaque drift. Drift settlement uses the same distinct preflight-Err versus Core-mismatch ordering before its lifetime-bound retirement callback；wrong engine returns raw drift with the borrowed permit still available for recomposition. No prepared successor or drift owner crosses a helper boundary

Define the ECS exact-take boundary

```rust
pub(crate) struct BoundBackTargetResolution { owner: VerifiedBackTargetExecutionOwner, reservation: BoundBackCapacityReservation }
pub(crate) struct BoundBackNoTargetResolution { candidate: VerifiedBackNoTargetCandidate, reservation: BoundBackCapacityReservation }
pub(crate) struct BoundBackContradictionResolution { owner: VerifiedBackResolutionContradiction, reservation: BoundBackCapacityReservation }

pub(crate) enum BoundBackResolutionHandoff {
    Target(BoundBackTargetResolution),
    NoTarget(BoundBackNoTargetResolution),
    Contradiction(BoundBackContradictionResolution),
}

pub(crate) enum TakeBackResolutionOutcome {
    Matched(BoundBackResolutionHandoff),
    Mismatch { reservation: BoundBackCapacityReservation, handoff: BackResolutionOwnerHandoff, evidence: BackResolutionEvidence },
}

pub(crate) enum BackResolutionEvidenceDisposition { Current, Foreign, ConsumedReplay }
pub(crate) enum BackTargetResumeOutcome { Resumed { start: BackTargetStart, retirement: BackCapacityRetirementEvidence }, Permanent { owner: BackTargetPermanentOwner, retirement: BackCapacityRetirementEvidence }, CapacityRejected { bound: BoundBackTargetResolution, error: BackCapacityRetireError }, ReceiverMismatch { bound: BoundBackTargetResolution, evidence: BackResolutionEvidence } }
pub(crate) enum BackNoTargetBeginOutcome { Begun(BackPreparationOwner), Permanent(BackNoTargetSourceDriftPermanentOwner), CapacityRejected { bound: BoundBackNoTargetResolution, error: BackCapacityRetireError }, ReceiverMismatch { bound: BoundBackNoTargetResolution, evidence: BackResolutionEvidence } }
pub(crate) enum BackContradictionOutcome { Failed { terminal: BackContradictionTerminalEvidence, retirement: BackCapacityRetirementEvidence }, CapacityRejected { bound: BoundBackContradictionResolution, error: BackCapacityRetireError }, ReceiverMismatch { bound: BoundBackContradictionResolution, evidence: BackResolutionEvidence } }

impl RouterEntry {
    pub(crate) fn take_back_resolution_handoff(
        &mut self,
        handoff: BackResolutionOwnerHandoff,
    ) -> TakeBackResolutionOutcome;
    pub(crate) fn validate_back_resolution_evidence(
        &self,
        evidence: &BackResolutionEvidence,
    ) -> BackResolutionEvidenceDisposition;
}

pub(crate) fn resume_bound_back_target(
    entry: &mut RouterEntry,
    bound: BoundBackTargetResolution,
) -> BackTargetResumeOutcome;
pub(crate) fn settle_back_target_permanent(
    entry: &mut RouterEntry,
    owner: BackTargetPermanentOwner,
) -> BackTargetPermanentSettlementOutcome;
pub(crate) fn begin_bound_back_no_target(
    entry: &mut RouterEntry,
    bound: BoundBackNoTargetResolution,
) -> BackNoTargetBeginOutcome;
pub(crate) fn fail_bound_back_contradiction(
    entry: &mut RouterEntry,
    bound: BoundBackContradictionResolution,
) -> BackContradictionOutcome;
```

`take_back_resolution_handoff` temporarily separates capacity from its accepted view only to call Core verify. Before that exact consumption, ECS saves the handoff's Clone、non-authorizing `BackResolutionEvidence`. On `Matched`, `VerifiedBackResolutionHandoff::into_parts` returns the accepted view plus a sealed verified branch owner; ECS infallibly recomposes the view with the original capacity into the same `BoundBackCapacityReservation`, then pairs that bound reservation with the verified owner in the branch-specific struct. No split can recover a raw handoff. After consumption, ECS must pass the saved evidence through the real production predicate `RouterEntry::validate_back_resolution_evidence(&saved_evidence)`；该 predicate 只委托 Core classifier 并映射结果，不建立 ECS ledger 或读取 opaque field，真实结果必须为 `BackResolutionEvidenceDisposition::ConsumedReplay`。Tests may only observe that predicate result, never inject the disposition directly, and the gate must fail if either predicate is removed or bypassed. exact `Matched` 的唯一 Core-private 写入是对应 record 的 `Current→Consumed`，其余 Core state、World、history 与 public 均不变。On foreign `Mismatch`, Core returns raw accepted view + raw handoff + evidence and ECS likewise reconstructs the original bound reservation before returning. `BackCapacityRetirementEvidence` is diagnostic for capacity disposition and cannot substitute for resolution duplicate evidence

In supported production callsites, enqueue acceptance establishes a closed crate-private pairing between each Core Back resolution action and its exact `BoundBackCapacityReservation`; `take_back_resolution_handoff` is the only consumer of that pair. Step 7/8 do not add `MissingCapacity`、domain quarantine or a recovery protocol, and do not expose a broken internal invariant as a public or production lifecycle branch

`resume_bound_back_target` and `fail_bound_back_contradiction` borrow the exact entry engine + entry-owned issuer for their full duration. Each keeps the verified Core owner and complete bound reservation outside the closure, then calls `preflight_bound_retirement(&mut issuer, &bound, disposition)`. A preflight `Err` creates no guard/evidence、does not call Core and directly returns `CapacityRejected { bound, error }`. Successful preflight yields one lifetime-bound `OutstandingRetire` guard plus private prepared retirement evidence. The matching Core `*_with_retirement` callback captures only the guard and calls `guard.commit()`. Target receiver mismatch happens before prepared-target work, calls the closure zero times, drops the uncommitted guard/evidence and returns `ReceiverMismatch` after rebuilding the complete bound owner. An exact target receiver completes all Core refreeze、matcher、reconciliation、action/mount issue and permanent classification before calling the closure once；after it returns, the helper unconditionally drops the now-stale bound reservation and only the exact `Resumed | Permanent` arm seals the prepared evidence into `BackCapacityRetirementEvidence`. `fail_bound_back_contradiction` preserves the same zero/one callback discipline. No bound owner、guard、prepared successor or partially consumed branch escapes

`BackCapacityRetirementEvidence` is non-authorizing terminal evidence, not downstream execution or cleanup authority. In the same exact-entry target dispatch, the entry issuer performs the named consuming audit by requiring its existing `classify_retirement_evidence(&evidence)` predicate to report exact `ConsumedReplay`, then explicitly ends the evidence while atomically installing the corresponding Ready、FailureCleanup or Permanent branch. An incremental implementation that cannot install a branch stores the whole `{ branch owner, retirement }` in one transient typed composite; it may not drop the evidence or move it into `TargetPreparationOwner` or the failure-cleanup chain

`begin_bound_back_no_target` uses entry-local issuer `preflight_no_target_transfer(&mut issuer, &bound)` and Core `refreeze_back_no_target_source_with_transfer` in the same borrow. Preflight `Err` creates no guard/successor、does not call Core and returns `CapacityRejected` with the complete verified candidate + bound reservation. Successful preflight returns an `OutstandingToLeased` guard plus private prepared permit while that complete owner remains outside the closure. Core receiver mismatch calls the closure zero times、drops guard/successor and returns `ReceiverMismatch` with the complete bound owner；the exact receiver first rebuilds and exact-refreezes source into `PreparedUnchanged | PreparedDrift`，then calls only `guard.commit()` once；the Core callback tail moves the prepared branch after commit. The exact outer arm moves the stale bound by value into `PreparedNoTargetCapacityPermit::into_permit_after_exact_receiver` before returning `Begun` or source-drift `Permanent`

`settle_back_target_permanent` delegates to the exact `RouterEntry` engine's `settle_back_target_permanent_exact`; it has no fallback engine path, returns the original Core owner on mismatch and returns only Core terminal/settlement evidence on success

Move the existing Step 8 preparation shapes ahead of the tests and scaffold these exact signatures

```rust
pub struct PreparationBoundaryScopeView { inner: CorePreparationBoundaryScopeProjection }
#[doc(hidden)]
pub struct PreparationReceiverMismatchView { inner: CorePreparationReceiverMismatchProjection }
pub struct LifecycleContinuationCapacity { inner: LifecycleContinuationBinding }
impl LifecycleContinuationCapacity {
    pub fn preparation_boundary_scope(&self) -> PreparationBoundaryScopeView;
}
#[doc(hidden)]
pub enum BackTargetPreparationAcceptance<Accepted> {
    Matched(Accepted),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        mismatch: PreparationReceiverMismatchView,
    },
}
#[doc(hidden)]
pub enum BackTargetExecutionOwnerRef<'a> {
    Driving {
        history: &'a HistoryExecutionBindingView,
        continuation: &'a LifecycleContinuationCapacity,
    },
    Classifying {
        history: &'a HistoryExecutionBindingView,
        continuation: &'a LifecycleContinuationCapacity,
    },
    Prepared {
        history: &'a HistoryExecutionBindingView,
        continuation: &'a PreparedPostCommitContinuationOwner,
    },
}
#[doc(hidden)]
pub enum BackTargetPhysicalOwnerRef<'a> {
    ResumeAction(&'a BackTargetResumeAction),
    FrameTask(&'a RootFrameTask),
    MountRequest(&'a MountRequest),
    CommitTicket(&'a CommitTicket),
}
#[doc(hidden)]
pub enum BackTargetExecutionPairingOutcome {
    Matched,
    Mismatch(PreparationReceiverMismatchView),
}
#[doc(hidden)]
pub enum BackTargetDriverFailureReportMode {
    Normal,
    PhysicalResidual,
}
#[doc(hidden)]
pub struct BackTargetDriverFailureCleanupOwner { inner: CoreBackTargetDriverFailureCleanup }
#[doc(hidden)]
pub struct BackTargetDriverFallbackOwner { inner: CoreBackTargetDriverFallback }
#[doc(hidden)]
pub struct BackTargetDriverTerminalOwner { inner: CoreBackTargetDriverTerminal }
#[doc(hidden)]
pub struct BackTargetDriverTerminalEvidence { inner: CoreBackTargetDriverTerminalProjection }
#[doc(hidden)]
pub enum BackTargetDriverFailureCoreOutcome {
    FailureCleanup(BackTargetDriverFailureCleanupOwner),
    Fallback(BackTargetDriverFallbackOwner),
    Terminal(BackTargetDriverTerminalOwner),
}
#[doc(hidden)]
pub enum BackTargetDriverFailureReportOutcome {
    Reported(BackTargetDriverFailureCoreOutcome),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        mismatch: PreparationReceiverMismatchView,
    },
}
#[doc(hidden)]
pub enum BackTargetDriverFailureConsumeOutcome {
    FailureCleanup(FailureCleanupRequest),
    FallbackReleased,
    Terminal(BackTargetDriverTerminalEvidence),
    Mismatch {
        owner: BackTargetDriverFailureCoreOutcome,
        mismatch: PreparationReceiverMismatchView,
    },
}
#[doc(hidden)]
pub enum BackTargetAppliedReport {
    LegacyLeaf,
    WithOutlets(Vec<RouteSlotKey>),
}
#[doc(hidden)]
pub enum BackTargetAppliedReportOutcome {
    FrameTask {
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
    },
    MountRequest {
        capacity: LifecycleContinuationCapacity,
        request: MountRequest,
    },
    Commit {
        capacity: LifecycleContinuationCapacity,
        ticket: CommitTicket,
    },
    FailureCleanup(BackTargetDriverFailureCleanupOwner),
    Mismatch {
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        report: BackTargetAppliedReport,
        mismatch: PreparationReceiverMismatchView,
    },
}
#[doc(hidden)]
pub struct ProvenEmptyApprovalView { inner: CoreProvenEmptyApprovalProjection }
#[doc(hidden)]
pub struct ApprovedBackNoTargetCandidate {
    candidate: VerifiedBackNoTargetCandidate,
    approval: ProvenEmptyApprovalView,
}
#[doc(hidden)]
pub enum BackNoTargetApprovalOutcome {
    Approved(ApprovedBackNoTargetCandidate),
    Mismatch { candidate: VerifiedBackNoTargetCandidate, mismatch: PreparationReceiverMismatchView },
}
pub struct PreparedPostCommitContinuationOwner { inner: PreparedPostCommitContinuation }
#[doc(hidden)]
pub struct BackTargetCommitRetainedInputOwner {
    continuation: PreparedPostCommitContinuationOwner,
    manifest: CleanupManifest,
    evidence: CoreBackTargetCommitRetainedInputProjection,
}
#[doc(hidden)]
pub struct BackTargetCommitFailureCleanupOwner {
    inner: CoreBackTargetCommitFailureCleanup,
}
#[doc(hidden)]
pub struct BackTargetCommitRejectedPublicationOwner {
    inner: CoreBackTargetCommitRejectedPublication,
}
#[doc(hidden)]
pub enum BackTargetCommitPreparationFailure {
    RetainedInput(BackTargetCommitRetainedInputOwner),
    FailureCleanup(BackTargetCommitFailureCleanupOwner),
    RejectedPublication(BackTargetCommitRejectedPublicationOwner),
}
#[doc(hidden)]
pub enum BackTargetCommitPreparationOutcome<'a> {
    Validated(ValidatedCommit<'a>),
    Mismatch {
        owner: PreparedPostCommitContinuationOwner,
        manifest: CleanupManifest,
        mismatch: PreparationReceiverMismatchView,
    },
    PreparationFailed(BackTargetCommitPreparationFailure),
}
pub struct RouteFailureContinuationOwner { inner: RouteFailureContinuation }
pub struct TargetAuthoritySettlementOwner { inner: TargetAuthoritySettlement }
pub struct BackNoTargetAuthoritySettlementOwner { inner: BackNoTargetAuthoritySettlement }
#[doc(hidden)]
pub struct BackNoTargetCleanupBatchOwner { inner: CoreBackNoTargetCleanupBatch }
#[doc(hidden)]
pub struct RejectedBackContinuationOwner { inner: CoreRejectedBackContinuation }
#[doc(hidden)]
pub struct BackNoTargetCleanupAction { inner: CoreBackNoTargetCleanupAction }
#[doc(hidden)]
pub struct BackNoTargetCleanupBatchView { inner: CoreBackNoTargetCleanupBatchProjection }
#[doc(hidden)]
pub struct BackNoTargetCleanupInstallation {
    batch: BackNoTargetCleanupBatchOwner,
    continuation: RejectedBackContinuationOwner,
    action: BackNoTargetCleanupAction,
}
impl BackNoTargetCleanupInstallation {
    pub fn batch(&self) -> BackNoTargetCleanupBatchView;
    pub fn complete(self, completion_digest: u64, settled_count: usize)
        -> BackNoTargetCleanupCompletionOwner;
}
#[doc(hidden)]
pub struct BackNoTargetCleanupCompletionOwner {
    installation: BackNoTargetCleanupInstallation,
    completion: CleanupCompletion,
}
#[doc(hidden)]
pub struct BackNoTargetTerminalEvidence { inner: CoreBackNoTargetTerminalProjection }
#[doc(hidden)]
pub enum BackNoTargetCleanupCompletionOutcome {
    Terminal(BackNoTargetTerminalEvidence),
    Mismatch { owner: BackNoTargetCleanupCompletionOwner, mismatch: PreparationReceiverMismatchView },
}
#[doc(hidden)]
pub struct PreparedBackNoTargetPublication {
    publication: CoreBackNoTargetPublicationCapability,
    installation: BackNoTargetCleanupInstallation,
}
#[doc(hidden)]
pub enum BackNoTargetReadyCoreOutcome {
    Prepared(PreparedBackNoTargetPublication),
    Mismatch { approved: ApprovedBackNoTargetCandidate, mismatch: PreparationReceiverMismatchView },
}
#[doc(hidden)]
pub enum BackNoTargetAuthorityCoreOutcome {
    Accepted(BackNoTargetAuthoritySettlementOwner),
    Mismatch { candidate: VerifiedBackNoTargetCandidate, mismatch: PreparationReceiverMismatchView },
}
pub(crate) struct ProvenEmptyProjection { /* §5.3 exact ECS physical source/protected/manifest facts; never Core approval */ }
pub(crate) struct PreparationReclaimOwner { domain: PreparationCapacityDomain, identity: u64 }
pub(crate) struct PreparationHostEpoch { scope: PreparationBoundaryScopeView, host: RouteHostId, epoch: u64 }
pub(crate) enum OutstandingBoundaryProducer { Action(u64), Ticket(u64) }

pub(crate) enum PreparationRetryEvidence {
    RetryableCapacity { domain: PreparationCapacityDomain, reclaim_owner: PreparationReclaimOwner, release_generation: PreparationReleaseGeneration },
    UnfinalizedBoundaryFacts { host_epoch: PreparationHostEpoch, outstanding: OutstandingBoundaryProducer },
}

pub(crate) enum LifecyclePreparationOutcome<Ready, Owner> {
    Ready(Ready),
    PendingRejected { owner: Owner, evidence: LifecyclePreflightEvidence },
    AuthorityLost { owner: Owner, evidence: LifecyclePreflightEvidence },
    RetryablePreparationFailure { owner: Owner, evidence: PreparationRetryEvidence },
}

pub(crate) enum PreparationPermanentEvidence {
    UnreachablePendingRejected(LifecyclePreflightEvidence),
    CapacityDomainContradiction { domain: PreparationCapacityDomain, generation: PreparationReleaseGeneration },
    PhysicalPreparationContradiction(LifecyclePreflightEvidence),
}
pub(crate) struct PreparationPermanentOwner<Owner> { owner: Owner, evidence: PreparationPermanentEvidence }
pub(crate) enum PreparationDecision<Ready, Owner, PermanentOwner> {
    Classified(LifecyclePreparationOutcome<Ready, Owner>),
    Permanent(PermanentOwner),
}

pub(crate) struct BackPreparationOwner {
    candidate: VerifiedBackNoTargetCandidate,
    capacity: NoTargetCapacityPermit,
}
pub(crate) struct BackNoTargetSourceDriftPermanentOwner {
    drift: BackNoTargetSourceDriftOwner,
    capacity: NoTargetCapacityPermit,
}
pub(crate) enum BackNoTargetPermanentOwner {
    Physical(PreparationPermanentOwner<BackPreparationOwner>),
    SourceDrift(BackNoTargetSourceDriftPermanentOwner),
}
pub(crate) struct TargetPreparationOwner {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
}
pub(crate) struct FrozenBackTargetDriverZeroPhysicalProof {
    /* task + pending lineage；complete registry/relationship target-protected partition；target failure manifest exact-zero */
}
pub(crate) struct BackTargetDriverZeroPhysicalOwner {
    pending: PendingCommit,
    proof: FrozenBackTargetDriverZeroPhysicalProof,
}
pub(crate) enum BackTargetDriverFailurePhysicalOwner {
    ProvenZero(BackTargetDriverZeroPhysicalOwner),
    Residual(FailedPreparedBranch),
}
impl BackTargetDriverFailurePhysicalOwner {
    pub(crate) const fn report_mode(&self) -> BackTargetDriverFailureReportMode;
}
pub(crate) struct PendingBackTargetDriverFailureInput {
    driving: PendingBackTargetDriving,
    task: RootFrameTask,
    physical: BackTargetDriverFailurePhysicalOwner,
    kind: ViewFailureKind,
    message: &'static str,
}
pub(crate) struct PendingBackTargetAppliedReportInput {
    driving: PendingBackTargetDriving,
    task: RootFrameTask,
    pending: PendingCommit,
    report: BackTargetAppliedReport,
}
pub(crate) struct PendingBackTargetAppliedReportMismatch {
    owner: PendingBackTargetAppliedReportInput,
    mismatch: PreparationReceiverMismatchView,
}
pub(crate) struct PendingBackTargetAppliedMountRequest {
    driving: PendingBackTargetDriving,
    request: MountRequest,
    pending: PendingCommit,
}
pub(crate) struct PendingBackTargetDriverFailureReported {
    history: HistoryExecutionBindingView,
    physical: BackTargetDriverFailurePhysicalOwner,
    outcome: BackTargetDriverFailureCoreOutcome,
}
pub(crate) enum PendingBackTargetDriverFailure {
    Unaccepted(PendingBackTargetDriverFailureInput),
    ReceiverMismatch {
        owner: PendingBackTargetDriverFailureInput,
        mismatch: PreparationReceiverMismatchView,
    },
    Reported(PendingBackTargetDriverFailureReported),
    ConsumeMismatch {
        owner: PendingBackTargetDriverFailureReported,
        mismatch: PreparationReceiverMismatchView,
    },
    PhysicalMismatch {
        owner: PendingBackTargetDriverFailureReported,
        evidence: BackTargetDriverPhysicalMismatchEvidence,
    },
}
pub(crate) struct BackTargetDriverPhysicalMismatchEvidence {
    /* Residual paired with Fallback or Terminal */
}
pub(crate) enum PendingBackTargetResumeBlocked {
    Start {
        start: BackTargetStart,
        retirement: BackCapacityRetirementEvidence,
    },
    PairingMismatch {
        start: BackTargetStart,
        retirement: BackCapacityRetirementEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    DrivingPairingMismatch {
        owner: PendingBackTargetDriving,
        mismatch: PreparationReceiverMismatchView,
    },
    DriverFailure(PendingBackTargetDriverFailure),
    AppliedReportMismatch(PendingBackTargetAppliedReportMismatch),
    AppliedMountRequestPending(PendingBackTargetAppliedMountRequest),
    Permanent {
        owner: BackTargetPermanentOwner,
        retirement: BackCapacityRetirementEvidence,
    },
    CapacityRejected {
        bound: BoundBackTargetResolution,
        error: BackCapacityRetireError,
    },
    ReceiverMismatch {
        bound: BoundBackTargetResolution,
        evidence: BackResolutionEvidence,
    },
}
pub(crate) struct PendingBackTargetDriving {
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
}
pub(crate) struct BackTargetClassifiedPhysicalOwner {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
}
pub(crate) struct PreparedBackTargetPhysicalOwner {
    classified: BackTargetClassifiedPhysicalOwner,
    focus_leaving: Vec<Entity>,
}
pub(crate) struct BackTargetReadyAcceptanceOwner {
    physical: PreparedBackTargetPhysicalOwner,
    continuation: LifecycleContinuationCapacity,
    frozen_manifest: FrozenCleanupManifest,
    manifest: CleanupManifest,
}
pub(crate) struct PendingBackTargetReadyAccepted {
    physical: PreparedBackTargetPhysicalOwner,
    continuation: PreparedPostCommitContinuationOwner,
    frozen_manifest: FrozenCleanupManifest,
    manifest: CleanupManifest,
}
pub(crate) struct PendingPreparedBackTargetCommit {
    physical: PreparedBackTargetPhysicalOwner,
    continuation: PreparedPostCommitContinuationOwner,
    ledger: PreparedCleanupLedger,
    manifest: CleanupManifest,
}
pub(crate) enum PendingBackTargetClassification {
    Active(TargetPreparationOwner),
    PairingMismatch {
        owner: TargetPreparationOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    EntryMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    ReadyAwaitingAcceptance(BackTargetReadyAcceptanceOwner),
    ReadyPlanRetry {
        owner: BackTargetReadyAcceptanceOwner,
        evidence: PreparationRetryEvidence,
    },
    ReadyPlanPermanent(PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>),
    ReadyAcceptanceMismatch {
        owner: BackTargetReadyAcceptanceOwner,
        mismatch: PreparationReceiverMismatchView,
    },
    ReadyAccepted(PendingBackTargetReadyAccepted),
    ReadyReserveFailure {
        physical: PreparedBackTargetPhysicalOwner,
        continuation: PreparedPostCommitContinuationOwner,
        manifest: CleanupManifest,
        error: CleanupLedgerReserveError,
    },
    PendingRejectedUnaccepted {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    PendingRejectedAcceptanceMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    PendingRejected {
        physical: BackTargetClassifiedPhysicalOwner,
        continuation: RouteFailureContinuationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLostUnaccepted {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLostAcceptanceMismatch {
        owner: TargetPreparationOwner,
        evidence: LifecyclePreflightEvidence,
        mismatch: PreparationReceiverMismatchView,
    },
    AuthorityLost {
        physical: BackTargetClassifiedPhysicalOwner,
        settlement: TargetAuthoritySettlementOwner,
        evidence: LifecyclePreflightEvidence,
    },
    Retry {
        owner: TargetPreparationOwner,
        evidence: PreparationRetryEvidence,
    },
    Permanent(PreparationPermanentOwner<TargetPreparationOwner>),
}
pub(crate) enum PendingBackTargetCommitFailure {
    PairingMismatch {
        owner: PendingPreparedBackTargetCommit,
        mismatch: PreparationReceiverMismatchView,
    },
    ReceiverMismatch {
        owner: PendingPreparedBackTargetCommit,
        mismatch: PreparationReceiverMismatchView,
    },
    Failed {
        physical: PreparedBackTargetPhysicalOwner,
        ledger: PreparedCleanupLedger,
        failure: BackTargetCommitPreparationFailure,
    },
}
pub(crate) enum PendingBackTargetExecution {
    ResumeBlocked(PendingBackTargetResumeBlocked),
    Driving(PendingBackTargetDriving),
    Classifying(PendingBackTargetClassification),
    Prepared(PendingPreparedBackTargetCommit),
    CommitFailure(PendingBackTargetCommitFailure),
}
pub(crate) struct PreparedBackNoTarget { approved: ApprovedBackNoTargetCandidate, capacity: NoTargetCapacityPermit, projection: ProvenEmptyProjection }
pub(crate) struct InstalledBackNoTargetCleanup {
    ledger: CleanupLedger,
    installation: BackNoTargetCleanupInstallation,
}
pub(crate) struct SettledBackNoTargetCleanup {
    ledger: CleanupLedger,
    completion: BackNoTargetCleanupCompletionOwner,
}
// Add these exact fields to the existing RouterEntry; no parallel global table.
pub(crate) struct RouterEntry {
    /* existing fields unchanged */
    pending_back_target_execution: Option<PendingBackTargetExecution>,
    installed_back_no_target_cleanup: Option<InstalledBackNoTargetCleanup>,
    pending_back_no_target_completion: Option<SettledBackNoTargetCleanup>,
    installed_back_no_target_authority: Option<InstalledBackNoTargetAuthoritySettlement>,
    installed_target_preparation_permanent: Option<InstalledTargetPreparationPermanentSettlement>,
}
pub(crate) enum BackNoTargetCleanupDriverOutcome {
    Terminal(BackNoTargetTerminalEvidence),
    ReceiverMismatch { owner: SettledBackNoTargetCleanup, mismatch: PreparationReceiverMismatchView },
}
pub(crate) enum BackNoTargetFinalizationOutcome {
    Published,
    CapacityRejected { owner: PreparedBackNoTarget, error: BackCapacityRetireError },
    ReceiverMismatch { owner: PreparedBackNoTarget, mismatch: PreparationReceiverMismatchView },
}
pub(crate) struct BackNoTargetAuthoritySettlementDisposition {
    settlement: BackNoTargetAuthoritySettlementOwner,
    retirement: BackCapacityRetirementEvidence,
    physical: BackNoTargetAuthorityPhysicalSettlementOwner,
}
pub(crate) struct FrozenBackNoTargetAuthoritySource { /* exact router/host/epoch/request/attempt source */ }
pub(crate) struct FrozenBackNoTargetProtectedClosure { /* complete protected relationship closure */ }
pub(crate) struct FrozenBackNoTargetAllowedWriteSet { /* exact settlement-only ECS write targets */ }
pub(crate) struct BackNoTargetAuthorityPhysicalSettlementOwner {
    source: FrozenBackNoTargetAuthoritySource,
    protected: FrozenBackNoTargetProtectedClosure,
    allowed_write: FrozenBackNoTargetAllowedWriteSet,
    manifest: FrozenCleanupManifest,
}
pub(crate) struct InstalledBackNoTargetAuthoritySettlement {
    physical: BackNoTargetAuthorityPhysicalSettlementOwner,
    settlement: BackNoTargetAuthoritySettlementOwner,
    retirement: BackCapacityRetirementEvidence,
}
#[doc(hidden)]
pub enum BackNoTargetAuthoritySettlementReceiverOutcome {
    Matched(BackNoTargetAuthoritySettlementOwner),
    Mismatch { owner: BackNoTargetAuthoritySettlementOwner, mismatch: PreparationReceiverMismatchView },
}
pub(crate) enum BackNoTargetAuthoritySettlementDispatchOutcome {
    Dispatched,
    EntryMismatch { disposition: BackNoTargetAuthoritySettlementDisposition, evidence: LifecyclePreflightEvidence },
    ReceiverMismatch { disposition: BackNoTargetAuthoritySettlementDisposition, mismatch: PreparationReceiverMismatchView },
}
pub(crate) struct BackPermanentTerminalEvidence { /* ownerless Failed(AuthorityLost) or existing settlement */ }
pub(crate) enum BackPermanentSettlementOutcome {
    Settled { retirement: BackCapacityRetirementEvidence, terminal: BackPermanentTerminalEvidence },
    CapacityRejected { owner: PreparationPermanentOwner<BackPreparationOwner>, error: BackCapacityRetireError },
    Mismatch { owner: PreparationPermanentOwner<BackPreparationOwner>, evidence: LifecyclePreflightEvidence },
}
pub(crate) enum BackNoTargetSourceDriftPermanentSettlementOutcome {
    Settled { retirement: BackCapacityRetirementEvidence, terminal: BackNoTargetSourceDriftTerminalEvidence },
    ReceiverMismatch { owner: BackNoTargetSourceDriftPermanentOwner, evidence: BackNoTargetSourceDriftEvidence },
    CapacityRejected { owner: BackNoTargetSourceDriftPermanentOwner, error: BackCapacityRetireError },
}
pub(crate) struct FrozenTargetPreparationPermanentBranch { /* original authority/permanent branch; never inferred from count */ }
pub(crate) struct TargetPreparationPermanentCleanupOwner {
    splice: SealedReconciliationSplice,
    branch: FrozenTargetPreparationPermanentBranch,
}
pub(crate) struct TargetPreparationPermanentDisposition {
    cleanup: TargetPreparationPermanentCleanupOwner,
    settlement: TargetAuthoritySettlementOwner,
}
pub(crate) struct InstalledTargetPreparationPermanentSettlement {
    cleanup: TargetPreparationPermanentCleanupOwner,
    settlement: TargetAuthoritySettlementOwner,
}
#[doc(hidden)]
pub enum TargetPreparationPermanentSettlementReceiverOutcome {
    Matched(TargetAuthoritySettlementOwner),
    Mismatch { owner: TargetAuthoritySettlementOwner, mismatch: PreparationReceiverMismatchView },
}
pub(crate) enum BackTargetPreparationPermanentSettlementOutcome {
    Prepared(TargetPreparationPermanentDisposition),
    Mismatch { owner: PreparationPermanentOwner<TargetPreparationOwner>, evidence: LifecyclePreflightEvidence },
}
pub(crate) enum TargetPreparationPermanentDispatchOutcome {
    Dispatched,
    EntryMismatch { disposition: TargetPreparationPermanentDisposition, evidence: LifecyclePreflightEvidence },
    ReceiverMismatch { disposition: TargetPreparationPermanentDisposition, mismatch: PreparationReceiverMismatchView },
}
pub(crate) enum BackNoTargetPreparationOutcome {
    Ready(PreparedBackNoTarget),
    Retry { owner: BackPreparationOwner, evidence: PreparationRetryEvidence },
    AuthoritySettlement(BackNoTargetAuthoritySettlementDisposition),
    Permanent(BackNoTargetPermanentOwner),
    CapacityRejected { owner: BackPreparationOwner, error: BackCapacityRetireError },
    ReceiverMismatch { owner: BackPreparationOwner, mismatch: PreparationReceiverMismatchView },
}
pub(crate) type ReconciliationPreparationDecision =
    PreparationDecision<BackTargetReadyAcceptanceOwner, TargetPreparationOwner, PreparationPermanentOwner<TargetPreparationOwner>>;
pub(crate) enum ReconciliationPreparationOutcome {
    Matched(ReconciliationPreparationDecision),
    Mismatch { owner: TargetPreparationOwner, evidence: LifecyclePreflightEvidence },
}

pub(crate) fn classify_back_no_target(
    world: &World,
    entry: &mut RouterEntry,
    owner: BackPreparationOwner,
) -> BackNoTargetPreparationOutcome;
pub(crate) fn classify_back_target(
    world: &World,
    entry: &mut RouterEntry,
    owner: TargetPreparationOwner,
) -> ReconciliationPreparationOutcome;
pub(crate) fn settle_back_target_preparation_permanent(
    entry: &mut RouterEntry,
    owner: PreparationPermanentOwner<TargetPreparationOwner>,
) -> BackTargetPreparationPermanentSettlementOutcome;
pub(crate) fn dispatch_target_preparation_permanent(
    entry: &mut RouterEntry,
    disposition: TargetPreparationPermanentDisposition,
) -> TargetPreparationPermanentDispatchOutcome;
pub(crate) fn dispatch_back_no_target_authority_settlement(
    entry: &mut RouterEntry,
    disposition: BackNoTargetAuthoritySettlementDisposition,
) -> BackNoTargetAuthoritySettlementDispatchOutcome;
pub(crate) fn settle_back_no_target_source_drift_permanent(
    entry: &mut RouterEntry,
    owner: BackNoTargetSourceDriftPermanentOwner,
) -> BackNoTargetSourceDriftPermanentSettlementOutcome;
pub(crate) fn consume_prepared_back_no_target(
    entry: &mut RouterEntry,
    owner: PreparedBackNoTarget,
) -> BackNoTargetFinalizationOutcome;
pub(crate) fn bind_canonical_zero_ledger_to_batch(
    slot: ManifestNeutralLedgerSlot,
    batch: BackNoTargetCleanupBatchView,
) -> CleanupLedger;
pub(crate) fn drive_back_no_target_zero_completion(
    entry: &mut RouterEntry,
) -> BackNoTargetCleanupDriverOutcome;
pub(crate) fn drive_back_target_driver_failure(
    entry: &mut RouterEntry,
);
pub(crate) fn prove_back_target_driver_zero_physical(
    world: &World,
    pending: PendingCommit,
    task: &RootFrameTask,
) -> Result<BackTargetDriverZeroPhysicalOwner, FailedPreparedBranch>;
```

Also scaffold the existing exact `PreparationBoundaryScopeView`、two-variant `PreparationRetryEvidence`、`ProvenEmptyProjection`/target-protected source types、`LifecycleContinuationCapacity` and conversion signatures

```rust
impl RouterEngine {
    #[doc(hidden)]
    pub fn validate_back_target_execution_pairing(
        &self,
        owner: BackTargetExecutionOwnerRef<'_>,
        physical: BackTargetPhysicalOwnerRef<'_>,
    ) -> BackTargetExecutionPairingOutcome;
    #[doc(hidden)]
    pub fn report_back_target_failed(
        &mut self,
        history: &HistoryExecutionBindingView,
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        kind: ViewFailureKind,
        message: &'static str,
        mode: BackTargetDriverFailureReportMode, // only physical.report_mode() at this atomic callsite
    ) -> BackTargetDriverFailureReportOutcome;
    #[doc(hidden)]
    pub fn consume_back_target_driver_failure(
        &mut self,
        owner: BackTargetDriverFailureCoreOutcome,
    ) -> BackTargetDriverFailureConsumeOutcome;
    #[doc(hidden)]
    pub fn report_back_target_applied(
        &mut self,
        history: &HistoryExecutionBindingView,
        capacity: LifecycleContinuationCapacity,
        task: RootFrameTask,
        report: BackTargetAppliedReport,
    ) -> BackTargetAppliedReportOutcome;
    pub fn accept_back_target_ready(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> BackTargetPreparationAcceptance<PreparedPostCommitContinuationOwner>;
    pub fn accept_back_target_pending_rejected(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> BackTargetPreparationAcceptance<RouteFailureContinuationOwner>;
    pub fn accept_back_target_authority_lost(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> BackTargetPreparationAcceptance<TargetAuthoritySettlementOwner>;
    #[doc(hidden)]
    pub fn prepare_back_target_commit<'a>(
        &'a mut self,
        owner: PreparedPostCommitContinuationOwner,
        ticket: &CommitTicket,
        manifest: CleanupManifest,
    ) -> BackTargetCommitPreparationOutcome<'a>;
    #[doc(hidden)]
    pub fn approve_back_no_target_logical_zero(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
    ) -> BackNoTargetApprovalOutcome;
    #[doc(hidden)]
    pub fn accept_back_no_target_ready_with_canonical_zero_bind(
        &mut self,
        approved: ApprovedBackNoTargetCandidate,
        commit_canonical_zero: impl FnOnce(),
    ) -> BackNoTargetReadyCoreOutcome;
    #[doc(hidden)]
    pub fn accept_back_no_target_authority_lost_with_retirement(
        &mut self,
        candidate: VerifiedBackNoTargetCandidate,
        scope: PreparationBoundaryScopeView,
        commit_retirement: impl FnOnce(),
    ) -> BackNoTargetAuthorityCoreOutcome;
    #[doc(hidden)]
    pub fn publish_prepared_back_no_target(
        &mut self,
        prepared: PreparedBackNoTargetPublication,
        install: impl FnOnce(BackNoTargetCleanupInstallation),
    );
    #[doc(hidden)]
    pub fn complete_back_no_target_cleanup(
        &mut self,
        owner: BackNoTargetCleanupCompletionOwner,
    ) -> BackNoTargetCleanupCompletionOutcome;
    #[doc(hidden)]
    pub fn validate_back_no_target_authority_settlement_receiver(
        &mut self,
        owner: BackNoTargetAuthoritySettlementOwner,
    ) -> BackNoTargetAuthoritySettlementReceiverOutcome;
    #[doc(hidden)]
    pub fn validate_target_preparation_permanent_settlement_receiver(
        &mut self,
        owner: TargetAuthoritySettlementOwner,
    ) -> TargetPreparationPermanentSettlementReceiverOutcome;
}

pub(crate) fn fail_unreachable_back_no_target_pending_rejected(
    owner: BackPreparationOwner,
    evidence: LifecyclePreflightEvidence,
) -> BackNoTargetPermanentOwner;
pub(crate) fn settle_back_no_target_physical_permanent(
    entry: &mut RouterEntry,
    owner: PreparationPermanentOwner<BackPreparationOwner>,
) -> BackPermanentSettlementOutcome;
```

`PreparationBoundaryScopeView` has one producer: the read-only `LifecycleContinuationCapacity::preparation_boundary_scope` projection from the same capacity passed to the receiver. The view is validation-only；it cannot consume or reconstruct the capacity and grants no mutation or publication authority. `PreparationReceiverMismatchView` is Core-owned、opaque and non-authorizing. Only a Core exact receiver creates it; ECS can retain it for diagnostics but cannot construct it、inspect raw identity or rebuild an owner. The three target acceptance receivers exact-match engine、capacity and that scope and return `BackTargetPreparationAcceptance::Matched | Mismatch`；Mismatch returns the original capacity + opaque view with zero write. `LifecyclePreflightEvidence` remains ECS crate-private and stays paired with the original ECS physical/preparation owner

`RouterEntry::pending_back_target_execution` is the sole entry-local stage owner for target execution and does not extend `PendingCommit`. `ResumeBlocked` retains every uninstalled start/permanent/capacity-rejected/receiver-mismatch/driver-failure/applied-report-mismatch/applied-mount-request whole owner，so no second safe-stop field is needed. `Driving` holds only history + continuation capacity and exact-pairs with the existing physical action owner；real ticket + splice whole-transition it through `Classifying`. That subenum owns every physical-classification、branch-specific acceptance、Ready plan-preparation and ledger-reserve state，including `ReadyPlanRetry | ReadyPlanPermanent` whole pre-accept owners. Accepted PendingRejected/AuthorityLost hold only `BackTargetClassifiedPhysicalOwner { splice, history }` + converted Core continuation/settlement + classifier evidence，never the old capacity or normal Ready ledger. `Prepared` exists only after Ready acceptance and ledger reserve succeed，owning physical、prepared continuation、`PreparedCleanupLedger` and Core `CleanupManifest`. `CommitFailure` retains the same prepared owner on pairing/receiver mismatch or `{ physical, prepared ledger, true Core failure }`. Every transition preflights destination vacancy、whole-takes the current variant and installs exactly one successor；publication、dedicated failure/authority handoff or terminal leaves the field empty

`validate_back_target_execution_pairing` is the single Core opaque exact predicate for action injection、retry restore、final-task → ticket and commit entry. It borrows the current stage authority plus the existing action/frame/mount/ticket owner and exact-matches engine、request、transaction、scope and task/ticket identity without consuming or reconstructing either owner. Mismatch moves the unchanged current whole owner + opaque evidence into `ResumeBlocked::DrivingPairingMismatch`、`Classifying::PairingMismatch` or `CommitFailure::PairingMismatch`；no automatic per-tick retry or public write is allowed

Before any target-active FrameTask failure report，ECS requires ordinary `pending_failed_branch` empty and whole-moves Driving + exact task + typed physical owner + kind/message into `ResumeBlocked::DriverFailure::Unaccepted`. `prove_back_target_driver_zero_physical(world, pending, &task)` is the sole zero producer：it follows task identity + pending target lineage through the same World registry/relationship closure，builds complete target/protected partition and proves only the target failure manifest exact-zero. Existing active/preserved stays protected and may exist. The producer must not reuse `FrozenCleanupManifest::from_failed_prepared_branch` empty-owner shortcut，because that path does not inspect World/registry and cannot detect observer residual. Physical is only `ProvenZero` or `Residual(FailedPreparedBranch)`；`physical.report_mode()` is the sole mode producer

`report_back_target_failed` exact-matches history、raw capacity and task before mutation. Mismatch reports zero times and reconstructs `ReceiverMismatch` whole；Matched keeps any temporary continuation private to that call and returns only a true post-report `FailureCleanup | Fallback | Terminal` owner，never `Option` or raw capacity. Report and consume are synchronous with no intervening World write、schedule boundary or yield，so sealed ProvenZero is not rebuilt

`drive_back_target_driver_failure` whole-takes Reported and is the sole caller of `consume_back_target_driver_failure`. Residual + Fallback/Terminal becomes typed `PhysicalMismatch` without consuming Core owner. Otherwise Core exact consumer validates the bound fresh fallback attempt/action and returns a real cleanup request、`FallbackReleased` with that action still engine-held、terminal evidence or whole-owner Mismatch. ECS reconstructs `ConsumeMismatch` on wrong receiver；exact cleanup alone installs `pending_failed_branch`，exact fallback/terminal clear the stage，after which only the normal action loop may take the released fallback action. The driver never opens private inner、reads `engine.action()` or infers from `Option`/count

`report_back_target_applied` is the only target path for `LegacyLeaf | WithOutlets`. It exact-matches history、capacity、task and report before mutation；Mismatch returns all four owners and installs `ResumeBlocked::AppliedReportMismatch`. `FrameTask | MountRequest | Commit` return the same raw capacity and rerun exact pairing. Applied FailureCleanup directly seals the already realized pending/frame owner as `Residual(FailedPreparedBranch)` and enters `DriverFailure::Reported`；it must not attempt ProvenZero after Core mutation

FrameTask continues this target FrameTask slice and Commit enters Classifying. MountRequest is not sent to generic `pending_mount_request`：ECS installs `AppliedMountRequestPending { driving: history + returned capacity, request, pending }` in the same target field，keeps the ordinary mount field empty and safe-stops before action selection until the target MountRequest owner slice is implemented. Root final task → ticket and intermediate outlet report may not call generic consuming applied APIs or use `expect`

These rules cover identity/expected-outlet contradiction、host/authority loss、binding/driver absence、BuildFailed/ApplyFailed、apply stale、realization with or without residual、`push_frame` failure、parent preflight rejection、applied-outlet reconciliation/mount failure and leaf final-report error. A driver report identity mismatch remains the existing paired `PendingFrameDrive` retry and is not a failure. `MountRequest` follows the same owner rule when that action slice is implemented；Ready/publisher semantics do not change

`classify_back_target(world: &World, entry: &mut RouterEntry, owner)` and `classify_back_no_target(world: &World, entry: &mut RouterEntry, owner)` receive the same World explicitly and read `RouteEntityRegistry` only from it；neither accepts an independently supplied registry/reference. Existing `drive_navigation_system` already uses `World::resource_scope`，so the closure can retain the mutable `RouterNavigation` entry while passing that same World as read-only physical context

`classify_back_target` is pure physical classification and never calls a Core acceptance receiver. Its Ready output is `BackTargetReadyAcceptanceOwner { physical, raw continuation, frozen_manifest, manifest }`；all fallible exact Entity manifest freeze and Core `CleanupManifest` construction complete before this owner is installed as `Classifying::ReadyAwaitingAcceptance`. The scoped normal-Ready driver whole-takes that owner and，inside the same `World::resource_scope` call stack，prepares all existing registry lifecycle、claim rebind/root rebase、input gate、focus and presentation mutation plans before Core acceptance. These plans are borrowed stack capabilities，not entry fields or lifecycle owners. Plan preparation failure installs `ReadyPlanRetry { whole pre-accept owner, evidence }` or `ReadyPlanPermanent(PreparationPermanentOwner<whole pre-accept owner>)`

With plans live in that borrow，Ready acceptance Mismatch first drops them with zero write，then restores `ReadyAcceptanceMismatch` with the whole pre-accept owner. Matched acceptance installs and immediately whole-takes `ReadyAccepted`，then calls `CleanupLedger::reserve(frozen_manifest)`. Reserve failure first drops plans with zero write，then installs `Classifying::ReadyReserveFailure { physical, continuation, manifest, error }`；the by-value error owns the returned exact frozen manifest and kind. Success installs and immediately whole-takes top-level `Prepared`，calls `prepare_back_target_commit` and enters the existing publisher without schedule boundary or yield. Bridge mismatch/failure also drops plans before saving the complete existing `CommitFailure` owner. Normal success leaves neither `ReadyAccepted` nor `Prepared`

The existing publisher closure only applies those already prepared borrowed plans after `Validated` and binds the reserved ledger to the existing cleanup batch in the same borrow. The atomic tail performs no allocation、query、validation or plan rebuild. Borrowed plans never enter `RouterEntry`，survive a failure state or become a second publisher/lifecycle owner；they cannot be rebuilt after acceptance or validation

`BackTargetCommitPreparationFailure` is classified only by the inner Core operation from its real post-error state：`RetainedInput` binds the returned continuation + manifest to the still engine-held capability，`FailureCleanup` owns the actual Core failure-cleanup state，and `RejectedPublication` owns the actual retained rejected publication authority including its capability. The integration wrapper may not swallow `IntegrationError` and infer `RouteFailure | AuthoritySettlement`. No branch exposes an ownerless gap、second publisher、retaining report API or generic lifecycle abstraction

`approve_back_no_target_logical_zero` is the only producer of `ProvenEmptyApprovalView` and `ApprovedBackNoTargetCandidate`. It exact-matches the engine + verified candidate, reconstructs the current Core source and validates logical no-predecessor/zero before returning the opaque approval beside the same candidate. Mismatch returns the original verified candidate + `PreparationReceiverMismatchView`. ECS cannot construct or clone the approval; the approval is validation-only, cannot reconstruct an owner, and never substitutes for ECS `ProvenEmptyProjection`. `classify_back_no_target` calls this producer only after physical proven-empty succeeds, and `PreparedBackNoTarget` must carry the resulting approved composite

`BackNoTargetReadyCoreOutcome` is `Prepared(PreparedBackNoTargetPublication) | Mismatch { approved, mismatch: PreparationReceiverMismatchView }`. The sealed move-only prepared publication retains the Core publication capability plus the whole opaque cleanup installation, and its construction performs no public write. `BackNoTargetAuthorityCoreOutcome` returns the exact candidate + opaque mismatch view on receiver mismatch or the existing authority settlement owner on match

`VerifiedBackNoTargetCandidate` owns unresolved lifecycle lineage only; it does not contain a ready/failure continuation or cleanup action. Retry returns that candidate、permit and evidence with no public request-phase/trace write. `PreparedBackNoTarget` contains the Core-issued approved candidate composite + ECS physical projection + permit, but no ledger、continuation or action

`consume_prepared_back_no_target` borrows the exact entry engine + entry-owned issuer once, keeps the complete approved candidate/permit/projection owner outside the callback and runs `preflight_back_canonical_zero(&mut issuer, &permit, &projection)` without mutation. Preflight `Err` creates no guard/successor、does not call Core and returns `CapacityRejected` with that complete owner. Successful preflight returns a `LeasedToCanonicalZeroSlot` guard plus private non-authorizing prepared slot successor. Core `accept_back_no_target_ready_with_canonical_zero_bind` validates receiver + approved composite first；Mismatch returns that composite、invokes the callback zero times、drops guard/successor and returns `ReceiverMismatch` after ECS reconstructs the complete original owner. Matched invokes the closure exactly once to move only `guard.commit()`. The exact outer arm then moves the stale `NoTargetCapacityPermit` into `PreparedManifestNeutralLedgerSlot::into_slot_after_exact_receiver` and returns `PreparedBackNoTargetPublication` without publishing. The later `publish_prepared_back_no_target` installation closure alone consumes that slot、binds it to the non-authorizing batch projection and stores the initialized ledger + whole installation before Core publishes `HistoryIntentRejected → CleanupPending`

`drive_back_no_target_zero_completion` is the same-schedule producer/consumer after that publication. It takes the exact entry field as a whole, proves its ledger has exact zero settled obligations, derives the existing canonical completion digest/count, and calls `BackNoTargetCleanupInstallation::complete` once to form `BackNoTargetCleanupCompletionOwner` without exposing action/continuation fields. `RouterEngine::complete_back_no_target_cleanup` exact-consumes that Core owner into `Failed(BackTargetUnavailable)`. Receiver mismatch returns the complete Core completion owner + `PreparationReceiverMismatchView`; ECS recomposes it with the settled ledger as `SettledBackNoTargetCleanup` and can restore it whole without losing batch/action/continuation. Exact terminal leaves both RouterEntry fields and every Core publication/completion owner empty

`classify_back_no_target` returns the specialized `BackNoTargetPreparationOutcome`, not generic `PreparationDecision`: `Ready` carries only `PreparedBackNoTarget`; `Retry` returns the complete original owner + typed evidence; `AuthoritySettlement` carries `BackNoTargetAuthoritySettlementDisposition`; `Permanent` carries the branch permanent owner. It has no ordinary `PendingRejected` variant: that unreachable classifier result converts directly to `Permanent`. Issuer preflight `Err` returns `CapacityRejected` before guard/Core call；a later Core receiver mismatch returns `ReceiverMismatch` after callback zero and guard/successor drop，and both preserve the complete original owner

The pre-proof `AuthorityLost` arm follows the same boundary inside `classify_back_no_target`: ECS freezes `BackNoTargetAuthorityPhysicalSettlementOwner` from the complete source/protected closure、allowed-write set and exact authority manifest. Issuer preflight `Err` creates no guard/evidence、does not call Core and returns `CapacityRejected` with the complete owner. Successful `preflight_no_target_retirement` returns a `LeasedRetire` guard + prepared evidence，then Core exact-matches candidate + scope before the callback. Mismatch calls it zero times、drops guard/evidence and returns `ReceiverMismatch` with the complete owner；exact match calls only `guard.commit()` once，then drops the stale permit and seals evidence into `BackNoTargetAuthoritySettlementDisposition`

`dispatch_back_no_target_authority_settlement` is the sole disposition consumer. It validates the exact entry and calls the Core receiver before mutation; entry/engine mismatch returns the entire disposition. Exact dispatch moves the physical owner + Core settlement owner into the existing failed/scoped authority settlement field, installs only its authority ledger/residual under the frozen allowed-write set, and publishes only authority-specific `Running(Recovering) + requested=None`. It never populates `installed_back_no_target_cleanup`, never records `HistoryIntentRejected`/normal `CleanupPending`, and introduces no publisher. Existing authority cleanup completion consumes the stored physical owner first and then the same stored Core settlement owner to reach the original HostLost/AuthorityLost terminal; retirement evidence remains diagnostic and terminal inventory clears the authority field

`PreparationPermanentOwner<TargetPreparationOwner>` is the post-splice ECS physical-preparation owner: it retains the original splice、history binding and continuation together with exact physical evidence. `classify_back_target` exact-matches the `RouterEntry` engine before any Core conversion; cross-router/entry mismatch returns that original `TargetPreparationOwner`. `settle_back_target_preparation_permanent` only prepares `TargetPreparationPermanentDisposition { cleanup, settlement }`; `cleanup` freezes the original authority/permanent branch beside the splice, so later continuation never infers branch from obligation count or disposition

`dispatch_target_preparation_permanent` is the sole disposition consumer. It validates exact entry and the Core settlement receiver before mutation; either mismatch returns the entire disposition. Exact dispatch stores `InstalledTargetPreparationPermanentSettlement { cleanup, settlement }` as one entry-local continuation while preserving the two linear owners. The existing physical cleanup driver consumes only `cleanup`; after its exact receipts complete, it selects the already-frozen original branch and consumes the stored `settlement` into the existing scoped authority settlement. Existing authority completion reaches terminal and clears the installed field; the disposition never reaches the normal publisher

`BackTargetPermanentOwner` is not accepted by target physical classification and remains exclusive to pre-splice `resume_back_target/freeze_back` source drift. Conversely `settle_back_target_preparation_permanent` never accepts that Core owner, so no consumer can receive both permanent authorities or mint a second publication path

`begin_bound_back_no_target` completes verified receiver check、issuer transfer preflight、source reconstruction and exact refreeze into `PreparedUnchanged | PreparedDrift` before its lifetime-bound guard callback, all in one entry borrow. The callback only moves `guard.commit()`；the Core tail moves the already prepared branch after callback return. The exact outer arm consumes the stale bound into the prepared permit successor，then `Unchanged` creates `BackPreparationOwner` while `Drift` pairs opaque drift with that permit. `classify_back_no_target` handles only later physical proof/classification and never repeats source refreeze

`settle_back_no_target_source_drift_permanent` keeps the complete `{ raw drift, permit }` outside the callback. Issuer preflight `Err` creates no `LeasedRetire` guard/evidence、does not call Core and returns `CapacityRejected`；Core receiver mismatch after successful preflight calls the callback zero times、drops guard/evidence and returns `ReceiverMismatch` with the full composite. Exact receiver calls only `guard.commit()` once、terminals immediately，then ECS drops the stale permit and seals prepared retirement evidence in that exact arm

Missing symbols、placeholder panics、test setup failures and directly injected outcomes do not count as RED

| RED/gate | Required production assertion |
| --- | --- |
| `accepted_back_view_exact_lineage_rejects_cross_scope` | Core consuming verify accepts exact accepted+raw handoff lineage and returns move-only `VerifiedBackResolutionHandoff`; its split returns accepted view + one verified branch owner and never raw handoff. Mismatch returns the original accepted view、raw handoff and evidence, then ECS reassembles the untouched capacity owner |
| `verified_back_resolution_recomposes_bound_capacity_for_all_branches` | Matched recomposes returned accepted view + original capacity into the same `BoundBackCapacityReservation`; branch helper keeps that complete owner outside the callback, preflights a lifetime-bound guard + private prepared successor and passes only `guard.commit()` to the exact Core consumer |
| `verified_back_handoff_cannot_reenter_raw_consumers` | compile/static gate proves resume、no-target refreeze and contradiction terminal accept only their branch-specific verified owner; matched owners cannot convert back to `BackResolutionOwnerHandoff` or cross branches |
| `back_trusted_commit_callbacks_are_not_facade_reachable` | compile/static facade fixture proves every callback-bearing `*_with_*` method and `publish_prepared_back_no_target` is doc-hidden、absent from facade exports, and unusable by facade/adapter code because no verified/prepared Core owner is obtainable. Static callsite audit finds exactly one ECS helper callsite per method; each conditional callback captures one lifetime-bound guard、calls only `guard.commit()` and contains no owner-by-value capture、lookup/index、allocation、validation、`expect`、`unwrap`、unsafe or hook write. Prepared successor conversion occurs only in the exact outer outcome：retirement drops the stale owner before sealing evidence，while transfer and canonical-zero consume the stale owner by value during successor conversion. The gate does not claim to constrain a malicious direct Core caller |
| `back_resolution_handoff_foreign_and_duplicate_are_zero_write` | gate covers non-Back/queued Back `None`、action/helper in-transit and foreign saved observed evidence as `Current`、Core mismatch diagnostic evidence as `Foreign`、Matched/terminal/evicted saved evidence as `ConsumedReplay`. Retained classification first requires supplied/stored exact equality; Current cannot terminal/evict, evicted replay requires exact opaque scope + both watermarks. Matched leaves no raw replay owner and its only Core-private write is the exact `Current→Consumed` record state; all other Core state plus World/history/public remain unchanged. The test must use the real ECS predicate + Core classifier, never inject disposition, and fail if either predicate is removed or bypassed; capacity retirement evidence cannot stand in for duplicate evidence |
| `back_no_target_retry_preserves_owner_and_public_snapshot` | verified candidate remains the sole unresolved lifecycle lineage owner paired with the same permit; Retry returns candidate、permit and typed evidence intact, with byte-for-byte unchanged request/public snapshot and no phase、trace、continuation、cleanup action or terminal |
| `back_no_target_logical_zero_approval_requires_exact_core_candidate` | after ECS physical proven-empty, the real Core producer exact-matches receiver + verified candidate and refreezes current logical source/no-predecessor before returning `ApprovedBackNoTargetCandidate`; mismatch returns the original candidate + evidence and emits no approval/public write |
| `back_no_target_approval_cannot_be_forged_or_replace_physical_proof` | compile/static gate proves facade/ECS cannot construct、clone or synthesize `ProvenEmptyApprovalView`; the only Ready owner contains a Core-issued approved composite and a distinct ECS `ProvenEmptyProjection`, and removing either producer prevents Ready |
| `back_target_ready_moves_exact_owner_without_publication` | pure physical Ready freezes ECS/Core manifests into the whole pre-accept owner；borrowed publisher-plan preparation precedes acceptance and returns that owner through typed Ready plan Retry/Permanent；Matched installs and whole-takes `ReadyAccepted` before reserve，reserve success installs and whole-takes Prepared，failure retains the exact owner with zero publication |
| `back_target_pending_rejected_converts_exact_owner_before_public_state` | PendingRejected converts the exact owner before any public state |
| `back_target_authority_lost_converts_exact_owner_before_public_state` | AuthorityLost converts the exact owner before any public state |
| `back_target_retry_returns_exact_owner_capability_and_evidence` | Retry returns the exact live owner、capability、continuation and evidence with zero write |
| `back_target_resume_action_follows_real_reconciliation` | real direct entering returns `FrameTask`，a nested target whose prior parent/slot is not reusable returns `MountRequest`，duplicate/equivalent and pure-truncate zero-entering return the real `Commit` action；the current slice keeps initial Ready MountRequest whole in `ResumeBlocked::Start` with the generic mount field empty；unused reserved frame/task identities remain permanently burned |
| `back_target_dispatcher_consumes_verified_bound_target_through_normal_publication_once` | one real root FrameTask target uses atomic applied-report outcome and exact pairing to reach a real ticket，then stays in one `drive_navigation_system` `World::resource_scope` call stack from pure physical Ready through manifest freeze、borrowed plans、acceptance、ledger reserve、bridge validation and existing publisher exactly once；normal success leaves no ReadyAccepted/Prepared stage and terminal inventory is empty |
| `back_target_driver_failure_report_consume_uses_real_dispatcher` | ECS `--lib` dynamic test drives the real dispatcher and production predicates through atomic report + sole consume for cleanup、fallback/terminal and wrong report/consume receiver；same-World ProvenZero rejects observer residual while protected active remains outside the target failure manifest；fixtures may alter predicate inputs but never inject outcomes |
| `back_target_atomic_applied_report_uses_real_dispatcher` | ECS `--lib` dynamic test drives the real applied-report dispatcher through `FailureCleanup → Residual`、MountRequest target-field safe-stop and Mismatch returning capacity、task、report and opaque evidence；generic mount/failed fields remain empty until the typed owner is consumed |
| `back_target_frame_task_failures_preserve_one_typed_execution_owner` | static gate audits every named FrameTask failure exit；ProvenZero must scan task lineage + registry/relationship closure、exclude protected active and reject observer residual，removing proof、using empty failed-branch shortcut or allowing independent mode fails；Residual cannot consume Fallback/Terminal；wrong failure/applied/consume receiver returns every owner；applied cleanup is directly Residual，FrameTask/Commit preserve exact pairing，MountRequest stays whole in target safe-stop with generic mount field empty；Fallback action remains unavailable until Core consume + stage clear；no generic consuming report/private-inner/action/Option inference，all outcomes close inventory |
| `back_target_dispatch_owners_are_linear_and_exact` | static/focused gate proves both classifiers accept the same `&World` and read registry only from it；one RouterEntry field owns mutually exclusive `ResumeBlocked | Driving | Classifying | Prepared | CommitFailure` stages，including typed Ready plan Retry/Permanent whole owners，with no second safe-stop Option；Ready publisher plans are borrowed stack capabilities prepared before acceptance，never stored or rebuilt，and failure drops them before saving the exact stage owner；Matched installs/whole-takes ReadyAccepted then Prepared in one borrow，reserve failure alone persists ReadyReserveFailure with the returned frozen manifest；only `prepare_back_target_commit` reaches the existing publisher，whose atomic tail performs no allocation、query or validation，while every mismatch/failure remains typed and terminal inventory is empty |
| `back_target_mount_exhaustion_is_failure_cleanup_not_resume_action` | real nested mount issue exhaustion produces `BackTargetStart::FailureCleanup(FailureCleanupRequest)`，never a resume action or panic，and flows only into the existing failure-cleanup owner chain |
| `back_target_resume_action_is_not_facade_reachable` | compile/static facade fixture proves `BackTargetResumeAction`、`BackTargetStart` and their variants cannot be named or reexported through the facade，while the facade export golden/profile remains unchanged |
| `back_target_receiver_mismatch_calls_zero_and_exact_receiver_calls_once` | wrong receiver returns the exact verified owner before preparation、drops the uncommitted lifetime-bound guard and leaves the borrowed bound owner live；exact `FrameTask | MountRequest | Commit`、FailureCleanup and Permanent fixtures complete preparation first，then call `guard.commit()` exactly once |
| `back_target_prepares_all_fallible_work_before_retirement_callback` | source、host/top-slot、active/child-slot baseline refreeze，matcher/reconciliation，generation/depth checks and mount issue all complete before the callback；same-engine drift/contradiction returns typed Permanent/AuthorityLost with callback once，not Mismatch or panic |
| `back_target_callback_tail_is_infallible_and_query_free` | static callsite gate proves each conditional receiver has one ECS production callsite and callback captures neither owner nor prepared Core branch、只 move `guard.commit()`；guard commit may match only a prebuilt operation-specific transition，forbids owner/key lookup、key remove、Vec index/query、allocation、validation、`expect`、`unwrap`、panic or unsafe。Separate assertions prove transfer retains the occupied live entry and updates it to `Leased`，retire consumes the held entry、returns lifecycle to `Free` and applies `Reusable | PermanentlyRetired` metadata，canonical-zero consumes the held `Leased` entry and moves its exact cell lease into the slot |
| `back_target_zero_entering_commit_needs_no_frame_report` | duplicate/equivalent zero-entering consumes the returned `CommitTicket` without frame report，publishes the exact destination entry/cursor/revision once and makes replay zero-write |
| `back_target_zero_entering_preserves_physical_contract` | equivalent target remains physically zero-write；pure-truncate with no entering still prepares and settles the exact leaving cleanup manifest |
| `back_target_and_contradiction_receiver_mismatch_never_commits_retirement` | wrong engine returns the verified target/contradiction owner, uncommitted guard drop keeps issuer zero-write and ECS reconstructs the complete branch bound owner; exact receiver calls `guard.commit()` once，then ECS drops the stale bound and exposes only the exact arm's prepared retirement evidence |
| `back_target_resume_source_drift_preserves_owner_and_terminals_once` | before any ECS splice, exact receiver calls retirement commit once then Core resume/freeze may return `BackTargetPermanentOwner`; its exact settlement reaches existing AuthorityLost terminal once |
| `back_target_physical_permanent_preserves_spliced_owner_and_disposition` | after splice, physical permanent retains the exact `TargetPreparationOwner`; classify cross-router mismatch and settlement engine mismatch each return it unchanged, while exact settlement yields the single physical-cleanup + authority-settlement disposition and never a `BackTargetPermanentOwner` |
| `back_contradiction_terminals_once_without_world_or_history_write` | real structural contradiction retires bound capacity, consumes accepted owners once, reaches `Failed(AuthorityLost)` exactly once and leaves zero World/presentation/history write or outstanding owner |
| `back_no_target_proven_empty_protects_sparse_current_topology` | real claim/registry/closure perturbations prevent proof; retained-empty/slot-only current Entities remain protected and zero-write, with authority derived only from exact bindings and each Entity's own role |
| `back_no_target_preparation_branches_preserve_exact_owners` | exact classifier signature receives the same `&World` as entry and reads registry only from it；specialized outcome returns Ready prepared owner、Retry original owner+evidence、AuthoritySettlement whole disposition or Permanent owner and has no ordinary PendingRejected variant. Ready issuer preflight `Err` returns `CapacityRejected` without Core call；Core mismatch calls guard commit zero times and reconstructs the full owner；exact Core calls guard commit once and the outer arm converts the stale permit into the prepared slot successor. Batch ledger bind belongs only to the later publication closure. Authority returns `{ physical, Core settlement, ECS retirement }` as one disposition |
| `back_no_target_unreachable_pending_rejected_becomes_permanent_owner` | unreachable PendingRejected moves every owner into `BackNoTargetPermanentOwner::Physical(PreparationPermanentOwner<BackPreparationOwner>)` without cleanup/trace |
| `back_no_target_refreeze_receiver_mismatch_never_commits_transfer` | issuer preflight `Err` returns `CapacityRejected` before guard/Core call；wrong engine returns `ReceiverMismatch` with guard commit zero and complete bound owner；exact receiver first rebuilds and exact-refreezes source into `PreparedUnchanged | PreparedDrift`，then calls only `guard.commit()` once，its Core tail moves the prepared branch，and the exact outer arm consumes the stale bound through `into_permit_after_exact_receiver` to produce candidate+permit or drift+permit |
| `back_no_target_source_drift_terminals_once` | single helper keeps raw drift+permit local；issuer preflight `Err` returns `CapacityRejected` without Core call，Core mismatch returns `ReceiverMismatch` with guard commit zero and the full composite，exact receiver calls guard commit once then terminals and seals retirement evidence |
| `back_no_target_ready_receiver_or_issuer_mismatch_never_binds` | issuer preflight `Err` creates no guard/successor and returns `CapacityRejected` without Core call；wrong receiver/approval returns `ReceiverMismatch` with guard commit zero and the full `PreparedBackNoTarget`；exact case commits the `LeasedToCanonicalZeroSlot` guard once and converts the stale permit into the prepared slot successor. Batch ledger bind is tested only in the later publication closure |
| `back_no_target_first_publication_has_installed_owners` | at the first observable `HistoryIntentRejected`, the exact entry already contains one whole `InstalledBackNoTargetCleanup { ledger, installation }`; batch/action/rejected-back continuation remain private inside that same installation. Its header is safely `Installed`, obligation/receipt vectors are initialized empty, and the allocation counter is unchanged. The trusted installation closure runs once before the final Core public tail; no observer can see publication while the whole owner remains only inside `PreparedBackNoTargetPublication` |
| `back_no_target_source_is_not_ecs_reachable` | compile/static gate proves ECS compiles without importing `FrozenHistorySource` or inspecting candidate/drift internals; only the public doc-hidden Core refreeze outcome crosses the crate boundary |
| `core_preparation_receivers_do_not_name_ecs_preflight_evidence` | static source gate proves `LifecyclePreflightEvidence` occurs only in ECS crate-private physical/outcome surfaces and never in Core source、`RouterEngine` inherent signatures or Core-owned outcomes; every Core receiver mismatch uses opaque `PreparationReceiverMismatchView` |
| `back_no_target_permanent_capacity_contradiction_terminals_once` | permanent domain/capacity contradiction has no live producer、never retries and terminals once |
| `back_no_target_ready_terminal_closes_all_lifecycle_owners` | canonical-zero bind installs the whole field and drives `HistoryIntentRejected → CleanupPending`; same-schedule driver takes it whole、proves exact-zero digest/count and creates the whole completion owner. Core exact completion reaches `Failed(BackTargetUnavailable)`; mismatch restores the whole settled owner and exact terminal leaves entry fields plus action、ticket、capability、candidate、continuation、reservation、completion and lifecycle owner empty |
| `back_no_target_authority_settlement_terminals_once` | pre-proof AuthorityLost freezes complete physical source/protected/allowed-write/manifest and produces one `{ physical, settlement, retirement }` disposition; unique dispatch mismatch restores it whole, exact dispatch only enters existing authority-specific `Recovering`, and existing scoped completion terminals once with no normal trace/ledger/continuation or outstanding authority owner |
| `back_target_physical_permanent_terminal_closes_all_lifecycle_owners` | post-splice physical permanent first prepares then uniquely dispatches its whole frozen-cleanup + Core-settlement disposition; mismatch restores it whole, physical completion consumes the pre-frozen branch and stored settlement, and terminal leaves zero action、ticket、capability、splice、continuation、cleanup or settlement owner |
| max-depth and duplicate entries | real shallow prefix burns unused ids; duplicate-location back completes one exact publication by entry identity |
| negative capability | compile-fail/facade plus static review prove accepted/capacity/handoff owners cannot be cloned、constructed、inspected or converted; no runtime field-absence assertion |

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib --no-run -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_resolution_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_resolution_handoff_foreign_and_duplicate_are_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib accepted_back_view_exact_lineage_rejects_cross_scope -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib verified_back_resolution_recomposes_bound_capacity_for_all_branches -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail verified_back_handoff_cannot_reenter_raw_consumers -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_trusted_commit_callbacks_are_not_facade_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_no_target_logical_zero_approval_requires_exact_core_candidate -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_no_target_approval_cannot_be_forged_or_replace_physical_proof -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_ready_moves_exact_owner_without_publication -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_pending_rejected_converts_exact_owner_before_public_state -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_authority_lost_converts_exact_owner_before_public_state -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_retry_returns_exact_owner_capability_and_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_resume_action_follows_real_reconciliation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_dispatcher_consumes_verified_bound_target_through_normal_publication_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_driver_failure_report_consume_uses_real_dispatcher -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_atomic_applied_report_uses_real_dispatcher -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue back_target_frame_task_failures_preserve_one_typed_execution_owner -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue back_target_dispatch_owners_are_linear_and_exact -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_mount_exhaustion_is_failure_cleanup_not_resume_action -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_target_resume_action_is_not_facade_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_receiver_mismatch_calls_zero_and_exact_receiver_calls_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_prepares_all_fallible_work_before_retirement_callback -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue back_target_callback_tail_is_infallible_and_query_free -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_zero_entering_commit_needs_no_frame_report -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_zero_entering_preserves_physical_contract -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_and_contradiction_receiver_mismatch_never_commits_retirement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_resume_source_drift_preserves_owner_and_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_physical_permanent_preserves_spliced_owner_and_disposition -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_retry_preserves_owner_and_public_snapshot -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_contradiction_terminals_once_without_world_or_history_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_proven_empty_protects_sparse_current_topology -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_preparation_branches_preserve_exact_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_unreachable_pending_rejected_becomes_permanent_owner -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_refreeze_receiver_mismatch_never_commits_transfer -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_source_drift_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_ready_receiver_or_issuer_mismatch_never_binds -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_first_publication_has_installed_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_permanent_capacity_contradiction_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_ready_terminal_closes_all_lifecycle_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_authority_settlement_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_physical_permanent_terminal_closes_all_lifecycle_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_no_target_source_is_not_ecs_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue core_preparation_receivers_do_not_name_ecs_preflight_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue history_no_target_matrix_rows_are_canonical -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
```

Expected: scaffold compiles; behavioral groups fail on named production predicates and negative gates pass only for intended privacy/non-Clone reasons. Both new ECS `--lib` focused commands must compile、reach the real dispatcher/predicate and fail on their named missing behavior before implementation；missing symbols、setup failure、placeholder panic or direct outcome injection invalidates the RED

- [ ] **Step 8: Implement live resolution, typed handoff and disposition**

1. Core consumes queued accepted owner, performs one live preview and emits Target、NoTarget or Contradiction through `EngineAction::BackResolution`
2. target matching/Core logical reconciliation consume the real maximum-pool prefix and burn unused ids; no selector comes from requested、bool or snapshots
3. Core request record 固定字段 `back_resolution` 在非 Back 与 queued Back raw handoff 生成前为 `None`，生成 exact observed evidence 后且 action 对外可见前安装 `Some(Current)`，exact `verify_back_resolution_handoff` 的 `Matched` 原位改为 `Some(Consumed)`，其他路径保持 `Current`。ECS `take_back_resolution_handoff` 先保存 Clone、non-authorizing observed evidence，再移动 accepted view + raw handoff；`Matched` wrapper 只拆为 accepted view + 一个 branch-specific verified owner，ECS 将该 view 与原 capacity 重组为同一个 `BoundBackCapacityReservation`。随后 `RouterEntry::validate_back_resolution_evidence(&saved_evidence)` 只委托 Core classifier 并映射，retained path 先要求 supplied/stored exact equality，evicted path 要求 exact opaque scope + `evicted_through` + `highest_issued`，必须得到 `ConsumedReplay`。foreign saved observed evidence 保持 `Current`，Core mismatch diagnostic evidence 是 non-observed-form 并分类 `Foreign`。`Current` 不得 terminal/evict，只有 `Consumed` 可进入 terminal retention/eviction。gate 断言 Matched 唯一 Core-private 写入是 exact record state，其余 Core、World/history/public 不变。ECS 不建 ledger、不读取 opaque field，capacity retirement evidence 不能替代 duplicate evidence，测试不能注入 disposition，移除或绕过任一真实 predicate 时必须失败。Supported production callsites 继续依赖 enqueue 建立的 Core action + exact bound reservation crate-private closed invariant，不新增 `MissingCapacity`、domain quarantine 或 recovery branch
4. Every later Core entry point consumes only its verified branch type: target resume cannot accept raw target handoff, no-target refreeze cannot accept raw candidate, and contradiction terminal cannot accept raw contradiction or another verified branch
5. Verified NoTarget stays paired with the recomposed bound reservation；only `begin_bound_back_no_target` may transfer it. Issuer preflight `Err` creates no guard/successor、does not call Core and returns `CapacityRejected` with the full bound. Successful preflight yields `OutstandingToLeased` guard + prepared permit while the bound remains outside the closure. Core mismatch calls the callback zero times、drops guard/successor and returns `ReceiverMismatch` with the full bound；the exact receiver first rebuilds and exact-refreezes source into `PreparedUnchanged | PreparedDrift`，then calls only `guard.commit()` once，after which its Core tail moves the prepared branch. The exact outer arm moves the stale bound into `into_permit_after_exact_receiver` to produce candidate+permit or drift+permit

Target direction remains existing Core publication flow

```text
ECS exact take retains the complete bound capacity outside the callback
-> entry-owned issuer preflights a lifetime-bound guard + private prepared retirement evidence
-> Core rejects a wrong receiver before preparation and callback
-> exact Core receiver refreezes source/host/slot baselines and prepares matcher/reconciliation/action/mount issue
-> PreparedReady | PreparedFailureCleanup | PreparedPermanent
-> retirement callback only moves guard.commit and burns the live capacity identity exactly once
-> exact outer outcome drops the stale bound and seals the prepared retirement evidence
-> callback tail performs occupied-entry remove + cell in-place assignment only
-> same exact-entry dispatcher requires existing retirement classifier == ConsumedReplay and explicitly ends the non-authorizing evidence while installing one typed branch
-> Ready stores current_history_capability and issues HistoryExecutionBindingView + continuation + BackTargetResumeAction
-> the single entry stage field installs ResumeBlocked or Driving；no second safe-stop Option
-> Core opaque pairing predicate validates action injection and every later physical owner transition
-> FrameTask: report/apply the real task to obtain CommitTicket
   MountRequest: keep the complete initial Ready owner in ResumeBlocked::Start and safe-stop until the target action slice exists；do not enter the generic mount handshake
   Commit: use the returned CommitTicket directly with no frame report
-> target-active FrameTask failure proves ProvenZero or retains Residual，then whole-moves Driving + task + physical + kind/message into ResumeBlocked::DriverFailure::Unaccepted while pending_failed_branch remains empty
-> report_back_target_failed mismatch reports zero times and restores ReceiverMismatch whole；Matched returns true post-report Cleanup | Fallback | Terminal without raw capacity
-> DriverFailure Reported rejects Residual+Fallback/Terminal；otherwise consume_back_target_driver_failure returns request | FallbackReleased | terminal evidence | whole-owner mismatch
-> root leaf and outlet reports use report_back_target_applied only；Mismatch installs AppliedReportMismatch whole，FrameTask/Commit retain capacity and exact pairing，MountRequest enters target-field safe-stop，true cleanup seals pending as Residual and enters DriverFailure::Reported
-> final-task/ticket pairing exact；Driving whole-transitions to Classifying(TargetPreparationOwner)
-> drive_navigation_system resource_scope passes the same &World to classify_back_target/classify_back_no_target；registry is read only from that World
-> classify_back_target performs pure physical classification only
-> Ready freezes exact ECS FrozenCleanupManifest and Core CleanupManifest before acceptance，then installs ReadyAwaitingAcceptance with the whole pre-accept owner
-> ReadyAwaitingAcceptance whole-take prepares registry lifecycle、claim rebind/root rebase、input gate、focus and presentation plans as borrowed stack capabilities
-> plan failure drops borrowed plans and installs ReadyPlanRetry or ReadyPlanPermanent with the whole pre-accept owner + typed evidence
-> Ready acceptance Mismatch installs ReadyAcceptanceMismatch with that owner；PendingRejected/AuthorityLost mismatch keeps reconstructed TargetPreparationOwner + original classifier evidence + mismatch
-> with plans live in the same resource_scope borrow，Matched installs/whole-takes ReadyAccepted，reserves the ledger，then installs/whole-takes Prepared for bridge validation
-> acceptance mismatch、reserve failure or bridge failure first drops plans with zero write，then stores the existing complete failure stage；reserve failure alone uses ReadyReserveFailure
-> Validated existing publisher closure only applies prepared plans and binds the ledger；atomic tail has no allocation、query or validation
-> normal success leaves no ReadyAccepted/Prepared and never crosses a schedule boundary or yield
-> unique prepared history/publication owner

PreparedFailureCleanup -> existing failure-cleanup owner chain only
PreparedPermanent -> pre-splice typed permanent/authority settlement only
```

Core never constructs ECS `BackTargetReadyAcceptanceOwner`、`PreparedCleanupLedger` or the entry stage enum

- target resolution stays inside `resume_bound_back_target(&mut RouterEntry, bound)`: the entry-owned issuer first performs `preflight_bound_retirement(&mut issuer, &bound, Target)` to create a lifetime-bound guard + private prepared evidence while `bound` remains outside the closure；wrong engine returns the verified target before preparation、calls the closure zero times、drops the uncommitted guard/evidence and lets ECS restore the complete bound branch；exact receiver refreezes history source、host/top-slot and active/child-slot baseline，再完成 matcher、reconciliation、generation/depth validation、first-action construction and mount issue into private `PreparedReady | PreparedFailureCleanup | PreparedPermanent` before invoking a callback that only moves `guard.commit()` once。After the exact outer outcome returns，ECS unconditionally drops the stale bound and seals the prepared evidence into the returned `Resumed | Permanent` retirement evidence；guard commit never performs lookup/index/query、allocation、validation、`expect`、`unwrap`、unsafe or action-domain `unreachable!`
- the exact-entry dispatcher treats returned `BackCapacityRetirementEvidence` as non-authorizing terminal evidence，requires the existing issuer classifier to report exact `ConsumedReplay` and explicitly ends it in the same dispatch that installs Ready、FailureCleanup or Permanent. Any branch not installed immediately enters the same field's `ResumeBlocked { complete start/owner/bound + retirement/evidence/error }` subenum；retirement never enters `TargetPreparationOwner` or the failure-cleanup chain，and no second dispatch safe-stop field is allowed
- `Resumed(BackTargetStart::Ready)` carries one linear `FrameTask | MountRequest | Commit` action with the same execution binding and continuation. ECS installs `Driving` only after `validate_back_target_execution_pairing` exact-matches `FrameTask | Commit`；retry restore、pending frame/final task → real ticket and commit entry rerun the same borrowed predicate. Mismatch retains the complete unchanged stage owner + opaque view，never reconstructs authority or retries every tick. FrameTask obtains its commit through real report/apply and direct Commit performs no frame report. Initial Ready MountRequest remains whole as `ResumeBlocked::Start` with ordinary mount fields empty and safe-stops until the target MountRequest action slice is implemented；the generic handshake never holds this owner
- every target-active FrameTask failure first freezes same-World physical as `ProvenZero { PendingCommit, proof } | Residual(FailedPreparedBranch)`，derives mode only from that variant and installs `DriverFailure::Unaccepted` with ordinary failed field empty. ProvenZero scans task lineage + registry/relationship closure，proves target failure manifest zero and excludes protected active；the empty failed-branch shortcut is forbidden. Atomic failure/consume returns exact typed owners，Residual+Fallback/Terminal fail-closes without consume. Atomic applied report exclusively handles leaf/outlets；cleanup directly becomes Residual，FrameTask/Commit keep exact pairing，MountRequest safe-stops whole in the target field with ordinary mount field empty. Generic consuming report、`expect`、private-inner/action/Option inference are forbidden；paired `PendingFrameDrive` identity retry remains non-failure
- the minimum root FrameTask slice must reach a real `CommitTicket`、exact splice、physical classifier and typed Core Ready acceptance before commit preparation. Every variant transition is one whole-take；other resume/classifier/acceptance branches remain in `ResumeBlocked | Classifying | CommitFailure` with public zero-write until their dedicated consumer exists
- `Resumed(BackTargetStart::FailureCleanup)` either atomically moves the real `FailureCleanupRequest` into the existing failure-cleanup chain or remains whole in `ResumeBlocked::Start`；it never constructs `TargetPreparationOwner`、splice or publication owner，and mount issue exhaustion reaches only this branch
- same-engine source/host/baseline drift and generation/depth contradiction return typed `Permanent/AuthorityLost` after the exact receiver calls retirement once，not `Mismatch`，only this pre-splice branch enters `settle_back_target_permanent`，which delegates to Core `settle_back_target_permanent_exact`，settlement wrong engine preserves the Core owner + evidence and exact success returns existing AuthorityLost terminal/settlement evidence
- after a real `CommitTicket`, ECS whole-takes `Driving`、builds exact splice and installs `Classifying::Active(TargetPreparationOwner)` before calling pure physical `classify_back_target`. Cross-entry mismatch、Retry and outer Permanent retain the complete raw owner + typed evidence in the corresponding `Classifying` variant. PendingRejected/AuthorityLost remain `*Unaccepted { owner, evidence }` until their typed Core receiver runs；Matched acceptance converts capacity once into `PendingRejected { physical: BackTargetClassifiedPhysicalOwner, continuation, evidence }` or `AuthorityLost { physical: BackTargetClassifiedPhysicalOwner, settlement, evidence }`，while acceptance Mismatch restores raw `TargetPreparationOwner` + original `LifecyclePreflightEvidence` + opaque view in its branch-specific variant. These branches do not reserve or reuse the normal Ready ledger；their dedicated consumer later prepares its own failure/authority scope. None may enter a generic chain without that whole-owner consumer
- pure physical Ready classification completes exact Entity manifest freeze and Core manifest construction before installing `ReadyAwaitingAcceptance { physical, raw continuation, frozen_manifest, manifest }`. Its scoped driver whole-takes this owner and prepares existing registry lifecycle、claim rebind/root rebase、input gate、focus and presentation plans as borrowed stack capabilities before acceptance；failure drops plans and installs `ReadyPlanRetry | ReadyPlanPermanent` with the whole pre-accept owner and typed evidence. With plans live in the same `World::resource_scope` borrow，Matched acceptance installs/whole-takes `ReadyAccepted`，reserve success installs/whole-takes `Prepared`，then `prepare_back_target_commit` validates and the existing publisher applies only those plans. Acceptance mismatch、reserve failure and bridge failure first zero-write drop plans before saving the complete existing stage owner；reserve failure alone uses `ReadyReserveFailure` with returned frozen manifest/kind. Normal success leaves no ReadyAccepted/Prepared，crosses no schedule boundary/yield，and its atomic tail performs no allocation、query、validation、freeze or owner/plan reconstruction
- target physical outer `Permanent` remains `PreparationPermanentOwner<TargetPreparationOwner>` and only `settle_back_target_preparation_permanent` consumes it, returning it on engine mismatch or preparing the whole frozen splice cleanup + Core authority settlement disposition. `dispatch_target_preparation_permanent` is the unique consumer: it validates exact entry/Core receiver before mutation, returns the whole disposition on mismatch, and exact-installs it as one field. Physical cleanup then consumes the pre-frozen original branch and the same stored settlement; it never infers continuation from obligation count、flows through the pre-splice Core drift consumer or reaches the publisher
- verified no-target candidate owns only unresolved lifecycle lineage. `consume_prepared_back_no_target` issuer preflight `Err` creates no guard/successor、does not call Core and returns `CapacityRejected` with the complete owner. Successful preflight yields `LeasedToCanonicalZeroSlot` guard + prepared slot successor；Core mismatch calls the callback zero times、drops both and returns `ReceiverMismatch` with the complete owner. Exact Core calls only `guard.commit()` once，then the outer arm moves the stale permit into `into_slot_after_exact_receiver` and returns sealed `PreparedBackNoTargetPublication` with zero public write. Only its later publication closure binds that slot to the batch and stores the initialized ledger + whole installation before Core publishes
- pre-proof authority loss and permanent fail-closed may still enter existing settlement/terminal, but never publish the normal no-target `HistoryIntentRejected` or `CleanupPending` trace. The specialized `BackNoTargetPreparationOutcome` has no ordinary PendingRejected arm: unreachable PendingRejected converts directly to `Permanent`; AuthorityLost success returns `BackNoTargetAuthoritySettlementDisposition { physical, settlement, retirement }`; Retry、issuer preflight `CapacityRejected` before Core call and later Core `ReceiverMismatch` after callback zero each return the complete original owner. The unique authority dispatch validates exact entry/Core receiver before mutation, restores the whole disposition on mismatch, and exact-reuses only existing scoped authority settlement with authority-specific `Recovering`. Permanent capacity/domain contradiction retains `PreparationPermanentOwner<BackPreparationOwner>` for `settle_back_no_target_physical_permanent`
- source drift uses opaque raw `BackNoTargetSourceDriftOwner` paired with the original permit. `settle_back_no_target_source_drift_permanent(&mut RouterEntry, owner)` is the only ECS API: it keeps the full composite outside the callback, obtains a lifetime-bound retirement guard + private prepared evidence，then calls the Core exact drift consumer. Issuer mismatch returns the full composite; wrong engine calls the closure zero times、drops guard/evidence and returns raw drift for recomposition with the permit; exact receiver calls only `guard.commit()` once、terminals immediately，then ECS drops the stale permit and seals the prepared evidence. No prepared authority crosses the entry/issuer boundary
- pre-proof no-target AuthorityLost stays inside `classify_back_no_target(world: &World, entry: &mut RouterEntry, owner)`，which reads registry only from that World: after classification freezes the complete physical source/protected closure、allowed-write set、authority manifest and boundary evidence, issuer preflight returns a lifetime-bound retirement guard + private prepared evidence and the Core exact authority consumer uses the same callback rule，returning the full original owner after zero-call guard drop on mismatch or calling only `guard.commit()` once before the exact outer arm drops the stale permit and produces the whole physical + Core-settlement + ECS-retirement disposition
- contradiction stays inside `fail_bound_back_contradiction(&mut RouterEntry, bound)`: the entry-owned issuer first obtains a lifetime-bound guard + private evidence from `preflight_bound_retirement(&mut issuer, &bound, Contradiction)`, then the Core exact consumer either returns the verified contradiction with closure count zero、drops guard/evidence and lets ECS restore the bound branch，or calls only `guard.commit()` once before the exact outer arm drops the stale bound、seals retirement evidence and reaches existing `Failed(AuthorityLost)` with World/presentation/history zero-write and no outstanding owner or retry

Across these helpers, issuer preflight `Err` always occurs before guard construction and Core call，returning `CapacityRejected` with the complete owner；only a later Core mismatch returns `ReceiverMismatch` after callback zero and uncommitted guard/successor drop. Exact callbacks only move `guard.commit()` after all preparation. Guard commit matches a prebuilt operation-specific transition：retire removes the held live entry、returns lifecycle to `Free` and applies `Reusable | PermanentlyRetired` metadata；transfer retains and updates it to `Leased`；canonical-zero consumes the held `Leased` entry and transfers its exact cell lease into the slot. It performs no owner/key lookup、key remove、Vec index/query、allocation、validation、`expect`、`unwrap`、panic or unsafe. Retirement drops its stale owner then seals evidence；transfer and canonical-zero move their stale owner into the prepared successor conversion. This rule applies to target/contradiction retirement、no-target transfer、drift/authority retirement and canonical-zero bind；none may reintroduce owner-by-value + expectation callbacks、Option owner slots or Core-owned issuer guards

Before Step 9, drive all publication/closure gates GREEN: the first normal no-target public observation must already have the exact whole ledger+installation field; that branch must then reach `BackTargetUnavailable` after same-schedule whole-owner exact-zero completion with no outstanding entry field/action/ticket/capability/candidate/continuation/reservation/completion/lifecycle owner. No-target AuthorityLost must uniquely dispatch its whole physical/Core/retirement disposition into authority-only settlement and terminal once without a normal trace. Post-splice target physical permanent must uniquely dispatch and complete its frozen physical cleanup and stored Core authority settlement to terminal with no outstanding splice/cleanup/settlement or lifecycle owner

Rerun every Step 4 and Step 7 focused command, including these exact GREEN filters and static contracts

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_resume_action_follows_real_reconciliation -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_dispatcher_consumes_verified_bound_target_through_normal_publication_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_driver_failure_report_consume_uses_real_dispatcher -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_atomic_applied_report_uses_real_dispatcher -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue back_target_frame_task_failures_preserve_one_typed_execution_owner -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue back_target_dispatch_owners_are_linear_and_exact -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_mount_exhaustion_is_failure_cleanup_not_resume_action -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_receiver_mismatch_calls_zero_and_exact_receiver_calls_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_target_prepares_all_fallible_work_before_retirement_callback -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue back_target_callback_tail_is_infallible_and_query_free -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_resolution_handoff_foreign_and_duplicate_are_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_retry_preserves_owner_and_public_snapshot -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_contradiction_terminals_once_without_world_or_history_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_proven_empty_protects_sparse_current_topology -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_preparation_branches_preserve_exact_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_authority_settlement_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_no_target_source_is_not_ecs_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue core_preparation_receivers_do_not_name_ecs_preflight_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue history_no_target_matrix_rows_are_canonical -j 1
```

Expected: all commands exit `0` before public `back` is exposed

- [ ] **Step 9: Expose public back only after success, no-target and terminal owner-closure paths pass**

Do not remove the public exclusion until `back_no_target_first_publication_has_installed_owners`、`back_no_target_ready_terminal_closes_all_lifecycle_owners`、`back_no_target_authority_settlement_terminals_once` and `back_target_physical_permanent_terminal_closes_all_lifecycle_owners` are GREEN in addition to every Step 4/7 gate

Add

```rust
pub fn back(
    &mut self,
    router: RouterHandle,
) -> Result<NavigationRequestId, NavigationEnqueueError>;
```

Then

- remove only `bevy_ui_router::ecs::RouterNavigation::back` from spec section `10.1`
- keep `late-bound back complete evidence` planned
- keep History authority addition planned

Add public C1 evidence

| Test | Required evidence |
| --- | --- |
| `history_c1_replace_then_back_restores_prior_route` | push A, push B, replace B2, back to A with requested `Some(A)`, exact cursor, active mapping, leaving receipt and terminal success |
| `history_back_no_entry_is_synchronous_and_creates_no_request` | single-entry published snapshot, back returns `NoBackEntry`, request collection and router snapshot remain byte-for-byte equivalent |
| `history_back_then_push_truncates_forward_tail` | publish A/B/C, back to B, push D, assert entries A/B/D with cursor 2 and no replayable C identity |

C1 must assert target requested `Some`, exact active/history cursor, cleanup receipt, protected ancestor/slot identity and terminal success

- [ ] **Step 10: Verify Back execution in layers**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib request::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib history_authority::tests::back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_resolution_handoff_foreign_and_duplicate_are_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_retry_preserves_owner_and_public_snapshot -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_contradiction_terminals_once_without_world_or_history_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_proven_empty_protects_sparse_current_topology -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_preparation_branches_preserve_exact_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_authority_settlement_terminals_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_no_target_source_is_not_ecs_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue core_preparation_receivers_do_not_name_ecs_preflight_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue history_no_target_matrix_rows_are_canonical -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit back_capacity_order_observer_installs_across_crates_read_only -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_no_target_logical_zero_approval_requires_exact_core_candidate -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_and_contradiction_receiver_mismatch_never_commits_retirement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_refreeze_receiver_mismatch_never_commits_transfer -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_ready_receiver_or_issuer_mismatch_never_binds -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_first_publication_has_installed_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_ready_terminal_closes_all_lifecycle_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_target_physical_permanent_terminal_closes_all_lifecycle_owners -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c1_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit history_back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_target_resume_action_is_not_facade_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_trusted_commit_callbacks_are_not_facade_reachable -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail back_no_target_approval_cannot_be_forged_or_replace_physical_proof -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all gates exit `0`, the no-target first-publication gate and all three Task 4 terminal owner-closure tests (normal no-target、no-target authority and post-splice target permanent) are GREEN, both History public methods are no longer excluded, complete C3 evidence remains planned

- [ ] **Step 11: Independent Back execution review and commit**

Reviewers verify

- admission stamp cannot authorize target
- ECS capacity exists before Core request allocation
- running target binds exact entry identity, not location equality
- requested target is single-assignment
- no-target uses no fake frame/task identity，zero-entering target uses a real direct Commit，nested target may use a real MountRequest
- retry owner/evidence is returned intact
- matched resolution cannot recover raw handoff and every Core branch consumer requires the exact verified owner
- `BackTargetResumeAction` and `BackTargetStart` remain absent from facade exports and cannot be named through the supported facade; the facade export golden/profile remains unchanged
- callback-bearing `*_with_*` methods and the prepared no-target final consumer are doc-hidden trusted ECS integration APIs, not facade reexports; the supported facade/adapter cannot obtain their verified/prepared owners, and the callback is explicitly not authority
- target preparation covers history source、host/top-slot、active/child-slot baseline、matcher/reconciliation、generation/depth and mount issue before retirement，with `FrameTask | MountRequest | Commit` as the complete normal action domain and mount exhaustion as an independent FailureCleanup branch
- target issuer preflight `Err` returns `CapacityRejected` before guard/Core call；wrong receiver returns `ReceiverMismatch`、drops the uncommitted lifetime-bound guard and calls retirement zero times；exact Ready、FailureCleanup and Permanent branches call only `guard.commit()` once after complete preparation，then drop the stale bound and seal prepared retirement evidence
- same-engine source/host/baseline drift and generation/depth contradiction are typed Permanent/AuthorityLost，not Mismatch，unused identities from the full reserved pool remain permanently burned
- operation-specific static gates prove retire removes the already-held live entry、returns lifecycle to `Free` and applies `Reusable | PermanentlyRetired` metadata，transfer retains it and updates `Outstanding → Leased`，canonical-zero consumes the held `Leased` entry and moves its cell lease into the slot；callbacks only move `guard.commit()` and contain no owner/key lookup、key remove、Vec index/query、allocation、validation、`expect`、`unwrap`、panic or unsafe。Transfer/canonical-zero successor conversion consumes the stale owner by value；retirement alone drops stale owner before sealing evidence
- every mismatch reconstructs the complete original verified owner/composite, including its bound reservation or permit
- no-target classification uses its specialized outcome: Retry returns the original owner、AuthoritySettlement linearly owns the whole frozen physical settlement + Core settlement + ECS retirement disposition、unreachable PendingRejected maps directly to Permanent
- Core source、`RouterEngine` signatures and Core-owned outcomes never name ECS crate-private `LifecyclePreflightEvidence`; Core receiver mismatch is always opaque `PreparationReceiverMismatchView`, while ECS evidence remains paired with the exact ECS owner
- Core is the sole producer of opaque no-target logical-zero approval after exact receiver/candidate/source validation; approval cannot be forged and never replaces the independent ECS physical `ProvenEmptyProjection`
- the persistent World domain issuer signs exactly one issuer per `RouterEntry`; second production install rejects before domain issue without replacement/identity consumption/outstanding drift, while test-support producer `AlreadyInstalled` remains an independent entry-local journal rule
- capacity cells remain in the exact entry table and have no fixed install-time pool; growth is fail-closed before mint/Core reach, reclaim advances generation, stale ABA evidence returns the live owner and a no-successor cell is never reused
- reservation storage is fully initialized and safe; canonical-zero installs initialized empty storage without allocation、uninitialized memory or unchecked conversion
- normal no-target Ready returns a sealed prepared publication with zero public write; its sole installation closure stores one whole `InstalledBackNoTargetCleanup { ledger, installation }` before the first `HistoryIntentRejected`. The same-schedule driver takes it whole、proves exact-zero digest/count and forms the Core completion owner; mismatch restores the whole settled owner and exact terminal clears both entry fields plus all action/ticket/capability/continuation/completion owners
- no-target AuthorityLost freezes complete source/protected closure/allowed-write/manifest, uniquely dispatches the whole physical/Core/retirement disposition into existing authority-only settlement, never creates normal trace/ledger/continuation, and terminals once with no outstanding authority owner
- post-splice target physical permanent uniquely dispatches the whole frozen-cleanup + Core-settlement disposition, selects only the pre-frozen original branch after cleanup, and reaches terminal with zero outstanding splice、continuation、cleanup、settlement or lifecycle owner before public back exposure
- protected sparse topology remains zero-write

```powershell
git add -- crates/bevy_ui_router_core/src/history_authority.rs crates/bevy_ui_router_core/src/ids.rs crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/src/matcher.rs crates/bevy_ui_router_core/src/request.rs crates/bevy_ui_router_core/src/trace.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/src/test_support.rs crates/bevy_ui_router_ecs/Cargo.toml crates/bevy_ui_router_ecs/src/app_ext.rs crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/test_support.rs crates/bevy_ui_router_ecs/src/branch_transaction.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/snapshots.rs crates/bevy_ui_router_ecs/tests/dependency_boundary.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/support/facade_contract_cases.rs docs/facade-associated-api-golden.txt docs/superpowers/specs/2026-07-20-history-design.md
git diff --cached --name-only
git commit -m "feat: add late-bound back execution"
```

---

### Task 5: Close Back failure and authority semantics

**Files**

- Modify: `crates/bevy_ui_router_core/src/history_authority.rs`
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`
- Modify: `crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_journeys.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`
- Modify: `docs/superpowers/specs/2026-07-20-history-design.md`

**Interfaces**

- Consumes: complete Back execution owner chain from Task 4
- Produces: C2/C3 both branches, authority precedence, stale/replay closure and final History authority catalogue row
- Preserves: postcommit compound settlement remains Recovery-owned

- [ ] **Step 1: Write C2 and both C3 public RED journeys**

Add exact named tests

| Test | Setup |
| --- | --- |
| `history_c2_failed_back_preserves_cursor_and_resolved_target` | publish A/B, make A build or apply fail, enqueue back and drive fallback/cleanup to terminal |
| `history_c3_first_success_makes_second_back_no_target` | publish A/B, enqueue two backs before update, allow first to succeed |
| `history_c3_first_failure_retargets_second_back_to_same_entry` | publish A/B, enqueue two backs, fail first before publication, allow second to succeed |
| `history_back_from_error_presentation_uses_published_history` | reach Error presentation with history A/B, enqueue back, assert resolution uses history A rather than Error active location |

Required assertions

| Journey | First terminal | Second requested | History | Active |
| --- | --- | --- | --- | --- |
| C2 | original build/apply failure after fallback settlement | `Some(/items/1)` | cursor unchanged | error or preserved presentation per failure branch |
| C3 first success | success | `None` at `BackTargetUnavailable` terminal | cursor at start | first target remains active |
| C3 first failure | failed | `Some(/items/1)` | first failure leaves cursor unchanged, second success moves once | second target active |

All three assert exact trace, receipt ordering, no fallback for no-target and zero outstanding lifecycle owners at terminal

Run each journey before changing failure or authority production behavior

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c2_failed_back_preserves_cursor_and_resolved_target -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c3_first_success_makes_second_back_no_target -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c3_first_failure_retargets_second_back_to_same_entry -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_back_from_error_presentation_uses_published_history -j 1
```

Expected: compilation succeeds; C2 and C3-first-failure must exit nonzero on their missing failure-continuation predicates, while C3-first-success and error-presentation characterization may already pass

Record all four statuses and never weaken an already-green invariant to manufacture a RED

- [ ] **Step 2: Write authority, ABA and replay RED tests**

Add focused tests

| Test | Required setup and assertion |
| --- | --- |
| `history_source_aba_is_rejected` | freeze back, advance revision until locations/cursor match old projection, assert exact revision/entry mismatch rejects publication |
| `cross_incarnation_history_replay_is_zero_write` | create a second router with equal locations, replay first router capability/evidence, assert both routers remain exact |
| `precommit_authority_takeover_preserves_history_until_settlement` | drift current/preserved authority before commit, assert history never advances and active clears only after settlement receipt |
| `stale_back_report_receipt_and_publication_are_zero_write` | replay each stale artifact against a newer request, assert history/request/owner/audit snapshots stay exact |
| `active_cycle_enters_authority_settlement` | create a real current relationship cycle plus entering drift, assert AuthorityLost precedence and unique settlement owner |

The active-cycle test must mutate the real observed current relationship closure and pass through the production classifier

It must fail if the authority-loss predicate is removed or short-circuited

Run the focused authority group before Step 3

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib history_source_aba_is_rejected -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib cross_incarnation_history_replay_is_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib precommit_authority_takeover_preserves_history_until_settlement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib stale_back_report_receipt_and_publication_are_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib active_cycle_enters_authority_settlement -j 1
```

Expected: compilation succeeds; ABA/cross-incarnation characterization may already pass, while the command-specific precommit and stale-owner tests must exit nonzero on their named production predicates before Step 3

Record the exact initial status of all five commands and never weaken an already-green invariant to manufacture a RED

- [ ] **Step 3: Implement failure and authority precedence with existing settlement**

Required behavior

- target match/build/apply/mount failure drops unconsumed back capability
- fallback may publish Error presentation but never changes history
- C2 terminal preserves the resolved target in request snapshot
- queued next back does not reuse prior target, capability, attempt or receipt
- precommit host/current/preserved/leaving loss wins over entering rejection and no-target classification
- history stays unchanged during existing authority settlement
- active clears only after settlement completes
- precommit takeover emits HostLost/AuthorityLost path, not `HistoryIntentRejected`
- stale request/attempt/epoch/report/token/receipt cannot affect current history or lifecycle owner
- same scope cannot own both normal cleanup and no-target cleanup
- terminal leaves no outstanding action, ticket, capability, reservation or owner

Do not add postcommit compound settlement, new epoch, host re-admission or reaper behavior

- [ ] **Step 4: Activate final History machine rows**

After production and public tests pass

- set `late-bound back complete evidence` to `active`
- set `active_cycle_enters_authority_settlement` to `active`
- append the exact function name once to `AUTHORITY_REGRESSION_CATALOGUE`
- register one direct `#[test]` wrapper and one binary registration path
- require Root 20 + Nested 3 + History 1 = 24 bidirectional equality

- [ ] **Step 5: Verify Back failure in layers**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib history_source_aba_is_rejected -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib cross_incarnation_history_replay_is_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib active_cycle_enters_authority_settlement -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c2_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c3_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit history_back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
cargo fmt --all -- --check
git diff --check
```

Expected: all gates exit `0`, catalogue is exactly 24, all History-owned staged rows are active

- [ ] **Step 6: Three-angle independent review and commit**

Review responsibilities

- Core reviewer: exact identity, revision, capability, source refreeze, failure and terminal owner uniqueness
- ECS reviewer: manifest-neutral reservation, exact Entity proof, allowed write set, cleanup layering, receipt/replay
- Public reviewer: C1–C3, F1–F3, requested Some/None, exclusions, fixture/golden/profile consistency

Commit only after all reviewers report PASS with no Critical or Important finding

```powershell
git add -- crates/bevy_ui_router_core/src/history_authority.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs docs/superpowers/specs/2026-07-20-history-design.md
git diff --cached --name-only
git commit -m "feat: close history failure authority"
```

---

### Task 6: Run the History completion gate and close facts

**Files**

- Modify after all gates pass: `docs/current-architecture.md`
- Modify after all gates pass: `docs/roadmap.md`
- Modify after all gates pass: `docs/README.md`
- Modify after all gates pass: `docs/authority-transition-matrix.md`
- Modify after all gates pass: `docs/implementation-goals.md`
- Modify after all gates pass: `.superpowers/sdd/progress.md`
- Modify after all gates pass: `docs/superpowers/specs/2026-07-20-history-design.md`
- Modify after all gates pass: `docs/superpowers/plans/2026-07-20-history-implementation-plan.md`

**Interfaces**

- Consumes: completed Replace, Back execution and Back failure slices
- Produces: History PASS fact and Recovery as the next active stage
- Production semantics: none

- [ ] **Step 1: Run focused public journeys and regressions**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit history_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib no_back_entry_precedes_identity_exhaustion -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib retryable_back_preparation_returns_exact_owner_and_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib permanent_back_preparation_contradiction_fails_closed_once -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys reconciliation_journey_b_preserves_layout_and_replaces_leaf -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys root_push_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
& .\scripts\verify-authority-regressions.ps1
```

Expected: C1–C3、F1–F3 and existing A/B/D regressions pass, catalogue exact union passes

- [ ] **Step 2: Run full facade and crate gates**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_view -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_features -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_compile_fail_boundary -j 1
```

Expected: all commands exit `0`

- [ ] **Step 3: Run workspace gates serially**

```powershell
cargo nextest run --locked --workspace --all-features --config "build.rustc-wrapper = ''" -j 1
cargo test --locked --workspace --doc --config "build.rustc-wrapper = ''" -j 1
cargo check --locked --workspace --all-targets --all-features --config "build.rustc-wrapper = ''" -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: every command exits `0`

- [ ] **Step 4: Obtain final independent reviews**

Required PASS records

- Core lifecycle and history authority
- ECS physical authority and cleanup layering
- public facade, canonical facts and stage exclusions

If any review or gate produces a real RED, return to its owning slice and repeat that slice review before resuming completion

- [ ] **Step 5: Update facts only after successful gates**

Record

- exact implementation commit range
- exact successful commands
- History PASS
- Recovery as next stage
- active profile rows and authority count 24
- known Recovery-owned exclusions without claiming MVP PASS

Set plan status to `Complete` and spec status to completed historical evidence

- [ ] **Step 6: Validate documentation closure**

```powershell
rg -n "History PASS|Recovery|24|be25dbe" docs/current-architecture.md docs/roadmap.md docs/README.md docs/authority-transition-matrix.md docs/implementation-goals.md .superpowers/sdd/progress.md docs/superpowers/specs/2026-07-20-history-design.md docs/superpowers/plans/2026-07-20-history-implementation-plan.md
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue -j 1
git diff --check
```

Expected: facts agree, profile remains machine-valid, no current document claims Recovery or MVP complete

- [ ] **Step 7: Commit History completion facts**

```powershell
git add -- docs/current-architecture.md docs/roadmap.md docs/README.md docs/authority-transition-matrix.md docs/implementation-goals.md .superpowers/sdd/progress.md docs/superpowers/specs/2026-07-20-history-design.md docs/superpowers/plans/2026-07-20-history-implementation-plan.md
git diff --cached --name-only
git commit -m "docs: close history stage"
```

- [ ] **Step 8: Verify the exact committed tree**

```powershell
git status --short
git log --oneline --decorate -8
git diff HEAD^ HEAD --check
```

Expected: clean tree, completion commit visible, diff check exits `0`

## Plan Self-Review Checklist

- [ ] Every spec section `1–14` maps to at least one task or explicit non-goal
- [ ] Replace, Back execution, Back failure and Completion match the approved slice ownership
- [ ] `requested Option`、`IdentityExhausted`、late-bound evidence and authority addition each activate in exactly one owning slice
- [ ] public exclusions are removed only after the owning production journey is green
- [ ] exact history identity is private and cannot be reconstructed from snapshots
- [ ] manifest-neutral capacity and exact Entity manifest remain separate owners
- [ ] pure no-target creates no fake frame/task identity
- [ ] retry has only the two named producers and returns the original owner
- [ ] precommit and postcommit authority boundaries are not conflated
- [ ] no Task 6 step adds production semantics
- [ ] every code and edge-case step contains concrete content
- [ ] all later signatures match the interfaces introduced earlier
- [ ] every commit uses an exact staged file list
