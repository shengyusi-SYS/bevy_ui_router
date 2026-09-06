# 当前规划与历史证据

## 当前 MVP

- [统一 MVP spec](specs/2026-09-07-router-mvp-design.md)：范围、架构、公共语义和验收；状态以文件头为准。
- [唯一实施 plan](plans/2026-09-07-router-mvp-implementation.md)：结果阶段、依赖、验证、审查和提交。

二者是当前工作入口，其它旧阶段文档是历史证据，不另立当前路线。当前实现事实见 [Current Architecture](../current-architecture.md)。

## 已完成阶段的追溯

| 阶段 | 设计/合同 | 实施与证据 |
| --- | --- | --- |
| Root Push | [profile](../root-push-contract-profile.md) | [plan](plans/2026-07-11-root-push-implementation-plan.md)、[review](plans/2026-07-11-root-push-review-log.md) |
| Nested Push | [profile](../nested-push-contract-profile.md) | [plan](plans/2026-07-16-nested-push-implementation-plan.md) |
| Reconciliation | 旧 [Target](../target-architecture.md) / [Matrix](../authority-transition-matrix.md) 中对应合同 | [历史 plans 目录](plans/) 中阶段记录 |
| History | [spec](specs/2026-07-20-history-design.md) | [plan](plans/2026-07-20-history-implementation-plan.md) |
| Recovery | [spec](specs/2026-08-12-recovery-design.md) | [plan](plans/2026-08-12-recovery-implementation-plan.md) |

History、Recovery profiles 保存各自完成时的机器表和验证结果，不能据此要求新 MVP 维持固定 catalogue 或旧 Authority closure 前置；Recovery PASS 不包括本次生产 BSN 与完整 MVP。旧 Target/Matrix/Fixture/Allowlist/Golden/Journeys/Goals 与 Root/Nested profiles 已原位归档，保留正文与路径，相关 Markdown/Golden/AST gate 已被真实 consumer 替代。

## 维护规则

历史记录可用于追溯设计理由、已知风险与有效回归，不用于覆盖新合同或证明当前树已通过验证。一般保留正文与路径；仅纠正证据损坏、链接或事实误植。旧规范在必要语义和 parser/consumer 依赖迁移后可按计划删除/归档。

临时评估稿在必要结论迁入正式 spec/plan 并提交后删除，不建设永久第二套规划。新审查记录区分源码事实、设计判断、负责人处置和实际验证；已完成计划归历史，不再保留并行 active 状态。
