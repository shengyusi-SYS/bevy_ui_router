# Bevy UI Router Phase 0-3 Execution Flow Review Log

**日期:** 2026-07-07
**关联计划:** `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md`
**状态:** 已完成审查、修订和二轮收敛

---

## 1. 审查范围

本记录覆盖 Phase 0-3 execution flow 的流程级审查。审查目标不是重新评估 Router 架构，而是确认后续执行流程是否具备：

- 阶段门禁。
- 计划接受门。
- TDD 证据。
- 验证失败阻塞规则。
- 跨阶段放行证据。
- Phase 3A-3D 子阶段拆分。
- 与最终 roadmap 和当前仓库事实的一致性。

---

## 2. 第一轮审查方向

第一轮只读审查包含四个方向：

- 流程与门禁审查。
- 任务粒度与拆分审查。
- TDD、验证与证据审查。
- 事实一致性审查。

一个初始流程与门禁审查子代理因模型容量失败，随后补派同职责审查。

---

## 3. 第一轮共同根因

第一轮审查认为 execution flow 的方向正确，但暂不通过。共同根因是：

1. 证据门禁不够硬，存在用“解释失败”替代“验证通过”的风险。
2. TDD RED/GREEN 证据没有形成可审计对象。
3. Phase 3 gate 粒度与执行粒度不一致，3A-3D 被写成一个统一计划和一次最终验证。
4. 下一阶段入口只检查提交和工作区状态，不足以证明上一阶段 gate evidence 已固化。
5. 阶段计划写完后的计划接受门不够显式。
6. 旧决策文档中 facade default feature 与 Phase 3 聚合表述可能被误读。

这些问题不要求推翻 Phase 0-3 总体拆分，但要求修订 execution flow，并同步旧决策文档中的已收敛口径。

---

## 4. 已修订内容

### 4.1 Gate Evidence Model

新增 gate evidence model，明确：

- 每个 detailed plan 必须先通过 plan acceptance gate。
- 每个 implementation task 必须记录 RED/GREEN 证据。
- 每个 phase/sub-phase gate 必须记录 final commit SHA、验证命令、exit code、review checklist、风险和授权续行。
- `git log` 与 `git status` 只能辅助证据记录，不能替代 gate evidence。

### 4.2 失败验证阻塞

修订验证规则：

- required verification command failed 时，phase/sub-phase 不得完成。
- 环境限制导致命令无法运行时，gate 只能标记为 unverified 或 blocked。
- 只有用户明确书面接受风险时，才能继续。

### 4.3 TDD Evidence Ledger

新增 TDD evidence ledger，要求记录：

- RED command、exit code、output summary。
- GREEN command、exit code、output summary。
- tests added。
- full verification commands。
- commit SHA。
- skipped/failed command 的阻塞状态。

测试后补不能冒充 TDD，除非 RED 失败已经在实现前被记录。

### 4.4 Implementation Readiness Brief

新增 Task 0，要求在 Phase 0 detailed plan 前创建 implementation readiness brief，覆盖：

- Public API draft scope。
- State machine diagram scope。
- Error taxonomy finalization target。
- Adapter responsibility table scope。
- Test harness outline。
- Minimal examples outline。
- Phase 0 必须冻结的开放决策。

### 4.5 Phase 3A-3D 独立门禁

将原本统一的 Phase 3 detailed plan/execute 任务拆成：

- Phase 3A view contract/fake renderer。
- Phase 3B ECS integration。
- Phase 3C immediate BSN adapter。
- Phase 3D minimal nested routes example。

每个子阶段都有自己的 plan creation、self-review、plan acceptance gate、execution、verification、review gate 和 gate evidence。

### 4.6 局部任务继承全局来源

修订各 phase plan creation task，使其显式读取 Section 2 的全局 source documents。

### 4.7 文档自检统一

修订各 phase plan self-review：

- placeholder scan。
- trailing whitespace scan。
- `git diff --check`。

### 4.8 旧决策文档口径同步

同步 `docs/superpowers/specs/2026-07-07-project-decisions-design.md`：

- facade 默认 surface 表示 re-export 路径，不表示 default feature 启用 ECS。
- 当前规划基线是 default 不拉 Bevy；ECS、BSN、platform helper 显式 opt-in。
- examples 使用 workspace examples 或 dev-only package。
- Phase 3 在旧决策文档中保持聚合表述，执行级拆分以 execution flow 的 Phase 3A-3D gate 为准。

---

## 5. 第二轮审查结论

第二轮只读审查确认第一轮阻塞问题已经收敛：

- 流程与门禁方向通过。
- 任务粒度与拆分方向通过。
- TDD、验证与证据方向通过。
- 事实一致性方向通过。

第二轮仅提出非阻塞建议：

- Phase 3C 的 RED/GREEN “when feasible” 表述应绑定到 Section 6.4 的 unverified/blocked 规则。
- Phase 1/2 verification expected 可以补充 failed commands block completion。
- Task 9 completion review 可以显式要求复核所有 phase gate evidence artifact。
- 旧决策记录应说明 Phase 3 执行级拆分以 execution-flow 为准。

负责人已按这些建议做小修。

---

## 6. 当前结论

Phase 0-3 execution flow 已通过审查。当前不需要继续拆分总纲。

后续应按总纲进入：

1. Task 0: implementation readiness brief。
2. Phase 0 detailed implementation plan。
3. Phase 0 plan review and execution mode selection。

任何实现代码都必须等对应 detailed plan 通过 plan acceptance gate 后才能开始。
