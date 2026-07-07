# Bevy UI Router 架构与路线规划

**日期:** 2026-07-07
**状态:** 已通过多轮审查的规划基线
**关联审查记录:** `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-review-log.md`
**范围:** 通用 Bevy UI Router 的架构边界、核心语义、MVP 合同、阶段路线与验证策略。

---

## 1. 文档定位

本文是后续实现工作的主规划文档，不是实现代码，也不是某个宿主应用的集成方案。它只规划一个通用、可组合、可测试的 Bevy UI Router 框架。

本版只整理结构和表达，不改变已通过审查的技术决策与合同语义。审查过程、问题处置和通过结论见关联 review log，本文只保留规划正文所需的结论。

当前仓库事实：

- 当前仓库尚未拆分 workspace，`Cargo.toml` 仍是单 crate package。
- 当前根 crate `bevy_ui_router` 直接依赖自定义 Bevy 0.19 git 分支。
- 当前 `src/main.rs` 只是占位程序，尚未实现 Router crate、测试或示例。

本文中的 workspace、crate 和 feature 结构是已确认的目标结构，不是当前已存在的代码结构。Phase 0 的第一项工作是把当前单 crate 迁移到目标结构。

已确认方向：

- 将当前单 crate 迁移为 multi-crate Cargo workspace。
- 保留 `bevy_ui_router` 作为 facade crate。
- `bevy_ui_router_core` 不依赖 Bevy、BSN、平台 API 或应用特定概念。
- MVP public API 采用 builder-first route tree 声明。
- MVP 使用 raw `RouteLocation`、string params/search 和稳定 fingerprint，不实现 typed route/search public API。
- Phase 0-6 形成完整路线图；Phase 0-3 写到 implementation-plan 友好的粒度。
- View adapter 采用 fake/test renderer 与 immediate BSN adapter 双轨推进。
- 拆分后只有 Bevy-dependent crates 暂时 pin 到当前自定义 Bevy 0.19 git 分支，并在路线图中保留 upstream 迁移路径。
- 每个阶段以合同测试和中性示例验证，不以应用业务场景验证通用框架。

---

## 2. 目标与非目标

### 2.1 总体目标

Bevy UI Router 的目标是为 Bevy UI 提供通用、可组合、可测试的路由框架。

它应支持：

- route tree、nested route、index route、pathless layout 和 named slot。
- memory location/history，以及后续可插拔 location source。
- requested location、matched branch 和 active rendered branch 分离。
- branch reconciliation 与 common-prefix frame retention。
- app-owned host 与 router-owned subtree 明确分离。
- route frame identity 与 generation 可追踪。
- view build/apply 失败可归因、可清理、可进入最近 error boundary。
- renderer-agnostic view adapter contract。
- Bevy ECS integration 管理 entity ownership、hierarchy、visibility、pickability 和 focus finalization。
- BSN 作为首个具体 view adapter，而不是 Router core。
- 平台输入通过 router-neutral intent 进入 Router。
- 未来 loader、guard、revalidation、cache、transition、asset scene 和 DevTools 能力可以增量加入，而不推翻 core。

### 2.2 MVP 非目标

MVP 不实现：

- typed route/search public API。
- route 声明宏、derive macro、file-system route generation。
- async loader/action/revalidation。
- pending async guard。
- retained entity cache runtime。
- transition animation runtime。
- file-based BSN route scenes。
- hot reload。
- platform-specific back/deep-link packages。
- app-specific integration profile。
- 任何业务领域、品牌、主题、文案、权限或数据服务约定。

非目标不表示永不支持。它们应在架构中有明确预留，但不进入 Phase 0-3 的实现承诺。

---

## 3. 全局不变量

### 3.1 Ownership

Router 只管理 router-owned subtree。

应用创建并拥有 `RouteHost` 所在 entity 或宿主 shell。Router 可以在 host 下创建、替换、隐藏、缓存或清理 route-owned frame subtree，但不能销毁、替换或无理由重建 app-owned host。

任何 cleanup 都必须带有 ownership check。失败清理范围由 navigation transaction 记录限定，只触碰本次失败路径创建的 router-owned partial output 或已明确进入 cleanup 阶段的 router-owned subtree。

### 3.2 Identity And Generation

每个可见或待应用 frame 都必须能关联到：

- `RouterInstanceId`
- host identity
- slot identity
- `RouteFrameId`
- `RouteGeneration`
- route id

adapter 不得伪造 frame identity 或 generation。它只能通过公开 API 报告 view build/apply 结果。

### 3.3 Pending Is Not Active

pending scene、partial applied scene、matched branch 和 requested location 都不是 active UI。

active UI 只能来自：

1. route location 已匹配；
2. branch diff 已生成；
3. 目标 frame 已建立；
4. view build/apply 成功；
5. ECS finalization 完成；
6. identity/generation 校验通过；
7. core 提交 active rendered branch。

### 3.4 Stale Safety

不持有 `&mut World` 跨 `await`。

任何异步、deferred 或 queued 结果 apply 前必须执行 stale check。MVP 即使不实现完整 async loader，也必须在模型中预留校验字段和 apply 边界。

最低 stale check 维度：

- router id
- frame id
- generation
- task/load/apply token，等相关功能出现时加入

身份不匹配的结果必须丢弃，不能产生可见状态变化。

### 3.5 Stable Ordering

Router 不能依赖 Bevy query 未定义顺序。

需要稳定顺序时，必须使用显式 route index、slot key、route id、builder declaration order 或排序规则。排序依据必须可诊断、可测试。

### 3.6 Hidden Or Retained Subtree

一旦实现 hidden/retained subtree，就必须隔离输入并释放 focus。

隐藏或保留的 subtree 不能继续接收 pointer、keyboard、gamepad input，也不能保留 active focus。MVP 不实现 retained entity cache runtime，但 ECS finalization 必须独立成阶段，为后续能力留出位置。

### 3.7 Adapter Boundary

adapter 只能通过公开 API 影响 Router。它不能伪造 generation/frame identity，不能绕过 lifecycle 直接切换 active frame，不能拥有 ECS subtree cleanup authority。

---

## 4. Workspace、依赖与 Feature 边界

### 4.1 目标 Workspace

目标 workspace crate：

```text
bevy_ui_router_core
bevy_ui_router_view
bevy_ui_router_ecs
bevy_ui_router_bsn
bevy_ui_router_platform
bevy_ui_router
examples 或 test-support package
```

`bevy_ui_router` 是 facade crate。它面向用户提供常用 re-export，并通过 feature 控制 ECS、BSN、platform helper 等可选能力。

### 4.2 Allowed-Dependency Matrix

依赖方向是 DAG，不是线性链。

| Crate | 允许依赖 | 禁止依赖 |
| --- | --- | --- |
| `bevy_ui_router_core` | Rust std/alloc 级依赖和明确批准的小型通用依赖 | Bevy、BSN、platform crate、view crate、app profile |
| `bevy_ui_router_view` | core | Bevy ECS、BSN、platform crate、app profile |
| `bevy_ui_router_ecs` | core、view、Bevy | BSN、具体平台 package、app profile |
| `bevy_ui_router_bsn` | core、view、ecs、Bevy/BSN | platform crate、app profile |
| `bevy_ui_router_platform` | core | ecs、bsn、Bevy renderer、app profile |
| `bevy_ui_router` | 按 feature re-export 以上 crate | 自己定义新的 lifecycle authority |

不允许反向依赖，例如 core 依赖 Bevy component、BSN scene、platform helper 或 app integration profile。

跨层共享类型必须由最底层能合理拥有该语义的 crate 定义。若 core 需要消费某个类型，该类型不能定义在依赖 core 的上层 crate 中。

### 4.3 Feature Boundary

feature 分层分为“facade 可导出能力”和“default 实际启用依赖”两件事。

规划基线：

- default: core + view public contract，不拉入 Bevy。
- `ecs`: 启用 Bevy ECS integration re-export。
- `bsn`: 启用 immediate BSN adapter，并隐含或要求 `ecs`。
- `platform`: 启用 router-neutral platform helpers。
- `devtools`: 后续 diagnostics overlay 与 trace replay。
- examples 使用 workspace examples 或 dev-only package，不作为 facade feature 表达。

facade 应提供 ECS/BSN/platform re-export 路径，但实际启用由显式 feature 控制。这样可以避免 default feature 无条件拉入当前自定义 Bevy git dependency。

Phase 0 必须冻结最终 feature 名称和默认组合。若推翻“default 不拉入 Bevy”的基线，必须记录理由。

---

## 5. Crate 职责

### 5.1 `bevy_ui_router_core`

core 只拥有 Router 语义：

- `RouterInstanceId`
- `RouteLocation`
- memory history abstraction
- route id、slot id、frame id、generation
- `RouteTree` 与 builder API
- matcher
- `MatchedBranch`
- branch diff
- frame lifecycle model
- router-neutral `BackIntent`、`BackPolicy` 与 `BackCapabilitySnapshot`
- requested/matched/active state separation
- error taxonomy
- nearest error boundary resolution
- diagnostics snapshot types
- extension hooks for typed routes/search and app-defined payload/profile integration

core 不拥有 Bevy ECS 类型、BSN scene 类型、concrete UI widgets、platform APIs、app service/permission/theme/copy/business meta、app integration profile 本体或 renderer apply implementation。

### 5.2 `bevy_ui_router_view`

view crate 定义 renderer-agnostic view contract：

- view binding registry 或等价 builder contract。
- view descriptor/render plan。
- renderer apply request/result。
- view build failure 与 apply failure 映射。
- failure cleanup contract。
- fake/test renderer。

view crate 不拥有 route matching、history、branch reconciliation、Bevy `RouteSlot` 或 lifecycle authority。它只描述如何从 route frame 输入得到可应用的 view plan，以及如何报告结果。

view crate 的 public testing utility 可以包含 fake/test renderer。若测试支撑代码会增加依赖或污染正常 API，则通过 `test-support` feature 或独立 test-support package 暴露。

### 5.3 `bevy_ui_router_ecs`

ecs crate 将 Router 语义映射到 Bevy World：

- plugin setup。
- schedule sets。
- `RouteHost`。
- `RouteSlot`。
- route-owned frame root marker。
- ownership check。
- hierarchy mutation。
- cleanup。
- visibility finalization。
- pickability finalization。
- focus release/restore boundary。
- diagnostics publication。

ecs crate 可以执行 World mutation，但不能绕开 core lifecycle。它必须让“apply 成功”和“active commit”成为两个有序步骤。

### 5.4 `bevy_ui_router_bsn`

bsn crate 是第一个真实 renderer adapter：

- code-driven `bsn!` route views。
- immediate scene build/apply。
- BSN resolve/apply error mapping。
- partial output reporting and controlled cleanup integration。
- minimal nested routes example 的真实集成路径。

bsn crate 不拥有 route tree/matcher/history、branch reconciliation、frame lifecycle authority、guard/loader/action/revalidation、back/platform policy 或 app-specific profile。

BSN API 不稳定只能影响 bsn crate 或 feature surface，不能反向塑造 core 语义。

### 5.5 `bevy_ui_router_platform`

platform crate 提供 adapter glue 和 helper：

- 将平台事件翻译成 core 定义的 `BackIntent`。
- 将 deep link、custom URI 或窗口输入翻译成 core `RouteLocation`。
- 读取 core 定义的 `BackCapabilitySnapshot`。
- 为 multi-window 或 multi-surface 输入提供 router instance targeting helper。

MVP 中该 crate 保持轻量。它不接入具体平台 package，不承载业务权限、数据加载或应用渲染策略。

### 5.6 `bevy_ui_router`

facade crate 提供统一入口：

- re-export core common API。
- re-export view contract 和 fake/test renderer。
- feature-gated re-export ECS plugin、BSN adapter、platform helper。
- 提供面向用户的 prelude。

facade 不应隐藏底层 crate 边界。用户需要低层测试或高级集成时，应能直接依赖对应 crate。

---

## 6. 核心模型与公共类型归属

### 6.1 `RouteLocation`

`RouteLocation` 表示一次导航目标。

MVP 字段语义：

- path: raw path。
- search: raw 或 string-based query/search。
- fragment: optional。
- payload/extension: optional，保持 core-neutral。
- router instance target: optional 或由调用上下文绑定。

MVP 不做 typed route/search public API，但保留未来 codec 扩展点。

`RouteLocation` extension/payload 默认不参与 matching、branch diff、history serialization 或 diagnostics。若应用希望 payload 影响 navigation identity，必须显式提供 stable key 或 fingerprint，不允许 core 读取应用 payload 语义。

### 6.2 `RouteTree`

`RouteTree` 是静态声明，不是运行时状态。

MVP 支持 static segment、param segment、catch-all segment、index route、nested route、pathless layout、named slot、error boundary、route metadata extension、renderer-neutral view binding key 或 source id，以及 explicit declaration order/index。

Builder API 是用户主入口。底层结构仍需可测试，但不要求用户手写内部字段。

core `RouteTree` 不保存 view crate 的 factory trait、Bevy component、BSN scene 或闭包。renderer-specific factory registry 属于 view/facade/adapter 层；它可以把用户友好的 builder 调用降级为 core route tree + view binding registry。

### 6.3 `MatchedBranch` And `BranchDiff`

`MatchedBranch` 是 matcher 输出，包含 route id chain、params、search fingerprint、slot placement、error boundary candidates 和 matched metadata。它不是 rendered UI，也不拥有 frame root。

`BranchDiff` 比较旧 active branch 与新 matched branch。比较依据是 route id、slot identity、params/search fingerprint，以及 route 声明提供的 explicit key。

MVP branch diff 策略：

- 找到 common prefix。
- 保留 prefix frame。
- 从 leaf 到 root leaving changed suffix。
- 从 root 到 leaf building/applying new suffix。
- 只替换受影响 slot 内的 router-owned subtree。

### 6.4 `RouteFrame` And `RouterState`

`RouteFrame` 是 matched branch segment 的运行时实例，最低字段语义包括 frame id、route id、router instance id、host identity、slot identity、generation、params/search fingerprint、status、error state 和 diagnostics label。

ECS root entity reference 位于 ECS integration 层。core 可通过抽象 handle 或 diagnostics id 表达关联，不直接依赖 Bevy entity 类型。

`RouterState` 必须分离 requested location、matched branch、active rendered branch、pending navigation/apply state、error state 和 diagnostics snapshot。不能把 current location 等同于 active UI。

### 6.5 Public Type Ownership

公共类型归属必须在 Phase 0 冻结。规划基线如下：

| 类型或概念 | 归属 | 说明 |
| --- | --- | --- |
| `RouteLocation` | core | raw location boundary；typed codec 后续叠加 |
| `RouteTree` / builder | core | 只保存 renderer-neutral view binding key |
| view factory / registry | view 或 facade/adapter | 不进入 core route tree |
| `MatchedBranch` / `BranchDiff` | core | 不引用 Bevy entity |
| `RouteFrameId` / `RouteGeneration` | core | 构造器受控，adapter 不能伪造 |
| `BackIntent` / `BackPolicy` / `BackCapabilitySnapshot` | core | platform crate 只生产或读取这些 core 类型 |
| `RouteHost` / `RouteSlot` / frame root marker | ecs | Bevy component，不进入 core |
| `ViewDescriptor` / renderer result | view | renderer-agnostic，不要求 Bevy slot/entity |
| BSN scene binding | bsn | feature-gated，不进入 core/view 公共基线 |
| app profile | app 或 integration crate | core 只提供 extension hook，不拥有 profile 本体 |

需要防止 adapter 绕过公共 API 的类型，例如 frame identity、generation、commit token，应使用私有字段、受控构造器、crate visibility、sealed token 或 compile-fail tests 保护。具体机制在 Phase 0/2 冻结。

### 6.6 Raw Params/Search Fingerprint

Branch diff 依赖稳定 fingerprint，因此 raw boundary 也需要最低 canonicalization 合同。

MVP 基线：

- path params 来自 matcher 产生的 decoded segment 值。
- params fingerprint 由 route id、param name 和 param value 的稳定顺序生成。
- raw search 保留重复 key。
- raw search 中 key/value 顺序默认有意义；不同顺序会产生不同 fingerprint。
- percent-encoding normalization、空值、缺失值和 repeated key 表达必须在 Phase 1 冻结，并有测试覆盖。
- fragment 默认不参与 branch diff；需要参与时必须由 route policy 或 future typed codec 显式声明。
- app payload/extension 默认不参与 fingerprint。

该策略偏保守：它可能让语义等价但顺序不同的 query 触发重建，但不会把未定义的应用语义写进 core。

---

## 7. 生命周期、事务与错误合同

### 7.1 最小状态

MVP 实际执行状态：

```text
Matching -> Building -> Applying -> Active
                     -> Error
Active -> Leaving
```

类型或内部模型可预留 Guarding、ResolvingView、Entering、Pausing 和 Cached，但这些预留状态在 Phase 0-3 不代表已实现完整行为。

### 7.2 Navigation Flow

```text
RouteLocation request
  -> match route tree
  -> produce MatchedBranch
  -> diff with active rendered branch
  -> plan leaving/building frames
  -> create navigation transaction
  -> pre-apply stale/identity gate
  -> create/update router-owned frame roots
  -> build view descriptor/render plan
  -> pre-apply stale/identity gate for deferred result
  -> apply renderer output into ECS-provided route target
  -> finalize visibility/pickability/focus
  -> pre-commit identity/generation recheck
  -> commit active rendered branch
  -> publish diagnostics
```

stale/identity check 有两层：

- pre-apply gate：任何 deferred、queued 或 async-like 结果在写入 World 之前必须验证 router、host、slot、frame、generation 和相关 token。
- pre-commit recheck：view apply 与 ECS finalization 完成后，active commit 前再次验证身份，防止 apply 期间发生 pending overwrite 或 host/slot 变化。

### 7.3 Commit Rule

core 只能在以下条件全部满足后提交 active rendered branch：

- route match 成功或 error boundary fallback 已成功匹配。
- 所有需要 active 的 frame 都完成 view apply。
- ECS finalization 成功。
- router/host/slot/frame/generation 身份仍然匹配。
- failure cleanup 未报告阻塞性错误。

### 7.4 Failure Rule

失败必须按来源分类：

- not found。
- match conflict。
- guard rejected，Phase 0-3 默认只作为 reserved error kind；同步 guard 是否实现由 Phase 0 冻结。
- view build failed。
- view apply failed。
- stale result。
- adapter failed。
- host or slot unavailable。
- ECS finalization failed。
- cleanup failed。
- internal。

失败处理规则：

- 清理当前 navigation transaction 中已经创建的 router-owned partial output。
- 不触碰 app-owned host。
- 找最近 error boundary。
- 若 boundary view apply 成功，则提交 boundary 覆盖范围内的 active error UI。
- 若 boundary 也失败，则使用 router-level fallback 或进入 router error state。
- diagnostics 必须保留原始错误来源和失败 frame identity。

Error boundary 不破坏 history。除非 navigation policy 明确要求替换 location，否则错误 UI 只是当前 route branch 的渲染结果。

### 7.5 Navigation Transaction

每次 navigation 都必须形成 transaction。

transaction 记录 source requested location、target matched branch、branch diff、planned leaving frames、new or reused frame identities、generation/token、partial output handles 和 cleanup scope。

logical leaving 可以在 planning 阶段产生，但物理 despawn、hide、focus release 和 active replacement 的提交点必须受 transaction 控制。

默认 MVP 事务规则：

- 旧 active branch 在新 branch 或 error boundary 成功 commit 前保持可回退。
- 新 navigation 失败时，只清理本次 transaction 创建的 partial output。
- 失败清理不能误删仍作为 active fallback 的旧 frame。
- pending overwrite 会使旧 transaction 失效；旧 transaction 的 deferred/apply 结果必须在 pre-apply gate 被丢弃。
- missing host/slot 进入明确错误和 diagnostics，不由 Router 补建 app-owned host。

### 7.6 Frame Id And Generation Rules

Frame identity 和 generation 是 stale safety 的核心，不只是 diagnostics 字段。

MVP 基线：

- route id、slot identity、router id、host identity、explicit key 和 params/search fingerprint 全部匹配时，frame 可以作为 common prefix 复用。
- route id、slot identity、explicit key 或 params/search fingerprint 变化时，受影响 frame 需要新 frame id。
- host identity 或 router instance 变化时，不复用 frame id。
- 同一 frame id 被用于新的 build/apply attempt 时，generation 必须递增。
- retry、pending overwrite 后重新 apply、boundary fallback replacement、view key 变化但保留外层 layout 时，都必须有明确 generation 变化。
- stale check 必须比较 frame id 与 generation；只比较 route id 不够。
- adapter 只能接收由 Router/ECS 提供的 opaque frame context，不能自行构造 frame id/generation。

具体增量时机在 Phase 2 冻结，并由合同测试覆盖。

---

## 8. ECS、View Adapter 与 Platform 合同

### 8.1 ECS Host, Slot And Frame Marker

`RouteHost` 标记应用提供的宿主区域。它属于 app-owned entity。

`RouteSlot` 是 router-owned frame 或 app-owned host 内的挂载点。slot identity 必须稳定，不能依赖 query iteration order。Route layout 可以持有 slot 并在 child route 变化时保留自身 frame。

每个 route-owned frame root 必须带 marker，至少能关联 router instance id、host identity、slot identity、frame id、generation 和 route id。cleanup 时必须通过这些 marker 限定范围。

### 8.2 ECS Schedule And Authority

ECS integration 应至少区分以下阶段，具体 schedule set 名称在 Phase 0/3 冻结：

- collect navigation intents。
- match/plan navigation。
- apply hierarchy/view changes。
- finalize visibility/pickability/focus。
- commit active state。
- publish diagnostics。

关键要求是：World mutation 不散落在异步 callback 中，active commit 不早于 view apply 与 finalization。

orchestration 模式：

- core 拥有 lifecycle authority 和 commit 判断。
- ECS schedule 驱动 Bevy World mutation。
- Phase 3B 以 core-issued `FrameApplyRequest` 作为 canonical ECS/view handoff unit。
- non-fake adapter 的 renderer success/failure/partial output/disposal hint reporting 必须通过由 ECS bridge 从 `FrameApplyRequest` mint 的 controlled reporter/session；不能把 Phase 3A raw protocol constructors 改成 public。
- ECS finalization outcome 必须翻译为 core `FrameApplyOutcome`，并与该 request 的 token 一起通过 core public API 或受控 token 回报。
- 最终 sequence/router/host/slot/route/frame/generation/transaction stale acceptance 留在 core 内部；ECS/view/BSN/facade/adapter 不得基于公开 token 字段自行重建 stale check。
- core 根据回报结果决定 commit active、进入 error，或丢弃 stale transaction。
- ECS 不直接把 frame 标记为 active；adapter 也不能直接切换 active frame。

### 8.3 Visibility, Pickability, Focus

MVP 即使没有 transition runtime，也要把 finalization 作为独立概念：

- leaving/failed subtree 不应保持 active focus。
- hidden subtree 不应可 pick。
- replaced subtree cleanup 前应释放或转移 focus。
- active subtree 的 focus 恢复策略可以先做最小实现或 no-op，但不能让旧 subtree 继续持有 active focus。
- 如果当前 focused entity 属于 replaced、leaving、failed 或 hidden subtree，finalization 必须 clear focus 或转移到受控 fallback。
- active focus 不得指向非 active subtree。

### 8.4 Ownership Cleanup

cleanup 输入必须包含目标 router/frame/generation 或等价受控范围。

cleanup 不允许：

- despawn app-owned host。
- 清理其他 router instance 的 frame。
- 清理同一 host 下不属于当前 slot/frame 的 app-owned child。
- 依赖查询返回顺序决定清理对象。

adapter 可以报告 partial output 和 renderer-local disposal hints。真正的 ECS subtree cleanup 必须由 Router/ECS cleanup API 按 router、host、slot、frame、generation marker 执行。

### 8.5 View Adapter Boundary

view adapter 只负责：

- 接收 route props/frame context。
- 构造 renderer-specific view descriptor 或 render plan。
- 将 render plan 交给 renderer-specific apply path，目标由 ECS 层提供的受控 route target 表达。
- 报告成功、失败和 partial output。
- 报告 renderer-local disposal hints。

view adapter 不负责 location/history、matching、branch diff、frame lifecycle authority、active commit、platform back policy 或 ECS subtree cleanup authority。

### 8.6 Fake/Test Renderer

fake/test renderer 是合同测试的第一目标。

它应能模拟 build success、build failure、apply success、apply failure with partial output、stale result、deterministic render order 和 diagnostics output。它的价值是证明 Router 合同不依赖 BSN。

### 8.7 Immediate BSN Adapter

Phase 3 的 BSN adapter 只承诺 immediate code-driven BSN scene apply。

它应：

- 在 `bevy_ui_router_bsn` 中实现。
- feature-gated。
- 不进入 core dependency graph。
- 将 BSN resolve/apply error 映射为 Router view error。
- 在失败时报告已创建的 router-owned partial output，并请求或触发受控 ECS cleanup API。
- 不把 `queue_spawn_scene` 返回 entity 误认为 active UI。
- 不实现 queued/asset apply，但 adapter contract 必须为 future completion token 留位置。

Queued/asset scene 留到后续阶段，但状态机从第一版承认 Resolving/Applying/Active 的区别。

### 8.8 Platform Boundary

MVP 只需要 memory history 和基础 back intent 抽象。

platform layer 输出或读取 location update、back intent 和 core capability snapshot。core 消费 router-neutral intent，不关心来源是键盘、手柄、移动端手势、deep link 还是窗口系统。

多窗口或多 surface 支持不进入 MVP，但 `RouterInstanceId` 必须从第一版存在，避免后续事件归属缺失。

---

## 9. Error And Diagnostics

### 9.1 Error Taxonomy

以下 taxonomy 是对早期设计文档中错误来源的收敛结果。最终 enum 名称可以在 Phase 0/2 冻结，但来源分类不能混淆。

MVP error kind 至少区分：

- `NotFound`
- `MatchConflict`
- `GuardRejected`
- `ViewBuildFailed`
- `ViewApplyFailed`
- `AdapterFailed`
- `HostOrSlotUnavailable`
- `EcsFinalizeFailed`
- `CleanupFailed`
- `StaleResult`
- `Internal`

可预留但不完整实现：

- `LoaderFailed`
- `ActionFailed`
- `RedirectLoop`

### 9.2 Error Boundary

Error boundary 从失败 route 向上寻找最近候选。

boundary 只覆盖其声明的 router-owned region。它不能销毁 app-owned shell。

如果 boundary view build/apply 失败，错误继续向上归因，并在 diagnostics 中保留 error chain。

Error boundary 不隐式修改 history，也不把 requested location 改写成 error route。是否 replace location 属于明确 navigation policy，MVP 默认不做。

### 9.3 Diagnostics Snapshot

diagnostics 应是稳定、可测试的快照。

至少包含：

- router instance。
- host。
- requested location。
- matched branch。
- active branch。
- pending/error state。
- route id chain。
- frame id/generation/status。
- slot identity。
- error kind/source。
- adapter name 或 renderer path。

diagnostics 面向测试和开发工具，不应泄漏应用业务数据，除非应用通过 extension 明确提供。

---

## 10. Public API 与 Extension 方向

### 10.1 MVP API

MVP API 采用 builder-first。Phase 0/1 需要冻结最低 API 层次：

- `bevy_ui_router_core::prelude`: raw location、route tree builder、ids、history、diagnostics。
- `bevy_ui_router_view::prelude`: renderer-agnostic view contract、view binding registry、fake/test renderer，按 feature 控制。
- `bevy_ui_router_ecs::prelude`: `RouteHost`、`RouteSlot`、plugin、schedule set labels。
- `bevy_ui_router_bsn::prelude`: immediate BSN adapter，feature-gated。
- `bevy_ui_router::prelude`: facade 常用 re-export，不改变底层 crate ownership。

Builder API 风格示例：

```rust
RouteTree::builder()
    .route(...)
    .index(...)
    .child(...)
    .pathless(...)
    .slot(...)
    .error_boundary(...)
    .build()
```

具体函数名在 Phase 0/1 冻结。这里表达 API 风格，而不是最终签名。

Builder error 类型必须能区分 duplicate route id、slot conflict、invalid segment、ambiguous catch-all、missing view binding key、missing error boundary reference 等声明期错误。错误名称可后续冻结，但分类必须可测试。

### 10.2 Raw Boundary

MVP public navigation 接收 raw `RouteLocation` 或可转换为 raw location 的简单类型。

typed route/search 后续以 codec/helper/macro 叠加在 raw boundary 上，不替代 raw boundary。

### 10.3 Extension Points

core extension 点：

- route meta extension。
- route location payload/extension。
- app error payload。
- diagnostics label。
- future typed codec。

extension 最低合同：

- 默认不参与 route matching。
- 默认不参与 params/search fingerprint。
- 默认不进入 history serialization。
- 默认不进入 diagnostics，除非应用提供 redacted label 或 serializer。
- 跨线程或 deferred 使用的 payload 必须满足相应 `Send`/`Sync`/`'static` 约束。
- `Debug`、`Clone`、serialization 不能被 core 默认为一定存在；需要通过 wrapper、trait object 或 app-provided mapper 显式表达。

extension 不能让 core 固定某个应用的权限、服务、主题、文案或 shell policy。app integration profile 是 core 外的组合模式，不是 core 类型。

---

## 11. Phase 0-3 实施路线

### 11.1 Phase 0: Contract Freeze

**目标**

- 建立 workspace 与 core 合同地基。
- 冻结最容易污染后续实现的 identity、generation、ownership、diagnostics 和 API 风格。

**前置条件**

- 接受当前仓库仍是单 crate 的事实。
- 承认目标 workspace 结构是迁移目标，而不是现有结构。
- 保持 core 不依赖 Bevy、BSN、platform crate 或 app profile 的依赖边界。

**范围**

- 当前单 crate 到 workspace 的迁移。
- Cargo workspace 拆分。
- facade crate 初始 feature 策略。
- `bevy_ui_router_core` 无 Bevy dependency。
- raw `RouteLocation`。
- `RouterInstanceId`。
- route/frame/slot id 基础类型。
- memory history 的 `push`、`replace`、`back`。
- requested/matched/active state separation 的最小模型。
- initial diagnostics snapshot。
- route-owned 与 app-owned boundary 的测试模型。
- pending overwrite 与 missing host/slot 的错误/诊断模型。
- public type ownership 表冻结。

**非范围**

- route matcher 与 branch diff 完整实现。
- Bevy ECS host/slot/frame root runtime。
- BSN adapter。
- typed route/search public API。
- async loader/action/revalidation、pending async guard、retained entity cache、transition runtime。

**产物**

- multi-crate workspace 骨架。
- `bevy_ui_router_core` 初始 API 与测试。
- facade feature 基线。
- public type ownership 冻结记录。
- core diagnostics 的最低身份链。
- Phase 1-3 的入口条件说明。

**测试资产**

- core-only unit tests：location、history、ids、diagnostics。
- ownership boundary fixtures：app-owned host 与 router-owned subtree 的区分。
- pending overwrite 与 missing host/slot diagnostics fixtures。
- compile or dependency boundary checks，证明 core 不依赖 Bevy/BSN/app concept。

**最低验证**

- `cargo fmt --all --check`
- `cargo test -p bevy_ui_router_core`
- `cargo check -p bevy_ui_router --no-default-features`

**退出标准**

- core crate 可以独立编译。
- core tests 不依赖 Bevy。
- API 不暴露 BSN 或 app-specific concept。
- diagnostics 能表达 router、host、slot、frame、generation 的最低身份链。
- facade default 不无条件拉入 Bevy，除非 Phase 0 明确记录相反取舍。
- Phase 1-3 的入口条件清楚。

### 11.2 Phase 1: Route Tree And Branch Model

**目标**

- 建立 route declaration、matching 和 branch diff。

**前置条件**

- Phase 0 workspace、core API 基线和 public type ownership 已冻结。
- raw `RouteLocation`、id 类型、history 和 diagnostics baseline 已可测试。
- facade default feature 基线已明确。

**范围**

- builder-first `RouteTree`。
- route id 和 declaration order。
- static segment。
- param segment。
- catch-all segment。
- index route。
- nested route。
- pathless layout。
- named slot。
- route metadata extension。
- error boundary declaration。
- `MatchedBranch`。
- params/search fingerprint。
- raw search canonicalization 规则。
- common-prefix branch diff。

**非范围**

- view apply、ECS hierarchy mutation 或 BSN scene apply。
- runtime frame lifecycle 完整状态机。
- error boundary rendering 行为。
- typed route/search public API。
- app payload 默认参与 matching/fingerprint/history/diagnostics。

**产物**

- `RouteTree` builder 与声明期错误分类。
- matcher 输出 `MatchedBranch`。
- branch diff 模型。
- stable ordering 规则。
- raw params/search fingerprint 合同。

**测试资产**

- matcher fixtures：static、param、catch-all、index、pathless、nested、conflict。
- branch diff fixtures：sibling、child、params/search change、layout change、slot change。
- raw fingerprint canonicalization fixtures：重复 key、顺序差异、空值、缺失值、percent-encoding normalization。
- builder declaration permutation、slot key conflict、catch-all ambiguity 反例测试。

**最低验证**

- Phase 0 验证。
- matcher/branch diff 合同测试。
- raw fingerprint canonicalization 测试。

**退出标准**

- matcher 合同测试覆盖 static、param、catch-all、index、nested、pathless、not-found、conflict。
- branch diff 测试覆盖 sibling route、child route、params change、layout change、slot change。
- stable ordering 有显式规则。
- builder declaration permutation、slot key conflict、catch-all ambiguity 有反例测试。
- matched branch 和 active frame 仍然分离。

### 11.3 Phase 2: Frame State And Error Boundary

**目标**

- 建立 runtime frame model、failure path 和 error boundary 行为。

**前置条件**

- Phase 1 matcher、branch diff、fingerprint 和 stable ordering 已有合同测试。
- requested/matched/active state separation 已存在。
- core 仍不依赖 Bevy、BSN 或 platform crate。

**范围**

- `RouteFrameId`。
- `RouteGeneration`。
- frame status。
- frame records。
- frame id reuse rules。
- generation increment rules。
- navigation transaction model。
- error taxonomy。
- nearest boundary resolution。
- build/apply failure state。
- stale result rejection model。
- pre-apply stale gate 与 pre-commit identity recheck。
- 最小 fake apply-result/test harness，用于验证 build/apply outcome。
- diagnostics frame snapshot。

**非范围**

- Bevy ECS host/slot runtime 和 hierarchy mutation。
- immediate BSN adapter。
- async loader/action/revalidation。
- pending async guard。
- retained entity cache runtime 和 transition runtime。

**产物**

- runtime frame records 与受控 frame id/generation 构造路径。
- navigation transaction 模型。
- error taxonomy baseline。
- nearest error boundary resolution。
- stale rejection 与 pre-commit recheck 合同。
- fake apply-result harness。
- diagnostics frame snapshot。

**测试资产**

- frame lifecycle/error/transaction fixtures。
- fake apply-result harness：build success/failure、apply success/failure、partial output、stale result。
- transaction fixtures：pending overwrite、old active retained until commit、failure cleanup scope。
- nearest error boundary fixtures。
- missing host/slot 错误映射 fixtures。
- adapter identity compile-fail 或等价可见性边界测试。

**最低验证**

- Phase 1 验证。
- frame lifecycle/error/transaction/fake apply-result 测试。
- adapter identity compile-fail 或等价可见性测试。

**退出标准**

- failed build/apply 进入明确 error path。
- boundary 只替换 router-owned region。
- stale result 不产生 visible state change。
- app-owned host 不被 cleanup。
- diagnostics 可追踪 error source、route id、frame id、generation。
- pending overwrite 使旧 transaction 失效。
- missing host/slot 进入 `HostOrSlotUnavailable` 或等价错误，不补建 app-owned host。
- adapter 不能构造或伪造 frame id/generation，至少有 compile-fail 或可见性边界测试。

### 11.4 Phase 3: View Adapter Interface And Immediate BSN Adapter

**目标**

- 证明 renderer-agnostic contract 和真实 Bevy/BSN 集成可用。

**前置条件**

- Phase 2 的 identity/generation/error/transaction/stale 合同稳定。
- fake apply-result harness 已能验证 apply outcome；active commit ordering 由 Phase 3B ECS finalization 与 core commit integration 验证。
- core lifecycle authority 不被 ECS 或 adapter 绕开。

**范围**

- `bevy_ui_router_view` renderer contract。
- fake/test renderer。
- `bevy_ui_router_ecs` plugin、host、slot、frame root marker。
- schedule sets。
- hierarchy apply/cleanup。
- visibility/pickability/focus finalization。
- immediate code-driven BSN adapter。
- minimal nested routes example。

**非范围**

- queued/asset BSN scene lifecycle。
- file-based BSN route scenes。
- hot reload。
- platform-specific back/deep-link packages。
- retained entity cache runtime。
- transition animation runtime。

**产物**

- `bevy_ui_router_view` renderer contract 与 fake/test renderer。
- `bevy_ui_router_ecs` plugin、host、slot、frame root marker 和 schedule sets。
- controlled ECS cleanup API。
- visibility/pickability/focus finalization。
- `bevy_ui_router_bsn` immediate adapter。
- minimal nested routes example。

**测试资产**

- view contract/fake renderer tests。
- ECS fixtures：host/slot missing、route-owned marker、cleanup boundary、focus clear/transfer、pickability/visibility convergence。
- BSN adapter error mapping tests。
- neutral example assertions：active branch、frame root markers、generation、error mapping。
- diagnostics snapshots：稳定结构，不包含 app payload，除非显式 redaction。

**最低验证**

- Phase 3A: view contract/fake renderer tests。
- Phase 3B: `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs` 加 `cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs`，或等价 ECS integration/facade wiring tests；必须覆盖 core-issued `FrameApplyRequest` handoff、controlled reporter/session、core-owned stale acceptance 和 active commit ordering。
- Phase 3C: `cargo check -p bevy_ui_router --features bsn`；BSN adapter error mapping tests。
- Phase 3D: minimal nested routes example 的 run/test 命令；diagnostics 和 frame marker 断言。

**退出标准**

- Phase 3A fake renderer 合同测试证明 build/apply success、failure cleanup、stale result 无 partial output、apply failure partial cleanup 与 plan provenance；Phase 3B ECS/core integration 测试证明 core-issued `FrameApplyRequest` handoff、controlled reporter/session、core-owned stale acceptance 和 active commit ordering。
- ECS 测试证明 ownership cleanup 不触碰 app-owned host。
- BSN example 证明 immediate scene apply 能挂载 nested route。
- BSN failure 映射为 Router error。
- view apply 和 ECS finalization 完成前不能 active。
- focus finalization 证明 active focus 不指向 replaced/leaving/failed/hidden subtree。
- example 有可执行验收命令，并断言 active branch、frame marker 与 diagnostics。

**建议里程碑**

- Phase 3A: view contract 与 fake renderer 完整化。
- Phase 3B: ECS host/slot/frame root、ownership cleanup、controlled reporter/session、core apply handoff 与 finalization。
- Phase 3C: immediate BSN adapter。
- Phase 3D: minimal nested routes example。

---

## 12. Phase 4-6 路线图

### 12.1 Phase 4: Loader, Guard, And Revalidate

路线图范围：

- typed route/search exploration。
- sync guard 是否实现由 Phase 0 冻结；pending guard 预留落地。
- route-scoped task contract。
- async loader/action/revalidation。
- task/load id。
- stale check 覆盖 router、frame、generation、task。
- redirect loop detection。
- revalidation dependency model。

进入条件：

- Phase 0-3 的 identity/generation/error/apply 合同稳定。

### 12.2 Phase 5: Cache, Modal, And Transition

路线图范围：

- route-local serializable state。
- retained entity cache runtime。
- cache key、pause/resume、eviction。
- modal/drawer frame kind。
- transition policy。
- entering/leaving coexistence。
- back policy composition。
- hidden subtree input isolation 与 focus release。

进入条件：

- ECS ownership、visibility、pickability 和 focus finalization 已有可靠测试。

### 12.3 Phase 6: Asset-Driven BSN And DevTools

路线图范围：

- `.bsn` asset route views。
- queued/asset scene lifecycle。
- asset pending/resolving/applying/active/error 边界。
- hot reload：重建 view frame，保留允许保留的 route state。
- diagnostics overlay。
- navigation trace replay。
- upstream Bevy/official BSN API migration review。

进入条件：

- immediate BSN adapter 已稳定，queued scene 不会破坏 active commit 规则。

---

## 13. 验证策略

### 13.1 Contract Tests First

每个 Phase 都必须有对应合同测试。测试应覆盖行为边界，而不是只覆盖 happy path。

Phase 0-3 重点：

- core isolation。
- history push/replace/back。
- route matching。
- branch diff。
- frame identity/generation。
- stale result rejection。
- ownership cleanup。
- failure cleanup。
- nearest error boundary。
- diagnostics snapshot。
- no undefined query order。
- adapter cannot bypass public API。

### 13.2 Minimum Verification Matrix

具体验证命令会随 workspace 拆分调整，但每个阶段必须保留同等级证据。

| 阶段 | 最低验证 |
| --- | --- |
| Phase 0 | `cargo fmt --all --check`；`cargo test -p bevy_ui_router_core`；`cargo check -p bevy_ui_router --no-default-features` |
| Phase 1 | Phase 0 验证；matcher/branch diff 合同测试；raw fingerprint canonicalization 测试 |
| Phase 2 | Phase 1 验证；frame lifecycle/error/transaction/fake apply-result 测试；adapter identity compile-fail 或等价可见性测试 |
| Phase 3A | view contract/fake renderer tests |
| Phase 3B | `cargo test --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs` 加 `cargo check --config "build.rustc-wrapper = ''" -p bevy_ui_router --features ecs`，或等价 ECS integration/facade wiring tests |
| Phase 3C | `cargo check -p bevy_ui_router --features bsn`；BSN adapter error mapping tests |
| Phase 3D | minimal nested routes example 的 run/test 命令；diagnostics 和 frame marker 断言 |

如果当前环境因自定义 Bevy git dependency、网络或平台限制无法运行某项验证，最终报告必须明确列出未运行原因，不能声称通过。

### 13.3 Shared Test Asset Plan

Phase 0-3 至少需要这些测试资产：

- core-only unit tests：location、history、ids、diagnostics。
- matcher fixtures：static、param、catch-all、index、pathless、nested、conflict。
- branch diff fixtures：sibling、child、params/search change、layout change、slot change。
- fake apply-result harness：build success/failure、apply success/failure、partial output、stale result。
- transaction fixtures：pending overwrite、old active retained until commit、failure cleanup scope。
- ECS fixtures：host/slot missing、route-owned marker、cleanup boundary、focus clear/transfer、pickability/visibility convergence。
- compile-fail 或 visibility-boundary tests：adapter 不能构造 frame id/generation 或 commit token。
- diagnostics snapshots：稳定结构，不包含 app payload，除非显式 redaction。
- neutral example assertions：active branch、frame root markers、generation、error mapping。

### 13.4 Fake Renderer Before BSN Confidence

Router 合同先由 fake/test renderer 验证。

BSN adapter 用于验证真实集成风险，但 BSN 失败不能推动 core 改成 BSN-specific 语义。

### 13.5 Neutral Examples

Phase 3 提供 minimal nested routes example。

示例要求：

- 使用中性 route/page 名。
- 不使用业务领域对象。
- 不引入品牌、主题、权限、数据服务。
- 同一 route tree 尽量覆盖 fake/test renderer 路径和 BSN adapter 路径。

### 13.6 Documentation Checks

文档变更应检查：

- 没有 placeholder。
- 没有内部矛盾。
- 没有 app-specific assumptions。
- 没有把 BSN 写成 core。
- 没有把 pending scene 写成 active UI。
- 没有遗漏 ownership、generation、stale check、failure cleanup。

---

## 14. 风险与开放问题

### 14.1 自定义 Bevy Git Dependency

当前项目依赖自定义 Bevy 0.19 git 分支。风险包括：

- CI 和外部复现更脆弱。
- 发布到 crates.io 的路径需要重新评估。
- BSN API 变动可能影响 adapter。

规划要求：

- core 不依赖 Bevy。
- Bevy-dependent crates 隔离该风险。
- Phase 6 或更早设置 upstream migration review。

### 14.2 Facade Default Feature

如果 facade default feature 拉入 Bevy，用户安装成本更高；如果不拉入 Bevy，初学入口可能少一些“一键可用”体验。

当前规划基线：

- facade 提供 ECS/BSN/platform re-export 路径。
- default feature 不拉入 Bevy。
- `ecs`、`bsn`、`platform` 显式 opt-in。

Phase 0 可以推翻该基线，但必须记录原因，例如“用户体验优先于默认依赖最小化”。在没有新决策前，文档按保守 opt-in 规划。

### 14.3 Guard In MVP

既有文档有时把 guard 作为 MVP error fallback 范围提到，有时又把 pending/async guard 放到非 MVP。

规划基线：

- Phase 0-3 只保留 guard 类型和 error taxonomy 预留。
- `GuardRejected` 在 Phase 0-3 默认是 reserved error kind，不要求可达行为。
- 如需实现，只允许最小同步 guard，不实现 pending async guard。
- 具体是否实现同步 guard 在 Phase 0 冻结。

### 14.4 RouteStateStore 与 Retained Entity Cache

两者必须概念分离。

Phase 0-3 只做术语和 identity 预留：

- route-local serializable state 属 core/state contract。
- retained entity subtree cache 属 ECS/runtime cache。
- 两者不能共用同一个隐式生命周期。

Phase 5 才实现 retained entity cache runtime、eviction、pause/resume 和 hidden subtree input/focus 规则。否则很容易过早引入 input isolation、focus release 和 eviction 复杂度。

---

## 15. 后续交付物

规划阶段完成后，应产生：

- 主规划文档。
- 多子代理审查报告摘要。
- 审查问题处置记录。
- 如需要，结构整理后的最终重写版。
- 后续 implementation plan，按 Phase 0-3 拆成可执行任务。

实现阶段开始前，还应补充：

- Public API draft。
- State machine diagram。
- Error taxonomy final draft。
- Adapter responsibility table。
- Test harness outline。
- Minimal examples outline。
