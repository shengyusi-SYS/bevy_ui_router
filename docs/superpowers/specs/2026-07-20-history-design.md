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

只有既有 pre-commit authority settlement 接管 no-target 时可以公开该组合；这不是正常 no-target cleanup

| Window | Running status | Eventual terminal | Owner rule |
| --- | --- | --- | --- |
| proven-empty返回前被既有pre-commit authority settlement接管 | `Recovering` | settlement后`Failed(HostLost / AuthorityLost)` | unresolved lifecycle lineage与reservation线性转为authority settlement owner |

settlement 完成前仍是 `Running(...)`，表中的 terminal 只是预封存的 eventual cause

normal candidate 与 typed Retry 在 physical proven-empty + Core logical-zero approval + canonical-zero exact-bind 前完全 internal，不发布 `Running(CleanupPending)`、`HistoryIntentRejected`、normal cleanup trace 或 continuation

authority takeover 与 permanent fail-closed 可在 proof 前进入既有 settlement/terminal，但无 fallback、无 history 变化，也不发布正常 no-target 的 `HistoryIntentRejected` 或 `CleanupPending`

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
5. ECS issuer 预留 manifest-neutral unbound ledger slot owner
6. Core 预留 request、transaction、cleanup batch 与 opaque accepted branch-reservation identity/pool
7. Core 接受 request 后返回 `BackAcceptedReservationView`，ECS 以不可失败 move 将其与 capacity 绑定；该 view 只绑定 exact Core world seal projection、router/history incarnation、request、transaction、cleanup batch 与 accepted branch-reservation identity/pool

该 unbound reservation 只预留唯一 identity、domain 与 capacity，不声明 manifest、count 或 zero；Core token drift 或 allocator failure 由 issuer 以 `EnqueueRejected` exact-retire

entry install 只由 domain issuer 签发不可复用的 capacity domain，不预分配固定 pool，也不建立隐藏并发上限。每个 `BackCapacityIssuer` 持有 entry-local persistent slot table；cell 只在 `Free | Outstanding | Leased` 间原位转换，permit、slot 与 ledger owner 只线性持有 exact cell lease，不能把 cell 从 table 移出或从 projection 重建

`reserve` 先复用 `Free` cell；没有可复用 cell 时才执行 grow-on-demand，并在 owner/generation mint 前完成新 cell、fully initialized vacant header/storage 与 exact outstanding registry entry 所需的全部可失败容量预留。真实 growth failure 返回 `LedgerSlotUnavailable` 并在 Core allocator 前映射为 `IdentityExhausted`，不创建 request、不 mint owner/generation，且 Core、queue、public 与语义 capacity state 零写。成功 mint 后的 bind、retire、transfer 与 canonical-zero 都不得再分配

exact retire 烧毁 owner、推进 generation 并把同一 cell 原位归还 `Free`；no-target transfer 把同一 cell 转为 `Leased`，canonical-zero 只在该 cell 内安装安全初始化的 empty ledger。generation 没有后继时，该 cell 永久退出可复用集合，后续 reservation 只能 grow 新 cell。该路径不使用 `unsafe`、未初始化 storage 或 unchecked initialization conversion；`EnqueueRejected` 只属于 unbound retire 参数，bound disposition 不包含 enqueue rejection

allocator failure 或 Core token drift 后的 unbound retire 若理论上返回 `Rejected { owner, rejection, error }`，处理固定为

- outcome 直接返还原 `EnqueueRejected`，不要求该值为 `Copy`，也不允许调用方旁存或重建
- exact entry 把完整 owner、原 `EnqueueRejected` 与 retire error 移入唯一私有 fail-closed residual
- 不得 panic、丢弃 owner 或映射为普通 exhaustion
- residual 非空时当前及后续 Back 均沿用 exact-entry fail-closed gate 的 `HostUnavailable` typed return
- 正常 allocator failure 成功 exact-retire 后仍返回 `IdentityExhausted`

`BackAcceptedReservationView` 是 pre-resolution lineage，不绑定 normal attempt、eventual host/slot/frame/task/generation、destination 或 execution authority，也不能重建 Core owner

Core 接受后 reservation 与 accepted view 绑定；live Core exact verify 命中 `NoTarget` 时才转为同一 issuer outstanding 的 `NoTargetCapacityPermit`，Core 失败则 identity 烧毁并释放容量

`RouterNavigation` 本身是唯一 ECS World Resource，并持有唯一 persistent domain issuer；它没有 per-entry issuer field。`RouterNavigation::new` 初始化 domain issuer。安装 router 时先在 local 构造完整 `RouterEntry`，由 domain issuer 为该 entry 签发并安装不可重建的 `BackCapacityIssuer`，全部成功后才 push；失败 typed map 到既有 install error，不留下 ghost/partial entry。所有 back helper 都先由 exact `RouterHandle` 解析 entry，再借用该 entry 的 issuer

`BackCapacityOwnerId` 必须实现 `Ord + PartialOrd`，但排序能力不授权重建 owner。Core 的 `test-support` 定义与 layer-neutral sealed channel factory 固定在 `crates/bevy_ui_router_core/src/test_support.rs`；ECS 只在自己的 `Cargo.toml` 以 `test-support = ["bevy_ui_router_core/test-support"]` 转发该 feature。`app_ext.rs` 只增加一个 crate-private `RouterHandle` domain predicate，用于区分合法取得的 foreign handle 与 same-domain unknown；它不暴露 raw domain/router value、public constructor 或 forged-handle path。plugin 与 facade 无需变化，也不得重新定义 producer 或扩展 feature authority

production issuer install 使用 typed `AlreadyInstalled | Reserve(error)` 结果。installer 必须先检查 exact entry 的 issuer slot，再允许 domain issuer 签发；second install 在签发新 domain identity 前拒绝，不能替换既有 issuer、消耗 identity 或覆盖其 outstanding state。test journal channel 的 `AlreadyInstalled` 是独立 test-support 语义，不能代替 production issuer install guard

`test-support` 的跨 crate 顺序证据由 Core doc-hidden channel factory 产生 layer-neutral sealed producer。只读 `BackCapacityOrderObserver` 只增加 `observes_same_journal(&self, other: &Self) -> bool`，以 `Arc::ptr_eq` 一类 opaque identity equality 判定 journal relation；它不暴露 raw journal/domain identity 或 producer，不能记录事件或授权 lifecycle work。安装只由 ECS owner test-support module 的 doc-hidden free function `testing_install_back_capacity_order_observer(&mut RouterNavigation, RouterHandle)` 提供，public audit 直接调用该 owner function；它不是 `RouterNavigation` associated API，不由 facade re-export，也不更新 facade golden

installer 顺序唯一且固定：先拒绝 foreign domain；再解析 exact same-domain entry，缺失则拒绝 unknown；然后 preflight Core、issuer 与 facade 三个 producer slot 全部 vacant，任一 occupied 都以 `AlreadyInstalled` 拒绝；之后 Core factory 恰好调用一次，再把三个 sealed producer 不可失败地移动到同一 exact entry，最后返回 observer。factory creation 之后没有 validation、allocation 或其他 fallible installer，因此不引入 rollback。Step 6 micro-slice 3 只安装 empty channel：clone relation 为 true、两个 exact entry 的 independent observer relation 为 false，两个 snapshot 都为空，并且不记录 `Issued | Retired | TransferredNoTarget | BoundCanonicalZero | CoreAllocatorAttempt | CoreAllocatorFailure | FacadeTypedReturn`。这些真实顺序事件及 `RouterNavigation::back` 的 facade return record 全部由 micro-slice 4 实现。foreign/already-installed 由 public audit 证明，legitimately obtained foreign handle 来自第二个 App；same-domain unknown 由 ECS unit 使用既有内部 handle fixture 证明 factory closure 调用次数为零且 entry/producer slots 不变。所有拒绝均 typed、zero-write 且无 partial installation；Core 不命名 ECS type，测试只取得 observer/error，不取得 producer 或 write handle

micro-slice 4 只在真实生产边界记录事件

- `Issued` 位于 successful mint 之后，owner、generation 与 outstanding cell 已同时成立
- `CoreAllocatorAttempt` 位于 exact admission refreeze 成功之后并紧邻真实 `reserve_navigation_branch`
- `CoreAllocatorFailure` 只由该真实 reserve 的 `Err` 记录，stale/token drift 不记录 attempt 或 failure
- `Retired` 只在 exact owner burn、generation advance 与 cell reclaim 全部成功后记录
- `FacadeTypedReturn` 位于最外层 typed error return，且正常 allocator failure 路径上严格晚于 `Retired`

ECS allocator failure gate 对 `TestingReservationCounter::ALL` 的每个真实 counter

- 使用 fresh App 与 fresh observer
- 证明 exact 五事件序列 `Issued → CoreAllocatorAttempt → CoreAllocatorFailure → Retired → FacadeTypedReturn`、原 owner 已烧毁、reclaim generation 已推进
- 证明没有 ECS request、bound/pending reservation、public snapshot 或 World fingerprint 写入

独立 Core private companion gate 同样遍历 `TestingReservationCounter::ALL`，每项使用 fresh engine 与 fresh observer

- fault seam 生效后冻结 production-semantic full private snapshot，包含 allocator、queue、records、history、state/current/action/mount 等字段
- observer journal 是 test-support 证据通道，不属于该 snapshot，也不得掩盖生产状态变化
- 证明真实 allocator exhaustion 调用前后 full private snapshot 零写
- 证明 stale/token drift 在同样零写前提下不产生 allocator attempt/failure
- ECS 不新增 Core private snapshot accessor

该证据只补全既有 Step 6，不新增路线图、第二 publisher、classification 注入或公共 API

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

back 成为 running 时，Core 消费 accepted branch reservation owner 并只执行一次 live preview

- 有 target 时先冻结 exact destination，生成只携带 lineage 与完整 reserved frame/task identity pool 的 move-only raw target owner；该 owner 绑定 normal attempt 与 exact source/destination，但不提前安装 matcher、reconciliation 或 action，zero-entering 仍保留并烧毁未使用 identities
- exact target receiver 必须先在不解构 `VerifiedBackTargetExecutionOwner` 的前提下完成 pre-splice receiver preflight：exact world/router/history/scope、exact retained `ConsumedReplay` queued request 与 expected target，并要求 `action`、`current`、`pending_no_target_back`、`pending_back_no_target_authority`、`current_history_capability`、`pending_history_preparation_error`、`prepared_publication_authority` 全部为 `None`，且 `state.current_request() == None`。preflight 通过后才可解构 owner，并在 capacity retirement callback 前完成 source、host/top-slot、active/child-slot baseline exact refreeze，以及 matcher、logical reconciliation、first action 与 mount issue 的全部准备，形成 `PreparedReady | PreparedFailureCleanup | PreparedPermanent`
- `PreparedReady` 保持 engine-held exact history capability 的唯一关联，并以 `BackTargetResumeAction::{FrameTask(RootFrameTask), MountRequest(MountRequest), Commit(CommitTicket)}` 表达正常执行；`PreparedFailureCleanup` 独立交出 `FailureCleanupRequest`，不得伪装成 resume action
- 结构一致且无 predecessor 时产生 target-free no-target candidate，不签发 history capability，不合成 normal attempt 或 frame identity
- cursor/entry/location 不一致、target 深度超过 reserved maximum 或 accepted pool 不匹配产生 typed permanent contradiction

Core 通过 typed resolution handoff 单次移动 `Target(raw target owner) | NoTarget(raw candidate) | Contradiction(raw contradiction owner)` 三个分支之一；ECS 不能读取 `requested`、bool、snapshot 或 reservation projection 推断分支或 execution authority

raw handoff 不能授权后续 Core action。Core 必须消费 exact accepted view + raw handoff 做 lineage verify；Matched 返回 move-only verified wrapper，其唯一 split 是原 accepted view + branch-specific verified target/no-target/contradiction owner，不能拆回 raw handoff。ECS 用该 accepted view 与原 capacity 重组同一个 `BoundBackCapacityReservation`，再与 verified branch owner 配对；resume、source refreeze 与 contradiction consumer 也只接受各自 verified owner。foreign Mismatch 返还原 accepted view、raw handoff 与 evidence，ECS 同样重组完整原 capacity owner

duplicate 不等于 raw handoff replay，ECS 在 exact consumption 前保存可 Clone、non-authorizing 的 `BackResolutionEvidence`，Matched 消费 raw handoff 后必须把该 evidence 交给真实生产 predicate `RouterEntry::validate_back_resolution_evidence(&saved_evidence)` 并要求返回 `BackResolutionEvidenceDisposition::ConsumedReplay`，capacity retirement evidence 不能替代 resolution duplicate evidence，测试不得直接注入 disposition，移除或短路该 predicate 时 gate 必须失败

Core 是 raw Back resolution evidence consumption state 的唯一 owner，现有 request record 固定字段为 `back_resolution: Option<BackResolutionRecordState>`，其中 `BackResolutionRecordState::{Current(BackResolutionEvidence), Consumed(BackResolutionEvidence)}`。queued Back 在 raw handoff 生成前为 `None`，非 Back 始终为 `None`；生成 exact observed evidence 后且 action 对外可见前安装 `Some(Current)`，只有 exact `verify_back_resolution_handoff` 的 `Matched` 可原位改为 `Some(Consumed)`，不新增 collection 或消费后 allocation

Core 通过 doc-hidden `RouterEngine::classify_back_resolution_evidence` 返回 `BackResolutionEvidenceClassification::{Current, Foreign, ConsumedReplay}`。retained path 必须先比较 supplied evidence 与 stored evidence exact equality，再按 `Current | Consumed` 分类；foreign mismatch 前保存的 observed-form evidence 仍为 `Current`，Core 返回的 mismatch diagnostic evidence 是 non-observed-form，必须分类为 `Foreign`。record eviction 后必须由 exact opaque scope、`evicted_through` 与 `highest_issued` 共同证明 `ConsumedReplay`，不得只凭 `highest_issued` 推断。ECS crate-private predicate 只委托 Core 并映射 disposition，不建立 ledger、不读取 opaque field

`Current` record 不得 terminal 或 evict，只有 `Consumed` 可进入 terminal retention/eviction。duplicate gate 覆盖 queued `None`、in-transit/foreign saved `Current`、mismatch diagnostic `Foreign`、Matched/terminal/evicted `ConsumedReplay`，并断言 Matched 唯一 Core-private 写入是 exact record state，其余 Core、World、history 与 public projection 不变

supported production callsite 中 Core action 与 exact `BoundBackCapacityReservation` 的配对由 enqueue acceptance 建立，并只由 exact take 单次消费，构成 crate-private closed invariant，本阶段不新增 `MissingCapacity`、domain quarantine 或恢复协议，也不把该内部 invariant 破坏提升为公共或生产 lifecycle 分支

target resume、contradiction terminal 与 no-target transfer/refreeze 都必须在同一个 exact-entry helper borrow 内完成

target helper 先对 entry-owned issuer 做全部 non-mutating exact preflight

Core 随后必须在解构 `VerifiedBackTargetExecutionOwner` 前完成上述 exact pre-splice receiver preflight。任一 world/router/history/scope、retained consumed queued request/expected target、上述七个 busy slot 或 `state.current_request()` fact drift 都返回 receiver `Mismatch`、调用 callback 零次并返还整个原 verified owner；ECS 丢弃未提交 retirement guard 后可重组未退休的完整 bound branch。该分支是 typed zero-write safe-stop，不是 retry，也不得 mint `BackTargetPermanentOwner`

只有 preflight 通过后，exact receiver 才在 callback 前完成 history source、host/top-slot、active/child-slot baseline exact refreeze，以及 matcher、reconciliation、action 与 mount issue 的全部可能失败工作，再形成 `PreparedReady | PreparedFailureCleanup | PreparedPermanent`

prepared branch 形成后 callback 恰好调用一次，callback 后只允许不可失败地移动或安装已准备 owner/action，不得 query、allocation、validation、`expect` 或触发 hook/observer

same-engine source/host/baseline drift 与 generation/depth contradiction 使用 private typed `Permanent/AuthorityLost`，不是 `Mismatch`。preflight 后对 clone-local request transition/trace 的矛盾仍可 mint cause 为 `RequestRecord` 的同一个 permanent owner，因为原 retained queued consumed record 保持 exact 且可 terminalize

contradiction terminal 与 no-target transfer/refreeze 继续遵循 wrong receiver callback 零次、exact prepared branch callback 一次的规则，issuer expectation 与 callback 均不得跨出 helper

`BackTargetStart` 将正常 `Ready(BackTargetResume)` 与 `FailureCleanup(FailureCleanupRequest)` 分开

`BackTargetResume` 的 action、execution 与 continuation 是同一个 move-only resume 结果，并与 engine-held history capability 保持唯一关联

ECS 使用唯一 `RouterEntry::pending_back_target_execution` 阶段 enum 线性持有完整 target lifecycle owner，不扩展通用 `PendingCommit`。`ResumeBlocked` 保存所有未安装的 start/permanent/capacity-rejected/receiver-mismatch/driver-failure/applied-report-mismatch/applied-mount-request 完整分支，禁止第二 dispatch safe-stop field；`Driving` 只持 history execution + continuation capacity 并与既有 physical action owner exact 配对；real ticket + splice 后 whole-transition 为 `Classifying`，该阶段完整持有 pure physical classification、branch-specific acceptance、Ready plan retry/permanent、Ready accepted 与 ledger reserve failure owner。只有 Ready acceptance 与 ledger reserve 均成功后才进入同时持有完整 physical owner、prepared continuation、`PreparedCleanupLedger` 与 Core `CleanupManifest` 的 `Prepared`；bridge mismatch/failure 进入保留同一 physical/ledger/Core owner 的 `CommitFailure`。各 variant 互斥，每次转换只能 whole-take 后单次安装；进入 post-accept variant 后不得重建旧 capacity，也不得同时保留 `Driving`、`TargetPreparationOwner` 或 prepared owner。任何尚未接线的 typed 分支原样 safe-stop，publication、failure/authority handoff 或 terminal 后该字段必须为空

`FrameTask` 分支经真实 frame report/apply 取得 commit，`Commit` 分支不产生 frame report，两者取得 commit 后完成 physical preparation 并进入既有 `prepare_commit`。initial Ready 的 `MountRequest` 在当前 slice 必须连同完整 start owner 保留为 `ResumeBlocked::Start` 并 safe-stop，普通 mount field 保持空，等待 target MountRequest action slice

target-active root `FrameTask` 的所有失败出口必须先经过唯一 target-aware owner route：expected outlet、host/authority、binding/driver、report failure/stale、realization 的 Pending/Residual、`push_frame` 与 parent preflight 均不例外。该 route 只能把完整 `PendingBackTargetDriving + exact RootFrameTask + kind/message + Unverified::{Pending(PendingCommit), Residual(FailedPreparedBranch)}` whole-save、同栈继续 target report，或在证明 non-target 后返还原值给既有 generic 路径。普通与 nested 语义不变，target-active 时普通 `pending_failed_branch` 保持空。scanner 只能分类，不能扫描后重建 owner

`freeze_back_target_driver_physical_partition_source(world, entry, driving, task)` 是 pre-report source 的唯一 producer，只冻结 non-authorizing exact task lineage、host/top-slot、active/protected 完整 closure、mount/role/reconciliation 与 owner correlation，zero-write 且不消费 physical owner。freeze Err 由 caller 与 whole Unverified owner 合成 pre-report `PhysicalIndeterminate`

唯一 pre-report classifier 线性消费该真实 owner，并接收同一 `world: &World`、exact current `RouterEntry` 与 source 后 exact-refreeze，不再接收或重建 `RootFrameTask`。Pending 与 Residual 都必须验证 registry、World 与 relationship completeness、target/protected disjointness，输出只能是 `Reportable(ProvenZero | Residual)` 或 `Indeterminate { whole original owner, frozen typed evidence }`

scanner-only、registry-only、World-only residual、unclassified router-owned Entity、relationship/seal/lifecycle drift 或 protected overlap 均为 `Indeterminate`，在 `report_back_target_failed` 前 whole-save 并 safe-stop。当前 root FrameTask slice 只有 exact TopSlot/root lineage 可成为 `ProvenZero`；nested、registered-child 或稀疏 mount role 未完整时保持 `Indeterminate` 等待对应 action slice，不进入 generic path。`report_mode()` 只在 Reportable owner 上定义且 total，`Unaccepted` 只能持 Reportable

report 前必须把 exact pre-report source 线性封为 non-authorizing `BackTargetDriverPostReportRefreezeSource`；report success 将该 source 与 history、Reportable physical、Core outcome 一并移入 Reported。Reported、ConsumeMismatch、PhysicalMismatch 与 PostReportPhysicalIndeterminate 的所有 post-report owner 都必须携带同一 source

proof 与真实 owner 封装后只在 classify → report → consume 的同一调用栈有效，期间不得有 World write、schedule boundary 或 yield。任何 safe-stop 都必须降回 Unverified owner，不能保存或再次消费旧 proof；report receiver mismatch 保存 task、whole Unverified owner 与 mismatch，consume mismatch 保存 post-report source、Core owner、history、降回 Unverified 的 physical owner与 mismatch

post-report 继续只能调用 `reclassify_back_target_driver_post_report(world, current_entry, source, unverified)`，由同一 World 与 exact current entry context exact-refreeze，不接收或重建 `RootFrameTask`。它只返回可立即 consume 的 Reportable 或 `PostReportIndeterminate`；后者进入独立 post-report stage，绝不能再次 report 或把 Core outcome 当作 pre-report owner

唯一 target-specific Core receiver `report_back_target_failed` 必须在 report mutation 前 exact-match history、raw continuation capacity 与 task。Mismatch report 零次并返还 capacity、task 与 opaque mismatch；该 mismatch 是 Core-private tagged projection，至少冻结 expected scope、expected task/execution 与 observed current/action/stage receiver state，不得把 predicate 降为 bool 或写成同值 scope。Matched 在同一调用栈把 capacity 转成私有临时 continuation 后立即执行真实 report，不得跨 schedule 保存 `RouteFailureContinuationOwner`。Core 只能按真实 post-report state 返回 `FailureCleanup | Fallback | Terminal` owner/evidence，不返回 `Option`，post-report owner 不再含 raw capacity

target applied report 同样只能调用统一 atomic `report_back_target_applied(history, capacity, task, LegacyLeaf | WithOutlets)`。调用前必须在保留真实 pending owner 的同时使用唯一 source producer 完成 freeze，并把 source 线性封为 post-report source；freeze Err 与 whole applied context 合成 pre-report `PhysicalIndeterminate::AppliedReport`。该 API 在 mutation 前 exact-match history、capacity、task 并验证 report 输入；Mismatch 零写返还 capacity、task、report 与 opaque mismatch，ECS 保存包含 post-report source 的 whole `AppliedReportMismatch`。applied-report outcome 的 exact domain 是 `MountRequest | Commit | FailureCleanup | Mismatch`：`MountRequest | Commit` 返还同一 raw capacity，`FailureCleanup` 返回真实 post-report cleanup owner且不返 raw capacity

root final task → ticket 与 applied-with-outlets 均只能经过该 API。Commit 继续走 existing exact pairing并丢弃无授权作用的 post-report source；FailureCleanup 必须保留已实现 frame/pending 产生的真实 `Unverified::Residual(FailedPreparedBranch)` 与调用前封好的 post-report source，只经专用 post-report reclassifier 才可成为 consume-ready Reportable Residual 并进入同调用栈瞬态 `DriverFailure::Reported`，不得在 Core mutation 后尝试 ProvenZero 或再次 report；PostReportIndeterminate 必须进入独立 post-report stage。MountRequest 必须把 history、returned capacity、request 与 pending whole-save 为 target-field `AppliedMountRequestPending`，普通 mount field 保持空并在 action selection 前 safe-stop，等待对应 action slice。`FrameTask` 只保留为 initial resume action 或 mount completion 后的 action，不得成为 applied-report outcome 或 applied pending stage。静态 exact-domain gate 必须同时证明 Core private `AppliedReportTransition` 只有 `MountRequest | Commit | FailureCleanup`、receiver outcome 只额外包含 `Mismatch`，并拒绝 applied `FrameTask` variant/stage。reconciliation drift、mount issue 或内部 reissue 必须由 typed outcome 表达，不能使用 `expect`、`Option` 或事后 action 推断

`DriverFailure` 至少持有 pre-report `PhysicalIndeterminate | ReceiverMismatch`、同调用栈瞬态 `Unaccepted | Reported`、post-report `ConsumeMismatch | PostReportPhysicalIndeterminate | PhysicalMismatch`。dedicated consumer whole-take Reported，且只能调用 `RouterEngine::consume_back_target_driver_failure`，不能拆 private inner、读取 `engine.action()` 或 `Option`；static gate 禁止 `Unaccepted` 或 `Reported` 在 schedule 返回时残留

Core consumer exact 验证 outcome receiver：FailureCleanup 解封真实 request；Fallback 验证绑定的 fresh fallback attempt/action 后返回 `FallbackReleased`，action 仍由 engine 持有，ECS 清 stage 后才由正常 action 驱动取走；Terminal 返回真实 opaque evidence；wrong receiver 零写返还 whole Core owner + mismatch。consume mismatch 使用 Core-private tagged projection：FailureCleanup 冻结 expected request 与 observed current/stage，Fallback 冻结 expected attempt/action 与 observed action/current/stage，Terminal 冻结 expected request/router/status 与 observed request record/status。ECS 只整体保存 opaque view、history 与 physical owner，不能读取或重建 identity

FailureCleanup 才可把真实 request 与 Reportable physical owner移入既有 `pending_failed_branch` 并清 stage。Residual 的动态 FailureCleanup 只有在真实 root residual producer 已接通且 classifier 给出 Reportable Residual 时才可闭合，不能用 test seam 注入该 outcome。Fallback/Terminal 只接受 `ProvenZero`；Reportable Residual 与二者配对时不得调用 Core consumer，必须降回 Unverified 并把完整 post-report owner保存为 typed `PhysicalMismatch`。当前 `PhysicalResidual` Core mode 必定返回 FailureCleanup，因此该 PhysicalMismatch 输入在当前 slice 不可动态到达，只保留 fail-closed 防御，不伪造 journey。exact Fallback/Terminal consume 后清 stage；consume mismatch 同样降回 Unverified 并保留完整 Core owner，旧 proof 不得授权重试。generic `report_failed*`、`Option`、obligation count 或事后 `engine.action()` 不得参与 target failure 分类

上述 route 覆盖 identity/expected outlet、host/authority、binding/driver、BuildFailed/ApplyFailed、apply stale、realization 有无 residual、`push_frame`、parent preflight、原 `report_applied_with_outlets` failure-action 条件与 leaf final report error。只有证明 non-target 的普通/nested action 保留既有 generic 语义。report identity mismatch 仍是 paired `PendingFrameDrive` retry，不属于 failure。当前 root FrameTask slice 只闭合 exact TopSlot/root lineage；`MountRequest` 与不完整 nested/sparse role 等待各自 action slice，existing Ready/publisher 语义不变

当前 driver-failure slice 的真实 ECS `--lib` RED 只覆盖 target-aware early exit、report/consume exact receiver evidence、当前自然可达的 fallback/Terminal/mismatch 与全 owner safe-stop。Residual FailureCleanup、observer residual 与 protected overlap 只在真实 root residual producer 接通后、实现对应 producer 分支前各自加入真实 RED。atomic applied residual 与 MountRequest safe-stop 属于后续 atomic/action slice，在各自实现前另行建立 RED，不作为当前 driver-failure slice 的闭合条件。authority catalogue 静态 gate 继续审计全部出口，但不能替代动态证据；missing symbol、setup failure、placeholder panic 或直接注入 outcome 均不算 RED

`FailureCleanup` 只进入既有失败清理链，不能进入正常 publisher

`BackCapacityRetirementEvidence` 只证明 capacity lineage 已 terminal，是 non-authorizing evidence。target dispatcher 必须在同一 exact-entry dispatch 内通过 entry issuer 的现有 exact classifier 完成命名 consuming audit，并将 audit 与 `Ready | FailureCleanup | Permanent` 分支安装作为一个不可分割步骤；未实现分支暂存完整 `{ branch owner, retirement evidence }`，不得 drop，也不得把 evidence 塞进 `TargetPreparationOwner` 或 failure-cleanup chain

ECS 的 `LifecyclePreflightEvidence` 保持 crate-private physical/boundary evidence，任何 Core source、`RouterEngine` inherent method 或 Core-owned outcome 都不得命名它。`PreparationBoundaryScopeView` 的唯一 producer 是同一个 `LifecycleContinuationCapacity` 的只读投影；view 只供 exact receiver 校验，不能重建、消费或授权 owner。Core 的 opaque exact pairing predicate 必须在 action 注入、retry restore、final task → real ticket 与 commit 入口分别校验同一 history/continuation owner 与 returned action、pending frame/mount/final task 或 real ticket 的 exact engine、request、transaction 和 task identity，并以 Core-private tagged projection 保存 expected/observed receiver state。所有 projection 只校验，不能重建 owner。Core 的 `accept_back_target_ready | pending_rejected | authority_lost` 均返回 typed `Matched | Mismatch`；Mismatch 必须返还原 `LifecycleContinuationCapacity` 与单一 opaque、不可构造、不可检查且不授权重建 owner 的 `PreparationReceiverMismatchView`，并保持 zero-write。ECS 将 mismatch view 仅作 diagnostics，并继续把自身 physical evidence 与完整 ECS owner 绑定

所有 callback-bearing `*_with_*` 方法都是 `#[doc(hidden)]` trusted ECS integration API，不从 facade reexport；facade/adapter 既不能取得 verified/prepared Core owner，也不能通过受支持 API 调用这些方法。该模式沿用既有 `ValidatedCommit::publish(FnOnce())`：closure 本身不是 authority，verified/prepared Core owner 才是 authority；唯一 ECS callsite 与 callback body 由 static/compile gate 审计。这里不引入 attestor、signature 或 second receipt，也不声称 Rust 类型能阻止恶意 direct Core caller

typed handoff 只确定 running execution lineage，后续 `Ready | PendingRejected | AuthorityLost | RetryablePreparationFailure` physical classification 与 owner disposition 仍由 §4.4 独立决定

最小 root `FrameTask` slice 也必须经过真实 report/apply、real `CommitTicket`、splice、physical classifier、typed Core `Ready` acceptance 和既有 `prepare_commit`/publisher。尚未接线的 resume、classifier 或 acceptance 分支只能 typed safe-stop 并保持 public zero-write，不能绕过 classifier 或引入第二 publisher

contradiction handoff 必须 exact-match paired bound capacity，退休 ECS reservation 后由 Core 单次消费 accepted branch、request、transaction 与 cleanup batch owner；request 有界成为既有 `Failed(AuthorityLost)`，World、presentation 与 history 零写，不记录 `HistoryIntentRejected`，不留下 outstanding owner 或 retry

target 的 Core source/host/baseline drift、generation/depth contradiction 与 preflight 后 clone-local request transition/trace contradiction，在 ECS splice 与 retirement callback 前只能准备为唯一 opaque `BackTargetPermanentOwner`。它是 Task 4-only 的 pre-splice Core terminal owner；preflight drift 绝不能进入该 owner。cause-specific source 映射固定为：`SourceDrift::Refreeze` 的 expected=`destination.source`、stored observed/current=`observed_source`；`SourceDrift::Capability` 的 expected=`drift.capability.source`、stored observed/current=`drift.observed`；`AuthorityLost(cause)` 的 expected=stored observed/current=`destination.source`。settlement 必须 exact 校验 owner scope/reservation、当前 source 等于该 cause 的 stored observed/current source、当前完整 baseline 等于 stored observed baseline，以及 exact retained queued consumed request。settlement drift whole-return owner + typed evidence 且 zero-write；exact match 恰好 terminal 一次。terminal evidence 用无 optional 歧义的 opaque cause-specific variant 保留上述 source facts，以及 stored 完整 expected/observed `BackTargetPreparationBaseline`。projection 只用于 validation/evidence，不能重建 owner。该 owner 不取得 ECS splice、physical cleanup 或 World mutation authority，不进入 post-splice classifier，也不能被重建为任何 ECS permanent owner

当前 production resume 独占同步 `&mut self`，首次 `refreeze_back_source` 成功后到三个 `freeze_back` callsite 之间没有 history mutation，因此 `SourceDrift::Refreeze` 是当前真实 production source-drift producer，`SourceDrift::Capability` 只是首次 exact refreeze 后的 defensive unreachable recheck，不对应现有 public/production journey。允许最小 `cfg(test)` seam 在首次 refreeze 通过后、`freeze_back` 前只改变真实 `CanonicalHistory` source fact，再由 production `freeze_back` predicate 产生 `HistorySourceDrift`；seam 不得直接构造 owner、classification、permanent 或 terminal outcome。该防线的 terminal evidence 仍须证明 expected=`drift.capability.source`、observed=`drift.observed`、完整 expected/observed baseline 与 owner conservation

resume 成功并建立 splice 后只有两个不可互换的 permanent source：classifier permanent 保留整个 `PreparationPermanentOwner<TargetPreparationOwner>`；Ready-plan permanent 保留整个 `PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>`，包括 Ready 分支已经冻结的 physical owner、`FrozenCleanupManifest` 与 Core `CleanupManifest`。Ready 的两份 manifest 只作为 normal publication source evidence，不能授权或替代 permanent cleanup manifest；两个 source 不能降级、互转或由 snapshot/projection 重建

`classify_back_target` 与 `classify_back_no_target` 必须显式接收同一 `world: &World`，registry 只能从该 World 读取，不得以独立参数传入。现有 `drive_navigation_system` 的 `World::resource_scope` 在持有 `RouterNavigation` entry 时仍可提供该只读 physical context

`classify_back_target` 只做 physical classification，不得调用任何 Core acceptance receiver。其 Ready 结果必须完整持有 prepared physical owner、raw `LifecycleContinuationCapacity`、已冻结 ECS `FrozenCleanupManifest` 与已构造 Core `CleanupManifest`；所有 manifest freeze/build 可失败工作在 acceptance 前完成。这两份 manifest 只证明 normal Ready source，不得被 permanent 分支当作 obligation authority。normal Ready scoped driver whole-take `ReadyAwaitingAcceptance` 后，必须在同一 `World::resource_scope` 调用栈和同一 entry borrow 内准备现有 registry lifecycle、claim rebind/root rebase、input gate、focus 与 presentation mutation plans；这些 plans 只能是栈上 borrowed capabilities，不能存入 `RouterEntry` 或成为第二 lifecycle owner/publisher。plan preparation 失败保存 `ReadyPlanRetry { whole pre-accept owner, PreparationRetryEvidence }` 或 `ReadyPlanPermanent(PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>)`

plans 成功后必须在同一 borrow 内连续完成 Ready acceptance → 安装并 whole-take `ReadyAccepted` → ledger reserve → 安装并 whole-take `Prepared` → bridge validation → existing publisher，不得跨 schedule boundary 或 yield。acceptance mismatch、reserve failure或bridge failure必须先零写 drop borrowed plans，再保存现有完整 stage owner；reserve failure 使用 `ReadyReserveFailure` 保留 physical、prepared continuation、Core manifest 与按值返回 frozen manifest/kind 的 `CleanupLedgerReserveError`，不得重建 capacity/manifest 或自动重试。normal success 不得留下 `ReadyAccepted` 或 `Prepared`

`Prepared` whole-take 后才能调用唯一 consumer `prepare_back_target_commit`。该 bridge 在同一 Core receiver 内原子消费 continuation owner、real `CommitTicket` 与 Core manifest，连续完成既有 `prepare_commit` 与 `validate_publication`。`Mismatch` 必须用返回的 continuation + manifest 与 preserved physical/ledger owner 重组并保存完整 typed mismatch owner；`PreparationFailed` 必须把 preserved physical/ledger owner 与 Core typed failure 整体安装为 `CommitFailure`，只由 dedicated failure/authority consumer 继续，未实现时 public zero-write safe-stop；`Validated` 后 existing publisher closure 只能 apply 已准备 borrowed plans并绑定既有 cleanup batch，atomic tail 不得 allocation、query、validation、重建 plan、冻结 manifest 或重建 owner

`BackTargetCommitPreparationFailure` 只能由持有真实 post-error Core state 的内部 operation 分类并接管 continuation + engine-held capability：pre-mutation error 返回绑定原 continuation/manifest 与仍由 engine 持有 capability 的 `RetainedInput`，真实 failure cleanup 返回其 Core owner，prepare/validation 后 retained rejection 返回持有 sealed publication/capability 的 `RejectedPublication`。integration wrapper 不得吞掉 `IntegrationError` 后猜测 `RouteFailure | AuthoritySettlement`。classifier `PendingRejected | AuthorityLost` 在 acceptance 前分别保存原 `TargetPreparationOwner` + `LifecyclePreflightEvidence`，acceptance Mismatch 必须按 branch 保存重组 owner + 原 classifier evidence + opaque mismatch；Matched 后改为只持 splice/history 的 classified physical owner + converted `RouteFailureContinuationOwner | TargetAuthoritySettlementOwner` + evidence。这两支不得复用 normal Ready 的 focus set 或 `PreparedCleanupLedger`，其 dedicated consumer 后续分别准备正确 failure/authority scope。Retry 保留 raw owner + retry evidence，Permanent 保留 converted outer owner，任何未接线分支都不得误入普通 chain、丢失 physical owner、产生 ownerless `Consumed` 或新增 publisher

两个 post-splice producer 只能分别通过 branch-specific adapter 收敛：classifier adapter whole-destructure 原 source，把 actual `BackTargetClassifiedPhysicalOwner { splice, history }` 与 evidence 按值留在 classifier branch，只有其 continuation 交给 exact acceptance；Ready-plan adapter 按值保留 existing `PreparedBackTargetPhysicalOwner`、normal Ready `FrozenCleanupManifest`、Core `CleanupManifest` 与 evidence，只有其 continuation 被 exact acceptance 消费。两种 adapter 都必须 whole-own actual splice 中的 preserved + leaving current 与 entering pending，不得只保存验证 projection。每个 adapter 都从同一 `world: &World` 与 exact current `RouterEntry` freeze Entity target observations、每个 protected closure、allowed write set、source/router/host/request/transaction facts；验证 lineage projection 不能替代 branch 中的 actual splice/history owner。Ready-plan refreeze 必须与已有 normal source facts exact equality，不得丢弃重建。physical/entry mismatch 或 Core receiver mismatch 均 zero-write 返还整个原 typed source + typed evidence

两个 adapter 只可在 branch-specific exact preparation 与 exact `accept_back_target_authority_lost` 完成后产出同一种线性 `TargetPreparationPermanentDisposition`。同一 settlement 包含 splice current/pending failure-isolation 与 `HostLease → RetiredHostAuthority` 授权的 exact top-slot/host-child edge 两个子域。adapter/begin 前由 projected failure-isolation claims 独立构造 dedicated ECS `FrozenCleanupManifest`，独占 splice 内全部 exact router-owned Entity 与单一 top-slot obligation；Core `CleanupManifest` 只持 logical roots 与对应 digest/count binding，由 batch/completion exact 验证，Entity identity、attempt 与 receipt 不进入 Core。app-owned host 与 foreign Entity/component 保持 protected；host 不进入 `RouteEntityRegistry`、无 `RouteOwnershipSeal`，但保留 `RouterHostRegistration` 与其他 frozen host facts，除 exact `Children` edge effect 外不变。allowed write 分开记录 splice targets 与 host-edge effect，共享 slot 只产生一个 exact obligation。cleanup 持有 actual current/pending 与 exact host/top-slot claim；normal Ready frozen manifest + normal Core manifest 只参与 source equality，不能替代 dedicated ECS manifest或Core binding

唯一 dispatch 在 mutation 前验证 holder vacancy、exact current entry、same-World partition，并从 `RouterEntry.host` 线性取得包含 app-owned host、router-owned top-slot exact claim 与 epoch 的 `HostLease`，同时预备 target-specific ledger wrapper 与 ECS `RetiredHostBinding`。dispatch 把 settlement + dedicated manifest + HostLease 移入专用 Core begin owner；Core 先完成全部 receiver、request/transaction/manifest、unique batch、cleanup 与 terminal 可失败准备，再把 `retire_current(HostLease)` 作为最后可失败步骤。此前任何失败均 callback 零次并返还完整 HostBinding 与 whole owner；退休成功后不得恢复 live lease。callback 恰好一次、并在首个 Core/private/public write 前安装 `{ cleanup, Prepared target ledger, unique Core batch, RetiredHostBinding }`；Core assignment-only cleanup 同时保留 `RetiredHostAuthority` 并进入 public Recovering。holder ledger 显式区分 `Prepared | Active(Current | Isolated)`；Core 不复制 batch

cleanup driver 只能在 whole-take `Installed` 后单次把 target-specific `Prepared` 转为 `Active(Current)`；Active wrapper 持有实际 `CleanupLedger`、actual current/pending owner、`RetiredHostBinding` 与同一 target-specific partition validation facts。首个 physical tick 只允许 Current isolation：current frame/slot 与 top-slot registry 同 tick 转 `EntityLifecycle::Leaving`/settlement lifecycle，其中 top slot 的 registry `EntityLifecycle::Leaving` 对应物理 `ProvisionalSlotLifecycle::Leaving`，不得以 `Quarantined` 代替；该 tick 完成 exact registry claim rebind、shared-slot single-obligation normalization、input/focus isolation与每次 observer write 后 exact refreeze，entering pending 保持 `PreparedInert`，但不 detach top-slot/host `Children` edge，attempt、despawn 与 receipt 为零，exact 后才进入 `Active(Isolated)`。只有 Isolated 可执行 receipt/attempt/settlement；exact host-edge detach 只能由 `Active(Isolated)` 后续 cleanup settlement/attempt 执行，随后按既有规则 exact refreeze 并生成 receipt；top slot 单次结算，app-owned host 始终存活、不进入 `RouteEntityRegistry`、无 `RouteOwnershipSeal`、不 despawn，并保留 `RouterHostRegistration` 与其他 frozen facts，除 exact `Children` edge 外不变。任一 mismatch whole-restore cleanup + Active ledger + batch + retired host，且不得恢复 live lease。physical completion 持有 cleanup + Active(Isolated) ledger + RetiredHostBinding；Core completion只验证 logical root/digest/count binding，不持有 Entity/attempt/receipt。exact callback共同消费 ECS `RetiredHostBinding` 与 Core `RetiredHostAuthority`，随后才进入 `AwaitingHost` 并清空全部 owner

### 4.4 Typed preparation outcome

任何 authority/preparation boundary 都必须保留四类 typed outcome

| Outcome | Owner result | World/history result | Next step |
| --- | --- | --- | --- |
| `Ready` | 单次移动到prepared plan | 尚未publication | 进入commit preparation |
| `PendingRejected` | 原owner转为typed pending cleanup owner | old active/history不变 | exact pending cleanup后按continuation继续 |
| `AuthorityLost` | 原owner转为typed authority settlement owner | commit前history不变 | existing scoped settlement |
| `RetryablePreparationFailure` | 原capability/candidate、boundary evidence、reservation与已存在的continuation owner完整返还 | 严格零写 | 只由同一named producer重试 |

这四项是 `PreparationDecision<Ready, Owner, PermanentOwner>::Classified(LifecyclePreparationOutcome<Ready, Owner>)` 内的四种物理处置；外层 `Permanent(PermanentOwner)` 持有分支专用的 move-only permanent owner。`Permanent` 不是第五种物理 outcome，而是在没有 live producer 可证明未来可完成时线性持有原 authority 与 exact terminal evidence 的不可重试决策。

no-target 不直接使用该 generic outer outcome，因为 `AuthorityLost` success 会同时消费原 `BackPreparationOwner` 并产生独立 authority settlement disposition。它使用 specialized outcome：`Ready(PreparedBackNoTarget)`、`Retry { original owner, evidence }`、`AuthoritySettlement(BackNoTargetAuthoritySettlementDisposition)`、`Permanent(BackNoTargetPermanentOwner)`，以及消费前返还完整原 owner 的 typed issuer/receiver mismatch。该 specialized outcome 没有普通 `PendingRejected` variant；不可达的 classifier `PendingRejected` 直接转换为 `Permanent`。authority disposition 线性持有 Core settlement owner、ECS capacity retirement evidence、冻结 complete source/protected relationship closure、allowed-write set 与 exact Entity manifest 的 physical settlement owner，以及分类时独立构造的 obligation-only `BackNoTargetAuthorityManifest { cleanup: CleanupManifest }`

只有可证明未来能满足的容量或尚未finalized boundary facts可以产生 `RetryablePreparationFailure`

允许的 named producer 必须显式记录为 `RetryableCapacity | UnfinalizedBoundaryFacts`

| Exact evidence predicate | Typed producer | Original owner | Retry trigger |
| --- | --- | --- | --- |
| finite preparation/scratch capacity不足，且evidence绑定同domain的exact live reclaim owner与release generation | `RetryableCapacity` | capability/candidate/reservation与该分支已存在的continuation完整返还 | 只有reclaim owner推进或release generation改变后 |
| boundary facts尚未finalize，且evidence绑定同request/attempt/host epoch的exact outstanding action或ticket | `UnfinalizedBoundaryFacts` | capability/candidate/reservation与该分支已存在的continuation完整返还 | 只有该action/ticket状态改变后 |
| 无exact reclaim owner、无outstanding ticket、identity/domain永久耗尽或source drift | none，permanent branch classification | 按typed failure/authority transition单次消费 | never |

同一个 case 只能命中一行，capacity 与 boundary evidence 不得合并为通用 retry

永久 identity/domain exhaustion、source drift、owner/seal mismatch 与结构矛盾必须有界 fail closed

no-target 不可达的 `PendingRejected`、source drift 或永久 capacity/domain contradiction 都必须产生 move-only permanent owner。source refreeze 只能由 Core 完成：Core 消费 verified opaque candidate，从当前 authority exact 重建完整 source，返回原 verified candidate 或公开但 opaque 的 typed drift owner/evidence；ECS 不得引用 Core-private `FrozenHistorySource` 或检查 candidate 内部，只能把 drift owner 与原 `NoTargetCapacityPermit` 线性配对

source-drift settlement 只暴露一个 ECS helper。它在同一个 exact-entry borrow 内保留完整 `{ raw drift owner, NoTargetCapacityPermit }`，先完成 issuer non-mutating retirement preflight，再把 locally infallible retirement closure 交给 Core exact drift consumer。issuer mismatch 直接返还完整 composite；wrong engine 调用 closure 零次并返还 raw drift owner，helper 与原 permit 重组；exact receiver 调用 closure 恰好一次并立即产生 terminal evidence。不存在可跨 entry/issuer 移动的 prepared settlement owner 或 two-phase API。该路径不发布正常 no-target rejection/cleanup trace，terminal 后 ownerless且永不 retry

retry 前重建完整 source 并要求 exact equality，drift 时重新分类，不能沿用旧 `RetryablePreparationFailure`

History 不新增 generic retry producer

resolved no-target 的 reservation 是真实、manifest-neutral 的 ledger header/domain/storage slot owner；在 proven manifest 之前不得声明 zero ledger 或分配 manifest，且构造 `ProvenEmptyProjection` 的 bounded scratch capacity 或 boundary finalization 仍可返回上述 typed retry

`ProvenEmptyApprovalView` 只能由 Core typed producer `approve_back_no_target_logical_zero -> BackNoTargetApprovalOutcome` 签发：producer exact-match 当前 engine 与 verified candidate，从当前 authority 重建完整 source 并验证 logical no-predecessor/zero；Mismatch 返还原 candidate + opaque `PreparationReceiverMismatchView`。ECS 不能构造、clone 或合成 approval；approval 只校验 Core logical facts，不能重建 owner，也不能替代 ECS physical `ProvenEmptyProjection`。`PreparedBackNoTarget` 必须同时持有 Core-issued `ApprovedBackNoTargetCandidate`、原 permit 与独立 physical projection，缺任一都不能进入 Ready

proof 与 Core approval 成功后，唯一 `consume_prepared_back_no_target` helper 在同一个 exact-entry borrow 内先用 ECS 私有 completion 算法计算 opaque `expected_completion_digest`，并连同 canonical manifest digest/count 对同一 outstanding issuer reservation 做 non-mutating exact preflight，再把三项 commitment 与 locally infallible commit closure 交给 Core exact Ready consumer。Core 不重算或解释 ECS digest，只把 expected value 与原 Back resolution、cleanup batch、request、transaction、host lineage 一起冻结。issuer 或 approved-composite receiver mismatch 调用 closure 零次并返还完整 approved candidate、permit 与 physical projection；exact match 调用 closure 恰好一次，把 reservation 消费为 preflighted manifest-neutral canonical-zero slot，但不公开状态，只返回 sealed move-only `PreparedBackNoTargetPublication`。该 prepared owner 持有 Core publication capability与 opaque `BackNoTargetCleanupInstallation`；installation 私有地整体持有 cleanup batch、rejected-back continuation 与 cleanup action，ECS 只能从 read-only `BackNoTargetCleanupBatchView` 取得同一 manifest commitment 与 frozen expected completion digest

仍在同一个 helper/entry borrow 内，prepared publication 的 doc-hidden final consumer 以唯一 ECS installation closure 接收整个 `BackNoTargetCleanupInstallation`，仅用其 batch projection 不可失败地消费 captured canonical-zero slot并构造 fully bound ledger；ledger 保存 batch view 中的 frozen expected completion digest，并使用产生该 expected value的同一 ECS 私有算法结算实际 completion digest。closure 再将 `{ ledger, installation }` 作为 `PendingBackNoTargetCompletion::Installed(whole installed owner)` 安装到 exact `RouterEntry`；不得拆出 batch/action/continuation。只有整个 holder 安装完且 closure 返回后，Core 才发布 `HistoryIntentRejected → CleanupPending`。首个 `HistoryIntentRejected` 观察点必须已经能从 exact entry 找到这一 whole owner。final consumer 不得 query、allocation、validation、hook/observer-triggering write 或执行可失败工作；即使 obligations 为零也不再分配 header/domain/storage，不存在额外 `BindingView`、attestor 或 receipt protocol

publication closure 在同一 schedule 安装 canonical live `Installed { inert: None }` holder，随后 `settle_cleanup_system` 在每个 update 分类 holder，但 zero-completion driver 只整体 take 并驱动 live `Installed` 一次。它证明 ledger 的 exact-zero settled obligations，计算 manifest digest/count 与实际 canonical completion digest/count，再单次消费 installation 形成 no-target 专用 `BackNoTargetCleanupCompletionOwner`。四项 completion facts 只做结构绑定，identity 继续由 whole installation 内原 Back resolution、cleanup batch、request、transaction 与 host lineage 唯一持有，不复用普通 `CleanupCompletion` 或伪造 frame/task identity

ledger invariant rejection 必须恢复 `Installed { inert: Some(LedgerInvariant(BackNoTargetZeroCompletionPreparationEvidence)) }`；issuer 缺失与 reclaim rejection 分别恢复 `IssuerMissing { source }`、`ReclaimRejected { source, error }`，三者都保留完整 installed owner且不得调用 Core。ECS exact-zero proof 后必须对 ledger 内 installed capacity lease 完成 exact reclaim preflight，预建 `Leased -> Free + generation+1` 或 generation 耗尽时永久退役的不可失败 transition。Core 在 callback 前完成 receiver、全部 completion facts 与 terminal record/state 的可失败准备；receiver mismatch 或 wrong facts 调用 callback 零次并返回完整 completion owner + opaque mismatch view，ECS 丢弃未提交 reclaim guard，再恢复直接持有 whole `PreparedBackNoTargetZeroCompletion`、completion owner 与 mismatch 的 `PendingBackNoTargetCompletion::Settled`。inert `Installed` 与 `Settled` 都在 take 前返回并阻断 collect，后续 update 只分类、不重驱；只有未来具名 Recovery/authority trigger 可以消费，当前 History 不新增 completion retry producer

exact receiver 调用 callback 恰好一次且 callback 只能执行预建 reclaim commit，随后 Core 才发布 `CleanupSettled -> Failed(BackTargetUnavailable) -> RequestTerminal`；callback 后不得 query、allocation、validation 或执行其他可失败工作。exact terminal 后 pending holder、batch、action、continuation、capability、candidate、reservation、completion、capacity lease 与 lifecycle owner 全部为空

proven-empty 前的 `AuthorityLost` 也必须在分类所在的同一个 exact-entry helper 内先冻结 complete source/protected closure、allowed-write set 与 exact Entity manifest，形成 ECS physical settlement owner，并独立构造只包装 Core `CleanupManifest` 的 authority manifest，再做 permit retirement preflight并由 Core exact accept receiver 以同一 closure 规则完成：两份 manifest 都只冻结 obligation identity，不冻结未来 receipt disposition 或 completion expectation。no-target 不持有 `LifecycleContinuationCapacity`，不得构造或重建 `PreparationBoundaryScopeView`；Core 只 exact-match opaque candidate 内建的 engine/world/router/history/request/transaction/batch/source lineage，ECS physical authority evidence 只与冻结的 physical owner 配对。Mismatch 调用零次并返还完整 candidate、permit、physical owner 与 authority manifest；exact match 调用恰好一次提交 capacity retirement，建立唯一 Core-private `pending_back_no_target_authority::Accepted`，再产生整体 `BackNoTargetAuthoritySettlementDisposition { physical, settlement, retirement, manifest }`

该 disposition 只有一个 exact-entry dispatch consumer：它先把 exact `HostBinding` 转为 `HostLease`，在 ECS 侧准备独立 `RetiredHostBinding` 并预留全部 physical partition、ledger 与 whole-holder plan，再以 settlement + host lease + unchanged authority manifest 构成 begin owner。begin 的 receiver mismatch、真实具名容量不足、host admission retirement rejection 或同 receiver 的 host/manifest/pending-state 永久矛盾都返还 whole owner且callback零次；ECS 用返回的 HostLease 恢复 HostBinding并重组原 disposition。只有真实 receiver mismatch 携带 `PreparationReceiverMismatchView`，只有可证明未来释放的容量不足为 `Retryable`，其余上述矛盾为 typed `Permanent`

begin callback 前，Core 必须在 whole begin owner 仍持有原 `HostLease` 时完成全部可失败 receiver/admission 校验，并局部准备 Cleanup receiver projection、纯 Core batch、record/queue/tombstone/trace/retention 与 terminal plan。`retire_current(HostLease)` 是最后一个可失败步骤；Err 返还 HostLease、丢弃局部 preparation并重组 whole begin owner，callback/private/public 均零写。Ok 局部产生 Core-only `RetiredHostAuthority`，此后只允许 callback 把唯一 batch 与 ECS owners 安装为 `Installed`，再 assignment-only 安装 private `Cleanup { receiver, RetiredHostAuthority, terminal }` 与 public `Recovering`。private state 不持有 ECS batch，post-retirement tail 不得 query、allocation、validation、hook write 或保留 panic path

dedicated cleanup 每个 update 先分类 holder，只 whole-take `Installed` 一次；exact receipts 后计算 opaque actual completion digest、预建完整 physical completion disposition，再单次生成 dedicated completion owner。Core 只 exact 校验 outstanding Cleanup lineage、manifest digest/count、`settled_count == obligation_count` 与 completion structural binding；actual digest 只绑定该 completion 并留痕，不与任何 preclassification value 比较。wrong facts 或 receiver mismatch 调用 callback 零次并返还 whole completion owner及 typed mismatch evidence，ECS 恢复 `Settled { prepared physical completion, returned completion owner, mismatch }`。`Settled` 为 inert safe-stop，后续 update 在 take 前返回且不重驱；completion 不产生 retry/permanent outcome。exact callback 只提交预建 physical completion并清除 whole holder，之后 Core 消费 private Cleanup 中的 `RetiredHostAuthority`、assignment-only 发布原始 AuthorityLost terminal并清空 pending state。该 adapter 只能复用 existing scoped settlement 的内部纯 preparation/assignment 与 ECS physical primitive，不得复用或构造 current、FrameTask、FailureCleanupRequest、ordinary `CleanupCompletion` 或其公共 lifecycle path，也不得记录 `HistoryIntentRejected` 或 normal `CleanupPending`

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

`NoTarget` 只表示 frozen history 结构合法且 current cursor 没有 predecessor

entry identity/location 长度不等、cursor 越界、nonempty entries 缺少 cursor、cursor current entry/location 不一致等情况不是 `NoTarget`，必须携带 expected/observed source 形成 typed permanent contradiction，零写并有界 fail closed

running resolution verify 后，no-target 停在 Core-private verified-candidate-owned 状态；candidate 是唯一 opaque unresolved lifecycle lineage owner，不是 continuation owner

normal candidate 与 Retry 在 physical proven-empty + Core logical-zero approval + canonical-zero exact-bind 完成前不得发布 request phase、`HistoryIntentRejected`、`CleanupPending`、ledger、manifest、receipt、continuation、cleanup action 或 terminal，也不得由 `requested=None` 推断状态

canonical-zero bind 后返回的 `PreparedBackNoTargetPublication` 仍保持 public 零写；只有 exact entry 已整体安装 `PendingBackNoTargetCompletion::Installed(whole installed owner)` 后，final consumer 才能产生第一个 `HistoryIntentRejected`，且 installation 的 batch/action/continuation 不得拆开

pre-proof authority loss/permanent fail-closed 仍可进入既有 settlement/terminal，但不得伪装成正常 no-target rejection 或发布其 cleanup trace

single-queue no-target 的 `PendingRejected` producer 不可达；共享 enum caller 仍必须穷举该 variant，并立即 typed fail closed，不能创建 pending cleanup 或公开 trace

ECS 必须产生 typed `ProvenEmptyProjection`

它至少来自

- exact host 与 active claims
- request-attempt owner inventory
- registry exact mappings
- 完整 observed relationship closure
- explicit role partition
- 所有 existing current 明确归入 protected
- attempt target 为空且与 protected 不相交

完整 observed closure 不等同于 registered branch exact child set：unregistered app-owned/foreign 和明确 other-authority non-target descendant 归入 protected，这类合法 extras 本身不阻止 zero-obligation proof，exact anchor/claim/lifecycle/relationship invariant drift、cycle 或 unexpected same-authority Router-owned Entity 仍按 Matrix fail closed

registered extra 必须按固定顺序分类

1. 先冻结并校验 exact registry claim/owner、record lifecycle、registry manifest ⊆ physical manifest、seal、role-specific lifecycle/authority 和 observed parent/relationship
2. 任一 physical fact 漂移立即返回 `BackNoTargetPhysicalFreezeOutcome::Indeterminate`，携带 registered-extra-specific typed cause 与完整 expected + observed physical projection，不调用 authority comparator
3. physical facts 全部 exact 后才执行 same/other authority comparator；无法取得 opaque authority relation 时 cause 必须为 `OpaqueAuthorityRelationUnavailable`

| owner kind | exact physical prerequisites | authority classification |
| --- | --- | --- |
| committed `FrameShell` / `FrameContentTarget` / `FrameOutput` | exact registry claim/owner/record lifecycle、registered manifest ⊆ physical manifest、seal、`FrameLifecycle`、observed parent/relationship | 使用 Core `frame_versions_share_router` 比较 owner frame 与 current authority |
| committed/provisional `ChildSlot` | exact registry claim/owner/record lifecycle、registered manifest ⊆ physical manifest、seal、`ChildSlotLifecycle`、`ChildSlotAuthority`、observed parent/relationship | 使用 owner parent frame 与 Core frame comparator；不得从 anchor、binding 或 relationship 重建 owner |
| committed `HostTopSlot` | exact registry claim/owner/record lifecycle、registered manifest ⊆ physical manifest、seal、matching `ProvisionalSlotLifecycle`、observed parent/relationship | 仅使用 Core doc-hidden `host_version_shares_router_with_frame` 比较 opaque `RouterIdentity`；不要求 `HostId` 相同 |
| provisional `HostTopSlot` | exact registry claim/owner/record lifecycle、registered manifest ⊆ physical manifest、seal、matching `ProvisionalSlotLifecycle`、owner candidate revision、observed parent/relationship | 即使 physical facts exact 仍返回 `BackNoTargetPhysicalFreezeOutcome::Indeterminate` + `OpaqueAuthorityRelationUnavailable`；revision 仅是 observed owner fact，禁止据此反查 authority |

exact same-authority extra 返回携带完整 observed authority projection 的 `UnexpectedSameAuthorityDescendant`，exact other-authority extra 进入 protected 并保留同一完整 projection；same-authority、physical drift 与 provisional Indeterminate 都返还完整原 `BackPreparationOwner`，other-authority `ProvenEmpty` 移动同一原 owner

任一 registered extra 都不得成为 cleanup target 或 authority owner，也不得用于重建 owner/target，`authorized_targets` 始终为空

logical roots 为空不能推导 Entity obligations 为零

retained-empty、slot-only 或其他稀疏 topology 也不能绕过 proven-empty

slot-only 指合法 branch/lifecycle partition 中 slot Entity 独立按 exact role、binding 与 obligation 分类，即使该 partition 没有 frame obligation；不得据此构造 zero-frame standalone current branch

只有 proven-empty 且 authority valid 后，才允许不可失败配对转换

```text
raw BackNoTargetCandidate + BoundBackCapacityReservation
-> Core exact accepted-lineage validation
-> VerifiedBackResolutionHandoff
-> accepted view + VerifiedBackNoTargetCandidate
-> VerifiedBackNoTargetCandidate + recomposed BoundBackCapacityReservation
-> begin helper preflights entry-owned issuer transfer, then Core exact receiver calls the infallible transfer commit once
-> Core consumes verified candidate and exact-refreezes source; mismatch calls transfer zero times and restores the full bound branch
-> unchanged verified candidate + same permit
-> ECS constructs physical ProvenEmptyProjection
-> Core exact approval producer validates receiver + candidate + current logical source/no-predecessor
-> Core-issued ApprovedBackNoTargetCandidate + same permit + distinct physical projection
-> consume helper preflights canonical-zero, then Core exact Ready receiver calls bind once
-> Core returns sealed PreparedBackNoTargetPublication with publication capability + whole opaque BackNoTargetCleanupInstallation and public zero-write
-> final consumer invokes the sole ECS closure; it reads only the installation batch projection, consumes the captured canonical-zero slot into the fully bound ledger and installs PendingBackNoTargetCompletion::Installed into exact RouterEntry
-> installation closure returns
-> HistoryIntentRejected
-> CleanupPending
-> same-schedule settle classifies the canonical holder and takes live Installed once, proves exact zero, freezes manifest/completion digest+count and preflights exact installed-lease reclaim
-> whole installation becomes the no-target-specific BackNoTargetCleanupCompletionOwner while the reclaim guard remains uncommitted
-> ledger/issuer/reclaim permanent rejection restores inert Installed; Core receiver/wrong-facts rejection calls reclaim zero times and restores inert Settled; either blocks collect without per-update retry
-> exact Core completion compares actual digest with the Ready-frozen expected digest and calls reclaim once before the prebuilt terminal tail
-> CleanupSettled
-> Failed(BackTargetUnavailable)

source drift 分支由单一 helper 保留完整 raw composite，先做 issuer non-mutating exact preflight，再由 Core exact receiver 以 closure 0/1 次调用完成 locally infallible retire + terminal；不存在 prepared owner，不进入 proven-empty、ledger 或 publisher
```

不得直接使用无证明的 `FrozenCleanupManifest::empty()`，不得伪造 `FrameTaskIdentity`

若 proven-empty 前发现 pre-commit authority loss，消费 no-target candidate、accepted reservation lineage 与 capacity reservation，同时冻结 complete physical settlement closure/allowed-write/exact Entity manifest 与独立 obligation-only `BackNoTargetAuthorityManifest { cleanup }`，形成 `{ physical, Core settlement, retirement, manifest }` 整体 disposition。该分支没有 `LifecycleContinuationCapacity`，不得构造或重建 preparation boundary scope；ECS physical authority evidence 只与冻结的 physical owner 配对。唯一 dispatch 通过 dedicated accept → begin → Installed/Settled completion 驱动 assignment-only `Recovering` 与原始 AuthorityLost terminal，不记录 `HistoryIntentRejected` 或 normal `CleanupPending`

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
- actual completion digest 与 batch lineage 内 frozen opaque expected value exact equality
- completion structural binding

Core 不重算 Entity truth、ECS completion 算法或 receipt 的物理真实性

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

本矩阵中的 cleanup manifest 只冻结 obligation identity，不冻结未来 receipt disposition。receipt 结算后才产生 actual completion digest；canonical-zero 的独立 expected-digest batch commitment保持不变，no-target AuthorityLost 不建立对应预提交 completion commitment

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
| cleanup | Core logical roots + digest/count binding；ECS immutable exact Entity manifest + attempt/receipt |
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
| back | target exists at running | previous exact entry | 按 exact entry identity 原子提交 UI/history 后才移动 cursor；nested target 可先进入 mount handshake，duplicate/equivalent 可 zero-entering 且 physical zero，pure-truncate 可 zero-entering 但仍准备 exact leaving cleanup | cursor unchanged |
| back | target absent at running | none | not applicable | proven-empty 后 `BackTargetUnavailable` |
| back from Error presentation | published history decides | previous exact entry | normal Route commit，cursor moves | Error or old presentation follows normal failure rules，cursor unchanged |
| any command with current `None` | command规则决定 | normal Route target | first physical presentation，history按intent更新 | `None`保持或fallback Error |
| push/replace/back with current Error | history与intent决定 | normal Route target | Error不享受equivalent physical shortcut | failure按原Error/old state规则收敛 |
| lifecycle validation with `next=None` | unchanged | none | 无presentation/registry/cleanup/history physical mutation | unchanged |

`next=None` 与 `next=Some(equivalent route)` 是不同 case

Back target 的正常 action domain 是 `FrameTask | MountRequest | Commit`

nested entering 可以先产生 `MountRequest`；当前 slice 将 initial Ready owner 保留为 `ResumeBlocked::Start`，applied owner 保留为 `AppliedMountRequestPending`，均不进入 generic mount handshake，等待 target MountRequest action slice

mount issue exhaustion 独立产生 `FailureCleanupRequest`，不得伪装为 resume action

legitimate zero-entering reconciliation 直接产生 `CommitTicket`，不得为满足执行形状伪造 `RootFrameTask`；duplicate/equivalent 仍按 exact entry identity 更新 history，pure-truncate 即使没有 entering 也必须保留 leaving cleanup 的 exact owner、manifest 与 completion

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
| precommit authority loss | cancelled or unresolved owner consumed | target使用existing scoped settlement；no-target使用dedicated accept→begin→completion adapter | authority failure | settlement completion后active `None`/history old | HostLost/AuthorityLost | §4.4 exact evidence mapping或none | History invariant；no-target不得构造ordinary settlement authority |
| post-splice target permanent | unresolved owner转dedicated settlement | ECS Frozen manifest覆盖actual splice全部router-owned current/pending + top-slot obligation；Core仅logical roots/digest/count binding；app host protected | authority failure + retired-host authority | Current isolation后不可交互，exact cleanup后active/entry host `None`、history old | AuthorityLost后AwaitingHost | none | History Task 4 |
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

`MountedChildSlot` 的 exact parent/outlet/slot/anchor/claims 只锚定 support closure 复验；shell/content/output owner 仍来自 exact current/preserved `RealizedFrame`，registry projection 不得重建 owner

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
| accepted back owner | queued/running before resolution | verify Matched后与原capacity重组bound reservation，并与branch-specific verified owner配对 | wrapper只拆为accepted+verified，不能拆回raw handoff；foreign Mismatch返raw accepted+handoff |
| back resolution duplicate evidence | fixed `back_resolution`：non-Back与queued Back生成handoff前为None，action可见前安装`Some(Current(observed evidence))` | retained先exact比较supplied/stored；仅exact verify Matched原位转`Some(Consumed)`，Current不得terminal/evict | saved observed evidence按state为Current/ConsumedReplay，mismatch diagnostic为Foreign；仅Consumed可terminal/evict，evicted replay还需exact scope+双watermark；Matched除record state外Core/World/history/public零写 |
| target pre-splice permanent owner | Task 4-only Core `BackTargetPermanentOwner`；仅在 resume receiver preflight 通过后由 source/host/baseline、generation/depth 或 clone-local request contradiction mint | 不进入ECS splice、physical classification或cleanup，也不重建ECS owner；preflight逐项要求七个busy slot与current public request为空，drift whole-return verified owner且不得mint permanent | dedicated exact Core settlement按`Refreeze(destination.source, observed_source) | Capability(drift.capability.source, drift.observed) | AuthorityLost(cause, destination.source)` typed source variant校验current source、完整observed baseline与exact retained queued consumed request；Refreeze是当前真实production producer，Capability是仅由最小source-fact seam覆盖的defensive unreachable recheck；mismatch返还whole owner + typed evidence且zero-write，exact receiver terminal一次；terminal projection保留stored cause/source与完整expected/observed baseline且不能重建owner |
| target post-splice classifier permanent source | whole `PreparationPermanentOwner<TargetPreparationOwner>`，包括actual preserved + leaving current与entering pending | classifier adapter用同一World/current entry冻结exact target/protected/allowed-write/lineage，再由exact `accept_back_target_authority_lost`产生settlement；mismatch返whole source | 只在branch-specific preparation后转共同disposition；不可变成Ready-plan或pre-splice owner，也不能以projection替代actual splice owner |
| target post-splice Ready-plan permanent source | whole `PreparationPermanentOwner<BackTargetReadyAcceptanceOwner>`，包括actual preserved + leaving current与entering pending；normal Frozen/Core manifest仅为source evidence | Ready-plan adapter exact refreeze已有normal source facts，再冻结partition/lineage并exact accept；mismatch返whole source | 只在branch-specific preparation后转共同disposition；不可把normal manifest当作permanent obligation authority、丢弃重建或变成classifier source |
| target permanent disposition/holder | actual current/pending + splice targets/host-edge effect双allowed-write域 | ECS Frozen manifest独占全部exact splice Entity + top-slot obligation；Core manifest仅logical roots与digest/count binding，Core无Entity/attempt/receipt | Current首tick令current frame/slot/top slot转Leaving/settlement lifecycle并exact rebind/refreeze，pending保持PreparedInert且零attempt；仅Isolated settle。app host不进registry、无ownership seal，registration等事实保留；双retired-host owner完成后才AwaitingHost |
| no-target candidate | verified unresolved lifecycle lineage，等待physical proven-empty + Core logical-zero approval + canonical-zero bind；没有`LifecycleContinuationCapacity`，不得构造或重建preparation boundary scope | authority loss后Core只exact-match opaque candidate内建lineage，ECS physical authority evidence只与冻结的physical owner配对，再转为`{ physical, settlement, retirement, BackNoTargetAuthorityManifest { cleanup } }`整体 disposition；manifest只冻结obligation identity；retry/mismatch原样返还且public零写 | bind前不能mint normal continuation/action或发布normal cleanup |
| no-target authority accepted/begin/cleanup | Core private pending保存`Accepted → Cleanup { receiver, RetiredHostAuthority, terminal }`且不持有ECS batch | HostLease留在whole begin owner内完成全部fallible preparation；`retire_current(HostLease)`最后执行，Err返HostLease并重组whole owner，Ok后仅callback安装`Installed`→assignment-only Cleanup/`Recovering` | exact receipts提交actual digest；mismatch恢复inert `Settled`，exact terminal消费Core retired-host owner |
| no-target logical-zero approval | Core exact receiver + verified candidate + current source/no-predecessor validation单次签发，与physical projection分离 | mismatch返原candidate；ECS/facade不可构造或用其重建owner | 只与原candidate组成approved composite，不能单独授权Ready |
| no-target source-drift owner | Core exact refreeze消费verified candidate后opaque返回，与原permit配对 | 单一entry helper先做issuer preflight，再调用Core exact consumer | engine/issuer mismatch时closure零次且返还完整raw pair；exact时closure一次并terminal，不存在prepared owner |
| prepared no-target publication | canonical-zero bind后的Core exact Ready consumer单次mint，持有publication capability + whole opaque cleanup installation，public零写 | final consumer的唯一ECS closure只读batch projection并把whole installation与ledger安装到exact entry | closure返回后才发布；final tail不可失败且不再validation/query |
| pending no-target completion | exact entry以单一holder持有live/inert `Installed` 或 whole `Settled { prepared, completion, mismatch }`；batch/action/continuation保持私有整体 | settle每个update先分类holder，只take live `Installed { inert: None }`一次；typed inert `Installed` 与`Settled`在take前返回并阻断collect | 仅exact Core completion单次terminal并清空holder与所有publication/completion owner；未来具名Recovery/authority trigger才可消费inert holder |
| rejected-back continuation | whole cleanup installation内部持有并在首个public write前整体安装，与zero ledger正交持有 | 不因count/disposition重分类或从installation拆出 | exact zero completion owner由whole installation单次转换并消费 |
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
| Back execution | admission stamp、capacity reservation、verified late target、typed retry、success与pure no-target | C1、no-target zero、slot-only与retained-empty protected proof、NoBackEntry precedence、normal no-target与post-splice permanent terminal owner closure | `RouterNavigation::back` | 两条Task 4 terminal owner-closure gate GREEN后public back可用，late-bound完整证据仍待C3两分支 |
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
| `NavigationRequestSnapshot::requested -> Option` | Back execution | active | Back execution public contract PASS |
| `NavigationEnqueueErrorKind::IdentityExhausted` | Replace | active | allocator-derived public RED/GREEN PASS |
| late-bound back complete evidence | Back failure | active | complete C3 two-branch PASS |

`State` vocabulary 固定为 `planned | active`

item 必须唯一，未知 state、重复 item 或不在最终 Fixture/Golden/Journey 的 item 一律拒绝

public method 是否仍 excluded 只由 10.1 持有，不在本表重复

contract-delta harness 固定使用本节 exact heading 与四列表头，并校验 active item 已出现在当前 Fixture/Golden 与 owning slice public evidence

### 10.3 History authority regression additions

| Test function | Owning slice | State |
| --- | --- | --- |
| `active_cycle_enters_authority_settlement` | Back failure | active |

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
- focused `back_target_receiver_mismatch_calls_zero_and_exact_receiver_calls_once` 必须分别改变 `action`、`current`、`pending_no_target_back`、`pending_back_no_target_authority`、`current_history_capability`、`pending_history_preparation_error`、`prepared_publication_authority` 与 `state.current_request()` 的真实 predicate input；每槽都 whole-return verified owner、callback 零次、ECS bound reservation 未退休且可重组，并证明没有 permanent owner，seam 不得注入 mismatch/permanent outcome
- focused `back_target_pre_splice_permanent_settlement_terminals_once` 必须覆盖真实 production `Refreeze` source drift permanent 在 foreign settlement whole-return 后 exact terminal 一次、至少一个 real `Host | ActiveBaseline` cause 与一个 real `Generation | MaximumDepth` cause exact settle；`Capability` 只由首次 refreeze 后、`freeze_back` 前改变真实 `CanonicalHistory` source fact 的最小 `cfg(test)` seam 覆盖，并必须经 production `freeze_back` predicate 产生 `HistorySourceDrift`；terminal evidence 必须按 typed variant 证明 `Refreeze`、`Capability`、`AuthorityLost(cause)` 的固定 source 映射、stored 完整 expected/observed baseline 与 owner conservation，seam 不得直接构造 owner、classification、permanent 或 terminal outcome
- focused `back_target_classifier_permanent_freezes_exact_world_physical_facts`
- focused `back_target_ready_plan_permanent_preserves_frozen_facts`
- focused `back_no_target_ready_terminal_closes_all_lifecycle_owners`
- focused `back_no_target_completion_callback_tail_is_infallible_and_query_free`
- focused `back_target_physical_permanent_terminal_closes_all_lifecycle_owners` 必须证明 normal Ready ECS/Core manifest 仅为source evidence；dedicated ECS Frozen manifest独占exact splice + top-slot obligations，Core manifest只含logical roots与digest/count binding且无Entity/attempt/receipt。`Prepared → Active(Current) → Active(Isolated)` 的Current tick令current frame/slot/top slot同tick进入Leaving/settlement lifecycle并exact claim rebind/postwrite refreeze，pending保持PreparedInert且零attempt/despawn/receipt；仅Isolated settle。成功begin后`entry.host=None`且ECS/Core持双retired-host owner，completion后才AwaitingHost。app host始终存活、不进RouteEntityRegistry、无RouteOwnershipSeal、不despawn，RouterHostRegistration等事实除exact Children edge外不变；exact terminal零owner
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
