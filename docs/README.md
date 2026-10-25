# 文档

[English](README.en.md) | 简体中文

初次使用可以先[运行示例](../README.md#运行示例)，再阅读[接入指南](guides/integration.md)。每篇使用和开发文档都提供中英文切换。

| 我想…… | 从这里开始 |
| --- | --- |
| 了解 Router 解决什么问题 | [产品概览](overview.md) |
| 将路由接入 Bevy 应用 | [接入指南](guides/integration.md) |
| 添加应用自定义动画 | [过渡指南](guides/transitions.md) |
| 调试窗口、导航和输入 | [调试指南](guides/debugging.md) |
| 查询导航、Scene、生命周期和输入行为 | [行为合同](contracts/README.md) |
| 理解模块分工与数据结构 | [架构总览](architecture/README.md) |
| 跟踪一次导航如何执行 | [运行机制](architecture/runtime.md) |
| 修改 owner 交接或失败清理 | [生命周期执行协议](architecture/lifecycle.md) |
| 修复问题或实现功能 | [开发指南](development/README.md) |
| 运行检查、验证发布产物 | [验证指南](development/verification.md) |
| 测量导航和空闲更新成本 | [性能测量](performance.md) |
| 理解关键设计选择 | [设计理由](development/design-lessons.md) |
| 查看后续工作 | [路线图](roadmap.md) |

## 查阅 API 与历史

在仓库根目录生成 API 文档：

```sh
cargo doc --workspace --all-features --no-deps --locked
```

从 `target/doc/bevy_ui_router/index.html` 开始。使用示例链接到实际源码，开发指南按功能链接到实现与测试。已完成计划和实验过程通过 `git log -- docs` 追溯；保留的[性能数据](data/performance/README.md)带有源码和环境信息。
