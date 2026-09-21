# Bevy UI Router MVP Implementation Plan

> **For agentic workers:** 执行时使用 `subagent-driven-development` 或 `executing-plans`，按下述交付结果推进与审查。用户已授权统一规划、合理调度子代理和安排提交；不为每个小修复重复征求流程选择，不机械套用 TDD 或另立 spec。

**Goal:** 完成可从稳定 facade 使用的官方同步 BSN Router MVP，保留完整基础/嵌套路由、history、恢复和 owned-only/input/focus 语义。

**Architecture:** 复用 renderer-neutral Core/View；BSN 提供 one-shot Scene factory 与 outlet 声明，ECS optional bridge 独占 World、staging、publication 和 cleanup。复用 shell/content-root 两层结构，Scene 内容采用 current containment，不延续旧 data-only component manifest。

**Tech Stack:** Rust 2024、官方 crates.io Bevy `0.19.1`、Bevy ECS/UI/picking/input-focus/Scene、Cargo workspace。

**Spec:** [统一 MVP design spec](../specs/2026-09-07-router-mvp-design.md)。实现者同时读取 spec 和本计划；产品合同归 spec，本文件只拥有迁移顺序与证据。

**状态**：Active implementation plan。阶段 A 的统一合同、测试迁移与历史归档（含 ECS 遗漏补清），以及阶段 B 的候选精简和独立基线已完成。阶段 C 已落地合同/私有解析准备、原生 pointer 派发前及同调用撤权、私有 Scene 创建/adoption/失败结算，以及三个成功发布路径的常驻资源借用；生产 FrameTask 仍走旧 renderer，尚需将 native owner、current containment、mount、publication、输入/focus 与 cleanup 联动接入导航。阶段 D 的 idle host-loss 已完成空闲失效、容量、snapshot 和重新接纳的完整操作边界，并通过独立/交叉复审及冻结版本全量验证。官方 focus 队列的只读反射机制探针通过，但尚未产品接线。私有机制、探针与绿色基线不代表生产 BSN 或 MVP PASS。

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

迁移起点的 Core/View 作为锚点。ECS 主路径仍使用 `EcsViewDriver -> FrameContentWriter/ContentPlan -> RealizedFrame`；BSN 的反向依赖已在阶段 C 准备检查点修正，但尚未接入导航。输入 gate 的名单来自构建时输出，不能直接覆盖动态 native Scene。六个候选含可删产品外扩张，也含必须保留的前台安全修复。

### 当前执行重心（2026-09-21）

- 保留产品范围和 Core/View 分层；当前调整的是 ECS 操作职责与交付组织，不另开架构重写、第二份 spec 或全面安全整顿。已确认的创建期分配义务、adoption 后 containment、线性 owner 和输入隔离不能因精简消失。
- 已闭合 idle admission：所有相关 staging/activation 与外层资源重插结束时仍 non-Ready，随后复验实际旧残留、候选和新 slot，再用常驻 owner 完成最终确认。拒绝原因与新 slot 的物理清理证据分别保留；该局部修复结束，不扩成所有旧路径的无边界审计。
- 下一个产品交付是稳定 facade 的真实 BSN root 成功与真实 apply 失败，经普通 `App::update` 完成发布或恢复。native owner/current containment、mount、input/focus 和 cleanup 属于联动迁移；可分内部提交，但注册或私测单独通过不算该结果完成。
- 随实际消费者迁移收拢 ECS 私有完整操作入口，让准备、拒绝、原 owner 交接与最终发布由同一操作负责；只共享目的相同的验证，不叠加通用 wrapper/事务框架，不先整体重构 RouterEntry。旧 writer/report/replay 与内容名单在等价行为替代落地后退出，最小独立替身保留。
- 审查单元覆盖完整用户路径和最后有副作用的边界，包含同调用与下一普通更新；循环内运行定向验证，交付冻结后运行受影响完整 crate/最高层 gate，修复采用增量复审。独立/交叉意见须核对真实路径；不以评估文档数、测试数或局部绿色推算产品完成度。
- Scene focus 已找到并通过官方小探针验证的局部候选：用显式启用的官方 Reflect 只读真实追加队列，在原 Clone 上以公开 set/clear 保留合法操作、过滤本次非活动目标。生产接线仍须核对真实 owner/目标、完整效果区间与 source drift，不能把探针当完整隔离证明；也不默认全局回滚、构建期 focus 无效或成功后重放 AutoFocus。只有确需改变合法行为时才请用户裁决。

剩余工作按约四个主要交付单元组织：native root 联动、native nested/history/recovery、公开 consumer/示例与旧机制退出、最终验证审查集成。focus 机制的生产接线和行为验证属于 native 联动风险，不再预设必须增加用户限制。native 联动是最大不确定项，单元不等于小补丁、固定提交次数或工时承诺。各阶段原验收责任保持不变。

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
| `crates/bevy_ui_router_ecs/src/presentation/native_scene.rs`（新增） | 声明图创建期登记、私有 wrapper apply、adoption 与同次失败结算；不替代导航/publication |
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
- [x] 移除多份 Markdown/Golden 与源码互证 API/能力的 gate，包括专为 AST 闭世界扫描器维护的 mutation fixtures；替换为三类实际 Core/View/ECS 程序、六组独立 workspace 配置和六个真实权限边界拒绝点。普通导航/恢复/输入行为测试保留；旧 driver 仅为迁移 renderer，阶段 C 用真实 BSN 替代，独立验证能力长期保留。
- [x] 已修 Recovery profile 的 CRLF 回归在迁移基线和依赖迁移提交中重跑成功。无用途的 profile parser 整体退出后，不为留旧测试继续维护它；LF/CRLF 对照迁到同源 consumer TOML/Rust 写入→Cargo 运行路径，使用 Rust `Path`/`Command`，不拼接平台 shell。独立评估确认这是回归责任迁移，不是恢复 LF 假设；Windows 原生验证仍为 NOT_RUN。
- [x] 将依赖检查从精确私有 manifest/feature 列表改为语义约束：默认 facade 无 Bevy；Core/View 的 default/all-features 正常与构建依赖图均保持 renderer-neutral；官方 release crates 全部按包名选择并核对 `0.19.1` 与 crates.io。workspace/现有 consumer 共用 lock 检查；manifest 与实际 Cargo 配置检查 Bevy fork、patch、replacement 和路径覆盖，不限制普通依赖的合法配置。BSN→ECS 的现存错误方向仍在阶段 C 反转，Cargo 本身拒绝依赖环；这里不以依赖 gate 通过宣称 BSN 接线完成。
- [x] 有效合同已迁入经审查的统一 spec，旧文档 parser 引用已消除。Target/Matrix/Fixture/Allowlist/Golden/Journeys/Goals 和 Root/Nested profiles 原位归档，显式声明旧状态/清单不约束当前开发；保留路径和正文以免破坏历史引用。History/Recovery 完成记录继续作为历史证据，不以旧 catalogue 数量约束新设计。

**验证**：相关 facade/dependency/text tests；`cargo test --workspace --all-features` 作为迁移前基线，原样记录失败和环境限制。无 Rust 改动的合同提交仅要求 diff、引用和一致性检查；不能把文档审查写成 runtime PASS。

**提交边界**：统一 spec/护栏/索引为合同提交；本 plan 单独提交。测试合同迁移和历史材料删除另成可审查提交，不与 runtime 行为混提。必要决定全部迁入并提交后删除临时 `2026-09-07-mvp-reevaluation.md`，其证据可从 Git/任务历史追溯。

## 3. 阶段 B：精简候选，保持可用的独立基线

**交付结果**：不依赖 public audit/probe/reaper 的导航和输入基础；Core/View 与真实 ECS 路径仍可独立测试。

**涉及文件**：retention map 的 ECS 文件、`plugin.rs`、facade exports/features、`public_navigation_{journeys,authority_audit,input_isolation}.rs`、`test_support_features.rs`。

- [x] 按 retention map 移除公开 audit/probe、background reaper 调度、fairness 和仅服务它们的测试。先列出 private residual owner 的前台 callers，保留其实际义务，不能整块删除 `QuarantineLeaseTable` 来掩盖 outstanding ownership。
- [x] 仅公开 `RouterInputSet::{Refresh,Ready}` 和 `RouterScheduleSet::PublishSnapshots`。旧 public journey 中必须在内部暂停/注入的故障回归迁回 ECS 私有 tests；外部 consumer 只执行普通 `App::update` 和稳定入口。
- [x] 保留最小 fake renderer 驱动生产 Core/ECS 逻辑，先明确 fixture 控制的真实 build/apply 事实；不能注入 classification、terminal 或 publication 结论。旧 data-only executor 可暂为迁移工具，阶段 C/D 替代后退出，测试独立性本身长期保留。
- [x] 重跑 root/nested/reconciliation/history/recovery 的已有 public 行为与相应 ECS crate。若最高层回归失败，沿生产路径确定 cause；不得通过恢复旧 Terminal、删目标断言或增 retry 次数过关。

保留的迁移 renderer 只控制构建成功、构建/部分 apply 失败、缺失/重复 outlet、旧 session handle 与真实关系环等输入事实；实际 writer 拒绝、Core 分支分类、cleanup、publication 和请求终态继续走生产路径。它不证明官方 Scene 行为，阶段 C/D 将替换生产入口。

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
- [ ] 以真实 `AutoFocus` 的 staging 成功/失败对照验证原有效 focus、当前非活动 focus 与延后的 FocusGained/Lost；明确推荐路径与原生 Tab/dispatch 的接线边界，不把完整原生 Tab 支持偷偷加入 MVP。普通 typed 方法没有队列过滤接口，但官方 Reflect 可安全读取实际队列；已验证的追加候选保留原队列、合法 clear/set 及 cause，无需整体回滚或提前派发事件。采用时显式启用所需 feature，验证固定版本 schema、真实追加来源和目标分类；源码/来源不匹配不能伪装成已隔离。不得据此默认新增构建期 focus 限制或重放政策。
- [ ] 所有 allocation/query/claim/cleanup/input preparation 在 `Ready` 之前完成，publication tail 只执行已准备的不可失败原位 mutation；successful presentation/history 是最后语义步骤。检查范围包括外层资源 scope 的退出：官方 Bevy 0.19.1 的 `resource_scope` 会取出并重新插入 Resource component，不能把它当作无副作用借用。三个成功发布路径已迁往 exact resource Entity 联合借用；native owner/内容边界接入后仍须验证完整 preparation 与 publication，不能将此检查点推广为其它 terminal/cleanup/no-target/snapshot 路径全部闭合。失败留原 owner，stale 丢弃而不发布。
- [ ] 建立真实 BSN root consumer 和 build/resolve/apply error 回归；由外部系统在 `RouterScheduleSet::PublishSnapshots` 后读取同帧 snapshot，补足目前仅编译 label 的边界验证。resolve/apply 故障使用官方 Scene/template 的实际失败，不制造假的 Router report。验证 host sentinel/关系、旧 active、history、pending cleanup、partial subtree、focus 和 request failure 来源。

**已落实的发布迁移依赖**：无已提交 active 的 RouterFallback 在最终复验发现 host loss 时，原 Core authority settlement 因缺少 reconciliation 拒绝，错误进入协议 fail-close 而非 `HostLost`。独立评估及交叉复核后复用已消费的 failed Task source、原 HostLease 与 authority cleanup，仅为 unbound fallback 且 Core active kind 为 None 增加窄接纳分支；既有身份、Applying、action、retirement、lease 和 exact roots 校验继续复用，不伪造 reconciliation 或新增清理协议。Core 保留 Task source 的两种合法 roots 选择；当前已 realized 的 ECS caller 必须据真实 pending owner 提交包含 target 的完整 manifest，不能按物理数量反推逻辑 roots。拒绝返还原 task/lease；host loss 与 parent-only rejection 分别验证 `HostLost`、`RecoveryFailed`。完整 Core/ECS 和最高层 public/consumer 验收结果见执行记录；此窄修不代表有旧 active 的全部 projectionless host-loss、idle host loss 或其它 terminal/snapshot 资源边界已闭合。

同一回归进一步确认，无候选的空闲观察只匹配 permit 签发结果中的 `Ineligible`，会丢弃实际 `Issued` owner，令之后重新注册永久停在 `Outstanding`。本检查点复用既有 candidate rejection/pre-staging unavailable 结算处理全部返回类型，保留首次 `AwaitingHost` 的无候选语义；验证先经历无候选空闲更新再重新注册、无悬空 admission owner、旧请求不重放和新命令可用。这是既定 host/re-registration 合同的实际依赖，不通过移动注册时序绕过；不因此宣称阶段 D 的完全 idle host-loss 检测已完成。

**可行性证据，非产品 gate**：隔离临时 workspace 的官方 `0.19.1` 探针已验证 owned Scene 应用到既有 root、内部 outlet、resolve error、真实 UI layout/picking 的 shell 隔离，以及局部 pointer 缓存过滤；也复现未挂接 Node 和隐藏后缓存 Click 两个缺口。新增探针验证声明图分配、外层 wrapper、命名引用和失败产物追踪，合计 10 passed / 0 failed。此结果包含反例测试，不是 10 项产品能力通过；尚未证明 Router owner/observer drift、依赖 handle 保活、原子切换、完整手势撤销、keyboard/gamepad/focus 或 Windows。产品 tests 必须重新覆盖这些范围，不能引用临时 `/tmp` 路径作为持久依赖，也不能复制探针的直接 despawn 或计数检查作为权限证明。

**验证入口**：`cargo test -p bevy_ui_router_bsn`、`cargo test -p bevy_ui_router_ecs --all-features`、新增 facade `bsn` 集成 test 与独立 workspace consumer；涉及 lifecycle 的变化同时重跑最高层导航/恢复 journeys。

**提交边界**：依赖/合同与私有官方 Scene 解析准备可以先形成检查点；公开注册与能够成功执行的 native root realization 一起启用，避免安装接受一个只能失败的 driver。shell/current-containment/input/cleanup 的一致性作为一个联动迁移审查单元，可以分内部检查点但不能宣称仅注册成功就完成 BSN。暂留迁移 executor 必须标注用途/退出阶段，不新增兼容包装。

## 5. 阶段 D：嵌套布局、history 与恢复闭环

**交付结果**：普通带侧栏/工具栏的 layout 通过内部 default outlet 展示 child，leaf 导航保留 layout 状态；完整基础路由、history、错误恢复和 host 重新注册从推荐 facade 可达。

**涉及文件**：`bsn/src/outlet.rs`（若拆分）、ECS `mount.rs`、`presentation.rs`、`branch_transaction.rs`、`navigation.rs`、`cleanup.rs`、必要 Core 修复；facade public BSN journeys、`crates/bevy_ui_router/examples/bsn_nested.rs`（新增）、`README.md`。

- [ ] 在本 frame content closure 内解析单 outlet，停止于 child slot；支持 root 与内部 Node anchor，leaf 无 outlet 合法。需要 child 时缺失/重复/非 Node/失效声明明确失败，不默认挑选、不 fallback 到 root。
- [x] 补齐已确认的 idle host-loss 缺口：所有请求终态后的 registration removal/host despawn 在普通 `App::update` 下主动失效，不依赖新请求或 cleanup-in-flight；包括 Ready 无 active 的已绑定 host。验证旧成功结果/history 不回滚、owned-only cleanup、输入/focus 撤权和重新注册。原 grant 的请求独立 settlement、成功 retained 责任、容量/public snapshot 与 staging/资源重插后的最终接纳复验已通过独立/交叉审查和全量验证；不代表尚未接线的 native Scene 已完成相同验收。
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

阶段 A consumer 迁移与归档（`e0d52c4`）：上项依赖迁移已提交为 `3f8ff83`；本次删除 facade 的旧 Markdown/Golden/AST harness 后，六组真实正向 consumer 和六个单独拒绝点通过。迁移范围和新实现分别获独立只读审查；主代理按建议改用真实内部 entry/publication 入口、完整 handle 构造和 session 二次消费，不以缺字段或其它编译故障冒充权限拒绝。`cargo test -p bevy_ui_router --test facade_consumers --offline --locked`、格式检查及显式 consumer fixture 格式检查、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0。全量保留 29 项 public journeys、418 项 Core / 620 项 ECS 单元测试和 28 项 Core doctests。旧协议原位归档、当前事实/路线同步；当时记录“阶段 A 交付”，但其解析依赖清理结论有下述遗漏。Windows NOT_RUN，现有 warnings 未顺手处理；`bsn-wiring` 不证明 native Scene。

阶段 A 补漏：阶段 B 的调用者核查发现 ECS `authority_regression_catalogue` 仍解析已归档文档，之前“旧 parser 引用已消除”的范围判断不完整。独立只读核查确认其 18 项测试全部是文档/AST/源码形状扫描，没有 runtime 或独立 compile consumer；现已删除该文件及 `app_ext`/`snapshots` 两项同类 exact API shape gate、失去消费者的 ECS dev `syn`/`toml` 依赖。真实注册、snapshot retention、authority/cleanup/navigation 回归及外部正向/拒绝 consumer 保留。其它私有源码形状断言的合并仍按阶段 E 处理，不以本次补漏宣称所有静态测试已退出。

该补漏独立复核无阻断项，与阶段 B reaper 候选在同一工作区完成 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked`，均 exit 0；其中仍有 29 项 public journeys、独立 consumer 和完整 Core/ECS/View 回归。测试/依赖补漏单独提交，不含 runtime 修改；reaper 删除及其行为验证另记下一检查点。Windows NOT_RUN，现有 warnings 未顺手处理。

阶段 B reaper 检查点（本记录所在提交）：阶段 A 补漏已单独提交为 `d7155d9`。本次删除后台 reaper/fairness、专用 observation/evidence/游标和 `ReapQuarantines` 调度，保留前台 host、cleanup、presentation、input 与 stale-output 使用的 private residual owner、exact transfer 和 terminal provenance。旧 lease/fresh epoch 回归改走普通 `App::update`：旧实现会回收 claim，按新边界预期失败；删除后验证同一旧 owner/claims、Hidden/IGNORE/deny-all/三 modality 拒绝和 fresh epoch 不变。插件安装保留首次系统数量与重复安装列表一致性检查；facade 仅移除后台 GC 断言，并把 cleanup 采样点移到 Settle 后。

两位独立只读审查并交叉复核，首次安装测试缺口已补，定向复审无阻断项。补后 focused 回归、`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0；全量包括 597 项 ECS 单元测试、418 项 Core 单元测试、29 项 public journeys、独立 consumer 和 28 项 Core doctests。7 份当前文档的本地引用检查通过。Windows NOT_RUN，现有 warnings 未顺手处理。此检查点不完成阶段 B：public audit/probe/test-support 外扩面和其余公开内部调度仍待迁移，生产 BSN 与 MVP 集成尚未完成。

阶段 B surface/调度收口（`0493e48`）：reaper 检查点已提交为 `7e00b99`。本次删除公开 audit/probe DTO、计数端点、专属 projection/inventory/tracked-slot traversal、ECS/facade test-support features 和中间 public 调度标签。真正供私测使用的 back-capacity 顺序、delayed-report replay、host-admission 和 commit-seal seam 收为 `cfg(test)`；Core dev test-support 与前台 residual owner 保留。迁移关系如下：

| 原验证职责 | 当前验证落点 |
| --- | --- |
| audit consumer 中的 Back 同步拒绝、forward-tail 截断 | public journeys 的 `history_back_no_entry_is_synchronous_and_preserves_snapshots`、`history_back_then_push_truncates_forward_tail` |
| probe 计数之外的输入/focus 行为 | public nested replace 的旧目标撤权、preserved layout/new leaf 正向 gate、focus 释放；ECS 真实 picking 和生命周期输入私测保留 |
| commit seal 后 registration loss | 新增 ECS `host_registration_loss_after_commit_seal_cancels_publication_and_settles_owner`，有/无 active 两行，真实 registration 变化、取消 publication、原物理 owner 结算及严格 HostLost |
| post-commit cleanup、same-host retired edge、stale report 精确窗口 | 保留 ECS `post_commit_host_loss_enters_single_compound_without_rolling_back_history`、`same_host_readmission_waits_for_retired_edge_and_old_slot_disposition`、sealed/ownerless stale-output 与旧 lease/fresh epoch 私测；public 只验证明确 queued-loss、history 保留、re-admission/显式 restore 与 foreign 内容保护 |

普通 `App::update` 返回时本 fixture 已完成 cleanup，不能以“已观察 publication”推断仍处于内部窗口。测试迁移暴露完全 idle 的 registration loss 不启动恢复；它是既存 MVP 缺口，已列入阶段 D 和 current architecture，未通过新增前台请求冒充修复。两位独立审查及交叉复核确认上述分层迁移无剩余阶段 B 阻断项；同一编译意见合并处理，最后修订重新验证。

最终 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0；包含 582 项 ECS / 418 项 Core 单元测试、30 项 public journeys、独立 consumer 和 28 项 Core doctests。精确 seal focused 回归也通过。Windows NOT_RUN，现有 warnings 保留。阶段 B 的独立迁移基线完成；生产 BSN、idle host loss、MVP 验收和 main 集成未完成。

阶段 C 解析准备检查点（`9ee02ec`）：反转 BSN/ECS 依赖，新增官方 Scene 依赖与 optional bridge，提供 `BsnSceneFactory`、`BsnBuildError`、`DefaultOutlet` 及 facade 导出。私有 preparation 消费真实 owned Scene、检查必需 runtime、保留原引用与 ChildOf 顺序、拒绝 cached inclusion/重复引用，并准备 attempt 唯一匿名引用。resolve 与图预检失败也携带 dependencies 到后续结算；没有把 Scene 放进旧可克隆 artifact。公开注册和 native realization 尚未接入，阶段 C 各交付项仍保持未闭合。

该检查点的双路独立审查及交叉复核无阻断项；按意见补齐失败 dependency lifetime。`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，含 592 项 ECS（新增 10 项 preparation）、418 项 Core、30 项 public journeys、8 项 dependency contract、独立 consumer 和 28 项 Core doctests。另行 `cargo test -p bevy_ui_router_bsn --offline --locked` 与 facade `--no-default-features --features bsn` check 通过；BSN crate 自身尚无独立 tests，行为测试在 ECS 与 facade。facade 的官方 Scene outlet 声明测试不等于 Router native gate。7 份当前文档的本地链接检查通过，现有 warnings 保留，Windows NOT_RUN。下一依赖仍是 exact 创建期义务、wrapper apply、current-containment/input/cleanup 联动与真实 root consumer；consumer 同时补普通非 Scene asset handle 的正向用法，不能仅靠现有 ScenePatch dependency 用例代替。

阶段 C pointer 派发前撤权检查点（`09642d1`）：真实 UI/picking 测试复现旧页面离场后继续收到 Drag。修复在 gate mutation preparation 预备历史撤权目标，`Refresh` 清既有缓存，并在 `generate_hovermap` 后、`update_interactions`/`pointer_events` 前按目标过滤新命中、前次 hover、press/click/drag 与祖先缓存，复位已有 `PickingInteraction`；保留 app-owned 和其它有效目标。原子 apply 只替换已准备的 gate 数据，没有把查询/分配放进 publication tail。

两份独立审查及交叉复核无剩余阻断项。按审查修正 active 祖先误分类与 interaction 残留，并让真实回归在旧目标移除父链和 seal 后仍验证历史撤权；它同时验证有效祖先与独立 app-owned 交互保持。最终 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，包含 593 项 ECS、418 项 Core、30 项 public journeys、独立 consumer 与 28 项 Core doctests；focused 原生手势回归也通过。现有 warnings 保留，Windows NOT_RUN。

本检查点不闭合阶段 C 的 pointer 整项：publication 同一 exclusive 调用内直接清 Bevy 缓存、native current-containment 目标、Scene failure/AutoFocus 及 host-loss 接线仍在后续。下一实现继续声明图 exact 创建义务与 wrapper apply，同步迁移 adoption/input/cleanup；生产路径核查确认可复用 exact registry retirement 和局部 cleanup 证据，但不能直接复用旧 closed manifest 或永久输出名单。相关映射仅为临时实现依据，未记作 native apply 验收。

阶段 C 私有 native 创建检查点（本记录所在提交）：新增批量 registration/seal 预备与完整声明图先登记，传播解析 attempt，使用官方引用 seed/wrapper apply；成功后校验 live adoption、逐项释放 descendant 临时标记并退役 exact claims，返回最终真实闭包。失败结算区分 exact 叶节点删除与 protected 残留，保留 authored 组件值；写后复验全体剩余目标、关系和 shell 隔离。独立审查发现的合法声明/普通容器交错误判、遗漏 live quarantine/unresolved owner，以及 marker hook 后闭包/结算问题已修正。普通 Image 的成功/失败 dependency handles 也由实际 preparation 路径验证。

本次新增 13 项 native 私测覆盖双向引用与首写前完整登记、部分 Node 写入失败及原错误深度、合法交错拓扑、真实 hook 漂移、普通组件正向、既存 residual owner 保护和 marker-release 后结果；三项 registry batch 测试另验证完整容量、owner 冲突与 seal 永不复用。用例必须先建立真实触发窗口：官方 scratch 写入会替换提前附加的 Children，hook 不能在正常 root 挂接时提前触发；Entity 活数量使用 `count_spawned`，不是索引容量 `len`。

修正后两名独立代理复审并交叉评价，无剩余私有检查点阻断项；主代理核实对应生产路径与实际测试。定向 native 验证为 13 passed；`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，包含 609 项 ECS、418 项 Core、30 项 public journeys、8 项 dependency contract、独立 consumer 和 28 项 Core doctests。registry batch 另有 no-default-features 定向与完整 ECS 验证。现有 warnings 保留，Windows NOT_RUN。公开注册、导航/前台 owner、mount、输入/focus、统一 cleanup 及最高层 BSN consumer 均未闭合，`Unisolated` 仅为错误证据而非已隔离结果；下一步接入这些相互依赖的 production consumers，阶段 C 各产品交付项不因此打勾。

阶段 C pointer 同调用撤权检查点（本记录所在提交）：三个正常 publication consumer 与 authority isolation 均直接清理已授权目标的原生缓存和已有 interaction。新增资源借用采用 exact resource Entity 联合 query，不使用会 take/reinsert 的 `resource_scope`；默认禁用目标/资源仍可撤销历史输入，但正常 presentation 的查询资格不放宽。post-commit 原观察 partition 收窄为 owned obligations，并以实际历史撤权及当前 exact claims 授权 native 写入，保留 protected/app 状态。真实双 pointer 管线、Replace/Back/fallback/host-loss consumer、拒绝准备、无 gate、禁用实体和无操作 resource tick/observer 对照均由生产路径或真实输入事实验证。

两名独立代理复审并交叉评价，无剩余本检查点阻断，主代理核实了生产路径与运行输出。最终 all-features 与 no-default-features pointer 定向各为 7 passed；fmt、workspace all-targets/all-features check 和 workspace all-features test 均 exit 0，包含 615 项 ECS、418 项 Core、30 项 public journeys、独立 consumer 和 28 项 Core doctests。既有 warnings 保留，Windows NOT_RUN。此结果不闭合 native current-containment、Scene focus、idle host loss，或旧 gate/focus/registry 资源 scope 的整个 publication 边界；这些义务继续保留在阶段 C/D，不因 pointer 检查点通过而打勾。spec §7 创建总览同时校准为 §12 已选定的 factory/resolve/预检后才分配 UI 的顺序。

阶段 C 常驻资源 publication 检查点（2026-09-12，本记录所在提交）：normal reconciliation、Back target、projectionless fallback 在 World preparation 及外层资源 scope 退出后重新验证原来源，最终借用 exact resource Entity 上的常驻资源。原 active/cleanup owner 在 Core publication 前安装，发布后不再重新插入资源或补装 owner；registry index plan 在可变准备后重建。保留真实 Core commit seal 后 registration loss 的回归窗口，不用更早的失败替代。初始无 active fallback 的窄 HostLost 接纳及无候选 Issued permit 丢失修复一并落地，保持原 task/lease、完整物理义务和重新注册语义。

独立审查与交叉复核发现并修正 Back 最终 focus 来源漏验，以及等价 gate 因顺序改变而误写 tick。完整来源包含 leaving 与 RetainEmpty，来源漂移拒绝，单纯 focus 值变化仍允许发布。进一步用普通 `App::update` 复现了测试第二次手动 commit 曾掩盖的调度缺口：原 active 暂存于 splice，下一 Refresh 误撤 gate，后续 fallback 误变 AuthorityLost。最终在同调用复用原 pending-rejection consumer 归还 active；测试不再额外手动推进，验证原 entering owner、Apply/fallback、history 保持、app focus 与前台 owner 清空。最终两位原代理分别定向复审并交叉评价，无剩余本检查点阻断；主代理核实了生产路径、修复增量和实际命令输出。

最终冻结源码的 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked`、`cargo test --workspace --doc --all-features --locked` 均 exit 0。全量包含 420 项 Core、627 项 ECS、30 项 public journeys、独立 facade consumer、8 项 dependency contract、12 项 Core compile-fail 与 28 项 Core doctests。另行完整 ECS `--no-default-features` 为 604 项单元测试及 public API 通过；阶段 E 列明的 facade minimal/default/ecs/bsn 四组 check 与 `bsn_facade_features` test 均 exit 0。定向 Back focus 3 项、publication 21 项（含 RetainEmpty/资源 observer/初始 fallback）、gate 13 项、真实 pointer consumer 7 项均通过。既有 warnings 保留，Windows 原生验证 NOT_RUN。此结果不是完整 BSN 或 MVP 验收：下一依赖仍是 native store/执行与 RealizedFrame owner 转移、current containment/mount/input/focus/cleanup 的联动、真实 facade consumer，以及阶段 D/E 的其余闭环与 main 集成。

阶段 D idle host-loss 检查点（2026-09-21，本记录所在提交）：以真实 retired host grant 建立请求独立 settlement，覆盖已终态 active 与 Ready-empty、registration removal/despawn/同 Entity 重插，保留旧请求/history并仅接受新命令。容量错误保留 retry 和原 owner；receipt 完成与当前残留隔离分开；cleanup snapshot 反映实际 holder/receipt。重新接纳将唯一 pending owner 跨 staging/activation 和三层资源重插传递，最后复验后常驻确认；真实 retained 拒绝原因与新 slot 清理分开。只接纳三个 exact resource carrier 的预期重插，按每个 Router 完整操作依次确认，未引入全局事务或 reaper。

两位原审查代理完成独立增量复审与交叉复核，无剩余本任务阻断；主代理核实生产路径和真实日志。17 项 idle 定向和既有跨 Router 对照通过。最终冻结源码的 workspace all-targets/all-features check、完整 workspace test（含 422 Core、645 ECS、35 public journeys、consumer/compile-fail/View 与 doctest）、workspace doctest、完整 ECS no-default（622 单元测试）、四组 facade feature check、BSN feature test 和 fmt 均 exit 0；既有 warnings 保留，Windows NOT_RUN。这是旧/独立 renderer 路径上的 host 恢复交付，不是 native BSN 或 MVP 完成；下一工作回到阶段 C 联动接线。

同期 focus 可行性核查纠正了工程预设：Rust 字段私有不等于官方 Reflect 不可用。独立评估及交叉核对后，固定官方 0.19.1 的独立探针以显式 Reflect feature 读取实际操作后缀，用原 Clone 和公开 set/clear 保存原队列及其它有效目标，4 项测试通过，含真实 Scene AutoFocus 后成功/模板失败与延后事件。它仅证明追加来源下的局部机制；schema/source 不符明确拒绝而非伪造隔离。尚未接入 Router，不改变成功 AutoFocus 策略或扩大任意资源回滚合同，也不再预设必须牺牲合法构建期 focus 行为。

每个阶段结束只补充：交付 commit、实际命令与结果、审查结论/未决项、下一依赖；临时构建路径、完整日志和细碎排查过程不固化成长期经验。阶段内私有文件布局和检查点可调整，影响公共语义、范围或信任边界才需要重新确认。

复用经验按本项目范围保存：无兼容义务不等于取消架构边界；单 outlet 限制数量而非位置；真实 input 测试先证明正向管线；失败 continuation 不由 cleanup 物理结果反推；已验证实现事实与设计/探针证据分开。不要将其它项目记忆并入本项目。
