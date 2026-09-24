# Roadmap

**更新时间**：2026-09-24。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## Now

- 官方 native BSN、输入/focus、公开导航/恢复与推荐示例已完成。旧公开机制和冗余私有 fixture 已按真实调用者清理，Core/ECS/public 独立及交叉审查、冻结源码全量验证通过；本地 main 已快进集成，合并后 smoke 通过。
- 补充独立窗口示例、六包打包配置、打包消费者验证和单 Linux CI。Linux/WSLg 真实窗口、软件渲染和 OS 注入键鼠交互已验证；图形示例独立 workspace，避免污染 headless 测试。

## Next

- 按实际使用反馈维护已验收的 MVP；按所有者选择暂不授予开源许可证，先完成发布准备。正式发布前确定许可与版本。Windows 原生、最低 Rust、物理 GPU 加速和硬件设备输入未验证，不为这些未运行项扩张 MVP 功能或测试矩阵。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。

统一 spec/plan 与范围评审已完成；旧多文档/AST gate 已替换为实际 consumer、LF/CRLF 对照和语义依赖检查，历史协议原位归档。native 工作区已有真实运行路径，仍不能将历史基线或局部通过视为 BSN MVP 验收。
