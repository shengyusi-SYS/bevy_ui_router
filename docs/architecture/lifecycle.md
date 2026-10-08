# 生命周期执行协议

本文用于修改跨层 lifecycle：解释 Core action 如何交给 ECS、未完成责任保存在哪里，以及准备失败如何分流。用户行为和必须维持的不变量归[生命周期合同](../contracts/lifecycle.md)，物理结构与调度归[运行机制](runtime.md)。下述 holder 是私有实现，不是应用可调用的推进 API。

## Action 与物理责任交接

Core 的 [engine](../../crates/bevy_ui_router_core/src/engine.rs) 决定下一项 action，[integration](../../crates/bevy_ui_router_core/src/integration.rs) 提供受控交接；ECS [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs) 的 `RouterEntry` 保存跨 update 的原 owner 和物理进度。Core 的逻辑 projection 与 ECS 的 Entity 结构分别验证，不能由其中一方反推另一方的权限。

| Core 交接 | ECS 持有与准备 | 消费/下一责任 |
| --- | --- | --- |
| `FrameTask` | `PendingCommit` 内的 prepared branch 累积已准备层；未完成驱动由 `pending_frame_drive` 保留 task、effect 与 pending 内容 | apply/adoption 结果交回 Core；失败内容连同原 task/owner 转入 failed branch |
| `MountRequest` | 验证 parent/outlet 后建立 provisional slot，保存 receipt 与 confirmation | 确认后进入 prepared branch；拒绝后 slot 的原 owner 随失败分支结算，不能因确认失败丢弃 |
| `CommitTicket` 或最后一个 `final_task` | current/next projection 对接为 sealed reconciliation；准备物理切换、claim rebind、输入/focus 和 cleanup owner | publication work 消费预备状态，Core 最后发布；leaving 责任交给 cleanup |
| `FailureCleanup` | failed branch 冻结 manifest，预留 ledger，再与 Core cleanup batch 绑定 | 逐项物理 receipt 形成 typed completion；Core 消费原 continuation 决定恢复或 terminal |
| `BackResolution` | history authority 与 capacity reservation 配对，保存完整 source evidence，分别准备 target/no-target | 有目标进入既有执行与 publication；无目标不运行 view，完成明确的失败/authority 结算 |

拒绝、容量不足或 completion error 必须返还原 owner 给对应 holder；不能重新签发一个看似等价的 task。已发布 active、pending branch、cleanup ledger 和私有 residual 是不同责任，不以任一容器是否为空替代协议完成。

history 的签发与配对在 [history_authority.rs](../../crates/bevy_ui_router_core/src/history_authority.rs)。publication 后的 compound settlement 在 Core [post_commit.rs](../../crates/bevy_ui_router_core/src/engine/post_commit.rs) 与 ECS navigation 中，保留已发布结果。已退役缓存继续由原 normal token/ledger 持有，Live 缓存进入 authority 第二批；完整库存随原 receiver 冻结，联合完成分别结算两个 token。延迟删除非权威 preservation proof 时只移除原选定项，保留其他 Router 的合法变化。无运行请求的 host loss 则由 Core [idle_host_loss.rs](../../crates/bevy_ui_router_core/src/engine/idle_host_loss.rs) 和 ECS [idle_host_loss.rs](../../crates/bevy_ui_router_ecs/src/navigation/idle_host_loss.rs) 持有独立 settlement owner，不重开 terminal 请求。

## 准备结果与失败分流

`reconciliation_preflight` 检查 host/current 与 pending entering；[lifecycle_preparation.rs](../../crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs) 保存 source、expectation 和 observed state。下表归纳 reconciliation、projectionless fallback 与 Back target 外层 preparation 的语义类别，并非同一个通用 enum。结果携带相应 owner，各路径保留自己的具体 classification，不能互换为 bool 或通用错误。

| 结果 | 事实与后续路径 |
| --- | --- |
| `Ready` | 当前接收端与准备事实匹配；继续完成 publication preparation，尚不等于页面已经发布 |
| `PendingRejected` | entering/rebase 拒绝，projectionless fallback 另含 admission 与 target overlap；保留原 pending owner，清理本次产物，再按原失败来源进入恢复 |
| `AuthorityLost` | host、preserved 或 leaving current 的权限失效；隔离相应失效内容并进入 authority settlement，不能只清理 pending 后继续信任 current |
| 可重试准备不足 | 容量或可完成前置事实不足；保存原 owner/未完成义务，之后重试相应准备，不能重跑已消费的 Scene |
| `Permanent` | 进入明确的 fail-close 与责任结算；不能无限重试或退回普通导航来掩盖矛盾 |
| `PairingMismatch` / `EntryMismatch` | 拒绝消费并返还完整 owner 与 evidence，进入对应 mismatch/inert holder；后续按该协议显式结算，不能混作 pending rejection 或重建 owner |

同时发现 current authority loss 与 pending rejection 时，先处理 authority loss。Back target 的 registry mutation plan 或 leaving manifest 准备矛盾归 `Permanent`，不能仅按“target 有问题”归为 pending rejection。物理删除完成、无剩余 target 或保留了旧 active，均不能决定逻辑 continuation；它来自原 failure/authority、attempt 与 branch 分类。cleanup completion 还会独立验证该 continuation。

## 修改时定位哪些模块

| 机制 | 实现职责 |
| --- | --- |
| [branch_transaction.rs](../../crates/bevy_ui_router_ecs/src/branch_transaction.rs) | 对接 Core projection；稀疏 slot role、sealed splice、prepared/failed branch 和 claim rebind |
| [normal_publication.rs](../../crates/bevy_ui_router_ecs/src/navigation/normal_publication.rs) / [projectionless_publication.rs](../../crates/bevy_ui_router_ecs/src/navigation/projectionless_publication.rs) | 前者编排 reconciliation 与缓存命中，后者编排无 reconciliation 的 fallback；共用 retained commit、离场 forest 和 publication holder，各自保留 preflight 与失败来源 |
| [retained.rs](../../crates/bevy_ui_router_ecs/src/retained.rs) | 原长期 owner 与物理 payload 的选择、预分配、移交及拒绝返还；逻辑 preserve/retire 分区由 Core [retained.rs](../../crates/bevy_ui_router_core/src/engine/retained.rs) 决定 |
| [frame_local.rs](../../crates/bevy_ui_router_ecs/src/frame_local.rs) | 按 frame 自身 role/binding 验证局部 closure，遇 child slot 停止，不把子 frame 并入父内容权限 |
| [bsn.rs](../../crates/bevy_ui_router_ecs/src/bsn.rs) / [native_scene.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs) | 前者构建并准备声明图；后者对已准备声明预分配登记、官方 executor apply、adoption 和即时失败隔离 |
| [native_frame.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_frame.rs) | adoption 后 containment、native cleanup preparation 和依赖保活 |
| [native_failure.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_failure.rs) | Scene preparation/creation 失败的来源、依赖与物理 residual owner |
| [authority_isolation.rs](../../crates/bevy_ui_router_ecs/src/navigation/authority_isolation.rs) | current authority loss 的隔离写集与后续 settlement |
| [cleanup.rs](../../crates/bevy_ui_router_ecs/src/cleanup.rs) | target/protected evidence、operation-local removal plan、逐写复验及 receipt/ledger completion；[capacity.rs](../../crates/bevy_ui_router_ecs/src/cleanup/capacity.rs) 负责容量预留与持有 |

跨层改动先核对 action 接收端、当前 holder 和失败返回值，再修改实现。验证入口包括 [ECS native 回归](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs)、相关模块内部回归，以及[公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs)和[独立 BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs)；按[验证指南](../development/verification.md)选择完整 gate。
