# Reconciliation Implementation Plan

> **Execution rule:** implement task by task with `subagent-driven-development` or `executing-plans`
>
> Use TDD for every Rust behavior change

**Status:** executing

**Progress**

| 进度 | 提交 |
| --- | --- |
| Task 1 complete | `228c5bf` |
| Task 2 complete | `c627a9e` |
| Task 3 complete | `6f8e734` |
| Task 4 complete | `9415d03` + `4f4e4dc` |
| Task 5 complete | `ddce26d` |
| Tasks 6–8 pending | — |

**Stage contract:** `docs/reconciliation-contract-profile.md`

**Planning baseline:** `47da839`

**Target:** Phase 3B.1 `Reconciliation PASS`

**Goal:** 在唯一 `RouterEngine` publisher 下，把 Nested 的整支替换收窄为 slot-aware common-prefix splice，闭合 Journey B、nested fragment reconciliation、exact cleanup 与最小 pre-commit authority settlement

**Architecture:** Core 在 queued request 成为 running 时生成 sealed logical frame/slot roles，ECS 独占 exact Entity validation、物理 splice、ledger、receipt 与 audit

preserved、leaving、entering 只经过一次 preflight 与一次不可失败 commit tail

## Global constraints

- `RouterEngine` 继续独占 route classification、frame/slot identity、generation、commit、history 与 terminal publication
- ECS 只消费 Core sealed roles并分类 physical outcome，不得重算 common prefix或发布 active
- 不原样接入旧单 `RouteFrameScope` 的 `FrameReconciliationPlan`
- match 与全 branch reservation 在 enqueue 时冻结，projection 在 request 成为 running 时使用当时 active
- 未使用的 reserved frame/task IDs 与任何已签发 slot authority ID 永久 burn
- ordinary entering 使用新 frame id 与 initial generation
- nested fragment 只 remount deepest leaf，复用 frame id并 `checked_next` generation
- Core active state必须保留 parent-owned empty logical child-slot binding
- ECS slot lifecycle严格执行 `PreservedActive | RetainEmpty | ActivateExisting | MountNew | Leaving`
- Router-created Entity 继续 registry-first、inert write、relationship write、post-write exact validation
- preserved closure zero mutation，leaving/entering mutation全部在 preflight 前解析
- commit tail禁止 query、分配、组件插入、relationship写入或可失败验证
- Core cleanup只验证 logical roots、batch、digest/count与completion digest
- ECS独占 exact Entity manifest、attempt、ledger、receipt与quarantine lease
- normal Journey B 必须得到 `Removed` receipt与独立 `Removed` tombstone，不接受 quarantine 代替
- physical `AuthorityLost` 纳入最小 pre-commit settlement，不实现完整 Recovery
- generation overflow保持特殊 pre-mutation `Failed(AuthorityLost)`，旧 active/history不变
- 不解锁 `replace`、`back`、完整 Recovery、Authority closure、`RouterInputProbes`、MVP或BSN
- `CommitPublished` 后的 host/registration loss 与 authority-loss transfer 仍归后续 Recovery
- authority catalogue保持 Root 20 + Nested 3 = 23，不新增最终 vocabulary 外的名称
- app-owned host 与 foreign Entity继续零越权写入
- 不依赖 query iteration order，不持有 `&mut World` 跨 `await`

## Execution ownership

- Root agent是唯一 index/commit owner
- 子代理不得暂存或提交
- 只用精确文件列表 `git add -- ...`
- Core/ECS authority接线由单一实现负责人串行整合
- 实际文件写入串行，Task 6/7只可并行做只读测试设计或外部 patch 准备
- 全 workspace gate使用单 build job与单 nextest test thread串行执行

## Preflight before Task 1

```powershell
git rev-parse 47da839
git ls-files --error-unmatch docs/reconciliation-contract-profile.md
git ls-files --error-unmatch docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md
git status --short
cargo check --locked --workspace --all-targets --all-features -j 1 --config "build.rustc-wrapper = ''"
```

Expected: contract与plan均已单独提交，worktree clean，workspace check exit 0

---

### Task 1: Core slot-aware reconciliation projection

**Files**

- Modify: `crates/bevy_ui_router_core/src/frame.rs`
- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`

**Produces**

- private per-frame `Preserved | Leaving | Entering` projection
- private sealed logical child-slot roles与empty slot binding
- `#[doc(hidden)]` opaque integration view，ECS只读消费且无 public constructor
- running-time active baseline validation
- ordinary、fragment、equivalent、append、truncate、suffix replace分类

- [x] **Step 1: Write RED tests**

Add Core tests for

- `/items/1 -> /items/2` preserves layout and boundary slot, leaves/enters one leaf
- append reuses retained empty slot or requests `MountNew`
- truncate emits `RetainEmpty` and no slot cleanup at the cut
- nested fragment preserves ancestors, reuses leaf frame id and advances generation once
- provenance/raw search change forces common prefix zero
- equivalent emits no frame/slot effect but retains push intent
- request B queues before A completes, then projects from A's newly published active when B becomes running
- preserved baseline drifts after projection, so no later task or commit ticket is issued
- unused full-branch reservations remain burned

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::reconciliation_projection_ -j 1
```

Expected RED: new assertions fail because runtime still schedules from branch index 0 and does not retain empty slot bindings

- [x] **Step 2: Implement the private projection**

Requirements

- keep old `BranchDiff` as classifier input
- create a private slot-aware projection instead of widening public authority API
- expose roles to ECS only through the existing hidden integration module，facade不导出且ECS不能构造或重算
- compute projection only in `start_next`
- retain logical `parent frame + outlet key + slot id` even when no child frame is active
- fragment delta overrides all-preserved `BranchDiff` result at deepest leaf
- fail active baseline drift before issuing a task or commit ticket

- [x] **Step 3: Verify and review**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::reconciliation_projection_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib frame::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib branch_diff::tests:: -j 1
cargo fmt --all -- --check
git diff --check
```

Review gate: slot identity不伪造成Entity authority，queued active时序与ID burn闭合，旧Root/Nested分类不回退

```powershell
git add -- crates/bevy_ui_router_core/src/frame.rs crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(core): plan slot-aware reconciliation"
```

---

### Task 2: Core suffix scheduling and sealed terminal ordering

**Files**

- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_core/src/integration.rs`
- Modify: `crates/bevy_ui_router_core/src/mount.rs`

**Produces**

- tasks only for entering suffix
- direct target for a preserved registered child slot
- fragment single effect with new fragment input
- leaving-only logical cleanup roots
- exact failure and history ordering

- [x] **Step 1: Write RED tests**

Add tests for

- first entering leaf targets the preserved boundary slot without a new mount token
- append with no slot uses the existing sealed mount protocol
- truncate can reach `ReadyToSwitch` with no entering frame
- fragment issues exactly one effect, same frame id, generation `+1`
- fragment effect apply failure cleans only the new generation, while the old generation active/history remain unchanged until cleanup settles
- entering failure roots contain only this request's applied entering suffix
- `CleanupSettled` precedes `RequestTerminal`
- commit前history不变，publish后push intent只提交一次
- generation overflow has no cleanup/fallback/active/history mutation

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::reconciliation_schedule_ -j 1
```

Expected RED: current engine cannot issue a task directly into a preserved registered slot and still replaces the full branch

- [x] **Step 2: Implement scheduling**

Requirements

- extend only private/sealed mount target shapes
- prefill target branch with preserved frames and bindings
- issue entering tasks root-to-leaf from the cut index
- keep `applied_frames` scoped to this request
- keep cleanup roots leaf-to-root and limited to leaving suffix
- publish complete spliced active branch through the existing prepared commit
- equivalent and truncate still consume a sealed commit ticket

- [x] **Step 3: Verify and review**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::reconciliation_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib integration::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core -j 1
git diff --check
```

Review gate: single publisher不变，preserved frame不进入failure cleanup，fragment不会被zero-effect吞掉

```powershell
git add -- crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_core/src/integration.rs crates/bevy_ui_router_core/src/mount.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(core): schedule reconciled suffixes"
```

---

### Task 3: ECS preserved branch and slot splice model

**Files**

- Modify: `crates/bevy_ui_router_ecs/src/branch_transaction.rs`
- Modify: `crates/bevy_ui_router_ecs/src/mount.rs`
- Modify: `crates/bevy_ui_router_ecs/src/registry.rs`

**Produces**

- split active branch at the sealed cut
- exact preserved/leaving/entering partitions
- exact slot-role lifecycle plans
- infallible branch splice after preflight

- [x] **Step 1: Write data-structure RED tests**

Cover

- occupied boundary slot stays `PreservedActive`
- truncate boundary slot becomes `RetainEmpty` and remains registered
- append reuses `ActivateExisting` or accepts `MountNew`
- leaving-parent slots remain in leaving obligations
- splice rejects duplicate frame、slot、outlet或owner facts
- no partition crosses a child-slot authority boundary

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib branch_transaction::tests::reconciliation_ -j 1
```

Expected RED: `RealizedBranch` and `PendingCommit` still model whole-active replacement only

- [x] **Step 2: Implement sealed splice structures**

Requirements

- no public Entity selector or slot constructor
- preserve exact claim、owner、seal与parent/outlet mapping
- model empty registered slot independently from active child frames
- precompute lifecycle transitions and claim rebinds
- splice consumes validated plans once

- [x] **Step 3: Verify and review**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib branch_transaction::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib mount::tests:: -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib registry::tests:: -j 1
git diff --check
```

Review gate: slot role与frame role分离但同属一个sealed splice，truncate不制造orphan或第二registry authority

```powershell
git add -- crates/bevy_ui_router_ecs/src/branch_transaction.rs crates/bevy_ui_router_ecs/src/mount.rs crates/bevy_ui_router_ecs/src/registry.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ecs): model preserved branch splices"
```

---

### Task 4: ECS apply, preflight and atomic suffix commit

**Files**

- Modify: `crates/bevy_ui_router_ecs/src/branch_transaction.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/presentation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/input.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/audit.rs`
- Modify: `crates/bevy_ui_router_ecs/src/frame_local.rs`
- Modify: `crates/bevy_ui_router_ecs/src/authority_regression_tests.rs`

**Produces**

- apply into a preserved parent slot
- one preflight over global baseline and three exact closures
- one atomic suffix splice and Core publication
- post-commit leaving-suffix ledger only
- consuming rejection that restores old active and pending entering owners
- role-sensitive claim rebind for frames and registered child slots

- [x] **Step 1: Write RED tests**

Cover Journey B commit schedule and each slot role

- `RetainEmpty` exact transition为 registry `Active -> PreparedInert`、slot `Active -> RegisteredInert`、Hidden、`Pickable::IGNORE`、direct gate deny、focus release且mapping保留
- `ActivateExisting` exact transition恢复 lifecycle与Visible，但slot仍为 `Pickable::IGNORE`、direct gate deny，只有child frame gate授权交互
- preserved closure的foreign state零 mutation，entering foreign/protected/cycle为`PendingRejected`，preserved/current unauthorized inbound parent或cycle为`AuthorityLost`
- leaving foreign/protected parent可隔离后quarantine，leaving owner/seal/manifest/cycle corruption为`AuthorityLost`
- 任一 preflight reject保持World、registry、gate、Core publication零写
- sealed commit plan只允许对已存在组件做预解析的in-place value mutation，lifecycle observer计数保持零

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::reconciliation_commit_ -j 1
```

Expected RED: commit currently takes the whole active branch and rebases a whole entering branch

- [x] **Step 2: Implement apply and preflight**

Preflight must verify

- exact host epoch、top-slot claim、registry partition、active/gate baseline与sealed ticket
- preserved/leaving/entering Entity、manifest、owner、seal、lifecycle、parent、slot、outlet与acyclic traversal
- retained empty slots and occupied slots by exact parent/outlet/mount binding rather than vector position
- role-sensitive foreign component/descendant/protected-parent partition
- all presentation、registry、slot、gate与focus mutation plans before the first write
- sealed exact Entity/owner/seal/manifest/parent/lifecycle facts与唯一允许的in-place writes
- any rejection returns the original active、pending entering and all cleanup owners without clone or World reconstruction

Commit order

1. isolate leaving suffix and release focus
2. apply slot role transitions
3. activate entering suffix
4. atomically splice registry/branch/gate state
5. invoke the existing Core prepared publication

- [x] **Step 3: Verify and review**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::reconciliation_commit_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib cleanup::tests::nested_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib presentation::tests:: -j 1
git diff --check
```

Review gate: preserved closure zero mutation，commit tail不可失败，同一phase没有双interactive leaf

The commit tail must not insert、replace or remove components，change relationships，trigger lifecycle observers or perform post-write queries

Observer-capable staging and cleanup writes remain outside this tail and retain exact post-write validation

```powershell
git add -- crates/bevy_ui_router_ecs/src/branch_transaction.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/presentation.rs crates/bevy_ui_router_ecs/src/input.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/audit.rs crates/bevy_ui_router_ecs/src/frame_local.rs crates/bevy_ui_router_ecs/src/authority_regression_tests.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat(ecs): commit reconciled suffixes atomically"
```

---

### Task 5: Minimal pre-commit authority settlement

**Files**

- Modify: `crates/bevy_ui_router_core/src/engine.rs`
- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/host.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/snapshots.rs`
- Modify: `crates/bevy_ui_router_ecs/src/audit.rs`

**Produces**

- role-sensitive `PendingRejected | AuthorityLost | Ready`
- `RecoveringAuthority` settlement with exact terminal order
- active clear and existing full admission rerun without navigation replay

- [x] **Step 1: Write RED tests**

Cover

- preserved/current/host corruption enters `Running(Recovering)` and rejects queued requests
- leaving authority corruption blocks commit
- isolatable leaving foreign/protected drift commits with quarantine receipt
- entering drift only settles pending and preserves old active/history
- entering foreign/protected parent/cycle and missing exact claims remain `PendingRejected` with old active/history unchanged
- receipts precede `CleanupSettled` and final `RequestTerminal`
- current request ends `Failed(AuthorityLost)` and Core/ECS active are both `None`
- exact valid registration re-admits to `Ready + None` without replay
- authority settlement transfers the same immutable outstanding obligations without a second ledger or duplicate receipt

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::reconciliation_authority_loss_ -j 1
```

Expected RED: current active preflight failure only restores pending work and retries

- [x] **Step 2: Implement the narrow settlement**

Requirements

- reuse sealed cleanup batches and existing host admission protocol
- fail queued requests once, preserve request ordering and never replay them
- distinguish capacity overflow from physical authority loss
- map invalid registration/topology/admission failure to existing `Unavailable` reasons
- keep complete Recovery behavior out of scope

- [x] **Step 3: Verify and review**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core --lib engine::tests::reconciliation_authority_loss_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::reconciliation_authority_loss_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib host::tests:: -j 1
git diff --check
```

Review gate: terminal前无未结foreground obligation，re-admission不伪造Recovery或第二publisher

```powershell
git add -- crates/bevy_ui_router_core/src/engine.rs crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/host.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/snapshots.rs crates/bevy_ui_router_ecs/src/audit.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "feat: settle reconciliation authority loss"
```

---

### Task 6: Private physical reconciliation regressions

**Files**

- Modify: `crates/bevy_ui_router_ecs/src/navigation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/presentation.rs`
- Modify: `crates/bevy_ui_router_ecs/src/cleanup.rs`
- Modify: `crates/bevy_ui_router_ecs/src/audit.rs`

Owner mapping: navigation 负责 scheduled journey、slot role 与 classification，presentation 负责 DefaultPlugins evidence，cleanup 负责 attempt、receipt、ledger 与 quarantine matrix，audit 负责独立 World/registry removal mapping

- [ ] **Step 1: Inventory and add fixed physical scenarios**

Evidence discipline

- 先把现有测试逐项映射到下列 required regressions，直接复用已有证据且不添加重复测试
- 至少一个新增或扩展的 physical journey 使用 official `App::update` schedule，从 producer 运行到 settlement，再多运行一次 update 证明不重放
- direct preflight、finalizer 或 ledger injection 仅作 supplemental evidence，不能替代 scheduled journey
- private ECS 测试保存并断言 exact Entity、claim、owner、seal、lifecycle、relationship 与 World facts，snapshot 或 fingerprint 只用于校验，不能作为事实替身
- production behavior change 先得到 failing regression，若 coverage-only 测试证明的是已有行为，执行记录必须如实标为 evidence addition 而非 behavioral RED
- navigation、presentation、cleanup、audit 四组新增或扩展证据统一使用 `reconciliation_` 前缀，四条 focused commands 各至少命中一项，出现 0 tests 即失败
- 不新增 authority catalogue name，catalogue 保持 `23/23/23`

Required regressions

- operation×role 按合同映射共同覆盖，suffix replace/fragment occupied slot 对应 `PreservedActive`，truncate 对应 `RetainEmpty`，append reused empty slot 对应 `ActivateExisting`，append without empty slot 对应 `MountNew`，leaving parent 对应 `Leaving`
- 每个映射到的 slot role 保存 exact registry/slot lifecycle、visibility、picking、pointer/keyboard/gamepad direct gate、focus 与 private audit mapping
- equivalent 证明 zero entering、zero leaving、zero effect，以及 Entity、relationship、presentation registry、slot lifecycle 与 cleanup ledger physical zero write，同时允许正常 request/history publication
- fragment 只消费 Core sealed 的同 frame id、generation `+1` identity，恰好物理应用一次 effect，ECS 不得重新分类或分配该 identity
- Journey B 保存 preserved layout shell/content/outlet/slot 的 exact identity 与 relationships，在 schedule boundary 上证明 old leaf isolation 与 focus release 早于 new leaf activation，正常 cleanup 的每个 receipt 均为 `Removed`，old exact entities 与 registry claims 均不存在且无 orphan slot
- private audit 不依赖 cleanup receipt，独立以 exact World absence 加 registry absence 推导 old leaf `Removed`，公开 `AuditEntityId` evidence 留给 Task 7
- target-first matrix 覆盖 missing、owner、seal、required component manifest、foreign descendant/component、protected inbound parent 与 cycle 的 role-sensitive 分类
- authorized leaving owned target 可因 foreign descendant/component 或 protected inbound parent 把 owned residual 转交 quarantine，foreign/protected non-target Entity、component 与 edge 保持不变
- preserved、leaving 与 non-target current 的 missing Entity/claim、owner/seal/required component manifest 或 cycle drift 依合同分类 `AuthorityLost`，preserved foreign additions 保持 `Ready` 且 zero mutation
- entering identity/foreign/protected/cycle failure 分类 `PendingRejected` 并保留 old active
- mutation 限于 allowed write set，authorized leaving/entering targets 与每个 preserved、non-target current、protected non-target closure 均不相交
- leaving child-slot 仅在 owned sets 不相交时独立 quarantine，重叠集合通过 removal DAG 提升或合并
- app-owned host components、pre-existing child identity、relationship 与 relative order 全部不变
- entering staging 的 hook/observer writes 与 post-write validation 在 read-only switch preflight 前完成，以 schedule-boundary World evidence 和 counters 证明 preflight zero mutation，`Ready` atomic tail 无 insert/remove/despawn/relationship write/hook/observer，也无 refreeze、query、fallible validation 或 reclassification
- post-commit cleanup 覆盖 foreign child、foreign component、protected parent 与 cleanup hook/observer reentry
- 每次 cleanup hook/observer-capable mutation 后逐次重建 observed relationship closure 与 target/protected partition 并复验，drift 以更高 revision attempt supersede old versioned attempt，old attempt 只保留已完成且可验证的 receipts，不得继续 mutation 或 settle new attempt，也不得重复 obligation/receipt 或改写 app-owned state
- Task 6 明确排除 post-commit host/registration loss、ledger transfer 与 severe Recovery terminal
- cleanup identity matrix 覆盖 zero obligation、partial settlement、duplicate receipt、wrong batch/request/transaction/epoch、unknown obligation、unknown token、superseded attempt 与 completion replay
- Core token/completion 复用既有证据，不重复新增同层测试
- ECS table matrix 对 attempt、receipt、obligation 与 binding 的接受和拒绝逐项作证，每种拒绝均为 ledger 与 World zero mutation
- 至少一个 scheduled cross-layer request-to-settler journey 证明 batch/request/epoch binding，错误 binding 下 request/history 不推进，settlement 后额外 update 不重放
- 扩展现有 nested DefaultPlugins fixture，证明 new active primitive 有 reachable Node layout，最终 hover 与 target-event delivery 只到 new active primitive，不要求 raw backend hits 排除 pending 或 leaving candidates

Focused commands

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib navigation::tests::reconciliation_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib presentation::tests::reconciliation_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --lib cleanup::tests::reconciliation_ -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-features --lib audit::tests::reconciliation_ -j 1
```

- [ ] **Step 2: Run affected crate gates and review**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-features -j 1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-authority-regressions.ps1
cargo fmt --all -- --check
git diff --check
```

Expected: physical tests pass，authority 仍为 `23/23/23`

Review gate

- real World evidence 分别覆盖每个 slot role、preserved protected closure、authorized leaving/entering target 与其他 protected non-target partition
- authorized targets 与 preserved protected closure、其他 protected non-target partition 保持不相交，所有 mutation 都在 allowed write set 内
- read-only preflight 为 zero mutation，`Ready` atomic publication tail 仅有 no-hook in-place mutation，且无 refreeze、query、fallible validation 或 reclassification
- versioned cleanup attempts 正确 supersede，scheduled binding 与 cleanup rejection 均无重复或越权 mutation
- independent audit mapping 由 exact World 与 registry facts 推导，不由 receipt 或 snapshot 代替

```powershell
git add -- crates/bevy_ui_router_ecs/src/navigation.rs crates/bevy_ui_router_ecs/src/presentation.rs crates/bevy_ui_router_ecs/src/cleanup.rs crates/bevy_ui_router_ecs/src/audit.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "test(ecs): prove reconciliation physical lifecycle"
```

---

### Task 7: Public Journey B, fragment and facade evidence

**Files**

- Modify: `crates/bevy_ui_router/tests/public_navigation_journeys.rs`
- Modify: `crates/bevy_ui_router/tests/public_navigation_authority_audit.rs`

- [ ] **Step 1: Write public RED tests**

Journey B must assert

- layout frame version/root/outlet anchor/child slot unchanged
- app-owned host components、children identities与relative order unchanged
- after clearing change trackers，host sentinel/metadata/Node/Children与既有child `ChildOf`/components均非added/changed，app-owned lifecycle observer write counter保持零
- old leaf isolated before new leaf activation
- public audit payload binds `FrameIsolated` to the old leaf and `FrameActivated` to the new leaf，with no phase containing two interactive leaves
- receipt is `Removed` and precedes terminal
- old leaf audit id independently becomes `Removed`
- history entries/cursor are exactly `/items/1`, `/items/2`

Fragment must assert preserved ancestors/slot、same leaf frame id、generation `+1`、one effect with new fragment and the exact pushed history entry/cursor

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys reconciliation_journey_b_preserves_layout_and_replaces_leaf -- --exact
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit reconciliation_authority_is_preserved_and_old_leaf_is_removed -- --exact
```

Expected RED: current public path replaces the whole branch

- [ ] **Step 2: Close public evidence and facade gates**

Do not add public methods or test-support mutation seams

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-authority-regressions.ps1
git diff --check
```

Review gate:现有facade足以证明全部事实，replace/back/Recovery/InputProbes仍未解锁，authority为23/23/23

```powershell
git add -- crates/bevy_ui_router/tests/public_navigation_journeys.rs crates/bevy_ui_router/tests/public_navigation_authority_audit.rs
git diff --cached --name-only
git diff --cached --check
git commit -m "test: prove reconciliation public journeys"
```

---

### Task 8: Reconciliation completion gate and factual records

**Files**

- Modify: `docs/current-architecture.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/phase-3b1-architecture-review-log.md`
- Modify: `docs/reconciliation-contract-profile.md`
- Modify: `docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md`

- [ ] **Step 1: Run focused gates**

```powershell
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_core -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_view -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --all-features -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_export_allowlist -j 1
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test facade_authority_compile_fail -j 1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-authority-regressions.ps1
```

- [ ] **Step 2: Run full workspace gate serially**

```powershell
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features -j 1 --config "build.rustc-wrapper = ''"
cargo nextest run --locked --workspace --all-features --build-jobs 1 --test-threads 1 --config "build.rustc-wrapper = ''"
cargo test --locked --workspace --all-features -j 1 --config "build.rustc-wrapper = ''" --doc
git diff --check
git status --short
```

- [ ] **Step 3: Update factual record drafts**

Record exact plan commit、implementation range、actual counts、review outcomes placeholder and remaining History/Recovery/Authority/MVP work

- [ ] **Step 4: Run four independent read-only reviews and commit**

Review responsibilities

1. specification coverage and pending-slice honesty
2. Core lifecycle、single publisher、ID/generation与terminal ordering
3. ECS ownership、slot lifecycle、physical preflight、cleanup与authority settlement
4. public facade、Journey B、fragment与audit evidence

Resolve all Critical and Important findings，rerun affected gates and responsible reviews until PASS

Write final review outcomes into the drafts，then run one factual docs re-review before staging

Mark profile and plan `Complete` only after all gates and reviews pass

```powershell
git add -- docs/current-architecture.md docs/roadmap.md docs/phase-3b1-architecture-review-log.md docs/reconciliation-contract-profile.md docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md
git diff --cached --name-only
git diff --cached --check
git commit -m "docs: record reconciliation task pass"
git status --short
```

Expected: Reconciliation PASS与未完成的 History、Recovery、Authority closure、MVP保持清晰分离

---

## Plan review checklist

- [ ] profile `47da839` tracked and plan committed before Task 1
- [ ] production behavior change has RED/GREEN，coverage-only/docs use evidence/gate，every task still has a review gate and exact commit boundary
- [ ] logical frame/slot roles remain Core-owned
- [ ] exact Entity、manifest、ledger与receipt remain ECS-owned
- [ ] empty slot survives truncate and can be reactivated without identity reuse
- [ ] fragment preserves ancestors and advances only deepest generation
- [ ] physical AuthorityLost terminal is closed without implementing full Recovery
- [ ] normal Journey B proves both Removed receipt and independent tombstone
- [ ] authority catalogue remains exactly 23
- [ ] replace/back、Recovery、Authority closure、MVP and BSN remain pending
