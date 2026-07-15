# Current Architecture

**文档类型**: 当前事实
**状态**: Root Push Tasks 1-8 已闭合，准备进入 Task 9 原子切换
**as_of_commit**: `8ea9c25440e0b6d8d85f947a96173bb6bab6d8a2`
**事实来源**: workspace manifest、公共导出、实现代码、测试代码与已记录验证；历史计划不作为事实来源

`Cargo.lock` 已跟踪，Bevy dependencies 固定到 exact revision
`61f78f75684e6b0bc28a13a5933f8f55ca653fa7`；Root Push 后续门禁使用
`--locked`。

## 1. 当前定位

项目已经从单 crate 占位工程发展为包含 core、view、ECS、BSN、platform 和 facade 的 Cargo workspace

Tasks 1-8 已形成 Core engine/cleanup、View registry/effect、ECS content/driver、plugin/DTO、host candidate、exact Entity registry 与 inert staging 等私有地基，但这些能力还没有被一条可由 facade 公共 API 驱动的真实导航路径统一起来

因此，当前项目仍是“生命周期安全原型”，不是已经可用的 MVP Router

已有测试主要证明局部类型与低层协议，不能证明 `push`、`replace`、`back` 已经在 Bevy `App` 中闭环

## 2. Workspace 边界

```text
bevy_ui_router_core       Bevy-free 路由算法和生命周期合同
        ^
        |
bevy_ui_router_view       renderer-neutral view 协议与 fake renderer
        ^
        |
bevy_ui_router_ecs        Bevy entity、World、层级、输入和 focus 集成
        ^
        |
bevy_ui_router_bsn        预留的 BSN adapter crate

bevy_ui_router_platform   router-neutral back 类型的薄封装
bevy_ui_router            feature-gated facade 与 re-export
```

这一分层方向正确，应在 Phase 3B.1 中保留。需要重构的是跨层协调、身份映射和公共入口，不是推翻 workspace DAG。

## 3. 各层现状

### 3.1 Core

已经实现：

- builder-first `RouteTree`、静态 route id、path/index/pathless pattern 与声明顺序。
- `RouteMatcher`、参数提取、稳定 fingerprint 和匹配冲突处理。
- `BranchDiff`、common-prefix 计算和 route segment reuse key。
- frame identity 携带 generation，stale/cleanup 会校验该字段；进入/离开规划与 transaction apply token 已实现。
- `MemoryHistory` 的独立 push、replace、back 数据结构。
- diagnostics、error boundary key、cleanup plan 等局部数据合同。
- canonical `RouteLocation`、`RouteViewKey` 与 route view binding 数据。
- request/presentation/trace DTO、完整 root-task identity 与 sealed host admission 基础。
- Core-private、断开的 `RouterEngine`：内部匹配、单运行请求队列、首次与重复单 root push、generation advancement、sealed publication、私有 history intent 和 64 条 terminal record 保留。
- cleanup 只掌握 ordered logical roots、batch identity、opaque ECS digest/count、sealed token 与 aggregate completion；Entity manifest、attempt、ledger、receipt 和 disposition 不在 Core。

尚未集成：

- 现有生产调用仍走 `RouterRuntime`/transaction；该路径只持有 router/host/单个 slot、allocator、active 和 pending，并直接接受调用者提供的 `MatchedBranch`/`BranchDiff`。
- 新 `RouterEngine` 仍未被 plugin、system、ECS bridge 或 facade 调用。旧路径在 Task 9 原子切换前仍是唯一集成 publisher。
- 一个 runtime scope 的所有 frame 使用同一个 `RouteSlotId`；`RouteSlotKey` 只记录逻辑名称，尚未形成真实父 frame outlet 身份。
- 新私有 engine 已覆盖 root generation advancement/remount，但 live ECS/facade 路径尚未接入。
- Core binding、View registry/effect 与 ECS content driver 地基均已存在，但尚未组成 engine 驱动的真实解析与 realization 链

### 3.2 View

已经实现：

- `ViewBindingKey`、`RouteViewDescriptor`、build/apply outcome、partial output 和单次 report session。
- fake renderer 及 authority surface 的正向、负向合同测试。
- authority-free `ViewDriverKey`、`RouteViewRegistry` 与 `ViewBuildEffect` 根导出
- effect 构造与 request identity 保持私有，registry descriptor、insert 与 get 也保持私有

尚未集成：

- registry 尚无生产填充与解析入口，也没有 engine 驱动的 effect dispatch
- legacy contract、fake renderer 与 ECS apply bridge 仍保留，尚未迁移到统一 effect/report 路径

### 3.3 ECS

已经实现：

- `RouteHost`、`RouteSlot`、`RouteFrameRoot` 及 lifecycle/interaction 快照。
- frame apply session、inert prepare、activation、failure/stale cleanup、focus 与输入隔离辅助逻辑。
- 对 router、host、slot、route、frame、generation 的多字段校验，以及稳定排序。
- data-only `ContentPlan`、`FrameContentWriter`、`EcsViewDriver` 与一次性 report session 私有协议
- `RouterConfig`、opaque `RouterHandle`、host registration、navigation/snapshot/error DTO 与 schedule/input labels
- host candidate incarnation/revision/classification、exact Entity registry、ownership seal、current claim 与 quarantine lease 私有地基
- register-before-mutate 的 inert provisional top-slot staging 与写后复验

尚未集成或存在语义缺口：

- `RouterEcsState` 与 legacy apply/cleanup 仍是唯一集成路径，新 registry/content/host staging 尚未接入生产 authority
- `BevyUiRouterPlugin` 已幂等初始化新旧资源并配置 schedule set，但没有注册 lifecycle systems
- `RouterNavigation` 尚无公开导航行为，router installation、engine store 与 snapshot projection 也未实现
- provisional staging 尚未绑定真实 Core host grant，也未执行生产 confirmation 与失败 settlement
- 物理 realization、完整 preflight、同帧 presentation/input 切换与 ECS cleanup ledger 尚未实现
- legacy 路径仍先提交 Core、再逐个激活 ECS root，旧 marker cleanup 也未使用 exact registry receipt
- legacy root marker 可公开构造且 cleanup 会递归 despawn，仍是 Task 9 删除旧 authority 前的基线风险

### 3.4 BSN、Platform 与 Facade

- `bevy_ui_router_bsn` 只有空 prelude，尚无 adapter 实现。
- `bevy_ui_router_platform` 只 re-export back 相关 core 类型。
- facade 仍按 feature 把 core/view/ECS/BSN/platform 作为整 crate alias re-export，因此旧 authority 与 Tasks 5-8 新 DTO 都只能通过低层 owner-crate 路径到达
- facade prelude 尚无 install、register-host、push 与 snapshot 的完整公共流程，Task 9 仍需用 curated wrapper/allowlist 取代整 crate alias

## 4. 能力账本

| 能力 | Implemented | Integrated | 低层公共可达 | 推荐 facade 可达 | Demonstrated |
| --- | --- | --- | --- | --- | --- |
| route tree 与 matcher | 是 | 否 | 是 | 否 | 否 |
| branch diff 与 common-prefix | 是 | 低层集成 | 是 | 否 | 否 |
| frame identity/generation/stale | 私有新 engine 已实现 advancement；旧 live path 未切换 | 局部集成 | 间接 | 否 | 否 |
| inert/focus/input cleanup | 是 | 局部集成 | feature-gated 低层可达 | 否 | 否 |
| route-view binding | Core binding 与 View registry/effect 已实现 | 私有断开 | 低层 DTO 可达 | 否 | 否 |
| content plan/driver | 是 | 私有断开 | 否 | 否 | 否 |
| host candidate/inert staging | 是 | 私有断开 | 否 | 否 | 否 |
| exact ECS registry/claim | 是 | 私有断开 | 否 | 否 | 否 |
| 真实嵌套 mount | 否 | 否 | 否 | 否 | 否 |
| push/replace/back | 私有 engine 支持 root push；replace/back 仅 history 算法 | 否 | history 独立可达 | 否 | 否 |
| plugin 自动驱动 | 资源与 schedule set 地基存在 | 无 lifecycle systems | plugin 可添加 | 否 | 否 |
| diagnostics 闭环 | 数据类型存在 | 否 | 局部可读 | 否 | 否 |
| BSN immediate adapter | 否 | 否 | 否 | 否 | 否 |

## 5. 需要保留的可靠基础

Phase 3B.1 默认保留以下资产，并通过回归测试防止重构损坏：

- workspace 分层与 core 的 Bevy-free 约束。
- matcher、fingerprint、稳定声明顺序和 branch diff。
- frame id、generation 字段、transaction/task identity 与 stale 检查思路，新 engine 已实现 root generation advancement
- common-prefix frame 保留策略。
- app-owned host 与 router-owned subtree 的 ownership 原则。
- hidden/retained subtree 的输入隔离与 focus 释放规则。
- adapter 不拥有 commit authority 的负向 API 合同。

## 6. Task 9 接线基线

**Readiness**: Ready with in-task risks

- Tasks 1-8 的私有地基足以进入 Task 9，未发现需要提前拆出的实现阻断
- 新 `RouterEngine` 仍私有断开，legacy runtime/ECS/facade 路径仍是唯一集成 authority，这是原子切换前的预期状态
- Task 9 必须在同一切换中完成 grant 接线、物理 preflight、同帧 publication、最小 cleanup ledger、plugin/facade migration 与旧 publisher 删除
- 上述事项均已属于 Task 9 计划范围，因此不另设 readiness 阻断记录

## 7. 当前结论

统一逻辑 authority、View/ECS 私有协议与 exact ownership 地基已经形成，但真实 ECS/facade 主链仍使用旧 authority

项目仍处于实现早期，不是 Root PASS 或 MVP

下一阶段是 Task 9 原子切换，完成前不新增第二条 publisher 或兼容 authority 路径
