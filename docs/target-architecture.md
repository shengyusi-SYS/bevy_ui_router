# Target Architecture

**文档类型**: Phase 3B.1 规范性设计
**状态**: Phase 3B.1 `Plan PASS`
**取代范围**: 旧架构路线图中有关当前实现入口、Phase 3B Task 6 和 Phase 3C 前置条件的描述

## 1. 目标与范围

Phase 3B.1 要把生命周期安全原型收敛为一条真实 Router 主路径：外部只通过 facade 配置 route、绑定 view、注册 app-owned host，并发出 `push`、`replace`、`back`；插件系统完成匹配、逐层 apply、原子切换、离场清理、history 和 diagnostics 发布。

这是架构级局部重构。workspace 分层、matcher、fingerprint、`BranchDiff`、common-prefix、frame/task identity 和 stale 检查继续保留。MVP 支持同一 World 中多个隔离 router，但每个 router 至多一个 host。正常 presentation 使用一条有序 `MatchedBranch`；match failure 使用独立 fallback presentation，此时 matched 为 `None`。

MVP 只支持一条匹配链逐层挂载到命名 outlet。并行 named-outlet graph、retained cache、动画、loader/action 和异步 renderer 延后。

## 2. Authority 与依赖方向

```text
bevy_ui_router facade
  configuration + host declaration + navigation resources + snapshots
                                  |
                                  v
core RouterEngine（逻辑 authority）
  match/diff + history intent + presentation + logical IDs + diagnostics
                                  |
                   sealed mount/apply/commit protocol
                                  |
                                  v
ECS driver（Entity authority）
  unique registry + inert shells + staged apply + atomic switch + cleanup
                                  |
                                  v
view catalog + scoped ECS drivers -> fake / BSN
```

- Core 是 router、host、slot、frame、generation、transaction、task 和 navigation request ID 的唯一签发者。
- ECS 是 Entity 与逻辑 ID 映射、authority shell、lifecycle mutation 和 cleanup receipt 的唯一签发者。
- View 层只表达 renderer-neutral descriptor、effect 和 report 数据。
- Adapter 只能通过受限 content writer 构建内容，不能构造身份、写 registry、切换 active 或提交 history。

本文拥有架构边界与不变量；[Authority Transition Matrix](authority-transition-matrix.md) 是 token、closure、cleanup、quarantine 和 request terminal 的规范性细节来源。Matrix 可细化本文，但不能改变依赖方向或 ownership 安全边界；两者对生命周期细节表述冲突时以 Matrix 为准。公共签名以 [Facade Contract Fixture](facade-contract-fixture.md) 为准，公共路径以 [Facade Export Allowlist](facade-export-allowlist.md) 为准。

一个 World 只有一个 `RouterEngines` Resource 和一个 `RouteEntityRegistry` Resource，两者按 `RouterInstanceId` 分区。多 router 驱动按显式 router id 与 request sequence 排序，不依赖 query iteration order。

## 3. Crate 与类型归属

| 类型 | Owner | facade `ecs` canonical path |
| --- | --- | --- |
| `RouteViewKey`, `ActivePresentation`, `NavigationRequestId`, `RequestStatus` | core | `bevy_ui_router::prelude::*` |
| `RouterEngine` | core low-level | `bevy_ui_router_core::engine::RouterEngine`，不进入 facade prelude |
| `ViewDriverKey`, `RouteViewRegistry`, `ViewBuildEffect` | view | `bevy_ui_router::prelude::*` |
| `EcsViewDriver`, `FrameContentWriter`, `EcsViewReportSession`, `ViewBuildReport`, `SolidRect`, `ContentColor` | ECS | driver/writer、同crate report与封闭primitive数据从prelude导出；session构造器不导出 |
| `RouterHandle`, `RouterConfig`, `RouterHostRegistration`, `RouterNavigation`, `RouterSnapshots`, `RouterSnapshot`, `NavigationRequestSnapshot`, `RequestLookup` | ECS | `bevy_ui_router::prelude::*` |
| `RouterAuthorityAuditSnapshot`及其他audit DTO | ECS test-support | 仅随`ecs-test-support`进入`bevy_ui_router::ecs`与feature-filtered prelude；只读opaque identity |
| mount request/grant、commit ticket | core-to-ECS sealed protocol | 类型可跨 crate 使用，构造器私有，不从 facade 导出 |
| entity mount/cleanup receipt、authority marker、registry writer | ECS private protocol | 不从 facade 导出 |

`RouteNodeOptions` 增加普通 `RouteViewKey`，并可把 `ErrorBoundaryKey` 与 error view 绑定。既有 `with_error_boundary` 继续供 Bevy-free matcher/低层树使用；高层 `RouterConfig` 安装时若某 node 声明 boundary 却未绑定 error view，必须同步返回 `MissingErrorView`，也要拒绝无普通 view、无法解析的 binding 或 fallback。每个 node 按声明 index 先验证 normal view，再验证其 error boundary，因此同时缺失时 `MissingRouteView` 优先；低层 matcher 测试仍可构造无 view tree。ECS安装只消费core crate提供的内部compiled route manifest/query协议读取这些binding，不给facade `RouteNodeOptions/RouteNode`增加读取或authority accessor。`ViewBindingKey` 在此次 pre-MVP breaking migration 中移除，调用方显式迁移到返回 `Result` 的 `RouteViewKey::new`；`ErrorBoundaryKey` 只表示边界身份。

## 4. View 协议与受限写入

`bevy_ui_router_view` 保持 Bevy-free：

- `RouteViewRegistry` 把 `RouteViewKey` 唯一映射到包含 `ViewDriverKey` 的 descriptor，不保存 World callback。
- `ViewBuildEffect` 携带 view key、只读 `RouteViewInput` 和 renderer request id，不携带 Entity 或 Router authority token。
- `ViewBuildEffect` 与view registry仍属于renderer-neutral view crate；ECS immediate driver的 `ViewBuildReport`/kind改由ECS crate拥有，只是 `Applied | BuildFailed | ApplyFailed` 与脱敏message数据。这样 `EcsViewReportSession` 可用同crate私有字段/构造函数生成report，不需要Rust不存在的跨crate `pub(crate)`友元，也不公开factory。outlet/graph来自独立 `ContentPlan`，report不带Entity、disposal callback或acceptance capability。

MVP driver 是 immediate 协议：

```rust
pub trait EcsViewDriver: Send + Sync + 'static {
    fn key(&self) -> ViewDriverKey;
    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport;
}
```

`EcsViewReportSession` 与 `ViewBuildReport` 同属ECS crate。Session由ECS单次签发，构造器和report内部构造函数均crate-private；consumer只能消费session的三个terminal方法并读取返回report的kind/message，不能直接构造report。MVP不公开延迟report通道；旧report/stale replay由ECS私有回归验证。未来异步扩展必须使用单独task handle，并在apply前重新检查router/host/slot/frame/generation/transaction/task；不持有 `&mut World` 跨 `await`。

`RouteViewInput` 只读暴露 requested location、matched route id、route-local params、raw search、fragment、frame origin，以及 error/fallback 时的脱敏 `NavigationFailure`；它不暴露 generation、task、mount、Entity 或 commit capability。

`RouteLocation` 在进入 matcher、history intent 和 view effect 前先规范化 path：按 `/` 分段、丢弃空段后重新拼接为唯一前导斜杠且非根路径无尾随斜杠的形式，根路径固定为 `/`；search 与 fragment 原样保留。因此 `/items//1/` 与 `/items/1` 是同一 canonical location，不会形成“新 history location + 旧 view input”。Fragment不参与route match/fingerprint，但不能被零effect路径忽略。RouterEngine在branch diff之后计算view-input delta：canonical path/search/params/branch完全相同而fragment变化时，保留所有ancestor layout/outlet，只对最深matched normal frame执行remount，保留frame id并推进generation，driver收到新fragment；history按原push/replace intent只在commit后更新。只有canonical path、search与fragment都相同的等价location才允许真正zero-entering。Active error presentation不使用该优化，仍走普通navigation/recovery。

`FrameContentWriter` 不持有或访问 World；它只构造内存中的 session-local `ContentPlan`。`spawn_empty` 的“empty”只表示没有业务primitive数据：成功plan realization时它必须成为透明结构 `Node`，具有与其他Router结构Node相同的hidden/`Pickable::IGNORE` barrier、100% container layout、完整required-component manifest、`FrameOutput` owner与audit记录，因此可以安全作为parent或outlet anchor，绝不是physical non-Node Entity。MVP 另提供 `spawn_solid_rect(SolidRect)`，这是唯一生产可视 primitive。`SolidRect` 只包含经校验的非零像素宽高与 `ContentColor` 的四个 `u8` sRGBA channel，没有 Bundle、component、asset handle、callback、hook、observer 或 World capability。`add_child`、`declare_outlet`、`despawn` 只更新 plan 数据，driver 返回前不创建 Entity、不写 relationship、不触发 hook/observer。Plan graph 要求每个 handle 至多一个 parent，拒绝 self-edge、duplicate edge/parent和任何会让 parent 从 child 可达的 cycle；顶层 handles 默认挂到 content target，所有遍历使用显式稳定 index + visited set。失败 report 直接丢弃 plan，World 零变化。

成功 report 后，ECS 再按稳定拓扑顺序 realization：先为全部handles分配无component Entity并staging-register，再统一写 `Visibility::Hidden + Pickable::IGNORE`；`spawn_empty` handle解释为100% width/height透明container `Node`，`SolidRect` handle解释为锁定Bevy的 `Node { width: Val::Px, height: Val::Px, ..default() } + BackgroundColor`，最后写关系并逐阶段复验。锁定 revision 的递归 manifest closure固定包含 `Node, ComputedNode, ComputedStackIndex, ContentSize, ComputedUiTargetCamera, ComputedUiRenderTargetInfo, UiTransform, UiGlobalTransform, BackgroundColor, BorderColor, FocusPolicy, ScrollPosition, Visibility, InheritedVisibility, ViewVisibility, ZIndex, Pickable`；实现中的事实测试必须从锁定源码/注册后的 component set递归核对该集合，不能把 required insertion 当成不可见副作用。该内建集合没有自定义 component hook；应用自行注册的全局 observer仍属于宿主 authority，任何 observer-induced drift 都由 post-write validation 转成 `RejectedAfterMutation`/apply failure并清理 staging residual。`FrameContentWriter` 不接受任意 Bevy `Bundle`、component value、asset handle、bundle effect、callback、hook 或 observer；authority/lifecycle/inert 组件只由 ECS 内部写入，因此 adapter 不能借 lifecycle hook取得 `DeferredWorld`。outlet key/anchor 在 realization 后登记。

MVP 必须通过公开 driver 创建并提交真实 `SolidRect`，不能用 empty Entity、snapshot 或私有测试节点代替生产可视能力。`SolidRect` 是本轮唯一已审计的 sealed primitive；MVP 后的 BSN adapter直接复用它。若 BSN 示例需要文本、图片或其他 primitive，必须以新的 data-only variant 单独完成事实/安全审查和 hostile hook/observer regression后才能进入 facade allowlist，绝不恢复 generic Bundle/World callback。

重复 `ViewDriverKey` 一律返回 `ViewDriverRegistrationError`，其 `kind()` 为 `ViewDriverRegistrationErrorKind::DuplicateKey`；不替换、不复用原 driver。

## 5. Facade 与 feature 合同

推荐公共形状固定为：

```rust
pub trait AppRouterExt {
    fn register_ui_view_driver<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError>;

    fn add_ui_router(
        &mut self,
        config: RouterConfig,
    ) -> Result<RouterHandle, RouterInstallError>;
}

impl RouterConfig {
    pub fn new(
        routes: RouteTree,
        views: RouteViewRegistry,
        fallback: RouteViewKey,
    ) -> Self;
}

impl RouterHostRegistration {
    pub fn new(router: RouterHandle) -> Self;
}

impl RouterNavigation {
    pub fn push(&mut self, router: RouterHandle, location: RouteLocation)
        -> Result<NavigationRequestId, NavigationEnqueueError>;
    pub fn replace(&mut self, router: RouterHandle, location: RouteLocation)
        -> Result<NavigationRequestId, NavigationEnqueueError>;
    pub fn back(&mut self, router: RouterHandle)
        -> Result<NavigationRequestId, NavigationEnqueueError>;
}

impl RouterSnapshots {
    pub fn get(&self, router: RouterHandle) -> Option<&RouterSnapshot>;
    pub fn request(
        &self,
        router: RouterHandle,
        request: NavigationRequestId,
    ) -> RequestLookup<'_>;
}
```

`RouterHandle` 和 `NavigationRequestId` 没有 raw constructor。`RouterNavigation` 与 `RouterSnapshots` 是公开 Bevy `bevy_ecs::resource::Resource`，并按锁定Bevy事实同时实现mandatory `bevy_ecs::component::Component` supertrait；`RouterHostRegistration` 是公开 Component，三者由 `ecs` prelude 导出。消费者通过 `app.world_mut().resource_mut::<RouterNavigation>()` 入队，通过 `app.world().resource::<RouterSnapshots>()` 观察。命令只在下一次 `App::update` 的 `CollectNavigation` 阶段生效。

`BevyUiRouterPlugin` 明确实现 `Default`，consumer可用 `app.add_plugins(BevyUiRouterPlugin::default())` 构造并显式预装。首次调用 `register_ui_view_driver` 或 `add_ui_router` 时，`AppRouterExt` 幂等安装 plugin、resources 和真实 system chain；显式预装同一 plugin 时复用 install marker，禁止重复注册系统。隔离consumer必须分别覆盖自动安装与显式预装后调用extension，两条路径的schedule set、resource和driver/router注册行为相同且均只有一份。

`RouterScheduleSet` 是公开排序锚点，固定为以下 `Update` chain：

```text
ObserveAuthority
-> CollectNavigation
-> DriveNavigation
-> CommitPresentation
-> SettleCleanup
-> ReapQuarantines
-> PublishSnapshots
```

Host admission/loss 在 collection 前观察；shell 创建、immediate driver apply 和 mount handshake 在 `DriveNavigation` 的 exclusive systems 中推进；final preflight、lifecycle/registry/`RouterInputGate` 原子 switch 在 `CommitPresentation`；foreground ledger 在 `SettleCleanup`；旧 epoch reaper 不得影响 active authority；最后统一发布 snapshots 与下一次同步 enqueue 使用的 immutable admission snapshot。Router-created Entity 先以无 component empty allocation 存在并立即 staging-register；在离开同一 exclusive block 或加入任何可渲染/交互 component 前必须具备完整 inert barrier，不能依赖下一 set 补写。

每个`RouterHandle`携带不可伪造的World/router domain；每个`NavigationRequestId`携带同一domain、router identity与单调request sequence，均无raw accessor/constructor。一个World内及跨App复制的handle/id都必须按exact provenance比较，绝不能只按局部sequence命中。每个router保留最近64个terminal request records，按该router terminal sequence FIFO淘汰，永不淘汰queued/running request。`RouterSnapshots::request(router,id)`仅在exact domain/router/id仍保留时为`Found`；只有id已确认由同domain/router签发且后来淘汰时为`NotRetained`；foreign World/router、从未签发、未来sequence或错误配对一律`Unknown`。snapshot暴露最早保留sequence，无TTL。

Feature 图固定为：

- `default = ["view"]`，默认不引入 Bevy。
- `ecs` 隐含 `view`，提供完整 MVP facade、plugin 和 snapshots。
- `bsn` 隐含 `ecs`。
- 新增 `ecs-test-support`；聚合 `test-support` 只启用该封闭观察/探针面。旧 facade `view-test-support` 与 `FakeViewRenderer*` 依赖已经被废止的 build/apply 类型图，无法与新 allowlist 同时闭包，因此在本次 pre-MVP breaking migration 中移除；owner crate 可继续用自己的 `test-support` 做内部回归，但 facade 不再转发。
- 保留当前 `platform` 和空 `devtools` feature，不在本阶段扩展行为。
- facade module re-export 使用 [Facade Export Allowlist](facade-export-allowlist.md) 的精确清单，不再承诺整 crate 的旧 authority surface。

Facade manifest 的目标定义必须等价于：

```toml
[features]
default = ["view"]
view = ["dep:bevy_ui_router_view"]
ecs = ["view", "dep:bevy_ui_router_ecs"]
ecs-test-support = ["ecs", "bevy_ui_router_ecs/test-support"]
test-support = ["ecs-test-support"]
bsn = ["ecs", "dep:bevy_ui_router_bsn"]
platform = ["dep:bevy_ui_router_platform"]
devtools = []
```

ECS crate 同时必须显式声明 facade feature edge 所引用的只读 gate：

```toml
# crates/bevy_ui_router_ecs/Cargo.toml
[features]
default = []
test-support = []
```

`test-support` 不启用 lifecycle mutation harness，也不引入额外 authority constructor；它导出第 11 节的实时只读 audit Resource，并提供仅接受 opaque audit id、只能经过生产 `RouterInputGate` 的 bounded behavioral probe。Probe 可以增加自己的测试计数器，但不能改变 Router lifecycle、registry、authority或任意应用组件。

锁定 native picking 合同还要求 ECS 的正常（非仅 dev）`bevy_ui` dependency 精确启用其 integration feature；单独依赖 `bevy_picking` 不够：

```toml
bevy_ui = {
    git = "https://github.com/shengyusi-SYS/bevy.git",
    rev = "61f78f75684e6b0bc28a13a5933f8f55ca653fa7",
    version = "0.19.0",
    default-features = false,
    features = ["bevy_picking"],
}
```

Root push 必须用 feature tree/metadata 证明 `bevy_ui/bevy_picking` 在 `bevy_ui_router_ecs` 构建中实际启用且 `bevy_ui/ghost_nodes` 未启用，随后 native UI picking regression 才具有编译证据。

所有 workspace Bevy git dependencies（包括 facade direct dev-dependencies）统一使用上述 exact `rev`，不得继续使用可移动 branch。Root push 必须生成、审查并提交 workspace `Cargo.lock`；从该提交起所有 workspace check/test/tree/metadata gate 使用 `--locked`。重构前基准和当前未跟踪 lock 不是可交付锁定证据。

Facade integration tests直接使用`bevy_app`、`bevy_ecs`与`bevy_ui`的host/Node类型，因此三者必须以和ECS crate相同git/version/default-feature约束进入facade package的`[dev-dependencies]`，不能依赖传递依赖偶然可用。真实物理UI/picking regression位于ECS crate-private lib tests，所以exact-rev `bevy_internal`必须直接进入`bevy_ui_router_ecs [dev-dependencies]`，`default-features = false`且只启用`bevy_window, bevy_camera, bevy_ui, bevy_picking`；facade只有在自身test target实际引用时才声明该依赖。ECS测试只添加一次该配置产生的`DefaultPlugins`，由它唯一安装`UiPlugin`与`DefaultPickingPlugins`，不得再次手动添加`UiPickingPlugin`。`WindowPlugin::default()`已创建唯一primary window，测试必须查询并配置existing `PrimaryWindow`、断言数量恰为1，绝不再spawn第二个；随后只显式spawn一个`Camera2d`。由此获得input、transform、window、camera/visibility、UI layout、UI backend与picking hover/interaction主链。Facade export completeness target在facade package直接dev-depend `syn = { version = "2", features = ["full"] }`；ECS authority catalogue target在ECS package独立direct dev-depend同版`syn`，不能借用facade依赖。两者只补充、不能替代隔离consumer编译。Root push必须建立可扩展的固定harness、命名规则与本阶段profile所激活的test files/commands；[MVP Acceptance Journeys](mvp-acceptance-journeys.md) 第6节和 [Facade Export Allowlist](facade-export-allowlist.md) 第5节的完整文件、命令与catalogue是MVP gate，不要求未到所属切片的case在Root push伪通过。

Fixture 可达的 public error types（key、registry、route、content write、driver registration、router install/enqueue）都必须实现 `Debug + Display + std::error::Error + Send + Sync + 'static`；这样规范 fixture 中的 `?` 和 `Box<dyn Error>` 是可编译合同。

## 6. Router availability 与 admission

以下状态正交存在，不能合并成一个 enum：

```text
RouterAvailability = AwaitingHost | Ready | RecoveringAuthority | Unavailable(reason)
RequestStatus      = Queued | Running(NavigationPhase) | Succeeded | Failed(reason)
NavigationPhase    = Matching | Planning | ResolvingMount | PreparingFrame(n)
                   | ApplyingFrame(n) | RegisteringOutlets(n) | ReadyToSwitch
                   | Recovering | CleanupPending | PostCommitCleanup
ActivePresentation = None | Route { ... } | Error { ... }
ReportDisposition  = Accepted | StaleDiscarded | DuplicateRejected | InvalidRejected
```

只有 `Succeeded` 与 `Failed` 是 request 终态。规则：

- Router 安装后为 `AwaitingHost`；导航立即返回 `HostUnavailable`，不创建 request id。
- 一个 router 只能登记一个 host，一个 Entity 只能服务一个 router。
- 尚未登记 host 时，同一 update 有多个候选则全部拒绝并进入 `Unavailable(HostConflict)`；只剩一个候选后可自动接受。已有合法 host 时，额外候选单独拒绝，既有映射不变。
- `Ready` 时命令按全局 sequence 入队，每个 router 同时只有一个 running transaction。
- `RecoveringAuthority` 时新命令返回 `AuthorityRecovering`，不创建 request id。
- host/authority loss 使 queued request 立即失败；尚未 `CommitPublished` 的 running request先保持 `Running(CleanupPending|Recovering)`，receipt/settlement 完成后成为 `Failed(HostLost|AuthorityLost)`，history不推进。若外部 host loss发生在 `CommitPresentation` 已发布 presentation/history、但 `SettleCleanup` 尚未完成的窗口，请求改为 `Running(Recovering)`，原 cleanup obligations转入同一 authority settlement；已发布 history不回滚，active在 settlement中清空，最终请求为 `Succeeded` 并记录 `Severe + PostCommitCleanupAnomaly`（cause为 host/authority loss）。两种路径都不在新 slot 上静默重放。
- stale report 只记录 disposition，不改变当前 request。

Host loss settlement 期间 availability 为 `RecoveringAuthority`；registration 已移除或 host 已不存在且没有已确认新候选时，稳定态为 `Unavailable(HostLost)`。唯一candidate存在但不是合法全Node UI ancestry时稳定为 `Unavailable(InvalidHostUiTopology)`；Router只读观察同一registration，应用修复Node topology后自动重跑完整admission。上述 unavailable状态导航都返回 `HostUnavailable` 且不创建 request id。Active authority drift 保留合法 registration 时必须重新执行完整 admission：成功后为 `Ready + None`，post-write失败后为 `Unavailable(AuthorityEstablishmentFailed)`。Host admission对当前candidate revision闩锁任何`RejectedAfterMutation`；registration实例、host exact Entity generation或topology fingerprint未变化时不得自动重试、分配新ID或创建新lease，变化后才允许一次新attempt。

`RegistryInvariantViolation`只用于registry self-audit发现duplicate identity/mapping、impossible owner transition或internal seal contradiction；它不是普通host candidate失败。Router先按仍可证明的exact records完成isolation/settlement，再进入永久fail-closed的`Unavailable(RegistryInvariantViolation)`；该router instance不自动re-admit，应用必须安装新的router instance，旧receipt/lease不能影响新实例。

`ReadyNoActive` 是文档简写，表示 availability 为 `Ready` 且 active presentation 为 `None`，不是额外状态枚举。

## 7. Host、frame 与 mount handshake

Host 是 app-owned **UI Node Entity**。`RouterHostRegistration` 只是声明；应用必须在注册前自行给 host 插入锁定 Bevy `Node`，并保证 host 无 `ChildOf` 或其到 UI root 的每个实际 ancestor 都有 `Node`。Router只读校验这条 exact-Entity、visited、acyclic UI ancestry及 host 的 required-component closure，不给 host/ancestor补组件，也不更改其 layout/visibility/picking。ECS 在 host下创建 router-owned透明顶层 slot Node。Host 永远不是 slot/frame root，也不由 Router 重建、替换或销毁。

Host version 为 `router + host + registration_epoch`。安装 router 时只有 `RouterInstanceId`，没有预绑定 host id。Admission 使用 sealed handshake：

1. ECS 对唯一候选 Entity 做存在性、冲突、跨 router以及完整 UI Node ancestry预检，然后向 RouterEngine 申请单次 `HostAdmissionToken`；token 不分配 host id。缺少 `Node`、required component、出现 non-Node ancestor/cycle或 ghost-only路径的候选以 `InvalidHostUiTopology` 拒绝。
2. 在同一个 exclusive system 中再次验证候选 UI ancestry，并预检 host registry 与透明 top-slot Node spawn所需全部条件；失败时必须通过 sealed reject/cancel 转换消费 token，不分配 ID、不创建 Entity。
3. 预检成功后，engine 消费 token，单调分配新的 `RouteHostId + registration_epoch + top RouteSlotId` 并返回 `HostGrant`。分配器允许永久 ID gap，绝不回收失败 grant 的 ID。
4. ECS 先用无 component 的 allocation primitive 创建天然 inert Entity，并在任何可能触发 hook/observer 的 component/relationship mutation 前写入 private staging registry。随后先写 hidden、`Pickable::IGNORE` 和透明结构 `Node` 的完整 required closure，再写 authority/`ChildOf(host)`，最后对 exact Entity generation、host UI ancestry/edge、components、seal、registry 和 barrier 做写后复验。
5. 复验成功时，ECS 交回 grant，engine 只接受当前 outstanding grant并转为 `Confirmed`、发布 `Ready`。复验失败时 grant 转为 terminal `RejectedAfterMutation`，永不发布 host/slot；staging residual 通过 sealed cleanup batch 删除、quarantine 或 `AlreadyAbsent` 收敛。任何时刻都允许 provisional，但不允许 untracked/non-inert provisional。
6. ECS 为已确认 host 保留 opaque `HostLease`。registration removal 或 Entity loss 时，engine 消费当前 lease并签发 `HostSettlementTicket`；ECS 完成隔离/cleanup 后交回 ticket，engine 才使 running request 终结并发布新的 availability。

旧 epoch 的 admission token、grant、`HostLease`、cleanup token 或 settlement replay 一律 stale。已完成 transfer 的 live `QuarantineLease` 按自己的 generation/operation sequence 判断，不属于此规则。重新注册必得新的 host id、epoch、top slot 和后续 frame identity。

`RouterNavigation` 的同步 admission 判断只读取 RouterEngine 在上一系统阶段发布到该 Resource 的 immutable admission snapshot；它不直接查询 World，也不能提前猜测尚未确认的 host。

一个 frame version 为：

```text
router + host + slot + origin + frame + generation
origin = MatchedRoute(route_id) | RouteError(route_id) | RouterFallback
```

apply task 还携带 transaction、request sequence 和 task identity。生产 allocator 在保留 frame id 的 remount/retry 时推进 generation；新 frame 使用新 frame id 与 initial generation。

Frame mount target 只有：

```text
TopSlot { host_version, slot_id }
ChildOutlet { parent_frame_version, outlet_key }
```

`TopSlot` 已由当前 `HostGrant` 签发并登记，首次 frame 与 router fallback 直接使用它，不再次分配 slot。只有 `ChildOutlet` 需要以下 mount handshake。

不启用或依赖 Bevy experimental `ghost_nodes`。物理 UI ancestry 中从 app-owned host 到每个 primitive 的每个直接 parent都必须有 `Node`；Router-owned结构 Node背景透明、始终 `Pickable::IGNORE`，其 layout固定为承载单链 outlet所需的 100% width/height container。嵌套 Entity 结构固定为：

```text
app-owned host Node
  -> router-owned top slot Node
       -> parent authority root Node
            -> content target Node
                 -> router-owned frame-output outlet anchor Node/SolidRect（由driver声明）
                      -> router-owned child slot Node
                           -> child authority root Node
```

Core 与 ECS 通过 sealed handshake 连接：

1. RouterEngine 为 child outlet 签发单次 `MountRequestToken`，包含 router、host、transaction、task、parent frame version、outlet key 和 sequence，但没有 slot id。
2. ECS 验证实际 host/anchor但尚不创建 Entity；Entity 不进入 core。
3. ECS 在验证成功后把 token 交回 RouterEngine；engine 只接受当前 outstanding token，消费后单调分配唯一 `RouteSlotId` 并返回不可伪造的 `MountGrant`。重复、乱序、取消后 replay 均为 stale；失败 grant 留下 ID gap但不复用。
4. ECS 先创建 inert empty Entity并在任何 relationship/component mutation 前登记 staging grant/Entity，再写 hidden、`Pickable::IGNORE`、透明结构 `Node`及完整 required closure，最后写 slot authority 与 `ChildOf(anchor)`；写后复验 exact generation、全 Node ancestry、anchor edge、seal、registry 和 barrier。
5. 复验成功时 ECS 记录 `EntityMountReceipt` 并交回 grant，engine 转为 `Confirmed` 后才签发完整 child frame task。失败时 grant 转为 `RejectedAfterMutation`，不签发 child task，staging residual 进入 sealed cleanup。不存在 untracked provisional slot，但允许 tracked inert provisional。

`ContentPlan` 自身的 duplicate outlet key、foreign-session handle、非法 relationship 等错误在 driver 返回前形成 parent `ApplyFailed`，此时尚无 child target 或 mount token，World/registry 零变化。只有 parent plan 已被接受、RouterEngine 已规划 child target 后才进入 mount 分类：entering parent 缺少 required outlet，或已登记 anchor 在 realization/post-write 复验时失效，分别以 `MissingOutlet`/`InvalidOutlet` 消费 token并成为 `PendingRejected`；mount 若依赖 preserved/current parent 的既有 outlet，而该 anchor 的 identity、owner、seal 或存在性失效，则以 `ParentAuthorityLost` 消费 token并进入 `AuthorityLost`，禁止 fallback。

Adapter 只能声明 outlet anchor；不能签发 slot id 或 mount receipt。Host admission trace 与 navigation request trace 分离：host trace记录 `HostGranted/TopSlotRegistered/HostReady`，首次 navigation trace从 `FrameTaskIssued(parent, top_slot)` 开始。

Slot lifecycle 固定为：顶层 slot 归属 host version，在 host settlement 时清理；child slot 归属 `parent frame version + outlet key`，只要 parent preserved 就跨 leaf replacement 保留。parent 离场时先清理全部 child frames，再从最深层向上清理 child slots。Mount token 在 Entity 创建前失败时没有 cleanup；grant 后失败必须留下 staging/cleanup trace 和永久 ID gap。Audit 必须证明每个 nonterminal provisional slot 均 registered + inert，terminal 后没有 orphan/provisional slot。

## 8. Authority shell 与 frame-local 遍历

每个 entering frame 在 renderer 前由 ECS 创建：

```text
authority root Node（transparent，private identity/lifecycle/ownership seal，inert）
  -> content target Node（transparent，受限 writer 根，inert）
```

root/content target 从 empty allocation 起不含可渲染/交互 component，但在离开同一 staging exclusive block前必须先施加 hidden、private inert state、gate deny和 `Pickable::IGNORE`，再插入透明结构 `Node`及其递归 required closure；任何 observer drift写后复验。结构Node active时仍保持 `Pickable::IGNORE`。`SolidRect`的唯一active值固定为 `Pickable { should_block_lower: true, is_hoverable: true }`，不能使用其他hover/block组合或依赖未来default漂移；pending/leaving/quarantine使用`Pickable::IGNORE`，commit只在已有component上原地切换这两个literal值。未来primitive必须各自单独冻结active值。不得通过add/remove marker实现lifecycle。renderer返回后、任何Router-integrated input dispatch运行前，ECS对新内容重施barrier并释放指向该closure的 `InputFocus`。

`InteractionDisabled` 是 widget semantic marker，不作为 Router lifecycle barrier。Plugin 维护公开只读 `RouterInputGate` 和 `PreUpdate` 的 `RouterInputSet::Refresh -> Ready`：Refresh 在标准/Router-integrated dispatch 前重新扫描外部 drift、actual ancestors、registry/quarantine membership并只可收窄权限。每次 `CommitPresentation` 又在同一 exclusive switch 中原子替换 gate 的 active/blocked snapshot与所有预建 `Pickable` values：commit 前运行的 input看 old active + old gate/pickable，commit 后同帧运行的系统看 new active + new gate/pickable，永不交叉。pending/leaving/quarantine/foreign descendant 都返回 false，只有 committed active closure 返回 true。

锁定 Bevy UI picking/hover pipeline 的物理 pointer barrier 是 `InheritedVisibility + Pickable`；backend 可先产生原始 `PointerHits`，但 hover/event resolution 必须因 `Pickable::IGNORE` 不把 target 交给 observer。验收运行完整真实 pipeline并断言最终 hover/target event，而不是要求 raw hits 为空，也不假定 `InteractionDisabled` 生效。Router/adapter 提供的 target-addressed keyboard/gamepad dispatch 必须在 `RouterInputSet::Ready` 后调用 gate；若 dispatch 排在 `Update` commit 后，还必须显式 order after `RouterScheduleSet::CommitPresentation`。直接读取全局 input 后故意绕过 gate并操作某 Entity 的 app system 属于 app-owned raw-World authority，不属于 Router-integrated dispatch；Router 不声称能沙箱化宿主代码。

visibility/input/focus/cleanup 遍历必须 frame-local：outlet anchor 按 parent content 处理；遇到 child slot container 时处理 container 自身，然后停止下降，不访问 child authority root。每次 relationship traversal 都维护 exact-Entity visited set和稳定顺序；cycle/revisit 立即停止，不递归或级联删除。Plan 内 cycle 在 World mutation 前返回 `ContentWriteErrorKind::RelationshipCycle`；外部 reparent 导致的 pending cycle 为 `PendingRejected`，preserved/leaving/current active cycle 为 `AuthorityLost`，settlement 只按 registry exact set逐项隔离/清理/quarantine。parent 激活不能恢复 inert child，parent 离场不能绕过 child 的 leaf-to-root 顺序。

Registry 对 authority root、slot、content target 及 writer 创建的每个 content Entity逐项保存完整 Entity generation、private ownership seal、显式 owner 和该角色允许的 component-id manifest：

Registry authority保持以下不变量：

- ownership seal首次成功登记后永久消费，不因record删除或转入quarantine而复用
- lifecycle变更、删除和lease transfer使用包含exact Entity generation、owner、seal与预期lifecycle的current claim执行compare-and-swap，stale claim不得改变registry或lease table

```text
EntityOwner = HostTopSlot(host_version, slot_id)
            | ChildSlot(parent_frame_version, outlet_key, slot_id)
            | FrameShell(frame_version)
            | FrameOutput(frame_version, output_id, local_index)

QuarantineOwner = { lease_id, previous_owner: EntityOwner, former_host_version }
```

Router-created Entity 即使被 reparent，仍可按 exact Entity 安全隔离；如果该 generation 已不存在，则不得触碰可能复用 index 的新 Entity。Top-slot-only、child-slot-only 和无 active frame 的 quarantine 都可由 owner 联合类型表达。

每个 cleanup/preflight 还必须分类实际 inbound relationship：

```text
InboundParent = ExpectedOwned
              | AuthorizedHostTopSlotEdge
              | Missing
              | ProtectedAppOwned
              | OtherAuthorityOwned
```

只有 top slot 到当前候选/已确认 host 的 relationship bookkeeping 由 `HostGrant/HostLease` 明确授权。唯一允许的 host-side变化是锁定 Bevy 因 exact top-slot `ChildOf` 自动创建/更新/移除的 `Children` membership；Router 不直接写 `Children`，不得改变任何其他 child identity或它们的相对顺序。授权不包含销毁/替换 host或改写其他应用组件。普通 Router-owned Entity 只有在无 parent，或 parent 也是同一 cleanup plan 内的 exact Router-owned Entity 时才能直接 despawn。若 exact owned root 被 reparent 到 app-owned/protected parent 下，despawn 会通过 Bevy relationship hook 改写 parent，因此 Router 只能隔离 child、释放其 subtree focus并转入 `QuarantinedProtectedParent`，绝不移除该 relationship；等应用自行移除/改变 protected edge 后，后台 reaper 才能删除 exact owned residual。指向其他 router/lease 的 parent 按 authority corruption 处理，同样不能改写 parent。

Despawn 前还必须比较实际 component IDs 与 registry manifest。应用后来附加的未知 component 被视为 foreign app state；Router 不 remove/despawn它，以避免执行其 discard/remove/despawn hook，而是隔离 Entity并转入 `QuarantinedForeignComponent`。应用自行移除全部 foreign components 后才允许 reaper 继续。宿主直接注册全局 observer/hook 并绕过这些边界仍属于 app-owned World authority，但 Router 自身和 adapter 不提供该 capability。

清理前必须把实际 descendants 与逐实体 registry 做 ownership partition。只有所有待级联删除的 Entity 都已登记且 generation/seal 匹配时才能 despawn ancestor。若发现 foreign/app-owned descendant，Router 不移动、不改写、不删除该实体，也不删除会级联触碰它的 owned ancestor；按 Matrix 的 removal-DAG 算法选择互斥的 mounted-unit lease roots；漂移到预期 mounted boundary 外的 exact `FrameOutput` 可成为 detached output root。在每个 root 上施加 hidden/input barrier、释放 subtree focus，并登记 `QuarantinedForeignDescendant`。该 quarantine 不属于 active set，可在 foreign descendant 被应用移除后由后台 cleanup 重试。

## 9. 逐层 apply 与原子切换

详细分类与终态以 [Authority Transition Matrix](authority-transition-matrix.md) 为准。

- parent view apply 与 outlet mount handshake 完成后，child effect 才能产生。
- pending/prepared root 永远不是 active UI。
- 反序、重复、未知 task 或错误 mount report 在 mutation 前拒绝。
- 只有完成 canonical location 与 view-input delta 分类后仍为零 entering/leaving 的导航，才直接进入 `ReadyToSwitch`；fragment变化已转为最深normal frame remount，slash-only path差异已在 `RouteLocation` 入口规范化。

最终切换在一个 exclusive system 中执行。只读 preflight 对 preserved、leaving、entering 三套完整 Entity closure 逐实体验证 registry record、`EntityOwner`、Entity generation、ownership seal、actual component manifest、inbound parent、acyclic visited traversal、ancestry、frame generation、slot owner 和 foreign descendant/component partition；另验证 host epoch/top slot、active baseline、当前 gate baseline 与 commit ticket。Entering root 必须为 `PreparedInert` 且 visibility/input/focus barrier 完整，leaving closure 必须已经能生成不触碰 foreign Entity/component/protected parent 的隔离/cleanup plan。

Preflight 只有三种结果：

- `Ready(CommitMutationPlan)`：所有 switch mutation 已解析并保证不可失败。
- `PendingRejected`：缺陷仅位于 pending/entering closure；旧 active/preserved/host 完整，保持 active/history 不变并只 cleanup pending。
- `AuthorityLost`：host、preserved、leaving 或现有 active registry closure 失效；不执行 switch，进入 authority settlement，隔离/清理或 quarantine 后清空 core/ECS active。

成功写入顺序固定为：

1. leaving leaf-to-root 隔离全部输入并释放 focus。
2. entering root-to-leaf 激活，遵守 frame-local barrier。
3. registry 原子替换 active set。
4. `RouterInputGate` active/blocked snapshot 与预建 `Pickable` values 原子替换。
5. RouterEngine 以预检过的 sealed ticket 发布 presentation、history 与 diagnostics。
6. request 进入 `Running(PostCommitCleanup)`；离场 root leaf-to-root 删除，sealed cleanup batch 的全部 obligation 收到 terminal receipt 后才终结。

完整 preflight 后，第 1 至 5 步只允许对已经存在的 Router-private lifecycle/registry/gate 数据，以及预建 `Visibility/Pickable` values做 in-place mutation；禁止 insert/remove/despawn、relationship write、bundle effect、callback/hook、fallible lookup/validation，也禁止 core 先 commit 后尝试 ECS activation。`InteractionDisabled` 等 marker 不参与 switch。由此 switch 本身不触发生命周期重入并保持不可失败。正常 receipt 后 request 为 `Succeeded`。post-commit cleanup 的despawn/relationship操作仍按可重入路径处理：immutable obligation只冻结exact owned duty，不冻结物理disposition；每个版本化attempt按最新World证据重新分类，mutation后复验，漂移时supersede旧attempt并把未结算residual安全转为新attempt或quarantine lease。异常不回滚已发布presentation/history，而是暂停队列、进入`RecoveringAuthority`，settle后仍以`Succeeded`终结并附带高严重度diagnostic。

## 10. History、error recovery 与 cleanup

push/replace/back 先创建 history intent；只有正常 route presentation 原子提交成功才应用。back 先预览，UI commit 成功后才移动 cursor。

`back()` 的同步 admission 只读取上一轮 published history snapshot：snapshot 已无前项时返回 `NavigationEnqueueErrorKind::NoBackEntry` 且不创建 request。已入队 back 在成为 running 时必须重新预览权威 history；前序 queued command 可能已移动 cursor。若此时仍有前项，就以当时的目标继续；若已无前项，则该 request 以 `Failed(BackTargetUnavailable)` 终结，active/history 不变、不渲染 fallback，并通过零 obligation cleanup batch 收敛。前序 back 失败不会移动 cursor，因此后续 queued back 可以重新预览同一目标。

```text
ActivePresentation::Route { location, frames }
ActivePresentation::Error { requested, matched: Option<_>, boundary, frames, cause }
```

Error presentation 是已提交 UI，但对应 request 为 `Failed`，history intent 被取消。match failure 时 `matched = None`；mandatory router fallback 直接使用当前 host version 的既有 `TopSlot` 构建 `RouterFallback` frame。fallback 失败则保留旧 presentation，首次导航保持 `None`。

route apply/mount failure 从失败 route 向上选择最近绑定 boundary；只保留严格位于 boundary index 之上的合法 frame，新建 `RouteError` frame并清理其余 suffix。error frame 使用新 frame id，不被正常导航复用。Host/authority loss 没有合法 fallback。

Cleanup receipt disposition：

- `Removed`：正常 router-owned subtree 已删除。
- `RemovedDetached`：exact router-owned Entity 漂移，且 actual inbound parent 已验证为 missing、same-plan Router-owned 或授权 host-top-slot edge；先隔离 visibility、pointer、keyboard、gamepad 与 focus，再删除并记录高严重度诊断。未授权 parent 永远不能使用此 disposition。
- `AlreadyAbsent`：完整 Entity generation 已不存在；只收敛 registry。
- `RejectedProtected`：错误映射指向 host/其他 app-owned Entity；绝不删除，撤销映射并记录 invariant violation。
- `QuarantinedForeignDescendant`：owned ancestor 下存在未登记 Entity；只隔离 owned ancestor并释放 focus，不触碰 foreign entity，保留 quarantine 供后续重试。
- `QuarantinedProtectedParent`：exact owned root 的实际 inbound parent 是未授权 app-owned/protected Entity；只隔离 owned root，不删除 child、不移除 relationship、不改写 parent，保留独立 lease。
- `QuarantinedForeignComponent`：Router-owned Entity 上出现 manifest 外 component；不删除 Entity、不移除 component、不触发其 hook，保留 inert lease直到应用移除。

每次前台清理都使用 sealed cleanup batch。ECS 从 preflight/settlement plan 生成不可变 entity obligation manifest；RouterEngine 只校验逻辑 roots 后签发绑定 router、request、transaction、host epoch、batch id、manifest digest 与 obligation count 的 opaque `CleanupBatchToken`。ECS 按 token 保存唯一 ledger、接收 entity-level receipts；全部 obligation 恰好结算一次后，才把原 token 与 completion digest 交回 engine。Core 不读取 Entity，ECS 不能自行终结 request；无 obligation 的 batch 在同一阶段立即完成。重复 receipt、错误 batch、旧 epoch 和未知 token 都不得改变 ledger 或 request。详细转换以 Matrix 为准。

三种 quarantine disposition 都是 foreground settlement 的 terminal receipt：产生 receipt 时，owned root 已 hidden、三类输入均阻断、subtree focus 已释放，并从 active/pending registry 原子移入独立 `QuarantineLease` 表。Running request 可据此终结；旧 host settlement 可完成，新 host admission 和后续 queue 不被永久关系阻塞。

转入 quarantine 时签发全局唯一 `QuarantineLeaseId + lease generation + operation sequence`；该 live lease 的 authority 只覆盖 immutable exact owned set，与 `former_host_version` 解耦，后者仅用于 diagnostics。因而新 host epoch 建立后，live old-epoch lease 仍可对自己的 exact set 重施 inert barrier并安全 reap，却不能查询、改写或 settle 新 epoch。只有 closed/unknown lease、错误 lease generation/operation sequence、重复 completion，或旧 host cleanup/settlement token 的 replay 才是 stale。后台仅在 foreign descendants 与 protected parent edge 均消失后删除 exact owned residual；永久阻塞只保留 inert lease。

Top-slot quarantine 在 transfer 前还有强制 detach gate：旧 `HostLease` 仍有效时先移除且只移除旧 top slot 的授权 host edge，并写后复验 host 不再把该 exact slot 作为 child。若 observer 使 slot `AlreadyAbsent`，可直接收敛；若仍连接旧 host，settlement 保持 nonterminal并重试/诊断；若被应用改挂到其他 protected parent，则按 `QuarantinedProtectedParent` transfer。只有旧 host edge 已消失后才能确认 `HostSettlementTicket`、关闭旧 HostLease并允许同一 app host Entity 获得新 epoch。因此后台 quarantine reaper 永不需要修改 current/new epoch host relationship。

| cleanup 场景 | presentation/history | request 终态 | queue |
| --- | --- | --- | --- |
| normal apply 失败且 recovery 失败 | 保留旧 presentation/history | receipt 后 `Failed` | settlement 后继续 |
| error presentation recovery 成功 | error presentation 提交，history 不变 | suffix cleanup 后 `Failed` | terminal 后继续 |
| normal post-commit cleanup | 新 presentation/history 已发布 | `PostCommitCleanup` 后 `Succeeded` | 等待 terminal |
| host/authority loss after `CommitPublished`、foreground cleanup 前 | active经settlement清空；新history不回滚 | ledger转交/settlement后 `Succeeded` + severe `PostCommitCleanupAnomaly` | recovery期间暂停；queued request失败 |
| other post-commit anomaly | 不回滚新 presentation/history | recovery 后 `Succeeded` + diagnostic | recovery 期间暂停 |
| stale output cleanup | 当前 presentation/history 不变 | 当前 request 不变 | 不受旧 report 影响 |
| host/active graph loss before `CommitPublished` | active 变 `None`，history保持旧值 | running/queued requests `Failed` | host/authority恢复前拒绝 |

移除 host registration 时，先隔离/释放 focus，再清理 router-owned 顶层 slot，最后撤销映射，host 保持。应用 despawn host 时只以 `AlreadyAbsent` 收敛。Active root/slot/outlet 被删除、reparent 或清空 marker 时走 `AuthorityLost`：按逐实体 ownership 证据隔离、删除或 quarantine，core/ECS active 同时清空；host declaration 仍合法时重建顶层 slot并回到 `Ready + None`。running request 只在 settlement 完成后进入终态。

## 11. Diagnostics 与验收观察面

`RouterSnapshot::requested/matched/pending_phase/running_request`只汇总当前running request：没有running request时四者均为`None`，queued、active或last-terminal request不得占用这些字段。每个`NavigationRequestSnapshot`永久保存自身canonical requested location；match成功后保存immutable fingerprint，match失败或尚未完成match时为`None`，具体阶段由status/trace区分。`ActivePresentation`单独暴露已提交route location或error requested/matched、frames与failure，不能从router running摘要推断。其余router snapshot包含availability、history cursor/can-back、最早保留request sequence、cleanup summary和router-scoped diagnostics；request snapshot另含request-scoped diagnostics。`RequestStatus::Failed`携带可分类的只读failure。精确签名以Facade Contract Fixture为准。

`NavigationRequestSnapshot` 保存只读 lifecycle trace：request phase changed、history intent rejected、mount requested/resolved/granted/rejected、shell prepared、view applied、outlet registered、frame isolated/activated、commit published、cleanup disposition 和 request terminal。mount rejection 结构化区分 `MissingOutlet`、`InvalidOutlet`、`ParentAuthorityLost` 与 `RejectedAfterMutation`；事件只在语义转移时追加，不记录 polling。即使一次 update 完成多个阶段，也保留顺序证据。`Running(CleanupPending|Recovering|PostCommitCleanup)` 必须先于对应 cleanup settled，terminal 事件必须最后追加，因此消费者不依赖 update 间恰好可见的中间 snapshot。

`ecs-test-support` 的 `RouterAuthorityAudits` 明确实现 Bevy `Resource`，其只读 `RouterAuthorityAuditSnapshot` 包含 host version/app-host match、全部 slot、core frame 与 registry exact Entity 的一一对应关系、`parent frame + outlet key -> anchor + optional child slot` 拓扑、production primitive mapping、frame lifecycle、quarantine lease owner/kind/count，以及每次 lifecycle mutation 后的 visibility、pointer、keyboard、gamepad 和 focus 检查结果。该公开test-support面只证明consumer需要关联的route/primitive/输入/cleanup事实，不枚举content target、`spawn_empty`结构output或app ancestor；这些纯内部Node/edge/manifest不变量由固定ECS私有locked-Bevy回归直接查询World证明，不能为了测试方便扩展facade。

该 Resource 不等待 `PublishSnapshots`：Drive、Commit、Cleanup、Reaper 的每次相关 mutation必须在同一 exclusive block中同步更新audit mapping/event，因而测试系统可按 `after(DriveNavigation).before(CommitPresentation)` 和 `after(CommitPresentation).before(SettleCleanup)` 读取真实瞬态。公共映射只使用稳定、全局唯一、独立分配、不可还原且无法转换为 Bevy `Entity` 的 opaque `AuditEntityId`；它不编码 Entity index/generation/hash，允许跨 router/snapshot 比较，但不能作为 World query/mutation selector。Host audit只提供一个接受应用已知host `Entity` 的equality probe，不返回任何raw internal Entity或app ancestor identity。

Audit registry只为从router-owned mapping取得的ID提供`owned_entity_state -> PresentOwned | Quarantined | Removed | NotRetained | Unknown`；host equality ID和observed foreign target ID不在该方法定义域，返回`Unknown`且由调用方按来源区分。`Removed`只能在exact Entity generation已不存在且registry不再拥有它时发布，不能由cleanup receipt直接推导。活跃、prepared、leaving、quarantine owned ID永不淘汰；removed tombstone至少保留到引用它的request record淘汰，并另外保留最近256个FIFO。这样Journey B可保存旧leaf audit id并独立证明其exact Entity已移除。Audit没有marker constructor、World mutation或commit capability；它与ECS私有regression共同证明从未双active、host/outlet未被无理由重建和core/ECS一致。

`ecs-test-support` 另提供与 audit 分离、同样实现 Bevy `Resource` 的 bounded `RouterInputProbes`：只接受 `RouterHandle + AuditEntityId + RouterInputModality`，内部解析 exact target并通过生产 `RouterInputGate` 发送到 test-only hook-free counter observer，返回 `Delivered | Blocked | Unknown`；它不能取得 raw Entity、注册 callback、写 Router lifecycle/registry或触发应用 observer。Quarantine snapshot 可只读暴露 root 和 observed foreign targets 的 opaque audit ids。Public input test 用 probe 证明 gate/dispatch；ECS private regression 再用真实 Bevy target event observer证明同样结果。

## 12. 必须封闭的旧路径与非目标

- facade 推荐路径不接受外部 `MatchedBranch`、`BranchDiff`、apply token 或 commit capability。
- authority marker、mount/commit capability、report acceptance/replay capability、session constructor 和 registry writer 均不得从 facade 导出；driver 必需的一次性 `EcsViewReportSession` 参数类型及其 terminal report methods 按 allowlist 保留。
- renderer 不创建 authority root，测试不通过内部 runtime 驱动二次导航。
- plugin 必须注册真实 system chain。

这是 pre-MVP `0.1.0` 的一次有意破坏性 authority migration：`RouterEcsState`、`EcsFrameApplySession`、`RouteHost`/`RouteSlot`/`RouteFrameRoot` marker、core `RouterRuntime` authority entry 和 view `ViewReportSession::from_frame_request` 不再作为 facade/adapter 公共能力，改为 private authority 或只读 snapshot。安全边界不设弃用窗口，迁移说明随 Phase 3B.1 提交。

Facade allowlist 的逐 symbol 合同见 [Facade Export Allowlist](facade-export-allowlist.md)。本节只概括模块角色：

- `core` module：location、route tree、matcher、branch/diff、renderer-neutral keys、只读 IDs/presentation/request/diagnostics；不含 runtime/transaction mutation capability。
- `view` module：view keys、descriptor、registry与 effect/report 数据；不含旧 renderer test protocol或 acceptance session。
- `ecs` module：App extension、config/handle、host registration、navigation/snapshot resources、driver/writer trait、sealed production primitive、只读 lifecycle/audit snapshot；不含 state、marker、apply session、mount/commit/cleanup capability。
- `bsn`/`platform`：各自 adapter 或 router-neutral re-export；不获得 authority capability。

针对旧 `bevy_ui_router::{core,view,ecs}::*` authority 路径增加 compile-fail 测试。`ViewBindingKey` 不保留 alias，迁移到 `RouteViewKey`。

现有 `platform` feature 与 router-neutral back re-export 保持不变；`devtools` 保留为空的 reserved feature。本阶段不扩展两者，但 feature matrix 必须继续证明可独立编译。

暂停 Phase 3B Task 6、Phase 3C、BSN queued/streaming、file scene、parallel outlet graph、cache、transition、loader/action/revalidation 和平台壳层，直到 MVP journeys 全部通过。MVP 的 `SolidRect` 不属于暂停项；它是 facade 可达与真实可视 Journey PASS 的必要能力。
