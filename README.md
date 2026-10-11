# Bevy UI Router

面向 Bevy UI 的嵌套路由库：声明路由树和 Scene，通过 `push` / `replace` / `back` 切换页面，保留公共布局及局部状态，并统一处理 memory history、错误恢复、输入和焦点。Core/View 保持 renderer-neutral；推荐使用 facade 的 `bsn` feature。

当前尚未发布 crates.io，具体许可证和发布版本待确定，暂不授予开源许可证。功能与发布准备已有本地验证，运行环境及未验证项见[验证指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/development/verification.md)。

## 快速运行

在完整 checkout 根目录执行：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

[quick start](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_quick_start.rs)演示 Scene runtime、factory、路由注册、host、导航和 snapshot；[嵌套示例](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_nested.rs)演示侧栏、工具栏、内部 outlet 及真实 UI 布局，离开布局后返回另一子页仍保留原工具栏状态。这两个程序都是有限次更新的 headless 演示，不打开 OS 窗口；quick start 不运行 UI 布局。

交互窗口是独立、不可发布的应用 crate，图形依赖不进入 Router 库：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

窗口包含 push/replace/back、页面与布局计数、错误 fallback、焦点和跨页手势演示。需要桌面显示及图形驱动；示例选用 X11，Linux 构建需要 X11/xkbcommon 开发库。应用内 BRP 输入、布局查询与 renderer 截图见[窗口调试](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/debugging.md)。

## 接入应用

当前使用仓库本地 `path` 依赖；完整独立应用 manifest 和装配步骤见[接入指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md)。依赖声明为官方 Bevy `0.19`（`>=0.19.0, <0.20.0`），已验证 `0.19.0` / `0.19.1`，默认 lock 为 `0.19.1`。使用满足 Bevy 要求的 Rust 工具链；其 `bevy_ecs` 声明最低 Rust 1.95.0，本项目采用 2024 edition。

默认 feature 为 `view`，不引入 Bevy；`bsn` 包含 `ecs` 和 `view`。即使代码同步构建 Scene，也必须安装 Scene runtime。当前支持单条嵌套链、每个 layout 的单个 default outlet 与代码驱动 native Scene；浏览器 URL/history adapter、cached Scene inclusion 和异步加载未交付。factory 产生新的 Scene，注册或请求入队成功不代表页面已经发布。

应用拥有 host 及窗口/相机/插件配置。原生 pointer 使用 Bevy UI picking；target-addressed keyboard/gamepad handler 应在 `RouterInputSet::Ready` 后检查 `RouterInputGate`。原生 TabNavigation/InputDispatch 不会因安装 Router 自动遵守 gate。详细行为按 [专题合同](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/contracts/README.md)。

## 开发入口

- [文档索引](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/README.md)：产品、架构、专题合同、使用和验证入口。
- [架构图解](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/architecture/README.md)：crate 依赖和源码地图；运行机制另页说明。
- [开发指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/development/README.md)：按功能定位合同、实现和测试，组织后续设计与计划。
- [性能基线](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/performance.md)：公开 API benchmark、计时边界和历史数据；CI 只验证夹具行为，不设耗时门禁。
- [路线图](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/roadmap.md)：首次发布准备和尚未排期的能力。
