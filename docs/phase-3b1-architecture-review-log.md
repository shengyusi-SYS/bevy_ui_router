# Phase 3B.1 Architecture Review Log

**状态**: Phase 3B.1 `Reconciliation PASS`；结构重写评估为 `NO REWRITE`
**审查日期**: 2026-07-10；Nested completion 2026-07-18；Reconciliation completion 2026-07-20

## Round 1

四位只读审查者分别负责整体架构、ECS 生命周期与 authority、facade/MVP 可执行性和代码事实。四份结论均为 `NEEDS REVISION`。

### 共同根因

第一版定义了目标部件和正常稳定态，却没有把跨层 authority 的进入、失效、恢复和观察合同完全闭合。具体表现为 facade 旅程与内部攻击测试混层，error recovery/history 没有唯一终态，host loss 与 fallback 矛盾，renderer 写权限、全局 registry cardinality、frame-local traversal、cleanup receipt 和 feature 合同不足以直接实施。

### 负责人评估

接受并合并处理：

- 明确 World 内唯一、按 router 分区的 engine/registry，多 router 稳定排序。
- 固定受限 `FrameContentWriter`，不向 adapter 暴露 raw World/Commands 或 authority marker。
- 增加 frame-local traversal barrier。
- 定义 host registration/lost 状态机和 ErrorActive presentation/history 语义。
- 定义 sealed cleanup receipt 与 terminal disposition。
- 冻结 facade 签名形状、feature matrix 和 settled 条件。
- 将验收拆为 facade journeys、只读 audit 与 ECS regressions。
- 把 roadmap 改为端到端垂直切片。

调整后接受：

- 审查建议为 facade 增加观察面，但生产 API 只暴露逻辑 snapshot；Entity 映射放在无 mutation capability 的 `test-support` audit surface。
- stale 的合法 replay 保留在 facade journey；伪造 identity 只进入 ECS 内部回归，避免扩大生产 API。

不接受：

- 不将默认 facade feature 改为 ECS。默认继续 Bevy-free，完整 MVP API 明确由 `ecs` feature 提供。
- 不引入通用 World undo log。原子性由 exclusive system 的只读预检和不可失败 publish 保证。

### 修订结果

`target-architecture.md`、`mvp-acceptance-journeys.md`、`current-architecture.md`、`roadmap.md` 和文档状态用语已按共同根因统一修订，进入 Round 2。

## Round 2

三位新的只读审查者分别从整体架构与生命周期、facade/feature/验收、代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

第一轮修订已补齐概念，但 request、availability、presentation、report 和 cleanup 仍混在同一状态叙述中；公共装配没有反向写成可编译 fixture，feature gate 只检查命令成功，host 之外的 authority graph 漂移也没有收敛协议。

### 负责人评估

接受并统一修订：

- 正交化状态，只有 request `Succeeded/Failed` 是等待终态。
- 增加 request-id 查询、mandatory fallback、plugin 幂等安装和完整 fixture 顺序。
- 固定 immediate `EcsViewDriver`/writer/report session 合同与类型归属。
- 区分 registration removal、host despawn 和 active graph drift settlement。
- 精确定义 outlet anchor、child slot container 与 frame-local traversal。
- 收窄 preflight 的零变化保证，并显式允许后续 pending cleanup。
- 用消费者编译测试证明 feature API 可达，补齐兼容 feature 图。
- 前移 Root push 所需的最小 view/history 能力。

调整后接受：

- stale 回放仍是硬验收，但移入 ECS crate-private Scenario E；不为 immediate MVP 暴露延迟 report 通道。
- authority 攻击注入放在 `#[cfg(test)]` 私有单元测试；`ecs-test-support` 只提供只读 audit。

第二轮修订后进入 Round 3。

## Round 3

三位新的只读审查者分别从整体架构/authority、facade 合同、代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

逻辑 authority 与 Entity authority 的边界已有定义，但跨边界尚缺少完整的签发握手、receipt、失败终态和可观察证据；少量无正常 route、重复注册和长期记录边界也未闭合。

### 负责人评估

接受并统一修订：

- 增加 core mount request/token/grant 与 ECS Entity resolution/receipt 的 sealed handshake。
- 扩展 frame origin，显式支持 matched route、route error 和 router fallback；match failure 的 matched 为 `None`。
- 将 report acceptance session 移到 ECS authority，view report 保持无权威数据值。
- 明确 facade Resource 获取、下一 update 生效和 prelude 导出。
- 增加 AwaitingHost/HostConflict/RecoveringAuthority admission。
- 以 exact Entity generation 与 ownership seal 证明漂移实体仍可安全隔离，替换含糊的 detached cleanup。
- 定义 post-commit cleanup 的 request 终态与 queue 行为。
- 增加有界 request retention、生命周期 trace、重复 driver key 拒绝和独立 view-test-support gate。

调整后接受：

- 不把 post-commit cleanup 异常倒算成导航失败；presentation/history 已提交后，request 在 settlement 完成后仍为 `Succeeded`，同时记录高严重度诊断。
- 不开放延迟 report 或 mutation harness；时序由只读 request/audit trace 证明，stale 仍由 crate-private 回归覆盖。

第三轮修订后进入 Round 4。

## Round 4

三位新的只读审查者分别从跨层 authority、facade 可编译性和代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

身份签发主体已经明确，但 host admission/loss、slot 创建/取消、逐实体 ownership 和 settlement 仍未保证“每个 Entity 从出生起可追踪、每个 running request 在 cleanup 后才终结”。Facade 行为也需要一个独立的可编译 fixture 规范，旧低层公共面的迁移必须明确。

### 负责人评估

接受并统一修订：

- 增加 host admission token/grant/lease/settlement 与 registration epoch。
- grant 前不创建 slot；slot 创建、登记、确认在同一 exclusive block，定义 owner 与离场顺序。
- running request 在 host/authority loss cleanup 完成前保持非终态。
- registry 改为逐实体 ownership；遇到 foreign child 时 quarantine owned ancestor，绝不移动或删除 app-owned Entity。
- unresolved binding 改为安装失败合同，不作为不可达的 runtime facade failure。
- 保留并验证 platform/devtools feature。
- 将最小 registry/preflight 前移到 Root push。
- 明确移除旧 public authority/report surface 是 pre-MVP 的有意破坏性安全迁移。

按审查建议新增独立 facade contract fixture 文档，并随主规范进入下一轮审查。

## Round 5

三位新的只读审查者分别从对象生命周期、facade fixture 和代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

对象 authority 已分层，但顶层 slot、child slot、frame root 和 quarantine 尚未各自固定唯一 owner、创建时刻、复用与终止条件；fixture 同时缺少 route input、可比较 frame identity 和跨 session fault 的真实注入路径。Facade re-export 与旧公开 authority surface 也需要精确 allowlist。

### 负责人评估

接受并统一修订：

- 顶层 slot 改为 host grant 的一部分，首次 frame/fallback 直接使用；只有 child outlet 走 mount handshake。
- final preflight 覆盖完整 entering output closure，并保证 switch 第 1-4 步全部不可失败。
- quarantine receipt 可终结 foreground settlement，残留移入独立 lease 持续追踪，不阻塞新 host/queue。
- 补充只读 route input、frame identity accessors和真实跨 session anchor 捕获。
- facade journey 与 authority audit 分开在 `ecs`/`test-support` 下运行。
- 移除不兼容的 `ViewBindingKey` alias；确定 pre-MVP breaking migration 和精确 facade allowlist。
- Root push 前置 manifest、dev-dependency、feature 与 consumer target 同步。

第五轮修订后进入 Round 6。

## Round 6

三位新的只读审查者分别从完整 closure、facade 可断言性和代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

规范已能描述主流程，但 preflight 尚未对 preserved/leaving/entering 使用同一 Entity owner 模型，facade 的 allowlist、feature manifest 和 snapshot accessors 也还没有精确到逐 symbol/逐签名。

### 负责人评估

接受并统一修订：

- 定义 `EntityOwner`/`QuarantineOwner`，覆盖 top slot、child slot、frame shell/output 与 residual。
- preflight 输出拆为 `Ready`、`PendingRejected`、`AuthorityLost`，三套 closure 均逐实体 partition。
- HostGrant 的 grant/register/confirm 改为预检后的同一不可失败 exclusive block。
- 补齐 quarantine 前台终结、后台 reaper 和新 epoch 隔离验收。
- 统一 fallback 使用 `TopSlot`。
- public errors 增加 `Display + Error` 合同，fixture 补齐 snapshot、history、trace、cleanup 和 builder accessors。
- 新增逐 feature/module/symbol 的 Facade Export Allowlist。
- 固定完整 Cargo feature 图、direct dev-dependencies 和 compile-pass/fail targets。

第六轮修订后进入 Round 7。

## Round 7

三位新的只读审查者分别从状态矩阵、facade 编译边界和代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

状态语义仍分散在多份文档，缺少单一的 token/closure/epoch/quarantine 转换矩阵；facade allowlist 也尚未成为可机械执行的 symbol × path × prelude × feature 表。

### 负责人评估

接受并统一修订：

- 新增 Authority Transition Matrix，固定 host/mount reject/cancel、closure role 分类、preflight outcome、quarantine closure、epoch recovery 和 request terminal。
- AuthorityLost 后强制终结旧 host lease并通过完整 admission 获得新 host id/epoch/top slot。
- 补齐独立 HostTrace、cleanup/quarantine 与 authority audit 只读 accessors。
- 将 Facade Export Allowlist 重写为逐 symbol 的 module/prelude/feature 矩阵，并增加 required-features test target 表。
- 补齐 `RouteViewKeyError`，收窄 report capability 禁令，保留 driver 必需的一次性 session 类型。
- Facade fixture 补充 route input、可比较 frame identity 和真实跨 session anchor 捕获。

第七轮修订后进入 Round 8。

## Round 8

三位新的只读审查者分别从 authority/cleanup 架构、facade 可编译性与代码事实/跨文档一致性方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

前七轮已经固定主要对象和状态，但少数跨层合同仍停留在概括层：token 失败消费在两份文档中冲突，cleanup receipt 没有不可变 obligation ledger，quarantine root/reaper 没有确定算法，公共 snapshot/audit DTO 与 feature compile-fail 也不足以直接编写无歧义测试。文档权威顺序尚未覆盖新增加的规范附录。

### 负责人评估

接受并统一修订：

- 明确 admission/mount token 的所有失败都通过 sealed terminal transition 消费；ID 仍只在成功 preflight 后分配。
- 按 entering 与 preserved/current parent role 区分 child outlet failure：前者 `PendingRejected`，后者 `AuthorityLost` 且禁止 fallback。
- 增加绑定 request/transaction/epoch/manifest 的 sealed cleanup batch 与逐 obligation ledger，固定部分、重复、stale、零 obligation 和 completion replay 语义。
- 以 removal DAG 选择 maximal scheduled quarantine roots，并增加 reaper 对 exact、missing/reused、detached、protected 和 stale epoch 的完整转换表。
- 补齐 `ActivePresentation`、availability/failure、pending/retention 以及 authority mapping/quarantine/lifecycle audit 的精确只读 accessors。
- 为 ECS crate 明确 `test-support` feature，修正 platform canonical path，并把 module path 纳入 facade allowlist。
- 固定隔离 consumer crate 的生成布局、feature 组合、Cargo 命令和负向 rustc 诊断类别，明确 `required-features` 不能证明 feature-disabled 不可达。
- 重排文档权威顺序：Target 拥有架构不变量，Matrix 拥有生命周期细节，Fixture 拥有签名，Allowlist 拥有公共路径。

不接受扩大运行时能力；新增 API 均为生产只读 snapshot 或 `ecs-test-support` 只读 audit，不开放 Entity mutation、token constructor 或 delayed report capability。

第八轮修订后进入 Round 9。

## Round 9

三位新的只读审查者分别从总体 authority 架构、facade/消费者合同和代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

第八轮已把 transition 与编译 harness 细化，但状态机、DTO owner、只读能力边界、quarantine residual ledger 和 diagnostics 尚未投影成同一个可编译、可观察的 authority 合同。具体表现为 request lookup 跨 crate owner 不闭合、audit 暴露裸 Entity selector、host-loss 稳定态缺少公开枚举、detached frame output 与 root-loss reaper 不闭合，以及 severe diagnostics 没有读取路径。

### 负责人评估

接受并统一修订：

- 将 `RequestLookup` 与 `NavigationRequestSnapshot` 一并归 ECS；core 继续拥有 request identity/status，不产生 core -> ECS 依赖。
- 用不可还原、不可转换为 Bevy `Entity` 的 `AuditEntityId` 替代裸 Entity，保留跨 snapshot 一一对应证明但不泄露 World mutation selector。
- 明确 host loss settlement 后为 `Unavailable(HostLost)`，并同步 enqueue、Matrix、fixture 与 Journey G。
- 将漂移到 mounted boundary 外的 exact `FrameOutput` 定义为 detached quarantine root；root missing/reused/protected 时逐 Entity 收敛 immutable lease set，不能遗忘 surviving owned record。
- 为 router/request snapshot 增加带 severity、request/frame 关联的 diagnostics 读取面。
- 补齐 fixture 实际使用的 core 构造/读取签名，并统一 view failure 与 request failure 使用脱敏 `NavigationFailure`。
- 澄清 back types 始终由 core/root prelude 提供，`platform` 只增加同类型的 `platform::prelude` 次级路径。

部分接受 facade reviewer 的事实意见：旧 facade 整 crate re-export、目标 API/feature/test 尚未实现均是真实基线差距，但 `Current Architecture` 已把它们记录为尚未集成，本阶段本来就是实施前设计审查。因此这些事实不构成目标设计失败，也不能要求在 Plan PASS 前先改 Rust；它们继续作为实施计划的首批迁移任务和最终 factual gate。

第九轮修订后进入 Round 10。

## Round 10

三位新的只读审查者分别从总体 authority、facade/验收和代码事实方向复审，结论均为 `NEEDS REVISION`。

### 共同根因

跨层 owner 与故障稳定态已经闭合，但少量动态事实还没有完整投影到 request 终态、trace、只读拓扑 DTO 和强制 gate：queued back 在执行时可能失去目标，mount resolved 缺少事件，Journey B 无法比较 outlet exact identity，缺少普通 view 的安装失败也没有进入 fixture/验收。

### 负责人评估

接受并统一修订：

- back 在 enqueue 时使用 published snapshot 做快速拒绝，成为 running 时重读权威 history；若目标已消失，以 `Failed(BackTargetUnavailable)` 和零 obligation batch 终结，不渲染 fallback。
- 增加 `HistoryIntentRejected` 与 `MountResolved` trace，并把 Journey A 的有序子序列同步到三阶段 mount handshake。
- 增加按 `parent frame + RouteSlotKey` 唯一的 `AuthorityOutletMapping`，使用 opaque audit ids 跨 leaf replacement 比较 anchor 与 child slot。
- 固定 `RouterInstallErrorKind`，把 route node 缺少 normal view 纳入同步 install consumer gate。
- 负责人补充冻结公开 `RouterScheduleSet` 的精确 `Update` chain，使 host observation、drive、commit、cleanup、reaper 和 snapshot publication 的调度合同可实施。

上述问题均是既有目标的观察/终态投影缺口，不要求推翻统一协调层、ownership 模型或 Phase 3B.1 范围。

第十轮修订后进入 Round 11。

## Round 11

三位新的只读审查者分别从总体 authority、facade/验收和代码事实方向复审；事实审查为 `PASS`，其余两份为 `NEEDS REVISION`。

### 共同根因

Quarantine authority 仍与 current host epoch 和实际 Bevy relationship 边界耦合：live old-epoch lease 会被误判 stale，despawn 被 reparent 到 app parent 的 child 又会经 relationship hook 改写未授权 parent。同时，report disposition、中间 phase、removed-entity、host topology 等强制事实没有完整投影到结构化只读 DTO，旧 facade 负向 gate 也未证明不存在额外导出。

### 负责人评估

接受并统一修订：

- Quarantine 转移时签发独立 lease id/generation/operation sequence；former host epoch 仅作诊断，live lease 可继续 reap自身 immutable set，closed/unknown/replay 才 stale。
- 把 actual inbound parent edge 纳入 preflight/cleanup；只有无 parent、same-plan owned parent 或唯一授权 host-top-slot edge允许删除。未授权 app/other-authority parent 产生 `QuarantinedProtectedParent`，Router 不移除 relationship。
- Reaper 改为以 lease id 排序的确定性 round-robin cursor，永久 lease 不能饿死后续 lease。
- 增加结构化 `ReportDisposition`、phase/terminal trace；用有序 trace 证明同 update 内完成的中间安全态。
- Audit 扩展 host/slot/frame/outlet 闭合拓扑、host equality probe 和 retained entity tombstone，`Removed` 必须由独立 World/registry audit 得出。
- Forbidden inventory 补齐基准提交全部被移除的 core/view/ECS public symbols 与 nested modules；新增 `syn` source-structure completeness gate，禁止 glob re-export 和 underlying crate alias，仍保留隔离 consumer 编译。
- 负责人同步补齐 history entries/cursor、trace slot、host top-slot/trace slot 与 Copy/Hash trait 合同，确保规范 fixture 本身可编译、可断言。

这些修订扩大了 quarantine safety model 与观察面，但不改变 workspace DAG、统一协调层或 MVP 主路径，因此不需要推翻整体决策。

第十一轮修订后进入 Round 12。

## Round 12

三位新的只读审查者分别从总体 authority、facade capability 和代码事实方向复审；事实审查为 `PASS`，另外两份为 `NEEDS REVISION`，并共同指出 generic Bundle writer 是 P0。

### 共同根因

规范仍把 Bevy World mutation 视作可由 component-ID 预检封闭的普通写入；实际上 bundle effect、component hook、observer 和 relationship bookkeeping 都可在同步 mutation 中重入 World。由此 generic writer 可越权，host/mount grant 写后仍可能失效，旧 top slot 跨 epoch reap可能改写 current host；marker 快照和 symbol allowlist 也不足以证明输入隔离与关联 capability 不泄漏。

### 负责人评估

接受并进行局部协议重构，不推翻 Router 总体架构：

- 删除 `FrameContentWriter` 的 generic Bundle/component API；MVP 只保留 hook-free empty-content/handle hierarchy/outlet 操作。BSN 可视内容改为 MVP 后单独设计 data-only sealed primitive，重新安全审查。
- Host/mount grant 改为单调 ID + staging registry + reentrant mutation + post-write validation；成功后才 confirm，失败为 `RejectedAfterMutation`、保留 ID gap并结算所有 staging residual。
- Host settlement 在旧 HostLease 有效时先 detach + validate old top-slot edge，再 transfer quarantine/允许同一 host 获得新 epoch；后台 reaper 不修改 current host relationship。
- 增加 `RouterInputGate` 与 `PreUpdate RouterInputSet`，明确 Router-integrated target dispatch 范围；pointer/keyboard/gamepad 必须用真实 dispatch/counter 验收，任意宿主 raw input bypass 不伪装成 Router 可沙箱化能力。
- Facade gate 扩展到 capability-closed associated methods/fields/variants/traits/conversions；owner-crate `syn` audit 与 consumer compile-fail共同阻止 raw Entity/World/ID、clone session 和 generic writer 泄漏。

第十二轮修订后进入 Round 13。

## Round 13

Facade 与事实审查均为 `PASS`。原架构审查者长时间运行后因模型容量错误未形成结论，负责人未中断也未以多数通过代替，改用另一模型重跑同一职责；替补架构审查结论为 `NEEDS REVISION`。

### 共同根因

派生的 input permission 与 relationship graph integrity 尚未进入 sealed commit/preflight：gate 只在 `PreUpdate` 刷新会在 `Update` commit 后短暂保持 old permissions；writer 的同-session handle 限制也没有阻止 self-edge、duplicate parent 与 cycle，而锁定 Bevy relationship API 不提供环检测。

### 负责人评估

接受并统一修订：

- `FrameContentWriter` 明确为纯内存 `ContentPlan` builder，driver 返回前 World 零变化；`add_child` 在 plan mutation 前执行单 parent、去重、self-edge 和 visited reachability cycle 校验，成功后才由 ECS staging-realize。
- 所有 runtime relationship traversal 使用 exact-Entity visited set；pending cycle 为 `PendingRejected`，preserved/leaving/current cycle 为 `AuthorityLost`，settlement 不沿 cyclic descendants 递归 despawn。
- `RouterInputGate` active/blocked snapshot 加入 final preflight 与 exclusive commit 原子替换；commit 前为 old/old，commit 后同帧为 new/new。`PreUpdate Refresh` 只处理 drift并收窄权限。
- 增加 `ContentWriteErrorKind::RelationshipCycle`、public fixture cycle 注入、同 frame gate ordering 与外部 cycle regression。
- 负责人进一步把 component-id manifest 纳入 registry/cleanup：manifest 外 component 产生 `QuarantinedForeignComponent`，Router 不 remove/despawn它，也不触发宿主 hook。
- 为避免 input test 需要 raw Entity，负责人增加独立于只读 audit 的 bounded `RouterInputProbes`：只以 opaque audit id 驱动生产 gate和 test-only counter，ECS private regression再覆盖真实 target observer。

第十三轮修订后进入 Round 14。

## Round 14

事实审查为 `PASS`；架构与 facade 审查为 `NEEDS REVISION`。

### 共同根因

规范仍把 `InteractionDisabled` 当作可原子切换且被 native UI picker 物理执行的 barrier，但锁定 Bevy 中它是带 observer 的 marker，picker 实际读取 `Pickable + InheritedVisibility`。同时，associated API gate 只封闭少数高风险类型，其他 facade DTO/trait 仍可能通过新增 method/field/constructor/conversion泄露 authority。

### 负责人评估

接受并统一修订：

- Router lifecycle 不再使用 `InteractionDisabled`；每个未来 renderable primitive 预插入 `Pickable`，pending/leaving/quarantine 为 `Pickable::IGNORE`，commit 只 in-place 切换到审计过的 active value，并与 gate snapshot 同步原子替换。
- ECS private integration 必须运行锁定 `UiPickingPlugin`、真实 pointer location/`PointerHits`/target observer，证明 native picker 的 pending/active行为；public probe继续验证 Router gate。
- Closed-world associated catalogue 扩展到第 3 节全部 facade exports，记录 methods、fields、variants、traits、conversions和 derives；owner source 与 committed golden 双向比较，关键 authority deny patterns另有 compile-fail。
- 增加 `facade-associated-api.txt` golden deliverable；Target 新类型以 Fixture 为准，保留 core/view API 由 factual reviewer逐项确认。
- 明确 `ViewBuildReport` 只有 `kind/message` 读取面且只能由 sealed report session 构造，避免在“无 constructor”约束下让 ECS 无法读取 report。

第十四轮修订后进入 Round 15。

## Round 15

Facade 审查为 `PASS`；架构与事实审查均为 `NEEDS REVISION`，共同指出 native picking 所需的锁定 Bevy feature edge 未进入 manifest 目标。架构审查另指出 host `Children` relationship bookkeeping 与“应用组件不变”措辞冲突。

### 共同根因

规范已核对 Bevy runtime 行为，却仍把 dependency feature 和 relationship target hook 当作实现细节；它们实际上决定验收能否编译，以及 app-owned host 的允许变化边界。

### 负责人评估

接受并统一修订：

- ECS 正常 `bevy_ui` dependency 固定 `default-features = false, features = ["bevy_picking"]`；单独 direct `bevy_picking` 不作为证据。
- Feature gate 增加 `cargo tree -e features -i bevy_ui`，必须观察 `bevy_ui feature "bevy_picking"`，并与 manifest/factual review 双重核对。
- HostGrant/HostLease 的窄授权明确允许锁定 Bevy 因 exact top-slot `ChildOf` 自动维护 host `Children` membership；Router 不直接写 `Children`，不改变其他 child identity/relative order，也不改写任何其他应用组件。
- Journey A/G 与回归据此区分授权 relationship metadata 与受保护 host state。

第十五轮修订后进入 Round 16。

## Round 16

架构审查为 `PASS`；facade 与事实审查为 `NEEDS REVISION`。两份修订意见分别指出隔离 consumer 与根 workspace 的依赖解析上下文不一致、default feature 合同缺少真实启用场景，以及 outlet writer failure 与 mount-token rejection 被混为同一失败类别。事实审查同时确认基准提交没有 tracked `Cargo.lock`，仅使用可移动 Bevy git branch，因此本地解析 revision 不能称为可复现锁定合同。

### 共同根因

验收工件没有完整保存其所声称验证的解析上下文与生命周期角色：根 workspace、隔离 consumer、默认 feature 和 mount handshake 各自丢失了一部分关键前提，导致错误实现仍可能通过表面 gate。

### 负责人评估

意见成立，但不直接照搬其分类；核对现有 writer 协议后确认 duplicate outlet 与 foreign-session handle 在 `ContentPlan` 阶段已经被拒绝，此时不应伪造 mount token。统一修订如下：

- 全部 Bevy dependencies 固定到 exact revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`；Root push 生成、审查并提交 workspace `Cargo.lock`，主 workspace gates 使用 `--locked`。
- 隔离 consumer 各自生成并审计 lock，随后以 `--locked --offline` 执行 metadata/check；所有 Bevy git packages 必须解析到同一 exact revision。
- 增加真实 default-enabled consumer 与 `default_facade_features` target，精确断言 default 为 `['view']`，并通过 metadata/tree 证明默认图包含 facade/core/view且不引入 Bevy runtime。
- duplicate/foreign-session/cycle 保持 plan-level `ApplyFailed`，明确无 mount trace；accepted parent plan 后的 missing/invalid outlet 才以 `MountRejected(MissingOutlet|InvalidOutlet)` 成为 `PendingRejected`。
- 增加 crate-private preserved/current outlet corruption regression；该路径必须以 `MountRejected(ParentAuthorityLost)` 进入 authority settlement，禁止 fallback。
- facade trace 增加结构化 `MountRejectionKind`，使 public journey 与私有 authority regression 能机械区分 pre-grant rejection 和 `RejectedAfterMutation`。

这些修订不改变 workspace DAG、统一协调 authority 或 MVP 垂直切片，只补齐可复现性与失败角色的验证闭包。第十六轮修订后进入 Round 17。

## Round 17

架构、facade 与替补事实审查均给出 `NEEDS REVISION`。原事实审查者因模型容量错误退出，负责人没有把错误当作结论，也没有中断其他长时间运行的审查，而是使用不同模型重跑同一事实职责。

### 共同根因

前十六轮已经把 mutation authority 封闭得很严密，但“消费者实际能完成什么”和“test-support 如何机械证明”没有同步闭包：MVP writer只能产生 empty Entity，allowed associated API 没有递归封闭签名类型图，实时 audit Resource/同帧观察时序也没有成为精确公共合同。继续局部补句会保留一个安全但不可用的伪 MVP。

### 负责人评估

接受该宏观根因，并推翻“生产 `ContentPrimitive` 延后到 MVP PASS 后”的旧决定：MVP 必须先具备至少一种公开、可视、受限的 production primitive。具体处置如下：

- 在 MVP 内新增唯一 data-only `SolidRect`：仅含非零 `u16` 像素尺寸与四 channel `ContentColor`。公开 driver只能把它写入 `ContentPlan`；ECS staging-register后解释为锁定 Bevy `Node + BackgroundColor`，并预施 `Visibility::Hidden + Pickable::IGNORE`。required-component closure进入 manifest，写后复验仍覆盖宿主 observer drift。
- Journey A 与 authority/private physical regressions必须证明公开 driver产出的真实可视 Node提交成功；empty Entity、snapshot或 test-only node不能替代 Facade reachability/Demonstrated。
- Facade golden新增公开签名类型图闭包。`BranchDiffRoute` 随 `BranchDiff` 进入 allowlist；依赖已废止旧 build/apply graph 的 `FakeViewRenderer*` 与 facade `view-test-support` 移除，owner crate内部测试不受影响。
- 保留低层 `with_error_boundary`，但高层安装对已有 normal view却缺 error view的 node返回 `MissingErrorView`；验证顺序固定。
- Compile-fail catalogue按 case接受真实 rustc类别，补入 conversion/trait 的 `E0277` 以及 private field/constructor类别，不能用任意编译错误冒充。
- `RouterAuthorityAudits` 与 `RouterInputProbes` 明确为 test-support Bevy Resources；audit在 Drive/Commit/Cleanup/Reaper mutation点同步更新。测试系统按公开 `RouterScheduleSet` 插入相邻 set之间，机械观察 pending、active、leaving与quarantine IDs。
- 修正 Matrix 遗留的 `interaction-disabled` 措辞，统一为 hidden、`Pickable::IGNORE`、gate deny和 focus release。
- Fixture补齐 `ViewDriverRegistrationErrorKind::DuplicateKey` 的唯一错误形状，并把 production primitive、content mapping与 Resource traits纳入 closed-world合同。

不接受事实审查把“目标 gate 尚未在重构前基线实现”视为设计文档失败。`Current Architecture` 已明确记录当前 facade feature、manifest、lock、test target和 picking edge尚未迁移；Target/Journeys中的命令本来就是实施后的验收门槛，不声称现在已经通过。该报告核对出的当前事实仍保留为 Root push 的实施输入和最终 factual gate，但不要求在 Plan PASS 前先修改 Rust。

第十七轮修订扩大了 MVP 的生产能力与验收面，但不改变 workspace DAG、统一 RouterEngine authority 或纵向切片顺序。修订后进入 Round 18。

## Round 18

架构、facade 与替补事实审查均为 `NEEDS REVISION`。原事实审查者再次因模型容量退出，负责人保持另外两路运行并改用第三种模型完成事实职责，没有缩短核验范围。

### 共同根因

第十七轮把 `SolidRect` 前移到MVP，却只封闭了primitive数据和mutation authority，没有同步封闭锁定Bevy的**物理UI合同**：host/structural Entity并非全Node ancestry，required-component传递闭包漏项，picking插件链重复且不完整。与此同时，associated API golden仍是未来文件，post-commit/cleanup前窗口和可能运行0项的测试命令也没有唯一证据。

### 负责人评估

全部核心意见成立，并按共同根因统一修订：

- app-owned host必须是 `Node`，且无parent或到UI root的每个实际ancestor都有 `Node`；non-Node、cycle、ghost-only topology以 `InvalidHostUiTopology` 拒绝，Router不补写应用组件。
- top slot、authority root、content target、outlet anchor、child slot与所有primitive形成全Node直接ancestry；Router-owned结构Node透明、始终 `Pickable::IGNORE`，不启用/依赖experimental `ghost_nodes`。
- locked Bevy递归manifest补入 `UiGlobalTransform`，并固定完整 closure；实施事实测试从注册后的实际component set递归核对，避免手抄清单再次漂移。
- 物理回归用exact-feature `bevy_internal::DefaultPlugins` 一次安装input/transform/window/camera+visibility/`UiPlugin`/`DefaultPickingPlugins`，显式创建 `PrimaryWindow + Camera2d`；不再重复添加`UiPickingPlugin`。
- 新建并验收1194行 `docs/facade-associated-api-golden.txt`，覆盖当前allowlist的125个唯一类型/trait和全部canonical paths；新Target API以Fixture为准，保留API已从基准提取，签名类型图未闭包数为0。README将其纳入权威链，实现测试直接读取该文件。
- 补齐 `RouteNode`、`RouteParamsFingerprint`、`RouteSearchFingerprint`、`RouteSegmentReuseKey` 等已由允许签名引用的safe DTO，associated graph不再留给Root push临场扩大/删除。
- 隔离consumer的default声明改由TOML parser直接读取facade manifest；`cargo tree`只验证resolved graph，删除无效的dependency `metadata --no-deps`推断。
- host/authority loss按`CommitPublished`分界：commit前request失败且history不推进；commit后、cleanup前发生loss则ledger转交authority settlement，active清空、已提交history不回滚、request最终成功并带severe post-commit diagnostic。
- plan-level writer failure的driver-return World/registry零变化归入crate-private fingerprint regression；public Journey只证明trace/old presentation/history，不声称拥有不存在的中间观察点。
- 新增固定authority regression函数catalogue及专用test target；catalogue与源码test names双向比较，workspace执行行为。0 tests、全部filtered out或缺失target一律fail closed。
- 最终gate要求所有必建artifact先证明tracked，并增加`git diff --cached --check`与显式全文件空白扫描，避免用tracked-only diff证明未跟踪文件。

事实审查再次列出的“当前目标test/feature/lock尚未实施”仍属于 `Current Architecture` 已声明的基线差距，不作为Plan失败；其关于插件链、required closure、零测试和未跟踪diff的事实意见已按上文接受。

该轮需要重审物理UI承载层，但仍无需推翻统一RouterEngine、sealed authority协议或workspace DAG。修订后进入Round 19。

## Round 19

架构、facade 与事实审查均为 `NEEDS REVISION`。事实审查再次列出了多项目标合同尚未进入当前 Rust 基线的差距；负责人只把其中可由基线源码直接证明的新风险作为本轮文档修订，其余继续保留为 Root push 与最终事实验收输入，不把“实施尚未开始”误判为规划失败。

### 共同根因

第十八轮已经锁定物理 UI 承载层，但仍有少量逻辑角色没有投影成唯一、可构造、可编译验证的实体合同：`spawn_empty` 没有明确保证 Node ancestry，report 与 sealed session 跨 crate 分属导致合法构造路径断裂，golden 把自动 trait 当作显式源码 impl，测试目录也仍可能以“模块未接入”或“零测试”伪装通过。fragment-only 导航与 active picking值同样缺少唯一可执行语义。

### 负责人评估

意见成立，但不逐条照搬；按“物理实现、同 crate authority、证据类型”三个共同边界统一修订：

- `spawn_empty` 仅表示没有业务 primitive，实际必须创建透明的 Router-owned结构 `Node`，纳入完整manifest、ownership与audit，不能产生 non-Node中断。
- `EcsViewReportSession`、`ViewBuildReport` 与 `ViewBuildReportKind` 统一归 ECS crate所有；session仍只能 crate-private构造，driver只能通过 sealed session提交 report，消除跨 crate 私有构造死路。
- 删除 golden 的 unresolved-closure 哨兵。任何未解析签名类型、cfg分支或闭包边都 fail closed，不能生成或接受 golden成功结果。
- active `SolidRect` 的 picking值固定为 `Pickable { should_block_lower: true, is_hoverable: true }`；所有结构节点始终 `Pickable::IGNORE`，其他生命周期继续使用忽略值。
- fragment-only 导航分成两个确定分支：相同 fragment是完全零影响；fragment变化只 remount最深 normal matched frame，保留 frame ID、推进generation并保留祖先/outlet，成功commit后按原始动作更新history。
- authority regression catalogue不仅比较名称，还必须确认精确 `#[cfg(test)] mod authority_regression_tests;` 接线、直接 `#[test]`、无额外cfg/ignore，并比较 `--list` 与实际执行的非零通过数。
- associated API golden拆分为源码可枚举记录与 `compile-assert`。显式 impl/derive等由 `syn` 双向比较；`Send + Sync + 'static` 等自动性质由隔离 consumer泛型断言逐项编译证明，不再伪造成源码枚举结果。
- 物理回归复用 `DefaultPlugins` 创建的唯一 `PrimaryWindow` 并原位配置，断言窗口数量始终为1；测试只额外创建 `Camera2d`，不再重复生成窗口。
- `Current Architecture` 新增当前递归 cleanup 的事实风险：公开可伪造 root marker、缺少 foreign descendant与visited/cycle防护，可能越权删除app-owned实体或无法可靠收敛。该风险进入实现优先级，但不要求在 Plan PASS 前修改 Rust。

这些修订没有暴露新的系统级矛盾，不需要推翻 workspace DAG、统一 RouterEngine、sealed authority 或 MVP 垂直切片；文档进入 Round 20 复审。

## Round 20

架构、facade 与事实审查均为 `NEEDS REVISION`。三路独立报告都明确区分了目标尚未实施与规划本身不可证明的问题，也一致认为无需推翻 workspace DAG、统一 RouterEngine、sealed authority、feature 图或垂直切片。

### 共同根因

少数新增规范承诺仍没有完整投影到“可构造 facade 形状、只读观察 DTO、consumer/私有行为回归、当前环境可执行命令”四层证据链。最明显的是 `spawn_empty` 已被定义为成功的结构 Node，却只有失败plan覆盖且没有可枚举的结构output mapping；同类缺口还出现在plugin显式预装、安装错误优先级和matcher等价path的view-input delta。

### 负责人评估

所有发现成立，但不接受通过删除 `spawn_empty` 或暴露raw Entity来换取可测试性。统一修订如下：

- 保留 `spawn_empty` 作为受限writer的结构分组/outlet能力。成功fixture现在同时生成structural container与`SolidRect` child；新增仅在`ecs-test-support`开放的`AuthorityUiNodeMapping`/`AuthorityUiNodeRole`，用opaque entity/actual parent ID、角色、frame/local index和只读Node状态完整投影top slot、child slot、shell、content target与两类output。
- Host audit新增`ui_topology_valid`，结构mapping新增manifest/transparent/visibility/pickable证据；removed实体仍只由state/tombstone证明。这样公共audit可验证每个直接edge，locked-Bevy私有回归再证明真实component/layout/picking，不扩大World mutation authority。
- `BevyUiRouterPlugin` 固定实现`Default`；隔离consumer分别覆盖自动安装和`app.add_plugins(BevyUiRouterPlugin::default())`预装后调用extension，要求install marker复用且systems/resources只有一份。
- 安装错误consumer新增同一node同时缺normal/error view的case，机械固定`MissingRouteView`优先级。
- `RouteLocation` 在matcher/history/effect前把slash-only path差异规范化为单前导斜杠、无空段、非根无尾随斜杠的canonical path；新增Journey F3证明`/items//1/`不会发布新raw history而保留旧view input。
- 最终authority脚本gate不再假设系统安装`pwsh`可执行文件；PowerShell命令块直接以当前解释器调用`& .\scripts\verify-authority-regressions.ps1`。
- Authority regression catalogue新增成功结构output的locked manifest与topology回归。Facade allowlist扩展为130条canonical paths、127个唯一符号；associated golden扩展为1223行、1214条API记录，新增类型仍只在test-support feature可达。

这些修改补齐的是同一证据闭包，不改变生产facade authority边界。文档进入 Round 21 复审。

## Round 21

架构、facade与事实审查均为`NEEDS REVISION`。架构审查发现commit/admission快照后的持续World mutation尚未闭环；facade审查发现opaque struct形状、nested Cargo环境与audit ID状态域仍有机械歧义；事实审查确认锁定Bevy的公开路径为`bevy_ecs::resource::Resource`且`Resource: Component`，此前`system::Resource`与“Resource only”属于事实错误。

### 共同根因与粒度评估

真正的共同根因仍是“快照或声明没有覆盖后续可重入事实”，但审查流程也已出现证据合同递归膨胀：为了证明内部结构不变量而增加公开test-support DTO，随后又继续审查DTO自身。负责人接受用户对粒度的质疑，明确收敛标准：后续只有会导致ownership安全错误、生命周期没有唯一实现、公共facade capability泄漏或MVP旅程无法证明的问题阻塞Plan PASS；纯实现组织与脚本细节下沉实施计划，内部World不变量优先由ECS私有回归证明，不再自动扩展公开API。

### 负责人评估

- cleanup batch改为“immutable obligation identity + versioned cleanup attempt”：obligation只冻结exact owned duty，不冻结物理disposition；每个attempt按最新World证据重分类，hook/observer后复验，漂移则supersede旧attempt。已结算receipt不重复，未结算residual安全转入新attempt或terminal quarantine lease。
- Host admission新增私有`HostCandidateRevision`闩锁。任何`RejectedAfterMutation`只允许同revision失败一次；registration实例、host generation或topology fingerprint未变化时不再分配ID/lease，变化后才允许一次新admission。统一稳定错误名为`AuthorityEstablishmentFailed`。
- 增加跨`CommitPresentation -> SettleCleanup`的foreign/protected drift、cleanup observer重入和持久admission observer回归，直接验证app-owned state不被触碰与前景终态收敛。
- 修正五个Resource类型的locked-Bevy trait路径，并显式记录mandatory `Component` supertrait；golden/source gate不再伪造不存在的`system::Resource`。
- Facade source gate拒绝除`BevyUiRouterPlugin`外的public unit/zero-field struct，并为零public-field struct生成直接value/empty-literal构造负例；诊断类别补入`E0423/E0063`。所有nested Cargo命令统一清空`rustc-wrapper`。
- `entity_state`收窄并重命名为`owned_entity_state`，只定义Router-owned ID状态；host/foreign observed IDs不扩充公共枚举，按来源返回`Unknown`。
- 推翻第20轮“为每个结构Node增加公开mapping”的局部决策，删除`AuthorityUiNodeMapping/Role`与host topology布尔值。成功`spawn_empty`仍进入fixture，但真实Node/edge/manifest只由固定ECS私有locked-Bevy回归证明。

收敛后facade allowlist为128条canonical path、125个唯一符号；associated golden为1200行、1191条API记录。上位workspace DAG、统一RouterEngine、sealed authority、feature图与MVP切片不变。文档进入Round 22复审。

## Round 22

三路阻塞级审查均为`NEEDS REVISION`，但没有发现新的ownership安全、authority泄漏或上位架构问题。架构/facade报告集中于已公开状态的唯一归属与request provenance；事实报告只发现物理回归代码、direct dev-dependency和feature-tree gate分属不同Cargo package。

### 共同根因

内部identity/lifecycle已闭合，但少量公共状态词汇仍未绑定唯一request/terminal来源，个别验收证据也没有与实际test package同属一个Cargo解析图。这是公共观察投影与证据所有权的局部缺口，不需要继续扩大架构或test-support类型。

### 负责人评估

- `NavigationRequestSnapshot`新增`requested/matched`只读accessor，永久绑定该request的canonical location与match结果；`RouterSnapshot::requested/matched/pending_phase/running_request`只汇总当前running request，无running时全部为None，不能混入active、queued或last-terminal状态。
- `RouterHandle`与`NavigationRequestId`绑定不可伪造的World/router provenance。跨router、跨App、从未签发、future sequence或错误配对均为`Unknown`；只有同domain/router已签发后淘汰的ID为`NotRetained`，绝不只按局部sequence命中。
- recovery成功提交error presentation时request保留original failure kind；recovery自身也失败时唯一终态为`Failed(RecoveryFailed)`，两层cause进入trace/diagnostics。
- `RegistryInvariantViolation`固定为registry self-audit发现内部duplicate/impossible/seal contradiction后的永久fail-closed状态；settlement只触碰仍可证明的exact owned records，旧router不自动re-admit，应用安装新router instance。
- ECS安装读取route binding通过core-to-ECS内部compiled manifest/query协议，不给facade node/options增加getter。早期切片未完成细分cleanup前必须inert、fail closed，该实现组织进入详细计划。
- exact-rev`bevy_internal`与authority catalogue使用的`syn`归`bevy_ui_router_ecs [dev-dependencies]`；facade completeness的`syn`和自身integration Bevy依赖归facade package。`bevy_internal` tree gate改查`bevy_ui_router_ecs --all-targets`，与实际crate-private回归属于同一解析图。
- 非阻塞的Journey F2/F3字符串简写改成真实`RouteLocation` API；fixture中落入Rust fence的规范正文改成合法注释。隔离consumer的case-specific direct Bevy依赖细节下沉实施计划，不再升级公共合同。

当前facade allowlist仍为128条canonical path、125个唯一符号；associated golden为1202行、1193条API记录。文档进入Round 23最终阻塞级复审。

## Round 23

初始架构与facade审查者均因模型容量错误退出，负责人没有把错误当作结论，也没有中断仍在运行的事实审查，而是分别以不同模型补跑同一职责。事实审查直接`PASS`；两位替补最初共同指出高权威Fixture遗漏了golden已冻结、Journey F2实际调用的`RouteLocation::with_search/with_fragment/search/fragment`。

### 负责人评估与修订

该意见是唯一Plan阻塞，根因只是Fixture与golden/journey同步遗漏，不涉及API新增或架构决策。Fixture补齐四个既有method，并补明示例已使用的`RouteSlotKey: Default`。两位替补基于修订后的同一快照再次复核，均给出`PASS`；事实审查原`PASS`保持有效。

### 最终结论

- ownership安全：`PASS`。
- lifecycle唯一实现：`PASS`。
- facade capability封闭：`PASS`。
- MVP journeys与事实证据可证明性：`PASS`。
- 上位workspace DAG、统一RouterEngine、sealed authority、feature图与垂直切片：无需推翻。

第23轮三项职责全部通过。当前allowlist为128条canonical path、125个唯一符号；associated golden为1202行、1193条API记录。按既定流程，下一步只做独立的结构整理/整体重写必要性评估；评估前不提交、不进入Rust实现。

## 结构重写评估

两名独立评估者分别从整体架构/信息层级与后续实施可执行性审查当前权威链，结论均为`NO REWRITE`。

负责人接受该结论：README已定义单向权威顺序，Target、Matrix、Fixture、Allowlist、Golden、Journeys各自拥有不同合同面；跨文档重复主要是架构、公共调用和验收之间的必要投影，不是历史补丁堆积。整体重写会重新打开已通过的lifecycle/facade合同，增加golden与表格schema漂移风险，收益不足。

提交前只做非重写整理：README增加按问题查找索引，Target修正两处过期章节引用。Review Log保持历史证据原貌，Golden保持machine-readable格式，不拆分、不改写。至此Phase 3B.1达到`Plan PASS`；下一步是提交本轮规划文档，提交后暂停，不立即编写详细实施计划。

## Nested Push Task 9 completion

### 提交范围

- Contract origin：`1ccbcd7`
- Execution-plan commit：`eab1267`
- Implementation range：`eab1267..702e36e`
- Task 8 public authority：`5e8869c`
- Completion evidence fix：`702e36e`

### 验证事实

- Focused：Core 301、View 15、ECS 273、public journeys 5、authority audit 2、allowlist 46、compile-fail 40
- Authority：catalogue 23、registered 23、passed 23
- Workspace：nextest 659/659、0 skipped，doctest 28
- Format/static：fmt、workspace check、diff check green

### 独立复审

- Core lifecycle protocol：PASS
- Specification coverage：初审 findings 修正后 PASS
- Public facade/test evidence：初审 findings 修正后 PASS
- ECS ownership：实现 PASS，证据缺口由独立复审闭合
- Final lifecycle/physical review：PASS

### 阶段结论

- Nested Push：PASS
- Authority catalogue：Root 20 + Nested 3 = 23
- 仍未完成：Journey B/common-prefix Reconciliation、History replace/back、Recovery、Authority closure、MVP gate

## Reconciliation Task 8 completion

### 提交与实现 checkpoints

- Reconciliation contract start / implementation-plan commit：`0201d49`
- Implementation/evidence range：`228c5bf..5806c59`
- Task 1：`228c5bf`
- Task 2：`c627a9e`
- Task 3：`6f8e734`
- Task 4：`9415d03`，review fix `4f4e4dc`
- Task 5：`ddce26d`
- Task 6 plan gate：`9d7d2f0`，implementation `e5a26e6`
- Task 7 plan/public gate：`a694acb`，cleanup guardrail `9307d2f`
- Root fallback plans：`10dc5f4..81d2657`
- Task 7 prerequisite：`2e5cb3d`
- Task 7 public evidence：`5806c59`

Root Push 与 Nested Push 继续保持闭合，Reconciliation Tasks 1–8 已闭合

### Task 8 已执行门禁

以下结果全部来自精确干净 `5806c59` 树

- Core complete target：286 unit tests、28 doctests
- View complete target：8 unit、6 dependency-boundary、1 public API
- ECS all-features complete target：401 unit tests 与全部 integration targets
- public navigation journeys：7/7
- public authority audit：3/3
- facade export allowlist：46/46
- facade authority compile-fail：40/40
- authority catalogue：23/23/23
- workspace fmt 与 all-target/all-feature check 已通过
- workspace nextest：838/838，0 skipped
- workspace all-feature doctest：28/28
- `git diff --check` 已通过，tracked tree 保持干净并指向 `5806c59`

### 当前事实边界

- Core 仍是唯一逻辑 lifecycle publisher，独占 sealed logical frame/slot roles、request ordering、逻辑 router/host/slot/frame/request/transaction/task IDs、与 exact frame identity 绑定的 generation 及 terminal ordering；裸 generation 不要求跨 frame 全局唯一，ordinary new frame 使用 initial generation，fragment remount 对同一 active frame 使用 `checked_next`
- Core prepared commit capability 发布 logical active、push history 与 commit trace；matching cleanup/authority-settlement completion evidence 完成后由 Core 独占发布 request terminal，pre-mutation logical failure 可在 Core 内直接 terminalize。ECS 永不发布或重建 logical active/history/request terminal
- ECS-owned `ViewBuildReport` 与 `EcsViewReportSession` 是 facade-reachable public types；前者是只读 kind/message DTO，只有 ECS-issued one-shot session terminal method 可以构造，raw report identity/fields、direct constructors 与 session constructor/seal 保持 private
- 与 public view-driver report 分离，ECS exact physical `FrozenCleanupManifest`、`CleanupLedger`、`CleanupReceipt` 与对应 exact Entity facts 保持 ECS-crate-private。`CleanupManifest`、`CleanupBatch`、`CleanupCompletion` 及相关 integration methods 是 Core `#[doc(hidden)] pub mod integration` 内的 `pub` items；部分 individual methods 可另带 item-level `#[doc(hidden)]`，不得泛化为每个 wrapper/method 都带该 attribute。Facade 不 re-export 该 integration module 或这些 items；它们绑定 opaque Core batch/completion identity 与 metadata。ECS 独占 exact Entity validation、manifest、ownership seal、cleanup ledger domain、attempt revision、receipt、physical splice、cleanup 与最小 pre-commit authority settlement；Core 不重建或认证 ECS exact physical owner/manifest/ledger/receipt
- Facade 有意暴露 renderer-neutral `RouteViewRegistry`，`RouterInputGate` 与 test-support host mapping 等有限 API 接受 app-side Bevy Entity；封闭边界是 `RouterEngine`、ECS physical `RouteEntityRegistry`、router-owned raw Entity/World/registry conversion/accessor 与 authority/mutation constructor
- Journey B navigation 只经推荐 facade `push` 与真实 `App::update`；稳定 consumer 事实由 snapshot/trace/opaque audit 证明，commit-boundary interactive-leaf cardinality 另由 test-local、只读、非权威的 `World + RouterInputGate` sampler 验证
- Journey B、nested fragment、equivalent physical no-op、exact physical cleanup 与 scoped pre-commit authority-loss settlement 已有实现、集成与相应公开/私有证据；equivalent 为 zero entering/leaving/effect 与 zero physical mutation，但正常发布 request/push history
- authority settlement 开始时 ECS active/pending/host owner 单次移动进 cleanup ledger且 ECS active 清空；ledger 在 receipts 未完成时唯一持有 physical residual，completion 消费 ledger owner后 Core 清空 logical active，terminal state 无 active/lifecycle owner
- isolation、quarantine 与 receipt mutation 仅落在 exact Router-owned authorized target；foreign descendant/component、protected 或 other-authority parent 及其 component/relationship 全部零写，protected parent 本身不接收 quarantine receipt 或 mutation
- `replace`/`back`、完整 History、完整 Recovery、Authority closure、`RouterInputProbes` 与 MVP 仍未完成

### 最终独立复审

- Specification coverage and pending-slice honesty：PASS，no Critical/Important/Minor
- Core lifecycle、single publisher、logical IDs/generation 与 terminal ordering：PASS after review fixes，no remaining Critical/Important/Minor
- ECS ownership、slot lifecycle、physical preflight、cleanup 与 authority settlement：PASS after review fixes through draft5，no remaining Critical/Important/Minor
- Public facade、Journey B、fragment 与 audit evidence：PASS after review fixes，no remaining Critical/Important/Minor

### 阶段结论

- Task 8：complete
- Reconciliation：PASS
- 所有 implementation verification 继续精确归因于 clean `5806c59`
- 完整 History、Recovery、Authority closure、`RouterInputProbes` 与 MVP 仍未完成，未记录 MVP PASS
