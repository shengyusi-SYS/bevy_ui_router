# Authority Transition Matrix

**文档类型**: Phase 3B.1 生命周期规范附录
**状态**: Phase 3B.1 `Plan PASS`
**架构上位规范**: [Target Architecture](target-architecture.md)

**`ddce26d` pre-commit checkpoint**：仅最小 pre-commit settlement 已实现，post-commit Recovery 仍待完成

Target Architecture 拥有架构边界与不变量；本文拥有 token、closure、cleanup、quarantine 与 request terminal 的详细转换语义。本文不得改变 Target 的 ownership/依赖边界；若两者在生命周期细节上冲突，以本文为准。

## 1. 状态轴

每次 authority 决策由以下独立轴共同决定：

```text
object role       = Host | TopSlot | ChildSlot | PreservedFrame | LeavingFrame | EnteringFrame
ownership state   = ExactOwned | ProtectedForeignDescendant | ProtectedForeignComponent
                  | Missing | OwnerMismatch | ProtectedTarget
inbound parent    = ExpectedOwned | AuthorizedHostTopSlotEdge | Missing
                  | ProtectedAppOwned | OtherAuthorityOwned
token state       = Requested | Granted | Confirmed | Rejected | RejectedAfterMutation | Cancelled
host epoch state  = Current | Stale | Settling
quarantine state  = Live(lease_generation, operation_sequence) | Closed | Unknown
```

输出只能是：

```text
Ready(CommitMutationPlan)
PendingRejected(PendingCleanupPlan)
AuthorityLost(AuthoritySettlementPlan)
```

跨状态轴的 token 与证据规则：

- lifecycle capability 线性持有，只能单次移动或消费
- 完整 source projection、snapshot 与 fingerprint 只能验证，不能重建 owner
- 拒绝路径返还原 owner
- 延迟或 one-shot action 在消费前从当前 authority 重建完整 source projection 并要求 exact equality
- 逐元素有效不构成 mutation 或 publication 授权
- boundary evidence 在 freeze、classify、settle 间保留 source、exact expectation、observed state、obligation 与 owner 的类型化关联
- 只有可证明未来可完成的容量或前置事实不足可以 retry，永久矛盾必须有界 fail closed

## 2. Host Admission

| Event | Token/ID effect | ECS effect | Published state |
| --- | --- | --- | --- |
| unique candidate observed and candidate revision is not latched | issue `HostAdmissionToken(Requested)`; no host/slot ID | none | `AwaitingHost` |
| candidate缺少 `Node`/required closure，或到 UI root 的实际 ancestry含 non-Node/cycle/ghost-only edge | engine consumes token as `Rejected` | none；Router不补写 host/ancestor | `Unavailable(InvalidHostUiTopology)`；同一registration拓扑修复后可重新 admission |
| candidate conflict/disappears before grant | engine consumes token as `Rejected` | none | `Unavailable(HostConflict)` or `AwaitingHost` |
| app cancels registration before grant | engine consumes token as `Cancelled` | none | `AwaitingHost` |
| exclusive preflight succeeds | allocate new host id, epoch and top slot id; return `HostGrant` | none yet | unchanged |
| grant staging + component/relationship mutation | grant remains `Granted` | allocate empty inert Entity; staging-register before reentrant mutation; write then validate | unchanged |
| post-write validation succeeds | grant `Confirmed` | promote staging host/top-slot records | `Ready + None` |
| post-write validation fails or observer removes/rewrites Entity | grant `RejectedAfterMutation`; allocated IDs remain burned；latch current candidate revision | sealed cleanup/quarantine/`AlreadyAbsent` settles staging records | never publish host/slot；settlement后为 `Unavailable(AuthorityEstablishmentFailed)` |
| replay rejected/rejected-after-mutation/cancelled/confirmed token or grant | no allocator or World effect | none | unchanged; stale diagnostic |

Preflight failure always consumes the outstanding admission token through a sealed reject/cancel transition. 不存在“未消费但永久 outstanding”的 token。ID 只在 exclusive preflight 成功后分配，允许永久 gap；grant 后任何 Bevy mutation 都视为可重入且需写后复验。只有 validation 后 confirm 才可发布，失败 grant 的 staging residual 必须逐项 settlement。

`HostCandidateRevision` 是ECS私有、稳定单调的candidate revision，绑定registration component instance/revision、app host exact Entity generation与admission topology fingerprint：

- 实例身份使用显式incarnation，Bevy change tick只能作为观察事实，不能单独承担身份
- `RejectedAfterMutation`闩锁当前revision，revision未变时不再签发token、分配ID或创建quarantine
- registration重插或host identity/topology变化后产生新revision，允许一次新admission
- 初次安装与authority recovery共用该规则，持久observer不能触发无限重试

ECS ownership record另遵守：

- ownership seal首次成功登记后永久消费，删除、失败清理或转入quarantine都不释放
- registry transition只接受匹配exact Entity generation、owner、seal和预期lifecycle的当前claim，stale或历史claim零写拒绝

## 3. Host Loss And Recovery

| Event | Existing requests | Foreground settlement | Published availability / next epoch |
| --- | --- | --- | --- |
| registration removal, host exists，发生于 commit 前 | queued -> `Failed(HostLost)`；running -> `Running(CleanupPending)` | isolate；用旧 HostLease detach + validate old top-slot host edge；再 cleanup/quarantine并 confirm ticket | settlement 中 `RecoveringAuthority`；旧 edge 消失后为 `Unavailable(HostLost)`，request `Failed`且history不推进 |
| host Entity despawn，发生于 commit 前 | queued -> `Failed(HostLost)`；running -> `Running(CleanupPending)` | exact Entity records become `AlreadyAbsent`，其余 detached records 逐项 settlement，confirm ticket | settlement 中 `RecoveringAuthority`；完成后 `Unavailable(HostLost)`，request `Failed`且history不推进 |
| registration removal/host despawn，发生于 `CommitPublished` 后且 foreground cleanup 未完成 | committed request -> `Running(Recovering)`；queued仍 `Failed(HostLost)` | 已有 cleanup obligation ledger原子并入/转交 authority settlement，禁止重复 receipt；隔离并清空 active | availability `RecoveringAuthority -> Unavailable(HostLost)`；已提交 history不回滚；committed request settlement后 `Succeeded + Severe(PostCommitCleanupAnomaly)` |
| active authority drift with declaration still present | queued -> `Failed(AuthorityLost)`；running -> `Running(Recovering)` | isolate/cleanup/quarantine old epoch, confirm ticket | 对未闩锁的新candidate revision重跑一次完整admission；成功为新 id/epoch/top slot 的 `Ready + None`，post-write失败为 `Unavailable(AuthorityEstablishmentFailed)` |
| registry self-audit detects duplicate identity/mapping, impossible owner transition or internal seal contradiction | queued -> `Failed(AuthorityLost)`；running -> `Running(Recovering)` | isolate every still-exact known owned record；unknown/conflicting records only isolate or quarantine；confirm settlement ticket | fail closed为`Unavailable(RegistryInvariantViolation)`；该router instance不自动re-admit，应用只能安装新的router instance |

Running request becomes terminal only after foreground settlement receipt and engine confirmation. Old host admission/cleanup/settlement token or receipt cannot mutate or settle the new epoch；已转入独立 quarantine table 的 live lease 只按自身 lease generation/operation sequence 管理 immutable exact set，不再以 former host epoch 判 stale。

## 4. Child Mount Token

`ContentPlan` 在 driver 返回前拒绝 duplicate outlet key、foreign-session handle、self-edge、duplicate parent/edge 与 cycle。这些都是 parent view 的 plan-level `ApplyFailed`：不产生 child target、`MountRequestToken`、slot ID 或 Entity，trace 中也不得出现 mount event。下表只描述 parent plan 已被接受后，RouterEngine 已规划 child target 的 mount handshake。

| Event | Token/allocator | Entity/registry | Request |
| --- | --- | --- | --- |
| child target planned | issue `MountRequestToken(Requested)` | none | `Running(ResolvingMount)` |
| 已接受的 entering parent output 缺少 required outlet | consume token as `Rejected(MissingOutlet)` | none | `PendingRejected`；cleanup pending，允许 error recovery |
| entering output 已登记 anchor，但 realization/post-write 复验发现 identity、owner、seal、Entity 或唯一映射非法 | consume token as `Rejected(InvalidOutlet)` | none | `PendingRejected`；cleanup pending，允许 error recovery |
| preserved/current parent 的既有 outlet identity、owner、seal 或 Entity 失效 | consume token as `Rejected(ParentAuthorityLost)` | none | `AuthorityLost`；禁止 fallback，进入 authority settlement |
| request/parent cancelled | consume token as `Cancelled` | none | cleanup pending attempt |
| exclusive preflight succeeds | consume token, allocate slot id, return `MountGrant` | none yet | unchanged |
| grant staging + relationship mutation | grant remains `Granted` | allocate empty inert Entity；staging-register；write slot/`ChildOf` then validate | no child task |
| post-write validation succeeds | grant `Confirmed` | promote child slot + `EntityMountReceipt` | issue child frame task |
| post-write validation fails or observer removes/rewrites Entity | grant `RejectedAfterMutation`; slot ID burned | settle staging residual | pending failure/recovery；no child task |
| replay rejected/rejected-after-mutation/cancelled/confirmed token/grant | no allocator or World effect | none | current request unchanged; stale diagnostic |

Token reject/cancel is sealed and terminal。Grant 后允许 tracked inert provisional 和 post-write rejection，不允许 untracked provisional；所有失败 ID 永不复用。

## 5. Closure Classification

`ProtectedForeignDescendant` 表示 closure 中存在未登记 app-owned descendant；`ProtectedForeignComponent` 表示 exact owned Entity 上存在 registry manifest 外 component。两者都不是 owner corruption，Router 也都不取得其 ownership或触发其 hook。

| Closure role | ExactOwned | ProtectedForeignDescendant / Component | Missing / OwnerMismatch / seal mismatch | ProtectedTarget |
| --- | --- | --- | --- | --- |
| entering/pending | `Ready` | `PendingRejected`；quarantine pending unit，旧 active 不变 | `PendingRejected`；cleanup pending，旧 active 不变 | `PendingRejected`；绝不触碰 protected Entity |
| preserved active | `Ready` | `Ready`；保持 foreign Entity，不对 preserved closure mutation | `AuthorityLost` | `AuthorityLost` |
| leaving active | `Ready`，plan normal isolate/remove | `Ready`，plan isolate + foreground quarantine | `AuthorityLost` | `AuthorityLost` |
| current host/top slot | `Ready` | 正常 navigation 为 `Ready`；host settlement 时 plan quarantine | `AuthorityLost` | `AuthorityLost`，host 绝不删除 |
| child slot owned by leaving parent | `Ready` | `Ready` with slot quarantine | `AuthorityLost` | `AuthorityLost` |
| child mount under entering parent | `Ready` | `PendingRejected` | `PendingRejected` | `PendingRejected` |
| child mount under preserved/current parent | `Ready` | preserved closure 不 mutation；新 child output 自身按 entering 分类 | `AuthorityLost` | `AuthorityLost` |

因此，leaving foreign descendant 不阻止新 presentation 提交：preflight 生成不可失败的 quarantine plan，switch 先隔离 leaving lease root，再激活 entering。Preserved foreign descendant 不属于 Router mutation 范围。Entering foreign descendant 不允许激活，按 pending failure 回退。

混合 drift 的只读 preflight 必须先按每个 source 已封存的 exact role 独立验证并冻结分类结果，再合并整体 outcome；任一 source 的 `AuthorityLost` 不得被 entering 或 pending 的 `PendingRejected` 覆盖，也不得因先命中后者而跳过其余 source 验证

任何 actual relationship traversal 都使用 exact-Entity visited set；plan/session-local cycle 在 World mutation 前 `PendingRejected(RelationshipCycle)`，pending/entering external cycle 为 `PendingRejected`，preserved/leaving/current active cycle 为 `AuthorityLost`。分类后 settlement 依据 registry immutable exact set，不沿 cyclic descendants 递归 despawn。

Inbound parent 与上表 ownership/descendant 分类正交：

- `ExpectedOwned` 或 `Missing` 允许按已验证 plan 删除；`AuthorizedHostTopSlotEdge` 只允许当前 HostGrant/HostLease 通过 exact top-slot `ChildOf` 触发 Bevy `Children` membership维护，不直接写 host、不改变其他 child identity/relative order。
- entering/pending root 发现 `ProtectedAppOwned | OtherAuthorityOwned` 时为 `PendingRejected`，隔离并产生 `QuarantinedProtectedParent`，旧 active 不变。
- preserved/current root 发现未授权 parent edge 时为 `AuthorityLost`；leaving root 可继续 commit，但必须在 switch 前隔离并产生 `QuarantinedProtectedParent`。
- Router 永不通过 despawn/remove relationship 改写未授权 app-owned/other-authority parent；应用自行解除 edge 后才可 reap。
- Host settlement 必须在 current HostLease 关闭前 detach 并复验 old top-slot edge；只有 edge 已消失后才能把 top slot transfer 给独立 quarantine lease。后台 reaper 因而永不持有 `AuthorizedHostTopSlotEdge`。

## 6. CommitMutationPlan Closure

switch 进入 `Ready(CommitMutationPlan)` 前必须完成：

- current host version、top slot 与 registry partition
- preserved、leaving、entering 的每个 root/slot/content/output Entity
- `EntityOwner`、Entity generation、seal、actual component-id manifest、actual inbound parent classification、ancestry、frame generation 与 branch index
- entering `PreparedInert`、visibility、preinserted `Pickable::IGNORE`、gate keyboard/gamepad deny 与 focus barrier
- leaving normal cleanup 或 quarantine partition
- entering activation order、leaving isolation order、registry replacement 和 sealed core commit ticket
- old/new `RouterInputGate` active/blocked snapshot、prebuilt `Pickable` values 与原子 replacement
- entering staging 中每个可能触发 hook/observer 的写入均已在此前完成，并逐次通过 post-write validation
- 随后的 switch preflight 保持只读并冻结显式 role partition，先确定 exact leaving 与 entering lifecycle value 是 authorized switch target
- 剩余 preserved、non-target current 与 protected non-target closure 均与 authorized switch target 不相交，non-target current 只包含 authorized switch target role 之外的 current Entity
- 任何 preflight drift 都阻止进入 `Ready` 并转入适用的 reclassification 或 settlement 路径

cleanup 与 authority settlement 的 hookful mutation 和 post-write revalidation 不属于 `CommitMutationPlan`，由 §7 的 versioned cleanup/settlement attempt 处理

`Ready(CommitMutationPlan)` 后：

- `CommitMutationPlan` 只应用 Target Architecture 所述已经准备好的 no-hook in-place values
- atomic publication tail 中不发生 refreeze、query、可失败 validation 或 reclassification
- 执行顺序为 leaving isolate/focus release、entering activate、registry swap、input-gate + preinserted `Pickable` value swap、core publish
- 不 add/remove `InteractionDisabled` 等 marker
- 任何 closure corruption 都在 plan 生成前分类为 `PendingRejected` 或 `AuthorityLost`

## 7. Cleanup Obligation Ledger

每次 pending cleanup、post-commit cleanup 或 authority settlement 都形成一个 sealed batch：

```text
CleanupBatchToken = router + request/settlement + transaction + host epoch
                  + batch id + manifest digest + obligation count
CleanupObligation = exact Entity generation + EntityOwner + logical removal duty
CleanupAttempt = batch id + attempt revision + unsettled obligation digest + fresh World evidence
```

协议固定为：

Cleanup target/manifest 与 post-cleanup continuation 正交；前者描述必须 settle 的 exact obligations，后者由 engine 在 cleanup 前依据原始 failure/authority classification 与 attempt/branch role 封存，completion 不能根据 active presentation、obligation count 或 disposition 重新分类

1. ECS 根据已经通过的preflight/settlement plan生成确定顺序、不可变的entity obligation identity manifest，并向engine提交逻辑mounted roots、digest与count；manifest冻结“哪些exact owned records必须前景收敛”，不冻结未来物理`Removed/Quarantined/...` disposition，Entity不进入core。
2. Engine 验证 roots 属于当前 request/transaction/host epoch 后签发 opaque token；ECS 只接受 digest/count 与本地 manifest 完全相同的 token，并以 token 建立唯一 ledger。
3. 每次执行从当前未结算obligations生成递增revision的`CleanupAttempt`，先只读重做inbound parent、component manifest、descendant partition、cycle/visited与exact generation分类，再产生本attempt的物理plan。跨set app drift不能复用commit时的旧plan。
4. 每个可能触发relationship/component/despawn hook或observer的mutation后立即post-write validation。若证据变化使剩余plan失效，当前attempt只保留已完成且可验证的receipts，其余未结算obligations由更高revision attempt重新分类；旧attempt不能继续mutation，也不能结算新attempt。
5. 每个obligation只能由同batch、同exact Entity/owner的**实际**terminal disposition receipt结算一次。`Removed`、`RemovedDetached`、`AlreadyAbsent`、`RejectedProtected`、`QuarantinedForeignDescendant`、`QuarantinedProtectedParent`和`QuarantinedForeignComponent`都可终结前景obligation；quarantine receipt原子转交当时仍存在的immutable exact residual set给独立lease，因此持久drift不会让request永久nonterminal。
6. 完全相同的重复receipt返回`DuplicateRejected`；错误batch、旧epoch、错误request/transaction、未知obligation、未知token或superseded attempt revision返回`StaleDiscarded`/`InvalidRejected`，均无ledger、presentation、history或request状态变化。
7. 全部obligations恰好结算一次后，ECS把原token与completion digest交回engine。Engine只接受当前outstanding batch，随后才可消费预先封存的continuation或进入terminal；completion replay无副作用。Host/authority loss只能转交同一批未结算obligation identity，不能复制ledger或receipt。
8. obligation count为零时，batch在同一阶段签发并立即完成，仍留下batch trace；不得用“没有receipt”绕过terminal gate。

Core 只签发/验证 batch identity 和逻辑 roots，ECS 独占 Entity manifest 与 receipt authority，因此不存在 core 读取 World 或 ECS 自行终结 request 的反向依赖。

## 8. QuarantinePlan

Foreign descendant 所在的 scheduled mounted-unit closure 整体进入一个 lease：

```text
lease root = FrameShell | ChildSlot | HostTopSlot | DetachedFrameOutput
lease set  = transfer 时封存的 scheduled exact owned records；不依赖后续实际 ancestry
foreign set = lease root 下所有未登记 Entity，只观察 identity，不取得 ownership
protected parent = lease root 的未授权 app-owned/other-authority inbound parent edge，只观察 identity
foreign components = manifest 外 component IDs，只观察存在性，不 remove/despawn
```

规则：

1. 把 scheduled-for-removal 的 `HostTopSlot | ChildSlot | FrameShell` 及其级联删除关系建成 removal DAG。对每个 foreign descendant 先选择最近的 scheduled mounted-unit ancestor；若 exact registry-owned `FrameOutput` 已漂移到所有预期 mounted boundary 之外，则把该 output 作为 `DetachedFrameOutput` cleanup node，绝不沿实际 ancestry取得 foreign ownership。
2. 若删除某个更高 scheduled ancestor 会级联触碰已选 unit，则沿 DAG 向上提升到该 ancestor；反复执行直到固定点。最终选择按 ancestry 最大、也就是最高的 scheduled roots，合并重叠 closure；不同 lease 的 owned Entity 集合必须互斥。
3. 在一个 exclusive mutation 中，lease root 先切换为 hidden、`Pickable::IGNORE`、`RouterInputGate` deny并释放 subtree focus；所有 owned records 从 active/pending owner 原子转为 `QuarantineOwner`，并签发独立 `QuarantineLeaseId + lease generation + operation sequence`。`InteractionDisabled` 不参与 Router lifecycle barrier。
4. 三种 quarantine receipt 此时即可终结 foreground request/host settlement。Quarantine table 继续保存 former host version、previous owner、actual inbound edge、foreign component IDs 和 exact Entity generations。
5. Reaper 保存按 `QuarantineLeaseId` 排序的 round-robin cursor。每次 update 恰处理一个 live lease，处理后 cursor 前进到严格后继并在末尾回绕；当前 lease 关闭时从其后继继续，新 lease 插入不重置 cursor。因此永久 lease 不能饿死后续 lease，也不依赖 query iteration order。
6. Foreign set 为空且 protected parent edge 已由应用解除后，reaper 重新 partition；只删除 exact owned set并关闭 lease。
7. Foreign 或 protected parent 永久存在时 lease 永久 inert，但不阻塞新 host epoch、navigation queue 或 request retention；Router 永不移动、改写或删除 foreign/protected Entity 或 relationship。

| Reaper observation | Mutation | Lease result |
| --- | --- | --- |
| exact root、owner/live lease generation 有效，foreign descendant/component count > 0 或 protected parent edge 仍存在 | 验证并在 owned root 重施 inert barrier；不触碰 foreign，不移除 protected relationship/component | 保持 open；operation sequence 前进 |
| exact root、owner/live lease generation 有效，foreign count = 0 且 inbound parent 为 owned/missing | 删除 immutable ledger 中的 exact owned set | `Removed` 或安全 detached 时 `RemovedDetached`，关闭 |
| root missing 或 generation 已被复用 | 不触碰 replacement；root obligation 记为 `AlreadyAbsent`，逐项检查 immutable lease set；仍存在的 exact owned Entity 按确定顺序重组为 detached child lease 或安全删除 | 原 root lease 仅在全部 records 已 terminal 或原子转入 child lease 后关闭；不丢弃 surviving record |
| exact registry-owned root 被 reparent 到 same-lease owned parent 或无 parent，lease owner/generation 仍匹配 | 记录 severe diagnostic，只对 exact owned root 重施 barrier；foreign 清空后按 ledger 删除 | 保持 open，最终 `RemovedDetached` |
| exact registry-owned root 被 reparent 到 app-owned/protected/other-authority parent | 记录 severe diagnostic，只对 exact owned root重施 barrier；不 despawn、不 remove relationship | `QuarantinedProtectedParent`，保持 open直到应用解除 edge |
| quarantine lease 仍观察到 former host 的 authorized top-slot edge | 无 mutation；记录 invariant violation，恢复 host settlement detach gate | lease 保持 open；禁止新 epoch admission/后台删除 |
| lease/registry 错误映射到 host 或其他 protected app-owned target | 绝不移动、改写或删除 target；该 record 记为 `RejectedProtected` 并记录 invariant violation；其余 immutable records 仍逐项结算或转入 child lease | 全部 records 收敛后关闭 |
| live lease 的 former host epoch 已旧，但 lease generation/operation sequence 当前 | 只访问 immutable exact owned set；不得访问新 epoch registry | 按正常 reaper 行处理 |
| closed/unknown lease、错误 lease generation/operation sequence 或 duplicate replay | 无 mutation | stale，任何 live lease/current host epoch 不变 |

Foreground request 在 lease 创建 receipt 时已经 terminal；reaper 的关闭结果只进入 diagnostics/audit，不反向改写历史 request。

`QuarantineOwnerKind::FrameOutput` 只表示 `DetachedFrameOutput` root；正常位于 frame shell 内的 output 随 `FrameShell` lease 结算。无论 root 是否消失、被复用或映射损坏，lease 都以 immutable owned set 逐 Entity 收敛，不能因单一 root terminal 而遗忘其他 exact-owned Entity。

## 9. Request Terminal Matrix

| Outcome | Terminal timing | Status |
| --- | --- | --- |
| normal commit | post-commit normal cleanup receipt 后 | `Succeeded` |
| normal failure, no recovery attempted | pending cleanup/quarantine receipt 后 | `Failed(original cause)` |
| error presentation committed | suffix cleanup/quarantine receipt 后 | `Failed(original cause)`；recovery成功，presentation为Error |
| recovery attempt also fails | all recovery output cleanup/quarantine receipt 后 | `Failed(RecoveryFailed)`；原始cause与recovery cause保留在trace/diagnostics |
| host/authority loss before `CommitPublished` | host settlement receipt 后 | running -> `Failed`；queued 可提前 `Failed`；history不推进 |
| host/authority loss after `CommitPublished` but before foreground cleanup terminal | outstanding cleanup/authority settlement统一完成后；active清空、history不回滚 | committed request `Succeeded` + severe `PostCommitCleanupAnomaly`；queued request仍 `Failed(HostLost)` |
| other post-commit anomaly | authority settlement 后，不回滚 presentation/history | `Succeeded` + severe diagnostic |
| queued back becomes ineligible when running | zero-obligation cleanup batch 完成后；不渲染 fallback | `Failed(BackTargetUnavailable)`；active/history 不变 |
| stale report/token | old output cleanup receipt 后 | current request unchanged |

任何 terminal request 都满足 foreground cleanup 已结束；quarantine receipt 视为 foreground 已结束，但 background lease 可继续存在。

terminal evidence 还必须证明该 request/settlement scope 不再持有 outstanding action、ticket、capability 或 lifecycle owner；独立 quarantine lease 只能以自身 owner 存续，且不得回写该 terminal

## 10. Required Regressions

- admission/mount token 的 reject、cancel、confirm、重复和 stale replay。
- writer self-edge/duplicate-parent/cycle 的 World-zero rejection，以及外部 pending/active relationship cycle 的 role-sensitive classification与 visited traversal termination。
- host/mount grant 的 observer-induced despawn/reparent、`RejectedAfterMutation`、staging cleanup 和 allocator ID gap。
- persistent admission observer跨多update只产生一次失败revision；registration/topology revision变化后才允许一次新admission。
- host必须是合法 UI Node ancestry；non-Node/ghost-only/cyclic host拒绝且Router不补组件。全部 Router-owned结构 container均为透明、`Pickable::IGNORE`的 Node，nested physical layout/picking可达；成功 `spawn_empty` 必须同时由opaque结构拓扑audit与locked-Bevy私有回归证明，而不是只覆盖失败plan。
- `PendingRejected` 与 `AuthorityLost` 对同类 identity mismatch 的 role-sensitive 分类。
- leaving foreign child 能 commit + quarantine；entering foreign child 必须拒绝且旧 active 不变。
- `CommitPresentation -> SettleCleanup`窗口中的foreign child/component/protected-parent drift必须supersede旧cleanup attempt；cleanup hook/observer重入也必须停止旧plan、重分类且不触碰app-owned state。
- top-slot-only、child-slot-only、frame-output quarantine 的进入、foreground terminal、后台 reap 和永久 lease。
- unauthorized app-parent relationship 的 foreground terminal、永久 inert、应用解除后 reap，以及授权 host-top-slot edge 的正常清理。
- round-robin 中排序最前的永久 lease 与后续可 reap lease 并存，后者在有界轮次内关闭。
- cleanup batch 的零 obligation、部分完成、重复 receipt、错误 batch、旧 epoch、未知 obligation 与 completion replay。
- authority recovery 总是新 host id/epoch/top slot，旧 lease/reaper 不能影响新 epoch。
- 同一 app host Entity 重接纳前 old top-slot edge 必须已 detach；之后 reaper 不改写新 epoch host relationship。
- host 和其他 protected app-owned Entity 永不删除、移动或改写。
- 同 frame commit 前后的 gate/active pairing：old/old 或 new/new，绝不 old/new；PreUpdate refresh只收窄权限。
- 锁定真实 UI picking/hover pipeline：raw backend hits 可含 candidate，但 pending/leaving/quarantine `Pickable::IGNORE` 无最终 hover/target event，active value可触发；commit 只做 in-place value mutation且不触发生命周期 observer。
- 应用系统精确插入 `CommitPresentation` 与 `SettleCleanup` 之间并移除/despawn host：已提交history保留，active经settlement清空，request成功并带 severe diagnostic；同类 pre-commit host loss仍失败且不推进history。
