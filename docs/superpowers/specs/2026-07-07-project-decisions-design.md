# Bevy UI Router 项目决策设计记录

**日期:** 2026-07-07
**状态:** 已确认的准备阶段规划决策
**范围:** 实现前确认的项目级架构、MVP 范围和阶段规划决策。

---

## 目的

本文记录通用 Bevy UI Router 在准备阶段已经确认的项目决策。它不是实现计划，也不代表已经进入代码实现阶段。

本文的目标是在第一轮实现中保持范围聚焦，同时保留既有设计文档要求的架构边界：

- Router core 保持通用、可测试，并独立于 Bevy、BSN、平台 API、应用服务、主题、文案、权限和宿主应用假设。
- Bevy ECS integration 负责 entity、schedule、host、slot、visibility、pickability、focus 和 ownership 相关问题。
- BSN 是第一个具体 view adapter，而不是 Router core。
- 平台行为通过 router-neutral intent 和 location abstraction 进入 Router。
- 应用特定约定通过 extension 或 integration profile 留在 core 之外。

---

## 已确认决策

### 1. Workspace 形态

采用 multi-crate Cargo workspace。

已确认的 crate 形态：

- `bevy_ui_router_core`
- `bevy_ui_router_view`
- `bevy_ui_router_ecs`
- `bevy_ui_router_bsn`
- `bevy_ui_router_platform`
- `bevy_ui_router`
- examples 与 test support crate 或 package，按需要添加

理由：

- crate 边界可以强制执行最重要的架构规则：core 不能依赖 Bevy、BSN、平台 API 或应用特定概念。
- 独立 crate 更便于针对具体层编写合同测试。
- 未来 renderer、platform adapter 和 integration profile 可以演进，而不需要重写 core。

取舍：

- 相比单 crate，workspace 初始搭建成本更高，并且需要维护 feature 组合。

### 2. 对外入口

保留 `bevy_ui_router` 作为 facade crate。

facade crate 应 re-export 底层 crate 的常用 API，并通过 feature 控制可选集成。

建议默认 facade surface 包含：

- core 路由类型
- view adapter trait 与 fake/test renderer 支持
- ECS integration plugin 与 host/slot component 的 re-export 路径

建议 opt-in facade features 包含：

- ECS integration
- BSN adapter
- platform adapter helpers
- 仅开发期使用的工具

理由：

- 用户可以从一个简单入口开始使用。
- 内部 crate 边界依然明确且可测试。
- 可选集成可以继续 feature-gate，避免不稳定或平台特定依赖进入所有构建。

后续审查收敛说明：

- “默认 facade surface” 表示 facade 可以提供稳定的 re-export 路径，不表示 default feature 必须启用所有集成。
- 当前规划基线是 default feature 不拉入 Bevy；ECS、BSN 和 platform helper 通过显式 feature opt-in。
- examples 使用 workspace examples 或 dev-only package，不作为 facade feature 表达。

### 3. 规划范围

完整规划 Phase 0-6 路线图，但第一份详细实现计划只把 Phase 0-3 写到可执行粒度。

Phase 0-3 覆盖：

- 合同冻结
- route tree 与 branch model
- frame state 与 error boundary
- view adapter interface 与 immediate BSN adapter

Phase 4-6 在早期合同稳定前保持 roadmap 级别：

- loader、guard 和 revalidation
- cache、modal 和 transition runtime
- asset-driven BSN 与 dev tools

理由：

- 项目需要完整的架构方向，但非 MVP 系统不应在 core lifecycle 被证明前就进入实现或过度细化。

### 4. MVP API 风格

MVP public API 采用 builder-first 的 route tree 声明方式。

MVP 应提供清晰的手写 API，例如 `RouteTree` 或 route builder。底层数据结构仍需可直接测试，但用户不应被迫手动构造每一个内部字段。

理由：

- Builder API 明确、可测试，比宏更容易稳定。
- 在 derive macro、codegen 或 route DSL 出现前，API 已经可用。
- 避免过早把项目锁定在语法层决策上。

MVP 非目标：

- route 声明宏
- derive-based typed route generation
- file-system route generation

### 5. Typed Routes 与 Typed Search

MVP 可执行范围不包含 typed route 或 typed search public API。

MVP 使用：

- raw `RouteLocation`
- string path segments
- raw 或 string-based params/search
- 用于 branch comparison 的稳定 params/search fingerprint
- 面向未来 typed codec 的 extension 点

Typed route/search 支持移动到 Phase 4 或之后。

理由：

- 第一轮实现应先稳定 matcher、history、reconciliation、frame identity、generation 和 diagnostics，再引入 schema 与 generic codec 设计。
- 未来 typed API 应叠加在 raw boundary 之上，而不是替换 raw boundary。

### 6. View Adapter 策略

Phase 0-3 采用双轨 view adapter 策略。

轨道 A：fake/test renderer

- 用于合同测试。
- 必须在不依赖 BSN 的情况下证明 lifecycle、branch reconciliation、failure cleanup、diagnostics 和 ownership 规则。

轨道 B：immediate BSN adapter

- 在 `bevy_ui_router_bsn` 中实现。
- 通过 feature-gate 或独立 crate 与 core 隔离。
- 目标是支持 immediate code-driven BSN scene apply。
- 验证真实 Bevy/BSN 集成路径，同时不允许 BSN API 不稳定性塑造 core 语义。

理由：

- fake/test renderer 提供稳定的 Router 合同覆盖。
- BSN adapter 提前暴露真实集成风险。
- BSN 集成失败不应要求改变 core routing semantics。

### 7. Bevy 依赖策略

`bevy_ui_router_core` 不依赖 Bevy。

暂时让依赖 Bevy 的 crate pin 到当前仓库使用的自定义 Bevy 0.19 git 分支：

- `bevy_ui_router_ecs`
- `bevy_ui_router_bsn`
- 需要 Bevy 的 examples

路线图必须明确记录：后续迁移到 upstream Bevy 0.19+ 或官方 BSN API。

理由：

- 项目可以贴近当前 BSN 目标环境，同时防止实验性 Bevy 依赖泄漏到 core。
- 为未来可复现构建和公开发布约束保留清晰路径。

风险：

- 在 Bevy-dependent crates 使用自定义 git dependency 期间，CI、外部复现和发布能力可能受影响。

### 8. 验证策略

采用双轨验证。

Phase 0-3 的每个 milestone 都必须有聚焦的合同测试。Phase 3 还必须提供一个中性的 minimal nested routes example，覆盖：

- fake/test renderer 路径
- immediate BSN adapter 路径

重点测试：

- matcher 行为
- branch reconciliation 与 common-prefix retention
- frame identity 与 generation
- route-owned subtree cleanup
- failure cleanup 不触碰 app-owned host entities
- error attribution 与 nearest boundary behavior
- diagnostics snapshots
- 稳定排序，不依赖 Bevy 没有保证的 query iteration order

理由：

- 测试保护框架合同。
- 示例证明 public API 与集成形态可用。

---

## 架构边界摘要

### `bevy_ui_router_core`

只拥有 Router 语义：

- `RouteLocation`
- `RouteTree` 与 builder API
- matcher
- `MatchedBranch`
- memory history
- branch diff
- frame identity 与 generation
- frame lifecycle model
- error taxonomy
- diagnostics snapshot types
- 面向未来 typed route/search 与 app profile 的 extension 点

不能依赖：

- Bevy ECS types
- BSN
- platform APIs
- concrete UI widgets
- app-specific services、permissions、themes、copy 或 shell models

### `bevy_ui_router_view`

拥有 renderer-agnostic view contract：

- view factory contract
- render plan 或 view descriptor contract
- apply result 与 failure cleanup contract
- 用于合同测试的 fake/test renderer

它可以引用 core 中的 router identity，但不能拥有 history、matching 或 lifecycle authority。

### `bevy_ui_router_ecs`

拥有 Bevy integration：

- plugin setup
- schedule sets
- `RouteHost`
- `RouteSlot`
- route-owned frame root markers
- entity ownership and cleanup
- hierarchy mutation
- visibility、pickability 和 focus finalization
- 以 Bevy resource 或 event 发布 diagnostics

它不能允许 adapter 绕过 core lifecycle，也不能允许 adapter 伪造 frame identity/generation。

### `bevy_ui_router_bsn`

拥有 BSN rendering integration：

- code-driven BSN route views
- immediate BSN scene apply
- scene resolve/apply error mapping
- 报告已创建的 router-owned partial output，并通过受控 cleanup API 请求清理

它不能拥有：

- route matching
- history
- branch reconciliation
- frame lifecycle authority
- guard、loader、action、revalidation、back 或 platform policy

### `bevy_ui_router_platform`

拥有 router-neutral platform boundary types：

- `BackIntent`
- `BackPolicy`
- `BackCapabilitySnapshot`
- location source abstractions
- 面向未来 multi-window 或 multi-surface integration 的 router instance targeting

MVP 应保持该 crate 精简，避免接入具体平台 package。

---

## 数据流

预期 lifecycle：

```text
RouteLocation
  -> core matcher
  -> MatchedBranch
  -> branch diff
  -> frame plan with router, host, slot, frame identity, generation
  -> ECS creates or updates router-owned frame roots
  -> view renderer applies into RouteSlot
  -> ECS finalizes visibility, pickability, and focus
  -> core commits active rendered branch
  -> diagnostics snapshot
```

规则：

- requested location、matched branch 和 active rendered branch 必须分离。
- pending 或 partial applied scene 不是 active UI。
- frame 只有在 view apply 与 ECS finalization 成功后才能 active。
- stale async 或 deferred result 在产生可见状态变化前必须被拒绝。
- failure cleanup 只限于 router-owned local state 和 subtree。
- Router 不能销毁、替换或重建 app-owned host。

---

## 阶段路线图

### Phase 0: Contract Freeze

目的：

- 在加入 renderer 复杂度前，将最高风险的 lifecycle 规则转化为可执行合同。

可执行范围：

- workspace 与 facade 结构
- core crate isolation
- raw `RouteLocation`
- `push`、`replace`、`back` 的 memory history behavior
- initial frame identity 与 generation model
- route-owned 与 app-owned boundary tests
- diagnostics snapshot shape

退出标准：

- core 可以在不依赖 Bevy 的情况下编译。
- 合同测试覆盖 navigation history、frame identity、generation、ownership boundary 和 diagnostics 基础行为。
- public API 不要求 BSN 或 app-specific concepts。

### Phase 1: Route Tree And Branch Model

目的：

- 建立 route declaration 与 matching。

可执行范围：

- route builder API
- route ids
- static segments
- params
- catch-all segments
- index routes
- nested routes
- pathless layouts
- model 中的 named slot identity
- `MatchedBranch`
- common-prefix branch diff

退出标准：

- Matcher 测试覆盖 static、param、catch-all、index、pathless、not-found 和 conflict behavior。
- Branch diff 测试证明 common-prefix retention 与 changed suffix replacement。
- Matching order 是确定的；除非规则明确说明，否则不依赖 insertion ambiguity 或 query iteration ambiguity。

### Phase 2: Frame State And Error Boundary

目的：

- 分离 location、matched branch 和 active rendered branch，并明确失败行为。

可执行范围：

- route frame records
- MVP minimal statuses
- error taxonomy
- nearest error boundary resolution
- frame generation increment rules
- frame state 与 error attribution diagnostics

退出标准：

- Failed build/apply paths 进入明确 error state。
- Error boundary behavior 只替换目标 router-owned frame region。
- 测试禁止 app-owned host cleanup。
- Diagnostics 能识别 router、host、slot、frame identity、generation、route id 和 error source。

### Phase 3: View Adapter Interface And BSN Adapter

执行级拆分说明：

- 本文保留 Phase 3 的聚合决策口径。
- 后续执行流程已将 Phase 3 拆为 Phase 3A view contract/fake renderer、Phase 3B ECS integration、Phase 3C immediate BSN adapter、Phase 3D minimal nested routes example。
- 实施时以 `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md` 的 gate 拆分为准。
- Phase 3B 以 core-issued `FrameApplyRequest` 作为 ECS/view canonical handoff unit；non-fake adapter 的 renderer result reporting 必须通过 ECS bridge 从该 request mint 的 controlled reporter/session；ECS bridge 只能把 finalized renderer output 翻译为 core `FrameApplyOutcome` 并交回 core，最终 stale acceptance 和 active commit authority 留在 core。

目的：

- 证明 renderer-agnostic application 与第一个真实 renderer 集成。

可执行范围：

- `bevy_ui_router_view` renderer contract
- fake/test renderer
- ECS route host 与 slot integration
- frame root markers
- hierarchy apply and cleanup
- visibility、pickability 和 focus finalization hooks
- 依赖边界隔离下的 immediate code-driven BSN adapter
- minimal nested routes example

退出标准：

- Phase 3A fake renderer tests 证明 apply success、apply failure cleanup、stale result 无 partial output 和 plan provenance；controlled reporter/session、core-issued `FrameApplyRequest` handoff、core-owned stale acceptance 和 active commit ordering 由 Phase 3B ECS finalization 与 core commit integration 测试证明。
- BSN adapter 在中性 example 中展示 immediate scene apply。
- renderer apply 与 ECS finalization 完成前，frame 不能标记为 active。
- BSN failures 映射为 route view/apply errors。

### Phase 4: Loader, Guard, And Revalidate

路线图范围：

- typed params/search exploration
- sync guard first，并为 pending guard 预留
- route-scoped task contract
- async result identity 与 stale checks
- loader/action/revalidation dependency model
- redirect loop detection

进入条件：

- Phase 0-3 合同已经足够稳定，async result 可以指向清晰的 frame identity 与 generation。

### Phase 5: Cache, Modal, And Transition

路线图范围：

- route-local serializable state
- retained entity cache contract
- modal/drawer frame kind
- pause/resume lifecycle
- transition policy
- back policy composition
- hidden subtree input isolation 与 focus release

进入条件：

- ownership、cleanup、focus、visibility 和 pickability finalization 已有测试覆盖。

### Phase 6: Asset-Driven BSN And DevTools

路线图范围：

- `.bsn` asset route views
- queued/asset scene lifecycle
- hot reload 通过重建 view frame 并保留允许保留的 route state 实现
- diagnostics overlay
- navigation trace replay

进入条件：

- Immediate BSN adapter 稳定，并且 queued scene pending/apply/active boundary 已经显式建模。

---

## 已知风险

- 自定义 Bevy 0.19 git dependency 可能让 CI 和外部复现变脆弱，直到可以迁移到 upstream Bevy 或官方 BSN API。
- BSN API 不稳定可能影响 `bevy_ui_router_bsn`，但不能重塑 `bevy_ui_router_core`。
- 如果 Builder API 在 raw route model 稳定前试图覆盖 typed routes、macros、profiles 和 file routing，会变得过宽。
- Examples 可能无意中引入 app-specific assumptions；示例必须使用中性页面名和通用 UI。
- Retained entity cache、transition 和 async loader semantics 都很重要，但在 lifecycle 与 ownership contracts 稳定前实现它们会增加风险。

---

## MVP 非目标

- typed route/search public API
- derive macros 或 route DSL macros
- async loader/action/revalidation implementation
- pending async guard implementation
- retained entity cache implementation
- transition animation runtime
- file-based BSN route scenes
- hot reload
- platform-specific back 或 deep-link packages
- app-specific integration profiles
- business-specific metadata、permissions、services、copy、themes 或 shell policies

---

## 审阅说明

本文记录准备阶段已经确认的决策。详细任务步骤会留到后续 implementation plan。

进入实现计划前，应确认这些决策仍符合项目方向：

- multi-crate workspace 搭配 facade crate
- Phase 0-6 路线图，并将 Phase 0-3 写到可执行粒度
- builder-first MVP API
- MVP 使用 raw route/search
- fake renderer 与 BSN adapter 双轨推进
- Bevy 依赖隔离在 ECS/BSN crates
- 通过测试和中性示例进行双轨验证
