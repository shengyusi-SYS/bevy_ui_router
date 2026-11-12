# 路线图

[English](roadmap.md) | 简体中文

当前能力见[产品概览](overview.zh-CN.md)，具体行为见[行为合同](contracts/README.zh-CN.md)。本页记录后续工作。

## 正在进行

| 工作 | 交付内容 |
| --- | --- |
| 公开文档 | 中英文使用与开发指南、可运行示例入口、文件和隐私检查 |
| 公开查询性能 | 活动、当前输入和待恢复滚动查询共用借用期读取会话；[交付与验证计划](changes/query-performance/plan.md) |
| 发布准备 | 双许可证已确定为 MIT OR Apache-2.0；确定发布版本后，验证最终打包产物与独立消费方 |

具体动画效果由应用/UI 层实现。项目采用 MIT OR Apache-2.0 双许可证；发布版本留待维护者确定，当前接入方式是仓库本地依赖。

当前源码面向 Bevy `0.20`，依赖与独立 consumer 声明 `0.20.1`，可选 Tween 适配器声明 `bevy_tween 0.14.1`。兼容范围与检查见[版本要求](guides/integration.zh-CN.md#bevy-版本要求)和[验证指南](development/verification.zh-CN.md)。

## 后续方向

原生 Tab 导航与输入分发自动接入路由活动资格，需要 Bevy/UI 提供适合的公开扩展入口。该方向同时包括焦点准入和当前焦点桥的替换。现有应用可继续通过 `RouterInputGate` 接入键盘和手柄处理器。

其它方向包括浏览器 URL/history、异步 Scene 加载、typed route/search、多出口、其它 renderer 和公开实例逐出接口。每项工作先明确应用场景、期望行为和验证方法，再进入实施计划。

## 发布顺序

确定发布版本，更新 Cargo 元数据；随后运行[验证指南](development/verification.zh-CN.md)中的检查和 Cargo 打包验证。六个 crate 的依赖顺序为 Core → View → BSN / Platform → ECS → facade，其中 Platform 只依赖 Core。
