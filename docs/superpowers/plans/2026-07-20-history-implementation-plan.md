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
- Modify: `crates/bevy_ui_router_core/src/route_tree.rs`
- Modify: `crates/bevy_ui_router_core/src/matcher.rs`
- Modify: `crates/bevy_ui_router_core/src/request.rs`
- Modify: `crates/bevy_ui_router_core/src/trace.rs`
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_core/src/test_support.rs`
- Modify: `crates/bevy_ui_router_ecs/Cargo.toml`
- Create: `crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/lib.rs`
- Modify: `crates/bevy_ui_router_ecs/src/branch_transaction.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs`
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
    /* exact domain、owner/generation allocator、entry-local persistent cells and exact outstanding registry */
    private: BackCapacityPrivateSeal,
}

pub(crate) enum BackCapacityReserveError { DomainExhausted, OwnerExhausted, LedgerSlotUnavailable }
pub(crate) enum BackCapacityIssuerInstallError { AlreadyInstalled, Reserve(BackCapacityReserveError) }
pub(crate) enum EnqueueRejected { CoreTokenDrift, CoreAllocatorFailure(EnqueueBackErrorKind) }
pub(crate) enum BackCapacityRetireDisposition { Target, Contradiction, AuthorityLost, Permanent }
pub(crate) enum BackCapacityRetireError { ForeignDomain, StaleGeneration, ConsumedReplay }
pub(crate) struct BackCapacityRetirementEvidence { /* exact non-authorizing retired identity */ }
pub(crate) struct BackCapacityRetirementExpectation { /* move-only exact domain/owner/generation expectation; no registry mutation */ }
pub(crate) struct BackCapacityTransferExpectation { /* move-only exact bound owner expectation; no registry mutation */ }
pub(crate) struct BackCanonicalZeroExpectation { /* move-only exact permit/slot expectation; no registry mutation */ }
pub(crate) enum BackCapacityRetireOutcome<Owner> { Retired(BackCapacityRetirementEvidence), Rejected { owner: Owner, error: BackCapacityRetireError } }
pub(crate) struct NoTargetCapacityPermit { capacity: EcsCleanupCapacityReservation }

impl BackCapacityIssuer {
    pub(crate) fn reserve(&mut self) -> Result<EcsCleanupCapacityReservation, BackCapacityReserveError>;
    pub(crate) fn retire_unbound(&mut self, reservation: EcsCleanupCapacityReservation, rejected: EnqueueRejected) -> BackCapacityRetireOutcome<EcsCleanupCapacityReservation>;
    pub(crate) fn preflight_bound_retirement(&self, bound: &BoundBackCapacityReservation, disposition: BackCapacityRetireDisposition) -> Result<BackCapacityRetirementExpectation, BackCapacityRetireError>;
    pub(crate) fn commit_preflighted_bound_retirement(&mut self, bound: BoundBackCapacityReservation, expectation: BackCapacityRetirementExpectation) -> BackCapacityRetirementEvidence;
    pub(crate) fn preflight_no_target_transfer(&self, bound: &BoundBackCapacityReservation) -> Result<BackCapacityTransferExpectation, BackCapacityRetireError>;
    pub(crate) fn commit_preflighted_no_target_transfer(&mut self, bound: BoundBackCapacityReservation, expectation: BackCapacityTransferExpectation) -> NoTargetCapacityPermit;
    pub(crate) fn preflight_no_target_retirement(&self, permit: &NoTargetCapacityPermit, disposition: BackCapacityRetireDisposition) -> Result<BackCapacityRetirementExpectation, BackCapacityRetireError>;
    pub(crate) fn commit_preflighted_no_target_retirement(&mut self, permit: NoTargetCapacityPermit, expectation: BackCapacityRetirementExpectation) -> BackCapacityRetirementEvidence;
    pub(crate) fn preflight_back_canonical_zero(&self, permit: &NoTargetCapacityPermit, projection: &ProvenEmptyProjection) -> Result<BackCanonicalZeroExpectation, BackCapacityRetireError>;
    pub(crate) fn commit_preflighted_back_canonical_zero(&mut self, permit: NoTargetCapacityPermit, expectation: BackCanonicalZeroExpectation) -> ManifestNeutralLedgerSlot;
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
#[cfg(any(test, feature = "test-support"))]
impl RouterNavigation {
    #[doc(hidden)]
    pub fn testing_install_back_capacity_order_observer(
        &mut self,
        router: RouterHandle,
    ) -> Result<BackCapacityOrderObserver, BackCapacityOrderInstallError>;
}

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
- after mint, bind、exact retire、no-target transfer and canonical-zero perform no allocation. Retire burns the owner、advances generation and returns the same cell to `Free`; no-target keeps the same cell `Leased`. A cell whose generation has no successor is sealed permanently and later reserve grows a fresh cell
- `retire_unbound(..., EnqueueRejected)` handles Core token drift/allocator failure and preserves the original reservation on foreign/stale/replay; its success burns the identity before facade return
- bound retirement accepts only target/contradiction/authority/permanent disposition; `EnqueueRejected` remains the separate argument of `retire_unbound`. Bound retirement、no-target transfer、no-target retirement and canonical-zero bind each expose a non-mutating exact preflight plus a locally infallible commit used only by the same helper-local closure; expectation and owner never enter an outcome、field、queue or later helper
- all retire/transfer rejection variants return the original move-only owner; foreign domain、stale generation and replay cannot become `()`/bool, while success returns only non-authorizing retirement evidence and permanently burns the identity
- `RouterNavigation` is already the unique ECS World Resource, so it owns the single persistent `back_capacity_domain_issuer` initialized by `RouterNavigation::new`. It owns no per-entry issuer field. During router install, construct the entire `RouterEntry` locally, ask the domain issuer to sign/install its non-reconstructible `BackCapacityIssuer`, and push the entry only after every install step succeeds. Failure maps `BackCapacityIssuerInstallError::Reserve` to existing `IntegrationError::Exhausted` and impossible local-entry `AlreadyInstalled` to existing `IntegrationError::Invalid`; it leaves no ghost entry or partially installed entry
- `install_back_capacity_issuer` first checks the exact entry's issuer slot. `AlreadyInstalled` returns before `BackCapacityDomainIssuer::issue`, so a second install cannot consume a domain identity、replace the issuer or alter its outstanding registry; only a vacant slot may map `issue` failure to `BackCapacityIssuerInstallError::Reserve`. This production error is separate from test-only journal-channel `BackCapacityOrderInstallError::AlreadyInstalled`
- under `test-support`, the upper helper resolves that entry before channel creation, moves the Core producer into its engine and moves issuer + facade producers into test-only slots on the same entry. `RouterNavigation::back` records facade return only after resolving that entry and only through its facade slot. Tests receive only `BackCapacityOrderObserver`; Core defines only layer-neutral channels and never depends on ECS cleanup types. Public audit covers only legitimately obtainable foreign and already-installed handles; same-domain unknown uses the existing ECS-private handle fixture
- set the ECS feature exactly to `test-support = ["bevy_ui_router_core/test-support"]`. Core channel definitions and `issue_back_capacity_order_channels` live in `crates/bevy_ui_router_core/src/test_support.rs`; ECS only installs their sealed producers. No `app_ext.rs` or plugin change is required

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
   - reserve and exact-retire one real cell, reserve again and assert the same reusable cell has a new owner/generation; stale expectation/evidence cannot retire、transfer or authorize the current owner and every rejection returns the current move-only owner
   - also drive the real no-successor predicate: a generation-exhausted cell is never reused and the next successful reserve grows a different cell
   - GREEN only after growth preflights every fallible capacity before mint and reclaim/generation transitions are allocation-free
   - after those growth/reclaim gates are GREEN, add `back_capacity_issuer_second_install_preserves_outstanding` using one real reserved owner; assert a second install leaves the exact cell、outstanding owner and generation unchanged

3. **Read-only cross-crate observer install**
   - RED `back_capacity_order_observer_installs_across_crates_read_only` in the public audit covers two exact routers、per-entry journal isolation、a legitimately obtained foreign handle and already-installed rejection
   - RED `back_capacity_order_observer_same_domain_unknown_is_zero_write` stays in ECS unit coverage and uses the existing private handle fixture; do not add a public forged-handle constructor
   - GREEN only when all producer slots are preflighted before Core factory creation、each producer is sealed and single-install、tests receive only the observer and every reject is zero-write with no partial journal installation

4. **Real Core allocator failure order**
   - RED `back_capacity_precedes_core_reservation_failure_evidence`
   - for every real Core request/transaction/batch/frame/task pool counter, record only production boundaries and assert `ECS issue → Core allocator attempt/failure → ECS exact retire → facade typed return`
   - GREEN requires exact sequence、burned owner identity、advanced reclaim generation、no request/queue/public state and no injectable classification

5. **Queued accepted-reservation safe stop**
   - wire the accepted view and single Core queue only after micro-slices 1–4 are GREEN
   - stop at the queued unresolved owner boundary described below

- acceptance infallibly combines the ECS owner with the view returned by `AcceptedBackReservation::into_parts` into `BoundBackCapacityReservation`; at resolution, Core Matched returns the same accepted view beside one verified branch owner, so ECS recomposes that view with the original capacity into the same bound reservation. Mismatch returns raw view + raw handoff so ECS likewise reconstructs the original bound reservation
- `BackAcceptedReservationProjection` is independent from `HistoryExecutionBindingProjection` and binds only exact Core world seal projection、router/history incarnation、request、transaction、cleanup batch 与 opaque accepted branch-reservation identity/pool
- `BackAcceptedReservationView` has private fields, no public constructor and no method that can mint/reconstruct a Core owner or execution binding
- ECS consumes `AcceptedBackReservation::into_parts`, moves the view into capacity binding, then returns the copied request id to facade only after that move completes
- `request()` is read-only convenience and never authorizes binding or publication
- the accepted view binds no normal attempt、destination、eventual host/slot/frame/task/generation or branch selector
- the accepted view carries no authority to consume the branch pool、resolve a target or construct any later owner
- `AcceptedBackReservation` exposes only non-authorizing accepted lineage

Install the order channel only after the supplied `RouterHandle` resolves to the exact `RouterEntry` that owns engine + issuer + facade producer slot: call the Core factory once, install all three producers on that entry, then return the observer. Resolve foreign/already-installed identity against entry-local slots before factory creation; the ECS-private same-domain unknown case follows the same zero-write rule. Two entries cannot share a journal and `AlreadyInstalled` is decided per entry

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
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_order_observer_same_domain_unknown_is_zero_write -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit back_capacity_order_observer_installs_across_crates_read_only -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_precedes_core_reservation_failure_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib no_back_entry_precedes_identity_exhaustion -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
```

Expected: all nine gates exit `0`; admission-target、exact live verify、max-depth prefix/unused burn、duplicate-entry publication and every resolution/physical test remain RED for Step 8

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

pub struct BackTargetResume { /* root task + existing HistoryExecutionBindingView + continuation */ }
pub enum BackTargetCoreResumeOutcome {
    Resumed(BackTargetResume),
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
        -> (RootFrameTask, HistoryExecutionBindingView, LifecycleContinuationCapacity);
}

impl RouterEngine {
    pub fn verify_back_resolution_handoff(&mut self, accepted: BackAcceptedReservationView, handoff: BackResolutionOwnerHandoff) -> BackResolutionVerifyOutcome;
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

Target resume and contradiction terminal each accept a caller-provided locally infallible `FnOnce` that borrows helper-local bound owner + issuer expectation slots. Core first exact-matches receiver and verified owner; Mismatch returns that verified owner without calling the closure. Matched calls the closure exactly once, then and only then consumes the verified owner into resume/freeze or contradiction terminal. No Core prepared owner、closure or expectation is returned or stored. Target source drift after committed retirement remains the bounded pre-splice `BackTargetPermanentOwner`; `settle_back_target_permanent_exact` independently exact-matches its receiver

Every callback-bearing `*_with_*` method is a `#[doc(hidden)]` trusted ECS integration API and is absent from facade reexports. This follows the existing `ValidatedCommit::publish(FnOnce())` boundary: the callback is not authority; the verified/prepared Core owner is authority. Correctness relies on one audited ECS callsite that captures only helper-local owner/expectation slots and performs no query、allocation、validation or hook/observer-triggering write. Rust visibility/types are not claimed to defeat a malicious direct Core caller; the supported facade/adapter surface cannot obtain the verified owners or name/call these methods

No-target transfer/refreeze uses the same pattern: Core exact-matches receiver + verified candidate before calling the transfer closure; Mismatch returns the verified candidate and leaves helper-local bound reservation untouched. Matched calls transfer once, then exact-refreezes source and returns unchanged candidate or opaque drift. Drift settlement likewise validates the raw drift receiver before a single retirement callback and terminal consumption; wrong engine returns raw drift with the helper-local permit still available for recomposition. No prepared drift owner crosses a helper boundary

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
pub(crate) enum BackTargetResumeOutcome { Resumed { resume: BackTargetResume, retirement: BackCapacityRetirementEvidence }, Permanent { owner: BackTargetPermanentOwner, retirement: BackCapacityRetirementEvidence }, CapacityRejected { bound: BoundBackTargetResolution, error: BackCapacityRetireError }, ReceiverMismatch { bound: BoundBackTargetResolution, evidence: BackResolutionEvidence } }
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

`take_back_resolution_handoff` temporarily separates capacity from its accepted view only to call Core verify. On `Matched`, `VerifiedBackResolutionHandoff::into_parts` returns the accepted view plus a sealed verified branch owner; ECS infallibly recomposes the view with the original capacity into the same `BoundBackCapacityReservation`, then pairs that bound reservation with the verified owner in the branch-specific struct. No split can recover a raw handoff. On `Mismatch`, Core returns raw accepted view + raw handoff + evidence and ECS likewise reconstructs the original bound reservation before returning

`resume_bound_back_target` and `fail_bound_back_contradiction` borrow the exact entry engine + entry-owned issuer for their full duration. Each keeps the verified owner and bound reservation in helper-local slots, obtains `preflight_bound_retirement` without mutation, then calls the matching Core `*_with_retirement` method with a closure borrowing those slots. Receiver mismatch calls the closure zero times and the helper rebuilds the complete branch-specific bound owner; exact match calls it once to `commit_preflighted_bound_retirement` before Core owner consumption. No expectation、prepared Core owner or partially consumed branch escapes

`begin_bound_back_no_target` uses entry-local issuer `preflight_no_target_transfer` and Core `refreeze_back_no_target_source_with_transfer` in the same borrow. Receiver/issuer mismatch returns the complete verified candidate + bound reservation; exact receiver calls the transfer closure once, consuming accepted view + capacity into the same permit before returning `Begun` or source-drift `Permanent`

`settle_back_target_permanent` delegates to the exact `RouterEntry` engine's `settle_back_target_permanent_exact`; it has no fallback engine path, returns the original Core owner on mismatch and returns only Core terminal/settlement evidence on success

Move the existing Step 8 preparation shapes ahead of the tests and scaffold these exact signatures

```rust
pub struct PreparationBoundaryScopeView { inner: CorePreparationBoundaryScopeProjection }
#[doc(hidden)]
pub struct PreparationReceiverMismatchView { inner: CorePreparationReceiverMismatchProjection }
pub struct LifecycleContinuationCapacity { inner: LifecycleContinuationBinding }
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
    installed_back_no_target_cleanup: Option<InstalledBackNoTargetCleanup>,
    pending_back_no_target_completion: Option<SettledBackNoTargetCleanup>,
    installed_back_no_target_authority: Option<InstalledBackNoTargetAuthoritySettlement>,
    installed_target_preparation_permanent: Option<InstalledTargetPreparationPermanentSettlement>,
}
pub(crate) enum BackNoTargetCleanupDriverOutcome {
    Terminal(BackNoTargetTerminalEvidence),
    ReceiverMismatch { owner: SettledBackNoTargetCleanup, mismatch: PreparationReceiverMismatchView },
}
pub(crate) struct PreparedTargetPublication {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
    continuation: PreparedPostCommitContinuationOwner,
    focus_leaving: Vec<Entity>,
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
    PreparationDecision<PreparedTargetPublication, TargetPreparationOwner, PreparationPermanentOwner<TargetPreparationOwner>>;
pub(crate) enum ReconciliationPreparationOutcome {
    Matched(ReconciliationPreparationDecision),
    Mismatch { owner: TargetPreparationOwner, evidence: LifecyclePreflightEvidence },
}

pub(crate) fn classify_back_no_target(
    entry: &mut RouterEntry,
    owner: BackPreparationOwner,
) -> BackNoTargetPreparationOutcome;
pub(crate) fn classify_back_target(
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
```

Also scaffold the existing exact `PreparationBoundaryScopeView`、two-variant `PreparationRetryEvidence`、`ProvenEmptyProjection`/target-protected source types、`LifecycleContinuationCapacity` and conversion signatures

```rust
impl RouterEngine {
    pub fn accept_back_target_ready(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> PreparedPostCommitContinuationOwner;
    pub fn accept_back_target_pending_rejected(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> RouteFailureContinuationOwner;
    pub fn accept_back_target_authority_lost(
        &mut self,
        capacity: LifecycleContinuationCapacity,
        scope: PreparationBoundaryScopeView,
    ) -> TargetAuthoritySettlementOwner;
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

`PreparationReceiverMismatchView` is Core-owned、opaque and non-authorizing. Only a Core exact receiver creates it; ECS can retain it for diagnostics but cannot construct it、inspect raw identity or rebuild an owner. Core success receivers exact-match only their verified owner/capacity plus any required Core-owned `PreparationBoundaryScopeView`; `LifecyclePreflightEvidence` remains ECS crate-private and stays paired with the original ECS physical/preparation owner

`approve_back_no_target_logical_zero` is the only producer of `ProvenEmptyApprovalView` and `ApprovedBackNoTargetCandidate`. It exact-matches the engine + verified candidate, reconstructs the current Core source and validates logical no-predecessor/zero before returning the opaque approval beside the same candidate. Mismatch returns the original verified candidate + `PreparationReceiverMismatchView`. ECS cannot construct or clone the approval; the approval is validation-only, cannot reconstruct an owner, and never substitutes for ECS `ProvenEmptyProjection`. `classify_back_no_target` calls this producer only after physical proven-empty succeeds, and `PreparedBackNoTarget` must carry the resulting approved composite

`BackNoTargetReadyCoreOutcome` is `Prepared(PreparedBackNoTargetPublication) | Mismatch { approved, mismatch: PreparationReceiverMismatchView }`. The sealed move-only prepared publication retains the Core publication capability plus the whole opaque cleanup installation, and its construction performs no public write. `BackNoTargetAuthorityCoreOutcome` returns the exact candidate + opaque mismatch view on receiver mismatch or the existing authority settlement owner on match

`VerifiedBackNoTargetCandidate` owns unresolved lifecycle lineage only; it does not contain a ready/failure continuation or cleanup action. Retry returns that candidate、permit and evidence with no public request-phase/trace write. `PreparedBackNoTarget` contains the Core-issued approved candidate composite + ECS physical projection + permit, but no ledger、continuation or action

`consume_prepared_back_no_target` borrows the exact entry engine + entry-owned issuer once, keeps approved candidate/permit/projection in helper-local slots and runs issuer `preflight_back_canonical_zero` without mutation. It then calls Core `accept_back_no_target_ready_with_canonical_zero_bind`; Core validates receiver + approved composite first, Mismatch returns that composite and invokes the closure zero times, so ECS reconstructs the complete original owner. Matched invokes the closure exactly once to `commit_preflighted_back_canonical_zero`, which consumes the exact outstanding permit into one preflighted `ManifestNeutralLedgerSlot`, and returns `PreparedBackNoTargetPublication` without publishing. The helper captures that slot, then calls `publish_prepared_back_no_target` with the sole ECS installation closure. The closure receives one opaque `BackNoTargetCleanupInstallation`, reads only its non-authorizing batch projection, and calls `bind_canonical_zero_ledger_to_batch(slot, batch)` to consume the slot and return the fully bound `CleanupLedger` without allocation or failure. It then stores the whole un-split installation beside the ledger as `RouterEntry::installed_back_no_target_cleanup = Some(InstalledBackNoTargetCleanup { .. })`. Binding and field installation perform no query/validation and trigger no hook/observer. Only after the closure returns does Core first publish `HistoryIntentRejected → CleanupPending`

`drive_back_no_target_zero_completion` is the same-schedule producer/consumer after that publication. It takes the exact entry field as a whole, proves its ledger has exact zero settled obligations, derives the existing canonical completion digest/count, and calls `BackNoTargetCleanupInstallation::complete` once to form `BackNoTargetCleanupCompletionOwner` without exposing action/continuation fields. `RouterEngine::complete_back_no_target_cleanup` exact-consumes that Core owner into `Failed(BackTargetUnavailable)`. Receiver mismatch returns the complete Core completion owner + `PreparationReceiverMismatchView`; ECS recomposes it with the settled ledger as `SettledBackNoTargetCleanup` and can restore it whole without losing batch/action/continuation. Exact terminal leaves both RouterEntry fields and every Core publication/completion owner empty

`classify_back_no_target` returns the specialized `BackNoTargetPreparationOutcome`, not generic `PreparationDecision`: `Ready` carries only `PreparedBackNoTarget`; `Retry` returns the complete original owner + typed evidence; `AuthoritySettlement` carries `BackNoTargetAuthoritySettlementDisposition`; `Permanent` carries the branch permanent owner. It has no ordinary `PendingRejected` variant: that unreachable classifier result converts directly to `Permanent`. Issuer/receiver mismatch before consumption returns the complete original owner through `CapacityRejected`/`ReceiverMismatch`

The pre-proof `AuthorityLost` arm follows the same boundary inside `classify_back_no_target`: ECS freezes `BackNoTargetAuthorityPhysicalSettlementOwner` from the complete source/protected closure、allowed-write set and exact authority manifest, then passes only the Core-owned boundary scope to `accept_back_no_target_authority_lost_with_retirement`. The entry-owned issuer performs `preflight_no_target_retirement`, then Core exact-matches candidate + scope before the locally infallible retirement callback. Mismatch calls it zero times and returns the complete original `BackPreparationOwner` plus `PreparationReceiverMismatchView`; exact match calls it once and yields `BackNoTargetAuthoritySettlementDisposition { physical, settlement, retirement }`

`dispatch_back_no_target_authority_settlement` is the sole disposition consumer. It validates the exact entry and calls the Core receiver before mutation; entry/engine mismatch returns the entire disposition. Exact dispatch moves the physical owner + Core settlement owner into the existing failed/scoped authority settlement field, installs only its authority ledger/residual under the frozen allowed-write set, and publishes only authority-specific `Running(Recovering) + requested=None`. It never populates `installed_back_no_target_cleanup`, never records `HistoryIntentRejected`/normal `CleanupPending`, and introduces no publisher. Existing authority cleanup completion consumes the stored physical owner first and then the same stored Core settlement owner to reach the original HostLost/AuthorityLost terminal; retirement evidence remains diagnostic and terminal inventory clears the authority field

`PreparationPermanentOwner<TargetPreparationOwner>` is the post-splice ECS physical-preparation owner: it retains the original splice、history binding and continuation together with exact physical evidence. `classify_back_target` exact-matches the `RouterEntry` engine before any Core conversion; cross-router/entry mismatch returns that original `TargetPreparationOwner`. `settle_back_target_preparation_permanent` only prepares `TargetPreparationPermanentDisposition { cleanup, settlement }`; `cleanup` freezes the original authority/permanent branch beside the splice, so later continuation never infers branch from obligation count or disposition

`dispatch_target_preparation_permanent` is the sole disposition consumer. It validates exact entry and the Core settlement receiver before mutation; either mismatch returns the entire disposition. Exact dispatch stores `InstalledTargetPreparationPermanentSettlement { cleanup, settlement }` as one entry-local continuation while preserving the two linear owners. The existing physical cleanup driver consumes only `cleanup`; after its exact receipts complete, it selects the already-frozen original branch and consumes the stored `settlement` into the existing scoped authority settlement. Existing authority completion reaches terminal and clears the installed field; the disposition never reaches the normal publisher

`BackTargetPermanentOwner` is not accepted by target physical classification and remains exclusive to pre-splice `resume_back_target/freeze_back` source drift. Conversely `settle_back_target_preparation_permanent` never accepts that Core owner, so no consumer can receive both permanent authorities or mint a second publication path

`begin_bound_back_no_target` completes verified receiver check、issuer transfer preflight、infallible transfer callback and exact source refreeze in one entry borrow. `Unchanged` creates `BackPreparationOwner`; `Drift` pairs opaque drift with the callback-produced original permit. `classify_back_no_target` handles only later physical proof/classification and never repeats source refreeze

`settle_back_no_target_source_drift_permanent` is one ECS helper, not prepare/commit APIs. It keeps `{ raw drift, permit }` in helper-local slots, performs issuer retirement preflight, then calls Core `settle_back_no_target_source_drift_with_retirement`. Issuer or receiver mismatch happens before either half is consumed and returns the full composite; exact receiver calls the infallible retirement closure once and Core terminals immediately. ECS cannot move a prepared owner to another entry/issuer because none exists

Missing symbols、placeholder panics、test setup failures and injected outcomes do not count as RED

| RED/gate | Required production assertion |
| --- | --- |
| `accepted_back_view_exact_lineage_rejects_cross_scope` | Core consuming verify accepts exact accepted+raw handoff lineage and returns move-only `VerifiedBackResolutionHandoff`; its split returns accepted view + one verified branch owner and never raw handoff. Mismatch returns the original accepted view、raw handoff and evidence, then ECS reassembles the untouched capacity owner |
| `verified_back_resolution_recomposes_bound_capacity_for_all_branches` | Matched recomposes returned accepted view + original capacity into the same `BoundBackCapacityReservation`; branch helper keeps it local through issuer preflight and passes only a borrow-capturing infallible commit closure to the exact Core consumer |
| `verified_back_handoff_cannot_reenter_raw_consumers` | compile/static gate proves resume、no-target refreeze and contradiction terminal accept only their branch-specific verified owner; matched owners cannot convert back to `BackResolutionOwnerHandoff` or cross branches |
| `back_trusted_commit_callbacks_are_not_facade_reachable` | compile/static facade fixture proves every callback-bearing `*_with_*` method and `publish_prepared_back_no_target` is doc-hidden、absent from facade exports, and unusable by facade/adapter code because no verified/prepared Core owner is obtainable. Static callsite audit finds exactly one ECS helper callsite per method and no query/allocation/validation/hook write in its callback; it does not claim to constrain a malicious direct Core caller |
| `back_resolution_handoff_foreign_and_duplicate_are_zero_write` | only `EngineAction::BackResolution` selects a raw branch; ECS calls Core verify before capacity recomposition, foreign/duplicate replays return the complete raw handoff + accepted/capacity lineage and all rejection paths are World/history/public zero-write |
| `back_no_target_retry_preserves_owner_and_public_snapshot` | verified candidate remains the sole unresolved lifecycle lineage owner paired with the same permit; Retry returns candidate、permit and typed evidence intact, with byte-for-byte unchanged request/public snapshot and no phase、trace、continuation、cleanup action or terminal |
| `back_no_target_logical_zero_approval_requires_exact_core_candidate` | after ECS physical proven-empty, the real Core producer exact-matches receiver + verified candidate and refreezes current logical source/no-predecessor before returning `ApprovedBackNoTargetCandidate`; mismatch returns the original candidate + evidence and emits no approval/public write |
| `back_no_target_approval_cannot_be_forged_or_replace_physical_proof` | compile/static gate proves facade/ECS cannot construct、clone or synthesize `ProvenEmptyApprovalView`; the only Ready owner contains a Core-issued approved composite and a distinct ECS `ProvenEmptyProjection`, and removing either producer prevents Ready |
| `back_target_ready_moves_exact_owner_without_publication` | Ready moves the exact owner into prepared state before publication |
| `back_target_pending_rejected_converts_exact_owner_before_public_state` | PendingRejected converts the exact owner before any public state |
| `back_target_authority_lost_converts_exact_owner_before_public_state` | AuthorityLost converts the exact owner before any public state |
| `back_target_retry_returns_exact_owner_capability_and_evidence` | Retry returns the exact live owner、capability、continuation and evidence with zero write |
| `back_target_and_contradiction_receiver_mismatch_never_commits_retirement` | wrong engine returns the verified target/contradiction owner, commit counter remains zero and ECS reconstructs the complete branch bound owner; exact receiver calls commit once before Core consumption |
| `back_target_resume_source_drift_preserves_owner_and_terminals_once` | before any ECS splice, exact receiver calls retirement commit once then Core resume/freeze may return `BackTargetPermanentOwner`; its exact settlement reaches existing AuthorityLost terminal once |
| `back_target_physical_permanent_preserves_spliced_owner_and_disposition` | after splice, physical permanent retains the exact `TargetPreparationOwner`; classify cross-router mismatch and settlement engine mismatch each return it unchanged, while exact settlement yields the single physical-cleanup + authority-settlement disposition and never a `BackTargetPermanentOwner` |
| `back_contradiction_terminals_once_without_world_or_history_write` | real structural contradiction retires bound capacity, consumes accepted owners once, reaches `Failed(AuthorityLost)` exactly once and leaves zero World/presentation/history write or outstanding owner |
| `back_no_target_proven_empty_protects_sparse_current_topology` | real claim/registry/closure perturbations prevent proof; retained-empty/slot-only current Entities remain protected and zero-write, with authority derived only from exact bindings and each Entity's own role |
| `back_no_target_preparation_branches_preserve_exact_owners` | specialized outcome returns Ready prepared owner、Retry original owner+evidence、AuthoritySettlement whole disposition or Permanent owner and has no ordinary PendingRejected variant. Ready helper preflights issuer + Core receiver/approval; mismatch calls bind zero times and reconstructs the full owner, exact Core calls it once and returns sealed `PreparedBackNoTargetPublication` with zero public write. Authority returns `{ physical, Core settlement, ECS retirement }` as one disposition |
| `back_no_target_unreachable_pending_rejected_becomes_permanent_owner` | unreachable PendingRejected moves every owner into `BackNoTargetPermanentOwner::Physical(PreparationPermanentOwner<BackPreparationOwner>)` without cleanup/trace |
| `back_no_target_refreeze_receiver_mismatch_never_commits_transfer` | wrong engine returns verified candidate, transfer counter stays zero and complete bound owner is reconstructed; exact receiver calls transfer once and returns candidate+permit or drift+permit |
| `back_no_target_source_drift_terminals_once` | single helper keeps raw drift+permit local; issuer/receiver mismatch calls retirement zero times and returns full composite, exact receiver calls it once then terminals with no prepared owner |
| `back_no_target_ready_receiver_or_issuer_mismatch_never_binds` | wrong receiver/approval or issuer preflight calls canonical-zero bind zero times and returns full `PreparedBackNoTarget`; exact case calls bind once and returns sealed `PreparedBackNoTargetPublication` without publishing |
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

Expected: scaffold compiles; behavioral groups fail on named production predicates and negative gates pass only for intended privacy/non-Clone reasons

- [ ] **Step 8: Implement live resolution, typed handoff and disposition**

1. Core consumes queued accepted owner, performs one live preview and emits Target、NoTarget or Contradiction through `EngineAction::BackResolution`
2. target matching/Core logical reconciliation consume the real maximum-pool prefix and burn unused ids; no selector comes from requested、bool or snapshots
3. ECS `take_back_resolution_handoff` first moves accepted view + raw handoff through Core `verify_back_resolution_handoff`; `Matched` wrapper splits only into accepted view + one branch-specific verified owner, ECS recomposes that view with the original capacity into the same `BoundBackCapacityReservation` and stores both in the branch-specific bound owner. Mismatch returns raw accepted、raw handoff、capacity and evidence for the same original recomposition
4. Every later Core entry point consumes only its verified branch type: target resume cannot accept raw target handoff, no-target refreeze cannot accept raw candidate, and contradiction terminal cannot accept raw contradiction or another verified branch
5. Verified NoTarget stays paired with the recomposed bound reservation; only `begin_bound_back_no_target(&mut RouterEntry, bound)` may transfer it. The helper first obtains `preflight_no_target_transfer` from the entry-owned issuer, then calls the Core exact refreeze receiver with a helper-local infallible transfer closure. Receiver mismatch calls the closure zero times and returns the verified candidate so the helper restores the complete bound branch; exact receiver calls it once and returns either unchanged candidate + `NoTargetCapacityPermit` or raw source drift + the same permit. Retry returns the unchanged verified candidate + permit with no public write

Target direction remains existing Core publication flow

```text
ECS exact take + burn neutral capacity
-> synchronous typed Core resume of move-only target owner
-> Core stores current_history_capability and issues existing HistoryExecutionBindingView + continuation + root task
-> ECS splice/preparation
-> existing prepare_commit consumes capability before publication
-> unique prepared history/publication owner
```

Core never constructs ECS `PreparedTargetPublication`

- target resolution stays inside `resume_bound_back_target(&mut RouterEntry, bound)`: the entry-owned issuer first performs `preflight_bound_retirement(Target)`, then Core exact-matches the verified target before invoking the helper-local infallible retirement commit closure. Wrong engine calls the closure zero times and returns the verified target so ECS restores the complete bound branch; exact receiver calls it once, then resumes before ECS splice and returns `Resumed | Permanent(source drift)`. Only this pre-splice drift enters `settle_back_target_permanent`, which delegates to Core `settle_back_target_permanent_exact`; wrong engine preserves the Core owner + evidence and exact success returns existing AuthorityLost terminal/settlement evidence
- after `Resumed`, ECS builds `TargetPreparationOwner` and calls `classify_back_target(&mut RouterEntry, owner)`; cross-router/entry mismatch returns the original owner. Its outer physical `Permanent` is `PreparationPermanentOwner<TargetPreparationOwner>` and only `settle_back_target_preparation_permanent` consumes it, returning it on engine mismatch or preparing the whole frozen splice cleanup + Core authority settlement disposition. `dispatch_target_preparation_permanent` is the unique consumer: it validates exact entry/Core receiver before mutation, returns the whole disposition on mismatch, and exact-installs it as one field. Physical cleanup then consumes the pre-frozen original branch and the same stored settlement; it never infers continuation from obligation count、flows through the pre-splice Core drift consumer or reaches the publisher
- verified no-target candidate owns only unresolved lifecycle lineage. Before physical proven-empty + Core logical-zero approval + canonical-zero exact-bind, candidate and every Retry remain internal and make strict zero public request-phase/trace writes. After ECS constructs `ProvenEmptyProjection`, `approve_back_no_target_logical_zero` exact-matches the engine/candidate and current Core source before producing opaque `ApprovedBackNoTargetCandidate`; mismatch returns the original candidate + opaque Core mismatch view. `PreparedBackNoTarget` holds that approved composite、the same permit and the distinct physical projection; it contains no ledger、continuation or action. `consume_prepared_back_no_target(&mut RouterEntry, owner)` first performs issuer canonical-zero preflight, then calls the Core exact Ready receiver with a helper-local infallible commit closure. Issuer/receiver/approval mismatch calls the closure zero times and returns the full `PreparedBackNoTarget`; exact receiver calls it once to consume the permit into a preflighted manifest-neutral slot and returns sealed `PreparedBackNoTargetPublication` with zero public write. Still in the same helper/entry borrow, `publish_prepared_back_no_target` invokes the sole ECS closure with one opaque `BackNoTargetCleanupInstallation`; the closure reads only its batch projection, consumes the captured slot into the fully bound safe canonical-zero ledger and stores `{ ledger, whole installation }` before Core publishes. Same-schedule zero completion takes the field whole, proves exact-zero digest/count and converts the whole installation to the Core completion owner; receiver mismatch restores the whole settled owner and exact success reaches `Failed(BackTargetUnavailable)` with all fields/owners empty
- pre-proof authority loss and permanent fail-closed may still enter existing settlement/terminal, but never publish the normal no-target `HistoryIntentRejected` or `CleanupPending` trace. The specialized `BackNoTargetPreparationOutcome` has no ordinary PendingRejected arm: unreachable PendingRejected converts directly to `Permanent`; AuthorityLost success returns `BackNoTargetAuthoritySettlementDisposition { physical, settlement, retirement }`; Retry and both issuer/receiver mismatches return the complete original owner. The unique authority dispatch validates exact entry/Core receiver before mutation, restores the whole disposition on mismatch, and exact-reuses only existing scoped authority settlement with authority-specific `Recovering`. Permanent capacity/domain contradiction retains `PreparationPermanentOwner<BackPreparationOwner>` for `settle_back_no_target_physical_permanent`
- source drift uses opaque raw `BackNoTargetSourceDriftOwner` paired with the original permit. `settle_back_no_target_source_drift_permanent(&mut RouterEntry, owner)` is the only ECS API: it keeps the full composite local, completes issuer non-mutating retirement preflight, then calls the Core exact drift consumer with an infallible retirement closure. Issuer mismatch returns the full composite; wrong engine calls the closure zero times and returns raw drift for recomposition with the permit; exact receiver calls it once and terminals immediately. There is no prepare/commit API or prepared owner crossing the entry/issuer boundary
- pre-proof no-target AuthorityLost stays inside `classify_back_no_target(&mut RouterEntry, owner)`: after classification freezes the complete physical source/protected closure、allowed-write set、authority manifest and boundary evidence, issuer retirement preflight and the Core exact authority consumer use the same callback rule, returning the full original owner with closure count zero on mismatch or calling it once before producing the whole physical + Core-settlement + ECS-retirement disposition
- contradiction stays inside `fail_bound_back_contradiction(&mut RouterEntry, bound)`: the entry-owned issuer first performs `preflight_bound_retirement(Contradiction)`, then the Core exact consumer either returns the verified contradiction with closure count zero so ECS restores the bound branch, or calls the infallible retirement closure once before reaching existing `Failed(AuthorityLost)` with World/presentation/history zero-write and no outstanding owner or retry

Across these helpers, every irreversible Core/ECS consumption follows one rule: all issuer、receiver、source/approval and exact-owner checks complete before the callback; the callback tail is locally infallible owner movement/registry commit only, with no query、allocation、validation、hook/observer-triggering write or later fallible work. Callback-bearing `*_with_*` methods are doc-hidden trusted ECS integration APIs, not facade exports; their closure is not authority and only verified/prepared Core owners authorize consumption/publication. Preflight expectations never escape helper-local slots. The sole intentional returned prepared authority is `PreparedBackNoTargetPublication`, whose final consumer publishes only after its one ECS installation closure returns. This is the same trusted-callsite model as existing `ValidatedCommit::publish(FnOnce())`, not a claim that Rust prevents a malicious direct Core caller

Before Step 9, drive all publication/closure gates GREEN: the first normal no-target public observation must already have the exact whole ledger+installation field; that branch must then reach `BackTargetUnavailable` after same-schedule whole-owner exact-zero completion with no outstanding entry field/action/ticket/capability/candidate/continuation/reservation/completion/lifecycle owner. No-target AuthorityLost must uniquely dispatch its whole physical/Core/retirement disposition into authority-only settlement and terminal once without a normal trace. Post-splice target physical permanent must uniquely dispatch and complete its frozen physical cleanup and stored Core authority settlement to terminal with no outstanding splice/cleanup/settlement or lifecycle owner

Rerun every Step 4 and Step 7 focused command, including these exact GREEN filters and the two static contracts

```powershell
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
- no-target uses no fake frame/task identity
- retry owner/evidence is returned intact
- matched resolution cannot recover raw handoff and every Core branch consumer requires the exact verified owner
- callback-bearing `*_with_*` methods and the prepared no-target final consumer are doc-hidden trusted ECS integration APIs, not facade reexports; the supported facade/adapter cannot obtain their verified/prepared owners, and the callback is explicitly not authority
- target/contradiction retirement、no-target transfer/refreeze、source-drift retirement and canonical-zero Ready bind each complete issuer + Core receiver preflight in one exact-entry helper, call the infallible commit closure zero times on mismatch and once on exact match, and never leak an issuer expectation or closure
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
git add -- crates/bevy_ui_router_core/src/history_authority.rs crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/src/matcher.rs crates/bevy_ui_router_core/src/request.rs crates/bevy_ui_router_core/src/trace.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/src/test_support.rs crates/bevy_ui_router_ecs/Cargo.toml crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/branch_transaction.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/snapshots.rs crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/support/facade_contract_cases.rs docs/facade-associated-api-golden.txt docs/superpowers/specs/2026-07-20-history-design.md
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
