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
git add -- docs/README.md docs/superpowers/README.md docs/target-architecture.md docs/authority-transition-matrix.md docs/facade-contract-fixture.md docs/mvp-acceptance-journeys.md
git diff --cached --name-only
git commit -m "docs: amend canonical history contracts"
```

Expected staged set: exactly the six listed documentation files

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

- [ ] **Step 1: Extract the shared lifecycle evidence boundary before Back changes**

Move the existing `RequiredResourceSet`、`LifecyclePreflightSource`、`LifecyclePreflightExpectation`、`LifecyclePreflightObserved` and `LifecyclePreflightEvidence` definitions from `navigation.rs` into `lifecycle_preparation.rs` without changing variants or predicates

- make the moved types and every constructor/accessor used by sibling modules `pub(crate)`
- keep all fields private and preserve `const` where it exists
- update `navigation.rs` imports explicitly; `cleanup.rs` imports the shared types only when Step 8 begins using them
- do not re-export the types from the crate or facade

Add the shared four-way owner-preserving outcome and exact retry evidence without wiring Back behavior

```rust
pub(crate) enum LifecyclePreparationOutcome<Ready, Owner> {
    Ready(Ready),
    PendingRejected {
        owner: Owner,
        evidence: LifecyclePreflightEvidence,
    },
    AuthorityLost {
        owner: Owner,
        evidence: LifecyclePreflightEvidence,
    },
    RetryablePreparationFailure {
        owner: Owner,
        evidence: PreparationRetryEvidence,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationCapacityDomain(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationReclaimOwner {
    domain: PreparationCapacityDomain,
    identity: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationReleaseGeneration(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparationHostEpoch {
    router: RouterIdentity,
    host: RouteHostId,
    epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutstandingBoundaryProducer {
    Action(u64),
    Ticket(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparationRetryEvidence {
    RetryableCapacity {
        domain: PreparationCapacityDomain,
        reclaim_owner: PreparationReclaimOwner,
        release_generation: PreparationReleaseGeneration,
    },
    UnfinalizedBoundaryFacts {
        request: NavigationRequestId,
        transaction: TransactionId,
        host_epoch: PreparationHostEpoch,
        outstanding: OutstandingBoundaryProducer,
    },
}
```

Expose the module only as crate-private from `lib.rs`

`Owner` must contain the unique opaque Core-issued neutral continuation capacity for every branch once Back wiring begins

The capacity binds source scope before physical classification but does not preselect success, failure or authority semantics

The shared outcome never creates a second ECS continuation enum and every retry returns the same owner with its capacity intact

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

- [ ] **Step 4: Write linear reservation and admission RED tests**

Add tests

| Test | Required setup and assertion |
| --- | --- |
| `accepted_back_reserves_ecs_capacity_before_core_request_identity` | instrument real capacity/request counters, enqueue eligible back, assert capacity owner sequence precedes the full Core branch reservation and both bind the returned request |
| `no_back_entry_precedes_identity_exhaustion` | publish `can_back=false`, exhaust request counter, call back, assert `NoBackEntry`, no request and allocator remains exhausted/unconsumed |
| `admission_stamp_cannot_authorize_or_reconstruct_target` | admit from positive stamp, advance live history before running, assert target comes from new live cursor and differs from stamp preview |
| `back_reserves_installed_maximum_branch_depth` | install branches of depth 1 and 3, enqueue back, assert one reservation contains depth 3 and later matching consumes only its required prefix |
| `back_between_duplicate_locations_uses_entry_identity` | publish equal locations as distinct entries, back once, assert cursor/revision change while location equality cannot select or replay the target |

Expected assertions

- unknown router wins first
- unavailable router wins before history eligibility
- `NoBackEntry` wins before Core identity exhaustion
- a positive stamp can admit but only live Core history chooses target
- rejected or exhausted reservation identity is never reused

Obtain a semantic RED before Step 5

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib admission_stamp_cannot_authorize_or_reconstruct_target -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_reserves_installed_maximum_branch_depth -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib back_between_duplicate_locations_uses_entry_identity -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib accepted_back_reserves_ecs_capacity_before_core_request_identity -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib no_back_entry_precedes_identity_exhaustion -j 1
```

Missing symbols do not count as the semantic RED

If the exact canonical signatures are absent, introduce only their opaque type/signature shape and a fail-closed zero-write path, confirm the focused test compiles with `cargo test ... --no-run`, then rerun it

Expected: each focused test executable exits nonzero on its named production assertion, not on compilation, test setup or an injected classification

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

- [ ] **Step 6: Implement manifest-neutral capacity, full enqueue and late preview**

Define the ECS capacity owner before wiring the facade flow

```rust
pub(crate) struct EcsCleanupCapacityReservation {
    domain: PreparationCapacityDomain,
    owner: BackCapacityOwnerId,
    release_generation: PreparationReleaseGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackCapacityOwnerId(u64);

pub(crate) struct BoundBackCapacityReservation {
    capacity: EcsCleanupCapacityReservation,
    execution: BackExecutionReservationView,
}

pub struct BackExecutionReservationView {
    inner: HistoryExecutionBindingProjection,
}

pub enum EnqueueBackErrorKind {
    AuthorityRecovering,
    Exhausted,
}

pub struct AcceptedBackReservation {
    request: NavigationRequestId,
    execution: BackExecutionReservationView,
}

impl AcceptedBackReservation {
    pub fn request(&self) -> NavigationRequestId;
    pub fn into_parts(self) -> (NavigationRequestId, BackExecutionReservationView);
}
```

- reservation contains no Entity, manifest, count, logical root, frame, task, slot or generation
- rejection consumes the owner, burns its identity and releases capacity without creating a ledger entry
- acceptance infallibly combines the ECS owner with the view returned by `AcceptedBackReservation::into_parts` into `BoundBackCapacityReservation`
- `BackExecutionReservationView` has private fields, no public constructor and no method that can mint or reconstruct a Core reservation owner
- ECS consumes `AcceptedBackReservation::into_parts`, moves the view into capacity binding, then returns the copied request id to facade only after that move completes
- `request()` is read-only convenience and never authorizes binding or publication
- resolved target burns/releases this owner before normal physical preparation
- no-target consumes the binding view, burns the unused Core branch identities, then moves only ECS capacity beside the target-free candidate into Step 8 `BackPreparationOwner`
- `AcceptedBackReservation` is a non-authorizing view of the exact request/normal-attempt/transaction/cleanup-batch reservation and cannot reconstruct Core owners

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

If step 5 fails, burn the ECS reservation identity and release its capacity with zero request/public state

- allocator exhaustion maps to `IdentityExhausted`
- prepare-to-enqueue token drift maps to `AuthorityRecovering`

At running time match the resolved target and consume only the required prefix of the maximum-depth `ReservedBranchAttempt`

Unused reserved frame/task identities stay burned

Extend canonical history

```rust
pub(crate) struct BackDestination {
    source: FrozenHistorySource,
    target_cursor: usize,
    target_entry: HistoryEntryId,
    target: RouteLocation,
}

pub(crate) struct FrozenBackDestination {
    source_entry: HistoryEntryId,
    source_cursor: usize,
    target_entry: HistoryEntryId,
    target_cursor: usize,
    target: RouteLocation,
}

pub(crate) enum FrozenHistoryDestination {
    Push {
        target: RouteLocation,
    },
    Replace {
        target: RouteLocation,
        replaced: Option<HistoryEntryId>,
    },
    Back(FrozenBackDestination),
}

pub(crate) enum BackPreview {
    Target(BackDestination),
    NoTarget(FrozenHistorySource),
}

impl CanonicalHistory {
    pub(crate) fn preview_back(&self) -> BackPreview;
    pub(crate) fn freeze_back(
        &self,
        execution: HistoryExecutionBinding,
        destination: BackDestination,
    ) -> Result<HistoryCommitCapability, HistorySourceDrift>;
}
```

The eventual back effect updates cursor only, preserves entries and entry identities, and advances revision

Rerun all five Step 4 focused commands

Expected: every command exits `0` before no-target work begins

- [ ] **Step 7: Write manifest-neutral capacity and pure no-target RED tests**

Add tests in cleanup/navigation

| Test | Required setup and assertion |
| --- | --- |
| `back_capacity_reservation_contains_no_entity_manifest` | reserve before target resolution and prove the type exposes no Entity/count/roots or frame identity |
| `resolved_back_target_burns_unused_capacity_identity` | resolve a real target, assert the manifest-neutral identity is retired and absent from the normal cleanup ledger/receipt domain |
| `proven_empty_requires_exact_host_active_registry_and_full_closure` | perturb host claim, active claim, registry and descendant closure one at a time, assert no case can mint `ProvenEmptyProjection` |
| `retained_empty_and_slot_only_are_protected_by_entity_local_role` | construct both sparse topologies, resolve no-target, assert each Entity is protected from relationship/component writes |
| `back_no_target_settles_canonical_zero_without_frame_task_identity` | admit then remove live predecessor through an earlier queued publication, assert zero manifest/digest/count and exact terminal trace |
| `retryable_back_preparation_returns_exact_owner_and_evidence` | hold a real reclaim owner or outstanding ticket, assert retry returns the same reservation and exact producer evidence |
| `permanent_back_preparation_contradiction_fails_closed_once` | exhaust domain or drift source with no live producer, assert one bounded failure and no repeated preparation |

Run

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_capacity_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib resolved_back_target_burns_unused_capacity_identity -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib proven_empty_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib retained_empty_and_slot_only_are_protected_by_entity_local_role -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib back_no_target_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib retryable_back_preparation_returns_exact_owner_and_evidence -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib permanent_back_preparation_contradiction_fails_closed_once -j 1
```

Expected: compilation succeeds; the two capacity shape/burn tests may already pass from Step 6, while proven-empty, no-target settlement, retry and permanent-contradiction tests exit nonzero on their named production predicates

- [ ] **Step 8: Implement unified target and no-target preparation ownership**

Use the shared outcome for both resolved-target and no-target preparation

```rust
pub(crate) struct ProvenEmptyProjection {
    source: FrozenEmptySourceProjection,
    protected: ProtectedClosurePartition,
    manifest: FrozenCleanupManifest,
}

pub(crate) struct BackPreparationOwner {
    capacity: EcsCleanupCapacityReservation,
    candidate: BackNoTargetCandidate,
}

pub(crate) struct PreparedBackNoTarget {
    owner: BackPreparationOwner,
    projection: ProvenEmptyProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenEmptyEntityFact {
    entity: Entity,
    parent: Option<Entity>,
    owner: Option<EntityOwner>,
    claim: Option<ExactRegistryClaim>,
    role: BackObservedRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackObservedRole {
    Host,
    TopSlot,
    ChildSlot,
    CurrentFrame,
    RetainedFrame,
    AppOwned,
    Foreign,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenEmptySourceProjection {
    host_epoch: PreparationHostEpoch,
    observed_closure: Vec<FrozenEmptyEntityFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtectedClosurePartition {
    entities: BTreeSet<Entity>,
    edges: BTreeSet<(Entity, Entity)>,
}

pub(crate) type BackPreparationOutcome = LifecyclePreparationOutcome<
    PreparedBackNoTarget,
    BackPreparationOwner,
>;

pub(crate) struct TargetPreparationOwner {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
    continuation: LifecycleContinuationCapacity,
}

pub(crate) struct PreparedTargetPublication {
    splice: SealedReconciliationSplice,
    history: HistoryExecutionBindingView,
    continuation: PreparedPostCommitContinuationOwner,
    focus_leaving: Vec<Entity>,
}

pub(crate) type ReconciliationPreparationOutcome = LifecyclePreparationOutcome<
    PreparedTargetPublication,
    TargetPreparationOwner,
>;
```

Implementation rules

- replace `ReconciliationPreflightOutcome` with `ReconciliationPreparationOutcome`
- construct one `TargetPreparationOwner` before preflight and move that same owner through exactly one of the four variants
- every caller matches all four variants, no empty value or generic error may replace owner/evidence
- reserve `EcsCleanupCapacityReservation` before calling Core back enqueue
- if Core rejects, consume/cancel the capacity owner without reusing its identity
- on accepted request, bind reservation to exact request in `RouterEntry`
- `SealedReconciliationSplice` carries the opaque `HistoryExecutionBindingView` issued by Core, ECS cannot construct it
- target `Ready` moves `TargetPreparationOwner` to prepared commit while Core keeps the exact live capability in `current_history_capability`
- target `Ready` consumes the neutral capacity exactly once into an opaque `PreparedPostCommitContinuationOwner`, then moves it with the physical splice and history view into `PreparedTargetPublication`
- target `PendingRejected` moves the full physical owner to existing pending cleanup, then Core consumes its neutral capacity with exact current request stage and typed evidence into one opaque route-failure continuation owner held by that pending cleanup
- target `AuthorityLost` moves the full owner to existing authority settlement, then Core consumes its neutral capacity with exact current authority and typed evidence into the settlement owner
- target `RetryablePreparationFailure` returns the same `TargetPreparationOwner`; Core capability and the neutral continuation capacity remain untouched, World/history stay zero-write
- resolved target burns the unused manifest-neutral capacity identity and releases its capacity before normal physical preparation, it never becomes the normal cleanup owner
- no-target builds `ProvenEmptyProjection` from exact host, active claims, owner inventory, registry, full observed closure and role partition
- canonical empty manifest has zero Entity obligations and no synthetic frame/generation
- current, retained, top/child slot and app host stay protected
- no-target `PendingRejected` moves the physical parts of `BackPreparationOwner` to exact pending cleanup while Core converts its capacity into one opaque `BackNoTargetPendingContinuationOwner` held by that cleanup
- no-target `AuthorityLost` moves the physical parts to existing authority settlement and converts the same capacity into the settlement owner without `HistoryIntentRejected`
- no-target `RetryablePreparationFailure` returns the same candidate-owned capacity, reservation and exact evidence
- emit `HistoryIntentRejected` only when the no-target candidate is accepted into its foreground cleanup path, never before an authority-loss classification
- no-target trace order is `HistoryIntentRejected → CleanupPending → CleanupSettled → RequestTerminal`
- no-target terminal is `Failed(BackTargetUnavailable)` and never starts fallback
- retry returns original owner, unique neutral capacity and evidence, then only wakes after the named producer changes
- source drift, missing owner/ticket and permanent domain exhaustion fail closed once

Add a target-free Core owner rather than widening route work with optional frame identity

```rust
struct PendingBackNoTarget {
    request: NavigationRequestId,
    source: FrozenHistorySource,
    cleanup_batch: CleanupBatchId,
    continuation: LifecycleContinuationCapacity,
}

struct LifecycleContinuationBinding {
    router: RouterIdentity,
    request: NavigationRequestId,
    source: LifecycleContinuationSource,
    transaction: TransactionId,
    host_epoch: u64,
}

enum LifecycleContinuationSource {
    Target(NormalAttemptIdentityProjection),
    BackNoTarget { cleanup_batch: CleanupBatchId },
}

#[doc(hidden)]
pub struct LifecycleContinuationCapacity {
    inner: LifecycleContinuationBinding,
}

#[doc(hidden)]
pub struct PreparedPostCommitContinuationOwner {
    inner: PreparedPostCommitContinuation,
}

#[doc(hidden)]
pub struct RouteFailureContinuationOwner {
    inner: RouteFailureContinuation,
}

#[doc(hidden)]
pub struct BackNoTargetPendingContinuationOwner {
    inner: BackNoTargetPendingContinuation,
}

pub struct BackNoTargetCleanupAction {
    request: NavigationRequestId,
    batch: CleanupBatch,
}

struct PendingBackNoTargetCompletion {
    request: NavigationRequestId,
    batch: CleanupBatch,
    continuation: BackTargetUnavailableContinuation,
}

pub struct BackNoTargetCandidate {
    request: NavigationRequestId,
    batch: CleanupBatch,
    continuation: LifecycleContinuationCapacity,
}
```

Required Core/ECS handoff

- Core creates `PendingBackNoTarget` from the original reserved request and its cleanup batch
- integration first exposes one opaque `BackNoTargetCandidate` that owns the exact neutral continuation capacity without emitting rejection trace
- ECS combines the candidate with its exact capacity reservation as `BackPreparationOwner`
- only `Ready(PreparedBackNoTarget)` lets Core consume the candidate, convert the neutral capacity to `BackTargetUnavailableContinuation`, store `PendingBackNoTargetCompletion`, and issue a non-authorizing `BackNoTargetCleanupAction` with zero logical roots
- `AuthorityLost` consumes the candidate into existing settlement through the capacity conversion below without `HistoryIntentRejected`
- target preparation receives one opaque `LifecycleContinuationCapacity` from Core before physical classification and stores it in `TargetPreparationOwner`
- the capacity is non-Clone, has no public constructor/accessor, binds exact request/transaction/epoch plus either a real target attempt or target-free cleanup batch, and is never projected into snapshots
- the no-target source contains no frame, task, slot or generation identity
- it contains no success, failure, authority or no-target branch selector
- ECS cannot construct, clone, inspect or infer any converted Core continuation
- Core conversion methods consume the capacity with the typed outcome, exact current request stage/authority and boundary evidence; Core alone selects and freezes the branch cause
- ECS cannot provide an arbitrary terminal cause
- every ready, pending, authority-loss or retry branch moves the same capacity; no ECS continuation type exists
- target `Ready` converts capacity into `PreparedPostCommitContinuationOwner`, which remains in the move-only prepared/sealed publication through atomic publish and postcommit cleanup
- no-target `Ready` converts capacity and stores it in Core `PendingBackNoTargetCompletion` before returning the cleanup action; the action carries no continuation authority
- ECS binds the exact request reservation to `CleanupOwner::BackNoTarget`
- ECS submits canonical zero count/digest through the existing Core cleanup binding
- Core validates batch, zero logical roots, canonical count/digest and completion binding
- cleanup completion must exact-match and consume Core `PendingBackNoTargetCompletion`; continuation semantics are never reconstructed from the action, manifest count or disposition
- that single consumption publishes `BackTargetUnavailable` and leaves no target-free owner

Rerun every Step 7 focused command

Expected: all seven commands exit `0` before public `back` is exposed

- [ ] **Step 9: Expose public back only after success and no-target paths pass**

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
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_journeys history_c1_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit history_back_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
cargo fmt --all -- --check
git diff --check
```

Expected: all gates exit `0`, both History public methods are no longer excluded, complete C3 evidence remains planned

- [ ] **Step 11: Independent Back execution review and commit**

Reviewers verify

- admission stamp cannot authorize target
- ECS capacity exists before Core request allocation
- running target binds exact entry identity, not location equality
- requested target is single-assignment
- no-target uses no fake frame/task identity
- retry owner/evidence is returned intact
- protected sparse topology remains zero-write

```powershell
git add -- crates/bevy_ui_router_core/src/history_authority.rs crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/src/matcher.rs crates/bevy_ui_router_core/src/request.rs crates/bevy_ui_router_core/src/trace.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs crates/bevy_ui_router_ecs/src/lib.rs crates/bevy_ui_router_ecs/src/branch_transaction.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/snapshots.rs crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs crates/bevy_ui_router/tests/support/facade_contract_cases.rs docs/facade-associated-api-golden.txt docs/superpowers/specs/2026-07-20-history-design.md
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
