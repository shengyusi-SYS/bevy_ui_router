# MVP Documentation Guardrails Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Status:** Complete

**Implementation commits:** `bcef428`、`bb4fabe`、`7eefc02`、`a737923`

**Final reviews:** 规范质量 PASS、事实一致性 PASS、跨模块架构 PASS

**Goal:** 沉淀后续 MVP 切片可复用的 authority 与验收护栏，并把 tracked 文档更新到 Reconciliation Task 5 的真实 checkpoint

**Architecture:** 长期不变量进入 `AGENTS.md` 与 Authority Matrix，阶段计划纪律进入 `implementation-goals.md`，当前摘要进入 architecture、roadmap、contract 与 active plan。Authority Matrix 只可携带窄范围 implementation checkpoint，用于区分 target semantics 与 implemented scope。Task 5 的私有类型、排错过程和测试数字不进入长期规范

**Tech Stack:** Markdown、Git、PowerShell、`rg`

## Global Constraints

- 不新增独立经验汇总或第二份路线图
- 不重复已有 current/next、稀疏拓扑、typed preflight 与 publication-last 条款
- 不把 Task 5 私有类型、具体 caller、测试数量或排查过程写成通用架构
- 不宣称 Reconciliation PASS、完整 Recovery 或完整 workspace test PASS
- History、Recovery、Authority closure 与 MVP 保持未完成
- Root agent 是唯一 index/commit owner，子代理不得 stage 或 commit

---

### Task 1: 固化跨阶段实现与验收护栏

**Files:**

- Modify: `AGENTS.md`
- Modify: `docs/implementation-goals.md`
- Modify: `docs/authority-transition-matrix.md`

**Interfaces:**

- Consumes: `docs/superpowers/specs/2026-07-20-mvp-documentation-guardrails-design.md`
- Produces: History、Recovery、Authority closure 可直接引用的 owner、projection、evidence、closure 与 journey 约束

- [x] **Step 1: 更新 `AGENTS.md` 核心硬约束**

在现有 identity、typed preflight 与 publication-last 条款附近加入以下语义，允许为消除重复而合并相邻原句

```text
lifecycle capability 必须线性持有并单次移动或消费；snapshot、fingerprint 与 projection 只能校验，不能重建 owner，拒绝路径必须返还原 owner
延迟执行或 one-shot lifecycle action 必须冻结完整 source projection，消费前从当前 authority 重建并要求 exact equality，逐元素仍有效不足以授权 mutation 或 publication
boundary evidence 在 freeze、classify、settle 间不得降为 Entity、bool、空值或通用错误，必须保留 source、exact expectation、observed state、关联 obligation 与 owner；只有可证明未来可完成的容量或前置事实不足可以 retry，永久矛盾必须有界 fail closed
lifecycle mutation/focus set 必须来自完整 observed relationship closure 和显式 role classification
exact entering、leaving、pending cleanup 或 authority-invalid current 可以按 active plan 成为 authorized target
role classification 先确定 authorized target，剩余 preserved、non-target current、app-owned、foreign 与 other-authority non-target closure 保持为 protected，non-target current 只包含 authorized target role 之外的 current Entity
authorized target closure 与每个 non-target protected closure 不相交，mutation 只能落在 allowed write set 内
可能触发 hook/observer 的 staging、cleanup 或 recovery 写入后重建并复验 target/protected partition
atomic publication tail 不包含会触发 hook/observer 的写入、query 或可失败 validation
```

- [x] **Step 2: 更新 `AGENTS.md` 验证要求**

加入与 Step 1 一一对应的最小验证条款

```text
拒绝、重试与 completion error 后 capability 仍唯一可追踪
延迟 action 的完整 source projection 在消费时 exact refreeze
永久 drift 不进入无界 retry，typed evidence 与 owner 不在 helper 边界丢失
role classification 允许 exact lifecycle target，同时保持 authorized target 与 non-target protected closure 不相交
每个 hook/observer-triggering staging、cleanup 或 recovery 写入后都重建并复验 target/protected partition
Ready 前完成 preparation，atomic publication tail 不做 refreeze、query、可失败 validation 或 reclassification
```

- [x] **Step 3: 更新阶段计划与完成证据规则**

在 `docs/implementation-goals.md` 的执行与质量门槛加入

```text
History、Recovery 与 Authority closure 的阶段 spec/plan 必须先列出角色×拓扑×物理漂移的终态矩阵，每格明确 exact owner、允许写集、receipt/terminal 与 retry 条件
lifecycle/authority 终态至少由一条真实 producer 经正式 system schedule 到 settler 的旅程证明，并覆盖多轮收敛与额外一轮 no-replay；注入 fact、直接 finalizer 与私有分类测试只能补充
Task PASS 前除 focused RED/GREEN 外，还必须通过受影响完整 crate 与相关 facade/consumer gate；未运行门禁只能记录为待完成
阶段完成后同步 current architecture、roadmap、authority matrix 与阶段记录，只有 completion gate 通过后才能标记阶段 PASS
```

- [x] **Step 4: 补充 Authority Matrix 共通语义**

在文档标头增加 `ddce26d` checkpoint，明确仅最小 pre-commit settlement 已实现，post-commit Recovery 仍待完成

在状态轴或 token 共通规则中加入完整 source projection、线性 capability 与 typed evidence 要求

switch 进入 `Ready` 前，entering staging 已完成全部 hook/observer-triggering write 并逐次 post-write validated，随后的只读 switch preflight 冻结 role partition，并先确定 exact leaving 与 entering lifecycle value 作为 authorized switch target

剩余 preserved、non-target current 与 protected non-target closure 与 authorized switch target 不相交，non-target current 只包含 authorized switch target role 之外的 current Entity

cleanup 与 authority settlement 的 hookful mutation 和 post-write revalidation 不属于 `CommitMutationPlan`，由 §7 versioned cleanup/settlement attempt 处理

在 `Ready` 后的 `CommitMutationPlan` 只应用已准备的 no-hook in-place values，atomic publication tail 不做 refreeze、query、可失败 validation 或 reclassification

- [x] **Step 5: 验证规范不重复、不越界**

Run:

```powershell
rg -n "source projection|boundary evidence|producer|no-replay|authorized target|non-target protected|pre-commit checkpoint" AGENTS.md docs/implementation-goals.md docs/authority-transition-matrix.md
rg -n "RetainedLost|PreparedAuthorityHostEdge|RetryableCapacity|六个生产|377" AGENTS.md docs/implementation-goals.md docs/authority-transition-matrix.md
git diff --check
```

Expected: 第一条命令能定位通用规则；第二条无输出；`git diff --check` exit 0

---

### Task 2: 更新 Reconciliation 当前事实与剩余路线

**Files:**

- Modify: `docs/current-architecture.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/reconciliation-contract-profile.md`
- Modify: `docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md`

**Interfaces:**

- Consumes: tracked commits `228c5bf`、`c627a9e`、`6f8e734`、`9415d03`、`4f4e4dc`、`ddce26d`
- Produces: Task 6–8 编写 brief、执行与 completion review 使用的唯一 tracked checkpoint

- [x] **Step 1: 更新 Current Architecture**

将标头改为

```text
状态: Reconciliation 实施中，Tasks 1–5 complete
as_of: Reconciliation Task 5，ddce26d
```

保留 Root 与 Nested PASS 事实，新增独立的 Reconciliation checkpoint，准确记录

```text
Core 已完成 slot-aware projection、preserved suffix splice 与 sealed logical publication
ECS 已完成 role-sensitive physical preflight、原子 splice、exact cleanup 与最小 pre-commit authority settlement
authority loss 使用既有唯一 publisher、ledger 与 receipt stream，清空失效 Core/ECS active，不 fallback、不 replay、不推进 history
Task 6 私有物理回归、Task 7 公共 Journey B/fragment、Task 8 completion gate 尚未完成
```

不得把 Task 5 focused gate 写成完整 workspace nextest PASS

- [x] **Step 2: 更新 Roadmap**

将状态更新为 Reconciliation 执行中，`Now` 只列 Task 6–8

```text
Task 6: private physical reconciliation regressions
Task 7: public Journey B、fragment 与 facade evidence
Task 8: Reconciliation completion gate 与 factual records
```

新增 `Next`，顺序保持 History → Recovery → Authority closure → MVP gate

- [x] **Step 3: 更新阶段 contract checkpoint**

在 `docs/reconciliation-contract-profile.md` 标头增加

```text
实现 checkpoint: Tasks 1–5 complete，ddce26d；Tasks 6–8 pending
```

在最小 pre-commit settlement 段落说明它复用既有唯一 ledger/receipt stream，终态后可重新 admission，但不会重放导航；不改变完整 Recovery 非目标

- [x] **Step 4: 更新 active implementation plan**

将状态改为 executing，并在标头增加进度表

```text
Task 1 complete: 228c5bf
Task 2 complete: c627a9e
Task 3 complete: 6f8e734
Task 4 complete: 9415d03 + 4f4e4dc
Task 5 complete: ddce26d
Tasks 6–8 pending
```

保留 Task 6–8 checkbox 与“只有 Task 8 后 Complete”的原门禁

- [x] **Step 5: 验证跨文档一致性**

Run:

```powershell
rg -n "Reconciliation 实施中|Tasks 1–5|ddce26d|Tasks 6–8|History|Recovery|Authority closure|MVP" docs/current-architecture.md docs/roadmap.md docs/reconciliation-contract-profile.md docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md
rg -n "Reconciliation PASS|完整 workspace.*PASS" docs/current-architecture.md docs/roadmap.md
git diff --check
git status --short
```

Expected: checkpoint 与剩余顺序在四份文档一致；`Reconciliation PASS` 只作为明确未达到的目标或门禁出现；无 whitespace error

---

### Task 3: 独立复审、修正与提交

**Files:**

- Review: Task 1 与 Task 2 的全部文档 diff
- Modify if needed: only files listed by Task 1 or Task 2

**Interfaces:**

- Consumes: 两个任务的实现报告与完整 diff
- Produces: 可提交的规范、事实基线与审查结论

- [x] **Step 1: 运行三项独立只读审查**

审查职责

```text
规范审查: 新规则是否通用、可验证且无重复
事实审查: commits、阶段状态、已完成与未完成边界是否准确
架构审查: 是否保持单一 publisher、Core/ECS authority 分层且未提前实现 Recovery
```

- [x] **Step 2: 修正全部 Critical 与 Important finding**

只修改计划列出的文档，修正后由原职责 reviewer 复审

- [x] **Step 3: 运行最终文档门禁**

Run:

```powershell
git diff --check
git status --short
git diff --name-only
```

Expected: 只出现计划列出的 tracked Markdown 文件，无 staged 文件，无 whitespace error

- [x] **Step 4: 精确暂存并提交已分步完成**

提交边界

- `bcef428`：通用 lifecycle evidence 护栏
- `bb4fabe`：Reconciliation Task 5 事实 checkpoint
- `7eefc02`：Tasks 1–5 checkbox 一致性修正
- `a737923`：target partition、atomic-tail 与非规范记录修正
- 本文状态由其所在 closeout commit 闭合
