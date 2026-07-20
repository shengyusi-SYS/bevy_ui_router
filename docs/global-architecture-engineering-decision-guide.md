# Bevy UI Router 全局架构与工程决策指导

> 文档类型：dated、non-normative decision guide
> 日期：2026-07-20
> 主要分析基线：`c23d9de`
> 目的：提前纠正宏观方向、优化工程流程、避免后续资源浪费

## 1. 定位与采用边界

本文不是 Roadmap、当前实现规格、阶段计划、实时状态源或完成证明。
**Canonical invariant**：发生冲突时，严格按 `docs/README.md` 当前“权威顺序”列出的完整顺序判断。本文不在该顺序中，只是参考与决策输入；本文不重建、缩写或替代该顺序。
本文分析会引用 AGENTS、Target、Matrix、facade 合同、acceptance journeys、Roadmap、阶段 profile/plan、源码与 Git 记录，但引用不改变各来源在 `docs/README.md` 中的权威位置。factual record 只证明其绑定提交或快照上的事实，不是冲突解决层。

**Canonical invariant**：本文可以质疑 canonical 计划，但不能静默覆盖它。
建议要成为任务、blocker、验收条件或实现授权，必须有可复核证据、明确工程决策、canonical 文档修订和相应评审。
没有 canonical amendment，就没有新的隐式 blocker。

## 2. 四类标签

| 标签 | 含义 | 执行影响 |
|---|---|---|
| **Canonical invariant** | 已由 AGENTS、Target、Matrix 或公共合同冻结 | 当前实现和评审必须遵守 |
| **Dated fact** | 绑定明确日期、提交或只读快照 | 支持分析，不代表实时状态 |
| **Candidate optimization** | 有成本收益依据的流程、结构或工具建议 | 默认非 blocker，采用前正式决策 |
| **Future design hypothesis** | 对尚无独立 spec/plan 阶段的前瞻模型 | 仅作设计输入，不授权 runtime |

标签不会因措辞强烈自动升级；Candidate optimization 也不能因进入临时清单而变成 gate。

## 3. 决策摘要

1. **Canonical invariant**：保留 Core / View / ECS / Adapter / Facade 分层，不以全量重写规避真实 lifecycle/authority 复杂度。
2. **Canonical invariant**：Router 只管理 router-owned subtree；app-owned host、protected 和 foreign closure 不得因局部便利被改写或销毁。
3. **Canonical invariant**：authority、retry、terminal 和 publication 必须保留 exact identity、线性 owner/capability 与 role-sensitive evidence。
4. **Dated fact**：当前 Reconciliation plan 把 Task 7 Steps 1–3 定为 coverage-only public evidence；只有新增断言产生真实 RED，才先修订计划并进入最小生产修复。
5. **Future design hypothesis**：History、Recovery、Authority closure 可分别形成独立 spec/plan；本文只提供候选问题清单。
6. **Canonical invariant**：验证声明必须绑定实际成功输出和 exact observed code state；dirty worktree 结果不能归因到 `HEAD`。
7. **Candidate optimization**：机器状态源、本地 validator 和 CI provenance 值得评估，但不自动阻塞 History。
8. **Candidate optimization**：机械拆分只在热点冲突、owner 调用链或评审成本可测地阻碍交付时启动；若启动，先处理内联 tests。
9. **Candidate optimization**：harness 优先复用 Cargo 图、lock/tree 和 helper 编译；compile-fail 负例仍逐 case 隔离。
10. **Dated fact**：分析基线固定到带 UI 物理行为补丁的 Bevy revision，这是 dependency compatibility 分析的明确输入事实。
11. **Candidate optimization**：可提前盘点发布元数据和使用者文档，但未获 canonical 采用前不构成 Reconciliation/History blocker。
12. **Canonical invariant**：BSN adapter 只能通过公开、data-only 边界接入，不得取得 Router identity、lifecycle、registry、commit 或任意 World mutation authority。

## 4. 无状态依赖的决策地图

本文不定义 A → B → C 的第二套阶段状态机。

**Candidate optimization**：不同决策轴可独立积累证据；遇到共享 authority mutation 或 canonical scope 时，串行/隔离要求仍由相应 canonical 来源决定。

| 决策轴 | 关键问题 | 可并行产物 | 进入执行的条件 |
|---|---|---|---|
| Runtime safety | owner、role、allowed writes、terminal 是否唯一 | 只读审查、focused test 设计 | canonical task 出现证据并正式修订 |
| Evidence integrity | 结果绑定哪个 tree、lock、toolchain | provenance schema、validator spike | 工程流程正式采用 |
| Maintainability | 热点是否拖慢修改、复审或合并 | churn/冲突/耗时度量 | 收益高于迁移和 gate 成本 |
| Dependency compatibility | fork patch 是否仍必要 | ledger、临时 overlay 设计 | revision 变更获批 |
| Release usability | 包能否发布、用户能否从 facade 上手 | metadata/README inventory | 发布准备任务获批 |
| Adapter feasibility | 宏输出能否 lowering 为 closed plan | data-only IR spike | post-MVP adapter spec/plan |

```text
dated evidence or canonical invariant
  -> recommendation with cost/benefit
  -> explicit decision
  -> canonical amendment
  -> scoped task and verification
```

### 4.1 可并行

**Candidate optimization**：以下只读研究或隔离验证通常可以并行：

- History、Recovery、Authority 的只读问题清单、矩阵草案和 public journey 设计；
- 测试启动次数、热点 churn、共享文件冲突和 reviewer trace cost 度量；
- Bevy ledger、发布 inventory、provenance 与 sealed lowering 可行性研究；
- 使用隔离 checkout/target 的互不共享构建产物验证。

### 4.2 串行或隔离边界

**Canonical invariant**：以下工作受现有 authority、计划采用顺序或共享 workspace 验证纪律约束：

- 同一 lifecycle/authority 状态机的生产修改；
- canonical plan 修订及依赖该修订的实现；
- 共享 workspace target 中的 Cargo/rustc 验证。

**Candidate optimization**：若未来正式采用机械拆分、History 或 Recovery 工作，建议把同一共享文件的机械拆分与语义修改隔离，并把会同时改写相同 publication、terminal、cleanup 或 settlement owner 的工作串行化。

## 5. Architecture 与 authority 风险

### 5.1 Ownership、identity 与 sparse topology

**Canonical invariant**：frame/root 必须关联 Router、host、slot、frame identity 和 generation；admission/authority revision 还要绑定显式实例身份。
ownership seal、authority ID 和 capability 消费后不得复用；registry mutation 要验证 exact Entity、owner、seal 与 lifecycle。

current 和 next 分别冻结、分别校验。`next=None` 是只参与验证的 no-op，不得产生 presentation、registry 或 cleanup mutation。
retained-empty、slot-only 等稀疏拓扑只能依赖 exact parent、outlet、slot、mount binding 和 Entity 自身 role，不能从数量、位置或宽泛 lifecycle 并集推导权限。

先分类 authorized target，再把 preserved、non-target current、app-owned、foreign 和 other-authority closure 固定为 protected。
mutation 只能落在 allowed write set；hook/observer-capable 写入后必须重建并复验 target/protected partition。

### 5.2 Registry-first 与 delayed action

**Canonical invariant**：Router-created Entity 先登记 ownership，再写 component/relationship；post-write drift 时保持 inert 并 fail closed。
snapshot、fingerprint 和 projection 只能校验，不能重建 owner。
延迟 action 消费前必须从当前 authority exact refreeze 完整 source projection。

## 6. Retry、terminal 与 publication

### 6.1 Retry

**Canonical invariant**：ready、pending rejection、authority loss 和 retryable shortage 不得压缩成空值、bool 或 generic error。跨 retry 稳定的是原 owner 以及 scope、batch、request 等 canonical identity；拒绝或准备失败必须继续线性返还、移动或消费原 owner，不能重建替身。

`CleanupAttempt` 绑定当前 fresh World evidence。新证据要求递增 attempt revision 并 supersede 旧 attempt；旧 revision 不得继续 mutation 或结算新 attempt。不得要求每次 retry 保持同一 attempt identity，也不得自行发明统一 retry budget；只有 owning plan/spec 明确定义 budget 时才适用该 budget。

只有显式、可证明未来可完成的容量或前置事实不足可以 retry。retry 分支要指出缺少的事实及能改变它的 producer；永久 identity、ownership、registry、manifest 或 relationship 矛盾按 role 有界 fail closed。

**Dated fact**：当前 Reconciliation plan 的 projectionless fallback admission 要求 foreign/stale task 在 admission 前零写入退出，并返还类型化 evidence 与原 owner。

**Canonical invariant**：Matrix 的 role-sensitive 分类保持以下区别：

- pending/entering contradiction 进入 `PendingRejected`，只清理对应 pending/entering 局部 owner，旧 active 保持不变；
- host/current/preserved/leaving 的 authority drift 进入 `AuthorityLost` settlement；
- leaving foreign descendant/component 或 protected inbound parent 可以是 `Ready`，但必须在 publication 前准备隔离与 quarantine；leaving target 自身的 authority drift 仍是 `AuthorityLost`；
- preserved foreign closure 仍是 protected，保持 `Ready` 且零 mutation。

### 6.2 Terminal

**Canonical invariant**：failure cleanup target/obligation 与 cleanup 后 continuation 分别冻结；continuation 只来自原始 failure/authority、attempt 和 branch classification。
duplicate receipt、stale replay、invalid replay 和 completion replay 都应零副作用拒绝，但必须保留各自 evidence。
terminal 后同一 scope 不得残留 action、ticket、capability 或 lifecycle owner；验证应覆盖多轮 settlement 和额外一轮 no-replay。

### 6.3 Publication

**Canonical invariant**：可见切换前完成 validation、allocation、ordering、owner composition、claim rebind 和 cleanup reservation。

```text
Freeze/Classify (read-only)
  -> Prepare (not published)
  -> Atomic tail (prepared in-place mutation only)
```

atomic tail 不得包含 query、结构写入、owner 安装、hook/observer-triggering write 或可失败 validation。
publication 后不得从 World 反推 current，也不得补装 registry/owner。

## 7. 与当前 Reconciliation 计划文本的关系

**Dated fact**：绑定 `docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md` 当前文本，Task 7 Steps 1–3 是 Journey B、fragment、authority audit 和 facade 的 public evidence；Step 0 闭合后，Steps 1–3 默认 coverage-only。
只有新增断言经公开 `RouterNavigation::push` 和正式 `App::update` schedule 得到真实 RED，才能说明新的 production gap。

出现真实 RED 时应保存最短复现，判断是否属于当前合同；若需生产变更，先修订 plan/profile、文件范围和 gate，再修复该 RED 的最小根因并重跑最高层证据。
没有真实 RED 时，不因本文的宏观风险分析重开已闭合实现；本指南也不为 Task 8、History 或其他后续工作新增 blocker。

**Canonical invariant**：若 public journey 不可证明、owner/capability 无法唯一追踪、canonical 文档直接冲突、计划触碰 protected state，或 gate 可被零测试/错误 tree 伪装，实现范围不能私下扩张；应先提出 canonical amendment。资源成本显著高于等价方案可以作为修订建议的补充证据，但本身不授权改写范围。

## 8. 未来 spec 的待回答问题

以下均为 **Future design hypothesis**，不是实现规格。

### 8.1 History

- push、replace、back、equivalent/no-op 各自产生什么 intent；
- intent 绑定哪个 request、branch、generation 和 publication；
- stale、pending、failed、fallback、post-commit cleanup 如何影响 history；
- publication 失败时 history owner 如何返还或消费；
- background quarantine 为什么不能改写已发布 history；
- facade journey 如何证明 cursor、entries 与 active presentation 同源。

### 8.2 Recovery

- route/view/mount failure 与 host/authority loss 的 role 差异；
- preserved active、pending output、authority-invalid current 的 allowed writes；
- error presentation、fallback、保留旧 active、清空 active 的互斥条件；
- cleanup manifest 和 continuation 如何独立冻结；
- recovery 再失败时如何进入唯一 terminal；
- stale recovery result 如何在首次可见 mutation 前被丢弃。

### 8.3 Authority closure

- multi-router、lease、quarantine reaper、cleanup replay 的 exact authority；
- other-authority closure 如何持续 protected；
- foreground terminal 如何与 background disposition 解耦；
- pointer、keyboard、gamepad 与 focus isolation 如何闭合；
- opaque audit 能证明什么、不能授权什么；
- stress、public journey、private World evidence 如何绑定同一 tree。

这些问题清单建议未来 spec 显式覆盖 role、topology、physical drift、owner、allowed writes、receipt、terminal 和 retry producer。

## 9. Exact-tree 证据与候选工具

### 9.1 硬护栏

**Canonical invariant**：未看到命令实际成功输出，不能声称验证通过。验证结论必须绑定该输出对应的 exact observed code state；dirty worktree 的事实不能伪装成 `HEAD` 的验证结果。

### 9.2 Candidate optimization

**Candidate optimization**：需要更强跨机复现时，可采用完整 provenance 字段集：commit/tree OID、clean/dirty 事实、lock digest、toolchain、gate/feature profile、case count、退出码和不可变 artifact/digest。dirty tree 还可用等价 snapshot/diff identity 绑定 exact observed state，而不冒充 `HEAD`。

`status.toml`/`status.json`、只读 validator 和 CI provenance 不是 exact-state 证明的唯一办法。只有现有 gate 明确要求，或状态漂移反复出现、多代理无法确定实际 tree、milestone 需要跨机复现、手工 provenance 成本持续偏高等触发器成立时，才考虑采用这些字段或工具。
采用时可先实现本地只读规则，再让 CI 复用。

**Canonical invariant**：这些字段或工具要成为 gate，须先修订相应 canonical 工程流程；未完成该采用过程时，它们不是 Task 8、History 或其他阶段的隐式 gate。

## 10. 测试、harness 与机械拆分

**Dated fact**：截至 `c23d9de`，`navigation.rs` 约 19,962 行，内联 tests 约占 77%；只移动生产函数不会自动消除热点。

**Candidate optimization**：只有共享文件反复冲突、owner 调用链难以复审、小语义变更持续产生大范围无关 diff，或后续工作被文件结构显著拖慢等成本可测时，才评估机械拆分。
若该候选被正式采用，建议先纯机械外移/归档 tests，保持 exact name、path/filter、cfg 和 production predicate 覆盖，再逐个协议边界移动生产代码；也建议避免在同一 diff 中同时拆模块、改状态 enum、错误分类和公共 API。未获采用时，这些建议不构成 blocker，结构美观本身也不是采用理由。

**Candidate optimization**：harness 优化先度量每个 profile 的 Cargo 启动次数、冷/热耗时、case count 和重复 helper 编译。
若正式采用，可优先复用同 profile 的 lock/tree/prebuild，让正例参数化并共享 isolated consumer target；脚本建议输出 gate、tree、profile、case count 和退出码。
compile-fail 负例建议继续逐 case 隔离并断言允许的 rustc category/fragment，避免共享预生成依赖退化成一次混合失败。
候选评估中的 API/semver 工具只作补充；若要替代现有 feature non-reachability、opaque capability 或 closed-world authority gate，应另行提出对应 canonical amendment。

## 11. Bevy fork

**Dated fact**：项目固定到 `61f78f7`，位于 Bevy 0.19 release 后四个提交，涉及 text measurement、recursive clip check、Parley 0.10 和 text scroll clip clamp。

**Canonical invariant**：无论依赖 revision，AGENTS 与 Target 已冻结 app-owned host、layout/visibility、picking、input isolation 和 focus release 等物理合同。revision 变更若进入实现，仍须用受影响的物理证据重新证明这些合同；`cargo check` 单独不能证明物理行为。

patch ledger、upstream lane 和固定 cadence 是 **Candidate optimization**。
ledger 应记录 commit、upstream 状态、理由、受影响 physical gate 和移除条件。
lane 应使用独立临时 worktree 与 manifest/config overlay 指向候选 ref，并保存 overlay/lock diff；推荐在 revision 变化、里程碑或 ledger 事实变化时运行，而非占用每次 checkpoint。

## 12. 发布风险

**Dated fact**：截至本文日期读取的 `docs/roadmap.md` 未把发布准备列为 Reconciliation、History 或 Recovery 的前置任务。分析基线上缺少完整发布元数据和根级使用者入口，内部 path dependency 与 git Bevy dependency 尚无本文可引用的发布策略。

**Canonical invariant**：本指南不在 `docs/README.md` 的权威顺序中，不能把上述发布事实提升为默认 blocker。

**Candidate optimization**：低成本候选包括明确 `publish = false`、根 README 的支持范围/facade 示例、LICENSE 与统一 metadata、用户入口/实现 crate 区分、path + version 和发布顺序。
package dry run、semver/API diff 与最小 consumer 留给正式发布候选；只有 roadmap 或独立发布任务采用后才形成交付要求。

## 13. `bsn!` adapter

**Dated fact**：分析基线中的 `bsn!` 是 Rust 代码宏，不是 `.bsn` 文件加载、流式更新或热重载系统。截至本文日期读取的 `docs/roadmap.md` 把 immediate BSN adapter 和真实 nested journey 放在 MVP 后，并继续暂停 queued/streaming BSN、file scene 与 hot reload；这是 Roadmap 已采用的 post-MVP 排序方向，不是 runtime spec 或完成状态。

**Canonical invariant**：adapter 只能通过公开 API 影响 Router，不能构造 generation/frame identity、裸 registry 或 commit capability，也不能绕过 lifecycle 切换 active frame。当前 Target 的 data-only content 边界不向 adapter 暴露通用 Scene application API 或等价 arbitrary World mutation。

**Dated fact**：分析基线锁定的 Bevy source 中，通用 Scene application API（如 `EntityWorldMutSceneExt::apply_scene`）可应用任意 template/bundle、创建 related Entity 并触发 observer/relationship effect；事后 refreeze 无法撤销已经发生的 app-owned side effect，也不能补救 registry-first 破坏。

**Future design hypothesis**：一种可研究的 lowering 是让 adapter 接收 Router 签发的 exact request，并只输出可复验的 sealed data-only plan/outcome；Router 继续负责 stale check、preflight、publication 和 cleanup continuation。独立 spike 可研究宏 authoring 能否产生只含已审计 primitive、relationship 和 props 的 closed IR。
若公共 API 无法安全检查 erased template，候选方向是等待上游 inspection/IR boundary 或设计受限 router-side frontend，而不是“先 apply 再校验”。该假设不授权实现。

## 14. 资源投入与停止规则

**Candidate optimization**：本指南建议优先把额外资源投向 owner/capability/terminal 可证明性、真实 public RED、exact-state 证据、已测得的 harness/热点成本、fork physical compatibility，以及已被独立 spec/plan 采用的新能力。

**Canonical invariant**：遇到以下 authority、证据或范围矛盾时，停止扩张并回到现有决策边界：

- 永久矛盾仍走无 producer 的 retry；
- 新实现没有真实 RED 或 canonical amendment；
- dirty tree 未绑定 exact snapshot/diff identity，或把 dirty 结果归因于 `HEAD`、clean tree、Task/阶段 PASS；
- future hypothesis 被直接当成 runtime 授权；
- Bevy 候选只通过编译，没有 physical evidence；
- adapter 需要 arbitrary World、裸 registry 或 identity/commit authority；

**Candidate optimization**：机械拆分候选正式采用后，若同一 diff 混入语义、public API 或错误分类，建议暂停该候选并重新切分评审边界；未采用时不形成独立 blocker。工具候选的治理成本持续高于减少的重复工作且没有新风险证据时，建议停止继续投入。

**Candidate optimization**：正式采用提案建议说明问题证据、当前成本、预期收益、迁移成本、失败回退、采用触发器和 canonical 落点。

## 15. Dated Git 证据

**Dated fact**：以下不描述实时状态。截至 `c23d9de`，2026-07-07 至 2026-07-20 共 175 个提交：`docs` 63、`feat` 53、`fix` 32、`test` 20、`chore` 5、`build` 1、其他 1。
提交中位数为 2 个文件、约 219 行 churn；14 个提交超过 10 个文件，33 个超过 1,000 行；该历史范围没有 merge commit 和 tag。

| 累计 churn 热点 | 增删行 |
|---|---:|
| `crates/bevy_ui_router_ecs/src/navigation.rs` | 31,558 |
| `crates/bevy_ui_router_ecs/src/cleanup.rs` | 17,297 |
| `crates/bevy_ui_router_core/src/engine.rs` | 10,522 |
| `crates/bevy_ui_router/tests/support/facade_contract_cases.rs` | 6,638 |
| `crates/bevy_ui_router_ecs/src/presentation.rs` | 6,021 |

这些数据支持测量协议热点和 review/tooling 成本，不能单独证明流程失控或模块划分错误。
`2e5cb3d` 是基线后的 Root fallback implementation checkpoint；本文未运行其 gate，不自行宣称 Task、Journey 或阶段通过。

## 16. 证据锚点

- `AGENTS.md`：ownership、retry、partition、publication 和验证护栏；
- `docs/README.md`：文档权威顺序；
- `docs/target-architecture.md`：crate、data-only content、publication、History/Recovery 总体合同；
- `docs/authority-transition-matrix.md`：role-sensitive closure、ledger、receipt 和 terminal；
- `docs/implementation-goals.md`：阶段 spec/plan、TDD、public journey 和范围控制；
- `docs/roadmap.md`：唯一 Now / Next / Paused 顺序；
- Reconciliation profile/plan：Task 7/8 的 dated task 文本与执行证据；其冲突处理仍服从 `docs/README.md`；
- manifests、`Cargo.lock` 和 authority verifier：feature、test target、pinned source 与 catalogue 事实；
- ECS driver/content 和 pinned Bevy Scene source：data-only 边界与通用 Scene effect surface。

本文统计来自只读 Git history、manifests 和 source 检查。
本文未运行 Cargo、测试、格式化、静态检查、package dry run 或物理 UI 验证，因此不提供新的完成声明。
