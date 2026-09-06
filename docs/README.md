# Bevy UI Router 文档索引

**更新时间**：2026-09-07。当前以包含官方同步 BSN adapter 的完整 MVP 为目标；统一设计已通过独立审查及交叉复核，进入实施，产品尚未完成。旧 data-only API 无兼容义务，不因此删去基础路由、内部 outlet、renderer-neutral 边界或输入/focus 责任。

## 当前依据

已确认用户决策和 [AGENTS.md](../AGENTS.md) 优先。文档分别承担不同职责：

| 文档 | 职责 |
| --- | --- |
| [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md) | 产品范围、架构、公共语义、设计理由与验收结果；状态以文件头为准 |
| [唯一实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md) | 迁移依赖、结果阶段、验证、审查和提交边界 |
| [Current Architecture](current-architecture.md) | 当前代码事实与缺口，不把目标写成已实现 |
| [Roadmap](roadmap.md) | Now / Next / Post-MVP，不复制合同 |
| [历史证据索引](superpowers/README.md) | 已完成阶段、研究和审查的追溯入口 |

## 历史协议（原位归档）

旧 [Target](target-architecture.md)、[Matrix](authority-transition-matrix.md)、[Fixture](facade-contract-fixture.md)、[Allowlist](facade-export-allowlist.md)、[Golden](facade-associated-api-golden.txt)、[Journeys](mvp-acceptance-journeys.md) 与 [Goals](implementation-goals.md) 已原位归档，不再独立定义新增 MVP 范围。必要语义已迁入统一 spec，测试不再解析这些文件；保留原路径与正文供历史追溯，不再维护多表/API 同步。其“canonical”或 Authority closure 前置表述均属于历史。

[Root profile](root-push-contract-profile.md)、[Nested profile](nested-push-contract-profile.md) 和已完成 History/Recovery profiles 同样属于旧阶段证据，相关 parser 已删除。固定 catalogue、public audit/probe、background reaper/fairness 不因旧表存在而成为新 MVP 验收门。

## 参考与证据口径

[全局工程决策指导](global-architecture-engineering-decision-guide.md) 和编号研究资料是非规范参考，不能另立范围或完成标准；引用旧依赖/API 时核对当前官方版本。

实现存在、生产集成、推荐 facade 可达和真实用户旅程通过分别记录。审查通过或探针通过不等于产品 gate 通过；历史 Recovery PASS 也不等于当前 BSN 或 MVP PASS。只有统一 spec 的实际验收、独立审查和集成全部完成，才能结束总体 MVP 目标。
