# Reconciliation 阶段合同 Profile

**状态**: Complete — Reconciliation PASS

**实现 checkpoint**: Tasks 1–8 已闭合；Task 8 focused/full workspace 门禁精确归因于 clean `5806c59`，四路独立复审均通过

**设计基线**: `a977bd0`

**前置状态**: Nested Push PASS，最终实现 `702e36e`

**阶段结论**: Reconciliation PASS

**适用规范**: [Target Architecture](target-architecture.md)、[Authority Transition Matrix](authority-transition-matrix.md)、[MVP Acceptance Journeys](mvp-acceptance-journeys.md)、[Facade Contract Fixture](facade-contract-fixture.md)、[Nested Push Contract Profile](nested-push-contract-profile.md)

## 1 目标与边界

本切片把 Nested 的安全整支替换收窄为 common-prefix reconciliation

包含

- Journey B `/items/1 -> /items/2`
- layout frame version、root、outlet anchor 与 child slot 保留
- 只替换 leaf frame 与其 frame-local outputs
- nested fragment 保留 ancestors 与 child slot
- fragment deepest leaf 保留 frame id，generation 精确推进一次
- append、truncate、suffix replace 与 equivalent 的内部分类回归
- preserved、leaving、entering 一次物理 preflight 与一次原子 publication
- leaving leaf 的输入隔离、focus 释放、正常 cleanup receipt 与独立 audit tombstone

## 2 非目标

- `replace` 与 `back` 的公开 history 语义
- Journey C 与 Journey F2 的完整 facade PASS
- 完整 Recovery，包括 error presentation、mount recovery、host loss 后重建与重放策略
- `CommitPublished` 后且 foreground cleanup 未完成时的 host/registration loss、ledger transfer 与 severe anomaly 终态
- background reaper、完整多 router stress 与 Authority closure
- 最终 MVP gate
- BSN adapter 与 MVP 后 primitive 扩展

本切片只推进既有 `push` history entries，Journey B 成功后 entries 与 cursor 精确对应 `/items/1`, `/items/2`，fragment reconciliation 不提前宣称 `replace` history 合同完成

本切片仍包含 pre-commit `AuthorityLost` 的最小 settlement，用于安全隔离并清空失效 Core/ECS active

Authority Transition Matrix 对 post-commit host/registration loss 的最终语义保持不变，该窗口由后续 Recovery 切片实现，Reconciliation PASS 不宣称覆盖

## 3 Owner 与 authority 边界

| Owner | 本切片职责 | 禁止越界 |
| --- | --- | --- |
| Core | 决定 `Preserved | Leaving | Entering` roles、逻辑 router/host/slot/frame/request/transaction/task IDs、与 exact frame identity 绑定的 generation、sealed commit 与 publication | 读取 Entity、物理 authority identity、manifest 或 receipt |
| ECS | 消费 sealed roles，签发 ownership seal、cleanup ledger domain 与 attempt revision，验证 exact closure、分类 physical outcome、执行 splice、cleanup 与 audit | 改写逻辑 roles/IDs/generation、route matching、history/terminal publication或第二套 lifecycle |
| View | 根据 sealed frame task 生成受限 `ContentPlan` | 判断 preservation、构造 slot/frame identity或提交 active |
| Facade | 公开 push、snapshot、trace、opaque audit、renderer-neutral `RouteViewRegistry` 与 `RouterInputGate`/test-support host mapping 等有限 app-side Bevy Entity 输入 | 暴露 `RouterEngine`、ECS physical `RouteEntityRegistry`、router-owned raw Entity/World/registry conversion/accessor 或 authority/mutation constructor |

Core 保持唯一逻辑 lifecycle publisher

- Core prepared commit capability 发布 logical active、push history 与 commit trace
- Core 在匹配的 cleanup/authority-settlement completion evidence 完成后独占发布 request terminal；pre-mutation logical failure 可在 Core 内直接 terminalize，不需要 commit publication capability
- ECS 永不发布或重建 logical active、history 或 request terminal
- ECS-owned `ViewBuildReport` 与 `EcsViewReportSession` 是 facade-reachable public types。`ViewBuildReport` 是只读 kind/message DTO，只有 ECS-issued one-shot session terminal method 可以构造；raw report identity/fields、direct constructors 与 session constructor/seal 保持 private
- 与 public view-driver report 分离，ECS exact physical `FrozenCleanupManifest`、`CleanupLedger`、`CleanupReceipt` 与对应 exact Entity facts 保持 ECS-crate-private。`CleanupManifest`、`CleanupBatch`、`CleanupCompletion` 及相关 integration methods 是 Core `#[doc(hidden)] pub mod integration` 内的 `pub` items；部分 individual methods 可另带 item-level `#[doc(hidden)]`，不得泛化为每个 wrapper/method 都带该 attribute。Facade 不 re-export 该 integration module 或这些 items；它们绑定 opaque Core batch/completion identity 与 metadata。Core 验证 logical roots、exact batch/completion binding、count 与 required empty/non-empty shape，但不从物理事实重建或认证 ECS exact physical owner、manifest、ledger 或 receipt

不得新增 reconciliation runtime、第二个 publisher、逻辑 route/lifecycle allocator 或 adapter authority

## 4 Core 分类与私有 projection

分类输入同时绑定

- active branch fingerprint 与 target branch fingerprint
- router、host epoch 与 top slot
- 每层 branch index、route reuse key 与 mount slot provenance
- active frame version、parent frame version、outlet key 与 logical child slot identity
- canonical path、search 与 fragment delta

match 结果与全 branch frame/task reservation 在 enqueue 时冻结

queued request 成为 running 时，Core 才以当时 published active 计算 reconciliation projection，不能复用 enqueue 时的 active snapshot

Core 产生 slot-aware 私有 projection，为每层表达 `Preserved | Leaving | Entering` 并携带 expected parent/slot continuity

不得把旧 `FrameReconciliationPlan` 原样接入真实 nested engine，其单一 `RouteFrameScope` 形状只可作为分类行为与测试资产

真实接线必须覆盖 top slot 与逐层 child slot，不能用 raw slot projection 代替 opaque identity

分类规则

| 场景 | Preserved | Leaving | Entering |
| --- | --- | --- | --- |
| append | 全部旧 branch | 无 | 新 suffix，root-to-leaf |
| truncate | 新 branch 全部 | 旧 suffix，leaf-to-root | 无 |
| suffix replace | exact common prefix | 旧 suffix，leaf-to-root | 新 suffix，root-to-leaf |
| equivalent | 全部 active branch | 无 | 无 |
| nested fragment | deepest leaf 之前全部 | 旧 deepest leaf generation | 同 frame id 的下一 generation |

provenance 或 raw search 改变时 common prefix 固定为 0，整支 branch 进入 leaving/entering

普通 entering 使用 reserved 新 frame 与 initial generation

nested fragment 使用 deepest leaf 原 frame id 与 `checked_next` generation，恰产生一次携带新 fragment 的 deepest effect

generation 只与 exact frame identity 一起解释，裸 generation 数值不要求跨不同 frame 全局唯一

未使用的 reserved frame/task IDs 永久 burn，不因 preservation、失败或等价路径复用

完全等价 push 是 physical no-op：zero entering、zero leaving、zero view effect 与 zero physical mutation，但仍按正常 request 与 push intent publication history

fragment commit 前 active/history 不变，成功 push 后 history 才推进到新 fragment

fragment generation 溢出是 pre-mutation logical capacity failure，failure kind 固定为 `AuthorityLost`，但不触发 physical settlement、cleanup 或 fallback，old active/history 保持不变

任一 preserved baseline 与当前 Core active 不一致时禁止签发 commit ticket

## 5 Slot owner 与边界生命周期

Core 为 boundary slot 签发 sealed logical splice role，ECS 只把该 role 落为 exact lifecycle mutation

| 场景 | Core slot role | ECS exact lifecycle |
| --- | --- | --- |
| suffix replace / fragment，slot occupied | `PreservedActive` | zero mutation，exact identity 与 outlet mapping 保留 |
| truncate | `RetainEmpty` | 同一 commit 中 registry `Active -> PreparedInert`、`ChildSlot Active -> RegisteredInert`、Visibility Hidden |
| append 复用 empty slot | `ActivateExisting` | 同一 commit 中 registry `PreparedInert -> Active`、`ChildSlot RegisteredInert -> Active`、Visibility Visible |
| append 无 empty slot | `MountNew` | 走既有 sealed mount，registry-first staging并复验 |
| leaving parent | `Leaving` | parent-owned child slot 纳入 exact cleanup |

`RetainEmpty` 保持 `Pickable::IGNORE`、direct slot gate deny并释放 focus，不进入 cleanup

Core/ECS 保留 logical/exact slot binding 与 audit mapping，稳定态是 empty registered inert slot

`ActivateExisting` 后 `Pickable` 仍为 `IGNORE` 且 direct slot gate 仍 deny，child descendants 的交互只由 frame gate 授权

leaving parent 的物理顺序固定为 child frame → parent-owned child slot → parent frame

top slot 继续归 host epoch，不因普通 reconciliation 重建

## 6 ECS preflight 与 atomic splice

ECS 消费 Core sealed roles，在一次只读 preflight 中验证 preserved、leaving、entering exact closures并分类 physical outcome

每个 closure 均验证

- exact Entity generation、owner、seal 与 lifecycle claim
- exact host epoch、top-slot claim 与 registry partition
- frame version、branch index、slot owner 与 expected parent
- outlet anchor、child slot 与 parent frame version 连续性
- component manifest、inbound edge、acyclic traversal 与 foreign partition
- gate baseline、visibility、pointer、keyboard、gamepad 与 focus barrier
- active baseline 与 sealed commit ticket

preserved closure 只验证且不产生 mutation，leaving closure 预生成隔离顺序与 exact cleanup obligations

entering closure 必须已完整 `PreparedInert`，所有 activation values 在 preflight 前建立

allowed write set 只包含 exact Router-owned authorized target。isolation、quarantine 与 receipt mutation 不得落到 foreign descendant/component、protected 或 other-authority parent 及其 component/relationship；protected parent 本身不接收 quarantine receipt 或 mutation

preflight 成功后生成一个不可失败的 `CommitMutationPlan`

执行顺序固定为

1. leaving leaf-to-root 隔离输入并释放 focus
2. entering root-to-leaf 激活
3. registry 对 active branch 做一次 atomic splice
4. input gate 与预建 `Pickable` values 原子替换
5. Core 消费 sealed ticket并发布 presentation、push history 与 trace

splice 只替换 suffix mapping，preserved frame、root、outlet anchor 与 child slot 的 exact identity 不变

commit tail 不再 query、分配、插入组件、改 relationship或执行可失败验证

## 7 Drift、cleanup 与终态

| Closure evidence | Physical outcome |
| --- | --- |
| host、current 或 preserved authority drift | `AuthorityLost`，禁止 switch |
| leaving owner、seal、required component manifest 或 cycle corruption | `AuthorityLost`，禁止 switch |
| leaving authorized target 遇到 foreign descendant/component 或 protected inbound parent，且 target 可安全隔离 | 只隔离 exact Router-owned target并以该 owned residual 的实际 quarantine receipt 收敛；foreign/protected non-target 与 edge 零写 |
| entering drift | `PendingRejected`，只 cleanup pending |

physical `AuthorityLost` 执行本切片最小 pre-commit settlement

- 复用既有唯一 lifecycle publisher、ledger 与 receipt stream，不创建第二套 ledger 或重复 receipt
- availability 先进入 `RecoveringAuthority`，current running request 进入 `Running(Recovering)`
- queued requests 立即 `Failed(AuthorityLost)`，不执行 switch、fallback 或重放
- settlement 开始时，ECS active/pending/host owner 单次移动进 authority-settlement cleanup ledger，ECS active 立即变为 `None`，history 不推进
- receipts 未完成时，该 ledger 唯一持有 physical residual，request 保持 `Running(Recovering)`
- 全部 settlement receipts 完成后消费 ledger owner，Core 清空 logical active，terminal state 不再持有 active/lifecycle owner
- `CleanupSettled` 早于 current request 最后的 `RequestTerminal`
- current request 精确终结为 `Failed(AuthorityLost)`

若原 host registration 仍 exact 且 topology 合法，终态后可重跑既有 full admission，成功稳定为 `availability=Ready, active=None`，但不重放 navigation

registration 或 host 无效、admission 失败时使用既有 `Unavailable` reason，绝不静默重放旧 request

以上只闭合 Reconciliation 所需的 authority settlement 窄终态，不是完整 Recovery

entering-only 缺陷为 `PendingRejected`

- 保持旧 active 与 preserved closure
- failure roots 只取本 request 已 applied entering logical roots
- ECS 为这些 roots 冻结 exact Entity obligations并只 settlement pending outputs
- 不发布 history
- `CleanupSettled` 早于最后的 `RequestTerminal`

正常 replacement commit 后

- 旧 leaf 已 hidden、不可交互且不持有 focus
- 新 leaf 成为唯一可交互 leaf
- cleanup ledger 只包含 leaving suffix 的 exact Entity obligations
- 每项 obligation 以实际 terminal disposition 收敛
- request 在全部 receipt 后才 `Succeeded`

Journey B 正常 PASS 不接受 quarantine 替代，必须得到旧 leaf 的 `Removed` receipt，保存的 `AuditEntityId` 还必须由独立 World-existence + registry-absence audit 返回 `Removed`

receipt 与 audit tombstone 是两条独立证据，不能相互推导

## 8 验收矩阵

| 验收项 | 必须证明 |
| --- | --- |
| Journey B identity | layout frame version/root/outlet anchor/child slot 前后完全相同，leaf identity 更新 |
| Journey B order | `FrameIsolated(old_leaf)` 早于 `FrameActivated(new_leaf)`，receipt 早于 `RequestTerminal` |
| Journey B history | 成功后仅追加 `/items/2`，entries/cursor 精确为两个 push entries |
| Journey B cleanup | old leaf 正常 `Removed` receipt，稳定 audit tombstone 为 `Removed` |
| Journey B safety | 任一 phase 不存在两个可交互 leaf，app-owned host 零越权写入 |
| nested fragment | ancestors/root/outlet/slot 保留，deepest leaf frame id相同、generation `+1`、恰一次新 fragment effect |
| equivalent physical no-op | zero entering、zero leaving、zero effect 与 zero physical mutation，active identity 不变且 request/push history 正常 publication |
| append | preserved prefix不变，只从首个新 index逐层 mount |
| truncate | 只清理多余 suffix，retained parent 的 empty registered inert slot 与 audit mapping 保留 |
| suffix replace | leaving leaf-to-root、entering root-to-leaf，一次 splice与一次 Core publication |
| authority drift | preserved/current drift为 `AuthorityLost`，无 fallback、无 history推进 |

Journey B navigation 只通过推荐 facade `push` 与真实 `App::update` 驱动。稳定 consumer 事实由 snapshot、trace 与 opaque audit 证明；commit-boundary interactive-leaf cardinality 另由 test-local、只读、非权威的 `World + RouterInputGate` sampler 验证。ECS crate-private regression 证明 exact Entity 与物理 mutation，Core regression 证明分类、logical identity、sealed ticket 与唯一 publication

## 9 已执行门禁与目标状态定义

以下门禁已在精确干净 `5806c59` 树执行并通过

- Core complete target：286 unit tests、28 doctests
- View complete target：8 unit、6 dependency-boundary、1 public API
- ECS all-features complete target：401 unit tests 与全部 integration targets
- public navigation journeys 7/7，public authority audit 3/3
- facade export allowlist 46/46，facade authority compile-fail 40/40
- authority catalogue 23/23/23
- workspace fmt 与 all-target/all-feature check
- workspace nextest 838/838，0 skipped
- workspace all-feature doctest 28/28
- `git diff --check`，且 tracked tree 保持干净并指向 `5806c59`

实现与证据 checkpoints

- Reconciliation contract start / implementation-plan commit：`0201d49`
- Implementation/evidence range：`228c5bf..5806c59`
- Tasks 1–5：`228c5bf`、`c627a9e`、`6f8e734`、`9415d03` + `4f4e4dc`、`ddce26d`
- Task 6 plan gate：`9d7d2f0`；implementation：`e5a26e6`
- Task 7 plan/public gate：`a694acb`；cleanup guardrail：`9307d2f`
- Root fallback plans：`10dc5f4..81d2657`
- Task 7 prerequisite：`2e5cb3d`；public evidence：`5806c59`

最终独立复审结果

- Specification coverage and pending-slice honesty：PASS，no Critical/Important/Minor
- Core lifecycle、single publisher、logical IDs/generation 与 terminal ordering：PASS after review fixes，no remaining Critical/Important/Minor
- ECS ownership、slot lifecycle、physical preflight、cleanup 与 authority settlement：PASS after review fixes through draft5，no remaining Critical/Important/Minor
- Public facade、Journey B、fragment 与 audit evidence：PASS after review fixes，no remaining Critical/Important/Minor

四路职责全部通过且无 remaining Critical/Important/Minor，Profile 状态为 Complete

本次 Reconciliation PASS 由以下闭合事实共同支持

- Journey B 稳定 consumer 事实可由公开 facade snapshot/trace/opaque audit 观察，commit-boundary interactive-leaf cardinality 由 test-local read-only sampler 补充证明
- nested fragment 的 ancestor preservation 与 deepest-leaf generation 语义闭合
- append、truncate、suffix replace、equivalent 四类内部回归闭合
- 生产代码仍只有 Core 一个 lifecycle publisher
- preserved/current drift 全部 fail closed 为 `AuthorityLost`
- 最小 pre-commit authority settlement 在开始时把 ECS owner 单次移动进 ledger并清空 ECS active，在 completion 时消费 ledger owner、清空 Core logical active，且无 fallback/history 推进
- cleanup receipt 与独立 audit tombstone 均证明旧 leaf 已移除
- 文档仍明确 History、Recovery、Authority closure、`RouterInputProbes`、MVP 与 BSN 未完成
