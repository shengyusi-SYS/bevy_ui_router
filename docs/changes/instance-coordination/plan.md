# 实例协调实施计划

状态：第 1–4 步的实现和公开回归已完成；workspace 全特性、Core/ECS 完整 crate、无默认特性与独立 consumer 通过。第 5 步的父/fresh 子 Back、当前输入与既有 entry 滚动组合、示例、现行文档、代表性能和现行树打包/产物 consumer 已闭合；剩余窗口验收纳入最终扩展树收口。与[设计](design.md)配套，根代理统一 index/提交与 Cargo 验证，独立子代理交叉审查。沿当前 main 推进，不新建分支。

**目标：** 先交付当前已发布输入，再将 leaf 保留替换为按实例和 exact 父依赖的协调，闭合混合激活/fresh、失败及依赖淘汰。

**架构：** Core 选择目标来源并准备输入/库存决策；ECS 按实例自身持有原 owner/payload，沿现有 preparation、publication 和 cleanup 消费。View 中立、BSN 只执行 fresh Scene；不增加第二套缓存 runtime。

**技术：** 现有 Rust/Bevy workspace、同步 Scene/BSN、标准容器。约束和验收归[设计](design.md)，检查入口归[验证指南](../../development/verification.md)，整体投入归[路线图](../../roadmap.md#工作量风险与-roi)。下列内部模块列出目标接缝，未完成接口的名字为设计名，未冒充现有源码。

## 交付顺序与模块接缝

`1 当前输入 → 2 实例/payload 等价迁移 → 3 混合来源与父保留 → 4 内部库存/依赖淘汰 → 5 组合收口`。步骤 3/4 是一个行为发布单元；3 是开发审查点，4 闭合后统一提交。剩余预算统一归[路线图](../../roadmap.md#工作量风险与-roi)。各提交保持可运行，结构迁移和行为扩展分开，不能中间交付“保父但无条件退休内部子库存”的临时语义。

| 模块 | 输入 → 输出 | 唯一职责 |
| --- | --- | --- |
| Core `view_input.rs` | 匹配/恢复事实 → published input value/ref | 构建快照与当前输入的共同字段语义，不持有 Entity |
| ECS `active_scope.rs`、`route_inputs.rs` | 当前 registry/hierarchy → exact active frame/input | 只读归属解析与公共 SystemParam |
| Core `engine/instances.rs`、`engine/retained.rs` | frozen current/inventory + matched target → source selection/retention partition | 前者持有实例事实与依赖排序，后者编排选择、LRU 与原 capability；不持有物理 payload |
| ECS `instance.rs` | 原 frame + 其 own slots → 唯一实例 payload | 分拆/组合物理持有，不决定匹配或保留政策 |
| ECS `branch_transaction.rs` 与 `navigation/normal_publication.rs` | 逐实例来源、mount、原 owners → prepared transaction | 混合准备、写集/owner 组合及统一 publication 接收 |

保留现有 integration 作为 Core/ECS 受控门面；新的内部模块不直接公开 capability 构造器。普通查询 DTO 不重建 owner。每次涉及 lifecycle/failure 分类的交付，除局部回归外运行完整受影响 crate 与已闭合 facade/consumer gate。

## 1. 当前已发布输入（4–7 人日）

文件：修改 `crates/bevy_ui_router_core/src/{view_input.rs,engine.rs,integration.rs,lib.rs}`、`crates/bevy_ui_router_ecs/src/{activity.rs,lib.rs}`、`crates/bevy_ui_router/src/lib.rs`；新建 `crates/bevy_ui_router_ecs/src/{active_scope.rs,route_inputs.rs}`。测试扩展 `crates/bevy_ui_router/tests/{public_activity.rs,public_navigation_journeys.rs,consumers/ecs.rs,consumers/bsn.rs}` 和 `crates/bevy_ui_router_view/tests/public_api.rs`。

输入：当前 matched/error/fallback publication 的确切事实及 live registry/hierarchy。产出：设计中的 `RouteInputs::current(Entity) -> Option<RouteInputRef<'_>>` 和五个只读 getters；Core prepared publication 的逐 frame 输入集合，生命周期与 frame 保持一致。

- [x] 先在公开 fixture 增加父/子 route-local params、preserved 父新 location、旧 factory 快照和 mutable Query 并用断言；初始执行应因 API 缺失失败，确认验证的是真实公开入口。
- [x] 在 `view_input.rs` 分离可持有的 published value 与借用 ref，保留 `RouteViewInput` 的单次构建快照合同。Core 为正常、preserved、error/fallback 各 frame 准备确切值；公开旅程同时断言 normal 前缀的 failure 为 None、错误 frame 为原 failure，不从成功 history 反推错误输入。
- [x] 从 `activity.rs` 提取同证明目的的 exact active-frame resolver，让 RouteActivity 与 RouteInputs 共用；补最近 inactive 边界、动态后代、同 Entity 实例替换及其它 Router 的真实事实回归。
- [x] 将输入接入现有 publication preparation/tail，验证 pending/stale 保持旧值、发布后 terminal 前读新值及 no-op 零物理写入。输入存储提前准备，tail 不插组件、不运行可失败查询。
- [x] 导出 ECS facade/prelude，View public API 验证中立借用形状，独立 consumer 仅依赖 facade 使用查询。更新当前导航/输入合同与接入指南后提交 `feat: expose current published route inputs`。

局部运行 Core/View 完整测试和 facade `public_activity`、`public_navigation_journeys`、`facade_consumers`；该步 publication 变更还须执行下文完整交付门禁。

时序与零写证据扩展 ECS `src/navigation_tests.rs`：真实 prepare/commit/settle 阶段验证进入内容无输入、旧 active 保持旧值、publication 后 `PostCommitCleanup` 终结前读新值；既有 no-op mutation trace 和 stale Back report/receipt/publication 回归同时核对已发布输入。公开普通 `App::update()` 可能在同轮完成发布和清理，不以更新后的样本冒充内部阶段观察。View API 测试是借用形状的编译证据；无 BSN consumer 仅验证接线与 host 拒绝，成功导航消费由独立 BSN consumer 和公开旅程证明。

## 2. 实例事实与物理 payload 等价迁移

文件：新建 `crates/bevy_ui_router_core/src/engine/instances.rs`、`crates/bevy_ui_router_ecs/src/instance.rs`；修改 Core `src/{frame.rs,engine.rs,integration.rs,engine/retained.rs}` 与 ECS `src/{lib.rs,retained.rs,branch_transaction.rs,frame_local.rs,navigation/normal_publication.rs,cleanup.rs}`。测试沿用 `crates/bevy_ui_router_core/src/integration_retained_tests.rs`、ECS `src/navigation/native_tests.rs`、facade `tests/retained_navigation.rs`。

输入：现行 leaf 实例、原 owner、RealizedBranch。产出：以原 `RouteFrameVersion` + `HostVersion` 识别的库存事实、exact 父依赖，以及每份只含一个 RealizedFrame/own slots 的原 payload；行为仍为已交付 leaf 范围。

- [x] 保留 leaf 命中、fragment 精确匹配、LRU、Back、拒绝返还、host loss/readmission 的现有回归，补齐原实例激活后再次离场的身份/LRU 回归。覆盖 Core `retained_hit_matches`、`retained_plan_partition_matches`、`owner_projections_match` 和 ECS `retained_payload_is_current`，不直接注入分类结果。
- [x] 将 Core 实例事实归并为原 `RouteFrameVersion` + `HostVersion` 与 exact 父依赖，删除 retained 身份发号。owner 集合按原身份对比，库存仍按成功离场的 LRU 顺序；冻结 source/seal 和旧计划失效语义不变。
- [x] 引入 frame-local payload，迁移唯一 owner 的持有位置。库存依赖只存身份，外部父 slot 仅作 mount alias；保持 existing source projection、seal 和原 owner 的线性转移。
- [x] 将 current target steps 与 fresh creation obligations 分开；leaf 激活不再进入 fresh `applied_frames`/failure roots。现阶段同一协议选择单个 leaf，保证拒绝原 owner 返回与 cleanup 写集不变。
- [x] 迁移后删除被替代的 retained 身份发号、库存专属 `RealizedBranch` 容器/单帧方法及消费端的 branch 形状假设；保留必要投影/证据，不因重命名丢失错误来源。active/fresh/failed 的分支交易集合、single-hit 政策和 Fresh/Retained 发布分类留至步骤 3。通过门禁后以 `refactor: unify retained instance ownership` 独立提交。

审查重点是等价性和 owner 迁移，不在这一步顺带放宽 params/search、增加父保留或更改失败 continuation。

## 3. 混合来源与父实例保留

文件：Core `src/{engine/instances.rs,frame.rs,engine.rs,integration.rs}`；ECS `src/{instance.rs,branch_transaction.rs,presentation.rs,frame_local.rs,navigation.rs,navigation/normal_publication.rs,cleanup.rs}`。测试扩展 Core `src/integration_retained_tests.rs`、ECS `src/navigation/native_tests.rs`、facade `tests/retained_navigation.rs`、`tests/public_navigation_journeys.rs`。

输入：步骤 2 的完整库存与原 payload、matcher 目标、步骤 1 的输入 preparation。产出：来源为 `Current* → Retained* → Fresh*` 的逐位置 target selection、显式父来源、fresh obligations，以及可一次消费的 prepared splice/owner 接收器。

- [x] 先增加 `R/P/C → R/Q → R/P/D` 的 P(true)/C(false) 公开旅程；断言 P 原 Entity/局部状态/factory 次数、C 消失、D fresh、当前输入更新、P 在 D 准备期间仍 inactive。保留带 fragment 的祖先输入轨迹。
- [x] Core 选择独立目标来源，替换 `retarget_retained_leaf`、single-entering 门槛与 `from_preserved_parent: bool` 推断。fresh 父后只允许 fresh 子，slot 选择先检查选中父原 slot，缺失才发原 grant/stage/confirm。
- [x] ECS staging/mount predicate 接受本请求精确选中的 Retained/Hidden 父；使用原 slot 只借用，新 slot 暂归 preparation。触发写入后复验 target/protected，验证按 own payload 与 child boundary 分开，P 不加入 fresh branch 假冒创建内容。
- [x] 迁移 `classify_failed_boundary`、`exact_current_task_outlet` 和 `FailureCleanupSourceProjection`，冻结原 selection/owner 的库存边界来源；区分库存 OwnInvalid、fresh drift 和共享 authority loss。补 fresh 子尚未分配输出时的失败归属，不能从 active/prepared 子集推断选中 P 的事实。
- [x] 将 publication 的全局 Fresh/Retained 分支替换为逐 frame/slot 的 current/next；提前组合原 owners、claim rebind、输入/focus/scroll、cleanup 和接收器，atomic tail 沿原要求执行。
- [x] 增加 fresh D 构建拒绝、preflight 拒绝、带库存前缀 route-error/fallback、准备中 host loss 的生产事实回归。验证原 P owner 返还/移交一次，原 slot 和其它子库存 protected，新 slot/fresh D 仅由本次失败清理，history continuation 未被缓存路径改写。
- [x] 完成局部测试和只读交叉审查后进入步骤 4，尚不单独提交或宣布父保留交付；保留 inactive 父的合法子库存与依赖结算必须一同闭合。

这一步的控制流和 failure receiver 必须能表达多实例来源，不能先叠加一个 P 专用临时状态机再在下一步推翻。

## 4. 内部子库存、容量与依赖清理

文件：Core `src/{engine/instances.rs,engine/retained.rs,engine.rs,integration.rs}`；ECS `src/{retained.rs,instance.rs,presentation.rs,registry.rs,frame_local.rs,cleanup.rs,navigation/retained_miss.rs,navigation/normal_publication.rs}`；测试沿用步骤 3 的三个层级。

输入：步骤 3 的逐实例选择和 publication/失败接收器。产出：active/inactive 父均可存活的 exact 库存、确定的实例 LRU、依赖 cleanup forest 和现有 authority settlement 的完整 owner 集合。

- [x] 增加 P/C 均保留及多个内部子切换回归，覆盖尚未被选中时的库存 preservation，再证明 matcher 可返回 P/D 而不恢复旧 C；最深 leaf fragment 条件仍成立。多个合法候选按最近成功离场和稳定身份选择，重复 frame/owner 登记有界拒绝。
- [x] 替换 `scope_survives` 和 authority assembly 中“父必须 next active”的条件，迁移 `presentation.rs` 的 shared-authority predicate 与 `registry.rs` 的持续 preservation proof，从同一冻结库存核对 exact 父依赖。activation/preservation 各按自身角色接受有效 Retained 父，不全局扩张 staging 许可；inactive 子不占 active slot，但保留完整 exact mount。
- [x] 按设计计费与 child-first 离场顺序实现容量 0/1、内部子独立淘汰、父淘汰依赖闭包；失败不刷新 LRU。使用标准容器，不引入整分支 blob 或第二个淘汰队列。
- [x] ECS 消费原 payload/owners 构建 child-first cleanup forest，校验无重复 claim。库存 OwnInvalid 使用原证据/owner/任务 continuation 局部结算依赖，清理成功且 authority 有效后从失效位置起 fresh，合法当前 Q 保持 protected；失败或 authority 变化沿既有分流有界终结。
- [x] 验证发布前 host loss 不提交 history，post-commit authority 保留已提交结果及原 normal token/ledger，仅接收仍 live 的实例；idle 不重开旧请求。stopped/quarantine settlement 仍一次收敛，不将父 slot 算入子销毁写集。
- [x] 删除不再被生产使用的 single-hit/single-leaf 选择、旧 retained parent-only-active 分支及重复校验；必要 identity、post-write drift、protected closure 回归先迁移。完整门禁与审查后统一提交 `feat: coordinate parent and child instance inventory`，同步已落地合同和架构。

## 5. 高层组合与文档归位（4–7 人日中的本专题部分）

文件：facade `tests/{retained_navigation.rs,public_navigation_journeys.rs,public_activity.rs,scroll_restoration.rs,facade_consumers.rs,consumers/bsn.rs}`；文档 `docs/{overview.md,roadmap.md,contracts/navigation.md,contracts/lifecycle.md,contracts/input-focus.md,architecture/README.md,architecture/runtime.md,architecture/lifecycle.md,guides/integration.md,development/README.md,development/verification.md}`；按实际接缝更新 `crates/bevy_ui_router/examples/bsn_nested.rs`。

- [x] 在同一个公开旅程组合 Back、原 P 激活/fresh D、当前输入与既有 entry 滚动；用真实 BSN 内容和动态后代观察活动资格，复用已闭合 pointer/focus gate，确认旧访问意图不因相同 Entity 复活而串用。
- [x] 用独立 facade consumer 接入 RouteInputs 和父/子保留；示例演示布局状态存续与新 location，保持业务中立。相同输入/no-op、params/search/fragment 和 fallback/readmission 回归全部保留。
- [x] 对选定代表规模复测切换/命中/依赖淘汰，两轮完整串行 flat/nested 优化结果归[性能基线](../../performance.md#父子库存实测)。不含 layout/render，尚未作严格 cleanup 内部归因；真实优化按证据另估。
- [ ] 最终扩展树组合/窗口验收；实现树完整门禁及现行打包/产物 consumer 已通过，源码和边界归[验证指南](../../development/verification.md)。不以历史窗口或临时探针冒充父子新路径。
- [x] 已按落地行为更新合同、当前架构图、指南、示例和验证入口；路线图仅留未完成工作。必要语义已迁入所属合同，最终窗口收口后本设计/计划退出默认树，由 Git 追溯。

统一滚动意图和通用 UI 输入 hook 的跨专题最终组合，只有各自功能落地后才执行，不用本步骤预先声称完成。集中验收预算 4–7 人日还包含那部分，不在各专题重复加算。

## 验证与审查门禁

开发时先运行新增行为所在测试，确认它因缺少所需行为失败，再执行对应改动。局部定位命令：

```bash
cargo test -p bevy_ui_router_core --all-features --locked
cargo test -p bevy_ui_router_view --all-features --locked
cargo test -p bevy_ui_router_ecs --all-features --locked
cargo test -p bevy_ui_router --features bsn --test retained_navigation --test public_navigation_journeys --test public_activity --test scroll_restoration --test facade_consumers --locked
```

publication/lifecycle/failure 交付提交前，由唯一 Cargo 负责人运行以下完整门禁；相同最终树已通过且没有新变更，不机械重复：

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo test -p bevy_ui_router_ecs --no-default-features --locked
```

每次交付由独立代理只读复核 Core 选择/continuation 与 ECS 原 owner/写集/publication，另一视角核对公开行为。实现 shared publication 的修改错开；审查一致不能替代成功命令输出。根代理检查 exact staged 文件和 diff 后提交 main，不纳入职责外变更。

原生输入公开 hook 与访问级滚动恢复不塞入本计划；滚动统一意图已归[导航合同](../../contracts/navigation.md#按访问恢复滚动)，输入实施依赖正式接缝。第 1–4 步已实现；本专题收口按第 5 步继续，输入/滚动扩展不因既有组合回归通过而完成。
