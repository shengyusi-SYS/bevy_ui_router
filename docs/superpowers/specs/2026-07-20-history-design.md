# History 阶段设计

**用户设计确认**: 已确认

**Spec independent review**: PASS，Core lifecycle、canonical boundary、stage profile 三路复审关闭全部 Critical/Important

**Plan independent review**: 尚未开始

**Runtime gate**: Spec 与 Plan 必须分别 PASS 后才能修改 runtime

**事实基线**: Reconciliation PASS，`2fa5332`

**阶段顺序**: History → Recovery → Authority closure → MVP gate

**规范关系**: 本文记录已确认的 History 决策，并在 canonical README 完成显式登记后兼任阶段 profile，实施前仍须按文档权威顺序修订 Target、Authority Matrix、Facade Fixture、golden 与 journeys

## 目标

- 从推荐 facade 接通 `replace` 与 late-bound relative `back`
- 保持 Core 为唯一 history 与 request authority、唯一逻辑 publisher
- 让 presentation 与 history 只在同一个不可失败 atomic publication tail 中发布
- 让失败、zero cleanup、authority loss、stale replay 都保持 exact owner 与有界终态
- 用完整但可审查的语义矩阵指导垂直切片，不展开无意义的笛卡尔积

## 非目标

- 不实现 call-time exact-entry `traverseTo`
- 不增加公开 command、entry id、revision、capability 或 authority API
- 不把 `MemoryHistory`、`BackCapabilitySnapshot` 或 platform DTO 接入 Router authority
- 不完成 post-commit compound settlement、host re-admission、new epoch rebuild 或 background reaper
- 不交付 `RouterInputProbes`、multi-router stress、Authority closure 或 MVP gate
- 不引入第二份路线图、重复 contract profile 或 speculative history adapter

## 1 Authority 与模块边界

| 模块 | 唯一职责 | 明确禁止 |
| --- | --- | --- |
| Core `RouterEngine` | command、request、canonical history、history identity、intent、capability、presentation、terminal 与 diagnostics | 读取 World、持有 Entity manifest、接收 snapshot 重建 owner |
| ECS | World preflight、role partition、allowed write set、exact Entity manifest、cleanup attempt、ledger 与 receipt | 解释 intent 后自行发布 history、维护第二 cursor、从 snapshot 取得 authority |
| Facade | `RouterNavigation::{push,replace,back}` 与只读 `RouterSnapshots` | 伪造 request、entry、revision、owner 或 publication capability |
| View | 根据 sealed task 产生受限 content plan/effect | 写 history、registry、active presentation 或 route lifecycle |
| Adapter/compatibility DTO | router-neutral 数据转换 | 接入 RouterEngine 或成为 admission authority |

现有公开 `MemoryHistory` 与 compatibility types 继续是 detached values

ECS 可以缓存 Core 在 `PublishSnapshots` 产生的只读 admission stamp，但该缓存只支持同步快速拒绝

## 2 私有 history identity 与 capability

### 2.1 Identity

- 每个 RouterEngine history 具有不可复用的 incarnation
- 每个 entry 具有不可复用的私有 identity
- `push` 与 `replace` 创建的新 entry identity 由 history incarnation 与 creator request identity 封存
- `back` 只移动 cursor，不创建 entry identity
- 初始 revision 使用 incarnation-bound sentinel
- 每次成功 `push`、`replace` 或 `back` 都把 revision 更新为该 publication request identity
- 等价 `replace` 与等价 route publication 同样推进 revision，避免内容恢复相同后的 ABA
- fallback、failure 与 background disposition 不更新 revision

相同 `RouteLocation` 可以出现在多个 entry，location 不能代替 entry identity

### 2.2 Source 与 destination

`push` 与 `replace` 在 enqueue 时冻结 canonical target，在成为 running 时冻结 exact history source

`back` 在 enqueue 时只保存 relative intent 与 admission evidence，在成为 running 时从 Core live history 解析 destination

exact source 至少绑定

- router 与 history incarnation
- history revision
- current entry identity 与 cursor
- ordered entry projection
- request、normal attempt 与 transaction

resolved-target back 另外绑定 destination entry identity、cursor 与 canonical location

`PreparedPublication` 前从当前 Core authority 重建完整 source 并要求 exact equality

永久 drift fail closed，不 retry、不静默 retarget

### 2.3 Linear capability

history publication capability 为私有、non-Clone、single-use

它绑定 router、request、normal attempt、transaction、intent、exact source、destination 与 prepared presentation

状态只允许

```text
unissued -> frozen -> PreparedPublication -> consumed
                  \-> cancelled
```

- unresolved back 不签发 capability
- resolved-target back 在 source/destination 冻结后签发
- resolved-no-target back 永不签发 capability
- fallback attempt 永不持有原 history capability
- snapshot、fingerprint、cursor 或 location 都不能重建 capability

## 3 公共合同

### 3.1 Navigation request projection

`NavigationRequestSnapshot::requested()` 改为

```rust
pub fn requested(&self) -> Option<&RouteLocation>
```

| Request | queued | running after resolution | terminal |
| --- | --- | --- | --- |
| push | `Some(canonical target)` | unchanged | unchanged |
| replace | `Some(canonical target)` | unchanged | unchanged |
| back with target | `None` | `Some(resolved canonical target)` before `Matching` | unchanged |
| back without target | `None` | normally not published as an intermediate snapshot | `None + BackTargetUnavailable` |

`RouterSnapshot::requested()` 保持现有 optional 签名，只镜像当前 running request 的 resolved target

按 request 查询可以在 queued back 上观察 `requested=None`

正常 no-target back 在同一 Router schedule 内完成 proven-empty settlement，因此不发布 `running_request=Some + requested=None`

只有以下 no-target preparation 窗口可以公开该组合

| Window | Running status | Eventual terminal | Owner rule |
| --- | --- | --- | --- |
| typed retryable preparation shortage | `CleanupPending` | proof成功后`BackTargetUnavailable`，否则按后续exact classification | 原candidate、reservation、evidence与continuation capacity完整返还 |
| proven-empty返回前被既有pre-commit authority settlement接管 | `CleanupPending / Recovering` | settlement后`Failed(HostLost / AuthorityLost)` | 线性转为authority settlement owner |

settlement 完成前仍是 `Running(...)`，表中的 terminal 只是预封存的 eventual cause

authority takeover 窗口无 fallback、无 history 变化、无 `HistoryIntentRejected`

这些例外不能扩展到 resolved-target back、push 或 replace

snapshot 在 enqueue 后、下一次 `PublishSnapshots` 前仍可能返回旧值或 `Unknown`

### 3.2 Enqueue errors

新增

```rust
NavigationEnqueueErrorKind::IdentityExhausted
```

它表示 request、transaction、attempt、cleanup 或 admission reservation 的永久容量耗尽

- 无 request id
- enqueue 对公开 queue/request/World 状态为零变化
- 已消费私有 identity 只烧毁不复用
- 不得映射为 `AuthorityRecovering`
- `back` 必须先检查 `NoBackEntry`，再申请 capacity，因此 `NoBackEntry` 优先
- producer 必须来自真实 allocator/domain predicate，test seam 只能改变该 predicate 的输入事实

## 4 Enqueue 与运行数据流

### 4.1 Reservation

push/replace enqueue 原子预留 request-scoped normal attempt

- request identity
- transaction identity
- cleanup batch identity
- 与 matched depth 对应的 frame/task pool

back enqueue 顺序

1. 校验 router identity，unknown router 返回 `UnknownRouter`
2. 执行既有 availability gate，非 `Ready` 分别返回 `HostUnavailable | AuthorityRecovering`
3. 校验 published admission stamp 的 world、router 与 history incarnation
4. `can_back=false` 时返回 `NoBackEntry`
5. ECS 预留 manifest-neutral `EcsCleanupCapacityReservation`
6. Core 预留 request、transaction、cleanup batch 与 normal branch pool
7. Core 接受 request 后，ECS 以不可失败步骤把 reservation 绑定 exact router incarnation、request、normal attempt 与 batch

`EcsCleanupCapacityReservation` 只预留唯一 identity、domain 与 capacity，不声明 manifest、count 或 zero

Core 失败时 ECS reservation 的 identity 烧毁并释放容量

`NoBackEntry` 只优先于后续 `IdentityExhausted`，不能越过 router identity 或 availability gate

back 的 branch pool 使用 installed immutable route tree 的最大深度

实际 matcher 只消费所需 prefix，未使用 identity 永久烧毁

nested mount sequence 与 slot identity 仍在 drive 阶段分配

永久 mount/slot exhaustion 有界进入 `MountFailed` 与 fallback，不进入 generic retry

fallback 在原失败 cleanup 完成后取得 fresh attempt

fallback reservation exhaustion 在任何 fallback World mutation 前成为 `RecoveryFailed`

### 4.2 Admission stamp

Core 在 `PublishSnapshots` 生成私有只读 stamp

```text
world identity
+ router identity
+ history incarnation
+ published revision
+ can_back
```

ECS 只缓存 latest published stamp

- stamp 可重复读取
- revision 可以落后于 Core live revision
- stamp 不选择 destination
- stamp 不签发 owner 或 capability
- cross-world、cross-router、cross-incarnation replay 零写并拒绝

### 4.3 Running

push/replace 使用 enqueue 时冻结的 canonical target，成为 running 时冻结 exact history source

back 成为 running 时只执行一次 live preview

- 有 target 时先写入 immutable resolved target，再进入 `Matching`
- 无 target 时形成 typed no-target candidate，不签发 history capability
- target 深度超过 reserved maximum 属于内部永久矛盾，零 World 写并 fail closed

### 4.4 Typed preparation outcome

任何 authority/preparation boundary 都必须保留四类 typed outcome

| Outcome | Owner result | World/history result | Next step |
| --- | --- | --- | --- |
| `Ready` | 单次移动到prepared plan | 尚未publication | 进入commit preparation |
| `PendingRejected` | 原owner转为typed pending cleanup owner | old active/history不变 | exact pending cleanup后按continuation继续 |
| `AuthorityLost` | 原owner转为typed authority settlement owner | commit前history不变 | existing scoped settlement |
| `RetryablePreparationFailure` | 原capability/candidate、boundary evidence、reservation与continuation owner完整返还 | 严格零写 | 只由同一named producer重试 |

只有可证明未来能满足的容量或尚未finalized boundary facts可以产生 `RetryablePreparationFailure`

允许的 named producer 必须显式记录为 `RetryableCapacity | UnfinalizedBoundaryFacts`

| Exact evidence predicate | Typed producer | Original owner | Retry trigger |
| --- | --- | --- | --- |
| finite preparation/scratch capacity不足，且evidence绑定同domain的exact live reclaim owner与release generation | `RetryableCapacity` | capability/candidate/reservation/continuation capacity完整返还 | 只有reclaim owner推进或release generation改变后 |
| boundary facts尚未finalize，且evidence绑定同request/attempt/host epoch的exact outstanding action或ticket | `UnfinalizedBoundaryFacts` | capability/candidate/reservation/continuation capacity完整返还 | 只有该action/ticket状态改变后 |
| 无exact reclaim owner、无outstanding ticket、identity/domain永久耗尽或source drift | none，permanent branch classification | 按typed failure/authority transition单次消费 | never |

同一个 case 只能命中一行，capacity 与 boundary evidence 不得合并为通用 retry

永久 identity/domain exhaustion、source drift、owner/seal mismatch 与结构矛盾必须有界 fail closed

retry 前重建完整 source 并要求 exact equality，drift 时重新分类，不能沿用旧 `RetryablePreparationFailure`

History 不新增 generic retry producer

resolved no-target 的 reservation 已覆盖 ledger identity/domain/capacity，但构造 `ProvenEmptyProjection` 的 bounded scratch capacity 或 boundary finalization 仍可返回上述 typed retry

proof 成功后 ledger binding 与 zero completion 不再包含可失败 allocation

### 4.5 Publication

所有 rejectable work 在 `PreparedPublication` 前完成

```text
exact history refreeze
-> exact physical preflight
-> prepared in-place mutation plan
-> PreparedPublication
-> exclusive ECS mutation + Core presentation/history/trace publication
-> PostCommitCleanup
```

atomic tail 不包含 allocation、query、validation、insert/remove/despawn、relationship write、hook/observer-triggering write 或 fallible operation

history mutation

| Intent | Successful mutation |
| --- | --- |
| push | truncate forward suffix，append new entry，cursor 指向新尾项 |
| replace on empty | 创建首 entry，cursor 为 0 |
| replace on nonempty | 替换 current entry，保留 forward suffix |
| back | cursor 移到 frozen destination，entries 不变 |

normal route presentation、history、revision 与 `CommitPublished` 在同一 tail 可见

## 5 Failure、cleanup 与 authority

### 5.1 Outcome precedence

1. CommitPublished 前 host、current、preserved 或 leaving authority loss 最高优先
2. authority valid 时才区分 entering/pending rejection、route failure 与 no-target back
3. CommitPublished 后 history capability 已消费，后续异常不能回滚 history

foreign descendant/component 与 protected parent 按 exact role 分类，不能因存在 drift 就宽泛升级为 authority loss

### 5.2 Route failure 与 fallback

resolved target 的 match、build、apply、mount 或 pending-only failure

```text
cancel history capability
-> HistoryIntentRejected
-> failure cleanup
-> exact completion
-> fresh fallback attempt
```

- original history 保持
- failure cleanup manifest 与 original failure continuation 独立冻结
- fallback 不持有原 history capability
- fallback 成功只发布 Error presentation，请求仍以 original cause `Failed`
- fallback 失败在自身 cleanup 后成为 `Failed(RecoveryFailed)`
- fallback 两种结果都不改变 history

### 5.3 Back no-target

ECS 必须产生 typed `ProvenEmptyProjection`

它至少来自

- exact host 与 active claims
- request-attempt owner inventory
- registry exact mappings
- 完整 observed relationship closure
- explicit role partition
- 所有 existing current 明确归入 protected
- attempt target 为空且与 protected 不相交

logical roots 为空不能推导 Entity obligations 为零

retained-empty、slot-only 或其他稀疏 topology 也不能绕过 proven-empty

只有 proven-empty 且 authority valid 后，才允许不可失败配对转换

```text
NoTargetCandidate + EcsCleanupCapacityReservation
-> RejectedBackContinuation + canonical-empty Entity manifest/ledger
-> HistoryIntentRejected
-> CleanupPending
-> exact zero completion
-> CleanupSettled
-> Failed(BackTargetUnavailable)
```

不得直接使用无证明的 `FrozenCleanupManifest::empty()`，不得伪造 `FrameTaskIdentity`

若 proven-empty 前发现 pre-commit authority loss，消费 no-target candidate、normal attempt 与 capacity reservation，转入既有 scoped authority settlement，不记录 `HistoryIntentRejected`

### 5.4 Cleanup authority layering

| Fact | Authority |
| --- | --- |
| cleanup batch identity 与 logical root expectations | Core |
| exact Entity obligation identity manifest | ECS |
| manifest digest/count | ECS 计算并提交，Core 只做 token correlation |
| attempt revision、World evidence、physical plan、disposition | ECS |
| terminal receipts 与 completion digest | ECS |
| post-cleanup continuation 与 request terminal | Core |

exact Entity manifest 在 batch 内不可变

attempt supersede 只更新未结算 obligation 的 World evidence、plan 与 disposition

已结算 receipt 不重复，旧 attempt 不能继续写或结算新 attempt

Core completion 只验证

- current outstanding exact token
- batch、request、transaction 与 host epoch binding
- settled count
- canonical zero representation
- completion structural binding

Core 不重算 Entity truth，也不认证 ECS receipt 的物理真实性

cleanup target/manifest 与 continuation 分别线性持有

continuation 不能由 preserved active、logical root count、Entity count、empty manifest 或 disposition 反推

quarantine receipt 可以结束 foreground obligation

background lease 只能持有自身 owner，不能写 terminal request、presentation 或 history

### 5.5 Authority takeover 与 post-commit boundary

pre-commit authority loss 使用既有 scoped settlement

- 取消或消费 outstanding history owner
- history 不推进
- owner 线性移交
- settlement completion 后 request `Failed(HostLost | AuthorityLost)`

History 不实现 re-admission、new epoch、active rebuild 或 post-commit ledger transfer

后续 Recovery 必须遵守

- ordinary post-commit physical drift 先在原 immutable ledger 内 supersede attempt
- 只有 authority loss 或无法以正常 receipt/quarantine 收敛的异常才进入 authority settlement
- 原 batch、manifest、token 与 settled receipts 不扩展、不复制
- 新 authority-invalid targets 使用独立 settlement batch
- compound continuation 等待原 residual 与新 settlement batch
- committed presentation/history 不回滚
- terminal 为 `Succeeded + Severe(PostCommitCleanupAnomaly)`

这些规则在 History 冻结，但不计入 History PASS

### 5.6 Stale 与 replay

- 只有持有独立有效 sealed old-output owner 的 stale report 可以触发其自身旧 pending output cleanup
- stale、duplicate 或 foreign receipt/token 严格零写，不创建 cleanup、不改变 ledger、不取得 owner
- stale admission、source、capability 或 publication replay 不改变 current request、presentation、history 或 World

### 5.7 Terminal

- 持有 foreground batch 的 running scope 必须等待 exact completion
- terminal 后该 scope 不再持有 action、ticket、capability、continuation 或 lifecycle owner
- queued authority failure 可以没有 request-specific ledger
- `IdentityExhausted` 没有 request id，不属于 request terminal
- History 只保留 `RetryableCapacity | UnfinalizedBoundaryFacts` 两类 named preparation producer，不新增 generic retry

## 6 Allowed write set

| Phase | Authorized target | Allowed writes | Protected |
| --- | --- | --- | --- |
| enqueue/admission | Core request records、ECS reservation registry | logical queue/record 与 reservation state | World UI、active/history publication |
| staging | exact entering/pending set | staging registry、inert content 与 relationship writes，写后重建 partition | current preserved、app-owned、foreign、other authority |
| Ready tail | precomputed router-private lifecycle/registry/gate 与预建 `Visibility/Pickable` values | 不可失败原位 mutation + Core publication | allowed set 外全部 Entity/component/relationship |
| failure cleanup | exact pending/entering obligations | isolate、focus release、cleanup/quarantine 与 receipt | preserved/current/app-owned/foreign non-target |
| pre-commit authority settlement | role-classified authority-invalid current/pending target与已有 leaving residual | exact isolation、focus release、cleanup/quarantine | non-target current、app-owned、foreign、other authority |
| post-commit cleanup | immutable leaving obligations | versioned attempt 的 exact physical plan | new active、app-owned、foreign non-target |
| background reaper | lease immutable exact-owned set | inert barrier、safe reap 与 lease diagnostics | terminal request/history、新 epoch registry、foreign/protected |

app-owned host 除 HostLease 授权的 exact top-slot `Children` edge detach 副作用外全部 component 与 relationship 零写

任何可能触发 hook/observer 的 staging、cleanup 或 recovery write 后都必须重建 target/protected partition

## 7 Complete semantic matrix

### 7.0 Combination rule 与 case schema

每个可执行 case 先规范化为以下 tuple

```text
command
x history shape
x current presentation
x topology
x exact Entity role/evidence closure
x drift timing
x branch/attempt
x history capability state
x cleanup protocol state
x host epoch
```

各轴按以下优先级组合，不按表格出现顺序任选结果

1. 拒绝 stale、foreign、duplicate 或 cross-incarnation evidence，当前状态零写
2. 校验 router identity、availability 与 host epoch
3. 独立冻结 current 与 next，`next=None` 只校验且不进入 mutation plan
4. 在 CommitPublished 前先合并 host/current/preserved/leaving authority classification
5. current authority exact 后才处理 entering `PendingRejected`、no-target 与 route failure
6. `RetryablePreparationFailure` 返还完整原 owner/evidence，不产生 mutation 或 continuation 转换
7. `Ready` 后 exact refreeze source、target/protected partition与allowed write set
8. CommitPublished 分隔 precommit failure 与 postcommit no-rollback
9. immutable manifest/receipt completion 后才消费独立 continuation 或进入 terminal

每个 matrix case 必须能唯一导出以下字段

| Required field | Meaning |
| --- | --- |
| case key / stage owner | semantic equivalence class与当前完成门禁 |
| current / next | 独立冻结的logical projection，明确`None`或`Some` |
| exact owner | request、attempt、capability、batch、continuation与host epoch |
| target / protected | exact Entity role partition与disjoint proof |
| allowed writes | phase-local mutation与exact focus set |
| active/history before-after | 分别记录precommit、published与settled状态 |
| cleanup | logical roots、immutable Entity manifest、count与attempt |
| continuation | 与manifest正交的branch/cause owner |
| completion | receipt、terminal与replay结果 |
| retry | 唯一named producer或`none`，以及原owner返还 |

下列表格是上述 schema 的正交 classifier

若组合后仍可能导出两个结果，该 case 不具备 implementation-ready 条件

### 7.1 Command 与 history shape

| Command | History before | Resolved destination | Commit result | Failure result |
| --- | --- | --- | --- | --- |
| push | empty | new target | first entry | unchanged empty |
| push | cursor at middle | new target | truncate forward + append | entries/cursor unchanged |
| push | equivalent target at tail | new equivalent entry | physical zero，append entry，revision advances | unchanged |
| push | equivalent target at middle | new equivalent entry | physical zero，仍truncate forward + append，revision advances | unchanged |
| replace | empty | new target | first entry | unchanged empty |
| replace | cursor at middle | new target | replace current，forward preserved | entries/cursor unchanged |
| replace | equivalent target | equivalent replacement entry | physical zero，length/cursor unchanged，revision advances | unchanged |
| replace | fragment-only | remount deepest normal leaf | cursor entry replaced after commit | unchanged |
| back | `can_back=false` at published admission | none | no request，`NoBackEntry` | no state change |
| back | target exists at running | previous exact entry | cursor moves only after UI commit | cursor unchanged |
| back | target absent at running | none | not applicable | proven-empty 后 `BackTargetUnavailable` |
| back from Error presentation | published history decides | previous exact entry | normal Route commit，cursor moves | Error or old presentation follows normal failure rules，cursor unchanged |
| any command with current `None` | command规则决定 | normal Route target | first physical presentation，history按intent更新 | `None`保持或fallback Error |
| push/replace/back with current Error | history与intent决定 | normal Route target | Error不享受equivalent physical shortcut | failure按原Error/old state规则收敛 |
| lifecycle validation with `next=None` | unchanged | none | 无presentation/registry/cleanup/history physical mutation | unchanged |

`next=None` 与 `next=Some(equivalent route)` 是不同 case

### 7.2 Lifecycle outcome

| Timing/outcome | History capability | Cleanup owner | Continuation | Settled active/history | Terminal | Retry producer | Stage owner |
| --- | --- | --- | --- | --- | --- | --- | --- |
| enqueue rejection | unissued | none | none | unchanged | no request | none | History |
| resolved target normal commit | consumed in publication | postcommit leaving batch | success | new Route/new history | cleanup后Succeeded | none after PreparedPublication | History |
| equivalent push | consumed | proven physical-zero batch | success | same physical active/appended history | Succeeded | none | History |
| equivalent replace | consumed | proven physical-zero batch | success | same physical active/replaced history | Succeeded | none | History |
| no-target back | never issued | proven canonical-zero batch | rejected-back | unchanged | BackTargetUnavailable | §4.4 exact evidence mapping或none | History |
| route/pending failure | cancelled | pending/entering batch | original failure | old active/history | fallback后Failed | §4.4 exact evidence mapping或none | History |
| fallback success | none | fallback leaving batch | original failure | Error/old history | original cause Failed | §4.4 exact evidence mapping或none | History |
| fallback failure | none | fallback residual batch | recovery failure | old presentation/history | RecoveryFailed | §4.4 exact evidence mapping或none | History |
| precommit authority loss | cancelled or unresolved owner consumed | existing scoped authority settlement | authority failure | settlement completion后active `None`/history old | HostLost/AuthorityLost | §4.4 exact evidence mapping或none | History invariant，existing settlement |
| typed preparation shortage | original owner returned | no new ledger | unchanged | unchanged | still Running | `RetryableCapacity / UnfinalizedBoundaryFacts` | History/existing protocol |
| ordinary postcommit drift | already consumed | same immutable ledger/new attempt | success | committed state retained | normal receipt后Succeeded | none，attempt supersede不是request retry | Reconciliation/History invariant |
| postcommit authority loss | already consumed | old residual + new settlement batch | compound settlement | settlement completion后active `None`/history committed | Succeeded + severe | Recovery-defined | Recovery |
| stale old-output report | current capability unchanged | independent old-output owner | none for current | current unchanged | current unchanged | none | History/Authority closure |
| stale receipt/token replay | unchanged | none | none | unchanged | unchanged | none | History |
| background lease | none | lease owner only | none | terminal/history unchanged | no rewrite | reaper-owned，非request retry | Authority closure |

### 7.3 Role 与 physical evidence

| Role | Exact | Foreign descendant/component | Missing/owner/seal mismatch/cycle | Protected parent/other authority |
| --- | --- | --- | --- | --- |
| entering/pending | Ready | PendingRejected + pending quarantine | PendingRejected | PendingRejected，protected零写 |
| preserved current | Ready | preserve foreign，target零写 | AuthorityLost | AuthorityLost |
| leaving current | Ready + normal cleanup | commit后foreground quarantine | AuthorityLost | 只isolate exact Router-owned leaving target并为owned residual产生`QuarantinedProtectedParent`，protected parent与edge零写 |
| app-owned host | protected authority anchor | foreign state保持protected | missing为HostLost，其他host contradiction按authority分类 | host component零写，只允许HostLease授权的exact top-slot edge effect |
| protected current top slot | Ready，非attempt obligation | protected，当前case零写 | AuthorityLost | protected，除独立settlement外零写 |
| leaving/authority-invalid/pending-cleanup top slot | exact slot obligation | exact owned slot quarantine | AuthorityLost | isolate exact owned slot并产生`QuarantinedProtectedParent`，parent/edge零写 |
| protected current child slot | Ready，非attempt obligation | preserve foreign，当前case零写 | AuthorityLost | protected，当前case零写 |
| leaving/authority-invalid/pending-cleanup child slot | exact slot obligation | exact owned slot quarantine | AuthorityLost | isolate exact owned slot并产生`QuarantinedProtectedParent`，parent/edge零写 |
| child mount under entering | Ready | PendingRejected | PendingRejected | PendingRejected |
| child mount under preserved/current | Ready | entering child独立分类 | ParentAuthorityLost | AuthorityLost |
| app-owned/foreign/non-target current | protected | protected | protected或独立authority evidence | zero write |

mixed drift 先逐 source 分类再合并，任何真实 current authority loss 不得被 entering `PendingRejected` 覆盖

current 与 next 分别冻结和验证

retained-empty、slot-only 与其他 sparse topology 必须按每个 Entity 自身的 parent、outlet、slot identity、mount binding、owner、seal 与 lifecycle role 分类

不得从 frame/slot 数量、数组位置或关联 frame/slot 的 lifecycle 并集推导 authority

### 7.3.1 Sparse slot obligation derivation

| Exact role + lifecycle/binding | Owner | Entity obligation | Protected-parent result | Continuation | Receipt/terminal |
| --- | --- | --- | --- | --- | --- |
| app-owned host | HostLease只授权edge effect，不取得host ownership | never | host/other components与未授权relationship零写 | host/authority cause独立冻结 | host永不作为cleanup receipt target |
| protected current/retained top slot | exact router/host epoch/top-slot owner+seal | 当前attempt为零，slot明确进入protected | 当前case parent/edge与slot零写 | none | 不创建该attempt receipt |
| leaving/authority-invalid/pending-cleanup top slot | exact router/host epoch/top-slot owner+seal+frozen lifecycle role | exact slot identity，即使没有frame仍为非零obligation | isolate owned slot，`QuarantinedProtectedParent` lease持有owned residual，parent/edge零写 | normal或authority continuation与manifest正交 | `Removed / AlreadyAbsent / QuarantinedProtectedParent`后才允许foreground continuation |
| protected current/retained child slot | exact parent frame/outlet/mount binding+slot owner+seal | 当前attempt为零，即使parent retained-empty也明确进入protected | 当前case parent/edge与slot零写 | none | 不创建该attempt receipt |
| leaving/authority-invalid/pending-cleanup child slot | exact parent frame/outlet/mount binding+slot owner+seal+frozen lifecycle role | exact slot identity，即使没有frame root仍为非零obligation | isolate owned slot，`QuarantinedProtectedParent` lease持有owned residual，parent/edge零写 | normal/failure/authority cause独立冻结 | exact receipt后才允许request/settlement terminal |

slot-only 或 top-slot-only 不能只因 Entity 类型或 logical frame roots 为空决定 obligation count

no-target back 的 current/retained slot 必须进入protected并保持零 attempt obligation；只有 exact frozen lifecycle role 为 leaving、authority-invalid 或 pending-cleanup 时才成为authorized target

### 7.4 Protocol state

| Owner/evidence | Outstanding | Superseded/cancelled | Terminal/replay rule |
| --- | --- | --- | --- |
| unresolved back owner | queued/running before resolution | target后转capability，no-target后转candidate，authority loss后线性转settlement | 不可复制或从stamp重建 |
| no-target candidate | resolved no-target，等待proven-empty | authority loss后线性转settlement，retry时原样返还 | 只可转RejectedBackContinuation或authority settlement |
| rejected-back continuation | proven-empty后与zero ledger正交持有 | 不因count/disposition重分类 | exact zero completion后单次消费 |
| history capability | frozen exact source/destination | precommit failure cancels | PreparedPublication单次消费，replay零写 |
| cleanup capacity reservation | manifest-neutral | target exists则unused identity烧毁，authority loss则线性消费 | 只由proven manifest建立ledger |
| cleanup batch/token | exact request/transaction/epoch binding | owner可转交，identity不变 | completion replay零写 |
| Entity manifest | immutable obligation identities | 不可扩展或替换 | completion由exact receipts证明 |
| cleanup attempt | current revision plan | drift后superseded | 旧attempt不能写或settle新attempt |
| receipt | obligation exact-once | duplicate/stale/foreign拒绝 | 已settled不可重复 |
| normal/failure continuation | 独立冻结branch/cause | 只可类型化单次转换 | 只由exact completion消费 |
| fallback attempt/continuation | fresh owner，无history capability | failure后转fallback cleanup | original cause或RecoveryFailed只在completion后terminal |
| authority settlement continuation | exact precommit authority cause与owner | 不得从pending/no-target反推 | settlement completion后单次terminal |
| retryable preparation evidence | exact source、expectation、observed shortage与原owner | source drift后失效并重分类 | 仅named producer可重试，不能创建ledger/continuation |
| quarantine lease | foreground transfer后独立owner | operation revision可推进 | 不回写request/history |
| host epoch | current | settling后旧epoch stale | stale epoch owner不能影响新epoch |

### 7.5 Branch、attempt 与 host epoch

| Branch/attempt | Primary owner | History capability | Cleanup/continuation | Host epoch rule | Retry producer |
| --- | --- | --- | --- | --- | --- |
| normal push/replace | normal attempt | frozen after source freeze | normal success/failure | current exact | §4.4 exact evidence mapping或none |
| resolved-target back | normal back attempt | frozen after destination freeze | normal success/failure | current exact | §4.4 exact evidence mapping或none |
| resolved-no-target back | no-target candidate | never issued | proven-zero + rejected-back | current exact or settling takeover | §4.4 exact evidence mapping或none |
| fallback | fresh fallback attempt | none | fallback cleanup + original failure | current exact | §4.4 exact evidence mapping或none |
| precommit authority settlement | authority settlement owner | cancelled/unissued | authority continuation | settling | §4.4 exact evidence mapping或none |
| postcommit transfer | compound owner | already consumed | immutable old residual + separate new batch | settling | Recovery-defined |
| background reaper | quarantine lease | none | no request continuation | current或stale former epoch | reaper-owned only |

host epoch 为 `stale` 时只能处理其自身 sealed residual/lease，不能取得 current epoch registry、request 或 history owner

## 8 Stage ownership

| Capability | History gate | Later owner |
| --- | --- | --- |
| replace/back facade、late-bound target、atomic history commit | required | none |
| C1–C3、F1–F3 与完整 existing public journey regression | required | MVP重跑 |
| precommit history cancellation、zero settlement、fallback history invariant | required | Recovery扩展presentation policy |
| source ABA、cross-incarnation replay、terminal owner closure | required | Authority closure压力测试 |
| postcommit no-rollback与无history写口 | invariant only | Recovery实现compound settlement |
| host re-admission、new epoch、explicit replace rebuild | excluded | Recovery |
| background reaper、multi-router、opaque input probes | excluded | Authority closure |
| unified facade/authority/feature/workspace acceptance | excluded | MVP gate |

## 9 Vertical slices

| Slice | Production owner | Public evidence | Exclusions released | PASS boundary |
| --- | --- | --- | --- | --- |
| Replace | shared history capability/revision/refreeze、exact `IdentityExhausted` + replace flow | real facade replace、F1–F3、nested/retained-empty、allocator-derived exhaustion、failure/history unchanged | `RouterNavigation::replace` | replace矩阵与shared protocol绿，不宣称C1或History PASS |
| Back execution | admission stamp、capacity reservation、late target、typed retry、success与pure no-target | C1、no-target zero、slot-only与retained-empty protected proof、NoBackEntry precedence | `RouterNavigation::back` | public back可用，late-bound完整证据仍待C3两分支 |
| Back failure | C2/C3、precommit authority invariant、stale/replay与terminal closure | C2、C3 first-success/first-failure、role precedence、authority catalogue addition | none | 全部History-owned生产语义闭合 |
| Completion gate | no new production semantics | full fixed journeys、authority/facade/workspace gates与事实复审 | History stage marker | 新RED必须回开owning slice |

每片按同一 approved spec/plan 做独立 implementation 与 quality review，不新增逐片 spec

## 10 Machine-readable stage profile

### 10.1 Public contract and stage exclusions

History 开始时保留以下 exclusions

| Kind | Canonical path or normalized item | First slice |
| --- | --- | --- |
| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineLeaseId` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerKind` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputModality` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputProbes` | Authority closure |
| associated | `bevy_ui_router::ecs::RouterNavigation::replace` | History |
| associated | `bevy_ui_router::ecs::RouterNavigation::back` | History |
| associated | `bevy_ui_router::core::RouteNodeOptions::with_error_boundary` | Recovery |
| associated | `bevy_ui_router::core::RouteNodeOptions::with_error_view` | Recovery |
| associated | `bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines` | Authority closure |

facade harness 在 implementation plan 的首个非runtime任务切换到本表

parser 固定使用本节 exact heading 与三列表头，拒绝空表、重复 path、未知 kind/First slice、非canonical path

History rows 只能在 owning slice 的真实 RED/GREEN、public journey 与 review 通过后删除

其他 rows 保持不变

### 10.2 Contract delta activation

本表只持有无法由 exclusions 表或 authority catalogue 表表达的 staged contract fact

| Item | Owning slice | State | Claim boundary |
| --- | --- | --- | --- |
| `NavigationRequestSnapshot::requested -> Option` | Back execution | planned | Back execution public contract PASS |
| `NavigationEnqueueErrorKind::IdentityExhausted` | Replace | planned | allocator-derived public RED/GREEN PASS |
| late-bound back complete evidence | Back failure | planned | complete C3 two-branch PASS |

`State` vocabulary 固定为 `planned | active`

item 必须唯一，未知 state、重复 item 或不在最终 Fixture/Golden/Journey 的 item 一律拒绝

public method 是否仍 excluded 只由 10.1 持有，不在本表重复

contract-delta harness 固定使用本节 exact heading 与四列表头，并校验 active item 已出现在当前 Fixture/Golden 与 owning slice public evidence

### 10.3 History authority regression additions

| Test function | Owning slice | State |
| --- | --- | --- |
| `active_cycle_enters_authority_settlement` | Back failure | planned |

`State` vocabulary 固定为 `planned | active`

test function 必须唯一、属于 final MVP vocabulary，且不得与既有 Root 20 + Nested 3 重叠

catalogue harness 只激活 state 为 `active` 的 History row，并与既有 Root 20 + Nested 3 形成唯一 union

该 union 必须与 Rust catalogue、direct `#[test]` source registration、binary registration 与实际通过集合双向相等

History 不激活 `post_commit_host_loss_preserves_committed_history`、`host_reacceptance_detaches_old_top_slot`、multi-router、reaper 或 registry-closure cases

## 11 Canonical amendments before runtime

Spec independent review PASS、用户确认已提交 spec、Plan independent review PASS 后，先用独立 green documentation/profile commit 完成以下修订

- `docs/README.md` 与 `docs/superpowers/README.md`
  - 把本文登记为 History 期间唯一 active stage profile 的显式例外
  - 保留其他 historical specs/plans 的证据-only地位
  - 加入当前文档索引与 machine profile 路径

- `docs/target-architecture.md`
  - command 与 resolved target 分离
  - optional request projection
  - exact history incarnation、revision、entry 与 capability
  - manifest-neutral back capacity reservation与proven-empty
  - `IdentityExhausted`
- `docs/authority-transition-matrix.md`
  - no-target proven-empty
  - precommit authority precedence
  - cleanup layering与stage ownership
  - terminal与requested窄例外
  - `IdentityExhausted` 的无request、零公开变化、永久fail-closed、非recovering映射与错误优先级
- `docs/facade-contract-fixture.md`
  - `requested() -> Option<&RouteLocation>`
  - `IdentityExhausted`
  - replace/back observation semantics
- `docs/facade-associated-api-golden.txt`
  - owning slice RED step把`requested()`签名改为optional
  - owning slice RED step增加`IdentityExhausted` variant
  - 不增加新的public authority type或临时API
- `docs/mvp-acceptance-journeys.md`
  - C1–C3 的 `requested Some/None`、C3双分支与History/Recovery边界
  - facade consumer gate从真实 allocator/capacity predicate导出`IdentityExhausted`
  - `UnknownRouter -> availability -> NoBackEntry -> IdentityExhausted` precedence
- facade/profile harness
  - 从本文10.1读取public exclusions
  - 从10.2读取staged contract delta
  - 从10.3读取History authority catalogue additions
  - 三类item key互斥，重复事实必须失败

Target、Matrix、Fixture、Journey 与 README 的规范修订先独立提交

可执行 Golden、negative exclusions 与 consumer fixtures 在所属垂直 slice 的第一步生效并产生真实 RED，再于同一 slice GREEN，不提交长期坏树

## 12 Verification gate

每个 production semantic change 先有能在真实 predicate 上失败的 RED

每个 slice 按 focused → affected crate → public target 验证，并由独立 reviewer 检查同一 stage spec/plan

History completion 必须显式运行

- Core 全量 unit 与 doctest
- View 全量
- ECS 全量 unit 与 integration targets
- focused C1–C3 与 F1–F3
- 完整固定 `public_navigation_journeys` target，覆盖既有 A、B、D 回归
- `public_navigation_authority_audit`
- facade feature、golden、negative exclusion 与 isolated consumer gates
- 当前 History stage profile 激活的 authority catalogue与脚本
- focused `history_source_aba_is_rejected`
- focused `cross_incarnation_history_replay_is_zero_write`
- focused `no_back_entry_precedes_identity_exhaustion`
- focused typed preparation retry owner-return 与永久 contradiction fail-closed
- workspace nextest、check、doctest
- fmt、diff check 与 clean tree

retained-empty 与 slot-only 必须分别保留至少一个 replace/back 物理代表用例

上述 focused 名称可以落在 Core/ECS 私有测试，不进入 MVP authority catalogue；plan 必须冻结其exact target与producer

final review 分为

- Core lifecycle 与 history authority
- ECS physical authority 与 cleanup layering
- public facade、canonical facts 与 stage exclusions

Critical 与 Important 关闭后收敛，不以增加重复 review 代替证据

## 13 Documentation close

History completion gate 通过后才更新

- `docs/current-architecture.md`
- `docs/roadmap.md`
- `docs/README.md`
- `docs/authority-transition-matrix.md` 的实现 checkpoint，或记录复核后无需事实改动
- `docs/implementation-goals.md` 中的当前阶段事实
- `.superpowers/sdd/progress.md`
- 本 profile 的 activation/exclusion 状态

事实文档只记录实际提交与成功命令输出

Roadmap 进入 Recovery，不能写 `MVP PASS`

历史 Reconciliation profile 保持为历史证据，不回写

## 14 实施纪律

- 共享 workspace 串行修改 lifecycle runtime
- Root 为唯一 Cargo/rustc 验证、index 与 commit 负责人
- 子代理可按明确文件责任编写或并行只读审查
- 生产语义、fixture/golden 与 public journey 在 owning slice 内闭合
- completion gate 发现真实 RED 时回开 owning slice
- 超出 stage owner 的能力先修订 canonical 文档，不借 History 偷渡实现
