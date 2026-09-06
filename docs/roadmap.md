# Roadmap

**更新时间**：2026-09-07。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## Now

- 收口范围/架构/验证去复杂化评审，统一 spec、plan、工程护栏与事实索引。
- 从当前候选前向迁移，精简 public audit/probe/background reaper 外扩面；保留有效前台修复、私有残留隔离责任和已有导航行为。
- 将多文档互证的 gate 迁往可编译 consumer 与真实行为回归，保留 LF/CRLF 和依赖语义检查。

## Next

- 交付官方同步代码驱动 BSN root 切片，包括创建期分配责任、current containment、真实输入与 focus 隔离。
- 完成内部 default outlet 的嵌套布局、history、恢复和 host/stale 闭环；保留 renderer-neutral 独立验证。
- 通过推荐 facade 的独立 BSN consumer、示例、全量验证与独立交叉审查后集成 main；完成前不标记 MVP PASS。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。
