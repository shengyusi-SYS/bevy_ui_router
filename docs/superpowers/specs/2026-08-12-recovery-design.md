# Recovery 阶段设计

**Activation status**: current active canonical/profile stage; History is completed historical evidence. R1 contract delta and authority row are active; R2/R3 remain planned. This does not claim Recovery PASS, full Journey G, or MVP PASS. Background old-lease reap remains Authority-closure work.

**用户设计确认**: 用户已将 MVP PASS、阶段顺序与自主规划/调度权写入持续目标；本规范按该授权收敛 Recovery 设计

**Spec independent review**: PASS；Core lifecycle、ECS physical authority、public/canonical profile 三路复审关闭全部 Critical / Important / Minor

**Plan independent review**: PASS

**阶段状态**: current active canonical/profile stage；History is completed historical evidence；不得据此声明 Recovery PASS、完整 Journey G 或 MVP PASS

**事实基线**: `fe8532c`（History closure verified）；编写本规范时 tracked tree/index clean，用户未跟踪 `review-history-*` 材料不在本阶段范围

**阶段顺序**: History PASS → Recovery（current）→ Authority closure → MVP gate

**权威顺序**: 明确用户目标与 `AGENTS.md` → `docs/target-architecture.md` → `docs/authority-transition-matrix.md` → Facade Fixture/Allowlist/Golden → `docs/mvp-acceptance-journeys.md` → Current Architecture/Roadmap → 本规范 → implementation plan → runtime/tests。本文在 canonical activation commit 前只能提出 amendment，不能覆盖上游合同

## 目标

- 闭合 Journey D 的 route error boundary、router fallback、mount rejection 与 recovery failure 公开语义。
- 闭合 Journey G 中 Recovery-owned 的 foreground 子集：pre-commit host loss、post-commit compound settlement、稳定 unavailable、host re-admission 与 fresh epoch；background old-lease reap 隔离留给 Authority closure。
- 保持 Core 为唯一 request、presentation、history、availability、terminal 与 diagnostics authority。
- 保持 ECS 独占 exact Entity、role partition、allowed write set、cleanup ledger、receipt 与 host edge mutation。
- 让所有 failure/authority 路径线性返还或消费原 owner；snapshot、fingerprint、projection 只用于验证。
- 以三个依赖有序的小闭环完成 Recovery，不先引入通用 `RecoveryOwner` 或重写既有 reconciliation。

## 非目标

- 不实现 background quarantine reaper、round-robin、公开号码化 quarantine API 或 `RouterInputProbes`。
- 不闭合 multi-router registry partition、registry self-audit fail-closed 或全部 Authority catalogue；这些属于 Authority closure。
- 不新增动态 view/driver unregister；缺 view binding、缺 driver仍是安装期同步错误。
- 不引入 platform/BSN/声明式配置、应用 shell、主题、文案或业务 fallback。
- 不让 Router despawn、reparent、补 `Node`、替换或重建 app-owned host/ancestor。
- 不为旧外部版本设计迁移层；项目未发布，但 canonical 文档、公开合同和用户 WIP 在正式修订前保持不变。
- 不把机械文件拆分、命名清理或泛化 ledger 重构混入 Recovery 语义提交。

## 1 当前事实与缺口

| Area | 已存在且复用 | Recovery 必须补齐 |
| --- | --- | --- |
| route failure | `PendingFailedBranch`、`FailureCleanupRequest`、`CleanupLedger`、`RouteFailureContinuationOwner`、router fallback、`RecoveryFailed` | public boundary builders、bound error view、完整 plan/mount failure journey 与 exact trace |
| mount | token/grant/receipt、`MissingOutlet`、`InvalidOutlet`、`ParentAuthorityLost`、stale/replay 拒绝 | failure 类别到 `PendingRejected`/`AuthorityLost`/fallback 的公开与私有闭环 |
| pre-commit authority loss | existing scoped authority settlement、isolation、receipt、terminal | Journey G 公开 host removal/despawn 证据与 `HostLost` cause 映射 |
| post-commit cleanup | Core `WorkStage::PostCommitCleanup`、ECS `CleanupLedger`、immutable obligations 与 receipts | 原 ledger 原样转交 + disjoint authority ledger/batch 的 compound settlement、无重复 obligation/receipt、no rollback、severe diagnostic |
| admission | `HostCandidateClassification`、candidate revision/latch、token/grant/lease、top-slot staging | source-bound unavailable permit、`Unavailable(reason)` 发布、eligible re-admission、新 epoch、same-host old-edge gate |
| public surface | error key、view registry、snapshots、availability/diagnostics 已公开 | `RouteNodeOptions::{with_error_boundary,with_error_view}` 从 exclusions 移除并由 owning public RED/GREEN 证明 |

现有 root match/build/apply fallback 测试是可复用 characterization，不等于 Journey D 完整 PASS。现有 crate-private host settlement/readmission 测试是实现资产，不等于 post-commit compound settlement或公开 Journey G PASS。

## 2 Authority 与模块边界

| 模块 | 唯一职责 | 禁止事项 |
| --- | --- | --- |
| Core `RouterEngine` | request、attempt、failure continuation、presentation/history commit boundary、availability、terminal、diagnostics、logical cleanup token | 读取 World/Entity、解释 component manifest、从 snapshot 重建 owner |
| ECS navigation | live World preflight、target/protected partition、host candidate、exact Entity claims、physical isolation/cleanup/receipt | 自行发布 request terminal/history，伪造 Core continuation或 logical batch |
| ECS cleanup | immutable obligation ledger、versioned attempt、physical disposition、receipt、quarantine foreground transfer | 重新签发 Core batch、按数量推导 lifecycle、触碰 app-owned host/component |
| Facade | builders、navigation commands、只读 snapshots/audits | authority accessor、raw Entity、ticket、grant、lease、receipt、commit capability |
| View driver | 通过受限 writer 产生真实 plan input 或失败 report | 注入 classification/terminal、写 registry/host/history/lifecycle |

Core 与 ECS 的 compound transition 只通过私有 consuming owner 和 callback bridge连接。callback 只能在 Core exact receiver 与全部可失败准备完成后调用一次；mismatch 调用零次并返还整个 owner。

## 3 全局不变量

### 3.1 Commit 边界

`CommitPublished` 是唯一 no-rollback 边界：

- 边界前失败：presentation/history 保持旧值，running request在 cleanup/settlement 后 `Failed`。
- 边界后异常：presentation/history 不回滚；active 经 authority settlement 清空；原 request在 settlement 后 `Succeeded` 并携 `Severe + PostCommitCleanupAnomaly`。
- publication 前完成 owner 组合、capacity、排序、manifest seal、registry transition、input/focus transition、trace/diagnostic capacity 与 terminal preparation。
- publication tail 不含 query、allocation、fallible validation、relationship/component insert/remove/despawn 或 hook/observer-triggering write。

### 3.2 Owner 与 evidence

每个切片开工前必须冻结：

| Field | Requirement |
| --- | --- |
| input owner | 唯一当前 holder 与 exact receiver stage |
| output owner | success、retryable、permanent mismatch、terminal 各自的线性 owner |
| allowed writes | exact Entity/record/resource/state 的闭集 |
| forbidden writes | app-owned、foreign、other-authority、new epoch 与未授权 relationship |
| failure return | 原 owner 或包含原 owner 的 typed inert holder |
| acceptance evidence | public journey + Core/ECS exact regression + owner inventory/trace |

boundary evidence 必须保留 source、expected、observed、关联 Entity/obligation、owner 与 failure class；不得降为 `bool`、`Entity`、`None` 或通用错误。

### 3.3 Physical authority

- Router-created Entity 先登记 ownership，再写 component/relationship。
- 每次可能触发 hook/observer 的写入后 `World::flush`，重新构建 target/protected closure 并 exact refreeze；失败后停止后续写。
- input/focus isolation 先于 cleanup；hidden/retained/quarantined closure 始终阻断 pointer、keyboard、gamepad 并释放 focus。
- cleanup 只消费 exact current claim（Entity generation + owner + seal + lifecycle + manifest）。
- app-owned host、ancestor、existing child、foreign descendant/component 只读保护；host edge mutation仅限旧 `HostLease` 授权的 exact host→old-top-slot edge。

### 3.4 Current/next role freeze

每次 Recovery transition都构造私有 `RecoveryRolePartition`（实现可采用等义名称），逐 Entity 独立冻结 current 与 next：

| Role | Current | Next | Mutation authority |
| --- | --- | --- | --- |
| preserved prefix | exact active/current claim | same active/current claim | protected，零写 |
| old boundary + failed suffix | exact current/pending claim | leaving/absent/quarantined | authorized target only |
| prepared error output | registered inert claim | active error claim | publication前只隔离；publication tail原位切值 |
| post-commit newly active | exact committed active claim | authority-invalid settlement target | 只进入新 authority manifest |
| old leaving residual | original cleanup obligation | disposition-specific terminal | 只由原 ledger `L` |
| no-op entity | exact current claim | `None` | 合法校验项，不进入 mutation plan |

partition 同时冻结每个 non-target protected closure、完整 relationship closure、input/focus source、registry manifest 与 allowed write set。error publication后不得从新 active反推旧 current/boundary/suffix；cleanup continuation只来自原 failure/authority、attempt与branch分类，不从 active、obligation count、receipt disposition或empty manifest反推。

## 4 阶段切片

| Slice | Deliverable | Dependency | Public gate | Authority addition |
| --- | --- | --- | --- | --- |
| R1 Error presentation | boundary/fallback + plan/mount recovery matrix | History failure cleanup | Journey D | `error_outlet_role_classification_is_preserved` |
| R2 Compound settlement | pre/post-commit host loss + two-ledger single-compound transfer | R1 diagnostics/terminal facts | Journey G foreground host-loss rows | `post_commit_host_loss_preserves_committed_history` |
| R3 Re-admission/stale output | unavailable reasons + new epoch + same-host detach gate + sealed stale-output cleanup | R2 old host terminal | Journey G foreground re-admission rows + Scenario E | `host_reacceptance_detaches_old_top_slot` |
| R4 Completion gate | fixed profiles、facade、workspace、docs factual close | R1–R3 | D complete + G Recovery-owned foreground rows rerun | no new runtime semantics |

每片使用同一 approved spec，但拥有独立 TDD task、implementation review、quality review 与精确提交。R1–R3 不并行写 lifecycle runtime。

## 5 R1：Error presentation 与 mount recovery

### 5.1 Public configuration

Recovery 只激活以下既有 builder；不新增读取或 authority API：

```rust
pub fn with_error_boundary(self, key: ErrorBoundaryKey) -> Self
pub fn with_error_view(self, boundary: ErrorBoundaryKey, view: RouteViewKey) -> Self
```

`with_error_boundary` 只声明 boundary；已配置 route 未绑定对应 error view时 `add_ui_router` 同步返回 `MissingErrorView`。`with_error_view` 同时声明相同 boundary identity 和 view binding。普通 view 缺失优先于 error view缺失。

### 5.2 Owner transition

```text
sealed normal attempt
  -> typed failure report / mount rejection
  -> whole PendingFailedBranch {
       terminal: FailureCleanupRequest { independently frozen continuation },
       branch: ECS FailedPreparedBranch
     }
  -> destructure only at exact install boundary
  -> Core FailureCleanupPrepared holds continuation
     + ECS CleanupLedger holds exact physical branch
  -> exact completion jointly consumes Core stage + ECS ledger owner
  -> prepared bound-error / router-fallback action or terminal
  -> atomic error presentation publication
  -> suffix cleanup completion
  -> Failed(original cause)
```

若 recovery attempt 自身失败：

```text
recovery failure owner
  -> cleanup its exact residual
  -> terminal after all receipts
  -> Failed(RecoveryFailed) with original + recovery evidence
```

continuation 与 cleanup manifest独立冻结；continuation只能来自原 failure、attempt与branch分类。现有 `RouteFailureContinuationOwner` 专用于 History Back target pending-rejected acceptance，R1不得复用或跨 schedule保存它。若 completion bridge需要显式 owner，使用 R1-private、non-Clone `FailureCleanupCompletionOwner`，唯一 producer是 Core `FailureCleanupPrepared` 的 consuming transition。

`prepare_failure_cleanup_retaining` 安装前 mismatch返还`FailureCleanupRequest + FailedPreparedBranch`并立即重组原whole `PendingFailedBranch`。安装后 mismatch保留整个 `Core FailureCleanupPrepared + ECS CleanupLedger` pair，不得通过projection或receipt重建原request/continuation；不得在owner空槽中轮询。

### 5.3 Boundary 选择

- match failure 没有 matched branch，只能使用 mandatory router fallback；error presentation `matched=None`。
- build/apply/mount failure 从失败 route向祖先选择最近且 exact binding 的 boundary。
- 只保留 boundary 祖先侧、严格位于 boundary 之上的已提交合法 frames；boundary及其失败 suffix进入 cleanup。
- recovery frame使用新 frame id/generation，origin 为 `RouteError(boundary_route)`；router fallback origin为 `RouterFallback`。
- boundary identity/view binding在 planning 时冻结，消费前对 route tree compiled manifest与 driver registration exact refreeze。
- boundary丢失/driver失效是安装合同矛盾；MVP没有动态 unregister，runtime不伪造该输入。

### 5.4 Failure/mount 分类矩阵

| Input failure | Real predicate | Mount trace | Lifecycle class | Recovery target | Old active/history |
| --- | --- | --- | --- | --- | --- |
| match | matcher no branch | none | route failure | router fallback | commit前保持；history不推进 |
| build | driver `build_failed` | none | route failure | nearest boundary/fallback | 同上 |
| apply | driver `apply_failed`，含 tracked partial output | none | route failure + residual cleanup | nearest boundary/fallback | 同上 |
| duplicate outlet | writer duplicate key | none | plan-level `ApplyFailed` | nearest boundary/fallback | World/registry在plan返回前零写 |
| foreign-session outlet | writer session mismatch | none | plan-level `ApplyFailed` | nearest boundary/fallback | 同上 |
| relationship self-edge/cycle | writer plan validation | none | plan-level `ApplyFailed` | nearest boundary/fallback | 同上 |
| missing entering outlet | accepted parent plan缺 required outlet | `Requested -> Rejected(MissingOutlet)` | `PendingRejected` | nearest boundary/fallback | active/history保持 |
| invalid entering outlet | post-write entering anchor不再 exact | `Requested -> Rejected(InvalidOutlet)` | `PendingRejected` | nearest boundary/fallback | active/history保持 |
| preserved/current parent outlet失权 | existing anchor identity/owner/seal/edge drift | `Requested -> Rejected(ParentAuthorityLost)` | `AuthorityLost` | no fallback；authority settlement | settlement后active None，history旧值 |
| recovery build/apply/mount failure | same production predicates on error attempt | exact per class | recovery failure | no second recovery | old presentation/history保持；`RecoveryFailed` |

同一错误同时命中 pending defect 与 current/host loss时，`AuthorityLost` 优先。plan writer错误不得产生 mount token；mount rejection必须消费真实 token/grant/permit，不能由测试直接注入 enum。

### 5.5 Allowed write set

| Stage | Allowed | Forbidden |
| --- | --- | --- |
| plan validation | test/driver-local plan buffers only | World、registry、request terminal、host |
| failed-output isolation | exact pending/entering residual的 existing lifecycle/gate values；focus resource | preserved/current、host、foreign/app-owned |
| failed-output cleanup | exact failed obligations的 relationship/despawn/registry retirement/receipt | protected closure与不在manifest内的component |
| error publication | prebuilt registry/input values + Core prepared publication | allocation、query、hook write、post-publication owner install |
| suffix cleanup | exact leaving suffix obligations | preserved prefix、new error frame、host |

### 5.6 Public evidence

Journey D 必须从 facade真实执行并分别覆盖 match、build、apply、duplicate outlet、foreign-session outlet、plan cycle、missing outlet、recovery failure；至少一条 nested boundary success和一条 router fallback success。断言：

- immutable `requested/matched`、error origin/frame identity、history不推进；
- error commit前旧 active仍可见，commit后 Core/ECS error frames一一对应；
- plan-level failures无 mount events，missing outlet有 exact mount rejection序列；
- residual receipts在 fallback issuance前完成，terminal最后；
- recovery失败后无残留 roots/owners，下一合法导航可运行；
- app host fingerprint逐字段不变。

安装期 public/consumer gate另固定：

| Case | Expected |
| --- | --- |
| `with_error_view(boundary, view)` + registered normal/error drivers | router安装成功，nested failure选择exact bound error view |
| only `with_error_boundary` on node with normal view | synchronous `RouterInstallErrorKind::MissingErrorView` |
| node同时缺normal view与bound error view | `MissingRouteView` precedence |
| unknown normal/error/fallback view或未注册driver | 对应同步 install error，不创建router/host/request |
| facade surface | signatures逐字匹配Fixture/Golden，只删除两个exclusions，无新增accessor/authority symbol |

## 6 R2：Pre/Post-commit authority settlement

### 6.1 Pre-commit host loss

Pre-commit继续复用 existing scoped authority settlement，不建立 Recovery compound batch：

```text
HostBinding + running owner + active/pending physical owners
  -> exact authority-loss classification
  -> retired HostLease + one authority CleanupBatch
  -> isolation / receipts / completion
  -> active None + history unchanged + Failed(HostLost)
  -> Unavailable(HostLost)
```

queued requests可立即 `Failed(HostLost)`；running request必须先 `Running(CleanupPending|Recovering)`，在 foreground receipt和Core completion后 terminal。registration removal与host Entity absent都映射 `HostLost`，普通 active authority drift映射 `AuthorityLost`。

### 6.2 Post-commit compound owner

post-commit transfer的输入必须同时存在且 exact：

- Core current stage为 `PostCommitCleanup { outstanding }`；history capability已消费；request已含 `CommitPublished`。
- ECS持有与 original batch header匹配的唯一 normal `CleanupLedger L`；`L`可为 canonical-zero、unattempted、partial receipts + live issued attempt，或physical-settled但Core尚未terminal。transfer时要求`L`的batch/header/owner/receipt vector exact；未结算index才按当前registry与World重新分类，已结算index只验证receipt identity与冻结terminal projection，不要求原claim/owner/lifecycle仍current，也不重新observe。
- ECS `active`、`L`中的 leaving owner、`HostBinding`/`HostLease`与input/focus source可 exact refreeze；`L`的registry/World复验严格按上一条的settlement state区分。
- ECS已从committed active、old top slot与同一committed request/transaction仍持有且不在`L`中的old-epoch duties冻结`FrozenAuthorityManifest A0`并预留physical ledger capacity；`A0`尚未绑定Core batch。其他request/queue/new-epoch pending始终protected。`A0`与`L`的immutable obligation exact-Entity set（包含已结算项）先互斥；随后只对`A0` targets逐项证明current exact generation/claim/owner/seal/lifecycle/manifest，对`L`按settlement state验证。union覆盖全部authorized targets。
- 同一request/transaction/active/old-host-edge source没有竞争cleanup/authority holder、pending publication或foreign receiver。pre-existing independent quarantine/stale-output holder可以共存，其closure保持protected且不进入`A0`。

私有 consuming transition固定为：

```text
Core PostCommitCleanup owner
  + ECS exact original CleanupLedger L
  + FrozenAuthorityManifest / reserved physical ledger A0
  + HostBinding / HostLease
  -> PreparedPostCommitAuthorityTransfer
  -> Core exact receiver + new authority batch/capacity
     + terminal/diagnostic/queue preparation
  -> all-fallible A0/batch fact checks produce PreparedAuthorityLedgerBinding
  -> retire HostLease as final fallible step
  -> callback exactly once commits the prevalidated, infallible A binding
     and installs ECS { L + A + RetiredHostBinding }
  -> assignment-only Core stores RetiredHostAuthority/compound continuation
     and publishes RecoveringAuthority
  -> L settles only original residual
  -> A settles new authority-invalid non-top-slot targets leaf-to-root
  -> H settles exact old host-edge disposition
  -> A settles old top-slot Entity obligation only after H complete
  -> exact compound completion consumes L + A + H + continuation
  -> Core publishes terminal/unavailable state
```

`PreparedPostCommitAuthorityTransfer` 是 stage-private、non-Clone、不可公开构造的概念 owner；实现可采用等义名称，但必须在 plan 中冻结唯一 producer/consumer并用source-structure test证明无borrowed/generic旁路。ECS用外部 `Option<PreparedEcsPostCommitCompound>`/consuming guard 保留whole owner；callback只在success时take。Core batch header签发后、`retire_current`前，ECS以只读batch facts对`A0`完成manifest digest/count/domain/receiver复验，形成`PreparedAuthorityLedgerBinding`；callback中的`commit(batch)`只做不可失败owner move/assignment，不再验证或分配。Core mismatch、capacity/binding失败或`retire_current`失败时callback为零，返还`HostLease`，ECS用`RetiredHostBinding::restore`重组原`HostBinding`并保留`L`、active与`A0`/prepared binding。已消耗但未发布的私有identity只burn不复用。

从callback return到`RecoveringAuthority` publication完成的atomic tail内，不得再有query、validation、allocation、fallible owner操作或hook-capable write；只允许把prepared Core state、records与compound continuation原位赋值。`L/A/H`的hook-capable physical settlement只在该publication完成后的独立settlement stage运行。

ECS compound holder线性持有：

- 原 `L`及其`Leaving`/physical owner（即使receipts已完成也不提前drop）；
- 独立sealed authority `A`及当前active/同request old-epoch residual physical owners；
- ECS侧的`RetiredHostBinding + frozen old-host children/edge`；Core compound continuation侧的`RetiredHostAuthority`；二者共同形成跨层`H`且任一侧不能单独完成；
- `L`与`A`各自immutable obligations、receipts、ledger domain、attempt sequence、batch header与completion digest；
- cause、request/batch/host source projection与typed mismatch evidence。

### 6.3 Two-ledger single-compound rule

- `L`原 batch/manifest/token/domain/digest/count/receipts/attempt state原样转交；不签发第二normal batch，不复制或扩展旧obligations。
- `A`只签发一次新authority settlement batch；其manifest覆盖committed active、old top-slot Entity及同一committed request/transaction中其他不在`L`内的old-epoch exact duties；其他pending不在authorized set。
- `L ∩ A`首先用`L` immutable obligation exact-Entity set（包含已结算项）与`A` exact-Entity set比较并要求为空；不能让同一Entity借不同claim/owner进入两边。`A`每个target都必须证明current exact generation/claim/owner/seal/lifecycle/manifest且唯一。`L`只要求batch/header/owner/receipt vector exact：未结算index按当前事实重新分类，已结算index只验证receipt identity与冻结terminal projection，不要求原claim/owner/lifecycle仍current且不重新observe。intersection、coverage gap或同Entity不同claim在首写前永久拒绝。
- `H`只授权old host→old top-slot edge的exact detach或host/edge `AlreadyAbsent` proof；它不是`L/A`的Entity receipt。
- `A`把old top-slot obligation标记为typed host-edge dependency；`H`未complete时该index不可进入attempt、quarantine或receipt。`A`的其他leaf-to-root duties可以先结算；`H`完成并post-write refreeze后才处理top-slot Entity。
- 已结算`L` obligation不重新attempt/observe/emit receipt；partial residual只对未结算index操作。live issued attempt在transfer时无World写supersede，保留已接受receipts，以更高revision重分类residual。
- ordinary postcommit `L`在physical settlement后仍保留`Leaving` owner，直到normal Core completion或compound completion成功callback消费；completion error不得提前drop。
- compound continuation仅在 `L complete + A complete + H complete` 后可消费；zero `L`仍提供canonical completion facts，不跳过owner。
- wrong `L/A` batch、stale host、duplicate transfer、receiver mismatch均World/Core零写，callback零次，whole owner保存在typed inert holder。

### 6.4 Publication 与 terminal 顺序

```text
CommitPublished
RequestPhaseChanged(PostCommitCleanup)
[zero or more L::CleanupSettled(disposition)]
RequestPhaseChanged(Recovering)
[remaining L::CleanupSettled(disposition)]
[A(non-top-slot)::CleanupSettled(disposition)]
H::HostEdgeSettled
[A(top-slot)::CleanupSettled(disposition)]
compound CleanupSettled aggregate
Diagnostic(Severe, PostCommitCleanupAnomaly, exact cause)
RequestTerminal
```

当同一update压缩多个阶段时，trace顺序仍必须成立。最终：

- presentation active变 `None`，history保持已提交location/revision/cursor；
- committed request `Succeeded`；request/router diagnostics含同一事件身份与cause；
- queued requests `Failed(HostLost)`；
- availability为 `Unavailable(HostLost)`；
- Core current/action/capability/batch/settlement和ECS active/ledger/host holder均关闭；独立 quarantine lease可存续但不属于request owner。

### 6.5 Host-loss timing matrix

| Timing | History | Active | Running request | Cleanup owner | Terminal |
| --- | --- | --- | --- | --- | --- |
| before normal publication preparation | old | old→None through settlement | Recovering | new scoped authority batch | Failed(HostLost) |
| publication prepared但callback前 | old | old | original owner returned | no transfer/no write | nonterminal until exact retry/settlement |
| after `CommitPublished`, canonical-zero `L` | committed | new→None | PostCommitCleanup→Recovering | complete zero `L` + new `A/H` | Succeeded + severe |
| after `CommitPublished`, unattempted `L` | committed | new→None | same | unchanged `L` + new `A/H` | same |
| partial receipts + live issued attempt | committed | new→None | same | accepted receipts retained；attempt superseded；residual reclassified | same |
| physical-settled/Core-pending | committed | new→None | same | whole settled `L` owner + new `A/H` | same |
| duplicate transfer / wrong `A` batch | committed | unchanged | unchanged | whole `L/A/H` source returned；zero write | nonterminal |
| after normal request terminal | committed | committed active | no running request | new authority incident scope | Authority closure/next incident；不改写已terminal request |

最后一行不属于 R2 post-commit request transfer；实现不得借 Recovery 修改已终结request。

### 6.6 Physical writes

- registration removal：只移除应用自己已移除的事实；Router不补回 registration。Router可移除且只移除 old host→old top-slot exact edge。
- host despawn：所有已不存在 exact Entity以 `AlreadyAbsent` 收敛；不得扫描相同index或新generation。
- old host仍存在：host全部非-`Children` components、其他 children identity/order与ancestry保持不变。
- `L`和`A`分别逐obligation驱动；每次可能触发hook的remove/insert/despawn后立即flush、重建完整target/protected closure并exact refreeze，drift后不得执行下一写。
- despawn后必须先refreeze physical-absent transition，再做exact registry retirement/receipt commit，再次refreeze；不得把retirement/receipt写在第一次postwrite refreeze之前。
- 加入production-predicate回归：第一写hook把remaining target或protected Entity reparent/加foreign component时，后续World写为零，whole compound owner与既有receipts保留。

### 6.7 Public evidence

| Host-loss case | Schedule/input | Required public evidence | ECS/Core exact evidence |
| --- | --- | --- | --- |
| first navigation pending host despawn | before commit | history empty、active None、running cleanup→Failed(HostLost) | absent claims `AlreadyAbsent`、owner closure |
| existing active + new request pre-commit despawn | before commit | old history不推进、active settlement后None | one scoped authority batch、no fallback |
| existing active registration removal | before commit | host Entity/sentinel intact、Unavailable(HostLost) | exact old edge only、other host children/order unchanged |
| post-commit registration removal | `after(CommitPresentation).before(SettleCleanup)` | committed history retained、Succeeded+same severe diagnostic | `L/A/H` exact-once、queued HostLost |
| post-commit host despawn | same window | same no-rollback/terminal order | absent duties + disjoint manifests + no duplicate receipt |

R2 public fixture使用真实Bevy schedule set、真实registration removal/despawn与公开snapshots；fault seam不得直接注入host-loss classification或terminal。

## 7 R3：Unavailable、re-admission、new epoch 与 stale output

### 7.1 Availability classification

host observation是显式 typed transition，不以“无动作”代表状态：

| Candidate observation | Stable availability | Retry rule |
| --- | --- | --- |
| initial no candidate | `AwaitingHost` | 新candidate出现即可首次 admission |
| settled host loss且无candidate | `Unavailable(HostLost)` | 只在candidate fingerprint/revision变化后评估 |
| multiple candidates | `Unavailable(HostConflict)` | 集合变化后重分类 |
| unique invalid Node ancestry | `Unavailable(InvalidHostUiTopology)` | 同一registration topology fingerprint变化后重分类 |
| grant/staging post-write rejection | `Unavailable(AuthorityEstablishmentFailed)` | 同candidate revision闩锁；revision变化后一次新attempt |
| candidate/admission identity exhaustion | `Unavailable(AuthorityEstablishmentFailed)` + private permanent source kind + severe diagnostic | permanent for current router instance；candidate revision变化也不重开、不复用ID |
| unique valid candidate | 保持当前直到 exact confirm，随后 `Ready` | token/grant/lease single-use；失败返回owner或闩锁 |
| registry invariant contradiction | `Unavailable(RegistryInvariantViolation)` | Recovery不重开；仅新 router instance可恢复 |

`ReadyNoActive` 是 `Ready + ActivePresentation::None`，不是新枚举。`Unavailable` 下命令统一返回 `HostUnavailable` 且不创建 request id；`RecoveringAuthority` 返回 `AuthorityRecovering`。

### 7.2 Re-admission owner transition

Core先签发私有、non-Clone `HostAdmissionSourcePermit`（实现可采用等义名称），绑定router、availability incarnation、source kind/reason与当前无host事实。ECS将它与candidate registration incarnation、revision、topology fingerprint及latch状态组合成唯一prepared source；projection只验证，不能重建permit。

```text
HostAdmissionSourcePermit
  + exact unique ValidatedHostCandidate source owner
  -> HostAdmissionToken
  -> HostGrant + registered inert staged top-slot owner
  -> per-write flush + post-write exact refreeze
  -> prepared Core confirmation + prepared ECS HostBinding parts
  -> issuer confirmation as final fallible step, producing HostLease
  -> callback exactly once installs HostBinding
  -> assignment-only Core publication of fresh HostVersion/top-slot
     and ReadyNoActive last
```

grant/confirm前同时exact refreeze Core availability source与ECS candidate source。mismatch返还permit + grant/token + staged holder；若staged World已存在则保持registered inert并由其独立cleanup owner收敛，不能丢弃或从Entity重建。issuer confirmation失败时callback零次并返还whole source；callback后不得再有query、validation、allocation或fallible owner work。

eligible source精确为initial `AwaitingHost`，或candidate revision相对已发布unavailable source发生变化后的 `Unavailable(HostLost|HostConflict|InvalidHostUiTopology|AuthorityEstablishmentFailed[retryable staging rejection])`。identity-exhausted private source kind永久不允许；`RegistryInvariantViolation`、`RecoveringAuthority`、`Ready`永不允许。任何prior provisional top-slot settlement/staged cleanup owner必须先exact terminal并清空，fresh permit才可签发；revision变化不能绕过该owner gate。confirmation前availability/host snapshot不伪装Ready。

top-slot staging拆为逐写micro-step：登记ownership后，每次bundle/component/relationship insert都立即flush并重建candidate/target/protected closure；第一写hook reparent到app-owned parent或改变registration/topology时，后续写为零，tracked staged holder进入typed rejection/cleanup。

### 7.3 Same-host old-edge gate

同一个 app host Entity可在新registration incarnation中再次成为candidate，但必须满足：

- old `HostLease`已线性退休为 `RetiredHostAuthority + RetiredHostBinding`，这两个owner在edge settlement完成前不得drop；
- owners exact授权old host→old top-slot `ChildOf` detach；随后立即flush并证明old host `Children`不含slot、其他children identity/relative order与全部非-`Children` components等于冻结值；
- host或edge已不存在时只产生typed `AlreadyAbsent` edge proof，World零写；
- old top slot已absent或转入与new epoch解耦的inert quarantine lease；
- edge proof与old top-slot foreground disposition完成后共同消费两个retired owners，才允许same-host re-admission；
- new grant分配不同 `HostVersion`、top-slot id、Entity/claim/seal；任何旧identity不复用。

旧 quarantine lease可继续存在，但 Recovery不实现其后台reap。R3只证明re-admission发生时old lease与new epoch authority分离；“实际old lease reap不改写new epoch”由Authority closure的`old_quarantine_lease_cannot_mutate_new_epoch`证明。

### 7.4 No replay 与 history restore

- re-admission不重放旧 command、frame task、view report、mount token或commit ticket。
- ReadyNoActive时history保留；只有新的显式 `replace(history.current())` 才构建新presentation。
- rebuild使用fresh request/transaction/frame generation与new host epoch；成功后history按replace语义发布，不能从旧 active snapshot恢复owner。
- 无独立physical owner的old token/grant/receipt/completion replay必须分类为stale/duplicate/invalid并保持new epoch World、registry、request lifecycle、history与host projection零写；consumer可追加一次结构化report-disposition diagnostic，但不得改变allocator/current/action/queue。

### 7.5 Sealed stale-output cleanup

合法旧 view report只有在同时携带独立、仍有效且exact的 sealed old-output owner时，才可清理自己的pending output：

```text
stale report + SealedOldOutputOwner
  -> current receiver classifies StaleDiscarded with zero lifecycle/World write
  -> PendingStaleOutputCleanup { exact old branch, independent batch owner }
  -> isolate + cleanup/quarantine only that exact old output
  -> receipts/completion consume old-output owner
  -> current/new epoch request, active, history, host and queue unchanged
```

receiver mismatch返还report + old-output owner；无owner的stale report不得创建cleanup。old output与current/new epoch target/protected closure必须不相交；wrong host/frame/generation/transaction/sequence/task或reused Entity generation零写拒绝。cleanup可使用`AlreadyAbsent`或foreground quarantine disposition，但不渲染fallback、不创建request terminal、不取得current Core continuation。

### 7.6 Topology/stale matrix

| Case | Allowed action | Forbidden | Evidence |
| --- | --- | --- | --- |
| same registration修复缺Node ancestry | new topology revision后完整 admission | Router补Node/改parent | Unavailable→ReadyNoActive |
| cyclic/non-Node ancestry | 只读分类并发布reason | staging top slot | exact fingerprint + zero registry write |
| same rejected revision | no retry | new IDs/Entity/lease | latch unchanged |
| candidate/admission identity exhausted | publish bounded unavailable once | panic、wrap、unbounded retry | no World/registry write、ID不复用 |
| registration remove+reinsert same Entity | fresh incarnation/revision | reuse old candidate/lease | new HostVersion/top slot |
| old top slot foreign descendant | detach old host edge，foreground quarantine terminal | touch foreign child | old lease remains independent |
| old artifact replay without owner after new Ready | classify only | create cleanup或mutate new epoch | lifecycle/history/host projection equal；最多一次structured diagnostic |
| sealed stale output after new Ready | clean exact old output only | current/new target、request/history/host write | old receipts close；current lifecycle projection equal |

### 7.7 Public/private evidence

| Case | Public evidence | ECS/Core exact evidence | Stage owner |
| --- | --- | --- | --- |
| fresh app host after HostLost | ReadyNoActive、fresh host version/top-slot、history preserved | new lease/claim/seal；old owners closed | Recovery |
| same host Entity reinserted | new incarnation/version/top-slot；host sentinel/children preserved | retired edge proof before new admission | Recovery |
| invalid non-Node/cyclic ancestry | Unavailable(InvalidHostUiTopology)、command无request | zero staging/registry write | Recovery |
| same registration topology repaired | new revision后ReadyNoActive | exactly one new attempt | Recovery |
| explicit replace(history.current) | new route active、fresh frames、history canonical | no replay of old tasks | Recovery |
| old artifact without owner | public/new snapshot unchanged | stale/duplicate/invalid zero write | Recovery |
| sealed stale-output report | current public snapshot unchanged | exact old cleanup only | Recovery Scenario E |
| actual old quarantine lease reap after new epoch | no new host relationship/slot drift | lease operation sequence/round-robin | Authority closure |

## 8 Role、topology 与 physical drift 矩阵

矩阵按有不同 authority/terminal后果的等价类覆盖，不展开无意义笛卡尔积。

### 8.1 Role partition

| Role | Normal failure target | Host settlement target | Always protected |
| --- | --- | --- | --- |
| entering/pending | exact failed suffix/residual | exact known pending records | other request pending、foreign/app-owned |
| leaving | error boundary以下suffix或post-commit original obligations | original `L` exact residual | preserved prefix |
| current/preserved | normal recovery不写 | authority `A`只覆盖classified invalid current；still-valid non-target保持protected | preserved prefix/other-authority current |
| child slot/outlet | only owned slot bound to target parent | old epoch exact slots | preserved parent-owned slots |
| top slot | normal recovery不写 | `A`覆盖old slot Entity；`H`只覆盖retired-owner授权edge | host及其他host children |
| host/ancestor | none | read-only candidate/edge anchor | entity、components、parent、non-target children |
| quarantined owned root | none | foreground transfer可建立lease | foreign descendant/component/protected parent |

authorized target closure必须与每个protected closure不相交；任何intersection、unknown inbound parent、cycle/revisit或ambiguous owner在首写前永久 fail closed。

### 8.2 Sparse topology

| Shape | Required proof | Forbidden inference |
| --- | --- | --- |
| retained-empty frame | exact frame/root/slot bindings与per-Entity lifecycle | frame count == slot count |
| slot-only | owner + parent/outlet/slot identity + exact claim | array position或关联frame宽泛lifecycle |
| top-slot-only | old host version + route slot id + HostLease edge | active frame存在 |
| zero obligation | canonical digest + typed continuation | count为0推导success/terminal |
| partial receipts | exact settled index/disposition + residual classification | 重建manifest或重发receipt |

### 8.3 Drift classification

| Drift | Before first write | After hook-capable write | Outcome |
| --- | --- | --- | --- |
| target missing/exact absent | classify `AlreadyAbsent` only for exact generation | refreeze registry absence | receipt once |
| target foreign component | quarantine exact owned ancestor | refreeze after transfer writes | inert lease；不触发foreign hook |
| target protected parent | quarantine | refreeze edge remains protected | 不移除relationship |
| protected closure drift | zero write | stop remaining writes | whole owner typed inert |
| host candidate incarnation drift | zero write | stop before next write | stale/mismatch owner returned |
| target/protected cycle | visited exact-Entity detection | stop | PendingRejected或AuthorityLost by role |
| receipt replay/wrong batch | no physical/Core write | n/a | duplicate/stale/invalid |

## 9 Terminal 与观察面

| Case | Active | History | Request terminal | Availability | Outstanding owners |
| --- | --- | --- | --- | --- | --- |
| error recovery success | Error | old | Failed(original) | Ready | none for request |
| error recovery failure | old/None | old | Failed(RecoveryFailed) | Ready | none for attempts |
| pre-commit host loss | None | old | Failed(HostLost) | Unavailable(HostLost) | quarantine lease only if needed |
| post-commit host loss | None | committed | Succeeded + severe | Unavailable(HostLost) | quarantine lease only if needed |
| invalid host topology | None | preserved | no request | Unavailable(InvalidHostUiTopology) | no admission owner |
| re-admitted host | None until explicit command | preserved | no implicit request | Ready | one new HostLease |
| explicit restore replace | Route | current replaced canonically | Succeeded | Ready | none for request |
| sealed stale-output cleanup | unchanged current/new | unchanged | current request unchanged | unchanged | only independent old-output owner closes |

所有 request terminal 都要求：trace最后为 `RequestTerminal`，foreground cleanup已完成，同scope无 outstanding action/ticket/capability/batch/ledger/lifecycle owner。quarantine lease若存在，必须有独立 lease identity且不属于 terminal request。

## 10 Machine-readable Recovery profile

### 10.1 Public contract and stage exclusions

Recovery 开始时从 completed History profile继承：

| Kind | Canonical path or normalized item | First slice |
| --- | --- | --- |
| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineLeaseId` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerKind` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputModality` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputProbes` | Authority closure |
| associated | `bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines` | Authority closure |

R1只有在真实 public RED/GREEN、facade consumer、Journey D与review PASS后删除两个 Recovery-owned rows。Recovery completion时本表只剩 Authority closure rows。parser必须拒绝空表、重复path、未知kind/slice与非canonical path。

### 10.2 Contract delta activation

| Item | Owning slice | State | Claim boundary |
| --- | --- | --- | --- |
| boundary-bound error presentation complete evidence | R1 Error presentation | active | Journey D public contract PASS |
| post-commit compound settlement complete evidence | R2 Compound settlement | active | Journey G post-commit foreground rows PASS |
| host re-admission fresh-epoch complete evidence | R3 Re-admission/stale output | planned | Journey G foreground re-admission rows PASS |

completed History的以下active rows必须作为不可删除的历史union输入：

- `NavigationRequestSnapshot::requested -> Option`
- `NavigationEnqueueErrorKind::IdentityExhausted`
- `late-bound back complete evidence`

Recovery profile不是替换History rows。全局active delta集合在Recovery开始为History 3，Recovery completion为 `History active 3 ∪ Recovery active 3 = 6`。`State`只允许 `planned | active`；parser拒绝duplicate/empty、未知owner/state、非canonical evidence label。每个Recovery item只能由owning slice的真实RED/GREEN、public evidence、independent review和分层验证激活。

### 10.3 Recovery authority regression additions

| Test function | Owning slice | State |
| --- | --- | --- |
| `error_outlet_role_classification_is_preserved` | R1 Error presentation | active |
| `post_commit_host_loss_preserves_committed_history` | R2 Compound settlement | active |
| `host_reacceptance_detaches_old_top_slot` | R3 Re-admission/stale output | planned |

激活row与既有 Root 20 + Nested 3 + History 1形成唯一 union；Recovery completion catalogue为27项。source catalogue、direct `#[test]`、binary registration与实际passed集合必须双向相等且无ignored/extra/duplicate。

本表parser固定 `State = planned | active`，owning slice只允许 `R1 Error presentation | R2 Compound settlement | R3 Re-admission/stale output`；test function必须属于最终MVP catalogue vocabulary。empty table、空/重复函数、unknown state/slice/name、非direct test或额外cfg/ignore一律拒绝，不能把unknown row静默当作inactive。

Recovery不激活 `multi_router_registries_are_partitioned`、`old_quarantine_lease_cannot_mutate_new_epoch`、`quarantine_reaper_round_robin_is_fair` 或 `registry_invariant_violation_is_fail_closed`；它们属于 Authority closure。R3只证明旧前台owner/artifact不改写new epoch，不声称后台lease/reaper完整PASS。

## 11 Canonical amendments 与 profile activation

本文独立review PASS、用户确认written spec、implementation plan独立review PASS后，必须先做一个无runtime语义的canonical/profile activation commit：

- `docs/README.md` / `docs/superpowers/README.md`
  - 登记本文为Recovery唯一active stage profile；History spec保持completed historical evidence。
- `docs/target-architecture.md` / `docs/authority-transition-matrix.md`
  - 把post-commit transfer明确为原`L` + disjoint authority `A` + retired-host edge `H`的single compound owner/completion。
  - 明确source-bound unavailable admission permit、callback前最后fallible step与publication-last。
- `docs/mvp-acceptance-journeys.md` / `docs/roadmap.md`
  - 不改变Journey G最终语义，只标明Recovery关闭foreground/readmission rows；actual old-lease reap/new-epoch隔离仍由Authority closure拥有，完整Journey G PASS延后。
- facade/profile harness
  - 从本文10.1读取当前exclusions；从completed History 10.2与本文10.2构造exact union，不丢History active rows。
- authority catalogue harness
  - 以既有24项与本文10.3 active rows做exact union；planned rows不注册、不执行。

activation commit必须由facade/profile parser、catalogue parser与canonical factual review证明GREEN；它只授权后续runtime TDD，不得把planned row改为active或写Recovery PASS。

## 12 Test seam 与 TDD 纪律

- 每个行为切片先写可编译的真实 RED；首个失败必须位于缺失生产语义，不以source string、未定义symbol或人为panic代替动态证据。
- public fault seam只改变production predicate输入：driver plan/report、World host registration/entity/topology、schedule timing或observer/hook drift。
- seam不得直接注入 `PendingRejected`、`AuthorityLost`、terminal、receipt、classification、diagnostic或Core completion。
- private test可duplicate opaque artifact用于stale/replay，但必须送入真实consumer并断言current/new epoch零写。
- characterization GREEN与新 RED分开命名；新行为只由owning test/filter激活。
- 任一 RED需要扩大 public API、stage scope、owner model或canonical语义时，先修订本规范并复审，不在GREEN中偷渡。

## 13 Verification 与 review gates

每片按以下层级，只由唯一 Cargo/rustc负责人执行：

1. 单一命名 RED/GREEN与source registration证明。
2. 受影响 Core/ECS/facade crate tests。
3. owning Journey D或Journey G Recovery-owned固定public filters，并重跑受影响且已闭合的最高层public/facade gate。
4. facade profile/compile-fail与authority catalogue三重验证。
5. 片级 `cargo check`、`cargo fmt --all -- --check`、`git diff --check`。

受影响既有public gates固定为：

| Slice | Must rerun before slice commit |
| --- | --- |
| R1 | existing root match/build/apply/fallback journeys；History C2/C3 first-failure与replace/back failure journeys；facade install consumers |
| R2 | existing pre-commit authority/History failure journeys；post-commit cleanup wait/drift public/private gates；enqueue availability precedence |
| R3 | host admission/reconciliation/readmission regressions；History stale/replay and no-replay gates；facade availability/enqueue consumers |

R4在同一 committed candidate tree执行：

- facade完整profile与compile-fail；
- `cargo nextest run --locked --workspace --all-features --config "build.rustc-wrapper = ''" -j 1`；
- workspace doctests；
- workspace all-targets/all-features check；
- authority catalogue source/binary/runtime三重gate，期望27/27/27；
- Journey D complete、Journey G Recovery-owned foreground/readmission rows、Scenario E与所有受影响已闭合最高层public journeys；
- `cargo fmt --all -- --check`；
- `git diff --check` 与 `git diff --cached --check`；
- `git diff --exit-code` 与 `git diff --cached --exit-code`证明tracked worktree/index clean；
- `git ls-files --error-unmatch` 精确证明Recovery spec、plan、profile/canonical docs、public journeys与authority catalogue artifacts已跟踪；
- 对上述固定artifacts执行全文件trailing-whitespace scan并要求零命中；
- `git status --short`逐项证明剩余untracked均为用户WIP且未被stage。

每片至少做三角独立只读审查：

- Core lifecycle/commit boundary/terminal owner；
- ECS exact Entity/allowed writes/ledger/receipt；
- public facade/canonical journey/machine profile。

反馈按权威顺序、证据、scope与ROI裁决；不盲目采纳。Critical/Important关闭后停止扩张审查，Minor仅在低风险且不扩大语义时同片修正。

## 14 Implementation 与提交纪律

- Root是唯一 Cargo/rustc、index与commit负责人；子代理可承担明确文件写入或只读审查。
- 同一时刻每个文件只有一个writer；共享runtime不并行修改。
- `git add`只使用精确文件列表；禁止 `git add .`、`git add -A`。
- 提交前核对staged文件；用户未跟踪 `review-history-*` 永不暂存、删除或改写。
- 每个semantic slice独立提交；机械重构若必要，先单独证明ROI、单独审查/验证/提交。
- 未看到实际成功输出不得写PASS或更新事实文档。

## 15 Recovery completion definition

Recovery PASS仅在同一 verified committed tree同时满足：

- R1–R3全部真实 RED/GREEN、三角review无剩余 Critical/Important；
- Journey D完整矩阵与Journey G Recovery-owned foreground/readmission rows通过，Scenario E stale-output cleanup闭合；host/app-owned fingerprint与terminal owner closure成立；不声明actual old-lease reap或完整Journey G PASS；
- 两个 Recovery public exclusions已删除，其余Authority exclusions原样保留；
- History active 3保持、Recovery新增3个contract delta active、三个Recovery authority rows active，catalogue精确27项；
- full Recovery gate实际成功，命令输出与run identity写入completion record；
- current architecture、roadmap、README、Authority Matrix与SDD progress只同步已验证事实；
- tracked tree/index clean，用户WIP保持未触碰。

完成后本文转为 completed historical evidence，roadmap进入 Authority closure；不得写 `MVP PASS`或完整Journey G PASS。Authority closure必须另建并独立复审spec/plan与完整矩阵，完成old-lease reap/new-epoch隔离等剩余rows后才能声明Journey G完整；最终MVP gate只能在Recovery与Authority都完成的同一verified tree上执行。
