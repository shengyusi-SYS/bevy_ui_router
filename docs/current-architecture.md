# Current Architecture

**文档类型**: 当前事实

**状态**: Reconciliation 实施中，Tasks 1–5 complete

**as_of**: Reconciliation Task 5，`ddce26d`

**事实来源**: workspace manifest、实现、固定测试、公开旅程与独立复审

`Cargo.lock` 已跟踪，全部 Bevy git dependencies 固定到 exact revision `61f78f75684e6b0bc28a13a5933f8f55ca653fa7`

## 1 当前定位

项目已完成 facade 驱动的 Root 与首次 Nested push 垂直切片

- facade 安装 plugin、router、view driver 与 app-owned host
- `push` 经真实 `App::update` 完成匹配、构建、原子切换、cleanup 与 snapshot publication
- first、equivalent、fragment、changed-root、fallback success/failure 与重复 push 均有公开旅程
- `/items/1` 经真实 parent outlet、sealed child mount 与 root-to-leaf apply 提交两个 active frame
- Root 与 Nested profile 的 facade、opaque audit、feature graph、manifest 与 authority regression catalogue 已闭合

这是 Nested Push PASS，不是 Journey B、Reconciliation 或 MVP PASS

## 2 Authority 与 owner 边界

| Owner | 独占职责 | 明确不拥有 |
| --- | --- | --- |
| Core | route tree、matching、request、frame/history、逻辑 cleanup batch、sealed publication | World、Entity manifest、物理 receipt |
| View | renderer-neutral registry、descriptor、effect | World mutation、lifecycle authority |
| ECS | host admission、World、exact Entity claim、realization、presentation mutation、cleanup ledger、input/focus | route matching、app-owned host ownership |
| Facade | curated feature/API surface | engine、registry 与 mutation authority |

生产链只有一个 lifecycle publisher

`RouterNavigation` 私有持有 sealed Core `RouterEngine`，ECS report 只携带不可读取、不可构造的 opaque identity projection，最终 publication 仍由 Core sealed task 复验

## 3 Root 与 Nested 运行链

```text
PreUpdate: Refresh -> Ready

Update: ObserveAuthority
  -> CollectNavigation
  -> DriveNavigation
  -> CommitPresentation
  -> SettleCleanup
  -> ReapQuarantines
  -> PublishSnapshots
```

关键事实

- admission revision 绑定 registration incarnation 与 topology fingerprint
- Router-created Entity 先登记 exact ownership，再写 component/relationship，每次可能触发 hook/observer 的写入后复验
- pending、leaving、quarantine、unresolved residual 与 authority-invalid Active 均物理隔离 input 与 focus
- successful commit 的 mutation tail 已预构造且无可失败操作
- Core 只验证逻辑 roots、batch identity 与 completion digest
- ECS 独占 exact Entity obligations、attempts、ledger、receipt 与 quarantine
- app-owned host 与 foreign descendants 不被 Router 修改、替换或销毁
- seal、authority ID、host/frame incarnation 与 cleanup identity 不复用

## 4 已证明的 Root 与 Nested 能力

- 真实 Core → View → ECS → facade 主链
- repeated Root push、fragment remount、changed-root replacement 与 history publication
- match/build/apply failure、mandatory fallback 与 terminal diagnostics
- cleanup terminal gate、partial receipt、observer drift、quarantine transfer 与 duplicate/stale rejection
- stale report 在首次 World/registry 写入前按 sealed identity 拒绝
- app-owned host entity、组件、既有 children、relationship 与顺序保持不变
- locked Bevy layout、visibility、native picking 与 target event delivery
- parent outlet 登记后逐层 apply，完整 branch 一次 preflight 与一次 Core publication
- branch-changing 与 nested fragment-only navigation 在 Reconciliation 前安全替换整支 branch
- frame-local traversal、input/focus barrier、leaf-to-root exact cleanup 与 opaque outlet/content audit
- Authority catalogue 为 Root 20 + Nested 3 = 23

## 5 Nested completion evidence

- Contract origin：`1ccbcd7`
- Execution-plan commit：`eab1267`
- Implementation range：`eab1267..702e36e`，Task 8 为 `5e8869c`，evidence fix 为 `702e36e`
- Focused gates：Core 301、View 15、ECS 273、journeys 5、audit 2、allowlist 46、compile-fail 40
- Authority gate：catalogue 23、registered 23、passed 23
- Workspace gate：nextest 659/659、0 skipped，doctest 28，fmt/check/diff green

## 6 Reconciliation 实施 checkpoint

Root 与 Nested PASS 事实保持不变，Reconciliation 当前已完成 Tasks 1–5

- Core 已完成 slot-aware projection、preserved suffix splice 与 sealed logical publication
- ECS 已完成 role-sensitive physical preflight、原子 splice、exact cleanup 与最小 pre-commit authority settlement
- authority loss 复用既有唯一 publisher、ledger 与 receipt stream，清空失效的 Core/ECS active，不 fallback、不 replay、不推进 history
- Task 6 private physical reconciliation regressions、Task 7 public Journey B/fragment facade evidence 与 Task 8 Reconciliation completion gate 尚未完成

## 7 后续边界

尚未完成

- Reconciliation 的 private physical regressions、public Journey B/fragment evidence 与 completion gate
- replace/back History
- host-loss 与 stale replay Recovery
- background reaper、完整 audit 与多 router 压测的 Authority closure
- 最终 MVP gate
- MVP 之后的 BSN adapter 与嵌套示例

后续切片继续复用当前唯一 authority、exact claim、registry-first、post-write validation 与 app-owned host zero-write 约束
