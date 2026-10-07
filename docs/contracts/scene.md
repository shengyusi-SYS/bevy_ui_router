# Scene 与挂载合同

本页定义 factory 注册、同步 Scene 执行范围及 default outlet。host/owner 与 publication 见[生命周期](lifecycle.md)，pending focus 处理见[输入与焦点](input-focus.md)。使用步骤见[接入指南](../guides/integration.md)。

## 注册与运行装配

应用先注册 view driver，在 `RouteViewRegistry` 中建立 view 到 driver 的映射，以 `RouterConfig` 安装 Router，再在自有 host 添加 `RouterHostRegistration`。导航命令经 `RouterNavigation` 入队，结果经 snapshots 观察。

`default`/`view` 不拉入 Bevy；`ecs` 启用 ECS 集成；`bsn` 显式启用官方 Scene bridge。开启编译 feature 不等于运行环境已经就绪。

所有 driver key 共用注册命名空间。重复 key 同步拒绝，不替换旧 factory，也不调用 factory。安装同步校验 normal view、error view、fallback 和 driver binding；node 按声明顺序检查，同 node 的 normal view 缺失先于 error view 缺失。

`RouteViewRegistry::insert` 同样拒绝重复 view key，保留原 descriptor；view key 与 driver key 是不同映射层，不能用重复插入实现 rebind。

MVP 不提供动态 unregister/rebind。窗口、相机、字体、渲染配置与 asset 路径由应用选择。

公共装配入口的实现见 [App extension](../../crates/bevy_ui_router_ecs/src/app_ext.rs)。

## Scene profile 与创建责任

`AppBsnExt::register_bsn_view` 接受同步 factory：输入为只读 `RouteViewInput`，成功返回一个 fresh owned Scene，失败返回 `BsnBuildError`。

每个 view effect 调用一次 factory，并单次消费返回的 Scene；不克隆、缓存或重放旧 Scene。错误 presentation 使用自己的 factory 调用和身份。

输入提供 requested location、当前 matched route、route-local params、raw search、fragment、origin 和错误尝试的原始 failure。factory 不得到 World、Commands、Entity、slot、frame/generation 或 lifecycle 权限。

支持直接代码组合的 native Scene、标准 `ChildOf` 声明图、正常 component/bundle/fallible templates、声明图内命名引用与资源 handles。

cached Scene asset inclusion 包括已加载 cached patch，在 UI 分配前明确拒绝；依赖未就绪在 immediate profile 中失败，不自动转为 queued loading 或无限 retry。非 `ChildOf` 输出及 template 主动创建的声明闭包外实体由应用负责。

同步执行仍需要官方 Scene runtime：`AssetServer` 和 `Assets<ScenePatch>`。应用安装正常插件组合，最小环境可安装 `TaskPoolPlugin`、`AssetPlugin`、`ScenePlugin`。

含 BSN binding 的 Router 安装时缺少必要资源，返回 `MissingSceneRuntime`；运行期间资源撤销，在调用 Scene 前产生明确失败，不因访问缺失资源 panic。实际 UI、picking 和 focus 插件接线由集成指南说明。

执行使用官方 resolve/apply：消费 owned Scene，预检声明图，预分配并登记本次 exact Entity，再以私有 shell wrapper 和引用 seed 应用原 Scene。

root/child 原命名引用仍指向原内容 Entity，匿名节点使用 attempt 内唯一引用。保留 Scene 正常 component 值和 Children 声明顺序，不模拟或替换官方 relationship callbacks。

声明 Entity 提前分配；读取声明图的官方 getter 可使 leaf 带空 `Children` 并触发相应 hook。这是明确的执行差异，不承诺与裸 `apply_scene` 的全部中间状态一致。wrapper 的额外外层错误不污染原错误来源。

选择声明图预登记是因为 apply 可在建立 `ChildOf` 前失败；只在返回后扫描 Children 会遗漏未挂接产物。prepared payload 保留依赖 handles 到真实 apply/settlement 完成，不把 resolve 完成视为责任结束。

apply 与其返回后的 adoption/失败隔离在同一 exclusive 调用处理；未挂接的部分 UI 也必须由本次创建义务覆盖，在返回普通系统或下一输入/呈现边界前隔离。后续安全销毁可以延后，不能用“以后 cleanup”替代当前隔离。

实现见 [BSN preparation](../../crates/bevy_ui_router_ecs/src/bsn.rs) 与 [native Scene executor](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs)。

## Frame 内容与 default outlet

物理结构为 app-owned host → top slot → FrameShell → FrameContentTarget → Scene content；嵌套内容的 outlet 承载 child slot 和下一层 frame。

shell 的布局与隔离组件由 Router 控制；content root 的普通 `Node`、`Visibility`、`Pickable` 等由 Scene 定义。生命周期切换不逐项覆盖 Scene authored 值，也不为兼容旧内部模型增加多余 root 层。

需要 child route 的 layout 声明唯一 `DefaultOutlet`，位于内容根或内部 Node；leaf 不要求 outlet。声明是普通组件，不携带 authority。缺失、重复、非容器或关系失效产生 mount failure，不选第一个或退回 root。

父 layout 保留时保留同一 outlet anchor/child slot。声明与实际 binding 分离，移动 marker 不提供动态切换挂载点的能力。

消费 mount 前复核 parent/frame generation、anchor、slot 和关系。entering parent 的无效 outlet 是 pending rejection；已提交 preserved parent 丢失 binding 是 authority loss，不混成同一 retry/fallback 路径。

每个 frame 的内容遍历遇到 child slot 停止；嵌套 frame 以自身身份和角色管理。不以 frame/slot 数量、数组位置或关联角色并集推导 authority。
