# Facade Contract Fixture

**文档类型**: Phase 3B.1 公共 API 规范附录
**状态**: Phase 3B.1 `Plan PASS`
**上位规范**: [Target Architecture](target-architecture.md)

## 1. 最小公开接口

除 Target Architecture 已列出的 facade 类型外，fixture 固定以下可调用观察/构建接口：

```rust
impl RouteViewKey {
    pub fn new(value: impl Into<String>) -> Result<Self, RouteViewKeyError>;
}

impl RouteLocation {
    pub fn new(path: impl Into<String>) -> Self;
    pub fn with_search(self, search: impl Into<String>) -> Self;
    pub fn with_fragment(self, fragment: impl Into<String>) -> Self;
    pub fn path(&self) -> &str;
    pub fn search(&self) -> Option<&str>;
    pub fn fragment(&self) -> Option<&str>;
}

// Contract: RouteLocation::new removes empty slash-separated path segments;
// root remains "/", non-root paths have one leading slash and no trailing slash.

impl RouteParams {
    pub fn get(&self, name: &str) -> Option<&str>;
}

impl ErrorBoundaryKey {
    pub fn named(name: impl Into<String>) -> Result<Self, RouteError>;
}

impl RoutePattern {
    pub fn path(pattern: impl Into<String>) -> Result<Self, RouteError>;
}

impl RouteTree {
    pub fn builder() -> RouteTreeBuilder;
}

// RouteViewKey: Clone + Debug + Eq + Hash
// ViewDriverKey: Clone + Debug + Eq + Hash
// RouteSlotKey: Default + Clone + Debug + Eq + Hash

impl ViewDriverKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ViewDriverKeyError>;
}

impl RouteViewRegistry {
    pub fn new() -> Self;
    pub fn insert(
        &mut self,
        view: RouteViewKey,
        descriptor: RouteViewDescriptor,
    ) -> Result<(), RouteViewRegistryError>;
}

impl RouteViewDescriptor {
    pub fn new(driver: ViewDriverKey) -> Self;
}

impl RouteNodeOptions {
    pub fn new() -> Self;
    pub fn with_view(self, view: RouteViewKey) -> Self;
    pub fn with_error_boundary(self, boundary: ErrorBoundaryKey) -> Self;
    pub fn with_error_view(
        self,
        boundary: ErrorBoundaryKey,
        view: RouteViewKey,
    ) -> Self;
}

impl RouteTreeBuilder {
    pub fn root_with_options(
        self,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> Result<Self, RouteError>;
    pub fn root_id(&self) -> Option<RouteId>;
    pub fn route_with_options(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
        options: RouteNodeOptions,
    ) -> Result<RouteId, RouteError>;
    pub fn build(self) -> Result<RouteTree, RouteError>;
}

impl ViewBuildEffect {
    pub fn view(&self) -> &RouteViewKey;
    pub fn request_id(&self) -> ViewRequestId;
    pub fn input(&self) -> &RouteViewInput;
}

impl RouteViewInput {
    pub fn location(&self) -> &RouteLocation;
    pub fn route(&self) -> Option<RouteId>;
    pub fn params(&self) -> &RouteParams;
    pub fn raw_search(&self) -> Option<&str>;
    pub fn fragment(&self) -> Option<&str>;
    pub fn origin(&self) -> &FrameOrigin;
    pub fn failure(&self) -> Option<&NavigationFailure>;
}

impl ContentColor {
    pub const fn srgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self;
    pub const fn red(&self) -> u8;
    pub const fn green(&self) -> u8;
    pub const fn blue(&self) -> u8;
    pub const fn alpha(&self) -> u8;
}

impl SolidRect {
    pub fn new(
        width_px: u16,
        height_px: u16,
        color: ContentColor,
    ) -> Result<Self, ContentPrimitiveError>;
    pub const fn width_px(&self) -> u16;
    pub const fn height_px(&self) -> u16;
    pub const fn color(&self) -> ContentColor;
}

impl ContentPrimitiveError {
    pub fn kind(&self) -> ContentPrimitiveErrorKind;
}

pub enum ContentPrimitiveErrorKind {
    ZeroWidth,
    ZeroHeight,
}

pub enum ContentPrimitiveKind {
    SolidRect,
}

pub struct BevyUiRouterPlugin;

// BevyUiRouterPlugin: Default + bevy_app::Plugin

impl FrameContentWriter<'_> {
    pub fn spawn_empty(&mut self) -> Result<ContentEntity, ContentWriteError>;
    pub fn spawn_solid_rect(
        &mut self,
        rect: SolidRect,
    ) -> Result<ContentEntity, ContentWriteError>;
    pub fn add_child(
        &mut self,
        parent: ContentEntity,
        child: ContentEntity,
    ) -> Result<(), ContentWriteError>;
    pub fn despawn(&mut self, entity: ContentEntity) -> Result<(), ContentWriteError>;
    pub fn declare_outlet(
        &mut self,
        anchor: ContentEntity,
        key: RouteSlotKey,
    ) -> Result<(), ContentWriteError>;
}

impl ContentWriteError {
    pub fn kind(&self) -> ContentWriteErrorKind;
}

pub enum ContentWriteErrorKind {
    UnknownHandle,
    ForeignSession,
    SelfParent,
    DuplicateEdge,
    DuplicateParent,
    RelationshipCycle,
    DuplicateOutlet,
    InvalidDespawn,
}

impl EcsViewReportSession<'_> {
    pub fn applied(self) -> ViewBuildReport;
    pub fn build_failed(self, message: impl Into<String>) -> ViewBuildReport;
    pub fn apply_failed(self, message: impl Into<String>) -> ViewBuildReport;
}

impl ViewBuildReport {
    pub fn kind(&self) -> ViewBuildReportKind;
    pub fn message(&self) -> Option<&str>;
}

pub enum ViewBuildReportKind {
    Applied,
    BuildFailed,
    ApplyFailed,
}

impl NavigationRequestSnapshot {
    pub fn requested(&self) -> &RouteLocation;
    pub fn matched(&self) -> Option<&RouteBranchFingerprint>;
    pub fn status(&self) -> &RequestStatus;
    pub fn trace(&self) -> &[NavigationTraceEvent];
    pub fn diagnostics(&self) -> &[DiagnosticEvent];
}

pub enum RequestStatus {
    Queued,
    Running(NavigationPhase),
    Succeeded,
    Failed(NavigationFailure),
}

impl NavigationFailure {
    pub fn kind(&self) -> NavigationFailureKind;
    pub fn message(&self) -> &str;
}

pub enum NavigationFailureKind {
    Match,
    Build,
    Apply,
    Mount,
    HostLost,
    AuthorityLost,
    RecoveryFailed,
    BackTargetUnavailable,
}

impl RouterSnapshot {
    pub fn requested(&self) -> Option<&RouteLocation>;
    pub fn matched(&self) -> Option<&RouteBranchFingerprint>;
    pub fn active(&self) -> &ActivePresentation;
    pub fn availability(&self) -> &RouterAvailability;
    pub fn pending_phase(&self) -> Option<&NavigationPhase>;
    pub fn history(&self) -> &HistorySnapshot;
    pub fn running_request(&self) -> Option<NavigationRequestId>;
    pub fn earliest_retained_sequence(&self) -> Option<u64>;
    pub fn cleanup(&self) -> &CleanupSummary;
    pub fn host(&self) -> &HostSnapshot;
    pub fn diagnostics(&self) -> &DiagnosticSnapshot;
}

impl ActivePresentation {
    pub fn kind(&self) -> ActivePresentationKind;
    pub fn location(&self) -> Option<&RouteLocation>;
    pub fn requested(&self) -> Option<&RouteLocation>;
    pub fn matched(&self) -> Option<&RouteBranchFingerprint>;
    pub fn frames(&self) -> &[RouteFrameVersion];
    pub fn failure(&self) -> Option<&NavigationFailure>;
}

pub enum ActivePresentationKind {
    None,
    Route,
    Error,
}

pub enum RouterAvailability {
    AwaitingHost,
    Ready,
    RecoveringAuthority,
    Unavailable(RouterUnavailableReason),
}

impl RouterAvailability {
    pub fn unavailable_reason(&self) -> Option<RouterUnavailableReason>;
}

pub enum RouterUnavailableReason {
    HostConflict,
    InvalidHostUiTopology,
    HostLost,
    AuthorityEstablishmentFailed,
    RegistryInvariantViolation,
}

impl NavigationEnqueueError {
    pub fn kind(&self) -> NavigationEnqueueErrorKind;
}

impl ViewDriverRegistrationError {
    pub fn kind(&self) -> ViewDriverRegistrationErrorKind;
}

pub enum ViewDriverRegistrationErrorKind {
    DuplicateKey,
}

pub enum NavigationEnqueueErrorKind {
    UnknownRouter,
    HostUnavailable,
    AuthorityRecovering,
    NoBackEntry,
}

impl RouterInstallError {
    pub fn kind(&self) -> RouterInstallErrorKind;
}

pub enum RouterInstallErrorKind {
    MissingRouteView,
    MissingErrorView,
    UnknownRouteView,
    UnknownViewDriver,
    MissingFallbackView,
}

pub enum RouterScheduleSet {
    ObserveAuthority,
    CollectNavigation,
    DriveNavigation,
    CommitPresentation,
    SettleCleanup,
    ReapQuarantines,
    PublishSnapshots,
}

impl RouterInputGate {
    pub fn allows_pointer(&self, target: bevy_ecs::entity::Entity) -> bool;
    pub fn allows_keyboard(&self, target: bevy_ecs::entity::Entity) -> bool;
    pub fn allows_gamepad(&self, target: bevy_ecs::entity::Entity) -> bool;
}

#[cfg(feature = "ecs-test-support")]
impl RouterInputProbes {
    pub fn dispatch(
        &mut self,
        router: RouterHandle,
        target: AuditEntityId,
        modality: RouterInputModality,
    ) -> InputProbeResult;
    pub fn delivery_count(
        &self,
        target: AuditEntityId,
        modality: RouterInputModality,
    ) -> u64;
}

#[cfg(feature = "ecs-test-support")]
pub enum RouterInputModality {
    Pointer,
    Keyboard,
    Gamepad,
}

#[cfg(feature = "ecs-test-support")]
pub enum InputProbeResult {
    Delivered,
    Blocked,
    Unknown,
}

pub enum RouterInputSet {
    Refresh,
    Ready,
}

impl HistorySnapshot {
    pub fn current(&self) -> Option<&RouteLocation>;
    pub fn entries(&self) -> &[RouteLocation];
    pub fn cursor(&self) -> Option<usize>;
    pub fn len(&self) -> usize;
    pub fn can_back(&self) -> bool;
}

impl HostSnapshot {
    pub fn version(&self) -> Option<&HostVersion>;
    pub fn top_slot(&self) -> Option<RouteSlotId>;
    pub fn trace(&self) -> &[HostTraceEvent];
}

impl HostTraceEvent {
    pub fn kind(&self) -> HostTraceEventKind;
    pub fn version(&self) -> Option<&HostVersion>;
    pub fn slot(&self) -> Option<RouteSlotId>;
}

impl CleanupSummary {
    pub fn foreground_pending(&self) -> bool;
    pub fn quarantine_count(&self) -> usize;
    pub fn last_disposition(&self) -> Option<CleanupDisposition>;
}

impl DiagnosticSnapshot {
    pub fn events(&self) -> &[DiagnosticEvent];
}

impl DiagnosticEvent {
    pub fn kind(&self) -> DiagnosticEventKind;
    pub fn severity(&self) -> DiagnosticSeverity;
    pub fn request(&self) -> Option<NavigationRequestId>;
    pub fn frame(&self) -> Option<&RouteFrameVersion>;
    pub fn report_disposition(&self) -> Option<ReportDisposition>;
    pub fn redacted_message(&self) -> &str;
}

pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Severe,
}

pub enum DiagnosticEventKind {
    PendingOverwrite,
    HostOrSlotUnavailable,
    MatchFailed,
    MatchConflict,
    BranchPlanned,
    FrameLifecycle,
    ReportDisposition,
    CleanupDisposition,
    HostLifecycle,
    AuthorityInvariantViolation,
    PostCommitCleanupAnomaly,
}

pub enum ReportDisposition {
    Accepted,
    StaleDiscarded,
    DuplicateRejected,
    InvalidRejected,
}

impl NavigationTraceEvent {
    pub fn kind(&self) -> NavigationTraceEventKind;
    pub fn frame(&self) -> Option<&RouteFrameVersion>;
    pub fn outlet(&self) -> Option<&RouteSlotKey>;
    pub fn slot(&self) -> Option<RouteSlotId>;
    pub fn phase(&self) -> Option<&NavigationPhase>;
    pub fn mount_rejection(&self) -> Option<MountRejectionKind>;
    pub fn cleanup_disposition(&self) -> Option<CleanupDisposition>;
}

impl RouteFrameVersion {
    pub fn router(&self) -> RouterInstanceId;
    pub fn host(&self) -> RouteHostId;
    pub fn slot(&self) -> RouteSlotId;
    pub fn origin(&self) -> &FrameOrigin;
    pub fn frame(&self) -> RouteFrameId;
    pub fn generation(&self) -> RouteGeneration;
}

pub enum RequestLookup<'a> {
    Found(&'a NavigationRequestSnapshot),
    NotRetained,
    Unknown,
}

pub enum NavigationTraceEventKind {
    FrameTaskIssued,
    RequestPhaseChanged,
    HistoryIntentRejected,
    MountRequested,
    MountResolved,
    MountGranted,
    MountRejected,
    MountRejectedAfterMutation,
    ShellPrepared,
    ViewApplied,
    OutletRegistered,
    FrameIsolated,
    FrameActivated,
    CommitPublished,
    CleanupSettled,
    RequestTerminal,
}

pub enum MountRejectionKind {
    MissingOutlet,
    InvalidOutlet,
    ParentAuthorityLost,
    RejectedAfterMutation,
}

pub enum HostTraceEventKind {
    AdmissionRequested,
    AdmissionRejected,
    AdmissionRejectedAfterMutation,
    HostGranted,
    TopSlotRegistered,
    HostReady,
    SettlementStarted,
    SettlementCompleted,
}

pub enum CleanupDisposition {
    Removed,
    RemovedDetached,
    AlreadyAbsent,
    RejectedProtected,
    QuarantinedForeignDescendant,
    QuarantinedProtectedParent,
    QuarantinedForeignComponent,
}

#[cfg(feature = "ecs-test-support")]
impl RouterAuthorityAudits {
    pub fn get(&self, router: RouterHandle) -> Option<&RouterAuthorityAuditSnapshot>;
}

// RouterAuthorityAudits: bevy_ecs::resource::Resource + mandatory Component supertrait
// RouterInputProbes: bevy_ecs::resource::Resource + mandatory Component supertrait

#[cfg(feature = "ecs-test-support")]
impl RouterAuthorityAuditSnapshot {
    pub fn host(&self) -> Option<&AuthorityHostMapping>;
    pub fn slots(&self) -> &[AuthoritySlotMapping];
    pub fn frames(&self) -> &[AuthorityFrameMapping];
    pub fn outlets(&self) -> &[AuthorityOutletMapping];
    pub fn content(&self) -> &[AuthorityContentMapping];
    pub fn orphan_slots(&self) -> &[RouteSlotId];
    pub fn quarantines(&self) -> &[QuarantineSnapshot];
    pub fn events(&self) -> &[RouterLifecycleAuditEvent];
    pub fn owned_entity_state(&self, entity: AuditEntityId) -> AuditEntityState;
}

#[cfg(feature = "ecs-test-support")]
impl AuthorityContentMapping {
    pub fn frame(&self) -> &RouteFrameVersion;
    pub fn entity(&self) -> AuditEntityId;
    pub fn primitive(&self) -> ContentPrimitiveKind;
    pub fn width_px(&self) -> u16;
    pub fn height_px(&self) -> u16;
    pub fn color(&self) -> ContentColor;
    pub fn visibility_hidden(&self) -> bool;
    pub fn pickable_active(&self) -> bool;
}

#[cfg(feature = "ecs-test-support")]
impl AuthorityHostMapping {
    pub fn version(&self) -> &HostVersion;
    pub fn entity(&self) -> AuditEntityId;
    pub fn top_slot(&self) -> AuditEntityId;
    pub fn matches_app_entity(&self, entity: bevy_ecs::entity::Entity) -> bool;
}

#[cfg(feature = "ecs-test-support")]
impl AuthoritySlotMapping {
    pub fn id(&self) -> RouteSlotId;
    pub fn entity(&self) -> AuditEntityId;
    pub fn host(&self) -> &HostVersion;
    pub fn kind(&self) -> AuthoritySlotKind;
    pub fn parent_frame(&self) -> Option<&RouteFrameVersion>;
    pub fn outlet(&self) -> Option<&RouteSlotKey>;
}

#[cfg(feature = "ecs-test-support")]
pub enum AuthoritySlotKind {
    HostTop,
    ChildOutlet,
}

#[cfg(feature = "ecs-test-support")]
impl AuthorityOutletMapping {
    pub fn parent(&self) -> &RouteFrameVersion;
    pub fn key(&self) -> &RouteSlotKey;
    pub fn anchor(&self) -> AuditEntityId;
    pub fn child_slot(&self) -> Option<AuditEntityId>;
}

#[cfg(feature = "ecs-test-support")]
impl AuthorityFrameMapping {
    pub fn frame(&self) -> &RouteFrameVersion;
    pub fn root(&self) -> AuditEntityId;
    pub fn slot_entity(&self) -> AuditEntityId;
    pub fn lifecycle(&self) -> AuthorityFrameLifecycle;
}

#[cfg(feature = "ecs-test-support")]
pub enum AuditEntityState {
    PresentOwned,
    Quarantined,
    Removed,
    NotRetained,
    Unknown,
}

#[cfg(feature = "ecs-test-support")]
pub enum AuthorityFrameLifecycle {
    PreparedInert,
    Active,
    LeavingInert,
    Quarantined,
}

#[cfg(feature = "ecs-test-support")]
impl QuarantineSnapshot {
    pub fn former_host(&self) -> &HostVersion;
    pub fn root(&self) -> AuditEntityId;
    pub fn foreign_targets(&self) -> &[AuditEntityId];
    pub fn root_owner(&self) -> &QuarantineOwnerSnapshot;
    pub fn foreign_count(&self) -> usize;
    pub fn foreign_component_count(&self) -> usize;
    pub fn protected_parent(&self) -> bool;
    pub fn inert(&self) -> bool;
}

#[cfg(feature = "ecs-test-support")]
impl QuarantineOwnerSnapshot {
    pub fn lease_id(&self) -> QuarantineLeaseId;
    pub fn kind(&self) -> QuarantineOwnerKind;
    pub fn former_host(&self) -> &HostVersion;
    pub fn entity_count(&self) -> usize;
}

#[cfg(feature = "ecs-test-support")]
pub enum QuarantineOwnerKind {
    HostTopSlot,
    ChildSlot,
    FrameShell,
    FrameOutput,
}

#[cfg(feature = "ecs-test-support")]
impl RouterLifecycleAuditEvent {
    pub fn kind(&self) -> RouterLifecycleAuditEventKind;
    pub fn frame(&self) -> Option<&RouteFrameVersion>;
    pub fn entity(&self) -> Option<AuditEntityId>;
    pub fn visibility_hidden(&self) -> bool;
    pub fn accepts_pointer(&self) -> bool;
    pub fn accepts_keyboard(&self) -> bool;
    pub fn accepts_gamepad(&self) -> bool;
    pub fn focused(&self) -> bool;
}

#[cfg(feature = "ecs-test-support")]
pub enum RouterLifecycleAuditEventKind {
    ShellPrepared,
    FrameIsolated,
    FrameActivated,
    RegistryPublished,
    QuarantineOpened,
    QuarantineReaped,
}
```

`ContentEntity` 是 `Clone + Copy + Debug + Eq + Hash`、带 writer-session identity 的不透明句柄，不是裸 `Entity`。它可以由测试 driver 暂存以验证跨 session 拒绝，但不能用于 World 查询或 mutation。`RouteViewInput` 只读，不包含 authority identity。

Writer 只维护 session-local `ContentPlan`，所有方法在 driver 返回前 World 零变化。`spawn_empty` 创建无业务primitive数据的结构handle，但成功realization必须把它变为100% width/height、透明、始终`Pickable::IGNORE`的Router-owned `Node`，不能留下physical non-Node Entity；它可作为parent/outlet anchor并进入FrameOutput manifest/audit。`spawn_solid_rect` 只接受已验证的 data-only `SolidRect`，不存在接受 `Bundle`、component、asset handle、effect、callback、hook 或 observer 的重载；hierarchy 只能通过同 session handles调用 `add_child`，且在写 plan 前拒绝 self-edge、duplicate edge/parent 和 visited-reachability cycle。`despawn` 只接受本 session 创建、仍为空且尚未声明为 outlet/authority boundary 的 planned output；driver 返回成功后 ECS 才 staging-realize完整 plan并复验。方法不暴露 raw `World`、`Commands` 或 Entity。ECS 对所有handles先施加hidden/gate deny/`Pickable::IGNORE`并插入锁定Bevy `Node`递归closure；`SolidRect`再增加指定 `BackgroundColor`。

`ActivePresentation::location()` 只在 `Route` 返回值，`requested()`、`matched()` 与 `failure()` 只在 `Error` 返回值；match failure 的 error presentation 中 `matched()` 为 `None`，成功 match 后的 build/apply/mount recovery error 中为原 request 的 immutable fingerprint；`None` 的 frames 为空。`pending_phase()` 在没有 running request 时为 `None`；`earliest_retained_sequence()` 在尚无 retained terminal record 时为 `None`。`NavigationFailure` 只含分类和脱敏 message，不含 raw source、Entity 或 authority identity。

`AuditEntityId`、`QuarantineLeaseId` 都没有 raw constructor、数值 accessor 或到 Bevy `Entity` 的转换；前者由 audit registry 全局唯一、独立分配，不是 Entity bits/index/generation/hash 的投影，只实现 `Clone + Copy + Debug + Eq + Hash`，用于跨 router/audit snapshot 关联 exact Entity generation。`AuthorityHostMapping::matches_app_entity` 只做 equality probe，不返回 raw internal Entity；host/top-slot、slot/frame/outlet 的 Audit IDs 必须相互可连接。`AuthorityOutletMapping` 按 parent frame version 与 outlet key 唯一排序，anchor 可跨 leaf replacement 比较；child 未挂载时 `child_slot()` 为 `None`。

`owned_entity_state(id)` 只接受从top-slot、slot、frame root、content或quarantine-owned mapping取得的Router-owned ID；host equality ID与`foreign_targets()`中的observed app/foreign ID不在其定义域，传入时返回`Unknown`，consumer必须按ID来源区分。其 `Removed` 必须来自独立的 World-existence + registry-absence audit，不能直接信任 cleanup receipt；active/prepared/leaving/quarantine owned IDs永不淘汰，removed tombstone至少保留到引用request record被淘汰，并另保留最近256个。`QuarantineSnapshot::root_owner()` 描述lease root；`entity_count()` 是该lease immutable owned set的总数，`protected_parent()`区分inbound protected edge。Router-scoped diagnostics保留最近128个事件；只要request record尚被保留，其request-scoped diagnostics必须完整保留。Severe post-commit anomaly、结构化report disposition与invariant violation必须能通过request或router snapshot观察。

`RouterAuthorityAudits` 是 test-support 的实时 Bevy Resource，不受生产 `PublishSnapshots` 延迟控制。Drive/Commit/Cleanup/Reaper 每次 audit-relevant mutation 都在同一 exclusive block 内先更新 registry事实、再同步更新 mapping/event；因此位于相邻 `RouterScheduleSet` 之间的测试系统可获得 pending、active、leaving 与 quarantine 的当前 opaque IDs。`AuthorityContentMapping` 只描述由公开 writer 计划、ECS 解释的 production primitive；`visibility_hidden/pickable_active` 分别采样当前 `Visibility` 意图值与 `Pickable` active值，不把尚未运行 propagation 的 `InheritedVisibility` 冒充为同帧有效可见性。真实 effective visibility由安装完整 UI pipeline 后的私有物理回归验证。Mapping 不提供 raw component或 Entity。

`RouterInputProbes` 是 test-support 的封闭行为探针，不属于 audit snapshot：它只解析 opaque audit id、调用生产 gate并驱动 Router 自带的 hook-free counter observer；不能注册 consumer callback、返回 Entity、取得 World/registry或改变 lifecycle。`Blocked/Unknown` 不增加 counter，`Delivered` 恰增加对应 modality counter一次。

上面所有返回 `Result` 的 public error 类型都实现 `Debug + Display + std::error::Error + Send + Sync + 'static`。
所有用于 snapshot/trace 断言的 enum 和 ID 实现 `Debug + Eq + PartialEq`；无 payload 的 event/disposition enum 另实现 `Clone + Copy`。`ContentColor`、`SolidRect`、`ContentPrimitiveKind`、`RouterHandle`、`NavigationRequestId`、`ViewRequestId`、`AuditEntityId` 和 `QuarantineLeaseId` 实现 `Clone + Copy + Debug + Eq + Hash`。`RouterScheduleSet` 与 `RouterInputSet` 实现 `SystemSet + Clone + Copy + Debug + Eq + Hash`。`RouterInputGate`、`RouterNavigation`、`RouterSnapshots`、`RouterAuthorityAudits` 与 `RouterInputProbes` 都实现锁定Bevy的`bevy_ecs::resource::Resource`，并因此同时具有mandatory `bevy_ecs::component::Component` supertrait实现；`RouterInputGate`公开只读且没有mutation/raw-registry accessor，后两个类型仅在test-support下可达。

## 2. Immediate 测试 Driver

以下代码定义 public journeys 使用的唯一 fault driver 形状：

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy_ui_router::prelude::*;

#[derive(Clone)]
enum FixtureMode {
    Success { outlets: Vec<RouteSlotKey> },
    BuildFailure,
    ApplyFailure,
    MissingOutlet,
    DuplicateOutlet(RouteSlotKey),
    ForeignOutlet(RouteSlotKey),
    RelationshipCycle,
}

#[derive(Clone, Default)]
struct FixtureControl {
    modes: Arc<Mutex<HashMap<RouteViewKey, FixtureMode>>>,
    captured_anchor: Arc<Mutex<Option<ContentEntity>>>,
    observed_params: Arc<Mutex<Vec<(String, Option<String>, Option<String>)>>>,
}

struct FixtureDriver {
    key: ViewDriverKey,
    control: FixtureControl,
}

impl EcsViewDriver for FixtureDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        self.control
            .observed_params
            .lock()
            .expect("fixture observed-input lock must not be poisoned")
            .push((
                effect.input().location().path().to_owned(),
                effect.input().params().get("id").map(str::to_owned),
                effect.input().fragment().map(str::to_owned),
            ));

        let mode = self
            .control
            .modes
            .lock()
            .expect("fixture mode lock must not be poisoned")
            .get(effect.view())
            .cloned()
            .unwrap_or(FixtureMode::Success { outlets: Vec::new() });

        let solid_rect = || {
            SolidRect::new(
                320,
                180,
                ContentColor::srgba8(32, 160, 208, 255),
            )
            .expect("fixture solid rect dimensions must be non-zero")
        };

        match mode {
            FixtureMode::BuildFailure => report.build_failed("fixture build failure"),
            FixtureMode::ApplyFailure => {
                let _partial = content
                    .spawn_solid_rect(solid_rect())
                    .expect("fixture partial visible output should plan");
                report.apply_failed("fixture apply failure")
            }
            FixtureMode::MissingOutlet => {
                content
                    .spawn_solid_rect(solid_rect())
                    .expect("fixture missing-outlet content should plan");
                report.applied()
            }
            FixtureMode::DuplicateOutlet(key) => {
                let anchor = content
                    .spawn_solid_rect(solid_rect())
                    .expect("fixture visible outlet anchor should plan");
                content
                    .declare_outlet(anchor, key.clone())
                    .expect("first outlet declaration should succeed");
                let error = content
                    .declare_outlet(anchor, key)
                    .expect_err("duplicate outlet must be rejected");
                report.apply_failed(error.to_string())
            }
            FixtureMode::ForeignOutlet(key) => {
                let anchor = self
                    .control
                    .captured_anchor
                    .lock()
                    .expect("fixture captured-anchor lock must not be poisoned")
                    .expect("a prior settled session must capture an anchor");
                let error = content
                    .declare_outlet(anchor, key)
                    .expect_err("foreign session anchor must be rejected");
                report.apply_failed(error.to_string())
            }
            FixtureMode::RelationshipCycle => {
                let parent = content
                    .spawn_empty()
                    .expect("fixture parent handle should allocate");
                let child = content
                    .spawn_empty()
                    .expect("fixture child handle should allocate");
                content
                    .add_child(parent, child)
                    .expect("first relationship should be acyclic");
                let error = content
                    .add_child(child, parent)
                    .expect_err("relationship cycle must be rejected before realization");
                assert_eq!(error.kind(), ContentWriteErrorKind::RelationshipCycle);
                report.apply_failed(error.to_string())
            }
            FixtureMode::Success { outlets } => {
                if outlets.is_empty() {
                    let container = content
                        .spawn_empty()
                        .expect("fixture structural container should plan");
                    let visible = content
                        .spawn_solid_rect(solid_rect())
                        .expect("fixture leaf content should plan");
                    content
                        .add_child(container, visible)
                        .expect("fixture structural parent should accept visible child");
                }
                for key in outlets {
                    let anchor = content
                        .spawn_empty()
                        .expect("fixture structural outlet anchor should plan");
                    let visible = content
                        .spawn_solid_rect(solid_rect())
                        .expect("fixture visible outlet child should plan");
                    content
                        .add_child(anchor, visible)
                        .expect("fixture outlet anchor should accept visible child");
                    content
                        .declare_outlet(anchor, key)
                        .expect("fixture outlet should register");
                    *self
                        .control
                        .captured_anchor
                        .lock()
                        .expect("fixture captured-anchor lock must not be poisoned") = Some(anchor);
                }
                report.applied()
            }
        }
    }
}
```

Driver 通过测试自有的 `FixtureControl` 改变行为，不要求 Router 提供 driver unregister/replace API。重复 driver key 仍由 `AppRouterExt` 拒绝。

## 3. 可编译装配形状

```rust
use bevy_app::App;
use bevy_ecs::prelude::Component;
use bevy_ui::{Node, Val};
use bevy_ui_router::prelude::*;

#[derive(Component, Debug, Eq, PartialEq)]
struct HostSentinel(u32);

fn fixture_routes(
    layout: RouteViewKey,
    leaf: RouteViewKey,
    error: RouteViewKey,
) -> Result<RouteTree, RouteError> {
    let boundary = ErrorBoundaryKey::named("items")?;
    let mut builder = RouteTree::builder().root_with_options(
        RoutePattern::path("items")?,
        RouteNodeOptions::new()
            .with_view(layout)
            .with_error_view(boundary, error),
    )?;
    let root = builder.root_id().expect("fixture root must exist");
    builder.route_with_options(
        root,
        RoutePattern::path(":id")?,
        RouteNodeOptions::new().with_view(leaf),
    )?;
    builder.build()
}

fn install_fixture(app: &mut App) -> Result<(RouterHandle, FixtureControl), Box<dyn std::error::Error>> {
    let control = FixtureControl::default();
    let driver = FixtureDriver {
        key: ViewDriverKey::new("fixture")?,
        control: control.clone(),
    };
    let driver_key = app.register_ui_view_driver(driver)?;

    let layout = RouteViewKey::new("layout")?;
    let leaf = RouteViewKey::new("leaf")?;
    let error = RouteViewKey::new("error")?;
    let fallback = RouteViewKey::new("fallback")?;
    let routes = fixture_routes(layout.clone(), leaf.clone(), error.clone())?;

    let mut views = RouteViewRegistry::new();
    for view in [layout, leaf, error, fallback.clone()] {
        views.insert(view, RouteViewDescriptor::new(driver_key.clone()))?;
    }

    let router = app.add_ui_router(RouterConfig::new(routes, views, fallback))?;
    Ok((router, control))
}

fn install_preinstalled_fixture(
    app: &mut App,
) -> Result<(RouterHandle, FixtureControl), Box<dyn std::error::Error>> {
    app.add_plugins(BevyUiRouterPlugin::default());
    install_fixture(app)
}

fn push_and_settle(app: &mut App, router: RouterHandle, path: &str) -> NavigationRequestId {
    let request = app
        .world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new(path))
        .expect("fixture navigation should enqueue");

    for _ in 0..16 {
        app.update();
        let snapshots = app.world().resource::<RouterSnapshots>();
        if let RequestLookup::Found(snapshot) = snapshots.request(router, request) {
            if matches!(snapshot.status(), RequestStatus::Succeeded | RequestStatus::Failed(_)) {
                return request;
            }
        }
    }
    panic!("fixture request did not settle within 16 App::update calls");
}

fn first_navigation_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new();
    let (router, control) = install_fixture(&mut app)?;

    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(640.0),
                height: Val::Px(360.0),
                ..Default::default()
            },
            HostSentinel(7),
            RouterHostRegistration::new(router),
        ))
        .id();
    app.update();

    let layout = RouteViewKey::new("layout")?;
    control
        .modes
        .lock()
        .expect("fixture mode lock must not be poisoned")
        .insert(
            layout,
            FixtureMode::Success {
                outlets: vec![RouteSlotKey::default()],
            },
        );

    let request = push_and_settle(&mut app, router, "/items/1");
    let snapshots = app.world().resource::<RouterSnapshots>();
    let RequestLookup::Found(request_snapshot) = snapshots.request(router, request) else {
        panic!("completed request must remain retained");
    };
    assert_eq!(request_snapshot.status(), &RequestStatus::Succeeded);
    assert!(request_snapshot.trace().iter().any(|event| {
        event.kind() == NavigationTraceEventKind::CommitPublished
    }));
    assert!(control
        .observed_params
        .lock()
        .expect("fixture observed-input lock must not be poisoned")
        .iter()
        .any(|(path, id, fragment)| {
            path == "/items/1"
                && id.as_deref() == Some("1")
                && fragment.is_none()
        }));
    assert_eq!(app.world().get::<HostSentinel>(host), Some(&HostSentinel(7)));
    Ok(())
}
```

实现计划可以拆分文件，但不能改变上述消费者调用顺序和观察语义。上述成功 driver 必须同时计划至少一个 `spawn_empty` 结构container和一个 `SolidRect` child；`public_navigation_authority_audit` 通过 `AuthorityContentMapping` 证明primitive已被 ECS 解释为真实、可见且 active-pickable 的生产 UI。结构container的Node/parent/manifest/transparent/picking不变量由固定ECS私有locked-Bevy回归直接查询World证明，不增加公开结构映射。另一个隔离consumer必须调用 `install_preinstalled_fixture`，证明显式预装plugin后extension复用同一安装标记，schedule/resources与自动安装路径相同且没有重复system。

## 4. Failure 归属

| Failure | 注入位置 | Gate |
| --- | --- | --- |
| match failure | 导航到未匹配 path | facade Journey D |
| registered host不是 `Node`，或其实际 UI ancestry含 non-Node/cycle/ghost-only edge | 应用构造无效 app-owned host；Router不得补组件 | `RouterUnavailableReason::InvalidHostUiTopology` host admission consumer/audit test |
| route node missing normal view | 构造至少一个未调用 `with_view` 的 route node | `RouterInstallErrorKind::MissingRouteView` consumer test |
| route node declares boundary without error view | 在已有 normal view 的 node 上只调用 `with_error_boundary` | `RouterInstallErrorKind::MissingErrorView` consumer test |
| route node同时缺normal view与已声明boundary的error view | 同一node不调用`with_view`且只调用`with_error_boundary` | consumer test固定优先返回`RouterInstallErrorKind::MissingRouteView` |
| unresolved route/fallback binding | 构造缺项 `RouteViewRegistry` | `RouterInstallError` consumer test |
| unregistered driver | descriptor 引用未注册 `ViewDriverKey` | `RouterInstallError` consumer test |
| duplicate driver key | 连续注册相同 `ViewDriverKey` | `ViewDriverRegistrationErrorKind::DuplicateKey` consumer test；原 driver 保持 |
| build failure | `FixtureMode::BuildFailure` | facade Journey D |
| apply failure with partial output | `FixtureMode::ApplyFailure` | facade Journey D |
| missing outlet | layout `MissingOutlet` 返回 accepted plan，child mount 发出 token 后以 `MountRejected(MissingOutlet)` 失败 | facade Journey D |
| duplicate outlet | `FixtureMode::DuplicateOutlet` 在 plan 阶段返回 `ApplyFailed`；无 `MountRequested` | facade Journey D；crate-private fingerprint证明driver-return零写入 |
| foreign-session outlet | 首次成功 apply 捕获 anchor，后续 `FixtureMode::ForeignOutlet` 在 plan 阶段返回 `ApplyFailed`；无 `MountRequested` | facade Journey D；crate-private fingerprint证明driver-return零写入 |
| relationship cycle | `FixtureMode::RelationshipCycle` 只构造 plan handles | facade Journey D；crate-private fingerprint证明driver-return零写入 |
| entering realized outlet corruption | crate-private seam 在 accepted plan realization/post-write 后破坏 anchor identity/owner/seal/唯一映射 | ECS regression；`MountRejected(InvalidOutlet)` -> `PendingRejected` |
| preserved/current outlet corruption | crate-private seam 在 mount resolution 前破坏既有 anchor identity/owner/seal/Entity | ECS regression；`MountRejected(ParentAuthorityLost)` -> `AuthorityLost`，无 fallback |
| forged/stale identity | crate-private authority test | Scenario E / ECS regression |

## 5. Trace 断言

Journey A 至少要求以下有序子序列：

```text
FrameTaskIssued(parent, top_slot)
ShellPrepared(parent)
ViewApplied(parent)
OutletRegistered(parent, default)
MountRequested(child_outlet)
MountResolved(parent, default)
MountGranted(child_slot)
FrameTaskIssued(child, child_slot)
ShellPrepared(child)
ViewApplied(child)
FrameActivated(parent)
FrameActivated(child)
CommitPublished
RequestPhaseChanged(PostCommitCleanup)
CleanupSettled
RequestTerminal
```

Journey D 的 trace 另固定为：plan-level duplicate/foreign-session/cycle failure 不含任何 `MountRequested`/`MountRejected`；missing outlet 必须包含 `MountRequested -> MountRejected(MissingOutlet)`；crate-private entering corruption 必须包含 `MountRequested -> MountRejected(InvalidOutlet)`；preserved/current corruption 必须包含 `MountRequested -> MountRejected(ParentAuthorityLost)`，随后进入 authority settlement而不是 error fallback。`MountRejectedAfterMutation` 的 accessor 返回 `RejectedAfterMutation`，且只能出现在 grant staging 已开始的路径。

Journey B 至少要求 `FrameIsolated(old_leaf)` 早于 `FrameActivated(new_leaf)`，并在 `RequestTerminal` 前出现旧 leaf 的正常 cleanup receipt；保存的 old-leaf `AuditEntityId` 在稳定 audit 中必须为 `Removed`。Host loss/error cleanup 同样以 `RequestPhaseChanged`、`CleanupSettled`、`RequestTerminal` 的有序子序列证明中间安全态，不依赖它恰好跨越两个 `App::update`。Entity、input 和 focus 的逐步事实由只读 authority audit trace 证明。
