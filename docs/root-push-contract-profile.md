# Root Push 阶段合同 Profile

**状态**：Root push `Plan PASS`

**上位目标**：[Facade Export Allowlist](facade-export-allowlist.md) 与 [Facade Associated API Golden](facade-associated-api-golden.txt)

## 1. 作用

本文只定义 Root push `Task PASS` 时必须已经可达且可兑现的公共能力，不改变最终 MVP 的 facade allowlist、associated API golden 或验收旅程。后续切片只能从最终合同中逐步增加能力，不能引入最终合同之外的临时 API；MVP gate 必须以完整 allowlist/golden 取代本 profile。

维护归属固定为：本文独占 Root阶段 exclusions 与 authority regression catalogue；[MVP Acceptance Journeys](mvp-acceptance-journeys.md) 独占最终MVP集合；[Root Push Implementation Plan](superpowers/plans/2026-07-11-root-push-implementation-plan.md) 只拥有执行顺序、RED/GREEN命令与提交边界。

## 2. 已兑现能力

- default/view 保留最终 allowlist 中的 Bevy-free route、matcher、只读 DTO、view key/registry/effect 数据能力；旧 runtime、transaction、apply/commit authority 不可达。
- ecs 提供 `BevyUiRouterPlugin`、`AppRouterExt`、`RouterConfig`、`RouterHandle`、`RouterHostRegistration`、`RouterNavigation::push`、`RouterSnapshots`、单次 immediate driver/report、`FrameContentWriter` 的 `spawn_empty`/`spawn_solid_rect`/`add_child`/`despawn`、`SolidRect`、只读 `RouterInputGate` 与 schedule/input set。`push`可重复调用；Root阶段对单根route按最终语义完成等价zero-effect、fragment generation推进或整根替换，不暴露只能在Nested阶段构造child route的builder方法。
- ecs-test-support 只提供 Root push 可由真实 registry 证明的 host、top slot、单 root frame、production primitive、lifecycle 与 owned-state 映射。结构 Node、component manifest 和 app ancestor 只由 ECS 私有 World 测试证明。
- `platform` 保持现有 router-neutral back 数据 re-export；`bsn` 是空 module；`devtools` 不增加 symbol。

## 3. 延后能力

以下最终合同项在对应切片前不得从 facade 到达：

| 能力 | 首次允许切片 |
| --- | --- |
| `FrameContentWriter::declare_outlet`、outlet/child-slot audit DTO、mount trace/rejection 的运行时产生 | Nested push |
| nested/multi-frame common-prefix preservation 与 leaf-only replacement | Reconciliation |
| `RouterNavigation::replace`、`RouterNavigation::back` 及可执行 history preview | History |
| route error-boundary、host/authority recovery 的完整行为与相关 audit 状态 | Recovery |
| `RouterInputProbes`、完整 quarantine/reaper audit、跨 router authority 压力观察面 | Authority closure |

数据 enum 中为最终 MVP 冻结但尚未由 Root push 产生的 variant 可以存在于 owner crate；只有当其所属 public type 已列入本 profile 且全部 associated items 与最终 golden 完全一致时，才可经 facade 导出。不得为阶段实现增加 `Unsupported`、`NotImplemented` 或其他临时 public error/variant。

`MountRejectionKind`、`NavigationTraceEventKind` 的 mount variants及对应只读accessor属于已冻结的最终只读vocabulary，可在Root profile可见但Root运行时永不产生；真正延后的是outlet mapping、`declare_outlet`公共写入能力、child-slot authority和mount trace事件的产生。

### 3.1 Machine-Readable Exclusions

Root push gate 先读取最终 allowlist/golden，再按下表做精确排除；表外项目不得由实现自行增删。

| Kind | Canonical path or normalized item | First slice |
| --- | --- | --- |
| symbol | `bevy_ui_router::ecs::AuthorityOutletMapping` | Nested push |
| symbol | `bevy_ui_router::ecs::InputProbeResult` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineLeaseId` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerKind` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineOwnerSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::QuarantineSnapshot` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputModality` | Authority closure |
| symbol | `bevy_ui_router::ecs::RouterInputProbes` | Authority closure |
| associated | `bevy_ui_router::ecs::FrameContentWriter::declare_outlet` | Nested push |
| associated | `bevy_ui_router::ecs::RouterNavigation::replace` | History |
| associated | `bevy_ui_router::ecs::RouterNavigation::back` | History |
| associated | `bevy_ui_router::core::RouteTreeBuilder::route_with_options` | Nested push |
| associated | `bevy_ui_router::core::RouteTreeBuilder::route` | Nested push |
| associated | `bevy_ui_router::core::RouteNodeOptions::with_error_boundary` | Recovery |
| associated | `bevy_ui_router::core::RouteNodeOptions::with_error_view` | Recovery |
| associated | `bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::outlets` | Nested push |
| associated | `bevy_ui_router::ecs::RouterAuthorityAuditSnapshot::quarantines` | Authority closure |

The profile parser requires exactly these columns and rejects duplicate or unknown rows. An `associated` row removes the named method plus its golden signature record only; all other associated items on that type remain closed-world equal to the final golden. A `symbol` row removes the type and every golden record whose canonical path is that type. Final MVP validation applies no exclusions.

### 3.2 Root Authority Regression Catalogue

| Test function |
| --- |
| `cleanup_observer_reentry_supersedes_attempt` |
| `cleanup_receipt_replay_is_rejected` |
| `commit_gate_switch_is_same_frame_atomic` |
| `exact_owned_cleanup_cycle_is_world_zero_and_retains_authority` |
| `foreign_component_quarantine_avoids_hooks` |
| `foreign_descendant_quarantines_owned_ancestor` |
| `host_children_change_only_top_slot_membership` |
| `host_grant_observer_drift_rejects_after_mutation` |
| `host_ui_topology_requires_all_node_ancestry` |
| `pending_cycle_rejects_without_touching_active` |
| `persistent_admission_observer_latches_retry` |
| `plan_relationship_errors_are_world_zero` |
| `post_commit_cleanup_drift_supersedes_attempt` |
| `post_commit_cleanup_waits_for_receipt` |
| `protected_host_is_never_despawned` |
| `protected_parent_quarantine_preserves_relationship` |
| `solid_rect_required_components_match_locked_bevy` |
| `solid_rect_runs_real_layout_and_picking_pipeline` |
| `stale_report_identity_is_zero_write` |
| `structural_outputs_have_locked_manifest_and_topology` |

The catalogue parser accepts exactly one column and rejects zero, duplicate, or non-final-MVP names. Later profiles add only tests owned by their slice; MVP gate must equal the complete catalogue in `mvp-acceptance-journeys.md`.

## 4. Root Push 门禁

- `tests/public_navigation_journeys.rs` 在本阶段至少包含单 root push case；它不代表完整 Journey A-D/F-G 已通过。
- `tests/facade_export_allowlist.rs` 与 `tests/facade_authority_compile_fail.rs` 使用本 profile 过滤最终 allowlist/golden：已启用类型采用完整 associated-item 闭包，延后项必须 compile-fail。
- `scripts/verify-authority-regressions.ps1` 使用 Root push catalogue，比对 catalogue、测试二进制实际注册集合和实际通过数；后续切片只扩展 catalogue。
- Root push 只可标记 `Task PASS`。完整九目标 consumer matrix、完整 associated API golden、全部 MVP journeys 与 `MVP PASS` 留在 MVP gate。
