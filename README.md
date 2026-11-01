# Bevy UI Router

[English](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/README.en.md) | 简体中文

为 Bevy UI 组织页面、共享布局和返回记录。声明路由树和页面 Scene 后，用 `push`、`replace` 和 `back` 切换页面；Router 会保留可复用的布局，并处理页面挂载、错误呈现、输入和焦点。

- **嵌套页面**：支持路径参数、默认子页和无路径布局，每层布局提供一个默认出口。
- **连续体验**：布局和子页可分别保留实体与局部状态，返回时按访问记录恢复滚动位置。
- **当前上下文**：页面与复用布局都能读取当前路由、参数和活动状态。
- **按需集成**：Core/View 可独立使用；`bsn` feature 接入 Bevy Scene；桌面鼠标和 Android 返回通过可选 adapter 接入。

## 运行示例

克隆仓库后，在根目录运行最小示例：

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cd bevy_ui_router
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
```

成功时输出 `Succeeded` 和当前路径，然后退出。这个示例用有限次更新演示完整导航流程。接着运行嵌套示例，查看共享布局、当前输入、页面数据加载和滚动恢复：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

打开交互窗口：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

窗口可体验页面切换、返回、布局和页面计数、错误恢复、键盘交互与滚动。Linux 使用 X11；依赖安装和窗口操作见[接入指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md)。

可选 Tween 适配让应用只定义动画效果，由库协调开始、取消和完成：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features tween --bin tween_transitions
```

需要接入自己的动画系统时，手工 driver 示例提供平移、缩放旋转和连续导航取消：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
```

## 接入应用

当前从仓库使用本地依赖，下面的路径指向你的 checkout：

```toml
[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", features = ["bsn"] }
```

本库使用 Rust 2024 edition，以 Bevy 0.19 为起点，按 Router 发布系列维护版本兼容关系。当前源码对应 Bevy 0.19；版本对应表与 Cargo 说明见[接入指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#bevy-版本要求)。

从[接入指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md)开始：用 `BsnRouter` 直接关联路由与页面 factory，设置 `initial` 并挂载应用 host；普通页面系统通过 `commands.router(handle).push("/items/42?tab=info")` 导航。完整可运行源码见[最小示例](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/crates/bevy_ui_router/examples/bsn_quick_start.rs)。

| Feature | 用途 |
| --- | --- |
| `view`（默认） | renderer-neutral 的视图描述和注册 |
| `ecs` | Bevy 插件、host、导航、快照、活动状态和输入 gate |
| `bsn` | 推荐的 Scene 集成，包含 `ecs` 和 `view` |
| `tween` | 可选 `bevy_tween` 过渡协调，包含 `ecs`，具体效果由应用提供 |
| `platform` | 平台导航接口 |
| `desktop-back` / `android-back` | 鼠标后退侧键 / Android 传统返回键 |

应用负责窗口、相机、UI 样式及数据任务。Router 管理 host 中的路由内容。键盘和手柄处理器在 `RouterInputSet::Ready` 后查询 `RouterInputGate`；接线示例见指南。

## 文档与贡献

- [文档索引](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/README.md)：中英文使用与开发文档。
- [产品概览](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/overview.md)：能力与常用概念。
- [贡献指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/CONTRIBUTING.md)：开发入口和检查方法。
- [路线图](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/roadmap.md)：正在开发的能力及发布准备。

## 生产实践

[涟音 LianYin](https://lianyin.eztools.fun/zh-cn/) 是一款跨平台、跨设备音乐播放器，在实际产品中使用本库管理页面导航、历史返回、页面保留和滚动位置恢复。产品介绍与版本发布见[涟音仓库](https://github.com/shengyusi-SYS/lianyin)。

## 许可证

本项目采用双许可证，可选择 [MIT](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-MIT) 或 [Apache-2.0](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-APACHE)。
