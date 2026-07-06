# 05 — Non-MVP Capabilities In Bevy

**日期**: 2026-07-06  
**目标**: 说明非 MVP 能力在纯 Bevy UI Router 环境中的实际使用场景、必要性和架构预留方式。

---

## 1. 判断原则

非 MVP 不等于不重要。

对于通用 Bevy UI Router，需要区分三件事：

1. **首版是否实现**：MVP 是否需要交付完整功能。
2. **架构是否承认**：核心状态机、实体所有权、错误模型是否必须为它留位置。
3. **后续是否可插拔**：未来实现时是否能作为 adapter、policy、profile 或 feature 加入，而不推翻 core。

Bevy 环境不一定需要完整 Web Router 功能，但 Bevy 更强调 ECS 生命周期、异步资源、asset loading、entity ownership、input/focus 边界和 schedule 时序。凡是会改变这些边界的能力，即使不进 MVP，也必须预留。

---

## 2. 总览矩阵

| 能力 | Bevy 中的典型场景 | 是否真的需要 | 架构影响 | MVP 预留方式 |
| --- | --- | --- | --- | --- |
| async loader/action/revalidation | 文件浏览、设置保存、编辑器项目加载、存档列表、资源元数据读取、在线房间列表 | 通用框架很需要 | 影响 frame generation、error boundary、result apply、diagnostics | 定义 route-scoped task contract、generation、stale token、apply phase |
| pending async guard | 未保存修改确认、资源就绪检查、运行时能力检查、会话状态检查 | 中大型 UI 需要 | 影响 navigation pipeline，不能假设 guard 同步完成 | `GuardOutcome` 预留 `Pending`，frame state 预留 `Guarding` |
| transition runtime | 页面切换动画、modal 进入退出、输入屏蔽、焦点转移 | 体验型需求，不是所有应用需要 | 影响 entering/leaving frame 共存、visibility、pickability、focus | MVP 实现 no-op policy，但保留 `Entering` / `Leaving` 概念 |
| retained entity cache | 返回列表保留滚动、复杂面板保留选择状态、昂贵 UI 避免重建 | 工具、编辑器、复杂 UI 需要 | 影响实体生命周期和输入隐藏策略 | 先定义 cache contract，与 `RouteStateStore` 分离 |
| queued/asset BSN scene | 页面依赖图片、字体、scene asset、未来 `.bsn` 文件 | BSN 路线下很重要 | 影响 resolving/applying/active/error 边界 | BSN adapter 区分 pending 与 active |
| platform adapters | 系统返回、桌面快捷键、手柄返回、deep link、自定义 URI、多窗口输入 | 跨平台框架需要 | 影响 back、location source、router instance targeting | Core 只接收 `BackIntent` 和 location updates |
| app integration profile | 应用主题、文案、权限、数据服务、宿主 shell | 通用框架必须有边界 | 防止业务字段污染 core | `RouteMeta` extension 与 `RouteError` app payload |
| file-based BSN scenes | 外部 `.bsn` 文件维护 UI，设计工具链输出 scene | 可选 | 影响 view source abstraction | View adapter 不写死 Rust function |
| hot reload | 开发时快速调整 UI、工具面板热替换 | 可选增强 | 影响 observer、cleanup、state preservation | 先约定“重建 view + 保留 state”，不做 runtime patch 假设 |
| derive macro/codegen | typed routes 易用性、减少样板代码 | 开发效率需求 | 不影响 core 语义 | Public API 必须可手写实现 |
| forward navigation | 浏览器式前进、历史 cursor、桌面工具导航 | 部分应用需要 | 影响 history model | `LocationStore` 不封死为单向栈 |
| multi-window persistence | 多窗口编辑器、多 surface UI、独立面板 | 专业工具需要 | 影响 RouterInstance 与 location ownership | 预留 `RouterInstanceId`，持久化后置 |

---

## 3. 必须预留的能力

### 3.1 Async Loader / Action / Revalidation

#### 使用场景

- 页面进入时读取文件系统目录。
- 设置页保存配置后刷新局部状态。
- 编辑器打开项目时加载资源图谱。
- 存档列表读取磁盘元数据。
- 网络房间列表或账号信息刷新。
- asset metadata 或索引数据延迟加载。

#### 为什么 Bevy 需要

Bevy UI 运行在 ECS schedule 中。许多数据来源不能在构建 UI 的同一帧同步完成。如果 Router 假设 view build 必须立即拿到所有数据，就会迫使应用把异步状态散落在 view observer 或全局 resource 中，后续很难取消、重试、诊断和防止 stale 写回。

#### 架构预留

- `RouteFrameId`
- `RouteGeneration`
- task/load id
- result apply phase
- stale-result rejection
- loader/action error boundary
- revalidation dependency
- diagnostics trace

MVP 不需要实现完整 loader，但必须让 frame lifecycle 可以表达“view 已挂载，data 未完成”或“data 失败，进入 fallback”。

### 3.2 Pending Async Guard

#### 使用场景

- 离开编辑页面前确认未保存修改。
- 进入页面前等待资源或会话状态就绪。
- 检查运行平台是否支持某个 feature。
- 读取配置后决定 allow、redirect、block。

#### 为什么 Bevy 需要

Bevy 应用经常有运行时能力差异、资源加载阶段和用户确认流程。如果 guard 只能同步返回，应用会绕开 Router 自行阻止导航，导致 history、diagnostics 和 back policy 不一致。

#### 架构预留

- `GuardOutcome::Pending`
- `GuardOutcome::Redirect`
- `GuardOutcome::Block`
- `RouteFrameStatus::Guarding`
- redirect loop detection
- navigation cancellation policy

MVP 可以只实现同步 guard，但类型和状态机不能排斥 pending。

### 3.3 Queued / Asset BSN Scene

#### 使用场景

- route view 依赖图片、字体、主题 asset 或外部 scene asset。
- 后续支持 `.bsn` 文件作为 view source。
- 大型 UI 分块加载。

#### 为什么 Bevy 需要

Bevy 的 asset loading 是异步的。`queue_spawn_scene` 返回 entity 不代表 UI 已经 apply，也不代表可交互。如果 Router 把 entity 创建等同于 route active，back、focus、transition、error boundary 都会出现竞态。

#### 架构预留

- `ResolvingView`
- `ApplyingView`
- `Active`
- `ViewApplyFailed`
- queued scene completion token
- partial frame cleanup

MVP 可以只支持 immediate code-driven `bsn!`，但 BSN adapter 的状态边界必须从第一版就正确。

### 3.4 Platform Adapters

#### 使用场景

- 系统返回键或手势。
- 桌面快捷键。
- 手柄返回按钮。
- deep link / custom URI。
- 多窗口或多 surface 的输入路由。

#### 为什么 Bevy 需要

Bevy 的输入来源多样。Router core 如果直接写死某个平台的 back 行为，就会限制桌面、移动、主机、嵌入式 UI 和多窗口工具。

#### 架构预留

- `BackIntent`
- `BackPolicy`
- `BackCapabilitySnapshot`
- location source abstraction
- `RouterInstanceId`

MVP 可只实现 memory history 和基础 back，但 core 应只消费抽象 intent，不直接依赖平台 API。

### 3.5 App Integration Profile

#### 使用场景

- 应用定义自己的主题、文案、权限、数据加载、错误 payload。
- 应用有持久 host shell，不希望普通 route 销毁。
- 应用需要把 Router diagnostics 接入自身调试工具。

#### 为什么 Bevy 需要

通用框架不可能预知所有应用的数据、主题、权限和 UI 宿主策略。如果这些字段写进 core，Router 会迅速变成某一类应用专用框架。

#### 架构预留

- `RouteMetaExtensions`
- app-defined guard dependency
- app-defined error payload
- renderer convention hook
- app-owned host vs router-owned subtree contract

MVP 必须有 extension 机制，即使不提供任何具体 profile。

---

## 4. 架构要承认，但首版可 no-op 的能力

### 4.1 Transition Runtime

#### 使用场景

- 页面淡入淡出。
- modal、drawer、panel 进入退出。
- 旧 frame 离开期间禁止输入。
- 新 frame active 后恢复焦点。

#### 是否必须

小型 UI 可以完全不需要动画。但 transition 会影响 frame 共存、z-order、input、focus、visibility 和 back intent。如果首版完全没有 `Entering` / `Leaving` 概念，后续接入 transition 会重写 frame lifecycle。

#### 预留方式

- `TransitionPolicy::None` 作为 MVP 默认。
- frame lifecycle 保留 `Entering`、`Leaving`。
- focus/pickability/visibility finalization 独立成阶段。

### 4.2 Retained Entity Cache

#### 使用场景

- 返回列表后保留滚动位置和局部展开状态。
- 复杂编辑面板避免重新创建大量 entity。
- tab 或 nested route 切换时保留昂贵 subtree。

#### 是否必须

不是每个应用都需要保留 entity。但 `RouteStateStore` 与 retained entity cache 必须从概念上分离。否则后续会把可序列化状态和 runtime entity 混成一个系统。

#### 预留方式

- `RouteStateStore` 只负责可序列化状态。
- retained entity cache 只负责 runtime entity subtree。
- route frame 定义 cache key、pause/resume、eviction hooks。

### 4.3 Forward Navigation

#### 使用场景

- 桌面工具的历史前进。
- 浏览器式 navigation cursor。
- 文档/项目界面回退后再前进。

#### 是否必须

许多游戏菜单不需要 forward。桌面工具更可能需要。MVP 可不实现，但 history model 不应封死为只会 pop 的 Vec。

#### 预留方式

- `LocationStore` 概念上支持 cursor。
- `back` 不直接删除所有 future entries，除非 policy 规定。
- diagnostics 可表达 history capability。

### 4.4 Multi-Window Persistence

#### 使用场景

- 多窗口编辑器。
- 多个独立 UI surface。
- detached panel。
- split view。

#### 是否必须

单窗口应用不需要。但通用框架应至少预留 `RouterInstanceId`，否则后续多窗口事件、history、focus、diagnostics 都会缺少归属。

#### 预留方式

- 每个 host/surface 绑定 `RouterInstanceId`。
- Platform adapter 事件必须带目标 router。
- 持久化策略后置。

---

## 5. 可以后加的能力

### 5.1 File-Based BSN Scenes

这是开发流程和资源组织能力，不是 Router core 能力。

只要 view adapter interface 不写死为 `fn(...) -> impl Scene`，后续即可加入 file-based scene source。

### 5.2 Hot Reload

Hot reload 是开发体验能力。

推荐预设策略：

- 不对旧 entity tree 盲目 runtime patch。
- 重建 view frame。
- 保留 `RouteStateStore` 中可序列化状态。
- 清理旧 observer 与临时 entity。

### 5.3 Derive Macro / Codegen

Typed route derive 和 route codegen 能显著改善使用体验，但不应成为 core 的唯一入口。

要求：

- Public API 必须可手写实现。
- Macro 只是生成 route definition、params/search parser、typed navigation helper。

---

## 6. 推荐决策

### 6.1 MVP 必须实现

- route tree
- matcher
- memory history
- `push` / `replace` / `back`
- `RouteHost` / `RouteSlot`
- frame id / generation
- branch reconciliation
- immediate BSN adapter
- view apply failure cleanup
- basic error fallback
- diagnostics snapshot

### 6.2 MVP 必须预留但不实现完整功能

- async loader/action/revalidation
- pending async guard
- queued/asset scene lifecycle
- transition lifecycle
- retained entity cache contract
- platform adapter boundary
- app integration profile
- multi-router instance identity

### 6.3 可后续添加

- file-based BSN scenes
- hot reload
- derive macro/codegen
- forward navigation implementation
- multi-window persistence
- DevTools overlay
- navigation trace replay

---

## 7. 设计底线

最危险的 MVP 简化是：

```text
match location -> spawn view -> mark active
```

这个模型无法自然支持异步数据、queued scene、error boundary、transition、back intent、focus finalization 和 stale-result rejection。

更安全的最小模型是：

```text
request location
  -> match branch
  -> create/update frame
  -> build/resolve/apply view
  -> finalize visibility/pickability/focus
  -> commit active branch
```

首版可以让很多阶段是 no-op，但状态机必须承认这些阶段存在。

---

## 8. 结论

在纯 Bevy 环境下，并不是所有 Web Router 能力都必须实现。但通用 Bevy UI Router 必须从第一版就设计好：

- entity ownership
- async stale safety
- asset pending boundary
- route-local state vs retained entity cache
- back intent abstraction
- app-owned host vs router-owned frame subtree
- extension points for app-specific profile

这些不是功能膨胀，而是防止框架后续被真实 Bevy 使用场景推翻的架构地基。

