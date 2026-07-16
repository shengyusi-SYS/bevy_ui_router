# Bevy UI Router 文档索引

**状态**: Phase 3B.1 `Plan PASS`
**基准提交**: `23fe826033d09716553dd6922dd204c4a45b1a6c`
**更新时间**: 2026-07-11

## 权威顺序

发生冲突时，按以下顺序判断：

1. 已确认的用户决策与仓库根目录 [`AGENTS.md`](../AGENTS.md)。
2. [Target Architecture](target-architecture.md) 的架构边界、依赖方向与不变量。
3. [Authority Transition Matrix](authority-transition-matrix.md) 的生命周期转换细节；若生命周期细节与 Target 的概括冲突，以 Matrix 为准，但不得违反 Target 的架构不变量。
4. [Facade Contract Fixture](facade-contract-fixture.md) 的公共签名与消费者装配形状。
5. [Facade Export Allowlist](facade-export-allowlist.md) 的公共 module、path、prelude 与 feature 边界。
6. [Facade Associated API Golden](facade-associated-api-golden.txt) 的逐类型 associated item、trait 与签名类型图闭包。
7. [MVP Acceptance Journeys](mvp-acceptance-journeys.md) 的外部行为和验收门槛。
8. [Current Architecture](current-architecture.md) 对基准提交的事实描述。
9. [Roadmap](roadmap.md) 的执行顺序。
10. 历史计划、研究资料和审查记录仅作为证据，不定义当前目标。

## 当前文档

| 文档 | 作用 |
| --- | --- |
| [Current Architecture](current-architecture.md) | 记录当前代码已经实现、已经集成、公共可达和已演示的能力 |
| [Target Architecture](target-architecture.md) | 定义 Phase 3B.1 的模块边界、身份模型、数据流和提交语义 |
| [MVP Acceptance Journeys](mvp-acceptance-journeys.md) | 从 facade 公共 API 描述 MVP 必须通过的完整用户旅程 |
| [Facade Contract Fixture](facade-contract-fixture.md) | 固定可编译的消费者装配、fault driver 与 trace 观察合同 |
| [Facade Export Allowlist](facade-export-allowlist.md) | 固定逐 feature、逐 module、逐 symbol 的公共导出边界 |
| [Facade Associated API Golden](facade-associated-api-golden.txt) | 固定所有允许类型的字段、variant、method、trait 与签名闭包 |
| [Authority Transition Matrix](authority-transition-matrix.md) | 固定 token、closure、preflight、quarantine、epoch 与 request 终态 |
| [Roadmap](roadmap.md) | 记录 Now、Next 和暂停项，不重复架构正文 |
| [整体实施目标规划](implementation-goals.md) | 概括 MVP 实施目标、阶段主线、质量门槛与范围控制 |
| [Root Push 阶段合同 Profile](root-push-contract-profile.md) | 从最终 facade 合同中选择 Root push 已兑现能力、延后项与回归 catalogue |
| [Nested Push 阶段合同 Profile](nested-push-contract-profile.md) | 收窄首次嵌套 push、sealed child mount、逐层 apply 与阶段 exclusions |
| [Root Push 实施计划](superpowers/plans/2026-07-11-root-push-implementation-plan.md) | Root push 的任务顺序、TDD 门禁、文件范围与提交边界 |
| [Root Push 计划审查记录](superpowers/plans/2026-07-11-root-push-plan-review-log.md) | 记录多角度审查的共同根因、修订、复审与无需重写结论 |
| [Phase 3B.1 Review Log](phase-3b1-architecture-review-log.md) | 记录审查共同根因、负责人处置和复审结论 |
| [历史证据说明](superpowers/README.md) | 冻结旧计划和审查记录，说明其证据范围 |

## 按问题查找

- 架构边界、crate归属与数据流：查 [Target Architecture](target-architecture.md)。
- token、commit、cleanup、quarantine与终态：查 [Authority Transition Matrix](authority-transition-matrix.md)。
- 公共签名与消费者装配：查 [Facade Contract Fixture](facade-contract-fixture.md)；导出路径查 [Facade Export Allowlist](facade-export-allowlist.md)；逐类型关联API查 [Facade Associated API Golden](facade-associated-api-golden.txt)。
- 用户可观察行为与最终门槛：查 [MVP Acceptance Journeys](mvp-acceptance-journeys.md)。
- 实施顺序与当前基线差距：分别查 [Roadmap](roadmap.md) 与 [Current Architecture](current-architecture.md)。

## 参考资料

- [01 BSN scene authoring boundaries](01-BSN-scene-authoring-boundaries.md) 与 [02 Web router patterns for Bevy](02-Web-router-patterns-for-Bevy.md) 是研究参考，不是当前实现事实。
- [03 Core architecture and MVP](03-Core-architecture-and-MVP.md)、[04 Risks, tests and handoff](04-Risks-tests-and-handoff.md) 与 [05 Non-MVP capabilities](05-Non-MVP-capabilities-in-Bevy.md) 是早期设计基线；其中与当前权威文档冲突的内容已被取代。

## 状态用语

- **Implemented**：局部算法或类型存在并有针对性验证。
- **Integrated**：能力已接入统一运行路径。
- **Low-level reachable**：外部可以通过 facade 的 crate/module re-export 直接调用局部协议，其中部分受 feature 控制。
- **Facade reachable**：外部只用推荐的高层 facade API 即可触发完整流程。
- **Demonstrated**：真实 `App::update` 旅程已证明用户可观察行为。

只有实现、集成、推荐 facade 可达和真实旅程演示全部成立，才能把一项能力计入 MVP 完成度；低层可达只记录兼容面。`Plan PASS`、`Task PASS`、`Journey PASS` 和 `MVP PASS` 必须分别使用，不能相互替代。
