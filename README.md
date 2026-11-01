# Bevy UI Router

[English](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/README.en.md) | 简体中文

为 Bevy UI 组织页面、共享布局和返回记录。声明路由与页面 Scene，用 `push`、`replace` 和 `back` 导航；Router 管理页面挂载、共享布局复用、错误呈现和离场输入撤权。

- 布局和子页可分别保留局部状态，Back 按访问记录恢复滚动位置。
- 页面与复用布局可读取当前路径、参数和活动状态。
- 支持应用自定义过渡；提供可选 Tween 适配、桌面鼠标返回和 Android 传统返回接入。

当前源码对应 **Bevy 0.19**，使用 Rust 2024 edition，尚未发布。后续 Bevy 版本按 Router 发布系列适配，见[版本对应表](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#bevy-版本要求)。

## 用法一览

在已经安装 `AssetPlugin` 和 `ScenePlugin` 的 Bevy App 中，声明页面、设置首屏，再将 Router 挂到应用的 UI 容器：

```rust
use bevy_app::App;
use bevy_ecs::prelude::Commands;
use bevy_scene::bsn;
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

fn install_pages(app: &mut App) -> Result<RouterHandle, BsnRouterError> {
    let router = app.add_bsn_router(
        BsnRouter::new(
            layout(|_| Ok(bsn! { Node DefaultOutlet })).children([
                index(|_| Ok(bsn! { Node })),
                page("settings", |_| Ok(bsn! { Node })),
            ]),
        )
        .fallback(|_| Ok(bsn! { Node }))
        .initial("/"),
    )?;
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    Ok(router)
}

fn open_settings(commands: &mut Commands, router: RouterHandle) {
    commands.router(router).push("/settings");
}
```

这里的 `Node` 是页面内容占位；应用提供实际 Scene、容器布局、窗口和相机。`DefaultOutlet` 是布局中的子页位置，`fallback` 是应用提供的错误页面。后续导航也可调用 `.replace(...)` 和 `.back()`。Commands 在 Bevy 应用延迟命令时入队；需要立即取得请求 ID 时使用 `RouterNavigation`。

## 运行示例

先运行不需要窗口或显卡的完整示例：

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cd bevy_ui_router
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
```

成功时输出 `Succeeded` 和当前路径，然后退出。想直接体验点击、返回和页面保留，运行：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

窗口示例需要桌面显示和图形驱动；Linux 使用 X11。依赖与运行说明见[接入指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#运行仓库示例)。

| 接下来想试什么 | 示例入口 |
| --- | --- |
| 嵌套布局、query 更新、异步内容和滚动恢复 | [嵌套示例](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#运行仓库示例)，无窗口运行 |
| 用现成 Tween 组合过渡效果 | [Tween 示例与接入](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/transitions.md#使用-tween-适配) |
| 接入自己的动画系统 | [手工 driver 示例](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/transitions.md#运行手工-driver-示例) |

## 接入应用

当前使用指向本地 checkout 的依赖：

```toml
[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", features = ["bsn"] }
```

[接入指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#在独立-cargo-应用中使用)提供完整 `Cargo.toml` 与插件装配步骤。普通 Bevy UI 应用从 `bsn` 开始，Core/View 可独立使用。

| Feature | 用途 |
| --- | --- |
| `bsn` | 推荐的 Bevy Scene 页面集成，包含 `ecs` 和 `view` |
| `tween` | 可选 `bevy_tween` 过渡协调，具体效果由应用提供 |
| `desktop-back` / `android-back` | 鼠标后退侧键 / Android 传统返回键 |
| `view`（默认） | 与渲染器无关的视图描述和注册 |
| `ecs` | 自行装配 view driver 时使用的 Bevy 执行层 |
| `platform` | 独立的平台导航接口 |

## 常用任务

| 我想…… | 从这里开始 |
| --- | --- |
| 保留页面状态、控制 query 更新时是否重建 | [保留布局与页面](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#布局与页面保留) |
| 读取当前参数，控制后台任务与重新激活 | [加载内容与重新激活](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#动态内容与重新激活) |
| 返回时恢复滚动，等待异步内容 ready | [滚动恢复](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/integration.md#滚动恢复) |
| 排查首屏不显示、导航失败、状态未保留 | [导航与窗口调试](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/guides/debugging.md) |

## 文档与贡献

- [文档索引](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/README.md)：中英文使用、API 与开发入口。
- [产品概览](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/overview.md)：能力与职责边界。
- [贡献指南](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/CONTRIBUTING.md)：开发入口和检查方法。
- [路线图](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/docs/roadmap.md)：后续工作与发布准备。

## 生产实践

[涟音 LianYin](https://lianyin.eztools.fun/zh-cn/) 是一款跨平台、跨设备音乐播放器，在实际产品中使用本库管理页面导航、历史返回、页面保留和滚动位置恢复。产品介绍与版本发布见[涟音仓库](https://github.com/shengyusi-SYS/lianyin)。

## 许可证

本项目采用双许可证，可选择 [MIT](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-MIT) 或 [Apache-2.0](https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/LICENSE-APACHE)。
