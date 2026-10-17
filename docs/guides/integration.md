# 应用集成

推荐通过 facade 的 `bsn` feature 使用官方同步 Scene。支持范围见[产品概览](../overview.md)，factory 与 outlet 行为见
[Scene 合同](../contracts/scene.md)，模块关系见[架构](../architecture/README.md)。
以下命令使用仓库本地依赖；包尚未发布 crates.io。

## 运行仓库示例

从 checkout 根目录运行：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

| 示例 | 用途 | 运行环境 |
| --- | --- | --- |
| [bsn_quick_start.rs](../../crates/bevy_ui_router/examples/bsn_quick_start.rs) | 最短装配、入队、request 终态与 active 快照 | 有限次 headless 更新，不计算 UI 布局 |
| [bsn_nested.rs](../../crates/bevy_ui_router/examples/bsn_nested.rs) | 内部 outlet、跨 query 复用、push/replace/back、应用异步内容与显式滚动恢复 | 有限次 headless 更新，执行真实 UI 布局，无 GPU renderer |
| [windowed](../../examples/windowed/) | 可见页面、交互、focus、错误恢复 | 独立应用 workspace，配置窗口和 renderer |

前两个程序运行完成后退出，不打开操作系统窗口。
窗口示例从同一 checkout 单独启动：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

窗口、相机、字体和视觉样式由该应用配置，图形依赖不进入 Router 库默认功能。
Linux 示例选择 X11 后端，构建需要 X11/xkbcommon 开发库和可用的桌面显示、图形驱动。
这是示例的环境选择；库自身没有 X11 依赖。
应用局部自动化交互与截图见[调试指南](debugging.md)。

## 在独立 Cargo 应用中使用

例如把仓库目录命名为 `bevy_ui_router`，在旁边建立 `router-demo`，将 quick start
原样复制为 `router-demo/src/main.rs`。使用以下 manifest，其他目录布局调整 `path`：

```toml
[package]
name = "router-demo"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", default-features = false, features = ["bsn"] }
bevy_app = { version = "0.19", default-features = false }
bevy_asset = { version = "0.19", default-features = false }
bevy_ecs = { version = "0.19", default-features = false }
bevy_scene = { version = "0.19", default-features = false }
bevy_ui = { version = "0.19", default-features = false }
```

在 `router-demo` 中执行 `cargo run`。使用满足 Bevy 0.19 要求的 Rust 工具链。
库声明 `0.19`，允许稳定 `0.19.x`；仓库 lock 使用 `0.19.1`，最低验证版本为 `0.19.0`。
版本和独立消费者验证入口见[验证指南](../development/verification.md)。

若复制嵌套示例，再添加其布局、相机与输入所需的依赖：

```toml
bevy_input_focus = { version = "0.19", default-features = false }
bevy_internal = { version = "0.19", default-features = false, features = ["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"] }
```

这些依赖明确来自示例需求，不依赖本仓库 dev-dependency 的 feature 合并。
独立 consumer gate 会编译并运行两个示例的原样源码。

## 装配顺序

1. 安装应用的 Bevy 插件。最小 headless 环境安装 `TaskPoolPlugin`、`AssetPlugin`、
   `ScenePlugin`；可见 UI 另需应用自己的窗口、camera、UI/render 与输入配置。
2. 使用 `AppBsnExt::register_bsn_view` 注册 Scene factory，保存返回的 `ViewDriverKey`。
3. 建立 `RouteViewRegistry`，以 `RouteViewDescriptor` 将 view key 绑定到 driver key。
4. 使用 `RouteTree::builder` 声明路由与 normal/error view，准备一个 fallback view。
5. 调用 `add_ui_router(RouterConfig::new(routes, views, fallback))`，保存 `RouterHandle`。
6. 在符合[host 拓扑条件](../contracts/lifecycle.md#host-接纳与重新注册)的应用自有 `Node` 上添加 `RouterHostRegistration::new(handle)`，推进一次更新，
   从 snapshots 确认 host 已可用。
7. 通过 `RouterNavigation::{push,replace,back}` 入队；保存 request id，持续推进应用
   更新并从 `RouterSnapshots` 观察结果。

factory 只接收 `&RouteViewInput`，返回 fresh Scene 或 `BsnBuildError`。
重复 driver key 同步拒绝，不替换既有 factory；安装时同步检查所有 view/driver 绑定。
`register_bsn_view` 与 `add_ui_router` 自动幂等安装 Router foundations；也可显式预装
`BevyUiRouterPlugin`，无需重复维护系统链。

即使 Scene 完全由代码同步创建，也需要 `AssetServer` 和 `Assets<ScenePatch>`。
先安装正常 Asset/Scene runtime 再添加 Router；缺失会返回 `MissingSceneRuntime`。
Cargo feature 只控制编译，不能证明应用运行时资源已就绪。

## 嵌套内容与结果观察

layout Scene 在希望显示子路由的内部 `Node` 上声明一个 `DefaultOutlet`。
相邻子页切换会保留可复用的父 layout、outlet 及局部组件状态；离场实例默认清理，也可按下一节启用保留。
当前支持每个 layout 的单个 default outlet。Scene 加载 profile 见[Scene 合同](../contracts/scene.md)，
search/fragment 更新见[导航合同](../contracts/navigation.md)，错误恢复见[生命周期](../contracts/lifecycle.md)。

location 分开提供 path、search 与 fragment，例如 `RouteLocation::new("/items/42").with_search("sort=name").with_fragment("details")`。不要把完整 URL 传给 `new`；该例的 search/fragment 都保留原字符串，不自动添加或删除 `?`/`#`。

应用拥有 host 并决定其布局。Router 只管理授权结构和内容边界；不要把借用的
app-owned Entity 挂入该内容边界，也不要依赖 Scene observer 驱动 Router lifecycle。

入队的 `Ok(request_id)` 只表示命令被接纳。窗口应用由 `App::run` 持续更新；有限
headless 程序应给等待设置边界并检查 `RequestStatus::Succeeded` / `Failed`。
读取 `RouterSnapshots::request(handle, id)` 时，`NotRetained` 表示该 Router 的已完成
记录被淘汰，`Unknown` 表示无对应记录或身份不属于此 Router。

普通应用系统需要本轮完整 snapshots 时，在 `Update` 的
`RouterScheduleSet::PublishSnapshots` 之后运行。
active 已切换而请求仍在 `PostCommitCleanup` 是合法状态；避免仅凭一次 phase
或 factory 返回就把整个导航视为完成。

## 布局与页面保留

在已有 normal 父 route 下，为布局或页面 opt-in，并为 Router 配置非活动容量：

```rust
let page = RouteNodeOptions::new()
    .with_view(page_view)
    .with_keep_alive(true);
routes.route_with_options(parent, RoutePattern::path(":id")?, page)?;
let config = RouterConfig::new(routes.build()?, views, fallback)
    .with_keep_alive_capacity(2);
let router = app.add_ui_router(config)?;
```

按三个独立问题配置：

| 问题 | 配置与含义 |
| --- | --- |
| 离开后要留下这个实例吗？ | route 的 `with_keep_alive(true)`，默认关闭；不影响仍在 active 分支中的正常复用。 |
| 最多留下多少实例？ | Router 的 `with_keep_alive_capacity(n)`，默认 0；只计算非活动实例，父布局和子页分别计数。 |
| query 改变时还适用吗？ | route 的 `with_preserve_on_search(true)`，默认关闭；对当前内容和库存命中使用同一政策，不忽略 params 或其它复用身份。 |

只设 route 或只设容量不会使所有页面自动保留。库存绑定原来的 normal 父实例及其 outlet/slot；父实例可以 active 或 retained，直接挂在 host 下、没有 normal 父 frame 的 root 不能进入库存。父实例清理或被淘汰时，其依赖库存一并清理，所以实际库存可能少于容量。无需额外缓存 runtime、factory wrapper 或应用自建 Entity 仓库。

父、子分别配置：保留父布局不要求保留当前子页；返回时由目标路由选择子页，不自动恢复旧子目标。带 query 的页面之间导航时，稳定 root/layout 可开启 `preserve_on_search`；从第一个未开启或身份变化的节点起重建，子页不会继承父配置。search 与 fragment 同时变化时仍遵守最深 frame 的 fragment 重建政策。完整 LRU、命中与 history entry 规则见[导航合同](../contracts/navigation.md#实例保留)，失效结算见[生命周期](../contracts/lifecycle.md#非活动实例与长期-owner)。

## 动态内容与重新激活

动态页面沿用现有 Scene、Bevy 系统和 UI 布局：

1. layout Scene 用 `DefaultOutlet` 提供子路由插槽；factory 创建页面结构，`RouteViewInput` 只代表创建时输入。
2. 页面发布后，应用通过 `RouteInputs::current` 读取当前位置，再按应用自己的数据标识启动或更新后台任务。保留或缓存命中的实例不会重跑 factory。
3. 任务携带可移动数据或句柄，应用系统接回结果并更新页面内容；应用负责核对任务与数据是否仍匹配，并决定离场后继续还是取消。业务加载不交给 Router。
4. 重新激活继续使用原 Entity 和组件状态；需要随当前路由工作的系统使用 `RouteActivity` 与当前输入，隐藏页面不会自动暂停应用系统。
5. 晚到内容按正常 Bevy 调度创建和布局；需要等内容就绪再恢复滚动时，使用下一节的 `Explicit` 策略，对当前恢复意图发送 ready。

组合用法见[嵌套示例](../../crates/bevy_ui_router/examples/bsn_nested.rs)。当前输入、后台工作资格和输入 gate 的具体接法见下文；保留实例不需要更换 UI 容器或另建加载框架。

## 滚动恢复

在 Scene 的滚动容器上同时声明 `RouteScroll::new("body")`、Bevy 的 `ScrollPosition` 和配置了 scroll overflow 的 `Node`；`RouteScroll` 从 facade/prelude 导入。多个容器使用不同稳定 key，例如 `"body"` 和 `"sidebar"`，子页面可重复父布局的 key。

默认自动策略在容器唯一绑定后的首个正常布局恢复一次。若内容需要加载或动态确定范围，声明显式策略：

```rust
let marker = RouteScroll::new("body")
    .with_restore_policy(RouteScrollRestorePolicy::Explicit);
```

BSN 中通过官方 `PatchFromTemplate` 声明修改，可参考 [nested 示例](../../crates/bevy_ui_router/examples/bsn_nested.rs)。应用决定内容何时足够恢复，再对当前意图 ready；只读 lookup 可以和应用自己的可变 Query 一起使用：

```rust
fn allow_restore(
    restores: RouteScrollRestorations,
    mut commands: Commands,
    pages: Query<(Entity, &PageState)>,
) {
    for (scope, page) in &pages {
        if page.content_ready {
            if let Some(restore) = restores.pending(scope, "body") {
                commands.ready_route_scroll(restore.id());
            }
        }
    }
}
```

`scope` 是该 active frame 内的内容 Entity，容器尚未创建也能查询；`restore.position()` 是被冻结的二维逻辑像素目标。取消或应用决定其它滚动政策时使用 `commands.cancel_route_scroll(restore.id())`。处理用户滚动事件时可读取容器的 `RouteScroll::key()`，查询并取消对应意图，尤其适用于已经表达滚动意图但边界处数值未变的情况。旧 ID 不影响下一次 Back；ready 不会等待范围继续增长，完成后内容增长也不再次跳动。

新 UI 结构在 `Update` 或 `UiSystems::Prepare` 前创建并完成 deferred 写入，使相机等正常 propagation 生效。ready/cancel 与已有组件数值在 `RouterScrollSet::Prepare` 前生效；要覆盖本次恢复请求值，系统明确排在 Prepare 后、Layout 前。Commands 使用 Bevy 正常 deferred 同步。publication 与导航成功不代表本轮布局或恢复已经完成，最终位置在正常 Layout 后读取。

应用需安装正常 UI layout 并配置有效 camera/render target。包括早晚容器、缓存命中和保留父布局的规则统一归[导航合同](../contracts/navigation.md#按访问恢复滚动)；真实布局边界见[公开滚动旅程](../../crates/bevy_ui_router/tests/scroll_restoration.rs)。

## 输入与焦点接入

页面后台工作通过 facade/prelude 的 `RouteActivity` SystemParam 查询资格：

```rust
fn update_pages(activity: RouteActivity, mut pages: Query<(Entity, &mut PageState)>) {
    for (entity, mut page) in &mut pages {
        if activity.is_active(entity) {
            page.update();
        }
    }
}
```

activity 的范围和调度见[输入与活动合同](../contracts/input-focus.md#页面活动查询)；它可以与应用的可变组件查询共用，隐藏不会自动暂停系统。输入资格仍使用下面的专用 gate。完整用法见[公开活动旅程](../../crates/bevy_ui_router/tests/public_activity.rs)。

保留的布局或缓存命中的页面若需要本次路由位置，使用 `RouteInputs`，不要读取上次 factory 的 `RouteViewInput` 当作当前输入：

```rust
fn update_route_labels(inputs: RouteInputs, mut labels: Query<(Entity, &mut Text)>) {
    for (entity, mut text) in &mut labels {
        if let Some(input) = inputs.current(entity)
            && text.0 != input.location().path()
        {
            text.0 = input.location().path().to_owned();
        }
    }
}

app.add_systems(
    Update,
    update_route_labels.after(RouterScheduleSet::PublishSnapshots),
);
```

每轮仍查询当前归属和输入；仅文字变化时写入，避免相同值反复触发 `Changed<Text>`。

`RouteInputs` 和中立借用类型 `RouteInputRef` 从 facade/prelude 导入。`params()` 为所属 frame 的 route-local 参数，不自动合并祖先参数；`location()` 包含完整 path/search/fragment。错误呈现的 `failure()` 和 `origin()` 按该 frame 定义，正常前缀的 failure 仍为 None；fallback 的 `route()` 为 None、params 为空。非活动或失效归属返回 None，范围与 `RouteActivity` 相同。这是只读观察，不推进生命周期，也不替代输入 gate；跨 World mutation/await 前复制所需普通值。真实 Scene consumer 见[独立 BSN 消费者](../../crates/bevy_ui_router/tests/consumers/bsn.rs)。

标准 UI pointer 使用原生 UI picking。Router 在正常提交和后续输入刷新时维护
非活动内容隔离，并撤销历史手势状态；应用无需另建一套 pointer 路由。
运行时新增、隐藏或移动内容，应在输入刷新前完成，供当前 containment 判断使用。

目标定向 keyboard/gamepad 系统在 `PreUpdate` 的 `RouterInputSet::Ready` 之后处理，
并按 target 查询 `RouterInputGate::allows_keyboard` 或 `allows_gamepad`。
全局应用快捷键由应用按自己的语义管理。窗口示例展示显式 gated keyboard handler，
未接入手柄设备。

应用安装 `InputFocusPlugin` 提供原生焦点行为。Router 处理受管目标的发布与撤权，
但不会自动替应用选择首焦点；Scene 可以使用正常的应用焦点操作。
`TabNavigationPlugin` / `InputDispatchPlugin` 不会仅因安装 Router 自动遵守 gate，
当前示例也未安装它们；如需使用，应先完成实际目标资格集成与验证。

检查真实交互时，使用[调试指南](debugging.md)中的应用局部输入与 renderer 截图。
headless 成功不等于窗口、原生平台或硬件设备验收；当前证据边界见
[验证指南](../development/verification.md)。
