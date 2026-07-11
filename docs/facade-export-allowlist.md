# Facade Export Allowlist

**文档类型**: Phase 3B.1 公共导出规范附录
**状态**: Phase 3B.1 `Plan PASS`
**上位规范**: [Target Architecture](target-architecture.md)

## 1. 规则

Facade 不再整 crate re-export。每个 public module path 必须出现在第 2 节，每个 public type/value/trait symbol 必须出现在第 3 节；`Prelude = yes` 才能从 `bevy_ui_router::prelude` 导入。新增公共项必须先修改本文件，并同时增加 compile-pass/compile-fail。源码名称扫描不能替代 consumer 编译测试。

## 2. Allowed Modules

| Facade path | Required feature | Contract |
| --- | --- | --- |
| `bevy_ui_router::core` | default | curated core exports only |
| `bevy_ui_router::prelude` | default | rows marked `Prelude = yes`, filtered by enabled features |
| `bevy_ui_router::view` | `view` | curated view exports only |
| `bevy_ui_router::ecs` | `ecs` | curated ECS exports only |
| `bevy_ui_router::bsn` | `bsn` | Phase 3B.1 empty module |
| `bevy_ui_router::platform` | `platform` | router-neutral adapter namespace only |
| `bevy_ui_router::platform::prelude` | `platform` | preserved back types path |

`view`、`ecs`、`bsn` 和 `platform` 均须在 feature 关闭时不可达。Facade 不公开 `core::runtime`、底层 crate 的整 crate namespace 或额外的 nested prelude。三个 back types 本来就是 always-available core 类型，并继续从 facade root prelude 导出；`platform` feature 只增加指向同一类型的兼容路径 `platform::prelude`，关闭 platform 时只有该次级路径不可达。

## 3. Allowed Type/Value/Trait Symbols

| Symbol | Module path | Prelude | Required feature |
| --- | --- | --- | --- |
| `ActivePresentation` | `core` | yes | default |
| `ActivePresentationKind` | `core` | yes | default |
| `BackCapabilitySnapshot` | `core` | yes | default |
| `BackIntent` | `core` | yes | default |
| `BackPolicy` | `core` | yes | default |
| `BranchDiff` | `core` | yes | default |
| `BranchDiffRoute` | `core` | yes | default |
| `CleanupDisposition` | `core` | yes | default |
| `CleanupSummary` | `core` | yes | default |
| `DiagnosticEvent` | `core` | no | default |
| `DiagnosticEventKind` | `core` | no | default |
| `DiagnosticSeverity` | `core` | yes | default |
| `DiagnosticSnapshot` | `core` | yes | default |
| `ErrorBoundaryKey` | `core` | yes | default |
| `FrameOrigin` | `core` | yes | default |
| `HistoryError` | `core` | yes | default |
| `HistorySnapshot` | `core` | yes | default |
| `HostSnapshot` | `core` | yes | default |
| `HostTraceEvent` | `core` | yes | default |
| `HostTraceEventKind` | `core` | yes | default |
| `HostVersion` | `core` | yes | default |
| `MatchedBranch` | `core` | yes | default |
| `MatchedRoute` | `core` | yes | default |
| `MemoryHistory` | `core` | yes | default |
| `NavigationPhase` | `core` | yes | default |
| `NavigationFailure` | `core` | yes | default |
| `NavigationFailureKind` | `core` | yes | default |
| `NavigationRequestId` | `core` | yes | default |
| `NavigationTraceEvent` | `core` | yes | default |
| `NavigationTraceEventKind` | `core` | yes | default |
| `MountRejectionKind` | `core` | yes | default |
| `ReportDisposition` | `core` | yes | default |
| `RequestStatus` | `core` | yes | default |
| `RouteBranchFingerprint` | `core` | yes | default |
| `RouteError` | `core` | yes | default |
| `RouteErrorKind` | `core` | yes | default |
| `RouteErrorSourceCategory` | `core` | yes | default |
| `RouteExplicitKey` | `core` | yes | default |
| `RouteFrameId` | `core` | yes | default |
| `RouteFrameStatus` | `core` | yes | default |
| `RouteFrameVersion` | `core` | yes | default |
| `RouteGeneration` | `core` | yes | default |
| `RouteHostId` | `core` | yes | default |
| `RouteId` | `core` | yes | default |
| `RouteLocation` | `core` | yes | default |
| `RouteMatcher` | `core` | yes | default |
| `RouteMeta` | `core` | yes | default |
| `RouteNode` | `core` | no | default |
| `RouteNodeOptions` | `core` | yes | default |
| `RouteParams` | `core` | yes | default |
| `RouteParamsFingerprint` | `core` | no | default |
| `RoutePathSegment` | `core` | yes | default |
| `RoutePattern` | `core` | yes | default |
| `RoutePatternKind` | `core` | yes | default |
| `RouteSlotId` | `core` | yes | default |
| `RouteSlotKey` | `core` | yes | default |
| `RouteSearchFingerprint` | `core` | no | default |
| `RouteSegmentReuseKey` | `core` | no | default |
| `RouteTree` | `core` | yes | default |
| `RouteTreeBuilder` | `core` | yes | default |
| `RouteViewInput` | `core` | yes | default |
| `RouteViewKey` | `core` | yes | default |
| `RouteViewKeyError` | `core` | yes | default |
| `RouterAvailability` | `core` | yes | default |
| `RouterUnavailableReason` | `core` | yes | default |
| `RouterInstanceId` | `core` | yes | default |
| `RouteViewDescriptor` | `view` | yes | `view` |
| `RouteViewRegistry` | `view` | yes | `view` |
| `RouteViewRegistryError` | `view` | yes | `view` |
| `ViewBuildEffect` | `view` | yes | `view` |
| `ViewDriverKey` | `view` | yes | `view` |
| `ViewDriverKeyError` | `view` | yes | `view` |
| `ViewRequestId` | `view` | yes | `view` |
| `AppRouterExt` | `ecs` | yes | `ecs` |
| `BevyUiRouterPlugin` | `ecs` | yes | `ecs` |
| `ContentColor` | `ecs` | yes | `ecs` |
| `ContentEntity` | `ecs` | yes | `ecs` |
| `ContentPrimitiveError` | `ecs` | yes | `ecs` |
| `ContentPrimitiveErrorKind` | `ecs` | yes | `ecs` |
| `ContentPrimitiveKind` | `ecs` | yes | `ecs` |
| `ContentWriteError` | `ecs` | yes | `ecs` |
| `ContentWriteErrorKind` | `ecs` | yes | `ecs` |
| `EcsViewDriver` | `ecs` | yes | `ecs` |
| `EcsViewReportSession` | `ecs` | yes | `ecs` |
| `FrameContentWriter` | `ecs` | yes | `ecs` |
| `NavigationEnqueueError` | `ecs` | yes | `ecs` |
| `NavigationEnqueueErrorKind` | `ecs` | yes | `ecs` |
| `NavigationRequestSnapshot` | `ecs` | yes | `ecs` |
| `RequestLookup` | `ecs` | yes | `ecs` |
| `RouterConfig` | `ecs` | yes | `ecs` |
| `RouterHandle` | `ecs` | yes | `ecs` |
| `RouterHostRegistration` | `ecs` | yes | `ecs` |
| `RouterInputGate` | `ecs` | yes | `ecs` |
| `RouterInputSet` | `ecs` | no | `ecs` |
| `RouterInstallError` | `ecs` | yes | `ecs` |
| `RouterInstallErrorKind` | `ecs` | yes | `ecs` |
| `RouterNavigation` | `ecs` | yes | `ecs` |
| `RouterScheduleSet` | `ecs` | no | `ecs` |
| `RouterSnapshot` | `ecs` | yes | `ecs` |
| `RouterSnapshots` | `ecs` | yes | `ecs` |
| `SolidRect` | `ecs` | yes | `ecs` |
| `ViewBuildReport` | `ecs` | yes | `ecs` |
| `ViewBuildReportKind` | `ecs` | yes | `ecs` |
| `ViewDriverRegistrationError` | `ecs` | yes | `ecs` |
| `ViewDriverRegistrationErrorKind` | `ecs` | yes | `ecs` |
| `AuthorityContentMapping` | `ecs` | yes | `ecs-test-support` |
| `AuthorityFrameMapping` | `ecs` | yes | `ecs-test-support` |
| `AuthorityFrameLifecycle` | `ecs` | yes | `ecs-test-support` |
| `AuthorityHostMapping` | `ecs` | yes | `ecs-test-support` |
| `AuthorityOutletMapping` | `ecs` | yes | `ecs-test-support` |
| `AuthoritySlotKind` | `ecs` | yes | `ecs-test-support` |
| `AuthoritySlotMapping` | `ecs` | yes | `ecs-test-support` |
| `AuditEntityId` | `ecs` | yes | `ecs-test-support` |
| `AuditEntityState` | `ecs` | yes | `ecs-test-support` |
| `InputProbeResult` | `ecs` | yes | `ecs-test-support` |
| `QuarantineLeaseId` | `ecs` | yes | `ecs-test-support` |
| `QuarantineOwnerKind` | `ecs` | yes | `ecs-test-support` |
| `QuarantineOwnerSnapshot` | `ecs` | yes | `ecs-test-support` |
| `QuarantineSnapshot` | `ecs` | yes | `ecs-test-support` |
| `RouterAuthorityAuditSnapshot` | `ecs` | yes | `ecs-test-support` |
| `RouterAuthorityAudits` | `ecs` | yes | `ecs-test-support` |
| `RouterLifecycleAuditEvent` | `ecs` | yes | `ecs-test-support` |
| `RouterLifecycleAuditEventKind` | `ecs` | yes | `ecs-test-support` |
| `RouterInputModality` | `ecs` | yes | `ecs-test-support` |
| `RouterInputProbes` | `ecs` | yes | `ecs-test-support` |
| `BackCapabilitySnapshot` | `platform::prelude` | no | `platform` |
| `BackIntent` | `platform::prelude` | no | `platform` |
| `BackPolicy` | `platform::prelude` | no | `platform` |

`bsn` 在 Phase 3B.1 只保证空 module 可编译；`devtools` 不增加 symbol。BSN adapter 阶段通过新的审查变更本表。

`EcsViewReportSession` 是唯一允许 consumer 命名并消费、但不能构造/clone/读取 raw identity 的一次性 driver session。禁止的是 report acceptance、replay、commit capability及 session constructor，不是该参数类型本身。

### 3.1 Closed-World Associated API

第 3 节的**每一个** allowed type/trait 都采用 closed-world associated-item 合同，不只限于 authority ID。精确 machine-readable golden 已在设计阶段提交，固定为：

```text
docs/facade-associated-api-golden.txt
```

每行规范化记录 `feature | facade path | owner crate | item kind | public signature/field/variant/trait impl`。Target 新类型与 authority-relevant 类型逐项等于 Facade Contract Fixture；保留的普通 core/view API已经从基准提交提取并纳入同一golden，不得留给Root push临场决定。任何 public field、enum variant、trait method、inherent method、associated const/type、constructor、conversion、derive/explicit trait impl不在 golden 都失败。实现测试直接读取这份已审查文档，不另建可漂移副本。

Golden 还必须计算**公开签名类型图闭包**：每个 allowed item 的 public field、variant payload、参数、返回值、associated type bound 和显式 trait impl所引用的项目内类型，都必须在同一或更弱 feature 下拥有 allowed canonical path；否则源 item 本身不允许导出。标准库、Bevy公开类型和泛型参数可作为外部叶节点，但不能借 `Deref/From/AsRef` 等转换重新获得被禁止的项目内 authority 类型。`BranchDiff` 因此同时导出 `BranchDiffRoute`；旧 `FakeViewRenderer*` 因签名依赖已禁止的旧 build/apply graph而从 facade移除。下表是额外的不可放宽能力约束，不是完整覆盖范围：

| Type | Allowed inherent API | Allowed public traits |
| --- | --- | --- |
| `FrameContentWriter<'_>` | `spawn_empty`, `spawn_solid_rect`, `add_child`, `despawn`, `declare_outlet` | none |
| `ContentEntity` | none | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `ContentColor` | `srgba8`, `red`, `green`, `blue`, `alpha` | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `SolidRect` | `new`, `width_px`, `height_px`, `color` | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `BevyUiRouterPlugin` | none | `Default, bevy_app::Plugin` only |
| `EcsViewReportSession<'_>` | `applied`, `build_failed`, `apply_failed` | none |
| `RouterHandle` | none | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `NavigationRequestId` | none | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `AuditEntityId` | none | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `QuarantineLeaseId` | none | `Clone, Copy, Debug, Eq, PartialEq, Hash` |
| `RouterInputGate` | `allows_pointer`, `allows_keyboard`, `allows_gamepad` | Bevy `Resource` + mandatory `Component` supertrait only |
| `RouterInputProbes` | `dispatch`, `delivery_count` | Bevy `Resource` + mandatory `Component` supertrait only |
| `RouterAuthorityAudits` | `get` | Bevy `Resource` + mandatory `Component` supertrait only |
| `ViewBuildReport` | `kind`, `message` | `Debug` only；只能由 report session 生成 |
| `AppRouterExt` | `register_ui_view_driver`, `add_ui_router` | trait 本身，无其他 authority method |
| `RouterNavigation` | `push`, `replace`, `back` | Bevy `Resource` + mandatory `Component` supertrait only |
| `RouterSnapshots` | `get`, `request` | Bevy `Resource` + mandatory `Component` supertrait only |
| all audit DTOs | only Facade Contract Fixture accessors | snapshot traits only；无 raw Entity/World/registry conversion |

这些类型一律禁止 `Deref/DerefMut/AsRef/Borrow/From/Into` 到 raw identity、World、Entity、registry 或 authority type。`FrameContentWriter` 尤其没有 generic `spawn/insert(Bundle)`、`world(_mut)`、`commands`、callback/effect/observer registration API；`SolidRect` 只能由 ECS 解释为规范固定的内建 component closure。

## 4. Forbidden Symbols

以下名称在 facade root、对应 module 和 prelude 均必须不可达，或其 authority constructor/method 必须私有：

| Symbol | Former/likely module | Negative assertion |
| --- | --- | --- |
| `RouterEngine` | `core` | name unreachable |
| `RouterRuntime` | `core` | name unreachable |
| `NavigationTransaction` | `core` | name unreachable |
| `NavigationTransactionSnapshot` | `core` | name unreachable |
| `FrameApplyToken` | `core` | name unreachable |
| `FrameApplyRequest` | `core` | name unreachable |
| `FrameApplyOutcome` | `core` | name unreachable |
| `FrameReconciliationPlan` | `core` | name unreachable |
| `FrameCleanupPlan` | `core` | name unreachable |
| `MountRequestToken` | `core` | name unreachable |
| `MountGrant` | `core` | name unreachable |
| `CommitTicket` | `core` | name unreachable |
| `DiagnosticIdentity` | `core` | name unreachable |
| `RouteFrameBranch` | `core` | name unreachable |
| `RouteFrameIdentity` | `core` | name unreachable |
| `RouteFrameRecord` | `core` | name unreachable |
| `RouteFrameScope` | `core` | name unreachable |
| `CleanupScope` | `core` | name unreachable |
| `OwnedRegion` | `core` | name unreachable |
| `NavigationTransactionState` | `core` | name unreachable |
| `StaleApplyReason` | `core` | name unreachable |
| `StaleApplyResult` | `core` | name unreachable |
| `TransactionCommitError` | `core` | name unreachable |
| `TransactionCommitErrorKind` | `core` | name unreachable |
| `TransactionFailure` | `core` | name unreachable |
| `FakeViewRenderer` | `view` | name unreachable from facade; owner-crate test helper only |
| `FakeViewRendererMode` | `view` | name unreachable from facade; owner-crate test helper only |
| `ViewBindingKey` | `view` | name unreachable |
| `ViewReportSession` | `view` | name unreachable |
| `ViewRenderPlan` | `view` | name unreachable |
| `ViewApplyOutcome` | `view` | name unreachable |
| `ViewBuildRequest` | `view` | name unreachable |
| `ViewBuildOutcome` | `view` | name unreachable |
| `ViewDisposalHint` | `view` | name unreachable |
| `ViewFailure` | `view` | name unreachable |
| `ViewOutputId` | `view` | name unreachable |
| `ViewPartialOutput` | `view` | name unreachable |
| `ViewReportedOutput` | `view` | name unreachable |
| `ViewCommitAuthority` | `view` | name unreachable |
| `ViewFinalizeAuthority` | `view` | name unreachable |
| `RouterEcsState` | `ecs` | name unreachable |
| `EcsFrameApplySession` | `ecs` | name unreachable |
| `EcsFrameApplyReport` | `ecs` | name unreachable |
| `EcsFrameApplyStatus` | `ecs` | name unreachable |
| `BeginFrameApplySessionError` | `ecs` | name unreachable |
| `FinishFrameApplySessionError` | `ecs` | name unreachable |
| `CompleteFrameApplySessionError` | `ecs` | name unreachable |
| `RouteHost` | `ecs` | name unreachable |
| `RouteSlot` | `ecs` | name unreachable |
| `RouteFrameRoot` | `ecs` | name unreachable |
| `RouteFocusSnapshot` | `ecs` | name unreachable |
| `RouteFrameInteractionSnapshot` | `ecs` | name unreachable |
| `RouteFrameRootLifecycleSnapshot` | `ecs` | name unreachable |
| `CleanupCapability` | `ecs` | name unreachable |
| `CleanupBatchToken` | `ecs` | name unreachable |
| `CleanupBatchCompletion` | `ecs` | name unreachable |
| `CleanupObligation` | `ecs` | name unreachable |
| `CleanupReceipt` | `ecs` | name unreachable |
| `EntityMountReceipt` | `ecs` | name unreachable |
| `HostAdmissionToken` | `ecs` | name unreachable |
| `HostGrant` | `ecs` | name unreachable |
| `HostLease` | `ecs` | name unreachable |
| `HostSettlementTicket` | `ecs` | name unreachable |
| `PostCommitActivation` | `ecs` | name unreachable |
| `RouteEntityRegistry` | `ecs` | name unreachable |
| `EntityOwner` | `ecs` | name unreachable |
| `QuarantineOwner` | `ecs` | name unreachable |
| `QuarantineLease` | `ecs` | name unreachable |
| `EcsViewReportSession::new` | `ecs` | constructor private |
| `FrameContentWriter::spawn` | `ecs` | generic Bundle method missing |
| `FrameContentWriter::insert` | `ecs` | generic Bundle method missing |
| `FrameContentWriter::world_mut` | `ecs` | method missing |
| `FrameContentWriter::commands` | `ecs` | method missing |
| `ContentEntity::{raw,entity,from_raw}` | `ecs` | methods missing |
| `AuditEntityId::{raw,entity,from_raw}` | `ecs` | methods missing |
| `QuarantineLeaseId::{raw,from_raw}` | `ecs` | methods missing |
| `RouterHandle::{raw,from_raw}` | `ecs` | methods missing |
| `NavigationRequestId::{raw,from_raw}` | `core` | methods missing |
| `ContentEntity/AuditEntityId -> Entity` | `ecs` | `From/Into` conversion missing |
| `EcsViewReportSession: Clone` | `ecs` | trait bound fails |
| `ViewBuildReport::{new,success,failure}` | `ecs` | constructors missing |
| `ViewBuildReport: Clone/Default` | `ecs` | trait bounds fail |
| `AppRouterExt::{set_active,register_authority}` | `ecs` | trait methods missing |
| `RouterNavigation::{commit,set_active,apply_report}` | `ecs` | methods missing |
| `RouterAuthorityAuditSnapshot::{world,entity,registry}` | `ecs` | raw accessors missing |

以下旧 module path 也必须不可达；Facade 的 `core/view/ecs` 都是 curated wrapper，不是 underlying crate alias：

| Module path | Negative assertion |
| --- | --- |
| `core::{back,branch,branch_diff,diagnostics,error,fingerprint,frame,history,ids,location,matcher,meta,ownership,pattern,route_tree,runtime,slot,test_support,transaction}` | every nested module unreachable |
| `view::{prelude,test_support}` | nested module unreachable |
| `ecs::prelude` | nested module unreachable |

本表覆盖基准提交当前 facade/prelude 的全部“现有但目标不允许”导出，不只覆盖典型 authority 类型。后续实现新增的任何额外 public item 由第 5.1 节的 source-structure completeness gate 拒绝。

## 5. Consumer Test Matrix

Facade `Cargo.toml` 必须声明：

| Test target | Path | `required-features` |
| --- | --- | --- |
| `default_facade_features` | `tests/default_facade_features.rs` | `[]`；以真实 default features运行 |
| `public_navigation_journeys` | `tests/public_navigation_journeys.rs` | `["ecs"]` |
| `public_navigation_authority_audit` | `tests/public_navigation_authority_audit.rs` | `["test-support"]` |
| `public_navigation_input_isolation` | `tests/public_navigation_input_isolation.rs` | `["test-support"]` |
| `ecs_facade_features` | `tests/ecs_facade_features.rs` | `["ecs"]` |
| `bsn_facade_features` | `tests/bsn_facade_features.rs` | `["bsn"]` |
| `test_support_features` | `tests/test_support_features.rs` | `["test-support"]` |
| `facade_export_allowlist` | `tests/facade_export_allowlist.rs` | `[]`；内部按 cfg 分组 |
| `facade_authority_compile_fail` | `tests/facade_authority_compile_fail.rs` | `[]`；生成 feature-specific consumer |

Facade integration tests必须在facade package的`[dev-dependencies]`直接声明`bevy_app`、`bevy_ecs`、`bevy_ui`，version/git/exact rev/default-feature与ECS crate完全相同。ECS crate-private物理UI/picking regression必须在`bevy_ui_router_ecs [dev-dependencies]`直接声明同revision的`bevy_internal`，`default-features = false, features = ["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"]`，并使用其单个`DefaultPlugins`装配；不得同时手动添加`UiPickingPlugin`。ECS authority catalogue使用的`syn`也属于ECS direct dev-dependency；facade completeness使用的`syn`属于facade direct dev-dependency。Workspace`Cargo.lock`必须tracked；主workspace gates一律`--locked`。

Harness 解析 facade manifest并断言 feature key/edge 与 Target 完全相等；旧 `view-test-support` key 必须不存在，`FakeViewRenderer*` 只能由直接依赖 owner crate 的内部测试使用，不能通过 facade feature或path恢复。

`facade_export_allowlist` 对上表每个 allowed row 做 module path compile-pass，并按 `Prelude` 列验证 prelude。`BevyUiRouterPlugin::default()` 必须在独立consumer中先由 `App::add_plugins` 显式安装，再调用两个 `AppRouterExt` 方法，证明install marker复用且schedule/resources只注册一次；自动安装路径另独立覆盖。`facade_authority_compile_fail` 对每个 forbidden row生成最小 consumer。0 tests 或只做源码名称扫描不算通过。

### 5.1 固定 consumer harness

两个 test target 共用 `tests/support/facade_contract_cases.rs` 中的单一 case catalogue。运行时只在以下固定位置生成隔离 crate；不得写入源码目录：

```text
target/facade-contract-cases/
  pass/<feature-set>/<case>/Cargo.toml
  pass/<feature-set>/<case>/Cargo.lock
  pass/<feature-set>/<case>/src/lib.rs
  fail/<feature-set>/<case>/Cargo.toml
  fail/<feature-set>/<case>/Cargo.lock
  fail/<feature-set>/<case>/src/lib.rs
  cargo-target/
```

每个生成的 `Cargo.toml` 必须包含独立 `[workspace]`，并以 canonical absolute path 依赖当前 facade：

```toml
[package]
name = "facade-contract-<stable-case-slug>"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
bevy_ui_router = { path = "<current-facade-crate>", default-features = <case-default>, features = [<exact-case-features>] }
```

Case slug、source、default flag 和 feature list 必须由 catalogue 的稳定排序生成。`default` row（表示 always-available core）用 `default-features = false` 测一次；另有完整 `default-enabled` consumer 用 `default-features = true, features = []`，实际 import/use全部 view rows，并断言 manifest `default` 精确等于 `["view"]`。其他 row 只启用 Required feature，依靠 manifest 中声明的 implication。Module rows、每个 allowed symbol 的 module path、每个 `Prelude = yes` symbol 的 prelude path都各有正例；正例必须在 Rust item 中实际引用该 path，不能只有未使用 import。

Harness 使用 TOML parser直接读取 workspace facade `Cargo.toml`，断言 default精确为 `['view']` 及完整feature key/edge；不能从隔离consumer的 `cargo metadata --no-deps` 推断path dependency metadata。`default-enabled` case以 `cargo tree --locked --offline -e normal` 验证resolved graph必须包含 facade/core/view，不得包含 `bevy_app`、`bevy_ecs`、`bevy_ui`、`bevy_picking` 等 Bevy runtime crates。另一个 no-default case证明core-only；explicit `view` case与default-enabled导出集合相同。

同一 test target 使用 `syn` 递归解析 facade `src/lib.rs` 与其声明的本地 module files，并把所有 public modules、public item definitions、`pub use` 叶节点及其 `cfg(feature)` 归一化后与 catalogue 做双向相等比较。Facade wrapper 禁止 public glob re-export、underlying crate alias、`pub extern crate` 和生成 public API 的 macro；任何源码 public item 不在 catalogue、catalogue row 没有源码对应项、feature/prelude 不匹配都直接失败。所有allowed public struct还要审计unit/tuple/named shape：除golden头明确列出的`BevyUiRouterPlugin`外，public unit struct或零字段named struct直接失败；零public-field struct必须实际含至少一个private field。Harness为这些struct生成直接value constructor与空struct-literal负例，成功构造即失败。该 source-structure gate 用于证明“没有额外导出”和“opaque type不可伪造”，隔离 consumer 正例/负例继续证明 path 的真实可编译性，两者不能互相替代。

对第 3 节全部 allowed types/traits，harness 递归解析 owner crate source，比较 public fields/variants、public trait methods、inherent associated items、显式 trait impl与 derive traits，并与 committed golden 中的源码可枚举记录做双向相等比较；再递归展开每个签名引用的项目内类型，证明 allowed type graph 已闭包且没有 forbidden leaf。任何无法解析的签名类型、条件编译分支或类型闭包边都必须使生成与测试直接失败；禁止输出或接受“未解析哨兵”来表示成功。生成 public associated API 的 macro 禁止使用。

`compile-assert` 记录不参与 `syn` 源码枚举比较，而由隔离 consumer 中的泛型断言逐项编译证明；MVP 固定断言错误类型满足 `Send + Sync + 'static`。自动 trait或 blanket impl不能靠 owner source全局枚举，因此只验证 golden 明确要求和禁止的编译性质，不把它们伪装成显式 `trait-impl`。Golden 中每个可调用 method/constructor有 compile-pass 引用；已知 capability deny patterns（raw/Entity/World/registry、forging constructor、session clone、report constructor、额外 navigation/app-extension method）各有隔离 compile-fail。任意未预见的新 public associated item即使没有可猜测的负例名称，也会先被 source-vs-golden gate 拒绝。由其他 crate 增加的 trait impl受 orphan rules 限制；owner source audit与 consumer conversion negatives共同防止关联 capability 泄漏。

Harness 还读取本文第 2 至 4 节受控表格，按 heading 与 pipe-column schema 归一化 module/allowed/forbidden rows并与 catalogue 比较；表格 row 遗漏、重复或 drift 直接失败。这里不是用源码名称扫描替代编译，而是让规范、catalogue、facade source 和 consumer compiler 四者同时一致。

每个非 default module/symbol 另有 feature-disabled 负例：只启用其严格较弱的前置 feature，例如 audit row 只启用 `ecs`，不得依赖 Cargo `required-features` 跳过。每个 forbidden row 也生成负例；private constructor/method 单独生成调用负例。

每个 isolated case 先串行执行并审计独立 lock：

```powershell
cargo generate-lockfile --offline --config "build.rustc-wrapper = ''" --manifest-path <case>/Cargo.toml
cargo tree --locked --offline --config "build.rustc-wrapper = ''" -e normal --manifest-path <case>/Cargo.toml
cargo check --locked --offline --color never --config "build.rustc-wrapper = ''" --manifest-path <case>/Cargo.toml --target-dir target/facade-contract-cases/cargo-target
```

Harness 解析每个 generated lock，所有 Bevy git packages 必须是 exact source revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`；出现 branch 的其他 cached revision 立即失败。全部正例必须 exit 0。负例必须 exit nonzero，且 stderr 命中该 case catalogue 指定的真实 rustc 类别：unresolved path `E0432|E0433`、private item/method/field/constructor `E0603|E0624|E0616|E0451|E0423|E0063`、missing method `E0599`，或 missing trait/conversion `E0277`。每个 case 只接受 catalogue 为该意图声明的类别，不能用任意编译错误冒充；stderr 同时不得包含 dependency download、manifest parse、workspace membership、package resolution 或 linker failure。先运行每个 feature set 的正例预热依赖，再运行负例，确保基础设施失败不会伪装成 compile-fail。测试报告必须给出总 case 数，0 case、遗漏表格 row 或重复 case slug 都直接失败。
