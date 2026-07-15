# Current Architecture

**文档类型**: 当前事实
**状态**: Root Push Tasks 1–9 已闭合，Task 10–12 待实现
**as_of**: Task 9 原子 authority cutover
**事实来源**: workspace manifest、公共导出、实现代码、测试代码与已记录验证

`Cargo.lock` 已跟踪，Bevy dependencies 固定到 exact revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`

## 1 当前定位

项目已完成第一条可由 facade 驱动的 Root push 主链

- app 安装 plugin、router、view driver 与 app-owned host
- facade 入队 `push` 并通过 snapshot 观察 request
- Core `RouterEngine` 负责匹配、request、frame identity、history intent 与逻辑 publication
- ECS 负责 host admission、Entity realization、exact manifest、原子 presentation 切换、cleanup ledger、input 与 focus 隔离
- View 保持 renderer-neutral，只提供 registry、descriptor 与 effect

当前成果是 Root authority 主链闭合，不是 Root PASS 或 MVP

Task 10 仍需补齐真实 cleanup terminality、fallback、diagnostics、重复 push 与公开旅程

## 2 Workspace 与 authority 边界

| Crate | 当前职责 | 不拥有的能力 |
| --- | --- | --- |
| `bevy_ui_router_core` | Bevy-free route tree、matcher、request、frame、history、逻辑 cleanup roots 与 sealed publication | World、Entity manifest、物理 receipt |
| `bevy_ui_router_view` | renderer-neutral registry、descriptor 与 effect | World mutation、lifecycle authority |
| `bevy_ui_router_ecs` | host、World、driver report session、exact Entity claim、realization、physical cleanup、input 与 focus | route matching authority、app-owned host ownership |
| `bevy_ui_router` | curated feature facade 与 Root-profile 公共入口 | Core engine、registry mutation authority |
| `bevy_ui_router_bsn` | 预留 adapter 边界 | 当前无实现 |
| `bevy_ui_router_platform` | router-neutral platform 类型薄封装 | route lifecycle authority |

生产链只有一个 publisher

`RouterNavigation` 私有持有 sealed Core integration `RouterEngine`，旧 `runtime`、`transaction`、ECS `state`、`finalize` 与 `bridge` 已删除

## 3 Root 运行链

`Update` 固定顺序

```text
ObserveAuthority
  -> CollectNavigation
  -> DriveNavigation
  -> CommitPresentation
  -> SettleCleanup
  -> ReapQuarantines
  -> PublishSnapshots
```

`PreUpdate` 固定顺序

```text
Refresh
  -> Ready
```

关键事实

- host admission 绑定 registration incarnation、candidate revision、host、top slot 与 frozen exact claim
- entering frame 在 relationship 写入前完成 registry-first inert allocation
- commit 前冻结 owner、seal、manifest、parent、visibility、picking、barrier、gate、focus 与 Core ticket
- atomic mutation 只执行预构造值写入，不查询、不分配、不做结构 mutation 或可失败调用
- Core cleanup 只验证 logical roots、batch identity、digest/count 与 completion digest
- ECS 独占 exact Entity obligations、attempts、ledger、receipt 与 quarantine
- pending、leaving、quarantine、unresolved residual 与 authority-invalid Active 均 fail-closed 隔离 input 和 focus
- app-owned host 与 foreign descendants 不会被 Router 修复、替换或销毁

## 4 公共边界

facade 已由 whole-crate alias 改为显式 curated export

Root profile 当前公开

- plugin 与 router installation
- driver registration
- app-owned host registration
- `push`
- request 与 router snapshots
- renderer-neutral content/report DTO

当前不公开

- `RouterEngine` 与 Core integration authority
- `RouteEntityRegistry` 与 ECS mutation authority
- legacy runtime/report bridge
- Nested route declaration 与后续 MVP API
- schedule labels 的 prelude promotion

正向 facade 测试证明 install → host → push → snapshot 可达，负向 compile-fail 测试证明旧 authority 与 deferred API 不可达

## 5 已证明与未完成

已证明

- Root first push 的真实 Core → View → ECS → facade 主链
- changed-root 原子切换与旧 frame settlement
- exact Entity manifest、claim reincarnation 与 stale rejection
- locked Bevy layout、effective visibility 与 native picking 基线
- 多 router gate 隔离、pending/leaving/quarantine/residual focus 隔离
- authority-invalid Active 的 Hidden、IGNORE、deny-all 与 focus fail-closed
- workspace feature/dependency/public boundary

未完成

- Task 10 cleanup terminality、fallback、diagnostics、重复 push 与 Root journey
- Task 11 完整 facade allowlist、golden 与 closed-world contract
- Task 12 Root completion gate
- Nested、Reconciliation、History、Recovery、Authority closure 与 MVP gate
- BSN adapter 与 neutral nested example

## 6 当前结论

Root authority 已从旧 runtime 原子切换到统一 `RouterEngine` 主链，Core/View/ECS/facade 分层与 exact ownership 边界已经落地

项目进入 Root PASS 前的后半段实现阶段，下一步从 Task 10 开始，不新增第二条 publisher 或兼容 authority 路径
