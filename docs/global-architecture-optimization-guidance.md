# Bevy UI Router 全局架构与工程优化指导

> 状态：三路独立复审通过的定稿
> 日期：2026-07-20
> 稳定分析基线：`c23d9de`（`docs: strengthen lifecycle regression evidence`）
> 用途：指导后续优化、阶段取舍、验证和资源投入；不是当前实现规格，也不授权直接修改代码。

## 1. 执行摘要

项目的宏观方向成立：继续保留 Core / View / ECS / Adapter / Facade 的分层、Router-owned subtree 边界、线性 lifecycle capability、原子 publication 和 fail-closed authority 模型。当前最有价值的动作不是重写，也不是继续扩展功能，而是先让 Reconciliation 的终态协议闭合、状态记录可信，并降低单文件和测试工具链造成的变更放大。

推荐关键路径：

1. 完成 Task 7 Step 0 的 Root fallback 前置合同，并在同一步闭合 fallback-specific typed outcome、post-report continuation 和 capability terminal；
2. 完成 Task 7 Steps 1–3 的 Journey B、fragment、audit/facade public evidence；把永久矛盾、receipt/settlement 和 replay 修正推广到全部受影响路径；
3. 严格按既有计划通过 Task 8 Reconciliation completion gate；
4. Task 8 后另立工程治理 checkpoint：建立可证明 verified tree 的机器状态源，并按收益决定是否先做纯机械模块/测试拆分；状态语义重构不默认阻塞 History；
5. 依次独立验收 History、Recovery、Authority closure 和 MVP gate；
6. MVP 后再通过 sealed content primitive / closed plan lowering 接入代码侧 `bsn!` 宏 adapter。

History 的规格、矩阵和测试设计可以只读并行推进，但 History runtime 不应与仍在变化的 Reconciliation runtime 同时落地。完整 Authority closure 也不应整体前移到 History 之前；前台终态闭合是阻塞项，后台 quarantine reaper 和完整 lease 治理可以按既有路线后置。

## 2. 文档边界与证据口径

### 2.1 本文覆盖

- 目标架构、authority/lifecycle 协议、状态与模块边界；
- Reconciliation → History → Recovery → Authority closure → MVP 的阶段顺序；
- 测试、CI、Git、状态记录和依赖治理；
- Bevy fork 策略；
- `bevy_ui_router_bsn` 对代码侧 `bsn!` 宏的 adapter 边界；
- 发布准备和面向使用者的最小文档面。

### 2.2 本文不覆盖

- 不决定具体函数、字段或私有类型的最终命名；
- 不把当前未提交工作树视为已经验证的设计；
- 不要求本轮运行编译、测试、格式化或包发布；
- 不设计基于 `.bsn` 文件的加载、流式更新或热重载；本文中的 BSN 仅指 Rust 代码侧 `bsn!` 宏及其 scene expression 集成；
- 不把某个特定宿主、业务 shell、主题、权限或数据服务引入 Router core。

### 2.3 证据等级

本文使用三个等级，避免把风险推断写成已确认缺陷：

| 等级 | 含义 | 处理方式 |
|---|---|---|
| 已确认 | 文档、稳定提交、类型/分支或测试证据能直接支持 | 可以成为 gate 或明确整改项 |
| 高风险 | 机制存在，但普通 public journey 的生产可达性尚未完整证明 | 先补最短生产路径证据，再决定严重度 |
| 建议 | 工程优化或替代方案，不代表当前实现错误 | 用成本/收益和阶段 gate 决策 |

### 2.4 基线与工作树

- 稳定基线固定为 `c23d9de`，所有“当前架构已经具备”的结论默认只对该提交成立。
- 2026-07-20 07:52（Asia/Shanghai）的只读快照显示，工作树有 7 个已修改文件，约 `+14,494/-10,861`；其中 `crates/bevy_ui_router_ecs/src/navigation.rs` 占绝大部分。该数字会继续变化，只作为 WIP 风险信号，不作为质量结论。
- 后续评审必须同时记录“被验证的代码提交”和“记录状态的文档提交”，不能用后来改写的计划勾选反推早期实现已经通过。

## 3. 长期 Git 趋势及其含义

截至稳定基线，仓库可见的全部历史只覆盖 2026-07-07 到 2026-07-20 这 14 个自然日，共 175 个提交：`docs` 63、`feat` 53、`fix` 32、`test` 20、`chore` 5、`build` 1、其他 1。提交中位数为 2 个文件、约 219 行 churn；14 个提交超过 10 个文件，33 个提交超过 1,000 行 churn。历史中没有 merge commit 和 tag。这个窗口足以定位近期热点，不足以外推项目数月或数年的维护成熟度。

长期 churn 高点包括：

| 文件 | 累计增删行 | 解释 |
|---|---:|---|
| `crates/bevy_ui_router_ecs/src/navigation.rs` | 31,558 | 责任清单与 churn 共同提示协议驱动、调度、结算和测试入口汇聚风险 |
| `crates/bevy_ui_router_ecs/src/cleanup.rs` | 17,297 | 责任清单与 churn 共同提示 cleanup/authority 高变更风险 |
| `crates/bevy_ui_router_core/src/engine.rs` | 10,522 | 近期提交内容与 churn 提示核心状态机仍在收敛 |
| `crates/bevy_ui_router/tests/support/facade_contract_cases.rs` | 6,638 | 公共合同测试共享度高，但编译/维护成本集中 |
| `crates/bevy_ui_router_ecs/src/presentation.rs` | 6,021 | presentation 与 lifecycle 边界是持续热点 |

这些数据支持“需要协议稳定化和模块拆分”，但不能单独证明流程失控：无 merge/tag 不等于没有 review，大提交也可能是必要的机械变更。正确用法是把规模作为加强复审和 gate 的触发器，而不是硬性拒绝条件。

## 4. 不可退让的架构约束

后续任何优化不得通过削弱以下约束换取局部简化：

1. Router 只管理 router-owned subtree，不能重建或清理 app-owned host。
2. Core 不依赖 Bevy ECS、`bsn!`、平台、主题、权限、数据服务或应用 shell。
3. Router、host、slot、frame identity、generation 和 task/request identity 必须可追踪；revision 还必须绑定显式实例身份。
4. Ownership seal、authority ID 和 capability 消费后不得复用；拒绝路径必须返还原 owner。
5. Current 与 next 必须独立冻结和校验；`next=None` 是只参与验证的 no-op，不得产生物理 mutation。
6. Sparse topology 只依赖 exact parent/outlet/slot/mount binding 和 Entity 自身 role，不能从数量、数组位置或关联对象的宽泛 lifecycle 并集推断权限。
7. Router-created Entity 先登记 ownership，再写 component/relationship；任何可能触发 hook/observer 的写入后都要重建并复验 target/protected partition。
8. Publication 前完成全部可失败 preparation；atomic publication tail 只能执行已经准备好的、不可失败的原位 mutation。
9. 永久矛盾有界 fail closed；只有可证明未来可满足的容量或前置事实短缺才能 retry。
10. 延迟 action 消费前必须从当前 authority exact refreeze 完整 source projection；snapshot/fingerprint 只能校验，不能重建 owner。
11. Pending scene 不是 active UI；异步 apply 前必须覆盖 router/frame/generation/task 的 stale check。
12. Hidden/retained subtree 必须隔离 pointer、keyboard、gamepad input 并释放 focus。
13. Adapter 只能通过公开 API 影响 Router，不能伪造身份、generation 或 commit authority。

如果优化建议与上述约束冲突，应修改建议，而不是放宽约束。

## 5. 关键决策总表

| 议题 | 建议决策 | 不采用的近似方案 | 原因 |
|---|---|---|---|
| 总体架构 | 保留现有分层，局部收敛 | 重写 ECS integration | 当前复杂度主要来自真实生命周期约束，重写会重新发现同一组问题 |
| Preflight | 先做 role-sensitive 三类 disposition，再让 retry/fail-closed envelope 持有已分类 continuation | 扁平五类结果，或强行在“三种/四种”措辞中二选一 | retry/contradiction 是 preparation 状态或原因，不是 role-neutral physical disposition |
| Retry | 只允许有可完成性证据的短缺重试 | 所有空值、错误或 budget 内异常重试 | budget 不能把永久矛盾变成暂态 |
| 路线 | Task 7 Step 0 → Task 7 public evidence → Task 8 → 可选机械拆分 → History | 完整 Authority closure 全部前移 | 前台终态阻塞 History；后台 lease/reaper 不阻塞 |
| 状态表达 | 多个正交状态轴 | 一个覆盖所有组合的巨型 enum | branch、host、foreground、background cleanup 生命周期并非同一互斥轴 |
| 模块拆分 | 先外移/归档内联 tests，再按协议机械拆生产代码；状态模型另行决策 | 同一提交边移动边改语义，或把语义重构设成 History 的无条件 blocker | 降低热点成本，同时避免为了结构美观推迟用户价值 |
| Contract harness | 标准工具 + authority 定制检查混合 | 全部自研或全部替换 | 普通 API 和 authority capability 的风险不同 |
| Git gate | 任务/跨 authority checkpoint | 每个小提交都 PR，或只在 MVP 末复审 | 前者摩擦过高，后者反馈过晚 |
| Bevy | fork 为主、patch ledger、定期 upstream lane | 立即切官方 revision | 当前 fork 含 UI clipping/picking 相关补丁，必须做物理行为验证 |
| `bsn!` | MVP 后把审计过的宏子集 lowering 到 sealed `ContentPlan` | 通用 `Scene::apply_scene`，或让 ECS/facade 获得 Router authority | 任意 Scene 本身是 World-mutation capability；宏是 authoring 能力，不是 lifecycle authority |
| 发布 | 未完成 crate 先 `publish = false` | 现在直接发布 0.1.0 | 依赖策略、元数据和用户路径尚未闭合 |

## 6. 推荐协议模型

### 6.1 解决 Preflight“三类/四类”冲突

`AGENTS.md` 要求区分 ready、pending rejection、authority loss 和可重试准备失败；`target-architecture.md` 与 authority matrix 则把最终 physical disposition 固定为三类。二者并不矛盾：先按 Entity role 得到三类 intent，再把 preparation 结果表达为 prepared、retryable 或 fail-closed。Protocol contradiction 是 evidence/cause，不是 role-neutral 的第四种 physical disposition。

```rust
// 伪代码：强调线性所有权和类型分层，不固定最终字段名。
enum ClassifiedIntent<O> {
    Ready(ReadyIntent<O>),
    PendingRejected(PendingSettlementIntent<O>),
    AuthorityLost(AuthoritySettlementIntent<O>),
}

enum PreparationResult<O> {
    Prepared(PreparedDisposition<O>),
    Retryable(RetryState<ClassifiedIntent<O>>),
    FailClosed(FailClosedState<FailClosedContinuation<O>>),
}

enum PreparedDisposition<O> {
    Ready(ReadyPlan<O>),
    PendingRejected(PendingSettlement<O>),
    AuthorityLost(AuthoritySettlement<O>),
}

enum FailClosedContinuation<O> {
    PendingRejected(PendingSettlementIntent<O>),
    AuthorityLost(AuthoritySettlementIntent<O>),
}
```

`ReadyPlan<O>`、settlement、`RetryState<_>` 和 `FailClosedState<_>` 各自直接消费唯一 owner；不能把 `owner` 与一个可能已经持有 owner/capability 的 plan 或 residual 并列。每个 envelope 都冻结完整 source projection、exact expectation、observed state、关联 obligation、attempt identity 和 role classification：

- `RetryState` 额外持有 shortage、会改变该事实的 producer 和下一次 exact refreeze 条件；
- `FailClosedState` 额外持有 contradiction evidence；形成它之前必须用最新 exact evidence 重新做 role classification，并把 pending-cleanup 或 authority-settlement continuation 原样带到终态，不能包住一个仍声称 `Ready` 的 intent；
- foreign/stale task 若在 admission/role classification 前即不属于当前 scope，走独立的 no-write owner return，不伪装成 lifecycle disposition。

### 6.2 Role-sensitive 分类规则

| 观察事实 | role/disposition | preparation 结果 | 必须留下的证据 |
|---|---|---|---|
| 全部 exact authority 与 preparation 完成 | `Ready` | `Prepared(PreparedDisposition::Ready)` | sealed mutation plan、owner、完整 source projection |
| contradiction 仅位于 pending/entering target | `PendingRejected` | prepared settlement 或包住该 continuation 的 `FailClosed` | pending settlement intent、exact drift、原 owner |
| host/current/preserved/leaving authority 丢失 | `AuthorityLost` | prepared settlement 或包住该 continuation 的 `FailClosed` | authority settlement intent、exact drift、原 owner |
| leaving target 含可隔离 foreign descendant/component | `Ready` | prepared commit + quarantine reservation | target/protected partition、lease obligation |
| preserved non-target 含 foreign descendant | `Ready`，该 closure 零 mutation | prepared commit | protected closure evidence |
| 已分类 intent 遇到可证明未来可满足的短缺 | 保留原 disposition | `Retryable` | 完整 intent/owner、shortage、producer、attempt identity |
| task 在 admission 前已 foreign/stale | 不进入 lifecycle preflight | no-write owner return | source/expectation/observed identity |

“协议矛盾”不能把上述 role 差异抹平。私有 `RouteEntityRegistry` **resource** 整体缺失意味着当前 authority 基础设施丢失：先保留 `AuthorityLost` continuation，再因 settlement preparation 不可完成而进入 owner-bearing fail-closed residual。单个 registry record/manifest/binding 的缺失则仍按该 Entity 自身 pending、entering、current、preserved 或 leaving role 分类。

### 6.3 Retry、duplicate 与 replay

Retry budget 只是对真实暂态的安全熔断器，不是分类器。建议每个 retry 分支都能回答：

- 缺少的事实是什么；
- 哪个在途 producer 会改变该事实；
- 下一次观察的 identity 是否仍绑定同一 attempt；
- budget 耗尽后 owner/capability 如何进入唯一终态。

无法回答其中任一项时，不应重试。永久 drift 在第一次分类时就进入保留原 continuation 的有界 fail-closed residual；不能先反复 requeue，再用 budget 偶然终结。

Duplicate/replay 不属于成功 terminal receipt，也不是 preflight classification：

- 完全相同的重复 receipt 返回 `DuplicateRejected`；
- wrong batch/epoch/request/attempt 返回 `StaleDiscarded` 或 `InvalidRejected`；
- completion replay 无副作用；
- 三者都不能再次改变 ledger、request、presentation/history，不能重新取得或制造 owner，也不能触发 requeue。

### 6.4 终态与 capability 收敛

需要把以下链路作为一个协议闭环验证，而不是分别检查 helper 返回值：

```text
freeze exact source
  → classify with typed owner-bearing evidence
  → prepare target/protected partition and cleanup reservation
  → publish or settle exactly once
  → consume/transfer lifecycle capability
  → record terminal result
  → prove no outstanding action/ticket/owner remains for the same scope
```

重点修正方向：

- receipt 不能在边界处压缩成 bool；错误种类必须保留到 settlement；
- mount rejection 不能丢弃 kind 后统一走 retry；
- completion 不能只看 `.is_ok()` 决定是否 requeue；
- continuation 必须来自冻结的 failure/authority/attempt/branch classification，不能由 preserved active 或 obligation 数量反推；
- test-only fault seam 只能改变真实 predicate 的输入，不能直接注入 terminal classification。

稳定基线存在“错误 completion 可能再次排队”的机制证据，也能用 test-support 组合构造多 settle tick 风险场景；但当前仓库尚没有可引用的 focused cross-router regression，普通 facade/hostile observer 的生产可达性也未完整证明。应按“已确认协议/活性风险、测试可构造”整改，不应在没有 public journey 证据时宣称为已确认用户可见死循环。

### 6.5 Publication 边界

建议把 commit 路径稳定为三个阶段：

1. **Freeze/Classify（只读）**：重建完整 observed relationship closure、source projection、role 和 exact binding；
2. **Prepare（仍未 publication）**：分配、排序、组合 owner、准备 sealed claim-rebind plan、cleanup reservation，并在任何 hook/observer-triggering staging 后重建 partition；
3. **Atomic tail**：只执行已经准备好的不可失败原位 mutation，不 query、不结构写入、不安装 owner、不做可失败 validation。

“在 publication 后补 owner/registry”或“提交后从 World 结构反推 current”都必须视为协议破坏，而不是实现便利。

## 7. 状态与模块优化

### 7.1 不使用一个巨型生命周期 enum

建议至少保留四个正交轴：

```text
HostState
  ├─ AwaitingHost
  ├─ Ready
  ├─ SettlingLoss
  └─ RecoveringAuthority

ForegroundState
  ├─ Idle
  ├─ Resolving / Applying
  ├─ Mounting / Committing
  ├─ SettlingFailure
  └─ ForegroundCleanup

Option<RealizedBranch>
  └─ 当前已提交 presentation/history 对应的 branch

BackgroundCleanupState
  └─ quarantine leases、reaper observation、非阻塞资源治理
```

具体 variant 可以调整，但所有权规则应稳定：`ForegroundState` 线性持有 pending task、mount/commit capability、failed branch 或 foreground cleanup continuation；`RealizedBranch` 不因为 foreground attempt 存在就消失；background quarantine 不得重写历史 request 或阻塞已经取得 terminal receipt 的前台请求。

### 7.2 模块按协议拆分

Task 8 后设置一次 go/no-go 评估，而不是把全部重构设为 History 的硬前置。稳定基线的 `navigation.rs` 约 19,962 行，其中内联 `tests` 模块约占 77%；只拆生产函数不会显著降低热点、冲突或复审成本。推荐顺序：

1. 先把内联 tests 纯机械外移或按协议归档，保持 exact test name/path/filter 和 cfg 行为不变；
2. 再做生产代码的纯机械移动，保持 public API、类型、schedule 和语义不变；
3. 每次只沿一个协议边界拆分，建议目标模块为：
   - `drive`：命令/任务推进与只读观察；
   - `commit`：preflight、prepared plan 与 atomic publication；
   - `failure_settlement`：失败证据、cleanup manifest 与 continuation；
   - `cleanup_attempt`：前台 obligation/receipt；
   - `quarantine`：lease、后台 reaper 与 disposition；
   - `history_flow`：History intent、commit coupling 与 navigation policy（History 设计冻结后再创建）。
4. 每一步通过原有最高层 gate 后再继续；
5. 状态轴重构独立决策：只有现有状态表达已经阻塞正确性或让 History 无法安全落地时才前置，否则等 History/Recovery role 冻结后再实施。

不要以行数均衡为目标。模块的价值是让 owner、capability 和 terminal evidence 的进入/退出边界可以被审查。

### 7.3 拆分完成标准

- `navigation.rs` 不再承载绝大多数内联 tests，也不再同时拥有驱动、commit、failure settlement、quarantine 和 History 的完整实现；
- 外移测试的名称、cfg、exact filter 与 production predicate 覆盖保持不变；
- 任何 capability 的签发、移动和消费可在一个协议模块或一条显式调用链内追踪；
- 机械拆分 diff 中不混入 variant 重命名、错误分类变化或公共 API 变化；
- 状态重构 diff 能用既有 gate 证明 observable contract 不变，或明确记录有意变化。

## 8. 分阶段优化路线

### 阶段 A：Task 7 Step 0 — Root fallback 与必要协议闭合

**进入条件**

- 以 `c23d9de` 和当前 reconciliation 计划为基线；
- 将当前 7 文件 WIP 视为未验证实现，不在其上继续叠加 History/Recovery；
- 优先从稳定基线或独立只读 worktree 记录两条 public fallback journey 的真实 RED/现状证据；不得为重现 RED 丢弃当前用户 WIP。

**动作**

- 仅实现既有 active UI 下 projectionless Root fallback replacement 所需的最小路径；
- admission 绑定 exact current `RouterFallback` task、registered top slot、host/slot identity；
- publication 前完成 host、registry、ownership、relationship、input/focus、manifest 和 cleanup ledger preparation；
- 在本阶段内完成 fallback-specific role-sensitive intent、typed retry/fail-closed envelope、post-report continuation 和 owner/capability single-consumption；不得推迟到后续阶段；
- 永久 pending/entering drift 保留 `PendingRejected` continuation，current/host/leaving drift 保留 `AuthorityLost` continuation；只重试真实 preparation shortage；
- 不扩展 nested fallback 或完整 Recovery。

**退出 gate**

- 公开 fallback 成功/失败 journey 是最高层合同；
- 正常 Build/Apply failure 保留旧 active presentation，fallback 成功时原子替换；
- mismatch、observer drift 和 cleanup 失败不会越权写入或 replay；
- capability/owner 在每种终态唯一可追踪。

### 阶段 B：Task 7 Steps 1–3 — Public evidence 与全路径收敛

**进入条件**：阶段 A 的 public fallback contract 已恢复；没有同时进行 History runtime 变更。

**动作**

- 增加并跑通 Journey B、fragment、authority audit/facade public evidence；不得用已有 target 的零新增断言退出 Task 7；
- 把阶段 A 已验证的 6.1 模型推广到全部受影响 reconciliation/settlement 路径，消除 null/bool/generic retry 混用；
- 修正 receipt kind、mount rejection、failed settlement 与 completion requeue 链路；
- 对无 producer 的 registry/binding/manifest contradiction 保留原 role disposition 和 continuation，不使用 role-neutral retry；
- 为 genuine shortage 标记明确 producer 和 retry identity；
- terminal 后审计 outstanding action/ticket/capability/lifecycle owner 为零。

**退出 gate**

- 覆盖 ready、pending rejected、authority lost、retryable shortage、role-sensitive fail-closed；
- duplicate receipt、stale/invalid replay 与 completion replay 分别得到规范拒绝且零副作用；
- 同一错误在至少两个 settle tick 不会形成固定点；
- test-support 风险场景只能改变真实 predicate 输入，并应补成可引用的 focused regression；
- focused、受影响 crate、最高层 public journey/facade/consumer gate 全部有成功输出。

### 阶段 C：Task 8 Reconciliation completion gate

**动作**

- 原样执行既有 Task 8：focused Core/View/ECS、public journeys、authority audit、facade allowlist/compile-fail 与 authority verifier；
- 执行 full workspace `fmt`、`check`、`nextest`、doctest、`git diff --check` 和 `git status` gate；
- 完成四个独立只读 review；Critical/Important 清零后重跑受影响 gate；
- 更新既有 factual records，再做一次最终事实文档复审；
- 只在全部命令有成功输出后同步 current architecture、roadmap、review log、profile 和 plan，并标记 `Complete`。

**退出 gate**：Reconciliation 有一个可复现的 verified code commit/tree 和既有 gate profile；History、Recovery、Authority closure、MVP 仍明确未完成。新的机器状态源是合理后续优化，但除非先正式修订 Task 8 plan/profile，不得静默成为本阶段 blocker。

### 阶段 D：工程治理与可选机械拆分 checkpoint

**必做动作**：建立第 11 节的机器状态 provenance 与本地 validator，再考虑接入 CI。

**go/no-go 决策**：如果 `navigation.rs` 热点、共享文件冲突或 owner 调用链仍显著拖慢 History 评审，则按 7.2 先外移 tests、再机械抽取生产模块；否则直接进入 History，并把拆分排为并行禁止、后续独立 checkpoint。状态轴语义重构不默认在此执行。

**退出 gate**：状态记录可以证明 exact verified tree；若执行拆分，原有 Reconciliation public/consumer contract 必须完全不变。

### 阶段 E：History

**允许提前并行的工作**：只读规格、transition matrix、push/replace/back 测试设计、history intent 与 publication coupling 评审。

**runtime 进入条件**

- Task 7 Steps 0–3、终态收敛和 Task 8 均通过；
- 没有会重写历史 request 的后台 cleanup 语义；
- 已定义 no-op、stale、failure、fallback 和 post-commit cleanup 对 history 的影响。

**退出 gate**

- history intent 只在对应 publication 成功时提交；
- stale/failed/pending scene 不推进 history；
- `replace`、`back` 与当前 branch/generation exact 绑定；
- foreground terminal 后，后台 quarantine 不改写历史记录。

### 阶段 F：Recovery

**动作**：在 History 语义稳定后实现 error presentation、向上恢复、authority recovery 和明确失败通道。

**退出 gate**

- normal failure、fallback、error presentation、recovery success/failure 有互斥终态；
- cleanup target manifest 与 settlement continuation 独立冻结；
- 旧 presentation/history 的保留或替换由原始分类决定，不由物理 disposition 反推；
- app-owned host 始终 protected。

### 阶段 G：Authority closure

**动作**

- multi-router stress、完整 opaque audit、lease authority、background reaper、cleanup replay；
- 将 public input isolation 的保留 target 从 `expected_tests = 0` 提升为非零真实 journey；
- 完成 authority-specific public/private gates 和独立 factual review。

**退出 gate**：Authority closure 独立标记 PASS，并固定 exact verified tree；不能由后续 MVP PASS 倒推或替代。

### 阶段 H：MVP gate

**动作**

- 在 Authority closure 的同一 tree 上独立重跑 facade journeys、authority regressions、feature matrix、workspace tests、物理 UI 与 consumer gates；
- 关闭发布前公共 API/依赖/文档缺口；
- `public_navigation_input_isolation` 必须通过 `--list`/case count 证明非零，不能只依赖 Cargo target 退出 0。

**退出 gate**：MVP 的公共 journey、authority、feature、物理 UI 和 consumer evidence 同时绑定同一 verified tree。

### 阶段 I：MVP 后受限 `bsn!` adapter

`bsn!` 宏本身已是代码驱动 scene authoring 能力，后置不是因为宏或文件加载器不成熟，而是因为任意 Scene 表达的是广泛 World mutation；应在 Router lifecycle substrate 和 sealed content contract 稳定后，只接入可 lowering 到已审计 primitive/relationship 的子集，避免 adapter 需求反向塑造 core authority。

### 8.1 并行工作边界

- Task 8 前可并行：History、Recovery、Authority 的只读矩阵、测试设计和风险审查；
- Task 8 后，History runtime 串行推进时可并行准备 Recovery tests/spec、Authority audit/stress/reaper design 和 MVP inventory；
- 不并行：共享 lifecycle/authority 状态实现、同文件机械拆分与 History 实现、Recovery runtime 与 reaper/lease runtime、最终 MVP gate；
- 并行设计产物不能提前改变 runtime 或成为尚未批准阶段的隐式 blocker。

## 9. `bsn!` 宏集成边界

### 9.1 推荐职责

`bevy_ui_router_bsn` 应：

- 实现既有 data-only `EcsViewDriver` 合同，只接收 `ViewBuildEffect`、`FrameContentWriter` 和单次 report session；
- 在不调用通用 World scene mutation API 的前提下，把经过批准的代码侧 `bsn!` authoring 子集 lowering 为现有 sealed `ContentPlan` primitive/relationship；
- driver 返回前保持 World 零变化，失败时丢弃 session-local plan；
- 复用当前唯一已审计的 `SolidRect` 等 sealed primitive；文本、图片或其他 primitive 必须逐个完成 manifest、hook/observer 和 hostile regression 审查；
- 与其他 view adapter 共存，而不是成为唯一 renderer。

`bevy_ui_router_bsn` 不应：

- 把任意 `Scene`、`ResolvedScene`、bundle/template、World closure 或通用 `apply_scene` 当作受信 plan；
- 获得 `&mut World`、裸 Entity registry、generation 分配器、frame identity 构造器或 commit capability；
- 创建/替换 app-owned host；
- 直接把 scene 设为 active、推进 history 或决定 cleanup continuation；
- 伪造 frame/generation/task identity；
- 首版不得携带 observer/callback/hook；未来若需要局部 observer，必须先形成新的 sealed data-only variant 并单独审查，且永远不能成为 route lifecycle 权威源。

当前 `bevy_ui_router_bsn` 只有空 prelude，manifest 也没有 `bevy_scene` 依赖，因此本文描述的是未来设计约束，不是对现成 adapter 成熟度的声明。早期文档曾把即时 BSN 放入 MVP；当前权威 `target-architecture.md`、roadmap 和 supersession 说明已将其调整到 post-MVP，本文有意遵循后者。

### 9.2 建议握手

```text
code-side bsn! authoring source
  → 经批准的、无 World effect 的 lowering（若可证明可行）
  → EcsViewDriver 仅通过 FrameContentWriter 构造 sealed ContentPlan
  → driver terminal report；失败直接丢弃 plan，World 零变化
  → ECS 验证 plan graph/manifest
  → 为全部 handle 分配无 component Entity
  → registry-first staging-register ownership
  → 写 hidden/input barrier 与逐项审计 component
  → 写 relationship，并在每个可重入阶段后 exact revalidate
  → authority preflight / atomic publication
```

Pinned Bevy 的通用 `Scene::apply` 可以写任意 template/bundle、创建 related Entity、安装 observer/relationship；post-write validation 无法撤销已经触发的 app-owned side effect，也无法补救“新 Entity 未 registry-first”这一事实。因此不能采用“先 apply，再 refreeze”的握手。

在实现前先做一个有界 feasibility spike：证明宏 authoring 能在 **不消费 opaque arbitrary `Scene` world-mutation capability** 的情况下确定性地产生 sealed plan。若当前 Bevy 公共 API 无法安全检查/降低其私有 erased templates，则应等待合适的上游 IR/inspection boundary，或提供明确受限的 router-side authoring frontend；不能退回通用 `apply_scene`。

### 9.3 可取舍方案

1. **推荐：approved `bsn!` subset → 现有 sealed `ContentPlan`**。authority surface 最小，也直接复用已审计 realization。
2. **备选：私有 closed `BsnPlan` interpreter**。仅当无法直接复用 `ContentPlan` 且确有必要时使用；variant 集合必须闭合，每个 component/relationship 单独审计，首版禁 observer，最终仍由 ECS registry-first realize。
3. **条件性替代：trusted app-owned integration**。若目标是完整任意 BSN 语义，它不能伪装成 Router-managed view；可由应用在 Router authority 外自行管理，并承担所有 lifecycle/cleanup 责任。
4. **不采用：通用 `Scene::apply_scene` 或 ECS/facade 自行 commit**。二者都会绕过 data-only driver、registry-first ownership 和 atomic publication。

现有 `ContentPlan` 已经是安全中间层；不需要为了 BSN 先抽象一个通用 RenderPlan。只有第二个真实 renderer 证明有当前 plan 无法表达且仍可封闭的共同语义时，才评估更高层抽象。

## 10. 测试与验证体系优化

### 10.1 测试分层

| 层级 | 目的 | 触发时机 |
|---|---|---|
| Focused predicate/state test | 精确证明分类、owner 移动和边界条件 | 每次语义改动 |
| Crate gate | 防止模块内组合回归 | 每个任务 checkpoint |
| Authority verifier | feature/capability/opaque surface 硬约束 | authority 或 facade 改动 |
| Public journey/facade consumer | 证明用户可见合同和生产路径可达 | lifecycle/failure/terminal 改动 |
| Workspace/milestone gate | 跨 crate 与依赖闭合 | 阶段完成、tag、发布候选 |
| Physical UI lane | clipping、picking、input、focus 行为 | Bevy revision 变化及 MVP milestone |

局部分类测试不能重写既有 public contract；涉及 lifecycle、failure 或 terminal 分类时，必须重跑已闭合的最高层 public journey/facade/consumer gate。

### 10.2 Harness 降本

短期：

- 先记录每个 feature profile 的 Cargo 启动次数、冷/热耗时和重复 helper 编译成本，再按最大收益点优化；
- 把可共享的正例 mutation/fixture cases 收敛为一次编译、参数化执行；同一 feature profile 内只生成一次 lock/tree/prebuild，并复用结构化结果；
- compile-fail 负例仍逐 case 隔离并断言预期 rustc error category/fragment，避免多个错误互相遮蔽；可以共享预生成 lock/tree，但不能用一次混合失败代替逐例证明；
- 将 consumer fixture 组织成少数 workspace/profile，避免每个 assertion 重复启动相同 Cargo 图；
- 为每个脚本输出 gate 名、verified tree、feature profile、case count 和明确退出码。

中长期：

- 普通 public API snapshot 可评估 `cargo-public-api` 或 rustdoc JSON，输出可审查 diff；
- release diff 可评估 `cargo-semver-checks`；
- 保留自定义 feature graph closed-world、弱 feature non-reachability、authority capability leak、opaque session constructibility、manifest/source policy 检查；
- authority-bearing API 继续是 hard gate，不能降为仅供人工查看的 golden diff。

### 10.3 `autotests = false` 下的测试清单

未注册文件不自动等于覆盖丢失。对 `facade_features.rs`、`facade_compile_fail_boundary.rs`、`root_navigation_surface.rs`，先建立“旧 case → 当前固定 target / isolated consumer gate”的 1:1 映射：

- 映射完整：显式标记 superseded，并删除或归档旧文件，避免误导；
- 映射不完整：把缺失 case 加入已有固定 target，除非它确实需要独立 feature profile；
- 不要仅为消除未注册告警而盲目增加 `[[test]]`，那会放大重复编译成本。

`public_navigation_input_isolation.rs` 当前是一行保留 target。现阶段应在测试清单中显式标记 `reserved` / `expected_tests = 0`；进入 Authority closure/MVP gate 时必须转换为非零测试，并用 `--list` 或机器 case count 让 `0 tests` 明确失败，不能只看 Cargo target 的退出码。

### 10.4 本轮未执行的验证

本文审查遵循只读、尽量不编译的约束，本轮没有运行 Cargo、测试、格式化、静态检查、package dry run 或物理 UI 验证。文中的 gate 是后续执行要求，不是“已经通过”的声明。

## 11. 状态、CI 与 Git 治理

### 11.1 单一机器状态源

建议在 Task 8 按既有范围完成后新增一个最小 `status.toml` 或 `status.json`；如果希望把它提前成为 Task 8 blocker，必须先正式修订 plan/profile。Prose roadmap 只引用或由其校验。每个 gate 至少记录：

```toml
state = "planned" # planned | active | blocked | passed | regressed
plan_commit = "<commit>"
verified_code_commit = "<commit-or-empty>"
verified_tree_oid = "<git-tree-oid-or-empty>"
verified_worktree = "clean" # passed 必须为 clean；dirty 只能作为候选证据
lockfile_digest = "<digest>"
toolchain = "<rustc/cargo/toolchain identity>"
gate_profile = "<versioned-profile>"
result_artifact = "<immutable-artifact-id-or-path>"
result_digest = "<digest>"
record_commit = "<evidence-record-commit-or-empty>"
invalidated_by = "<commit-or-empty>"
```

规则：

- `passed` 必须绑定实际运行 gate 的 exact clean code commit/tree、lockfile、toolchain、profile 和不可变 result artifact；dirty worktree 的运行只能作为诊断，不能归因到 `HEAD` 并标记 PASS；
- 后续回归改为 `regressed` 并填写 `invalidated_by`，不抹掉历史 PASS；
- `record_commit` 指向先落地的不可变 factual evidence record，再由后续 status 更新引用；或由 CI 作为外部 provenance 派生，不能要求同一 blob 自引用尚未知的 commit hash；
- Task 6/7/8、roadmap、profile 的 prose checkbox 不再各自成为权威状态源；
- 先实现只读本地 validator，校验 schema、tree/commit 对应、clean-tree 要求、artifact digest、提交可达性、PASS 所需字段和 prose/status 差异；仓库建立 CI 后再复用同一 validator，避免先造两套规则。

### 11.2 CI 分层

快速 CI（普通 checkpoint）：

- status schema 与文档差异检查；
- `cargo fmt --check`、受影响 crate `cargo check`；
- feature/authority 的快速 verifier；
- 根据 diff 选择 focused/public gate。

共享本地 workspace 中，同一时刻只指定一名验证负责人运行 Cargo/rustc；其他代理做静态审查或等待。CI 若使用隔离 checkout/target directory，则可按 runner 隔离策略并行。

里程碑 CI：

- full locked workspace/nextest/doctest；
- 全部 public journeys、facade consumer 与 authority audit；
- feature matrix；
- 涉及 Bevy revision 时的物理 UI clipping/picking/input/focus lane；
- package/publish dry run（仅发布候选）。

### 11.3 Commit 与 review 边界

- 以任务或跨 authority checkpoint 为 review 单位，不要求每个微小提交都建 PR；
- 跨 owner、超过 10 个文件或大型非机械变更时，commit body 写明 Why / preserved invariants / gates；
- 规模阈值只触发额外复审，不自动拒绝；
- 机械移动与语义修改分开提交；
- reconciliation、History、Recovery、Authority closure、MVP 完成时创建 annotated milestone tag；
- 共享工作区严格指定唯一 index/commit 负责人，只 `git add` 精确文件列表，提交前检查 staged set。

对当前大型 WIP，集成前优先按“fallback contract”“terminal classification”“测试证据”“机械移动”拆成可独立说明的 checkpoint；不要为了得到小 diff 而重写或丢弃现有用户工作。

## 12. Bevy fork 与兼容性策略

当前 pinned revision `61f78f7` 相对官方 release 后含 4 个提交，涉及 text measurement、recursive clip check、Parley 0.10 和 text scroll clip clamp；这些变化触及 UI clipping/picking 的物理行为。因此：

1. 当前 fork 继续作为主线，不盲目改回官方 revision；
2. 建立 patch ledger：commit、上游链接/状态、依赖理由、受影响 gate、移除条件；
3. 每个里程碑或固定周期运行 upstream compatibility lane；
4. compatibility lane 使用独立临时 worktree 加临时 manifest/config overlay 指向候选 upstream ref，记录 overlay 与 lock diff；不手工改写主工作树 manifests；
5. compatibility lane 分为 compile/API 和 physical UI 两部分；仅 `cargo check` 通过不足以晋升；
6. 只有 ledger 中全部必要补丁已上游合并、替代或被证明不再需要，且 physical gates 通过，才提升官方 revision。

这一路线保留近期确定性，也避免 fork 永久漂移。Fork 不是默认永久策略，而是有退出条件的工程资产。

## 13. 发布与使用者体验

### 13.1 短期

- 未准备发布的 crate 显式 `publish = false`，避免误发布；
- 根 README 提供最小概念图、支持范围、非目标、MVP 状态和一个可复制的 route/view 示例；
- 示例优先展示公共 facade，不暴露内部 identity/capability；
- 明确哪些 crate 是实现细节、哪些是用户入口。

### 13.2 发布候选条件

- README、LICENSE、repository/documentation/keywords/categories 元数据完整；
- path + version 策略和 crate 发布顺序明确；
- 官方 crates.io 能获取或可接受的 Bevy 依赖策略已确定；
- package dry run、semver/public API diff、最小 consumer、全 gate 成功；
- MVP 状态记录绑定同一 verified tree。

这些不是 Task 7 的 blocker，但应在 Authority closure 后半段开始准备，避免 MVP gate 通过后才发现包无法发布或用户无从上手。

## 14. 资源投入与停止规则

### 14.1 优先投入

1. Task 7 Step 0 的 Root fallback、终态/capability 可证明性；
2. Task 7 Steps 1–3 的 public journey/audit/facade evidence；
3. Task 8 既有 gate 与事实记录；
4. exact-tree 状态 provenance 和 gate 可复现性；
5. 经 go/no-go 证明有收益的 tests/`navigation.rs` 机械拆分，以及 harness 启动次数降本；
6. History/Recovery 新能力；
7. Authority closure、MVP、`bsn!` adapter 和发布面。

### 14.2 暂停或拒绝条件

- 永久矛盾仍通过通用 retry 分支循环时，暂停新 lifecycle feature；
- Task 8 没有 verified commit/profile 时，不落地 History runtime；
- 机械拆分同时改变 observable semantics 时，拆开后再评审；
- Adapter 需要裸 registry、identity 构造、commit capability 或通用 `Scene::apply` 时，停止 Router-managed 集成并退回 sealed plan 设计；
- Bevy 升级只通过编译、未通过 physical gate 时，不晋升；
- public input isolation target 到 Authority closure gate 仍为零 case 时，不宣称 Authority closure；
- 状态 prose 与机器记录冲突时，不能宣称阶段 PASS。

### 14.3 建议度量

使用能反映风险闭合的指标，而不是以总代码行数为目标：

- 每个 terminal scope 的 outstanding capability/owner 数量是否归零；
- permanent contradiction 从观察到终结所需 settle tick（目标：一次分类、有界结算）；
- public journey 覆盖的 terminal classification 比例；
- 每个 feature profile 的 Cargo 启动/编译次数；
- 最大热点文件和单个语义 diff 的 churn 趋势；
- `passed` gate 可复现率和从 `regressed` 到重新验证的周期；
- fork patch ledger 中未上游补丁数量及其 physical gate 覆盖。

## 15. 下一批可执行清单

### Checkpoint 1：Reconciliation 阻塞项

- [ ] 固定并记录当前 WIP 的责任边界，不叠加 History/Recovery；
- [ ] 恢复两条 Root fallback public journey；
- [ ] 在 Task 7 Step 0 内落实 role-sensitive、owner-bearing preparation/result envelope；
- [ ] 永久 registry/binding/manifest contradiction 不再 retry；
- [ ] receipt/settlement/completion 保留 typed evidence；
- [ ] terminal audit 证明同一 scope 无 outstanding capability；
- [ ] 完成 Task 7 Steps 1–3 的 Journey B、fragment、audit/facade 新增证据；
- [ ] 运行并记录 focused → crate → authority → public consumer gate。

### Checkpoint 2：稳定化

- [ ] 严格完成 Task 8 full workspace gate、四路 review 与最终 factual docs re-review；
- [ ] Task 8 后建立 exact-tree 状态源和本地 validator；
- [ ] 对 tests/`navigation.rs` 机械拆分作 go/no-go；若执行，先外移 tests 且无语义变化；
- [ ] 状态轴改造单独决策，不默认阻塞 History；
- [ ] 建立测试 inventory 与 superseded 映射；
- [ ] 合并共享 fixture、减少重复 Cargo profile；
- [ ] 建立 Bevy fork patch ledger 和 upstream physical lane；
- [ ] 冻结 History runtime 的进入条件。

## 16. 方案取舍结论

以下替代方案在当前阶段不推荐：

- **全量重写**：不能消除真实 authority/lifecycle 复杂度，且会丢失已积累的边界测试；
- **History 先行**：会把尚未闭合的 terminal/replay 语义固化进 history；
- **完整 Authority closure 全部前移**：把不阻塞前台语义的 reaper/lease 治理放到关键路径，推迟用户价值；
- **一个巨型状态 enum**：组合爆炸并模糊独立 owner；
- **把语义性状态重构设为 History 的无条件 blocker**：边界尚会被 History/Recovery 角色补全，可能先付出错误抽象成本；
- **一次提交同时拆模块和改协议**：回归无法归因；
- **全部替换自定义 contract harness**：标准工具无法证明 capability 不可构造和 feature non-reachability；
- **立即切官方 Bevy**：忽略 fork 对物理 UI 行为的补丁；
- **在 MVP 前让 `bsn!` 深入 ECS lifecycle**：扩大 effect surface，并诱导 adapter 获得不应拥有的 authority；
- **MVP 后直接调用通用 `Scene::apply_scene`**：即使事后 refreeze，也无法撤销任意 template/relationship/observer 已产生的副作用，且违反 registry-first realization。

推荐方案的核心不是“少做”，而是让每一阶段只解决一个可验证的风险集合：先完整闭合 Task 7 和 Task 8，再按证据决定机械稳定化，随后增加 History/Recovery，最后分别关闭 Authority 和 MVP；`bsn!` 只进入已经证明安全的 sealed data-only 表达面。这样能最大限度复用既有实现和测试，同时减少在不稳定边界上继续堆叠功能造成的返工。

## 17. 独立复审记录

三名新子代理均只读复审，未编辑文件，也未运行 Cargo/编译。第一轮结果：

| 方向 | 结果 | 已吸收的主要修订 |
|---|---|---|
| 协议/authority | 0 Critical、5 Important、2 Minor | owner 直接进入 envelope；role 先于 retry/fail-close；duplicate/replay 独立；Task 7/8 gate 补全 |
| 路线/`bsn!` | 1 Critical、3 Important | 撤销通用 scene apply；改为 sealed plan lowering；补 Task 7 Steps 1–3；模块拆分改为可选；Authority/MVP 分开验收 |
| 工程流程 | 0 Critical、2 Important、若干 Minor | 机械拆分覆盖占大头的内联 tests；状态记录绑定 exact clean tree、lock/toolchain/artifact；负例继续逐 case 隔离 |

其中唯一 Critical 经本地源码复核成立：现有 `EcsViewDriver`/`FrameContentWriter` 是 World-zero、data-only 边界，而 Bevy 通用 Scene 能写任意 template/bundle、创建 related Entity 并触发 observer/relationship effect；事后 validation 不能恢复已经越权发生的副作用。本文已经删除原先的“预登记 target → `apply_scene` → refreeze”建议。

修订后，三名原复审代理分别复查自己提出的全部问题，协议、路线/`bsn!`、工程流程三路均返回 `PASS`，没有剩余 Critical/Important/Minor。该 PASS 只表示本文档已通过静态一致性复审，不表示仓库代码、测试或编译已经通过。

## 18. 主要证据锚点

- `AGENTS.md:23–54, 79–99`：owner、retry、boundary evidence、partition、publication、adapter 与验证硬约束；
- `docs/authority-transition-matrix.md:11–43, 107–178, 191–203, 221–252`：三类 disposition、role-sensitive closure、ledger/quarantine/terminal；
- `docs/target-architecture.md:61–91, 359–430`：data-only driver、registry-first content realization、原子切换、History/Recovery；
- `docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md:514–727`：Task 7 Step 0、Steps 1–3 和 Task 8 完整 gate；
- `docs/superpowers/plans/2026-07-11-root-push-review-log.md:43`：旧 facade/root targets 被固定 targets 与 `autotests = false` 取代；
- `crates/bevy_ui_router_ecs/src/driver.rs:95–104`、`content.rs:339–455`：当前 `EcsViewDriver` 与 sealed `FrameContentWriter` 事实；
- pinned Bevy `61f78f7` 本地 checkout 的 `bevy_scene/src/scene.rs:17–58`、`resolved_scene.rs:45–80,151–226`、`spawn.rs:454–474`：任意 Scene/related entity/template apply 的 effect surface；
- Git 全历史（截至 `c23d9de`）与本地 Cargo checkout：提交/churn/fork revision 数据来源。
