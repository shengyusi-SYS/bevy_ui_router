# Bevy UI Router Implementation Readiness Brief Review Log

**日期:** 2026-07-07
**关联文档:** `docs/superpowers/plans/2026-07-07-implementation-readiness-brief.md`
**状态:** 审查通过；不需要整理型重写

---

## 1. 流程说明

本记录说明 Task 0 readiness brief 的审查、负责人评估、修订和整理重写判断。

审查遵循当前项目规划流程：

- 先由负责人编写 readiness brief。
- 再交给多个子代理按职责方向只读审查。
- 子代理只给职责方向，不指定逐项审查清单。
- 负责人先评估审查结果和共同根因，再决定是否修订。
- 修订后进入复审。
- 复审通过后，再让子代理整体判断是否需要整理型重写。
- 若不需要重写，则保留当前文档结构并进入验证与提交。

---

## 2. 第一轮审查方向

第一轮使用五个只读审查方向：

- 流程门禁与执行流一致性。
- 架构、模块边界与 public API 归属。
- 验证、TDD 证据与测试入口。
- Phase 0 冻结粒度与后续拆分。
- 事实性审查。

第一轮没有发现需要推翻主路线图或重新规划的宏观问题。审查发现的问题集中在 readiness brief 的表达层和阶段归属上。

---

## 3. 负责人归因

第一轮问题有共同根因：初稿在把 Phase 0-3 的输入收敛为 Phase 0 readiness brief 时，少数条目把后续阶段的机制、integration 名称或诊断标签写得过像 Phase 0/core 决策。

该根因表现为：

- stale/pre-commit 身份校验维度从主规划的 router、host、slot、frame、generation 和 token 被压缩为 router、frame、generation。
- generation 具体递增时机被写成 Phase 0 决策，而主路线图要求具体时机在 Phase 2 冻结。
- fake renderer 的具体暴露位置被写成 Phase 0 选择，而执行流把 view contract 和 fake renderer 计划放在 Phase 3A。
- error taxonomy 使用 `EcsFinalizeFailed` 和具体 source layer 名称，容易让 core public API 固化 ECS/BSN/platform 概念。
- TDD/gate evidence 摘录短于 execution-flow 的完整证据模型。
- route root layout constraint 没有明确 Phase 0 抽象冻结与 Phase 3B 具体 ECS 验证的承接关系。

负责人判断：这些问题不要求推翻架构，也不要求重写整份文档；它们应通过定点修订解决。

---

## 4. 已采纳修订

已对 readiness brief 做以下修订：

- 在状态机图中补充 `Finalizing --> Stale` 路径。
- 将 stale check 和 pre-apply/pre-commit 维度统一为 router、host、slot、frame、generation 和相关 token。
- 将 generation 条目改为 Phase 0 冻结不变量，Phase 2 冻结具体递增时机。
- 将 fake renderer 条目改为 Phase 0 冻结边界，Phase 3A 冻结具体 crate、feature 和 API visibility。
- 将 `EcsFinalizeFailed` 改为中性的 `FinalizeFailed`。
- 明确 core error enum 不得固化 Bevy、ECS、BSN、platform package 或 app profile 名称。
- 将具体 renderer 或 integration 名称限制为 adapter-provided redacted diagnostics label。
- 补充 partial output cleanup handoff：adapter 只能报告 partial output handles、failure 和 disposal hints；真正 cleanup 由 Router/ECS cleanup API 在 identity、stale 和 ownership gate 下执行。
- 明确 core 拥有 router-neutral back intent/policy/capability 语义，platform crate 只做事件翻译、source helper、targeting helper 和 re-export glue。
- 补充 route root layout constraint 的 Phase 0 抽象冻结和 Phase 3B 具体化验证承接。
- 明确完整继承 execution-flow §6.2、§6.3、§6.4 的证据模型。
- 补充 target crate 尚不存在时的 unavailable command 处理。
- 将结尾改为只能创建 Phase 0 detailed plan，代码实现仍需等待 Phase 0 plan、自检、审查 gate 和用户执行模式确认。

---

## 5. 第二轮复审结论

第二轮复审仍由五个方向只读进行。

结论：

- 流程门禁方向通过；修订后没有实现授权误读。
- 架构/API 方向通过；error taxonomy、source category、cleanup handoff 和 BackIntent 归属已收敛。
- 验证方向通过；TDD/gate evidence、unavailable command 和 layout constraint 承接已对齐 execution-flow。
- Phase 0 粒度方向通过；stale、generation、fake renderer 和后续阶段机制均回到正确阶段。
- 事实性方向通过；仓库事实、Phase 0-3 拆分、feature baseline 和 BSN/Bevy 边界一致。

复审仅保留一个非阻塞措辞建议：状态机图中的 finalization 失败应保持中性表达。负责人已将该处改为 `finalization fails`。

---

## 6. 整理重写评估

复审通过后，五个子代理整体评估是否需要整理型重写。

一致结论：不需要整理型重写。

理由：

- 当前章节顺序正好对应 Task 0 输入包：文档定位、权威输入、Phase 0 API 边界、状态机、错误分类、adapter 权限、测试入口、示例、开放决策和 handoff。
- 文档有少量重复，但这些重复分别服务于冻结项、测试入口和 handoff checklist，属于 gate 文档的有益冗余。
- 强行重写收益低，风险是重新扰动已通过审查的阶段边界和 core/view/ecs/bsn/platform 归属。
- 提交前只需要轻量元数据清理，不需要新建候选重写版。

负责人采纳该结论：保留当前 readiness brief 结构，不创建重写候选，不替换正文。

---

## 7. 当前结论

`docs/superpowers/plans/2026-07-07-implementation-readiness-brief.md` 已通过 Task 0 readiness review。

下一步只能创建 `docs/superpowers/plans/2026-07-07-phase-0-contract-freeze-implementation-plan.md`。代码实现仍不得开始，直到 Phase 0 detailed plan 完成、自检、审查 gate 通过，并完成用户确认的执行模式。
