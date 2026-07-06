# 04 — Risks, Tests, And Handoff

**日期**: 2026-07-06  
**目标**: 给外部独立开发者提供风险、测试、阶段路线、交接边界和脱敏检查清单。

---

## 1. 关键风险

| 风险 | 约束 / 验收标准 |
| --- | --- |
| 把 BSN 当作 Router core | BSN 只负责 view 描述、解析与 apply；Router 状态机必须独立存在 |
| 异步结果 stale 写回 | loader/action 写回必须校验 router、frame、generation、load token |
| 嵌套路由退化为单 outlet | matcher 返回 matched branch；更新时执行 common-prefix diff |
| 实体所有权不清 | 每个 route frame root 必须有 owner marker；Router 只管理 router-owned subtree |
| 错误边界过粗 | 使用最近父级 error boundary；普通 frame 错误不得销毁 app-owned shell |
| transition 与 back intent 冲突 | transition 状态机必须支持 busy、cancel、timeout 和稳定 snapshot |
| hot reload 重复挂载 observer | 首版应 feature-gate；重载策略为重建 view、保留可序列化 state |
| state 与 entity cache 混淆 | route-local state 与 retained entity subtree cache 必须分离 |
| loader 直接操作 UI entity | 异步任务不得跨 await 持有 world 可变访问；写回集中到 apply phase |
| 测试只覆盖 happy path | 建立 matcher、branch、frame、back、error、cache、diagnostics 的黑盒矩阵 |

---

## 2. 框架级测试矩阵

| 维度 | 必测用例 |
| --- | --- |
| Matcher | static、param、catch-all、index、pathless、not-found、conflict |
| Route tree | nested branch matching、pathless layout lifecycle、named slot conflict |
| Branch reconciliation | sibling route、child route、params change、layout change、common-prefix retention |
| Frame lifecycle | matching、building、active、leaving、error、cleanup |
| Entity ownership | frame root marker、owned subtree cleanup、no orphan child |
| Error boundary | leaf boundary、parent boundary、root fallback、retry |
| Loader/action | success、error、stale-ignore、cancel、parallel、revalidate |
| Guard | allow、redirect、block、pending、error、loop detection |
| State/cache | frame id cache、route plus params cache、eviction、hidden input/focus behavior |
| Back intent | modal priority、history pop、blocked back、empty stack exit request |
| Transition | enter interrupted、leave interrupted、back during transition |
| Diagnostics | stable snapshot、navigation trace、error attribution |
| UI constraints | route root layout constraints、no full host rebuild、visibility/pickability/focus convergence |

---

## 3. BSN Adapter 测试

- immediate `bsn!` scene apply 成功后，frame 才能进入 active。
- `Scene`、`SceneList`、`Children` 挂载后必须位于 router-owned host 下。
- queued scene 必须显式表达 pending 与 active 边界。
- scene resolve failure 必须映射为 route view/apply error。
- asset missing 或 asset removed 必须进入 adapter error path。
- apply failure 后不得残留未归属 frame root。
- hot reload 不得重复注册 observer。
- BSN adapter 不得持有 location、history、loader 状态机职责。

---

## 4. Platform Adapter 测试

- back intent source 只生产 intent，不直接修改 Router 内部状态。
- `BackCapabilitySnapshot` 在 transition、loading、error、modal 状态下稳定可读。
- 移动端系统返回、桌面快捷键、手柄返回等输入映射到同一抽象。
- custom URI / deep link 只作为 location source，由 core 统一匹配。
- 多窗口场景下，平台事件必须路由到明确的 Router instance。
- 生命周期恢复时，initial route 可延迟提交且不破坏 history。
- platform adapter 不得承载业务权限、页面数据加载或应用渲染约束。

---

## 5. 实现阶段路线

### Phase 0: Contract Freeze

- 建立 push、replace、back、pending overwrite、missing slot 黑盒测试。
- 建立 route root layout constraint 测试。
- 明确 app-owned shell 与 router-owned frame subtree 边界。
- 暂不引入 BSN 迁移，先锁住已有行为。

### Phase 1: Route Tree And Branch Model

- 引入 `RouteNode`、`RouteTree`、index route、pathless layout。
- 用 `MatchedBranch` 表达当前匹配结果。
- branch 更新采用 common-prefix diff。
- 内部模型支持多个 route slot。

### Phase 2: Frame State And Error Boundary

- 分离 current location、matched branch、active rendered branch。
- 引入 frame id、generation、frame status。
- 建立 route frame root marker。
- build/apply 失败进入最近 error boundary。
- diagnostics 能稳定描述每个 frame 的状态。

### Phase 3: View Adapter Interface And BSN Adapter

- 定义 view description / render plan 与 renderer apply 的职责边界。
- BSN adapter 作为首个 view adapter。
- 同一 route tree 允许不同 view adapter 共存。
- view build/apply failure 必须可清理、可诊断。

### Phase 4: Loader, Guard, And Revalidate

- 引入 typed params/search。
- loader/action 使用 stale token 与 apply phase。
- guard pipeline 支持 allow、redirect、block、pending、error。
- revalidation 由 route-scoped dependency 描述，不绑定应用通信协议。

### Phase 5: Cache, Modal, And Transition

- 分离 route-local serializable state 与 retained entity cache。
- 定义 modal/drawer frame kind。
- 实现 pause/resume lifecycle。
- transition policy 与 back policy 必须可组合、可测试。

### Phase 6: Asset-Driven BSN And DevTools

- 在相关能力稳定后支持 `.bsn` asset route view。
- hot reload 策略为重建 view、保留 state。
- 提供 diagnostics overlay 与 navigation trace replay。
- asset apply failure 进入统一 error path。

---

## 6. 外部独立开发交接边界

| 层 | 应交付 | 不交付 |
| --- | --- | --- |
| Router core | route tree、matcher、location/history、matched branch、frame lifecycle、state/cache contracts、diagnostics | 具体应用页面、业务权限、应用数据服务 |
| Bevy ECS integration | plugin、schedule sets、route host/slot、frame root marker、hierarchy mutation、visibility/pickability/focus 收敛 | 宿主应用 shell 结构 |
| View adapter interface | view description、render plan、renderer apply contract、failure cleanup | 具体业务 view 类型 |
| BSN adapter | code-driven scene apply、scene resolve/apply error mapping、queued scene 边界 | Router 状态机 |
| Platform adapter | back intent source、deep link source、多窗口 location source | 业务 guard 与数据加载 |
| App integration profile | 由宿主应用自行定义的 meta、主题、文案、权限、数据与 shell 策略 | 通用 core API |

交接材料至少应包含：

- Public API draft。
- State machine diagram。
- Error taxonomy。
- Test harness 入口说明。
- Minimal examples，且只使用中性页面。
- Known non-goals。
- Adapter responsibility table。
- Desensitization checklist。

---

## 7. 禁止把 App-Specific Profile 写进 Core

- `RouteMeta` 的 core 字段只保留 cache policy、transition policy、history policy 等 Router 算法配置。
- 应用字段通过 typed extension 或 app-specific profile 注入。
- `RouteError` 只固定 matcher、guard、view build、scene resolve/apply、loader、internal 等来源。
- app-defined error payload 必须可挂载，但不能成为 core enum 的固定业务分支。
- loader/action 是 route-scoped task contract，不是应用通信协议。
- back 是 intent 和 policy 抽象，不绑定任何单一平台。
- app-owned persistent shell 与 router-owned frame subtree 必须分离。
- 示例不得包含具体业务领域对象、专有组件名或内部服务名。

---

## 8. 脱敏检查清单

- 未出现任何具体产品名、仓库名、本地绝对路径或组织内部路径。
- 未出现宿主应用专有组件名、页面名、资源名或迁移债务名。
- 未出现内部通信协议、发送器、事件通道的专有名称。
- 未出现应用主题 token、文案 key、翻译文件格式的专有名称。
- 未出现具体业务领域例子。
- 未出现具体 route 数量。
- 所有权限、数据加载、错误类型均以 app-defined 或 route-scoped 抽象表达。
- BSN 只作为 view adapter 出现，不作为 Router core。
- 平台行为只通过 platform adapter 与 intent source 表达。
- app-specific profile 只作为边界概念出现，不写入 core API。

