# MVP 后续文档与验收护栏设计

**状态**: 已实施的非规范设计记录

**事实基线**: Reconciliation Task 5，`ddce26d`

**规范归属**: canonical requirements 由 `AGENTS.md`、`docs/implementation-goals.md`、Authority Matrix 与当前阶段文档持有，本文件只记录已实施设计，不是规范来源

## 目标

为 Reconciliation Task 6–8、History、Recovery、Authority closure 与 MVP gate 固化少量可复用护栏，并修正当前已过期的阶段事实文档

本次只沉淀能够降低跨阶段返工的规则，不保存局部 bug、私有类型、测试数量或排查过程

## 文档分层

| 文档 | 只负责 |
| --- | --- |
| `AGENTS.md` | 跨阶段实现硬约束与最低验证要求 |
| `docs/implementation-goals.md` | 阶段计划、验收证据与文档闭环 |
| `docs/authority-transition-matrix.md` | lifecycle、authority、cleanup 的详细转换语义 |
| `docs/current-architecture.md`、`docs/roadmap.md` | 当前事实与剩余顺序 |
| 阶段 contract 与 implementation plan | 当前阶段范围、任务和完成门禁 |

不新增独立经验汇总或第二份路线图

## 需要沉淀的护栏

1. lifecycle capability 必须线性持有，snapshot 或 fingerprint 只能作为 evidence，不能重建 owner
2. 延迟执行的 lifecycle action 必须冻结完整 source projection，消费前从当前 authority 重建并要求 exact equality
3. boundary evidence 在 freeze、classify、settle 间必须保留 source、expectation、observed state、obligation 与 owner，永久矛盾不得退化为通用 retry
4. lifecycle mutation/focus set 必须来自完整 observed relationship closure 和显式 role classification
   - exact entering、leaving、pending cleanup 或 authority-invalid current 可以按 active plan 成为 authorized target
   - role classification 先确定 authorized target，剩余 preserved、non-target current、app-owned、foreign 与 other-authority non-target closure 保持为 protected，non-target current 只包含 authorized target role 之外的 current Entity
   - authorized target closure 与每个 non-target protected closure 不相交，mutation 只能落在 allowed write set 内
   - 可能触发 hook/observer 的 staging、cleanup 或 recovery 写入后重建并复验 target/protected partition
   - atomic publication tail 不包含会触发 hook/observer 的写入、query 或可失败 validation
5. lifecycle 终态至少由一条真实 producer 经正式 schedule 到 settler 的旅程证明，并覆盖多轮收敛与额外一轮 no-replay

## 阶段计划与验收

History、Recovery 与 Authority closure 的阶段 spec 和 plan 必须先列出角色、拓扑、漂移与终态矩阵，每个分支明确 exact owner、允许写集、receipt/terminal 与 retry 条件

任务完成证据同时包含 focused RED/GREEN、受影响完整 crate、facade/consumer gate 与必要的公开旅程，未运行的门禁只能记录为待完成

阶段完成后同步 current architecture、roadmap、authority matrix 和阶段记录，但只有对应 completion gate 通过后才能标记阶段 PASS

## 本次更新范围

- 在 `AGENTS.md` 和 `docs/implementation-goals.md` 落入上述通用护栏
- 将 `docs/current-architecture.md`、`docs/roadmap.md`、Reconciliation contract 与 plan 更新到 Tasks 1–5 完成、Tasks 6–8 待执行
- 只给 `docs/authority-transition-matrix.md` 补充必要的共通语义或 checkpoint，不改写尚未实现的 Recovery 目标状态
- 保留 History、Recovery、Authority closure 与 MVP 未完成的明确边界

## 验收

- 新规则不重复已有 current/next、稀疏拓扑、typed preflight 与 publication-last 条款
- 当前事实不得声称 Reconciliation PASS 或完整 workspace test PASS
- 文档之间的阶段状态、提交基线与剩余顺序一致
- 独立审查确认规范足够通用、可验证且没有把 Task 5 私有实现固化为公共架构
