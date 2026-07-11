# Current Architecture

**文档类型**: 当前事实
**状态**: Phase 3B.1 重构前基线
**as_of_commit**: `23fe826033d09716553dd6922dd204c4a45b1a6c`
**事实来源**: workspace manifest、公共导出、实现代码、测试代码与可执行验证；历史计划不作为事实来源

基准提交未跟踪 `Cargo.lock`，Bevy dependencies 使用可移动 git branch；本地解析到的 revision 不能视为仓库可复现合同。Phase 3B.1 的 exact revision 与 tracked lock 属于目标迁移，不是当前事实。

## 1. 当前定位

项目已经从单 crate 占位工程发展为包含 core、view、ECS、BSN、platform 和 facade 的 Cargo workspace。matcher、branch diff、frame identity、generation、transaction stale check、局部清理和输入隔离等基础已有较强的局部合同，但这些能力还没有被一条可由 facade 公共 API 驱动的真实导航路径统一起来。

因此，当前项目应定义为“生命周期安全原型”，而不是已经可用的 MVP Router。已有测试主要证明局部类型与低层协议，不能证明 `push`、`replace`、`back` 已经在 Bevy `App` 中闭环。

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

尚未集成：

- `RouterRuntime` 只持有 router/host/单个 slot、allocator、active 和 pending；它不拥有 route tree、matcher、history、requested location、matched branch 或统一 diagnostics。
- 首次导航和后续 reconciled navigation 直接接收调用者提供的 `MatchedBranch`，后者还接收调用者提供的 `BranchDiff`，统一公共路径没有掌握匹配和 diff 的来源。
- 一个 runtime scope 的所有 frame 使用同一个 `RouteSlotId`；`RouteSlotKey` 只记录逻辑名称，尚未形成真实父 frame outlet 身份。
- 生产 frame 只创建 initial generation；generation advancement/remount 尚未接入运行路径。
- `RouteTree` 不携带 renderer-neutral view binding，也没有将 error boundary 绑定到可解析 view。

### 3.2 View

已经实现：

- `ViewBindingKey`、`RouteViewDescriptor`、build/apply outcome、partial output 和单次 report session。
- fake renderer 及 authority surface 的正向、负向合同测试。

尚未集成：

- route node 与 view descriptor 之间没有绑定。
- 没有生产用 binding registry/resolver。
- fake renderer 协议和 ECS apply session 处于相邻但独立的路径，没有统一 effect/report 调度入口。

### 3.3 ECS

已经实现：

- `RouteHost`、`RouteSlot`、`RouteFrameRoot` 及 lifecycle/interaction 快照。
- frame apply session、inert prepare、activation、failure/stale cleanup、focus 与输入隔离辅助逻辑。
- 对 router、host、slot、route、frame、generation 的多字段校验，以及稳定排序。

尚未集成或存在语义缺口：

- `RouterEcsState` 的公共导航入口只有 `begin_first_navigation`；二次导航测试绕过公共边界直接访问内部 runtime。
- `BevyUiRouterPlugin` 只初始化资源和配置 schedule set，没有注册驱动导航的 systems。
- 当前 host/slot/frame测试不要求 app-owned host是 Bevy `Node`，Router-owned结构层也没有形成从UI root到内容的全Node直接ancestry；因此现有Entity层级不能证明真实UI layout/picking可达。
- 没有 ECS 唯一 registry 维护 host、slot、frame version 与 Entity 的一对一映射。
- 当前 ancestry 校验拒绝 slot 上方再出现 slot，因此不能表达 `host -> parent root -> child outlet -> child root` 的真实嵌套拓扑。
- core 先 commit，随后逐个激活 ECS root；激活没有成为 core publish 前的受检原子步骤。
- 离场 root 不由统一提交路径自动隔离并清理；已有测试允许前后两批 root 同时保持 active。
- renderer 可以通过公共 `RouteFrameRoot::from_frame_request` 构造权威 root marker，权限边界过宽。
- cleanup 主要按 marker 扫描，没有 registry receipt 证明实际移除了哪个已登记实体。
- 当前 cleanup 对命中 marker 的 root 直接递归 despawn，既不先拒绝 app-owned/foreign descendant，也没有独立 visited/cycle 防护；结合可公开伪造的 root marker，错误或恶意标记可能越权删除应用实体，异常层级还可能使遍历无法可靠收敛。这是重构前基线风险，不是目标架构允许的能力。
- pending root 由外部先创建再补做 inert prepare，无法结构性保证它从出生起就不可见、不可交互。

### 3.4 BSN、Platform 与 Facade

- `bevy_ui_router_bsn` 只有空 prelude，尚无 adapter 实现。
- `bevy_ui_router_platform` 只 re-export back 相关 core 类型。
- facade 只按 feature 把 core/view/ECS/BSN/platform 作为整 crate alias re-export；因此 `core::runtime`、旧 transaction/apply/session、ECS authority marker/state 等低层路径仍可达，现有 facade 测试还验证其中部分旧 prelude 类型。
- facade 没有 router 配置、host 注册、导航命令或状态观察的完整公共流程。Phase 3B.1 必须用 curated wrapper/allowlist 取代整 crate alias，并同步改写这些旧正向测试；这不是当前已经完成的能力。

## 4. 能力账本

| 能力 | Implemented | Integrated | 低层公共可达 | 推荐 facade 可达 | Demonstrated |
| --- | --- | --- | --- | --- | --- |
| route tree 与 matcher | 是 | 否 | 是 | 否 | 否 |
| branch diff 与 common-prefix | 是 | 低层集成 | 是 | 否 | 否 |
| frame identity/generation/stale | 字段与校验已实现；生产 advancement 未实现 | 局部集成 | 间接 | 否 | 否 |
| inert/focus/input cleanup | 是 | 局部集成 | feature-gated 低层可达 | 否 | 否 |
| route-view binding | 否 | 否 | 否 | 否 | 否 |
| 真实嵌套 mount | 否 | 否 | 否 | 否 | 否 |
| push/replace/back | history 算法存在 | 否 | history 独立可达 | 否 | 否 |
| plugin 自动驱动 | schedule 结构存在 | 否 | plugin 可添加 | 否 | 否 |
| diagnostics 闭环 | 数据类型存在 | 否 | 局部可读 | 否 | 否 |
| BSN immediate adapter | 否 | 否 | 否 | 否 | 否 |

## 5. 需要保留的可靠基础

Phase 3B.1 默认保留以下资产，并通过回归测试防止重构损坏：

- workspace 分层与 core 的 Bevy-free 约束。
- matcher、fingerprint、稳定声明顺序和 branch diff。
- frame id、generation 字段、transaction/task identity 与 stale 检查思路；生产 generation advancement 由 Phase 3B.1 补齐。
- common-prefix frame 保留策略。
- app-owned host 与 router-owned subtree 的 ownership 原则。
- hidden/retained subtree 的输入隔离与 focus 释放规则。
- adapter 不拥有 commit authority 的负向 API 合同。

## 6. 当前结论

问题不是某个局部函数缺失，而是缺少一个掌握导航来源、分阶段 effect、ECS 映射和最终提交的统一协调权威。继续追加 Task 6 或 Phase 3C 会把并行协议继续堆叠在不闭环的路径上，因此应暂停它们，先完成 Phase 3B.1 架构级局部重构。
