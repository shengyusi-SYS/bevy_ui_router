# 02 — Web Router Patterns For Bevy

**日期**: 2026-07-06  
**目标**: 提取 Web Router 生态中适合迁移到 Bevy UI Router 的结构和合同。

---

## 1. 总体目标

通用 Bevy UI Router 不应复制浏览器实现，而应吸收成熟 Router 的结构、生命周期和错误处理合同。

重点借鉴：

- route tree
- nested outlet
- layout route / pathless layout
- typed route / typed search
- loader / action / revalidate
- error boundary
- redirect / guard
- prefetch / preload

需要改造：

- 浏览器 URL 不能作为唯一 location source。
- DOM / HTML form 语义需要转为 ECS command 或 app service boundary。
- SSR、Server Components、HTTP request lifecycle 不应进入 core。
- React/Vue/Svelte 的组件缓存语义需要映射为 Bevy entity、Resource、Component 或 app-defined cache。

---

## 2. Web 生态可借鉴模式

| 生态 | 可借鉴模式 | Bevy 中需要改造 |
| --- | --- | --- |
| React Router | route module、嵌套路由、layout route、loader/action、error boundary、redirect、defer | 浏览器 history 不能作为唯一状态来源；DOM 表单语义需转为 ECS command 或 app service boundary |
| Remix | 路由即数据流、多级 loader 并行、action 后 revalidate、link prefetch、route boundary | SSR/HTTP 请求生命周期不应直接搬入本地 Bevy UI |
| TanStack Router | typed routes、typed search params、schema validation、beforeLoad、loader cache、preload、custom history | React 数据缓存生态需映射为 Bevy Resource、Component 或 app-defined cache |
| Next App Router | layout 持久化、page/loading/error/layout 分层、typed routes、prefetch | Server Components、文件系统强约定、SSR streaming 不应成为 Bevy Router core 依赖 |
| Vue Router | RouterView、RouterLink、navigation guards、route meta、named views、memory history | 动态组件缓存需改造成实体子树缓存、route state cache 或应用层策略 |
| SvelteKit | layout/page/error 层级继承、load、form actions、typed route data、parallel loading | 文件系统路由可借鉴，但 Rust crate 不应强依赖目录结构约定 |

---

## 3. Route Tree + Outlet

通用 Bevy UI Router 应采用 route tree 到 matched branch 的模型：

```text
Location
  -> match branch [root, workspace, section, detail]
  -> diff old/new branch
  -> preserve common prefix layouts
  -> replace changed outlet subtree
```

核心需求：

- route tree 支持 index route、nested route、pathless layout 和 named outlet。
- Router 渲染的是 matched branch，而不是单个页面。
- branch reconciliation 保留公共前缀 layout，只替换变化的 outlet/slot。
- route-owned frame subtree 与 app-owned persistent shell 边界清晰。
- active location、matched branch、active rendered branch 分离。

---

## 4. Layout Route 与 Pathless Layout

Pathless layout 不一定对应路径片段，但必须参与 branch reconciliation、生命周期和 outlet 挂载。

示例：

```text
RootLayout
  |- WorkspaceLayout
  |  |- IndexPage
  |  `- DetailPage
  `- SettingsLayout
     |- AccountPage
     `- ShortcutsPage
```

需求：

- layout route 可声明一个或多个 outlet。
- pathless layout 不改变 path，但能提供持久 UI 容器。
- layout 被保留时，其局部状态、焦点恢复信息和已加载缓存可继续存在。
- layout 被移除时，只清理 router-owned subtree，不触碰应用拥有的外部结构。

---

## 5. Loader / Action / Revalidate

| Web 概念 | 通用 Bevy 映射 |
| --- | --- |
| loader | route enter 前后启动的 route-scoped async task，产出 route-local state、Resource 更新或 render input |
| action | 用户交互触发的 mutation command，通过 app service boundary 执行副作用 |
| revalidate | mutation 完成或外部事件到达后，对声明依赖的 loader 精准刷新 |
| defer / Suspense | 保留 layout 和已有 frame，在 outlet 内显示局部 loading/fallback |
| redirect | loader、guard 或 action 返回新的 navigation intent |
| beforeLoad / guard | 进入 route 前执行同步或异步准入判断 |

约束：

- loader/action 不得持有 `&mut World` 跨 await。
- 异步结果写回必须校验 router id、frame id、generation、load id。
- stale result 必须丢弃，不能覆盖新 route frame。
- revalidation 必须基于声明依赖，避免无边界刷新。
- loader 错误应交给最近的 route error boundary。

---

## 6. Error Boundary

Web error boundary 可映射为 route branch 中最近的 `error_view` 或 fallback renderer。

```text
root boundary
  workspace boundary
    detail boundary
      detail fallback
```

需求：

- matcher、guard、loader、view build、view apply、internal error 有可区分来源。
- route error 只替换最近 boundary 覆盖的 outlet。
- error boundary 不破坏 history。
- error boundary 不销毁 app-owned persistent shell。
- 允许挂载 app-defined error payload，但 Router core 不固定业务错误枚举。

---

## 7. Typed Routes 与 Typed Search

Rust 适合把路径和查询参数提升为类型安全 API，同时保留 string path 作为边界格式。

```rust
#[derive(Clone)]
enum AppRoute {
    Dashboard,
    Project { id: ProjectId },
    Document { project: ProjectId, document: DocumentId },
    Settings { section: SettingsSection },
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ProjectSearch {
    query: String,
    sort: SortOrder,
    page: u32,
}
```

需求：

- 支持 typed route 到 string location 的双向转换。
- 支持 typed search params 与 schema validation。
- 支持 custom path codec。
- Router core 只关心解析、匹配、比较和生成 location。
- app-specific meta 通过 typed extension 或 profile 注入，不进入 core 字段。

---

## 8. Prefetch / Preload

Web 的 hover 和 viewport prefetch 在 Bevy 中应扩展为更通用的预热机制：

- pointer hover。
- focus hover。
- 键盘或手柄焦点即将移动到某个可导航项。
- 列表中可见项的详情预热。
- 最近访问 route 的 loader cache 保温。
- deep link 或外部 navigation intent 到达前的 guard 预检。

约束：

- prefetch 不创建可见 UI entity。
- prefetch 只准备数据、资源、判断结果、asset 或 render input。
- prefetch 结果必须有 cache key、generation 和失效策略。
- prefetch 失败不应进入 error boundary，除非用户真正导航到该 route。

---

## 9. Bevy 额外需求

| 需求 | 说明 |
| --- | --- |
| Location abstraction | 支持 memory location、custom URI、多窗口独立 location source |
| Router instance | 每个 host 可拥有独立 RouterInstance |
| RouteHost / RouteSlot | 明确 route frame 挂载点，支持 nested outlet 和 named outlet |
| Frame lifecycle | 至少区分 matching、building、active、leaving、error |
| Entity ownership | route-owned subtree 必须可追踪、可清理、可缓存 |
| Async stale guard | 所有异步写回都需要 generation 校验 |
| Focus management | route transition 后恢复或指定 focus owner |
| Input/back intent | 抽象为 BackIntent、BackPolicy、BackCapabilitySnapshot |
| Transition | entering/leaving frame 可短暂共存，输入和层级必须可控 |
| Diagnostics | 提供 branch、frame、loader、cache、error 的稳定快照 |

---

## 10. 不应照搬的 Web 模式

- 不把浏览器 URL 当作唯一真相。
- 不引入 SSR、Server Components 或 HTML request/response lifecycle。
- 不复制 DOM form action。
- 不强制文件系统路由。
- 不使用整页 error page。
- 不使用全局 loader 阻塞所有渲染。
- 不做无边界自动 revalidate。
- 不把主题、文案、权限、业务错误类型写进 Router core。
- 不把 view adapter 当成 Router 状态机。

---

## 11. 最小需求草稿

MVP 可以聚焦：

1. Route tree、matcher、index route、nested route、pathless layout。
2. RouterInstance、memory location、push、replace、back。
3. RouteHost、RouteSlot、route-owned frame root marker。
4. old/new matched branch diff，并保留 common prefix layout。
5. frame lifecycle 与 diagnostics。
6. code-driven view adapter interface。
7. fallback/error view，覆盖 guard、view build、view apply 失败。
8. 基础 back intent 抽象。
9. matcher、branch reconciliation、owned subtree cleanup、diagnostics snapshot 的合同测试。

暂不进入 MVP：

- async loader/action/revalidate。
- async guard pending。
- retained entity cache。
- transition animation。
- asset-backed route view。
- hot reload。
- 平台专用 back 集成。
- app-specific profile。

