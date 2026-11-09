# Bevy UI Router

[English](README.md) | 简体中文

给 Bevy UI 添加页面导航。用路径组织页面，在切换时复用公共布局，并通过 `push`、`replace`、`back` 管理访问记录。需要时，可以保留页面状态、恢复滚动位置，并接入自己设计的过渡动画。

当前支持 **Bevy 0.20.1**，尚未发布到 crates.io。

本项目100%由GPT旗舰模型开发（用来消耗没用完的额度），本人负责讨论、监督和反馈。

## 运行示例

想先看看现有功能，可以运行仓库里的交互示例：

```sh
git clone https://github.com/shengyusi-SYS/bevy_ui_router.git
cd bevy_ui_router/examples/windowed
cargo run
```

点击 Overview / Settings 切换页面，修改计数后切回来，再试滚动与 Back。没有桌面环境时，可以运行[无窗口示例](docs/guides/integration.zh-CN.md#运行仓库示例)。

## 接入应用

下面声明了首页 `/` 和设置页 `/settings`。`DefaultOutlet` 标记子页面放在哪里：

```rust
let pages = BsnRouter::new(
    layout(|_| Ok(bsn! { Node DefaultOutlet })).children([
        index(|_| Ok(bsn! { Text("Home") })),
        page("settings", |_| Ok(bsn! { Text("Settings") })),
    ]),
)
.fallback(|_| Ok(bsn! { Text("Page unavailable") }))
.initial("/");
```

页面内容使用 Bevy 的 `bsn!` 描述。

**[从这里开始：运行一个完整的双页窗口](docs/guides/integration.zh-CN.md)**。教程包含依赖、完整代码、导航操作和每一步的解释。

## 继续使用

- [页面状态与滚动](docs/guides/pages.zh-CN.md)：保留状态、读取当前参数、加载内容与恢复滚动。
- [过渡动画](docs/guides/transitions.zh-CN.md)：用 Tween 或已有动画系统定义效果。
- [系统返回](docs/guides/platform-back.zh-CN.md)：接入鼠标后退键和 Android 传统返回。
- [排查问题](docs/guides/debugging.zh-CN.md)：页面不显示、导航失败或状态没有保留。

API、行为规则及开发资料见[文档索引](docs/README.zh-CN.md)；参与开发见[贡献指南](CONTRIBUTING.zh-CN.md)。

## 生产实践

[涟音 LianYin](https://lianyin.eztools.fun/zh-cn/) 是一款跨平台、跨设备音乐播放器，在实际产品中使用本库管理页面导航、历史返回、页面保留和滚动位置恢复。产品介绍与版本发布见[涟音仓库](https://github.com/shengyusi-SYS/lianyin)。

## 许可证

本项目采用双许可证，可选择 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)。
