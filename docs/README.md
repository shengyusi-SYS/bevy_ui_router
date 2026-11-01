# 文档

[English](README.en.md) | 简体中文

## 使用 Router

第一次接入，从[双页窗口教程](guides/integration.md)开始。依赖、完整代码和运行步骤在同一页。

跑起来以后，再按需要阅读：

- [页面状态与滚动](guides/pages.md)：保留页面、读取参数、加载内容、恢复滚动与输入处理。
- [过渡动画](guides/transitions.md)：用 Tween 或自己的动画系统设计效果。
- [系统返回](guides/platform-back.md)：鼠标后退键与 Android 传统返回。
- [排查问题](guides/debugging.md)：从当前路由状态定位问题，再检查窗口与输入。

[产品概览](overview.md)列出能力范围；[版本对应表](guides/integration.md#bevy-版本要求)说明应选择哪个版本。

## 查询具体规则

[行为参考](contracts/README.md)按导航、Scene、生命周期和输入分类，适合查阅精确行为。API 文档可在仓库根目录生成：

```sh
cargo doc -p bevy_ui_router --all-features --no-deps --open
```

## 参与开发

从[开发指南](development/README.md)进入对应实现与测试。模块关系见[架构总览](architecture/README.md)，执行过程见[运行机制](architecture/runtime.md)。

[验证](development/verification.md) · [性能测量](performance.md) · [设计理由](development/design-lessons.md) · [路线图](roadmap.md)

已完成计划与实验过程通过 `git log -- docs` 查阅；[性能数据](data/performance/README.md)保留对应源码与环境信息。
