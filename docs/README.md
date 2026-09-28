# Bevy UI Router 文档索引

**更新时间**：2026-09-28。本项目是独立的通用 Bevy UI Router，面向开源发布。嵌套分支因 query/search 变化重建后的 Back 缺陷已修复，并通过官方 Bevy 基线的完整回归；当前范围与验收见 [MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md) 和 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。本库验收以仓库内可复现的测试、示例和发布检查为依据，不依赖特定消费应用的业务、测试状态或发布流程。Linux/WSLg 窗口使用软件渲染；Windows 原生、最低 Rust、物理 GPU 加速与硬件设备输入未验证。尚未正式发布，具体许可证待确定。

## 当前依据

已确认用户决策和 [AGENTS.md](../AGENTS.md) 优先。文档分别承担不同职责：

| 文档 | 职责 |
| --- | --- |
| [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md) | 产品范围、架构、公共语义、设计理由与验收结果；状态以文件头为准 |
| [唯一实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md) | 迁移依赖、结果阶段、验证、审查和提交边界 |
| [Current Architecture](current-architecture.md) | 当前代码事实与缺口，不把目标写成已实现 |
| [Roadmap](roadmap.md) | Now / Next / Post-MVP，不复制合同 |
| [性能基线](performance.md) | 最小 benchmark 的运行方式、计时边界与测量结果；不是性能 SLA |
| [历史证据索引](superpowers/README.md) | 已完成阶段、研究和审查的追溯入口 |

## 历史协议（原位归档）

旧 [Target](target-architecture.md)、[Matrix](authority-transition-matrix.md)、[Fixture](facade-contract-fixture.md)、[Allowlist](facade-export-allowlist.md)、[Golden](facade-associated-api-golden.txt)、[Journeys](mvp-acceptance-journeys.md) 与 [Goals](implementation-goals.md) 已原位归档，不再独立定义新增 MVP 范围。必要语义已迁入统一 spec，测试不再解析这些文件；保留原路径与正文供历史追溯，不再维护多表/API 同步。其“canonical”或 Authority closure 前置表述均属于历史。

[Root profile](root-push-contract-profile.md)、[Nested profile](nested-push-contract-profile.md) 和已完成 History/Recovery profiles 同样属于旧阶段证据，相关 parser 已删除。固定 catalogue、public audit/probe、background reaper/fairness 不因旧表存在而成为新 MVP 验收门。

## 参考与证据口径

[全局工程决策指导](global-architecture-engineering-decision-guide.md) 和编号研究资料是非规范参考，不能另立范围或完成标准；引用旧依赖/API 时核对当前官方版本。

实现存在、生产集成、推荐 facade 可达和真实用户旅程通过分别记录。当前 MVP 结论以统一 spec/plan 的最新修复与验收记录为据；历史 MVP/Recovery PASS、探针与单个检查点不能证明新组合已通过。未运行的平台项目继续按 NOT_RUN 记录。
