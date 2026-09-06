# Bevy UI Router MVP Implementation Plan

> **For agentic workers:** 执行时使用 `subagent-driven-development` 或 `executing-plans`，按下述交付结果推进与审查。用户已授权统一规划、合理调度子代理和安排提交；不为每个小修复重复征求流程选择，不机械套用 TDD 或另立 spec。

**Goal:** 完成可从稳定 facade 使用的官方同步 BSN Router MVP，保留完整基础/嵌套路由、history、恢复和 owned-only/input/focus 语义。

**Architecture:** 复用 renderer-neutral Core/View；BSN 提供 one-shot Scene factory 与 outlet 声明，ECS optional bridge 独占 World、staging、publication 和 cleanup。复用 shell/content-root 两层结构，Scene 内容采用 current containment，不延续旧 data-only component manifest。

**Tech Stack:** Rust 2024、官方 crates.io Bevy `0.19.1`、Bevy ECS/UI/picking/input-focus/Scene、Cargo workspace。

**Spec:** [统一 MVP design spec](../specs/2026-09-07-router-mvp-design.md)。实现者同时读取 spec 和本计划；产品合同归 spec，本文件只拥有迁移顺序与证据。

**状态**：Active implementation plan。两位独立审查者及交叉复核均确认可提交合同并开始实施；阶段 A 的设计收口已完成，测试合同迁移尚未完成。spec 第 12 节已选择并验证声明图预登记、私有 shell wrapper 与 attempt allocation obligations 的最小可行机制。当前候选实现和临时探针不代表 MVP PASS。

## 全局约束

- 官方 crates.io Bevy `0.19.1`，无 Bevy fork、patch 或 source replacement；Core/View 和 facade 默认功能不依赖 Bevy。
- 旧 data-only renderer API 无兼容义务；不取消 renderer-neutral 边界、最小独立测试替身或既有导航行为。
- 每个 layout 一个 default outlet，可位于内容根或内部容器；不实现 multiple/named/dynamic outlets。
- Router 只管理自身结构、Scene 创建期声明图分配义务及成功 adoption 后的当前 containment，不销毁或重建 app-owned host，不回滚任意应用副作用。
- publication-last、exact identity/stale 检查、线性 owner、失败清理和输入/focus 隔离持续有效。
- 无 binding 的 nested Push 失败保持 cleanup 后 RouterFallback；不能为迁移恢复旧 Terminal 分类。
- 不把 public audit/probe、background reaper/fairness、固定 catalogue 或 hostile World 沙箱作为 MVP 前置。
- 全程一个 index/提交负责人、一个 Cargo/rustc 负责人；子代理默认只读审查，授权实现时明确文件所有权。

## 1. 基线与实施方式

本轮起点：`sys/structural-governance`，`0799098`。选择在当前候选上前向迁移，`1712be4` 仅作为六个候选提交之前的比较点；不整体 reset/revert，不重新执行已完成的 Recovery 开发。

进入 runtime 阶段前，用 `git status --short`、`git branch --show-current`、`git worktree list` 复核实时环境，遵循 worktree skill 的已有隔离检查。用户变更不能纳入或回退。出现新并行实现需求时再选择独立 worktree；不为纯文档审查复制工作区。

当前 Core/View 可以作为迁移锚点。ECS 主路径仍使用 `EcsViewDriver -> FrameContentWriter/ContentPlan -> RealizedFrame`；BSN crate 尚无实现且反向依赖 ECS。输入 gate 的名单来自构建时输出，不能直接覆盖动态 native Scene。六个候选含可删产品外扩张，也含必须保留的前台安全修复。

### 1.1 候选行为 retention map

| 候选提交 | 迁移决定 | 不能丢失的结果 / 验证落点 |
| --- | --- | --- |
| `c8a9672` | 移除 background reaper、fairness 与公开调度前置 | 保留前台 cleanup/host 仍使用的 private residual owner，直到有等价替代；terminal 与残留隔离分开 |
| `9c33bfb` | 移除 public audit DTO、投影与专属 consumer | 导航/host snapshots 仍能表达请求结果；私有必要 owner 检查不因 DTO 删除而消失 |
| `433f344` | 移除 `input_probe.rs` 和 probe 接线 | 保留 production input gate，测试迁往真实输入路径和内部回归 |
| `03d8f46` | 移除 public probe/test-support 和中间调度依赖 | 保留真实 UI/picking 正向命中、非活动拒绝、keyboard/gamepad gate 和 focus 释放 |
| `df5c7b2` | 逐块保留实际安全修复，删除只为 probe/audit 服务的同步 | 不按 `input.rs` 整文件回退；受保护 subtree 与 focus 不被误伤 |
| `0799098` | 保留关系漂移和 ownership-cycle 修复结果，允许改写私有机制 | cleanup 写入后 drift 停止破坏性操作；永久矛盾有界收敛；原 owner 不丢失；旧 epoch 不发布 |

此表不要求保留全部旧测试形状。删除旧断言前，为同一用户结果或不变量指出替代测试；只验证已延期机制的测试可以随机制删除。

### 1.2 关键文件与职责

| 文件/模块 | 本次职责 |
| --- | --- |
| `crates/bevy_ui_router_core/src/{engine,branch,request,view_input,history}.rs` 及其现有模块 | 保留决策、history/fragment/search/recovery；除新证据所需修复外不做整体重写 |
| `crates/bevy_ui_router_view/src/` | 保留 key/descriptor/只读 contract；清除旧公开 report/writer 依赖时维持独立测试 |
| `crates/bevy_ui_router_bsn/{Cargo.toml,src/lib.rs}`；可拆出 `src/factory.rs`、`src/outlet.rs` | factory、`BsnBuildError`、`DefaultOutlet`，无 ECS 依赖 |
| `crates/bevy_ui_router_ecs/src/bsn.rs`（新增） | optional App extension/注册包装、官方 Scene 执行与错误映射 |
| `crates/bevy_ui_router_ecs/src/{app_ext,driver,content,presentation,frame_local,mount}.rs` | store 分派、Scene-writable root、frame-local containment、outlet binding；旧 writer 退出 |
| `crates/bevy_ui_router_ecs/src/{components,registry,cleanup,input,plugin,branch_transaction,lifecycle_preparation,navigation}.rs` | owned structure、隔离、publication、前台 cleanup/residual 与稳定调度 |
| `crates/bevy_ui_router_ecs/src/{audit,input_probe,test_support}.rs` 与 `registry/quarantine_reaper.rs` | 移除外扩 surface；只保留有真实前台用途或私有测试用途的最小部分 |
| `crates/bevy_ui_router/{Cargo.toml,src/lib.rs,tests/}` | facade feature/导出、独立 consumer、public journeys、删除旧 golden/probe 依赖 |
| `README.md`（新增）、`examples` 对应 crate 内示例、`docs/{README,current-architecture,roadmap}.md`、`AGENTS.md` | 推荐装配、事实/路线、轻量护栏；单一规范来源 |

文件拆分以实际职责和可审查性为准；以上新增私有文件名可在阶段开始时调整并同步本表，不因此另开架构需求。

## 2. 阶段 A：统一合同和可执行验证基线

**交付结果**：一份完整 spec、一份本计划；旧文档不再定义新增 MVP 范围。现有行为验证仍可运行，不把“删规范”变成“删合同”。

**涉及文件**：本 spec/plan、`AGENTS.md`、`docs/README.md`、`docs/roadmap.md`、`docs/current-architecture.md`；旧 Target/Matrix/Fixture/Allowlist/Golden/Journeys/Goals；`crates/bevy_ui_router/tests/support/facade_contract_cases.rs` 及相关 facade/dependency tests。

- [x] 完成两份独立 spec/plan 审查，交换原报告自由交叉评价；逐条核对代码与用户裁决。matcher/Back 文案与 pointer 撤权合同已修正；Scene 失败产物问题作为下项前置保留，不以审查数量或一致意见替代技术判断。
- [x] 完成 spec 第 12 节的窄可行性验证：选择官方 resolve 后的直接 ChildOf 图预分配/登记、私有 shell wrapper 和独立 attempt obligations，不预挂 staging 树。探针覆盖匿名首 child、深层部分失败、根模板前向引用失败、root/child 双向引用、用户 resolve 整体替换、正常 Children 顺序与空 Children 差异。cached Scene inclusion 在 UI 分配前拒绝；机制结论已落实到 spec/阶段 C。
- [x] 迁移基础路由、参数/search/fragment、single outlet 位置、独立 renderer 验证、错误恢复、history、host/input/focus 语义；收口复审及交叉复核无剩余合同阻断项，spec 标为 active，旧协议目录在索引中标为迁移/历史材料。
- [x] 重写 AGENTS 为长期护栏，保留身份、线性 owner、publication-last、局部 revalidation、owned-only、真实验证和共享工作区纪律；更新索引、current facts、roadmap 和历史定位。协议步骤、测试目录和已延期产品机制归 spec/历史，不按机械行数缩减。
- [ ] 移除以多份 Markdown 表互相证明 API/能力的 gate；改用能编译的正向 facade consumer、少量真实权限边界 compile-fail、生产行为测试。尚未迁移的旧消费者保留到其替代落地，明确它是临时验证依赖而非兼容义务。
- [ ] 复核并保留已修 Recovery profile 的 CRLF 回归，不重复安排旧修复；迁移仍需使用的文本/consumer harness 时以 `lines()` 或明确换行规范化消除残余 LF 假设，保留同份输入的 LF/CRLF 对照。使用 Rust `Path`/`Command` 构造临时 consumer，避免平台 shell 拼接。
- [x] 将依赖检查从精确私有 manifest/feature 列表改为语义约束：默认 facade 无 Bevy；Core/View 的 default/all-features 正常与构建依赖图均保持 renderer-neutral；官方 release crates 全部按包名选择并核对 `0.19.1` 与 crates.io。workspace/现有 consumer 共用 lock 检查；manifest 与实际 Cargo 配置检查 Bevy fork、patch、replacement 和路径覆盖，不限制普通依赖的合法配置。BSN→ECS 的现存错误方向仍在阶段 C 反转，Cargo 本身拒绝依赖环；这里不以依赖 gate 通过宣称 BSN 接线完成。
- [ ] 检查 canonical docs 的有效语义全部迁入且 parser 引用已消除后，再删除/归档被替代文件。已完成 History/Recovery 记录保留为历史证据，不以旧 catalogue 数量约束新设计。

**验证**：相关 facade/dependency/text tests；`cargo test --workspace --all-features` 作为迁移前基线，原样记录失败和环境限制。无 Rust 改动的合同提交仅要求 diff、引用和一致性检查；不能把文档审查写成 runtime PASS。

**提交边界**：统一 spec/护栏/索引为合同提交；本 plan 单独提交。测试合同迁移和历史材料删除另成可审查提交，不与 runtime 行为混提。必要决定全部迁入并提交后删除临时 `2026-09-07-mvp-reevaluation.md`，其证据可从 Git/任务历史追溯。

## 3. 阶段 B：精简候选，保持可用的独立基线

**交付结果**：不依赖 public audit/probe/reaper 的导航和输入基础；Core/View 与真实 ECS 路径仍可独立测试。

**涉及文件**：retention map 的 ECS 文件、`plugin.rs`、facade exports/features、`public_navigation_{journeys,authority_audit,input_isolation}.rs`、`test_support_features.rs`。

- [ ] 按 retention map 移除公开 audit/probe、background reaper 调度、fairness 和仅服务它们的测试。先列出 private residual owner 的前台 callers，保留其实际义务，不能整块删除 `QuarantineLeaseTable` 来掩盖 outstanding ownership。
- [ ] 仅公开 `RouterInputSet::{Refresh,Ready}` 和 `RouterScheduleSet::PublishSnapshots`。旧 public journey 中必须在内部暂停/注入的故障回归迁回 ECS 私有 tests；外部 consumer 只执行普通 `App::update` 和稳定入口。
- [ ] 保留最小 fake renderer 驱动生产 Core/ECS 逻辑，先明确 fixture 控制的真实 build/apply 事实；不能注入 classification、terminal 或 publication 结论。旧 data-only executor 可暂为迁移工具，阶段 C/D 替代后退出，测试独立性本身长期保留。
- [ ] 重跑 root/nested/reconciliation/history/recovery 的已有 public 行为与相应 ECS crate。若最高层回归失败，沿生产路径确定 cause；不得通过恢复旧 Terminal、删目标断言或增 retry 次数过关。

**验证入口**：`cargo test -p bevy_ui_router_core`、`cargo test -p bevy_ui_router_view`、`cargo test -p bevy_ui_router_ecs --all-features`、`cargo test -p bevy_ui_router --features ecs --test public_navigation_journeys`。重命名/迁移测试目标时同步命令，不固定原文件数量。

**提交边界**：先删除外扩 surface/调度与对应依赖，再提交必要私有回归迁移；每个提交保持可编译可审查，不批量 revert 六个 candidate。

## 4. 阶段 C：官方 BSN root 垂直切片

**交付结果**：独立应用从 facade 注册 owned Scene，在 app-owned host 上成功展示 root；真实 build/resolve/apply 失败清理 pending，不发布 partial UI；旧 active/input/focus 和 history 符合 spec。

**接口**：实现 spec 第 3 节的 `AppBsnExt::register_bsn_view`、`BsnSceneFactory`、`BsnBuildError` 和 `DefaultOutlet`。ECS store 按同一 `ViewDriverKey` 命名空间选择 native 或内部 fake executor，completion 仍由 ECS 消费真实执行结果后推进 Core。

**涉及文件**：BSN crate、新 `ecs/src/bsn.rs`、ECS/ facade Cargo 与 exports；`app_ext.rs`、`presentation.rs`、`frame_local.rs`、`input.rs`、`cleanup.rs`、`mount.rs`、`navigation.rs` 及对应 tests。

- [ ] 先反转 BSN 的错误依赖，再添加 ECS optional `bsn` bridge 和 facade feature 联动。以真实 `Box<dyn Scene>` 单次消费编译，测试 factory 每个 effect 恰好调用一次、重复 key 拒绝与 normal/error/fallback 安装校验。
- [ ] 在含 BSN binding 的 Router 安装入口检查 `AssetServer`、`Assets<ScenePatch>`；在调用前再次检查 runtime 资源。缺失分别产生 `MissingSceneRuntime` 安装错误或有来源的运行 failure，不调用缺失资源会 panic 的官方入口。
- [ ] 将现有 shell/content-target 改为 Router shell + Scene root，删除 authored 内容的封闭 component/构建输出清单假设。Router 结构 identity/seal 继续 exact 校验。按 spec 第 12 节使用官方 ScenePatch load/resolve，保活 dependency handles，预检直接 ChildOf 图及 cached/reference/capacity 边界，先登记 exact allocation obligations 再写组件；匿名引用使用本 attempt 唯一身份，不能复制探针固定编号。
- [ ] 将只含引用 seed 与原 resolved root 的私有 wrapper 应用到本次新建 inert shell；原 root 的引用仍指向 content target，wrapper 自身不携带它们。原模板和关系由官方 executor 按顺序执行，诊断去除最外层私有 wrapper 错误。返回同一 exclusive 调用内完成 adoption 或失败隔离，覆盖尚未挂接的 root/children；验证成功后才交接到 current containment，不延后到普通调度。
- [ ] 将 Scene content traversal、input/focus scope、cleanup scope 和 mount validation 联动迁移。不能仅新增 BSN wrapper 后把 Scene 偷渡回 cloneable `ContentPlan`，也不能让一个 scope 采用 current containment 而另一个仍使用永久扁平名单。
- [ ] 以 shell `Display::None` 作为非活动布局隔离基础；明确 `Refresh/Ready`、原生 UI 布局、picking dispatch 与 Router publication 的调度关系。验证 active 正向命中，并覆盖离场/host loss 后、下一次布局前的输入窗口；若缓存几何仍允许旧目标命中，先补齐最小 production 隔离机制，不能用多跑空帧掩盖缺口。
- [ ] 将 authorized target 的原生 hover/previous-hover/press/click/drag/祖先缓存撤销纳入 isolation preparation，并在真实 dispatch 前过滤非活动 owned scope；保留 app/其它有效目标的输入状态。真实按下/拖拽后离场或 host loss、旧目标作为 residual 存活、继续 Move/Release 都不得再触发其 observer；不能仅测试新命中，也不能误将 `PostHover` 排在官方 `pointer_events` 前。
- [ ] `RouterInputGate` 每个输入边界从当前 active frame-local Children closure 导出；publication 同步撤销旧权限、安装新权限并释放目标 focus。动态 child 添加/移出、显式 `Visibility::Visible`/自定义 `Pickable`、保留 app focus 都有真实对照；不覆盖 authored presentation 值。
- [ ] 以真实 `AutoFocus` 的 staging 成功/失败对照验证原有效 focus、当前非活动 focus 与延后的 FocusGained/Lost；明确推荐路径与原生 Tab/dispatch 的接线边界，不把完整原生 Tab 支持偷偷加入 MVP。
- [ ] 所有 allocation/query/claim/cleanup/input preparation 在 `Ready` 之前完成，publication tail 只执行已准备的不可失败原位 mutation；successful presentation/history 是最后语义步骤。失败留原 owner，stale 丢弃而不发布。
- [ ] 建立真实 BSN root consumer 和 build/resolve/apply error 回归。resolve/apply 故障使用官方 Scene/template 的实际失败，不制造假的 Router report。验证 host sentinel/关系、旧 active、history、pending cleanup、partial subtree、focus 和 request failure 来源。

**可行性证据，非产品 gate**：隔离临时 workspace 的官方 `0.19.1` 探针已验证 owned Scene 应用到既有 root、内部 outlet、resolve error、真实 UI layout/picking 的 shell 隔离，以及局部 pointer 缓存过滤；也复现未挂接 Node 和隐藏后缓存 Click 两个缺口。新增探针验证声明图分配、外层 wrapper、命名引用和失败产物追踪，合计 10 passed / 0 failed。此结果包含反例测试，不是 10 项产品能力通过；尚未证明 Router owner/observer drift、依赖 handle 保活、原子切换、完整手势撤销、keyboard/gamepad/focus 或 Windows。产品 tests 必须重新覆盖这些范围，不能引用临时 `/tmp` 路径作为持久依赖，也不能复制探针的直接 despawn 或计数检查作为权限证明。

**验证入口**：`cargo test -p bevy_ui_router_bsn`、`cargo test -p bevy_ui_router_ecs --all-features`、新增 facade `bsn` 集成 test 与独立 workspace consumer；涉及 lifecycle 的变化同时重跑最高层导航/恢复 journeys。

**提交边界**：依赖/注册和实际调用的最小接线可以独立提交；shell/current-containment/input/cleanup 的一致性作为一个联动迁移审查单元，可以分内部检查点但不能宣称仅注册成功就完成 BSN。暂留迁移 executor 必须标注用途/退出阶段，不新增兼容包装。

## 5. 阶段 D：嵌套布局、history 与恢复闭环

**交付结果**：普通带侧栏/工具栏的 layout 通过内部 default outlet 展示 child，leaf 导航保留 layout 状态；完整基础路由、history、错误恢复和 host 重新注册从推荐 facade 可达。

**涉及文件**：`bsn/src/outlet.rs`（若拆分）、ECS `mount.rs`、`presentation.rs`、`branch_transaction.rs`、`navigation.rs`、`cleanup.rs`、必要 Core 修复；facade public BSN journeys、`crates/bevy_ui_router/examples/bsn_nested.rs`（新增）、`README.md`。

- [ ] 在本 frame content closure 内解析单 outlet，停止于 child slot；支持 root 与内部 Node anchor，leaf 无 outlet 合法。需要 child 时缺失/重复/非 Node/失效声明明确失败，不默认挑选、不 fallback 到 root。
- [ ] preserved parent 复验 exact anchor/slot/generation，child slot 独立于 marker；测试 layout、anchor、slot 和局部应用状态在 leaf 切换时保持。把 entering mount rejection 和 committed authority loss 分开，覆盖稀疏/保留 prefix，不能由 frame/slot 数量反推权限。
- [ ] 从 BSN factory 观察实际 route-local params、raw search、fragment、origin/failure。覆盖 index/pathless/静态/params/wildcard、同优先级不同 leaf 的 `MatchConflict`、参数变化的 suffix、search 整链重建、fragment-only deepest normal frame generation 推进、等价 location 的零物理 mutation。
- [ ] 完成 push/replace/back 的成功、失败 back 和串行入队结果；区分 `NoBackEntry` 同步拒绝（无 request）和入队后的 `BackTargetUnavailable`。back 在执行时选择目标，失败不改 cursor，错误 presentation 不写成功 history。
- [ ] 完成最近 boundary/error view、fallback、恢复自身失败、无 binding nested Push cleanup 后 RouterFallback；分别验证原 failure continuation 和实际 cleanup scope，不按 preserved active 或 obligation 数量选择分支。
- [ ] 验证 publication 前后 host loss 的不同结果、同 Entity registration 移除重插、重复候选、旧 epoch/task stale、重新注册仅接受新命令。保留最小 RouterHandle scope 回归，不扩成 multi-router stress。
- [ ] 将普通嵌套示例和短小 quick start 做成实际编译/运行消费者。运行环境明确安装 Asset/Scene/UI/picking/focus 所需插件，不能依赖本 workspace dev feature 偶然统一。

**验证**：真实 BSN public journeys、ECS focused + 完整 crate、受影响 Core/View tests。故障只能通过实际生产输入或私有 seam 改变 predicate 的事实输入；最高层 consumer 不依赖 owner crate、test-support、中间 schedule、audit/probe。

**提交边界**：outlet/nested 保留语义、history/input 更新、recovery/host 组合可形成独立审查提交；具体顺序可随依赖调整，不能每个检查点重写整份 spec。

## 6. 阶段 E：清理、独立审查与集成

**交付结果**：新 MVP 推荐路径完整、旧 data-only 公开面与临时桥退出，验证证据和事实文档一致，满足条件后集成 main。

- [ ] 删除不再被必要内部 fixture 使用的旧 writer/report/ContentPlan/outlet executor 及其 public 导出、兼容 consumer 和 golden。保留最小 fake renderer；删的是旧 API 形状，不是独立验证能力。
- [ ] 合并重复测试，移除固定 catalogue、Markdown 循环解析、全量 hostile-source 扫描的维护前置。保留能防止 authority 暴露的少量 compile-fail 和实际 consumer 验证，不把迁移承诺变成未经证明的删除。
- [ ] 对照 spec 第 10 节逐行附上实际 gate/结果；当前架构只记实现事实，roadmap 只留 Now/Next/Post-MVP，旧文档不再同时称自己 canonical。
- [ ] 执行下列最终验证；独立 consumer 的 target/workspace 与产品测试隔离，同一共享 target 不并行 Cargo。

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo test --workspace --doc --all-features --locked
cargo check -p bevy_ui_router --no-default-features --locked
cargo check -p bevy_ui_router --locked
cargo check -p bevy_ui_router --no-default-features --features ecs --locked
cargo check -p bevy_ui_router --no-default-features --features bsn --locked
cargo test -p bevy_ui_router --features bsn --test bsn_facade_features --locked
```

- [ ] 独立 workspace facade BSN consumer 在自身 lock 上核对官方解析图与 feature 前置，运行真实 root/nested/error；应用只依赖 facade 与实际需要的官方 Bevy crates。LF/CRLF 对照必须运行；Windows 原生验证未执行则保持 NOT_RUN，不以 Linux 文本测试替代。
- [ ] 安排 Core、ECS、public/consumer 三视角独立审查；复杂关键问题用两个代理自由交叉评价。主代理核实证据并修正，重跑受影响最高层 gate；尚有阻断问题时不激活。
- [ ] 仅在 spec 验收结果、独立审查与全量验证齐全后记录 MVP completion；检查 main 当前状态与 worktree 后按 finishing-branch 流程集成。合并后重跑必要 smoke，记录具体集成 commit；未完成集成不能把总体 goal 标为 complete。

**提交纪律**：仅精确 `git add <files>`，提交前检查 `git diff --cached --name-status` 和 staged diff；不包含他人文件。需要远端 push/PR 或扩大动作范围时按当前用户授权判断，不因“MVP 完成”自动发布包。

## 7. 执行记录与调整规则

本 spec/plan 已完成两份独立审查与交叉复核：matcher/Back 的公共语义简述已修正；pointer 历史撤权与 Scene 创建期义务均有明确设计处置。第三位代理只读核查公开 Scene graph/reference/wrapper 接口，主代理独立编译并运行机制探针。修订后两位再次收口审查并交换报告交叉评价，均确认无合同提交/开始实现的阻断项；owner/observer drift、dependency handle 保活、完整手势和 focus 验证继续属于阶段 C 的产品责任。

合同提交 `4bfbc4f` 仅修订 spec、工程护栏、索引、路线和当前事实，未改 runtime；双路独立复审与交叉复核 GO，7 份当前文档的本地链接检查及 staged diff 检查通过。临时探针 `cargo test --offline` 为 10 passed / 0 failed，格式检查成功。

迁移前 `cargo test --workspace --all-features --locked` 已完成，exit 0，包含 Recovery CRLF 回归、29 项 public navigation journeys、完整 Core/ECS/View 与 28 项 Core doctests。命令仍报告现有 unused/deprecation 等 warnings，未作为本轮文档工作顺手修复；Windows 原生验证未运行。此结果是旧 runtime 的迁移基线，不是生产 BSN 或新 MVP PASS。下一工作是阶段 A 的测试合同迁移与阶段 B 候选精简，保留已有行为证据。

阶段 A 的依赖测试迁移已落地：新增共享 source/version policy 和独立 dependency contract，移除 Core/View/ECS 的精确私有 manifest 清单及未注册的重复 facade 测试；尚保留旧 Markdown/Golden consumer gate，未变更 runtime。独立审查的配置覆盖、qualified replacement 与非默认 feature 三项漏检已修正，定向复审无阻断项。`cargo test -p bevy_ui_router --test dependency_contract --locked` 为 7 passed；`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，后者包含 29 项 public journeys、418 项 Core 和 620 项 ECS 单元测试及现有 consumer/CRLF 回归。现有 warnings 与 Windows NOT_RUN 保持如实记录；阶段 A 仍待真实 consumer 替换和旧文档整理。

每个阶段结束只补充：交付 commit、实际命令与结果、审查结论/未决项、下一依赖；临时构建路径、完整日志和细碎排查过程不固化成长期经验。阶段内私有文件布局和检查点可调整，影响公共语义、范围或信任边界才需要重新确认。

复用经验按本项目范围保存：无兼容义务不等于取消架构边界；单 outlet 限制数量而非位置；真实 input 测试先证明正向管线；失败 continuation 不由 cleanup 物理结果反推；已验证实现事实与设计/探针证据分开。不要将其它项目记忆并入本项目。
