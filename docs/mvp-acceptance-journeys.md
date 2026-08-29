# MVP Acceptance Journeys

> **Recovery activation status:** Journey D 的 R1 Error presentation 合同已通过；Journey G 仍为后续 Recovery 验收工作。background old-lease reap 与剩余完整 Journey G rows 仍属于 Authority closure；当前不是 Recovery 或 MVP 整体 PASS 声明。

**文档类型**: 外部行为验收规范
**状态**: Phase 3B.1 `Plan PASS`
**前置规范**: [Target Architecture](target-architecture.md)
**公共 fixture**: [Facade Contract Fixture](facade-contract-fixture.md)

## 1. 三层验收

MVP 验收按职责分为三层，不能互相替代：

1. **Facade journeys**：只通过 `bevy_ui_router` 的推荐公共 API 发起导航，在真实 `App::update` 中断言 UI、history 和生产 snapshot。
2. **Authority audit**：只通过 `test-support` 的只读 snapshot 证明 core frame 与 ECS Entity 一一对应，不获得 mutation capability。
3. **ECS regressions**：在 `#[cfg(test)]` 的 crate 私有单元测试中注入不可由生产 API 构造的错误身份、恶意 report 和 cleanup 异常。

Facade Journey PASS 是第一道门；authority 与负向回归也必须通过，三层全部通过才可能宣布 MVP PASS。

## 2. 公共装配与稳定状态

Facade journeys 统一采用：

1. 先用 `AppRouterExt::register_ui_view_driver` 注册 immediate 测试 driver。
2. 用返回的 `ViewDriverKey` 构造 view registry，再调用 `add_ui_router(RouterConfig::new(routes, views, fallback))`；该调用幂等安装 plugin 与 system chain。
3. 应用创建带非零尺寸 `Node` 的 app-owned host Entity并插入 `RouterHostRegistration`，调用 `App::update` 完成登记；host无 parent或其到 UI root 的每个实际ancestor也必须有 `Node`。
4. 测试 driver 只使用公开 immediate effect/report 和受限 content writer。
5. 通过公开 Resource `RouterNavigation` 发出命令，用公开 Resource `RouterSnapshots` 的 `request(router, request_id)` 查询 `RequestLookup`。
6. 最多运行 16 次 `App::update`；request 必须进入 `Succeeded` 或 `Failed`，否则测试失败并输出 phase trace。

Facade journey 的“稳定”只依赖生产观察面：该 router 没有 running request、没有待处理 foreground cleanup，生产 snapshot 不含 prepared/leaving frame。Entity 级事实由独立 authority audit gate 验证，不作为 `ecs`-only facade test 的依赖。

## 3. Facade journeys

### Journey A：首次嵌套 push

- **Given**：`/items/:id` 匹配 layout 与 leaf，layout 声明 `default` outlet。
- **When**：执行 `push("/items/1")`。
- **Then**：parent shell 先于 renderer 创建且保持 inert；公开 driver 为 layout 与 leaf 各计划至少一个 `spawn_empty` 结构container及其 `SolidRect` child，child effect 只在 parent outlet 登记后产生；commit 前所有 entering root与output不可见、不可交互、不持有 focus；commit 后 layout 与 leaf 恰为 active，两个 `AuthorityContentMapping` 均为 `SolidRect`、非零尺寸、`visibility_hidden=false` 且 `pickable_active=true`。ECS私有物理回归使用exact-feature `bevy_internal::DefaultPlugins` 一次性安装input/transform/window/camera+visibility/`UiPlugin`/`DefaultPickingPlugins`，复用并配置`WindowPlugin`生成的唯一`PrimaryWindow`且断言数量为1，只额外spawn一个`Camera2d`，不得重复添加`UiPickingPlugin`或第二个primary window；随后直接查询World确认host、top slot、shell、content target、outlet、child slot、structural output和primitive构成全Node直接ancestry，结构output透明且始终`Pickable::IGNORE`，primitive具有真实`Node + BackgroundColor + Visibility + Pickable`、递归required-component closure、非零layout和传播后的effective visibility，不是empty Entity或test-only node。history只有`/items/1`；app-owned host Entity、所有非-`Children`应用组件及既有child identities/relative order不变，`Children`只允许新增exact top slot membership。

测试 driver 读取公开 phase trace，必须观察到 parent applied 在 child effect 之前，并通过 frame origin/slot/generation accessors区分 parent/child。独立 authority audit 再证明两个 active frame version 各有且仅有一个 Entity。

### Journey B：二次叶子替换

- **Given**：Journey A 已提交。
- **When**：执行 `push("/items/2")`。
- **Then**：layout frame version、root Entity 和 outlet Entity 保留；旧 leaf 先隔离并释放 focus；新 leaf 使用 layout 当前 generation 的 outlet；稳定状态中保存的 old-leaf `AuditEntityId` 为 `Removed`，正常 cleanup receipt 精确记录 `Removed`；任何 phase trace 都不存在两个可交互 leaf；history entries/cursor 精确对应 `/items/1`, `/items/2`。

### Journey C1：replace 与成功 back

- **Given**：当前 history 为 `/items/1`, `/items/2`。
- **When**：执行 `replace(RouteLocation::new("/items/3"))`，成功后执行 `back()`。
- **Then**：两次命令都经过 match、plan、staged apply 和 atomic switch；replace 的 request snapshot 从 enqueue 起为 `requested=Some(/items/3)`；back 在 running resolution 后冻结为 `requested=Some(/items/1)`。replace 后 history 为 `/items/1`, `/items/3`；back 成功后 active/history current 均为 `/items/1`；每次都保留 layout、替换 leaf。

`NavigationRequestSnapshot::requested -> Option` — ACTIVE（Back execution；owning boundary 为 `Back execution public contract PASS`，当前只声明该 Back 执行公共契约通过，不声明 Back failure、History 或 MVP 整体通过）

### Journey C2：失败 back

- **Given**：独立 fixture 的 history 为 `/items/1`, `/items/3`，当前 active 为 `/items/3`，并为 `/items/1` 注入 build failure。
- **When**：执行 `back()`。
- **Then**：back queued 时 `requested=None`，running resolution 后冻结为 `requested=Some(/items/1)`，即使 route failure 或 fallback 后也保持该 resolved value。request 为 `Failed`；recovery 成功时 presentation 为 `Error`，否则 `/items/3` 保持 active；两种情况 cursor 都不移动，不能出现 history-only back。

### Journey C3：排队 back 的执行期重检

- **Given**：history 只有 `/items/1`, `/items/2`，同步 admission snapshot 允许 back。
- **When**：在一次 `App::update` 前连续入队两个 `back()`。
- **Then**：两个 queued request 均可观察 `requested=None`。第一个成功后 cursor 到 `/items/1`；第二个成为 running 时重新预览，因没有前项而以 `requested=None + Failed(BackTargetUnavailable)` 终结，不渲染 fallback，不改变 active/history，并留下 `HistoryIntentRejected` trace。该 no-target terminal 必须由 `ProvenEmptyProjection`、canonical zero manifest 与 exact zero completion 证明，不能由空 logical roots 推导。若第一个 back 因 view failure 失败，cursor 不动，第二个可重新预览并独立冻结 `requested=Some(/items/1)` 后尝试。

`late-bound back complete evidence` — ACTIVE（Back failure；`complete C3 two-branch PASS`）

`NavigationEnqueueErrorKind::IdentityExhausted` — ACTIVE（Replace；allocator-derived public RED/GREEN PASS）

`IdentityExhausted`、`NavigationRequestSnapshot::requested -> Option` 与 `late-bound back complete evidence` 已分别在 Replace、Back execution、Back failure slice 激活；这些状态只声明 History 公开合同 PASS，不声明 Recovery 或整体 MVP PASS。独立 facade consumer gate 从真实 allocator/domain capacity predicate 导出 `IdentityExhausted`；test seam 只能改变该 predicate 的输入事实。该错误不创建 request、request id、公开 queue/request/World 变化或 terminal，已消费 private identity 不复用。back 的 enqueue precedence `UnknownRouter -> availability -> NoBackEntry -> IdentityExhausted` 已由 Back execution public contract 固定。

### Journey D：view、mount 与 error recovery

公开 fixture 分别注入 match、build、apply、outlet 缺失、outlet 重复、foreign-session outlet 和 session-local relationship cycle 失败。duplicate/foreign-session/cycle 都是 parent plan-level `ApplyFailed`；public journey证明trace不含 `MountRequested`/`MountRejected`且旧presentation/history在recovery前不变。driver返回瞬间的World/registry零变化没有公开set间观察点，改由ECS crate-private plan regression在调用前后直接比较World/registry fingerprint证明，不能用recovery后的snapshot替代。missing outlet 的 parent plan 被接受，随后必须观察 `MountRequested -> MountRejected(MissingOutlet)` 和 `PendingRejected` recovery。每种公开失败还必须证明：

- 原 history intent 被取消，history 保持失败前状态。
- 原 active presentation 在 recovery commit 前保持可见。
- 最近绑定 boundary 或 router fallback 使用新的 recovery attempt 和完整 identity。
- recovery 成功后 snapshot 为 `ActivePresentation::Error` 且 request 为 `Failed`；core/ECS 对 error frames 一一对应。
- 成功 match 后进入 recovery 的 error presentation，其 `matched()` 与原 request 保存的 immutable fingerprint 一致。
- recovery失败则旧presentation保持active，所有attempt的inert roots被清理，request精确为`Failed(RecoveryFailed)`；原始failure与recovery failure均保留在trace/diagnostics。
- 稳定后下一次合法导航可以执行。

ECS crate-private regression 另外在 accepted entering output realization 后破坏 outlet authority，要求 `MountRequested -> MountRejected(InvalidOutlet)`、`PendingRejected` 且旧 active 不变；再破坏 preserved/current parent 的既有 outlet，要求 `MountRequested -> MountRejected(ParentAuthorityLost)` 后进入 `AuthorityLost` settlement、清空失效 active且绝不渲染 fallback。这样 plan writer 错误、pending mount 缺陷和现有 authority 丢失不会被同一个通用 `ApplyFailed` 掩盖。

match failure时对应`NavigationRequestSnapshot::matched()`与 committed error presentation 的 `matched()` 必须均为`None`；router snapshot的running摘要在该request terminal后清空，不能用来替代request记录或active presentation。mandatory fallback经顶层host outlet构建`RouterFallback` error frame。fallback成功后request按原始`Match` cause为`Failed`、history不推进、error presentation原子提交；fallback也失败时request为`Failed(RecoveryFailed)`，保留旧presentation，首次导航则保持`None`。

缺少普通 view 的 route node、无法解析的 route/fallback binding 和未注册 driver 都是 `add_ui_router` 的同步 `RouterInstallError` 合同，由 facade consumer test 验证；MVP 没有动态 unregister，因此不伪造成 runtime Journey D。

`boundary-bound error presentation complete evidence` — ACTIVE（R1 Error presentation；`Journey D public contract PASS`；只声明本 slice 的 public contract 与 owner closure 已通过，不声明 Recovery 或 MVP 整体 PASS）

### Journey F1：完全等价的零 entering 导航

- **Given**：目标location的path、search、fragment与active完全一致，matched branch也相同。
- **When**：执行等价 replace。
- **Then**：不产生 view effect，request 在系统链中直接提交；history 按 replace 语义更新，不遗留 pending。

### Journey F2：fragment-only叶子重挂载

- **Given**：active为 `/items/1#summary`，layout与leaf均已提交。
- **When**：执行 `replace(RouteLocation::new("/items/1").with_fragment("details"))`。
- **Then**：matcher/branch fingerprint保持相同，layout frame/root/outlet与child slot保留；leaf保留frame id但generation推进，公开driver恰收到一次fragment为`details`的新effect。新leaf按正常inert apply/atomic commit替换旧leaf，history current只在commit后变为新fragment；不得走zero-effect或保留按`summary`构建的leaf。

### Journey F3：matcher等价path规范化

- **Given**：active canonical location为 `/items/1`。
- **When**：执行 `replace(RouteLocation::new("/items//1/"))`。
- **Then**：命令入队前location规范化为 `/items/1`，matcher、view input与history只观察canonical path；不产生view effect或generation变化，request按等价replace提交且history current仍为 `/items/1`。不得发布保留重复/尾随斜杠的新history location。

### Journey G：host loss 与恢复

分别在首次导航 pending时、已有active但新request尚未commit时由应用despawn host，以及host存活时移除registration；另安排应用system精确位于 `after(CommitPresentation).before(SettleCleanup)` 移除registration/despawn host。必须证明：

- Router 从不执行 host despawn，也不重建 host。
- pre-commit host loss时 running/queued request均为 `Failed(HostLost)`，core/ECS active同时变空，history保持旧值且不推进。queued request可立即失败；running request trace必须按序出现 `RequestPhaseChanged(CleanupPending|Recovering)`、隔离/focus证据、`CleanupSettled`、`RequestTerminal`，最终才为 `Failed(HostLost)`。
- post-commit/cleanup前窗口中，`CommitPublished` 已推进presentation/history。host loss把request改为 `Running(Recovering)`，原cleanup ledger与authority settlement不得重复结算；active最终清空、history保持已提交新值、request在settlement后为 `Succeeded`，request/router diagnostics都含同一 `Severe + PostCommitCleanupAnomaly` host-loss事件。该窗口中的queued request仍为 `Failed(HostLost)`。
- registration removal 先隔离、释放 focus、清理 router-owned subtree，host Entity 保持；despawn 路径用 `AlreadyAbsent` 收敛。
- 不尝试渲染 fallback，availability 为 `Unavailable(HostLost)`，后续命令在重新注册前被拒绝。
- 新 app-owned host 注册后状态为 `ReadyNoActive`，显式 replace 可以恢复 history current，并使用全新的 host/slot/frame identity。
- 注册无 `Node` 的host、或把host放到non-Node/cyclic ancestry下，必须得到 `Unavailable(InvalidHostUiTopology)`；Router不补写host/ancestor。应用修复同一registration的UI topology后才可重新完成admission。
- 当 old top slot 因 foreign descendant 被 quarantine 后重新接纳同一个 app host Entity，必须先由旧 HostLease detach 并复验 old top-slot edge，再确认 settlement；新 epoch 创建不同 top slot。随后 old lease reap 不改变新 host relationship/slot mapping。

另由 crate 私有回归删除、清空 marker 或 reparent active root/slot/outlet，证明 registry 的 exact Entity generation/ownership seal 足以先将漂移 subtree 的 visibility、pointer、keyboard、gamepad 和 focus 全部隔离，再安全清理；随后 core/ECS active 清空、history 保留，并在合法 host 下重建到 `ReadyNoActive`。向 router-owned subtree 挂入未登记 app-owned child 时，该 child Entity/组件必须保持不变，owned ancestor 被 quarantine 且完全 inert。

## 4. Authority audit

在每个稳定状态运行统一只读审计：

- 每个 core active frame version 恰好对应一个 registry active root，反向亦然；公共测试只比较不可转换为 Bevy `Entity` 的 `AuditEntityId`。
- 每个 registered outlet 恰好对应一个按 `parent frame + RouteSlotKey` 唯一的 `AuthorityOutletMapping`；Journey B 前后 layout 的 anchor 与 child slot audit id 均保持相同。
- 每个公开 driver 计划的 production primitive 恰好对应一个 `AuthorityContentMapping`；mapping 的 frame/entity/kind/尺寸/color与 registry manifest一致，pending/leaving/quarantine 一律 `visibility_hidden=true, pickable_active=false`，仅 committed active primitive可变为 `false/true`。传播后的 effective visibility由独立物理回归证明。
- `AuthorityHostMapping` 的 equality probe 匹配应用创建的 host Entity；不同 router 的 host audit id 不得相同。Host及其app-owned ancestry的Node合法性由admission结果与ECS私有World回归证明，不暴露ancestor identity。
- top slot、frame shell、content target、outlet anchor、child slot与`spawn_empty`结构output的实际直接edge、Node manifest、透明背景和`Pickable::IGNORE`由固定ECS私有World回归证明；公共audit不为内部结构复制一套DTO。
- 保存的旧 frame/root audit id 在 cleanup 后必须由独立 audit 返回 `Removed`；receipt 与 tombstone 两条证据缺一不可。
- host/slot/frame 映射唯一，不存在重复 identity、额外 active root、prepared active root 或 leaving root。
- authority root 和 slot 位于登记 host 的 router-owned subtree，host 永远不是 frame root。
- 多 router 场景互不共享 host、slot、frame、pending 或 diagnostics。

中间态审计还必须证明 parent 激活不会恢复 inert child，frame-local traversal 在 child authority root/slot 处停止。

输入隔离不得只断言 marker/布尔快照。`public_navigation_input_isolation` 从实时 `RouterAuthorityAudits`/quarantine mapping取得 opaque target ids，通过 `RouterInputProbes` 为 active、pending、leaving、foreign-descendant quarantine 和 protected-parent quarantine分别发送 pointer/keyboard/gamepad：probe 必须调用生产 `RouterInputGate`，只有 active target 返回 `Delivered` 且 counter +1；其余均 `Blocked`、counter不变，`InputFocus` 也不能指向非 active closure。ECS private integration 另用公开 driver 创建的 `SolidRect`、真实 `UiPickingPlugin`/pointer location、hover resolution 和 target observer重复验证：raw `PointerHits` 允许包含 backend candidate，但 pending/leaving/quarantine 的 `Pickable::IGNORE` 不得进入最终 hover或触发 target event，active value才可触发。`InteractionDisabled` 不作为这项证据。故意读取全局 input并绕过 gate 的宿主 system 属于 app-owned authority，不计为 Router-integrated dispatch。

同一测试必须使用公开 `RouterScheduleSet` 精确安排观察系统。系统 A 位于 `after(DriveNavigation).before(CommitPresentation)`：实时 audit 已含新 pending `SolidRect` ID，但它被 probe 阻止，同时旧 active仍可交互。系统 B 位于 `after(CommitPresentation).before(SettleCleanup)`：同一新 ID 已 active且可交互，保存的旧 active ID 已成为 leaving且被阻止。系统 C 位于 cleanup/reaper 相邻 set 后验证 quarantine仍被阻止。`RouterAuthorityAudits` 的 mutation-point更新不得等待 `PublishSnapshots`；下一帧 `PreUpdate Refresh` 可因外部 drift 收窄权限，但不能恢复 pending/leaving/quarantine target。

## 5. ECS authority regressions

crate 私有测试覆盖错误 router、host、slot、route、frame、generation、transaction、sequence、task、parent mount，及未知、重复、乱序 report。这里包含 **Scenario E：旧合法 report 的 stale 回放**。每项在预检/接收时零写入，随后只允许清理对应 pending output；当前 active presentation、active registry、history 和 app-owned host 保持不变。

还必须覆盖：

- host 带有相似应用组件时 cleanup 永不 despawn host。
- host 预先拥有多个 app children 时，admission/settlement 只增删 exact top-slot membership；其他 child identities 与 relative order、全部非-`Children` components 始终不变。
- mismatched root 不扫描删除另一 request 的 root。
- renderer 无法获得 raw World/Commands、复制 authority marker、写 registry 或 commit。
- hostile component hook/bundle/effect 无法传入 `FrameContentWriter`；相关 consumer 必须 compile-fail。内部 `ChildOf` observer 诱发 host/mount provisional despawn/reparent 时只产生 `RejectedAfterMutation`、ID gap 和 staging cleanup，绝不发布 Ready/child task。
- 持久host admission observer跨多个update反复破坏provisional top slot时，同一candidate revision只允许一次`RejectedAfterMutation`与ID gap；后续帧不重试。只有registration重插入、host generation或topology fingerprint变化后才允许一次新admission。
- `SolidRect` 只允许非零 `u16` 像素尺寸和四channel `ContentColor`；consumer无法传入任意component/Bundle/asset/callback。ECS私有回归核对锁定Bevy `Node` required-component closure已全部进入manifest，active `Pickable`字段精确为`should_block_lower=true, is_hoverable=true`，并在应用observer诱发staging drift时拒绝而不发布可见primitive。
- adapter/消费者无法构造 `EcsViewReportSession`、mount token/grant、commit ticket 或 cleanup receipt。
- mount token 的重复、乱序、取消后 replay 不推进 allocator、不登记 Entity、不签发 child task。
- writer self-edge、duplicate parent 和 cycle 在 plan 阶段拒绝且 World/registry 零变化；crate-private 外部 reparent cycle 使用 visited exact-Entity traversal，pending 为 `PendingRejected`，preserved/leaving/current active 为 `AuthorityLost`，永不无限遍历/级联删除。
- host admission/grant/lease/settlement 的取消与旧 epoch replay 不发布 Ready、不复用 host id；running request 只在 settlement receipt 后 `Failed`。
- registry self-audit的duplicate mapping/impossible owner/seal contradiction必须fail closed到`Unavailable(RegistryInvariantViolation)`，清理只触碰仍可证明的exact owned records；旧router不自动重试，新router instance不继承其receipt/lease。
- 每个 slot 从创建起即有 registry owner；leaf replacement 保留 parent-owned slot，parent 离场按 child frame 后 child slot 的顺序清理且无 orphan slot。
- atomic preflight 对 preserved/leaving/entering 完整 closure 分类：pending-only 缺陷返回 `PendingRejected`，只清理 pending 且 active/history/host 不变；host/preserved/leaving/active closure 缺陷返回 `AuthorityLost`，不执行 switch，settlement 后 core/ECS active 同时清空。
- `RemovedDetached` 只允许无 parent、same-lease owned parent 或授权 host-top-slot edge，并先隔离再删除 exact owned Entity；`RejectedProtected` 永不触碰 protected target。Reparent 到 app-owned/other-authority parent 的 root 必须 `QuarantinedProtectedParent`，Router 不删除 child、不移除 relationship、不改写 parent；应用解除 edge 后才能 reap。这些异常 disposition 都不能计入正常 cleanup PASS。
- 应用给 Router-owned Entity 附加 manifest 外且带 hostile remove/despawn hook 的 component 时，Router 必须 `QuarantinedForeignComponent`，不删除 Entity、不移除 component、不触发 hook；应用自行移除后才能 reap。
- post-commit cleanup期间request保持`Running(PostCommitCleanup)`。应用在`CommitPresentation -> SettleCleanup`窗口添加foreign child/component或reparent leaving root时，旧physical attempt必须supersede并按最新World证据重分类；cleanup relationship/despawn hook或observer在执行中制造漂移时同样停止旧plan。已结算obligation不重复，未结算residual只能安全删除或以实际quarantine disposition终结；任何路径都不触碰app-owned state。正常receipt后才`Succeeded`，异常settlement不回滚已发布history/presentation。
- post-commit anomaly 以 `Succeeded` 终结时，request-scoped 与 router-scoped diagnostics 都能读取同一 `Severe + PostCommitCleanupAnomaly` 事件；report diagnostics 必须通过 `ReportDisposition` 结构化读取 `Accepted/StaleDiscarded/DuplicateRejected/InvalidRejected`，invariant violation 也能关联到 request/frame（如适用）。
- stable order 只来自显式 key/index/sequence。
- top-slot、child-slot、frame-shell 和 detached frame-output quarantine 都产生 terminal foreground receipt；新 host/queue 不阻塞，旧 lease 不能影响新 epoch。应用移除 foreign child 后，后台 reaper 逐项结算 immutable owned set并关闭 quarantine lease；root missing/reused 不能遗忘仍存在的 detached owned Entity，永久 foreign child 始终保留且完全 inert。
- live quarantine lease 即使 former host epoch 已旧也能继续安全 reap自己的 immutable set；closed/unknown/错误 sequence replay 才 stale。排序最前的永久 lease 与后续可 reap lease 并存时，round-robin 必须在有界轮次内关闭后者。
- 旧 `bevy_ui_router::core::runtime::RouterRuntime`、`view::ViewReportSession`、`ecs::{RouterEcsState,EcsFrameApplySession,RouteHost,RouteSlot,RouteFrameRoot}` authority 路径全部 compile-fail；allowlist 中的只读/高层 API 仍可编译。
- facade source completeness gate 必须证明基准提交所有未列入 allowlist 的旧 core/view/ECS symbols 与 nested module paths都已不可达，且不存在 glob re-export 或 underlying crate alias。

## 6. Feature 与命令门槛

本节定义MVP gate的完整终态。实施期间每个垂直切片使用已审查的阶段profile，从同一最终allowlist/golden中做纯减法，并在固定目标文件中逐步增加可兑现case；阶段profile不得增加最终合同之外的临时API。Root push只通过自己的`Task PASS`集合，不能把未到所属切片的target、catalogue项或journey标成已通过。

必须验证以下 feature 合同：

```powershell
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --test default_facade_features
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test ecs_facade_features
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features bsn --test bsn_facade_features
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test test_support_features
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features platform
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features devtools
cargo check --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --all-features
cargo tree --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs -e features -i bevy_ui
cargo tree --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --target all -e features,dev -i bevy_internal
```

消费者编译测试必须实际 import 并使用对应公共符号，不能仅依赖 Cargo 命令退出 0。`default_facade_features` 与 isolated default-enabled consumer必须证明 default 精确启用 view 且不引入 Bevy runtime；`ecs` 构建必须同时提供 view、封闭 `SolidRect` primitive和完整 MVP facade API。旧 facade `view-test-support` 必须不存在，`FakeViewRenderer*` 不得从 facade可达。

`bevy_ui` tree输出必须明确包含 feature `bevy_picking` 且不含 `ghost_nodes`，并由factual gate与ECS manifest fixture双重核对；仅看到 direct `bevy_picking` dependency不算通过。`bevy_ui_router_ecs --target all -e features,dev`的`bevy_internal` tree必须精确提供物理回归所需`bevy_window, bevy_camera, bevy_ui, bevy_picking`，证明依赖与实际crate-private test属于同一package；测试只注册其`DefaultPlugins`一次，不能重复注册`UiPickingPlugin`。

Feature-disabled 与旧 authority 路径必须通过 [Facade Export Allowlist](facade-export-allowlist.md) 规定的隔离 consumer harness 实际编译失败；`required-features` 只负责 Cargo target 调度，不能作为“符号不可达”的证据。负例必须按 case 落入预期的 unresolved/private/missing-method/missing-trait-or-conversion 诊断类别（含转换负例所需 `E0277`），依赖下载、manifest 或 workspace 配置失败一律不算通过。

公共合同测试还必须证明：重复 `ViewDriverKey` 返回 `ViewDriverRegistrationErrorKind::DuplicateKey` 且原 driver 未替换；`BevyUiRouterPlugin::default()` 可被consumer显式预装且随后extension不会重复注册system/resource；只有 `with_error_boundary` 而无 error view 的已配置 route 返回 `RouterInstallErrorKind::MissingErrorView`，同一node同时缺normal/error view时优先返回`MissingRouteView`；第65个以上terminal request按FIFO触发`NotRetained`，queued/running request永不被淘汰；同World跨router、跨App、从未签发、未来sequence或handle/id错误配对均为`Unknown`且绝不误命中，只有同domain/router已签发后淘汰的ID为`NotRetained`。

ECS私有回归函数名形成MVP固定catalogue，均位于`authority_regression_tests` module。专用catalogue target用`syn`同时证明crate root存在exact `#[cfg(test)] mod authority_regression_tests;`声明、当前阶段profile激活的每个函数有直接`#[test]`且没有`#[ignore]`或额外`cfg`、名称集合双向相等；缺失、重复、额外名称或0项都失败。`scripts/verify-authority-regressions.ps1`再运行`cargo test --lib authority_regression_tests:: -- --list`并把实际注册test集合与当前profile catalogue双向比较，随后执行该filter并要求passed数等于catalogue数、failed/ignored/measured均为0。源码扫描、binary registration和实际运行三项缺一不可；每个切片只能增加其profile拥有的项，MVP gate必须精确等于以下完整集合：

```text
active_cycle_enters_authority_settlement
cleanup_observer_reentry_supersedes_attempt
cleanup_receipt_replay_is_rejected
commit_gate_switch_is_same_frame_atomic
error_outlet_role_classification_is_preserved
exact_owned_cleanup_cycle_quarantines_exact_set_and_releases_focus
foreign_component_quarantine_avoids_hooks
foreign_descendant_quarantines_owned_ancestor
host_children_change_only_top_slot_membership
host_grant_observer_drift_rejects_after_mutation
host_reacceptance_detaches_old_top_slot
host_ui_topology_requires_all_node_ancestry
mount_grant_observer_drift_rejects_after_mutation
mount_token_replay_is_zero_write
multi_router_registries_are_partitioned
old_quarantine_lease_cannot_mutate_new_epoch
pending_cycle_rejects_without_touching_active
persistent_admission_observer_latches_retry
plan_relationship_errors_are_world_zero
post_commit_cleanup_drift_supersedes_attempt
post_commit_cleanup_waits_for_receipt
post_commit_host_loss_preserves_committed_history
protected_host_is_never_despawned
protected_parent_quarantine_preserves_relationship
quarantine_reaper_round_robin_is_fair
registry_invariant_violation_is_fail_closed
slot_lifecycle_has_no_orphans
solid_rect_required_components_match_locked_bevy
solid_rect_runs_real_layout_and_picking_pipeline
stale_report_identity_is_zero_write
structural_outputs_have_locked_manifest_and_topology
```

MVP gate中的测试文件固定为；各文件最晚在拥有首个可兑现case的切片创建，Root push不得用额外别名文件替代已经激活的固定路径：

```text
crates/bevy_ui_router/tests/public_navigation_journeys.rs
crates/bevy_ui_router/tests/public_navigation_authority_audit.rs
crates/bevy_ui_router/tests/public_navigation_input_isolation.rs
crates/bevy_ui_router/tests/default_facade_features.rs
crates/bevy_ui_router/tests/ecs_facade_features.rs
crates/bevy_ui_router/tests/bsn_facade_features.rs
crates/bevy_ui_router/tests/test_support_features.rs
crates/bevy_ui_router/tests/facade_export_allowlist.rs
crates/bevy_ui_router/tests/facade_authority_compile_fail.rs
docs/facade-associated-api-golden.txt
crates/bevy_ui_router_ecs/src/authority_regression_tests.rs
crates/bevy_ui_router_ecs/tests/authority_regression_catalogue.rs
scripts/verify-authority-regressions.ps1
```

最终至少运行：

```powershell
cargo fmt --all --check
cargo test --locked --config "build.rustc-wrapper = ''" --workspace --all-features
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features ecs --test public_navigation_journeys
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_authority_audit
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router --no-default-features --features test-support --test public_navigation_input_isolation
cargo test --locked --config "build.rustc-wrapper = ''" -p bevy_ui_router_ecs --test authority_regression_catalogue
& .\scripts\verify-authority-regressions.ps1
git diff --cached --check
git diff --check
```

专用test target与verification script必须存在且成功；script必须证明lib test binary实际注册并通过catalogue中的全部tests。0 tests、只存在源码但未声明module、`#[ignore]`、不满足cfg、全部filtered out、passed数不等于catalogue数或缺失target都不得视为gate通过。提交前所有必建artifact必须先由 `git ls-files --error-unmatch` 证明已跟踪；未跟踪文件不在 `git diff --check` 覆盖内，因此预提交还必须运行 `git diff --cached --check` 与显式全文件空白扫描。

只有 Facade journeys A-D、F-G、authority Scenario E、authority/input audit、ECS regressions、feature consumer matrix、workspace tests、格式和静态检查全部通过，并由 factual reviewer 核对真实系统注册与公共导出，才能宣布 `MVP PASS`。
