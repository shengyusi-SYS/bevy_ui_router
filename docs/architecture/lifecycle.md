# 生命周期执行协议

[中文](lifecycle.md) | [English](lifecycle.en.md)

本页帮助开发者追踪一项 action 从 Core 到 ECS，再到完成或失败的过程。用户行为归[生命周期合同](../contracts/lifecycle.md)，实体结构与调度见[运行机制](runtime.md)。文中的 holder 是保存原 owner 与执行进度的内部容器。

结构 manifest 只记录路由结构所需的组件存在性。Bevy 派生的 `CalculatedClip`、`Inherited<ComputedUiTargetCamera>` 和 `Inherited<ComputedUiRenderTargetInfo>` 可在正常 UI 更新中变化，不参与结构相等性；所有预登记与实际观察使用同一投影。暂存和 detach 若依赖这些信息，分别冻结并复验自己的 UI 继承事实，registry claim、owner、seal、lifecycle 与关系校验保持精确。

## Action 与物理责任交接

Core [engine](../../crates/bevy_ui_router_core/src/engine.rs) 决定下一项 action，[integration](../../crates/bevy_ui_router_core/src/integration.rs) 提供跨 crate 交接。ECS [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs) 的 `RouterEntry` 保存跨 update 的 owner 和物理进度。Core 的逻辑 projection 与 ECS 的实体结构分别验证。

| Core 交接 | ECS 准备与持有 | 完成后的责任 |
| --- | --- | --- |
| `FrameTask` | `PendingCommit` 累积 prepared branch；`pending_frame_drive` 保存未完成的 task、effect 和 pending 内容 | apply/adoption 结果交回 Core；失败内容连同原 task/owner 转入 failed branch |
| `MountRequest` | 校验 parent/outlet，建立 provisional slot，保存 receipt 与 confirmation | 确认后进入 prepared branch；拒绝时 slot 原 owner 随失败分支结算 |
| `CommitTicket` 或最后一个 `final_task` | current/next 对接为 sealed reconciliation，准备切换、claim rebind、输入/focus 和 cleanup owner | 消费 publication work，Core 发布，leaving 责任交给 cleanup |
| `FailureCleanup` | 冻结 failed branch manifest，预留 ledger，绑定 Core cleanup batch | 物理 receipt 组成 typed completion；Core 消费原 continuation 决定恢复或终结 |
| `BackResolution` | 配对 history authority 和容量预留，保存完整 source evidence，分别准备有目标与无目标路径 | 有目标进入执行与发布；无目标完成相应失败或 authority 结算 |
| `RetainedMissCleanupOwner` | 保存原 continuation、独立 cleanup token、依赖 forest ledger 和失效证据 | 完成后返还原 continuation；停止或 authority 终结路径消费相应原责任 |

拒绝、容量不足和 completion error 都返还原 owner。active、pending、cleanup ledger 与 residual 各有独立责任；完成判断依据对应协议和 receipt。

库存自身失效清理停止后，保留已删除项的 receipt。若剩余原 shell 可证明为空且已隔离，受控停止路径将它交给私有 residual owner，完成原 token，再把原 fresh task 送回 authority 观察与结算。

若停止的依赖清理仍有存活目标，且 host binding 已失效，专用 authority terminal 消费原 installed owner 与 lease，将 token、receipts、stop、payload 和库存 owners 交给 authority residual。该路径结束前台 continuation，保留物理未完成状态；拒绝会返还完整 owner/lease，残留继续阻止重新接纳。

history 签发与配对位于 [history_authority.rs](../../crates/bevy_ui_router_core/src/history_authority.rs)。发布后组合结算由 Core [post_commit.rs](../../crates/bevy_ui_router_core/src/engine/post_commit.rs) 决定逻辑完成，ECS [navigation/post_commit.rs](../../crates/bevy_ui_router_ecs/src/navigation/post_commit.rs) 推进原 owner，[cleanup/post_commit.rs](../../crates/bevy_ui_router_ecs/src/cleanup/post_commit.rs) 结算物理证据。已退休缓存保留原 normal token，仍存活库存进入 authority 的第二批，联合完成分别消费两个 token。完整库存随原 receiver 冻结；删除延迟 preservation proof 时只移除原选定项，保留其他 Router 的合法变化。

空闲 host loss 使用独立 settlement owner，由 Core [idle_host_loss.rs](../../crates/bevy_ui_router_core/src/engine/idle_host_loss.rs) 与 ECS [idle_host_loss.rs](../../crates/bevy_ui_router_ecs/src/navigation/idle_host_loss.rs) 处理，已终结请求保持终态。

## 延后退出与应用展示

[Core exit](../../crates/bevy_ui_router_core/src/engine/exit.rs) 将稳定 cleanup batch identity 与实际 manifest seal 分开。普通导航仍立即 seal；可见退出在 publication 移交原 `DeferredExitOwner`，释放后才从实际 containment 建立 manifest。history 始终在第一次 publication 提交，不在 release 重发。

ECS [transition](../../crates/bevy_ui_router_ecs/src/transition.rs) 只持有应用 session 与受控展示值；[deferred_exit](../../crates/bevy_ui_router_ecs/src/navigation/deferred_exit.rs) 持有原逻辑 owner、实体分区和冻结的隐藏目标。会话可先取消，精确 presence 投影持续到旧内容真实隐藏。可见性不改变 lifecycle/input。

发布前准备在原 active owner 仍可查询时调用应用，current 与 next claims 分别冻结。立即、retained 和 deferred 发布共用 [publication](../../crates/bevy_ui_router_ecs/src/navigation/publication.rs) 接收器；Back 保留原 continuation。退出 release 将真实 seal 接入原 ledger，不建立平行的动画清理状态机。

## 准备结果与失败分流

`reconciliation_preflight` 检查 host/current 与 pending entering。[lifecycle_preparation.rs](../../crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs) 保存 source、expectation 和 observed state。不同 preparation 路径有各自的具体类型，以下按含义对照。

| 结果 | 事实与后续路径 |
| --- | --- |
| `Ready` | 接收端与准备事实匹配，继续完成发布准备 |
| `PendingRejected` | entering/rebase 拒绝；projectionless fallback 还包括 admission 和 target overlap。保留原 pending owner，清理本次产物，按原失败来源恢复 |
| `AuthorityLost` | host、preserved 或 leaving current 的权限失效；隔离失效内容并进入 authority settlement |
| 可重试准备不足 | 容量或可完成的前置条件不足；保存原 owner 与未完成义务，重试该项准备 |
| `Permanent` | 永久矛盾，停止推进并结算责任 |
| `PairingMismatch` / `EntryMismatch` | 返还完整 owner 与 evidence，进入对应 mismatch/inert holder，再按该协议结算 |

current authority loss 与 pending rejection 同时出现时，优先处理 authority loss。Back target 的 registry mutation plan 或 leaving manifest 准备矛盾属于 `Permanent`。

cleanup 的物理义务与完成后的 continuation 独立冻结。continuation 取自原 failure/authority、attempt 和 branch 分类，并在 completion 时再次校验；删除数量、剩余 target 和旧 active 状态仅描述物理进度。

## 修改时定位哪些模块

| 模块 | 职责 |
| --- | --- |
| [branch_transaction.rs](../../crates/bevy_ui_router_ecs/src/branch_transaction.rs) | Core projection 对接、稀疏 slot role、sealed splice、prepared/failed branch、claim rebind |
| [normal_publication.rs](../../crates/bevy_ui_router_ecs/src/navigation/normal_publication.rs) / [projectionless_publication.rs](../../crates/bevy_ui_router_ecs/src/navigation/projectionless_publication.rs) | reconciliation/缓存命中与 fallback 两条发布准备路径；共用 retained commit、离场 forest 和 publication holder，各保留自身失败分类 |
| [retained.rs](../../crates/bevy_ui_router_ecs/src/retained.rs) / [instance.rs](../../crates/bevy_ui_router_ecs/src/instance.rs) | 原长期 owner 的选择与移交；单实例 payload 和预分配接收器。Core [retained.rs](../../crates/bevy_ui_router_core/src/engine/retained.rs) 决定保留/淘汰 |
| [frame_local.rs](../../crates/bevy_ui_router_ecs/src/frame_local.rs) | 按 frame 自身 role/binding 验证局部 closure，在 child slot 处停止 |
| [bsn.rs](../../crates/bevy_ui_router_ecs/src/bsn.rs) / [native_scene.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs) | 声明图准备、预分配登记、官方 executor apply、adoption、即时失败隔离 |
| [native_frame.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_frame.rs) | adoption 后的 containment、native cleanup 准备和依赖保活 |
| [native_failure.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_failure.rs) | Scene 创建失败的来源、依赖和 residual owner |
| [authority_isolation.rs](../../crates/bevy_ui_router_ecs/src/navigation/authority_isolation.rs) | current authority loss 的隔离写集和后续结算 |
| [cleanup.rs](../../crates/bevy_ui_router_ecs/src/cleanup.rs) | 删除计划、执行、receipt/ledger completion；[failure_partition.rs](../../crates/bevy_ui_router_ecs/src/cleanup/failure_partition.rs) 管理 target/protected 与逐写复验，[capacity.rs](../../crates/bevy_ui_router_ecs/src/cleanup/capacity.rs) 管理容量预留 |

改动时先找到 action 接收端、当前 holder 和失败返回值。验证包括 [ECS native 回归](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs)、模块内部回归、[公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs)与[独立 BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs)；具体组合见[验证指南](../development/verification.md)。
