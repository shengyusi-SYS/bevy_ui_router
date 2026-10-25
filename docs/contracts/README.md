# 专题合同

[中文](README.md) | [English](README.en.md)

本目录描述当前公开行为，以及实现这些行为需要维持的跨层规则。接入步骤见[使用指南](../guides/integration.md)，模块关系见[架构](../architecture/README.md)。

| 主题 | 内容 |
| --- | --- |
| [导航与观察](navigation.md) | 匹配、参数与 location、实例保留、history、滚动和 snapshots |
| [Scene 与挂载](scene.md) | factory 注册、Scene 创建、内容边界和 default outlet |
| [生命周期与清理](lifecycle.md) | host、身份、责任交接、发布、失败恢复和清理 |
| [输入与焦点](input-focus.md) | 输入资格、已有手势撤销、焦点、活动查询和窗口返回 |

## 共同职责边界

Core 决定匹配、请求顺序、逻辑 frame/generation、history、恢复和 presentation 发布。View 提供渲染无关的 key、descriptor、registry 与只读 effect/input。

ECS 执行 host/slot/frame 绑定、Scene 创建、staging、输入/focus、发布准备和 cleanup。BSN 提供 Scene factory、outlet 声明与 build error。Facade 组合常用 API 和可选 features；生命周期 owner 和 registry 写入由内部受控交接管理。

公共签名见源码和 rustdoc。修改某一行为时，同步更新对应专题及其测试；跨层机制的开发入口见[开发指南](../development/README.md)。
