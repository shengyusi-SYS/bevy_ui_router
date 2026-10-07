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
| [bsn_nested.rs](../../crates/bevy_ui_router/examples/bsn_nested.rs) | 侧栏/工具栏、内部 outlet、push/replace/back 与局部状态保持 | 有限次 headless 更新，执行真实 UI 布局，无 GPU renderer |
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
相邻 leaf 切换会保留可复用的父 layout、outlet 及局部组件状态；leaf 内容按导航重建。
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

## 输入与焦点接入

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
