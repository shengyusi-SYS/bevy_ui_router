# Scene 与挂载合同

[中文](scene.zh-CN.md) | [English](scene.md)

本页说明 factory 注册、同步 Scene 创建和嵌套页面的挂载。接入步骤见[使用指南](../guides/integration.zh-CN.md)，host 与发布规则见[生命周期](lifecycle.zh-CN.md)。

## 注册与运行装配

推荐用 `BsnRouter::new(root).fallback(factory)` 与 `AppBsnExt::add_bsn_router` 接入。`page(path, factory)` 声明有路径的页面或布局，`layout(factory)` 声明 pathless 布局，`index(factory)` 声明默认子页；`.children(...)` 保持显式层级，`.error_view(factory)` 在该节点直接绑定错误页。每份声明只有一个显式 root，库不补建隐藏父层。

声明编译成原有 Core 路由树、View registry 与 BSN factories。构造和安装不调用 factory；pattern/tree 错误保留原 `RouteError` 和声明位置。生成的 driver key 检查当前注册空间，多个 Router 可与手工注册共存。验证或安装失败不留下本批 factory 注册，成功后仍使用同一运行管线。

应用在自有 host 上添加 `RouterHostRegistration`。可用 `.initial(location)` 指定默认首屏；后续通过 Commands 或即时 `RouterNavigation` 导航，从 snapshots 观察结果。入队及首屏规则见[导航合同](navigation.zh-CN.md#请求history-与-presentation)。

| Feature | 提供的能力 |
| --- | --- |
| `default` / `view` | 渲染无关的视图模型，保持纯 Rust 依赖 |
| `ecs` | Bevy ECS 集成 |
| `bsn` | 官方 Scene bridge，包含 `ecs` |

高级显式接入仍可通过 `register_bsn_view`、`RouteViewRegistry` 与 `RouterConfig` 自行建立映射。driver key 共用注册命名空间，重复注册同步报错、保留原 factory，该次注册不调用 factory。`RouteViewRegistry::insert` 同样拒绝重复 view key，保留原 descriptor。安装时验证 normal view、error view、fallback 和 driver binding；按节点声明顺序检查，同节点先检查 normal view。

当前注册 API 用于装配期，动态 unregister/rebind 留待后续扩展；窗口、相机、字体、渲染配置和 asset 路径由应用配置。公开入口见 [App extension](../../crates/bevy_ui_router_ecs/src/app_ext.rs)。

## Scene profile 与创建责任

声明入口与 `AppBsnExt::register_bsn_view` 接收相同的同步 factory：输入为只读 `RouteViewInput`，返回一个新 Scene 或 `BsnBuildError`。fresh 路径每个 view effect 调用一次 factory，并单次消费 Scene。错误呈现使用自己的 factory 调用和身份；keep-alive 命中直接复用已经应用的实体。

factory 输入包含 requested location、matched route、route-local params、raw search、fragment、origin，以及错误尝试的原始 failure。factory 通过这些普通数据构建声明；World、Commands、Entity、slot、frame/generation 和 lifecycle 权限由 ECS 执行层持有。

支持代码组合的 native Scene、标准 `ChildOf` 声明图、component/bundle/fallible templates、图内命名引用和资源 handles。cached Scene asset inclusion（含已加载 patch）在 UI 分配前返回错误；依赖未就绪在同步执行中返回失败。非 `ChildOf` 输出和 template 主动创建的声明闭包外实体由应用管理。

同步 Scene 也需要 `AssetServer` 和 `Assets<ScenePatch>`。最小环境可安装 `TaskPoolPlugin`、`AssetPlugin`、`ScenePlugin`。含 BSN binding 的 Router 缺少这些资源时，底层安装返回 `MissingSceneRuntime`，声明入口通过 `BsnRouterError::Install` 保留该错误；资源在运行期间撤销时，在调用 Scene 前返回明确失败。

执行流程是：官方 resolve → 预检声明图 → 预分配并登记 exact Entity → 用私有 shell wrapper 和引用 seed 应用原 Scene。原 root/child 名称继续指向原内容 Entity，匿名节点使用本次 attempt 的唯一引用。Scene 的 component 值和声明的 Children 顺序保持，由官方 relationship callbacks 执行关系写入。

声明 Entity 提前分配；读取声明图的官方 getter 可能给 leaf 添加空 `Children` 并触发 hook。错误报告保留原来源。提前登记覆盖了 apply 在建立 `ChildOf` 前失败的情况，依赖 handles 持有到真实 apply/settlement 完成。

apply、adoption 和即时失败隔离在同一 exclusive 调用中完成。创建义务覆盖尚未挂接的部分 UI，在返回普通系统或下一输入/呈现边界前完成隔离，之后继续安全清理。实现见 [BSN preparation](../../crates/bevy_ui_router_ecs/src/bsn.rs) 和 [native Scene executor](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs)。

## Frame 内容与 default outlet

实体层级为：应用 host → top slot → FrameShell → FrameContentTarget → Scene 内容。嵌套页面通过父内容中的 outlet、child slot 挂载下一层 frame。

Router 管理 shell 的布局与隔离；Scene 定义内容根的普通 `Node`、`Visibility`、`Pickable` 等值，生命周期切换保留这些 authored 值。

有 child route 的 layout 声明唯一 `DefaultOutlet`，位于内容根或内部 `Node`；leaf 可以省略。缺失、重复、非容器或关系失效产生 mount failure。父 layout 复用时，同一个 outlet anchor 和 child slot 一起保留；marker 声明和实际 binding 分别管理，移动 marker 会按现有 binding 校验。

消费 mount 前复核 parent/frame generation、anchor、slot 和关系。entering parent 的无效 outlet 属于 pending rejection；已提交 preserved parent 丢失 binding 属于 authority loss，分别进入对应的失败路径。

每个 frame 的内容遍历在 child slot 处停止，嵌套 frame 按自身身份、role 和 binding 管理。具体创建与清理责任见[生命周期](lifecycle.zh-CN.md#cleanup-与内容边界)，pending focus 见[输入与焦点](input-focus.zh-CN.md)。
