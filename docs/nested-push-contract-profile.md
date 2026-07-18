# Nested Push 阶段合同 Profile

**状态**: Nested Push PASS

**合同起点**: `1ccbcd7`

**执行计划提交**: `eab1267`

**最终实现提交**: `702e36e`

**实现范围**: `eab1267..702e36e`

**适用规范**: [Target Architecture](target-architecture.md)、[Authority Transition Matrix](authority-transition-matrix.md)、[MVP Acceptance Journeys](mvp-acceptance-journeys.md)、[Facade Contract Fixture](facade-contract-fixture.md)

## 1 目标与边界

本切片建立第一条真实 nested push 主链

```text
public push
  -> matched root-to-leaf branch
  -> parent frame apply
  -> outlet registration
  -> sealed child mount
  -> child frame apply
  -> whole-branch preflight
  -> one atomic publication
```

包含

- Journey A 首次嵌套 push
- `TopSlot` 与 `ChildOutlet` 两类 mount target
- mount token、grant、receipt 的单次 sealed 协议
- parent outlet 登记后的逐层 root-to-leaf apply
- 全 branch 一次 preflight、一次 lifecycle publication
- mount 失败分类、staging 清理和永久 ID gap
- frame-local traversal、input/focus barrier 与 leaf-to-root cleanup
- Nested public API、opaque audit 与 facade gate

不包含

- common-prefix preservation、leaf-only replacement 与 Journey B
- nested fragment-only deepest-frame remount
- `replace`、`back` 与 history cursor 行为
- route-local error boundary、mount recovery presentation 与 host-loss recovery
- background reaper、multi-router stress 与 Authority closure
- BSN adapter

既有 Root match、root build 与 root apply failure 的 mandatory `RouterFallback` 语义必须保持

完全等价导航保持现有 zero-entering 语义，branch-changing nested push 在 Reconciliation 前安全替换整支 branch，不保留 parent frame 或 child slot

depth-one Root fragment generation 语义不得回退；nested fragment-only 在本阶段同样执行整 branch replacement，并验证旧 branch 完整收敛、新 branch 使用新 frame/slot identity、history 只随成功 publication 推进，Reconciliation 再把它收窄为 ancestor preservation 与 deepest-frame remount

## 2 Authority 与类型边界

Core 继续作为唯一逻辑 lifecycle authority

- 匹配有序 `MatchedBranch`
- 为每层分配 frame/task identity
- 签发和消费 mount token/grant
- 只在全部 frame 已 prepared 后签发一个 commit ticket
- 发布完整 active frame branch、history 与 trace

ECS 独占物理 authority

- exact Entity、registry、ownership seal、manifest 与 lifecycle claim
- outlet anchor 与 child slot 的物理校验
- registry-first staging、observer-capable write 后复验与 receipt
- branch preflight、input/focus mutation、cleanup ledger 与 quarantine

View 继续只传递 renderer-neutral registry、descriptor、effect 与受限 content plan

- driver 可声明 outlet anchor
- driver 不能分配 slot、构造 mount receipt、伪造 frame/generation 或发布 lifecycle
- `EcsViewReportSession` 继续私有携带 sealed report identity

不得新增 nested runtime、第二个 publisher、第二套 ID allocator 或 facade authority constructor

## 3 Core 状态机

Root-only frame task 收敛为可表达 branch index 与 mount target 的内部 task

```text
FrameMountTarget
  = TopSlot { host_version, slot_id }
  | ChildOutlet { parent_frame_version, outlet_key }
```

首次 nested attempt 按以下顺序推进

1. Core 为 branch 第 0 层签发 top-slot frame task
2. ECS 完成 parent plan realization并回报 accepted frame 与物理 outlet 集合
3. Core 根据 matched child target 签发不含 slot ID 的 `MountRequestToken`
4. ECS 解析 required outlet；缺失则消费 token 为 `MissingOutlet`
5. ECS 对存在的 anchor 做 exact identity、owner、seal、manifest 与 parent-frame 只读验证
6. Core 消费 resolved token，单调分配 child slot ID 并返回 `MountGrant`
7. ECS staging-register child slot，写 inert Node 与 `ChildOf(anchor)`，逐阶段复验
8. ECS 交回 receipt，Core 确认 grant 后才签发 child frame task
9. 重复直到 leaf prepared，随后 Core 签发整 branch commit ticket

正常 trace 至少保持以下有序子序列

```text
FrameTaskIssued(parent)
ShellPrepared(parent)
ViewApplied(parent)
OutletRegistered(parent)
MountRequested
MountResolved
MountGranted
FrameTaskIssued(child)
ShellPrepared(child)
ViewApplied(child)
FrameActivated(parent)
FrameActivated(child)
CommitPublished
RequestPhaseChanged(PostCommitCleanup)
CleanupSettled
RequestTerminal
```

全部 task、report 与 mount artifacts 共同绑定 router domain、host epoch、transaction、request、sequence 与预期阶段

frame task/report 另外绑定 exact frame version 与已确认 mount slot；`MountRequestToken`、`MountGrant` 和 mount receipt 只在 `ChildOutlet` 路径存在，并额外绑定 exact parent frame version 与 outlet key，`TopSlot` 路径不使用 sentinel parent/outlet

任何 reject、cancel、confirm 或 replay 都是单次终态，失败分配的 slot、frame、task、seal 与 authority ID 永不复用

## 4 ECS 物理模型

Nested ancestry 固定为

```text
app-owned host Node
  -> router-owned top slot Node
       -> parent frame shell Node
            -> content target Node
                 -> outlet anchor Node or SolidRect
                      -> child slot Node
                           -> child frame shell Node
```

`AcceptedContentPlan` 保留稳定排序的 `(local_index, RouteSlotKey)` outlet 声明，realization 将 local index 映射到 exact output Entity 与 claim

child slot 创建顺序固定为

1. empty Entity allocation
2. staging registry record
3. hidden、gate deny、`Pickable::IGNORE`
4. 透明 100% Node required closure
5. private authority component 与 `ChildOf(anchor)`
6. exact Entity、owner、seal、manifest、lifecycle、anchor edge 和唯一 mapping 复验
7. receipt confirm 后转为 registered inert child slot

所有 entering frames 和 slots 在 commit 前保持 hidden、不可交互且不持有 focus

## 5 Frame-local traversal、commit 与 cleanup

每个 frame closure 包含 shell、content target、frame outputs 和自己的 outlet anchor

遍历遇到 child slot 时处理 slot container 自身并停止，不进入 child frame shell

所有实际 relationship traversal 使用稳定顺序与 exact-Entity visited set，cycle 或 revisit 立即 fail closed

最终 preflight 同时验证完整 entering branch、现有 leaving branch、host/top slot、registry partition、gate baseline 与 publication ticket

`Ready(CommitMutationPlan)` 后的不可失败 switch 只执行预解析的原地变更

1. leaving leaf-to-root 隔离输入并释放 focus
2. entering root-to-leaf 激活，parent activation 不得越界恢复 child
3. registry active branch 与 gate snapshot 原子替换
4. Core 发布完整 presentation、history 与 trace

publication 后 request 进入 `Running(PostCommitCleanup)`，leaving frames 与 child slots 才由 sealed ledger 按 leaf-to-root exact obligations 执行可能触发 hook/observer 的 cleanup；全部 terminal receipts 被 Core 验证后 request 才终结

parent cleanup 不得沿物理 descendants 级联进入 child authority root，也不得触碰 app-owned host 或 foreign descendants

## 6 Failure 分类

| Failure | 分类 | 物理结果 | Nested 阶段结果 |
| --- | --- | --- | --- |
| duplicate、foreign-session、cycle | plan-level `ApplyFailed` | mount 前 World/registry 零变化 | entering cleanup，保留旧 active |
| entering parent 缺 required outlet | `MissingOutlet` | 已有 request，无 slot ID、无 child Entity | `PendingRejected`，安全清理 |
| entering anchor identity/owner/seal 漂移 | `InvalidOutlet` | 无 child slot | `PendingRejected`，安全清理 |
| grant staging 后 drift | `RejectedAfterMutation` | slot ID burned，residual settlement | request failed，安全清理 |
| existing parent outlet 失权 | `ParentAuthorityLost` | 隔离并 settlement | `AuthorityLost`，禁止 fallback |
| child apply 或 branch preflight 失败 | pending failure | 旧 active 保持，entering branch 清理 | request failed |

Root 已有 mandatory `RouterFallback` 继续处理 Root match/build/apply failure

Nested 本切片只为 child mount/apply failure完成分类、隔离与安全收敛，不提交新的 route-local error presentation，Recovery 阶段再接 error boundary、mount recovery 与 fallback 选择

所有 pre-commit failure 在物理收敛前保持非终态

- plan-level、`MissingOutlet`、`InvalidOutlet`、`RejectedAfterMutation`、child build/apply 与 pending preflight failure 进入 `Running(CleanupPending)`，Core 签发 sealed cleanup batch，全部 terminal receipts 完成后追加 `CleanupSettled`，再发布终态与最后一个 `RequestTerminal`
  - plan-level 与 pending preflight failure 使用 `NavigationFailureKind::Apply`
  - `MissingOutlet`、`InvalidOutlet` 与 `RejectedAfterMutation` 使用 `NavigationFailureKind::Mount`
  - child build/apply 分别使用 `NavigationFailureKind::Build` 与 `NavigationFailureKind::Apply`
- `ParentAuthorityLost` 进入 `Running(Recovering)`，authority settlement 完成后追加 `CleanupSettled`，再发布 `Failed(AuthorityLost)` 与最后一个 `RequestTerminal`
- `RequestTerminal` 前不得存在 foreground prepared frame、child slot、cleanup obligation 或未确认 settlement ticket

## 7 Public contract 与阶段 exclusions

Nested 阶段从最终合同中解锁且只解锁

- `RouteTreeBuilder::route`
- `RouteTreeBuilder::route_with_options`
- `FrameContentWriter::declare_outlet`
- `AuthorityOutletMapping`
- `RouterAuthorityAuditSnapshot::outlets`

其余延后项保持以下 machine-readable exclusions

| Kind | Canonical path or normalized item | First slice |
| --- | --- | --- |
| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineLeaseId` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerKind` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputModality` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputProbes` | Authority closure |
| associated | `bevy_ui_router::ecs::RouterNavigation::replace` | History |
| associated | `bevy_ui_router::ecs::RouterNavigation::back` | History |
| associated | `bevy_ui_router::core::RouteNodeOptions::with_error_boundary` | Recovery |
| associated | `bevy_ui_router::core::RouteNodeOptions::with_error_view` | Recovery |
| associated | `bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines` | Authority closure |

不得为阶段实现新增 `Unsupported`、`NotImplemented` 或临时 public error/variant

### 7.1 Nested Authority Regression Additions

| Test function |
| --- |
| `mount_grant_observer_drift_rejects_after_mutation` |
| `mount_token_replay_is_zero_write` |
| `slot_lifecycle_has_no_orphans` |

Nested 当前 catalogue 精确等于 [Root Authority Regression Catalogue](root-push-contract-profile.md#32-root-authority-regression-catalogue) 的 20 项与上表 3 项的集合并集，共 23 项

catalogue target 必须解析两个一列表格，拒绝空项、重复项与非最终 MVP 名称，并与 Rust 常量、`#[cfg(test)]` 直接注册集合双向相等；`scripts/verify-authority-regressions.ps1` 使用同一 23 项集合核对 binary registration 和实际通过数

## 8 观察面与验收

audit 必须证明

- 两个 active frame version 各有且仅有一个 exact root Entity
- root frame 挂 top slot，leaf frame 挂 child slot
- `(parent frame version, outlet key)` 唯一映射到 anchor 与 optional child slot
- anchor、slot 与 frame 只通过 opaque `AuditEntityId` 关联
- stable state 无 orphan 或 provisional child slot

公开 Journey A 必须证明

- `/items/1` 产生 layout 与 leaf 两次 effect
- child effect 严格晚于 parent apply、outlet registration 与 mount grant
- active presentation 含 root-to-leaf 两个 frame
- history 只提交一次 `/items/1`
- commit 前全 entering branch inert，commit 后两个 primitive 可见且 active-pickable
- app-owned host identity、组件、既有 children identity 与相对顺序保持

Nested replacement regression 必须证明 branch-changing push 使用整 branch replacement，先隔离旧 leaf-to-root closure，再原子激活新 branch，history 只随成功 publication 推进；该回归不宣称 Journey B 或 common-prefix PASS

Nested fragment regression 必须证明 fragment-only push 在本阶段使用整 branch replacement，旧 frames 与 child slots 全部收到 terminal receipt，新 branch 使用新 identity，history 只提交一次；该回归不宣称 Reconciliation 的 ancestor preservation 或 deepest-frame remount PASS

Root regression 必须继续证明 depth-one equivalent、fragment-only generation、changed-root、fallback 与 app-owned host 零写入语义不变

私有回归必须覆盖

- token reject、cancel、confirm、duplicate、stale replay 与 ID gap
- missing/invalid outlet、grant observer drift 与 staging cleanup
- registry-first 和每次 observer-capable write 后复验
- frame-local visibility/input/focus/cleanup traversal
- child-slot orphan/provisional 闭合
- 全 Node physical ancestry 与真实 picking
- app-owned host 与 foreign Entity 零写入

Nested PASS 需要公开 journey、opaque authority audit、ECS regression catalogue、facade closed-world gates、Root regression、workspace tests、格式检查和独立事实复审全部通过
