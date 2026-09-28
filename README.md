# Bevy UI Router

面向 Bevy UI 的嵌套路由库：路由树、location/参数匹配、`push` / `replace` / `back`、内部 default outlet、错误恢复，以及请求/呈现/history 快照。Core 与 View 保持 renderer-neutral；推荐通过 facade 的 `bsn` feature 使用官方代码声明 Scene。

MVP 已完成本地功能验收与 Linux/WSLg 窗口交互验证；以下使用仓库本地依赖，尚未发布 crates.io。合同与验证边界见[文档索引](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/README.md)、[统一 spec](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/superpowers/specs/2026-09-07-router-mvp-design.md)和[实施 plan](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/superpowers/plans/2026-09-07-router-mvp-implementation.md)。窗口验证使用软件渲染和 OS 注入的键鼠事件；Windows 原生、最低 Rust 版本、物理 GPU 加速与硬件设备输入尚未验证。

## 运行示例

需要满足官方 crates.io **Bevy 0.19.x** 要求的 Rust 工具链（`bevy_ecs` 声明最低 Rust 1.95.0，项目使用 2024 edition）；依赖声明 `0.19`（`>=0.19.0, <0.20.0`），已验证官方 0.19.0 与 0.19.1；仓库 `Cargo.lock` 固定 0.19.1 作为默认测试基线，不使用 Bevy fork、patch 或替换源。从此 checkout 根目录运行：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

[完整 quick start](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_quick_start.rs)展示最短装配路径：安装 Scene runtime → 注册 Scene factory → 组装 route tree / view registry → 添加 Router 和 app-owned host → 入队导航 → 普通 `App::update` → 从 `RouterSnapshots` 读取 `Succeeded` 与当前 location。返回的 request id 用于跟踪异步推进的请求，入队成功本身不表示页面已经发布。

[嵌套示例](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_nested.rs)创建真实的侧栏、工具栏和内部 `DefaultOutlet`，在 overview/settings/help 三页之间执行 push、replace、back。每步检查正尺寸的 `ComputedNode`，并验证 layout、工具栏局部状态、outlet 及其 child slot 的关系保留。程序会打印导航结果和页面计算尺寸，最后正常退出。

这两个程序都是有限次更新的 **headless 演示，不打开操作系统窗口或 GPU renderer**。quick start 只安装 `TaskPoolPlugin`、`AssetPlugin`、`ScenePlugin`，不计算 UI 布局；嵌套示例使用显式开启 window、camera、UI、picking 的 `bevy_internal::DefaultPlugins`，加 Scene runtime、`InputFocusPlugin` 和 `Camera2d`，实际执行官方 UI 布局。headless 运行不是视觉、真实设备交互或跨平台验收。窗口、Winit、renderer、字体及视觉样式应由应用自己的 Bevy 配置负责；Router 不代替应用选择这些配置。

### 交互窗口

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

[窗口示例](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/examples/windowed/src/main.rs)是独立的、`publish = false` 的应用 crate，配置 Winit、UI renderer、默认字体和相机；图形依赖不进入 Router 库的 feature。需要可用的桌面显示与图形驱动；Linux 示例选择 X11 后端，构建需要 X11/xkbcommon 开发库。

自动化交互调试使用示例专用的 Bevy BRP 入口，不操作系统鼠标或焦点：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features ui-debug
python3 examples/windowed/ui_debug.py capture /tmp/router-ui.png
python3 examples/windowed/ui_debug.py find "Layout counter"
python3 examples/windowed/ui_debug.py key Digit2 --logical '{"Character":"2"}'
```

该入口仅在 debug 构建显式启用 `ui-debug` 时监听 `127.0.0.1:15703`；窗口启动不请求焦点。`snapshot` 提供实际布局/文本，`click X Y` 使用窗口内逻辑坐标，`rpc router/input` 可分别发送按下/释放来检查跨页手势。输入沿 Bevy 的 WindowEvent 与 typed message 管线，截图来自渲染器；这验证引擎交互，不代替 OS 或硬件输入验证。调试模式保持窗口连续更新，不能用于功耗或正式性能测量。

- 点击 Overview / Settings 执行 push，Replace 替换为 overview，Back 返回。快捷键分别为 `1` / `2` / `R` / `B`。
- 点击 Layout counter 后切换子页：布局 Entity 和计数保持，页面局部计数重新创建。
- 点击页面按钮获得焦点，再按 `Space` 增加页面计数；切换后旧焦点被释放，新页面在点击前不响应 Space。
- 按住页面按钮，用 `1` 或 `2` 切换到另一页，再松开鼠标：旧页面不应收到 release/click。终端输出可核对目标 Entity。
- 点击 Fail（`E`）触发示例主动返回的 Build error，显示 fallback；失败不新增 history。随后点击 Overview 恢复。fallback 替换路由布局，因此不承诺跨 fallback 保留局部计数。

状态栏展示当前位置、history 数量/cursor、焦点和最近请求结果。导航栏属于应用 shell；页面键盘处理在 `RouterInputSet::Ready` 后检查 gate。

## 在独立应用中使用

例如把此仓库目录命名为 `bevy_ui_router`，在其旁边创建一个独立 Cargo 应用 `router-demo`，将 quick start 源文件复制为该应用的 `src/main.rs`。其 `Cargo.toml` 使用以下内容（其它目录布局请相应修改本地 `path`）：

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

在 `router-demo` 中执行 `cargo run`。若改用嵌套示例源码，再加入以下两项依赖；这些 feature 是该示例所需的真实布局和输入资源来源：

```toml
bevy_input_focus = { version = "0.19", default-features = false }
bevy_internal = { version = "0.19", default-features = false, features = ["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"] }
```

独立 consumer 测试在单独 workspace 中编译并执行这两个**原样源文件**，以 0.19.1 为固定基线显式声明上述依赖，不借用本仓库 dev-dependency 的 feature 合并。

## Runtime 与输入边界

`bsn` feature 包含 `ecs` 和 `view`；默认 feature 只有 `view`，没有默认 Bevy runtime。即使 Scene 完全由代码同步创建，也需要 `AssetServer` / `Assets<ScenePatch>`：先安装正常 Scene 插件，再调用 `add_ui_router`。缺失必要资源会产生安装错误。当前支持直接代码组合的 native Scene、标准 `ChildOf` 声明图和正常模板；cached Scene asset inclusion 不属于此 MVP profile。

应用拥有 host，Router 管理其路由内容。单个 `DefaultOutlet` 可以位于 Scene 内部；leaf 切换保留匹配的父布局及其局部组件状态。失败处理、输入撤权和 history 发布由 Router 生命周期决定，Scene observer 不是路由生命周期权威。

标准 UI pointer 使用原生 UI picking。应用的 target-addressed keyboard/gamepad 系统应在 `bevy_ui_router::ecs::RouterInputSet::Ready` 后读取 `RouterInputGate`，按目标权限处理输入；动态内容改动在输入刷新前提交。`InputFocusPlugin` 提供原生焦点资源，Router 处理受管目标的发布与撤权，但不会自动为应用选择首焦点。原生 `TabNavigationPlugin` / `InputDispatchPlugin` 不会仅因安装 Router 就自动遵守 gate；这里的示例未安装它们。窗口示例演示显式 gated keyboard handler，不接入手柄设备。详见 [spec 的输入与焦点合同](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/superpowers/specs/2026-09-07-router-mvp-design.md#71-输入与焦点)。

## 性能基线

```sh
cargo +stable bench -p bevy_ui_router --bench router_baseline --features bsn --locked > baseline.csv
```

测量无窗口的 Router/BSN 空闲帧、push/replace/back、失败恢复和有界重复使用，输出 CSV；不包含 UI 布局、GPU 或真实设备输入。计时边界、smoke 检查与结果见[性能基线说明](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/performance.md)。CI 只验证夹具行为，不设耗时门禁。

## 发布状态

本项目是独立的通用路由库，计划开源发布，尚未发布 crates.io。具体许可证与发布版本待确定；当前仓库尚无许可证文件，manifest 尚未声明 `license`。

CI 在单个 Linux 环境运行格式检查、完整 workspace 检查和测试（含独立消费者）；窗口交互验收单独记录在实施 plan，不由无显示的 CI 结果代替。

在完整 checkout 中执行发布预检（需要 Python 3.12+；不会上传包）：

```sh
cargo +stable package --workspace
RUSTUP_TOOLCHAIN=stable python3 scripts/check-packaged-consumer.py
```

第一步验证六个 `.crate` 的解压构建；第二步用仅指向这些解压产物的 Router patch 运行真实 BSN 消费者，并核对依赖来源。Bevy 仍来自官方 crates.io。打包需要联网解析临时 registry；当前 Cargo 的离线多包验证会报告内部 checksum 错误，不能将该失败当作验证通过。工作区有未提交修改时，审阅后可为第一步显式加 `--allow-dirty`。
