# Current Architecture

**类型**：实现事实与验证边界。**更新时间**：2026-09-23。**runtime 迁移**：从 `sys/structural-governance` 的 `0799098` 前向精简；idle host-loss 基线为 `8ce2b41`，Core 来源修正检查点为 `e7445a2`，native 联动为 `c203446`。独立/交叉审查、最终 fixture 差异审查、冻结版本完整验证及本地 `main` 快进后的公开 smoke 均通过；MVP 已在本地 main 验收。后台 reaper 与公开 audit/probe 已退出。

范围与验收以 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md) 为准，后续工作见 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## 已接受基线

`Cargo.lock` 已跟踪，直接 Bevy 依赖使用官方 crates.io `0.19.1`。MVP 结论另由实际产品验收与本地集成支持。

| 层 | 已接受实现 | MVP 结论 |
| --- | --- | --- |
| Core | builder/matching、请求顺序、branch diff、frame/generation、history、recovery 和唯一逻辑 publication | MVP 已验收 |
| View | renderer-neutral key、registry、descriptor、effect/input 合同 | MVP 已验收 |
| ECS | host/slot/frame binding、native 执行、staging、cleanup、输入/focus、physical authority 与 idle host-loss；旧 Data 注册仅在单元测试中编译，专属延迟回放及冗余 fixture 已退出 | MVP 已验收 |
| BSN | owned `BsnSceneFactory`、`BsnBuildError`、`DefaultOutlet`、公开注册与真实导航；依赖已反转 | MVP 已验收 |
| Facade | 导航、snapshots、真实 native public journeys；独立 Core/View/ECS/BSN consumer；推荐 quick start 与内部 outlet 示例；旧 writer/report 导出及注册已撤下 | MVP 已验收 |

迁移前主链为 `EcsViewDriver → FrameContentWriter/ContentPlan → RealizedFrame`；现在公开 BSN 注册经真实 FrameTask 分派到 native Scene，原 adoption 直接进入 RealizedFrame。旧 writer/report 不再公开，Data executor 的注册和可构造变体只在单元测试中编译，由私有扩展驱动原 Core/ECS 管线。仅供测试捕获/延迟释放旧输出的 owner、投影、调度与 cleanup 包装已成组删除；真实 `pending_frame_drive` 重试、stale 接收、旧残留隔离和写后复验保留。剩余 ContentPlan 执行器服务 no-BSN 独立行为验证，不要求为符号归零而删除。调度仅公开 `RouterInputSet::{Refresh,Ready}` 与 `RouterScheduleSet::PublishSnapshots`，中间 lifecycle 调度为 crate 私有，必要故障 seam 只在私测中编译。

BSN preparation 调用真实官方 `ScenePatch::load/resolve`：一次 factory 调用消费 fresh Scene，检查 runtime、cached inclusion 和重复引用，准备保持 ChildOf 顺序的声明图及 attempt 唯一匿名引用。成功与失败结果均携带 dependency handles，供实际 apply/settlement 保活；这不承诺为已结算的 inert residual 保活到实体销毁。已应用组件中的强 Handle 按 Bevy 自身规则保活。

私有 `presentation::native_scene` 消费同一 prepared Scene：先批量预备 registry 容量和 seal，再登记 shell 与全部声明 Entity，之后才写私有组件；引用 seed 与 shell wrapper 交由官方 executor 执行。成功 adoption 检查实际关系与其它 live owner，逐次复验临时标记移除产生的变化，退役 descendant 创建期 claims，并返回最后的真实内容闭包快照；该快照不是永久 ownership 名单。失败时只删除有 exact 证明的叶节点，保留 protected closure，并在每次关系/删除写入后复验剩余目标与隔离 shell；合法声明/普通容器交错不再误判为关系环。无法证明隔离时返回明确 `Unisolated`，不能当成 inert 或 publication 成功。导航消费这些失败产物的联动证据见下方已验证检查点。

原生 pointer 的既有 hover/press/click/drag/祖先缓存与 `PickingInteraction` 已在三个正常 publication 入口及 authority isolation 的同一 exclusive 调用内撤销。正常发布通过 exact resource Entity 联合查询预借资源/组件，pointer tail 只做原位修改；历史目标或资源 Entity 被禁用也不会漏清，正常 presentation 的默认查询过滤不因此放宽。post-commit 的观察 partition 不再被全量用作撤权目标，protected/app 状态保持。`Refresh` 与 `generate_hovermap` 之后、`update_interactions`/`pointer_events` 之前的定点过滤继续防止非活动命中派发。native 内容、输入及 Scene AutoFocus 已通过下述联动检查点验证。

三个成功发布路径（normal reconciliation、Back target、projectionless fallback）现已将 World preparation 及外层 `RouterNavigation` 资源 scope 的退出放到最终复验之前，再按 exact resource Entity 联合借用常驻 navigation/registry/gate/focus/pointer/presentation。原 active/cleanup owner 在 Core publication 前安装，发布后不再通过资源重新插入补装 owner。Back 最终复验比较完整 leaving 与 RetainEmpty focus 来源；来源漂移沿现有 pending rejection 同调用归还原 active，再由正常调度清理与 fallback，不让临时 owner 存放位置导致下一次 Refresh 误撤有效权限。仅 focus 值变化仍可发布，等价 gate 变化保持原顺序与 tick。native owner/current-containment 已接入这些路径；各类 terminal、cleanup、no-target 或 snapshot 的证据仍按实际路径区分，不由成功发布检查点统一外推。

无 active 的初始 unbound RouterFallback 在发布前失去 host 时，Core 接纳原 failed Task source 进入现有 authority settlement，保持 `HostLost` 而非协议错误；有 active 的 fallback 不由此窄分支接纳。Core 保留其合法逻辑 roots 选择，ECS 提交实际 pending 的完整物理义务。无候选观察复用完整 permit outcome 处理，避免丢失 Issued owner；完全 idle 的 host-loss 则由后续独立检查点闭合，见下文。

**focus 已通过本次联动验证**：官方 Reflect 只读真实操作后缀，原 Clone 与公开 set/clear 保留合法操作；官方 drain 接入保留应用初始化、函数/set 排序和延后事件时机，不需要 fork 或 unsafe 私有布局访问。仅由原 frame owner 在成功发布时兑现最后未被后发操作覆盖的 pending 意图，并分别核对当前 source-frame 归属与 prepared resulting 视觉资格。真实回归覆盖成功/失败、跨 drain 同值操作、无操作借用、资源 scope 尾部、Back/fallback、动态隐藏与移到 preserved frame 后不重放旧意图。共享资格不替代 ownership，不新增全局焦点管理。

`QuarantineLeaseTable` 仍承担前台 cleanup/host 无法安全销毁的私有残留 ownership；verified、unresolved、authority-lost 转移、stale projection 和输入/focus 隔离仍使用它。旧残留不持有 terminal 请求的前台 capability，也不会因 blocker 消失自动回收；Entity/metadata 可能长期保留，不能清空 table 冒充完成 cleanup。

普通 cleanup ledger 用一个停止原因保存原 refreeze/settlement 证据；永久停止通过现有隔离与终结入口移交整个 ledger，不另存一个永久等待的前台 holder。应用自有 host 的无关组件变更不再要求完整 manifest 相等；host staging 也不再冻结整个 World 的实体/组件清单，配套的资源载体回填白名单已删除。相关 host/slot 身份、关系、registry、quarantine 与输入事实仍需匹配，foreign target 内容仍隔离保全。Core 的普通 fail-closed 和三条 Back 终结路径复用私有记录准备/安装，分支的 source、owner 和结果分类保持独立；发布后的清理失败保留已发布的原请求结果与 history。

host trace 只保留最近 64 条事件，当前 top-slot 注册去重证据独立于日志保留，避免正常反复接纳导致诊断无限增长或日志淘汰改变 authority。

facade 合同验证已迁往独立 consumer 与语义依赖检查；ECS 遗漏的旧文档 catalogue、两项 AST API 清单及已替代的私有源码形状断言也已退出。独立 workspace 的真实 BSN consumer 已运行嵌套/history/错误恢复与 LF/CRLF 对照；旧 session 专属公开拒绝行随 API 删除，身份、handle、navigation/publication 权限拒绝仍保留。ECS-only consumer 只证明插件资源、安装拒绝和普通更新行为，不声称外部独立 renderer；无 BSN 的渲染/lifecycle 独立性由 ECS 私有真实管线测试验证。推荐示例也由独立 consumer 直接运行；完整同版本验证已通过，集成状态见 plan。

`a153520` 在同一独立 BSN consumer 内验证普通应用系统于 `Update` 的公开 `PublishSnapshots` 标签之后读取本轮完整 snapshots；实际 native 输入授权作为独立发布证据，防止延后一轮的旧快照靠 polling 通过。此 test-only 检查点没有新增 runtime 或公开 API。

原 public journey fixture 已整体改为真实 BSN factory/Scene，覆盖实际 build/resolve/部分 apply、内部 outlet、location 更新、history/Back、boundary/fallback/recovery 和 host 组合，未另建永久平行 suite。native 根被销毁后的 cleanup preparation 不再获取新内容 claims，但原义务仍由现有 ledger 结算；创建前拒绝时的 committed outlet 失效通过原 task/current binding 的 typed evidence 进入现有 authority settlement，避免空 failed manifest 掩盖永久失效。独立/交叉审查与冻结版本验证通过，未修改 Core 或新增公开协议。

## 已验证的 native 联动检查点

- 同一 key store 已支持 native factory；公开 `AppBsnExt` 经真实 FrameTask 调用官方 Scene，消费原 adoption 到 RealizedFrame，不走 cloneable ContentPlan/replay。
- current containment、动态子节点输入与 root/internal outlet 已接入；typed outlet binding 分开物理 anchor、authority source 和实际 metadata 更新。cleanup 仅从真实 owner 为当前目标准备临时义务，不恢复普通后代的永久 FrameOutput 名单。
- 真实回归覆盖 root 成功/实际 apply 失败恢复、内部 outlet/parent-only、原生 UI picking 手势撤权及 app focus 保持。独立 LF/CRLF 应用的嵌套 push/replace/Back/错误恢复已运行通过；包括最终 focus 修正的冻结源码已通过 workspace 全目标检查、全功能测试、doctests、facade feature 组合及格式检查。
- 未 adopt 创建产物的 failure/Back/terminal ownership 交接已通过生产路径测试和独立审查。原错误、执行依赖、物理残留责任与前台 continuation 分开交接，不以假 frame 或第二 runtime 适配旧类型；当前 typed cleanup 观测与原 owner 一起保留，不被此前容量错误遮蔽。
- 创建期 host registration 丢失不再误报普通 Apply failure；“fallback 已签发、仍有 active 时首次失去 host”使用明确 task/active 来源，复用既有 authority settlement 与重新接纳链。真实 HostLost/history/同 host 接纳、来源漂移及拒绝/重试已验证并审查。旧 Task-only 权限未扩大，没有把正常 HostLost 改成永久协议错误。

这些用户路径的完整消费结果已按统一 plan 验收；后续维护仍沿现有职责边界，不扩展为 Core 重写或全面安全审计。

本轮按真实反例完成局部减法：Refresh 复用一次 current-content/eligible 观察；共享 manifest 观测只排除官方动态 `CalculatedClip`，现有无分配和关系差分消费者保持一致，未放宽各自关系/authority 规则。普通隐藏和重新显示不再误报挂载失效。两位原审查者完成独立及交叉复核、两次修正增量复审，本检查点无剩余可行动 finding。既有 warnings 保留；Windows 原生验证未运行。

## 必须保留的行为

- 基础/嵌套路由、稳定遍历与 specificity 冲突诊断；不同 leaf 的最高优先级并列为 `MatchConflict`。
- common-prefix 保留、suffix 替换；search 变化重建整链，fragment-only 推进最深 normal frame generation，等价 location 可零物理 mutation。
- push/replace/back 成功后统一提交 history；`NoBackEntry` 是无 request 的同步拒绝，运行时无目标是 `BackTargetUnavailable`。
- boundary/error/fallback 与 host loss/re-registration；无 binding nested Push 失败 cleanup 后进入 `RouterFallback`，不恢复旧 Terminal 语义。
- exact identity、线性 owner、publication-last、owned-only cleanup、stale 拒绝、非活动输入/focus 隔离和 app-owned host 保持。

以上是已有行为与迁移保留项；整体验收以统一 plan 的各项真实证据及 main 集成为准。

**idle host-loss 已闭合**：所有请求终态后的 registration removal/host despawn 会主动处理，包括 Ready 无 active 的绑定；真实 retired grant 建立请求独立的 owner，旧请求/history 保持。重新接纳将原 grant/candidate/staged slot 交给唯一 pending holder，全部 staging/activation 与外层资源重插后再复验实际旧残留、候选和新 slot，以常驻资源借用完成确认；真实拒绝原因与物理清理证据分开保留。两个真实 hook 回归、17 项定向、冻结版本 workspace/feature 验证及独立/交叉复审通过；这是当时迁移 renderer 的完整基线，不能自动推广为全部 native 输入/cleanup 已验收。

## 历史与候选证据

Root、Nested、Reconciliation、History、Recovery 的完成记录见 [历史索引](superpowers/README.md)。Recovery 验证候选为 `494ae6b`；结构治理范围 `211874a..2f2cb42` 保存其行为，历史门禁只证明当时范围。

其后六个候选 `c8a9672..0799098` 同时包含 reaper/audit/probe 外扩面与必要前台修复；逐块保留/删除映射在实施 plan，不按整提交或整个 input 文件回退。

新方案已获得官方 Scene/UI/picking 的临时机制探针证据，包括失败时未挂接产物和缓存 Click 的真实反例、声明图预登记 wrapper 与局部 pointer 过滤的可行性。探针不在产品 runtime 中，不证明 ownership 漂移、完整输入/focus、Windows 或 MVP gate 已通过。

## 最终交付状态

推荐 quick start 与普通内部 outlet 示例已交付，并把原文件作为独立 workspace consumer 编译运行。嵌套示例经真实 UI 布局验证 Push/Replace/Back、history 和 layout/toolbar/slot 保持；它是有限 headless 示例，不是窗口、GPU 或真实设备验收。

2026-09-24 补充 `examples/windowed` 独立 workspace：Winit/UI renderer/font 由示例配置，不扩展库 feature。Linux/WSLg 软件渲染窗口经 OS 注入键鼠验证导航、布局计数保留、fallback/恢复、focus 释放和旧手势撤权；不等同于物理 GPU 或硬件设备验证。六包添加发布元数据和内部版本，标准 Cargo 打包及仅消费解压包的 BSN journey 通过；当前按用户选择暂不添加许可证或正式发布。

此前审查复现的三处接收端缺陷已修正并通过增量复审：混合 Router 诊断与去重共同维护 exact 保留关系；post-commit terminal 冻结原 attempt 结果；native 隔离预期只更新实际写入角色，严格保留 authored 内容观察。私有 fixture 减法的独立审查、完整 Core/ECS/facade/workspace 同版本验证及正常/fallback/bound-error 的普通更新后公开快照与单次终态均通过。本地 main 快进到 `7a34fcc`，合并后公开旅程与独立 consumer smoke 通过。后续局部重构见实施 plan；Windows 原生、最低 Rust 执行、物理 GPU 加速及真实硬件输入仍为 NOT_RUN。
