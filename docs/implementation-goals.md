# 整体实施目标规划

**状态**：实施总纲 `Plan PASS`；结构治理已完成，Now = Authority closure

**前置条件**：Phase 3B.1 `Plan PASS`

**更新日期**：2026-08-31

## 总体目标

在已冻结的架构、权威边界和公共 API 合同内，完成一个可由 facade 独立装配和驱动的 Bevy UI Router MVP。最终系统必须通过真实 `App::update` 完成导航、UI 构建、原子切换、历史提交、失败恢复与安全清理；core 与 ECS 的状态始终一致，router-owned subtree 的任何生命周期操作都不得破坏 app-owned host 或向 adapter 泄露权威能力。

## 实施主线

1. **Root push**：锁定依赖与 feature 图，建立 facade 合同门禁、插件和 host admission、单 frame 内容构建、registry/ownership/preflight、原子提交、清理与诊断闭环。
2. **Nested push**：引入真实 parent outlet 和逐层 mount，使嵌套路由沿同一身份、generation 与 authority 协议运行。
3. **Reconciliation**：实现 common-prefix 保留和 leaf 替换，验证离场输入/focus 隔离、精确清理及稳定 identity。
4. **History**：完成 `replace`、`back` 与执行期重检，确保 presentation 成功提交后才更新 history。
5. **Recovery**：分别闭合三类终态：route-level match/view/mount 失败可提交 error presentation 或保留旧 active UI；host/authority loss 不渲染 fallback，并在安全 settlement 后清空失效 active；stale result 只丢弃或清理旧输出，不改变当前 request、presentation 与 history。
6. **结构治理**：在不改变行为与公共 API 的前提下拆分巨型测试和稳定协议模块，降低 Authority 扩张前的审查与状态聚合风险。
7. **Authority closure**：补齐多 router、quarantine、后台 reaper、真实 input probe 和攻击性回归，关闭绕过 facade 或伪造生命周期身份的路径。
8. **MVP gate**：统一运行 facade journeys、authority audit、ECS 私有回归、feature/consumer compile gates 与 workspace 验证，并进行最终事实性审查。

当前事实：Root push、Nested push、Reconciliation、History、Recovery 与结构治理已完成。结构治理范围 `211874a..2f2cb42` 保持行为与公共 API 不变，并通过 workspace 1222/1222、doctest 28/28、authority 27/27/27 与独立复审。当前直接执行 Authority closure；`RouterInputProbes`、actual old-lease reap、完整 Journey G 与 MVP gate 仍未完成，因此当前结论不是 MVP PASS。

## 执行与质量门槛

- Authority closure 直接依据 Target、Authority Matrix、Facade Fixture、Allowlist/Golden 与 MVP Journeys 执行；除非用户明确要求，不新建阶段 spec/plan，也不把测试编写顺序当作流程目标。
- 变更按可审查 checkpoint 推进；每个 checkpoint 映射其权威 owner、允许写集、receipt/terminal、retry 条件和最高层 public/consumer gate，并保持可独立验证、提交与回退。
- 失败矩阵必须分列 cleanup target/obligation 与 cleanup 后 continuation，后者由原始 failure/authority classification 与 attempt/branch role 预先封存，不能从 active 状态、obligation 数量或物理 disposition 反推
- lifecycle/authority 终态至少由一条真实 producer 经正式 system schedule 到 settler 的旅程证明，并覆盖多轮收敛与额外一轮 no-replay
- 注入 fact、直接 finalizer 与私有分类测试只能补充该证据
- `Task PASS` 前除 focused RED/GREEN 外，还必须通过受影响完整 crate 与覆盖该语义的最高层 public journey、facade 或 consumer gate
- 未运行门禁只能记录为待完成，不能据此标记通过
- 实施按垂直切片推进，不先堆积未接入真实运行链路的横向协议；每片都从公开 facade 命令贯通到稳定 snapshot 或明确失败终态。
- 并行工作只用于边界清晰、写入范围互不冲突的任务；共享生命周期、公共 API 与 authority 变更由负责人串行整合。
- 所有冻结合同与项目硬约束都必须满足；评估审查结论时，将阻塞项归并到所有权安全、生命周期唯一语义、facade 能力泄露或 MVP journey 不可证明四类共同根因，版本细节和局部实现优化进入对应任务计划。
- `Task PASS`、`Journey PASS` 与 `MVP PASS` 分开记录。只有全部验收旅程、负向回归和事实性门禁均有实际成功输出，才能宣布 MVP 完成。
- 阶段完成后同步 current architecture、roadmap、authority matrix 与阶段记录
- 只有对应 completion gate 通过后才能标记该阶段 `PASS`

## 范围控制

范围与后续顺序统一以 [Roadmap](roadmap.md) 的 `Now`、`Next` 和 `Paused` 为准。所有暂停项都不得成为 MVP 的前置假设；恢复任何暂停能力前，必须重新进入独立设计与审查流程。
