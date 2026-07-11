# Roadmap

**状态**: Phase 3B.1 `Plan PASS`
**更新时间**: 2026-07-11

## Now：Phase 3B.1 垂直切片

每个切片都必须从 facade 命令经过真实 `App::update` 到稳定 snapshot，不能先堆完横向协议再接主路径。

1. **Root push**：先把全部 Bevy dependencies 固定到 exact rev `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`、提交 workspace `Cargo.lock`，同步 facade/ECS manifests、ECS `test-support`、`bevy_ui/bevy_picking` 正常 edge、无 `ghost_nodes` 的全Node物理UI ancestry、exact-feature `bevy_internal::DefaultPlugins`物理回归、direct dev-dependencies、default/no-default/explicit feature图与隔离consumer locks；建立可逐切片扩展的 facade/profile、associated-signature-type-graph、authority regression catalogue和固定test targets，Root push只要求本阶段 profile 全闭包，完整 allowlist/golden 与九目标矩阵在 MVP gate 收口；再实现 canonical `RouteLocation`、RouterEngine、精确 `RouterScheduleSet`/`RouterInputSet` chain、capability-closed facade、可显式预装且幂等的plugin、带candidate-revision失败闩锁的UI-capable host admission、安装时 normal-view/error-view/fallback/driver校验、mandatory fallback、data-only `ContentPlan` writer与生产 `SolidRect` primitive、递归required-component manifest、host/top-slot staging + post-write validation、按router分区的唯一registry、最小只读audit Resource、从Entity出生起登记的 `EntityOwner`/seal/component manifest/quarantine基础、inbound-parent/cycle classification、完整identity/preflight、sealed top-slot/frame lifecycle、immutable obligation + versioned cleanup attempt、可重复单根push的整根替换与push history、单frame inert shell、gate + prebuilt Pickable原子提交、phase/terminal trace与结构化diagnostics。内部Node拓扑/manifest只由ECS私有World回归证明，不扩展facade DTO。
   详细阶段边界见 [Root Push Contract Profile](root-push-contract-profile.md)，执行顺序与命令见 [Root Push Implementation Plan](superpowers/plans/2026-07-11-root-push-implementation-plan.md)。Roadmap 只维护切片顺序，不复制 profile exclusions、catalogue 或任务步骤。
2. **Nested push**：在第一片闭环上增加真实 parent outlet、`MountRequested/Resolved/Granted`、opaque outlet audit mapping、逐层 apply 与 frame-local traversal。
3. **Reconciliation**：二次 leaf 替换、common-prefix 保留、离场输入/focus 隔离与正常 cleanup receipt。
4. **History**：replace、back 执行期预览/失效终态与成功后提交，补齐 zero-entering；push 已在 Root push 闭环。
5. **Recovery**：view/mount failure、error presentation、host loss 与 stale replay。
6. **Authority closure**：在既有 registry/ownership/preflight/quarantine 上增加多 router 压测、host/slot/frame/outlet/tombstone 的 opaque `AuditEntityId` 只读 audit、独立 lease authority、protected-parent/foreign-component/removal-DAG/detached-output quarantine、公平逐 Entity 后台 reaper、真实 locked-UI-picker/input probe和 cleanup replay/攻击覆盖，不再补录基础 ownership。
7. **MVP gate**：A-D、F-G facade journeys，Scenario E authority regression，feature matrix、workspace tests 和 factual review。

## Next

1. 基于 MVP 已通过审查的 `SolidRect` 与统一 effect/report 实现 immediate BSN adapter；不得恢复 generic Bundle/World callback。
2. 若最终示例需要文本、图片或其他 primitive，先按 variant 单独完成数据模型、锁定 Bevy事实与安全审查，再扩展 allowlist。
3. 增加真实 BSN binding 的最终嵌套路由示例。
4. BSN 旅程通过后重新评估 Phase 3C。

## Paused

- 原 Phase 3B Task 6 与 Phase 3C。
- queued/streaming BSN、file scene、hot reload。
- parallel named outlet、retained cache、transition、loader/action/revalidation。

暂停项不能成为 Phase 3B.1 的前置假设。
