# Bevy UI Router

面向 Bevy UI 的嵌套路由库：路由树、location/参数匹配、`push` / `replace` / `back`、内部 default outlet、错误恢复，以及请求/呈现/history 快照。Core 与 View 保持 renderer-neutral；推荐通过 facade 的 `bsn` feature 使用官方代码声明 Scene。

MVP 已在本地 `main` 完成集成和验收；以下使用仓库本地依赖，不要求已发布的 crates.io 包。合同与验证边界见[文档索引](docs/README.md)、[统一 spec](docs/superpowers/specs/2026-09-07-router-mvp-design.md)和[实施 plan](docs/superpowers/plans/2026-09-07-router-mvp-implementation.md)。Windows 原生、最低 Rust 版本、窗口/GPU 及真实设备输入尚未执行验证。

## 运行示例

需要满足官方 crates.io **Bevy 0.19.1** 要求的 Rust 工具链（`bevy_ecs` 声明最低 Rust 1.95.0，项目使用 2024 edition）；使用仓库 `Cargo.lock`，不使用 Bevy fork、patch 或替换源。从此 checkout 根目录运行：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

[完整 quick start](crates/bevy_ui_router/examples/bsn_quick_start.rs)展示最短装配路径：安装 Scene runtime → 注册 Scene factory → 组装 route tree / view registry → 添加 Router 和 app-owned host → 入队导航 → 普通 `App::update` → 从 `RouterSnapshots` 读取 `Succeeded` 与当前 location。返回的 request id 用于跟踪异步推进的请求，入队成功本身不表示页面已经发布。

[嵌套示例](crates/bevy_ui_router/examples/bsn_nested.rs)创建真实的侧栏、工具栏和内部 `DefaultOutlet`，在 overview/settings/help 三页之间执行 push、replace、back。每步检查正尺寸的 `ComputedNode`，并验证 layout、工具栏局部状态、outlet 及其 child slot 的关系保留。程序会打印导航结果和页面计算尺寸，最后正常退出。

这两个程序都是有限次更新的 **headless 演示，不打开操作系统窗口或 GPU renderer**。quick start 只安装 `TaskPoolPlugin`、`AssetPlugin`、`ScenePlugin`，不计算 UI 布局；嵌套示例使用显式开启 window、camera、UI、picking 的 `bevy_internal::DefaultPlugins`，加 Scene runtime、`InputFocusPlugin` 和 `Camera2d`，实际执行官方 UI 布局。headless 运行不是视觉、真实设备交互或跨平台验收。窗口、Winit、renderer、字体及视觉样式应由应用自己的 Bevy 配置负责；Router 不代替应用选择这些配置。

## 在独立应用中使用

例如把此仓库目录命名为 `bevy_ui_router`，在其旁边创建一个独立 Cargo 应用 `router-demo`，将 quick start 源文件复制为该应用的 `src/main.rs`。其 `Cargo.toml` 使用以下内容（其它目录布局请相应修改本地 `path`）：

```toml
[package]
name = "router-demo"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", default-features = false, features = ["bsn"] }
bevy_app = { version = "=0.19.1", default-features = false }
bevy_asset = { version = "=0.19.1", default-features = false }
bevy_ecs = { version = "=0.19.1", default-features = false }
bevy_scene = { version = "=0.19.1", default-features = false }
bevy_ui = { version = "=0.19.1", default-features = false }
```

在 `router-demo` 中执行 `cargo run`。若改用嵌套示例源码，再加入以下两项依赖；这些 feature 是该示例所需的真实布局和输入资源来源：

```toml
bevy_input_focus = { version = "=0.19.1", default-features = false }
bevy_internal = { version = "=0.19.1", default-features = false, features = ["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"] }
```

独立 consumer 测试在单独 workspace 中编译并执行这两个**原样源文件**，显式声明上述依赖，不借用本仓库 dev-dependency 的 feature 合并。

## Runtime 与输入边界

`bsn` feature 包含 `ecs` 和 `view`；默认 feature 只有 `view`，没有默认 Bevy runtime。即使 Scene 完全由代码同步创建，也需要 `AssetServer` / `Assets<ScenePatch>`：先安装正常 Scene 插件，再调用 `add_ui_router`。缺失必要资源会产生安装错误。当前支持直接代码组合的 native Scene、标准 `ChildOf` 声明图和正常模板；cached Scene asset inclusion 不属于此 MVP profile。

应用拥有 host，Router 管理其路由内容。单个 `DefaultOutlet` 可以位于 Scene 内部；leaf 切换保留匹配的父布局及其局部组件状态。失败处理、输入撤权和 history 发布由 Router 生命周期决定，Scene observer 不是路由生命周期权威。

标准 UI pointer 使用原生 UI picking。应用的 target-addressed keyboard/gamepad 系统应在 `bevy_ui_router::ecs::RouterInputSet::Ready` 后读取 `RouterInputGate`，按目标权限处理输入；动态内容改动在输入刷新前提交。`InputFocusPlugin` 提供原生焦点资源，Router 处理受管目标的发布与撤权，但不会自动为应用选择首焦点。原生 `TabNavigationPlugin` / `InputDispatchPlugin` 不会仅因安装 Router 就自动遵守 gate；这里的示例未安装它们，也未演示设备输入。详见 [spec 的输入与焦点合同](docs/superpowers/specs/2026-09-07-router-mvp-design.md#71-输入与焦点)。
