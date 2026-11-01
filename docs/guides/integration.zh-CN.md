# 给 Bevy 应用加上页面导航

[English](integration.md) | 简体中文

这一页做一个有首页和设置页的窗口：按 `1`、`2` 切换，按 `B` 返回。依赖和代码都在下面，可以直接复制运行。已有 Bevy 应用也可以沿这三个步骤接入。

先按 [Bevy 官方环境准备](https://bevy.org/learn/quick-start/getting-started/setup/)安装 Rust 和平台系统依赖；已有能运行的 Bevy 窗口项目可以跳过这一步。

## 1. 创建项目

在存放项目的目录执行，让两个目录并排放置：

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cargo new router-demo
cd router-demo
```

如果已经下载 Router，直接在它旁边创建 `router-demo` 即可。将 `Cargo.toml` 替换为：

```toml
[package]
name = "router-demo"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = { version = "0.19", default-features = false, features = ["ui"] }
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", features = ["bsn"] }
```

这里启用 Bevy 的 UI 功能，以及 Router 的 `bsn` 页面集成。Router 尚未发布，暂时使用本地路径；已有项目只需添加 Router 依赖，并保留自己的 Bevy 配置。

## 2. 添加页面与导航

将以下完整代码放进 `src/main.rs`：

```rust
use bevy::prelude::*;
use bevy_ui_router::prelude::*;

#[derive(Resource)]
struct Pages(RouterHandle);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    let router = app
        .add_bsn_router(
            BsnRouter::new(
                layout(|_| {
                    Ok(bsn! {
                        Node { width: percent(100), height: percent(100) }
                        DefaultOutlet
                    })
                })
                .children([
                    index(|_| Ok(page_content("Home"))),
                    page("settings", |_| Ok(page_content("Settings"))),
                ]),
            )
            .fallback(|_| Ok(page_content("Page unavailable")))
            .initial("/"),
        )
        .expect("valid routes");

    app.insert_resource(Pages(router))
        .add_systems(Startup, setup)
        .add_systems(Update, navigate)
        .run();
}

fn page_content(title: &'static str) -> impl Scene {
    bsn! {
        Node {
            width: percent(100), height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Children [(Text(title))]
    }
}

fn setup(mut commands: Commands, pages: Res<Pages>) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(24)),
            ..default()
        })
        .with_children(|root| {
            root.spawn(Text::new("1: Home    2: Settings    B: Back"));
            root.spawn((
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    ..default()
                },
                RouterHostRegistration::new(pages.0),
            ));
        });
}

fn navigate(keys: Res<ButtonInput<KeyCode>>, pages: Res<Pages>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::Digit1) {
        commands.router(pages.0).push("/");
    } else if keys.just_pressed(KeyCode::Digit2) {
        commands.router(pages.0).push("/settings");
    } else if keys.just_pressed(KeyCode::KeyB) {
        commands.router(pages.0).back();
    }
}
```

代码中只有三处负责路由：

1. **声明页面。** `index` 对应父路径 `/`，`page("settings", ...)` 对应 `/settings`。外层 `layout` 共享一份布局，`DefaultOutlet` 是子页面的位置。闭包在需要创建页面时返回一份 Bevy Scene；这里用 `bsn!` 写它的 UI。
2. **选择显示位置。** `setup` 在一个普通 UI 容器上添加 `RouterHostRegistration`，路由页面就显示在这里。容器的尺寸、相机和窗口仍按普通 Bevy UI 配置。
3. **发起导航。** `navigate` 调用 `commands.router(handle)`。示例将 handle 保存在 `Pages` 资源中，便于系统使用。

`.initial("/")` 指定首次显示的页面。`.fallback(...)` 是必需的错误页面；它用于没有匹配页面或页面创建失败的情况。

## 3. 运行并切换页面

```sh
cargo run
```

窗口首先显示 **Home**。按 `2` 显示 **Settings**，再按 `B` 回到 Home。上方的快捷键提示始终保留，只有下面的页面区域切换。这些键是应用级快捷键。

导航有三种操作：

```rust
commands.router(pages.0).push("/settings");     // 新增一次访问
commands.router(pages.0).replace("/settings");  // 替换当前访问
commands.router(pages.0).back();                // 回到上一次访问
```

这些调用会排队切换页面，不会在调用处立即改变画面。没有更早的访问记录时，`back()` 不切页，并在日志中报告原因；需要在代码中处理结果时，见[导航排障](debugging.zh-CN.md#先定位导航问题)。

## 改成自己的页面

替换 `page_content` 即可使用自己的 UI。把导航栏放进 `layout`，将 `DefaultOutlet` 放在正文区域，切换相邻子页面时导航栏就能继续保留。

子路径相对父路由。列表详情可以写成 `page("items/:id", ...)`，然后导航到 `/items/42`。地址还可以附带查询参数和片段，例如 `/items/42?sort=name#details` 中，`?` 后是查询参数，`#` 后是片段；读取当前参数、保留页面与延迟内容的做法见[页面状态与滚动](pages.zh-CN.md)。

已有应用先安装 Bevy 插件，再调用 `add_bsn_router`；它会安装 Router 所需的系统。使用 `DefaultPlugins` 时，无需额外安装 Scene 插件。

<details>
<summary>应用自行选择 Bevy 插件时</summary>

确保已有 `AssetPlugin` 和 `ScenePlugin`，再添加 Router。需要手工组装路由树和视图注册时，见 [Scene 参考](../contracts/scene.zh-CN.md#注册与运行装配)。

</details>

接下来可以接入[页面控件的输入与焦点](pages.zh-CN.md#输入与焦点接入)、[过渡动画](transitions.zh-CN.md)或[系统返回](platform-back.zh-CN.md)。

## 运行仓库示例

完成上面的教程后，从 `router-demo` 进入旁边的 Router 仓库，运行计数保留、滚动恢复和错误页面演示：

```sh
cd ../bevy_ui_router/examples/windowed
cargo run
```

只验证路由接线、不打开窗口时，回到仓库根目录后运行：

```sh
cd ../..
cargo run -p bevy_ui_router --features bsn --example bsn_quick_start
```

它输出 `Succeeded` 和当前路径后退出。[嵌套示例](../../crates/bevy_ui_router/examples/bsn_nested.rs)进一步组合了布局、query 更新、延迟内容和滚动：

```sh
cargo run -p bevy_ui_router --features bsn --example bsn_nested
```

窗口示例需要桌面显示和图形驱动，在 Linux 使用 X11；系统依赖沿用本页开头的 Bevy 环境准备。需要检查窗口布局或截图时，见[调试指南](debugging.zh-CN.md#启动和观察窗口)。

## Bevy 版本要求

| Bevy | Router |
| --- | --- |
| `0.19` | 当前源码，尚未发布 |

本库从 Bevy 0.19 起，按 Bevy 次版本提供对应的 Router 发布系列；应用选择与自身 Bevy 匹配的版本。`version = "0.19"` 接受兼容的补丁更新，应用使用自己的 lockfile。

其它 Bevy 版本的适配进度见[路线图](../roadmap.zh-CN.md#bevy-020-适配待启动)。
