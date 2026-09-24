# Bevy UI Router 文档索引

**更新时间**：2026-09-24。包含官方同步 BSN adapter 的 MVP 已通过既定功能验收，并补充 Linux/WSLg 真实窗口、软件渲染与 OS 注入键鼠交互验证；Windows 原生、最低 Rust、物理 GPU 加速与硬件设备输入未验证。当前按所有者选择先完成发布准备，不授予开源许可证或正式发布。旧 data-only API 无兼容义务，基础路由、内部 outlet、renderer-neutral 边界与输入/focus 责任仍保留。

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

实现存在、生产集成、推荐 facade 可达和真实用户旅程通过分别记录。当前 MVP 结论以统一 spec 的实际验收、独立审查和本地 main 集成为据；历史 Recovery PASS、探针与单个检查点不能单独证明它。未运行的平台项目继续按 NOT_RUN 记录。
