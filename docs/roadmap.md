# Roadmap

**更新时间**：2026-09-23。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## Now

- 官方 native BSN、输入/focus、公开导航/恢复与推荐示例已完成。旧公开机制和冗余私有 fixture 已按真实调用者清理，Core/ECS/public 独立及交叉审查、冻结源码全量验证通过；本地 main 已快进集成，合并后 smoke 通过。

## Next

- 按实际使用反馈维护已验收的 MVP；Windows 原生、最低 Rust 版本及窗口/GPU/真实设备输入仍需在有相应环境时单独验证。远端发布另行决定。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。

统一 spec/plan 与范围评审已完成；旧多文档/AST gate 已替换为实际 consumer、LF/CRLF 对照和语义依赖检查，历史协议原位归档。native 工作区已有真实运行路径，仍不能将历史基线或局部通过视为 BSN MVP 验收。
