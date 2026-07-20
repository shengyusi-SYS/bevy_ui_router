# 历史计划与审查证据

**状态**: History active-profile exception；其余 frozen historical evidence
**冻结日期**: 2026-07-10
**冻结基准**: `23fe826033d09716553dd6922dd204c4a45b1a6c`
**superseded_by**: `docs/current-architecture.md`、`docs/target-architecture.md`、`docs/mvp-acceptance-journeys.md`

## 当前 History profile

[History 阶段设计](specs/2026-07-20-history-design.md) 的 `10.1 Public contract and stage exclusions`、`10.2 Contract delta activation` 与 `10.3 History authority regression additions` 是实施期间唯一 active stage profile。它们只选择当前切片的既有 canonical contract，不建立第二份 roadmap 或规范正文。

[History 实施计划](plans/2026-07-20-history-implementation-plan.md) 只约束任务顺序与验证边界，不改变 canonical authority，也不定义当前完成度。其他 historical spec、plan 和审查记录均保持 evidence-only。

## 证据范围

本目录下现有 `plans/` 与 `specs/` 文件记录过去的决策、任务拆分、审查过程和当时的验证结论。除上节列出的 History active profile 外，它们不定义当前架构、当前进度或 MVP 完成度。

这些文件仍可用于：

- 追溯某个类型、负向合同或测试为何存在。
- 判断历史任务当时是否按其局部范围通过。
- 在重构时识别应保留的可靠算法与已知风险。

这些文件不可用于：

- 证明当前 workspace 仍处于单 crate 或无实现状态。
- 证明 Phase 3B 已整体完成或真实导航已公共可达。
- 覆盖当前 Target Architecture 的 mount、registry、atomic commit 或 facade 决策。
- 以旧的 `PASS` 标签替代 Journey PASS 或 MVP PASS。

## 维护规则

- 原文件保留原路径和正文，原则上不再整理或重写。
- 只有证据损坏、链接错误或明确事实误植时才做最小修正，并在提交中说明。
- 新的 Phase 3B.1 实施计划可以继续存放在 `plans/`，但必须在文件头显式标记 `status`、适用的 Target Architecture 版本和验收文档。
- 新审查记录必须区分事实发现、设计判断、负责人处置和最终结论。
