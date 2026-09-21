# Roadmap

**更新时间**：2026-09-21。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## Now

- 候选精简、独立迁移基线及 idle host-loss 已完成；native 解析/创建及 pointer/publication 私有检查点已落地，生产 BSN 尚未接通。当前以真实 facade root 成功与失败闭环推进 BSN，联动 owner、内容边界及输入/focus，不扩成旧 renderer 全面整顿。

## Next

- 交付官方同步代码驱动 BSN root 切片，包括创建期分配责任、current containment、真实输入与 focus 隔离。
- 完成内部 default outlet 的嵌套布局、history、恢复和 host/stale 的真实 BSN 闭环；联动替换旧内容模型并保留 renderer-neutral 独立验证。
- 通过推荐 facade 的独立 BSN consumer、示例、全量验证与独立交叉审查后集成 main；完成前不标记 MVP PASS。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。

统一 spec/plan 与范围评审已完成；旧多文档/AST gate 已替换为实际 consumer、LF/CRLF 对照和语义依赖检查，历史协议原位归档。该进展不表示 BSN 产品路径已实现。
