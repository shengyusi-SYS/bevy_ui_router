# Reconciliation 阶段合同 Profile

**状态**: Reconciliation 实施中

**实现 checkpoint**: Tasks 1–5 complete，`ddce26d`，Tasks 6–8 pending

**设计基线**: `a977bd0`

**前置状态**: Nested Push PASS，最终实现 `702e36e`

**目标状态**: Reconciliation PASS

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
| Core | 决定 `Preserved | Leaving | Entering` roles、frame identity、generation、sealed commit 与 publication | 读取 Entity、物理 manifest 或 receipt |
| ECS | 消费 sealed roles，验证 exact closure、分类 physical outcome、执行 splice、cleanup 与 audit | 改写逻辑 roles、route matching、history publication或第二套 lifecycle |
| View | 根据 sealed frame task 生成受限 `ContentPlan` | 判断 preservation、构造 slot/frame identity或提交 active |
| Facade | 公开 push、snapshot、trace 与 opaque audit 观察面 | 暴露 engine、registry、Entity或 authority constructor |

Core 保持唯一 lifecycle publisher，ECS 只能交回 sealed report、digest、count 与 completion evidence，不能自行发布 active 或终结 request

不得新增 reconciliation runtime、第二个 publisher、ID allocator 或 adapter authority

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

未使用的 reserved frame/task IDs 永久 burn，不因 preservation、失败或等价路径复用

完全等价 push 保持 zero entering、zero leaving、zero view effect，但仍按正常 push intent publication history

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
| leaving foreign descendant/component 或 protected parent，可安全隔离 | 允许 commit，以实际 quarantine receipt 收敛 |
| entering drift | `PendingRejected`，只 cleanup pending |

physical `AuthorityLost` 执行本切片最小 pre-commit settlement

- 复用既有唯一 lifecycle publisher、ledger 与 receipt stream，不创建第二套 ledger 或重复 receipt
- availability 先进入 `RecoveringAuthority`，current running request 进入 `Running(Recovering)`
- queued requests 立即 `Failed(AuthorityLost)`，不执行 switch、fallback 或重放
- 隔离并 settlement 已知 exact owned closure，history 不推进
- 全部 settlement receipts 完成后 Core/ECS active 同时变为 `None`
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
| equivalent | zero entering、zero leaving、zero effect，active identity 不变且 push history 正常 publication |
| append | preserved prefix不变，只从首个新 index逐层 mount |
| truncate | 只清理多余 suffix，retained parent 的 empty registered inert slot 与 audit mapping 保留 |
| suffix replace | leaving leaf-to-root、entering root-to-leaf，一次 splice与一次 Core publication |
| authority drift | preserved/current drift为 `AuthorityLost`，无 fallback、无 history推进 |

公开 facade journey 证明 consumer 可见事实，ECS crate-private regression 证明 exact Entity 与物理 mutation，Core regression 证明分类、identity、sealed ticket 与唯一 publication

## 9 门禁与 PASS 定义

完成前必须通过

- Core running-time projection、reservation burn、classification、fragment generation 与 publisher uniqueness 回归
- ECS slot ownership、四类 splice、role-sensitive drift、settlement 与 exact cleanup 回归
- Journey B public journey、authority audit 与 facade contract gate
- Root、Nested 既有旅程与 authority catalogue 无回退
- workspace fmt、check、nextest、doctest 与 diff check
- Core lifecycle、ECS physical authority、public facade evidence 独立复审

Reconciliation PASS 只在以下条件同时成立时记录

- Journey B 全部稳定态和中间态事实可由公开 facade 观察
- nested fragment 的 ancestor preservation 与 deepest-leaf generation 语义闭合
- append、truncate、suffix replace、equivalent 四类内部回归闭合
- 生产代码仍只有 Core 一个 lifecycle publisher
- preserved/current drift 全部 fail closed 为 `AuthorityLost`
- 最小 pre-commit authority settlement 清空失效 Core/ECS active且无 fallback/history 推进
- cleanup receipt 与独立 audit tombstone 均证明旧 leaf 已移除
- 文档仍明确 History、Recovery、Authority closure、MVP 与 BSN 未完成
