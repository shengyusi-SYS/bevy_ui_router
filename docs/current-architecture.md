# Current Architecture

**文档类型**: 当前事实

**状态**: Reconciliation PASS

**as_of**: 精确干净树 `5806c59`

**事实来源**: workspace manifest、实现、固定测试、公开旅程、已执行门禁与最终独立复审

`Cargo.lock` 已跟踪，全部 Bevy git dependencies 固定到 exact revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`

## 1 当前定位

Root Push、Nested Push 与 Reconciliation Tasks 1–8 已闭合

Reconciliation 的当前事实必须按四个层次分别读取

- **Implemented**：Core 已实现 slot-aware common-prefix projection、sealed frame/slot roles、common-prefix-preserving suffix splice、request ordering、逻辑 router/host/slot/frame/request/transaction/task IDs、与 exact frame identity 绑定的 generation 及 terminal ordering；ECS 已实现 exact Entity validation、manifest、physical authority identity、attempt、ledger、receipt、physical splice、cleanup 与最小 pre-commit authority settlement
- **Integrated**：上述能力已接入既有 Core → View → ECS → facade 主链，仍由 Core 作为唯一逻辑 lifecycle publisher，没有第二套 reconciliation runtime、publisher 或逻辑 route/lifecycle allocator
- **Facade reachable**：navigation 只通过推荐 facade `push` 与真实 `App::update` 驱动；稳定 consumer 事实由 snapshot、trace 与只读 opaque audit 观察，commit-boundary interactive-leaf cardinality 另由 test-local、只读、非权威的 `World + RouterInputGate` sampler 验证
- **Demonstrated**：公开旅程已证明 Journey B 与 nested fragment；Core/ECS 回归已证明 append、truncate、suffix replace、equivalent physical no-op、exact physical cleanup 与 Profile 所述的 scoped pre-commit authority-loss settlement。Equivalent 路径 zero entering/leaving/effect 且 physical zero mutation，但仍正常发布 request 与 push history

Task 8 focused/full workspace 门禁已在精确干净 `5806c59` 树上通过，四路独立复审均通过，阶段结论为 Reconciliation PASS

## 2 Authority 与 owner 边界

| Owner | 独占职责 | 明确不拥有 |
| --- | --- | --- |
| Core | route tree、matching、request ordering、sealed logical frame/slot roles、逻辑 router/host/slot/frame/request/transaction/task IDs、与 exact frame identity 绑定的 generation、logical cleanup batch、history 与 terminal publication | World、Entity manifest、physical authority identity/attempt/ledger/receipt |
| View | renderer-neutral registry、descriptor 与 effect | World mutation、preservation classification、lifecycle authority |
| ECS | exact Entity validation、manifest、ownership seal、cleanup ledger domain、attempt revision、receipt、physical splice、cleanup、audit 与最小 pre-commit authority settlement | route matching、逻辑 role/ID/generation、history/terminal publication、app-owned host ownership |
| Facade | curated feature/API surface、renderer-neutral `RouteViewRegistry`、`RouterInputGate`/test-support host mapping 等有限 app-side Bevy Entity 输入与只读观察面 | `RouterEngine`、ECS physical `RouteEntityRegistry`、router-owned raw Entity/World/registry conversion/accessor 与 authority/mutation constructor |

生产链只有 Core 一个逻辑 lifecycle publisher

`RouterNavigation` 私有持有 sealed Core `RouterEngine`

- Core prepared commit capability 发布 logical active、push history 与 commit trace
- Core 在匹配的 cleanup/authority-settlement completion evidence 完成后独占发布 request terminal；pre-mutation logical failure 可在 Core 内直接 terminalize，不需要 commit publication capability
- ECS 永不发布或重建 logical active、history 或 request terminal
- ECS-owned `ViewBuildReport` 与 `EcsViewReportSession` 是 facade-reachable public types。`ViewBuildReport` 是只读 kind/message DTO，只有 ECS-issued one-shot session terminal method 可以构造；raw report identity/fields、direct constructors 与 session constructor/seal 保持 private
- 与 public view-driver report 分离，ECS exact physical `FrozenCleanupManifest`、`CleanupLedger`、`CleanupReceipt` 与对应 exact Entity facts 保持 ECS-crate-private。`CleanupManifest`、`CleanupBatch`、`CleanupCompletion` 及相关 integration methods 是 Core `#[doc(hidden)] pub mod integration` 内的 `pub` items；部分 individual methods 可另带 item-level `#[doc(hidden)]`，不得泛化为每个 wrapper/method 都带该 attribute。Facade 不 re-export 该 integration module 或这些 items；它们绑定 opaque Core batch/completion identity 与 metadata。Core 验证 logical roots、exact batch/completion binding、count 与必需的 empty/non-empty shape，但不从物理事实重建或认证 ECS exact physical owner、manifest、ledger 或 receipt

## 3 当前统一运行链

```text
PreUpdate: Refresh -> Ready

Update: ObserveAuthority
  -> CollectNavigation
  -> DriveNavigation
  -> CommitPresentation
  -> SettleCleanup
  -> ReapQuarantines
  -> PublishSnapshots
```

关键事实

- admission revision 绑定 registration incarnation 与 topology fingerprint
- Router-created Entity 先登记 exact ownership，再写 component/relationship，每次可能触发 hook/observer 的写入后复验
- pending、leaving、quarantine、unresolved residual 与 authority-invalid Active 均物理隔离 input 并释放 focus
- successful commit 的 mutation tail 已预构造，只允许不可失败的原位 mutation
- preserved closure 只验证且零 mutation；leaving/entering 按 sealed role 落为 exact physical plan
- ECS 独占 exact Entity obligations、physical authority identities、attempt、ledger、receipt 与 quarantine；Core 只验证 logical roots、exact batch/completion binding、count 与 required empty/non-empty shape
- isolation、quarantine 与 receipt mutation 只落在 exact Router-owned authorized target；foreign descendant/component、protected 或 other-authority parent 及其 component/relationship 全部零写，protected parent 本身不接收 quarantine receipt 或 mutation
- app-owned host 与 foreign descendants 不被 Router 修改、替换或销毁
- ownership seal、authority ID、Core-issued host/frame IDs 与 cleanup identity 一经按各自 owning rules 消费或签发即不复用
- generation 只与 exact frame identity 一起解释，不是全局唯一 scalar；ordinary entering 使用 fresh frame ID 与 initial generation，fragment remount 从同一 current active frame 派生 `checked_next`
- 未发布的 fragment generation 失败且 old active 保持不变时，后续 retry 可以再次派生相同 frame/generation tuple，并由 request/transaction/task identity 与 fresh ECS seals 区分；这不新增 runtime requirement

## 4 已证明且保持的 Root 与 Nested 能力

- facade → `App::update` → snapshot 的真实垂直主链
- repeated Root push、fragment remount、changed-root replacement 与 push history publication
- match/build/apply failure、mandatory fallback 与 terminal diagnostics
- cleanup terminal gate、partial receipt、observer drift、quarantine transfer 与 duplicate/stale rejection
- stale report 在首次 World/registry 写入前按 sealed identity 拒绝
- app-owned host Entity、组件、既有 children、relationship 与顺序保持不变
- locked Bevy layout、visibility、native picking 与 target event delivery
- parent outlet 登记后 root-to-leaf apply，完整 branch 一次 preflight 与一次 Core publication
- frame-local traversal、input/focus barrier、leaf-to-root exact cleanup 与 opaque outlet/content audit
- Authority catalogue 为 Root 20 + Nested 3 = 23

## 5 Reconciliation checkpoints

- Reconciliation contract start / implementation-plan commit：`0201d49`
- Implementation/evidence range：`228c5bf..5806c59`
- Task 1：`228c5bf`
- Task 2：`c627a9e`
- Task 3：`6f8e734`
- Task 4：`9415d03`，review fix `4f4e4dc`
- Task 5：`ddce26d`
- Task 6 plan gate：`9d7d2f0`，implementation `e5a26e6`
- Task 7 plan/public gate：`a694acb`，cleanup guardrail `9307d2f`
- Root fallback plans：`10dc5f4..81d2657`
- Task 7 prerequisite：`2e5cb3d`
- Task 7 public evidence：`5806c59`

## 6 Task 8 已执行门禁

以下 implementation/gate 结果全部属于精确干净 `5806c59` 树，不归因于其后的 Task 8 documentation change 或 completion record

- Core complete target：286 unit tests、28 doctests
- View complete target：8 unit、6 dependency-boundary、1 public API
- ECS all-features complete target：401 unit tests 与全部 integration targets
- public navigation journeys：7/7
- public authority audit：3/3
- facade export allowlist：46/46
- facade authority compile-fail：40/40
- authority catalogue：23/23/23
- workspace format/check：`cargo fmt --all -- --check` 与 all-target/all-feature check 已通过
- workspace nextest：838/838，0 skipped
- workspace all-feature doctest：28/28
- `git diff --check` 已通过，tracked tree 保持干净并指向 `5806c59`

## 7 Task 8 final reviews

- Specification coverage and pending-slice honesty：PASS，no Critical/Important/Minor
- Core lifecycle、single publisher、logical IDs/generation 与 terminal ordering：PASS after review fixes，no remaining Critical/Important/Minor
- ECS ownership、slot lifecycle、physical preflight、cleanup 与 authority settlement：PASS after review fixes through draft5，no remaining Critical/Important/Minor
- Public facade、Journey B、fragment 与 audit evidence：PASS after review fixes，no remaining Critical/Important/Minor

## 8 后续边界

尚未完成

- `replace`/`back` 与完整 History
- error presentation、mount recovery、post-commit host/registration loss 与 stale replay 的完整 Recovery
- multi-router stress、完整 opaque audit、lease authority、background reaper 与 cleanup replay 的 Authority closure
- `RouterInputProbes` 与对应公开 input journey
- 最终 MVP gate
- MVP 之后的 BSN adapter 与嵌套示例

Reconciliation factual close 已完成。History runtime implementation 只能在其 spec/plan 与完整 role/topology/physical-drift/terminal matrix 分别通过独立复审后开始；后续切片继续复用唯一 authority、exact claim、registry-first、post-write validation 与 app-owned host zero-write 约束
