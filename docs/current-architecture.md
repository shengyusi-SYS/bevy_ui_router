# Current Architecture

**类型**：实现事实与差距。**更新时间**：2026-09-12。**runtime 迁移**：从 `sys/structural-governance` 的 `0799098` 前向精简，后台 reaper 与公开 audit/probe 已退出；生产 BSN 迁移尚未完成。

范围与验收以 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md) 为准，后续工作见 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## 当前事实

`Cargo.lock` 已跟踪，直接 Bevy 依赖使用官方 crates.io `0.19.1`。依赖升级和历史验证不证明 BSN MVP 已交付。

| 层 | 已有实现 | 新 MVP 差距 |
| --- | --- | --- |
| Core | builder/matching、请求顺序、branch diff、frame/generation、history、recovery 和唯一逻辑 publication | 保留既有行为；仅按新证据修正，不整体重写 |
| View | renderer-neutral key、registry、descriptor、effect/input 合同 | 清除旧公开 writer/report 耦合时保持独立性 |
| ECS | host/slot/frame binding、data-only driver、manifest、staging、cleanup、输入/focus、physical authority；私有 native Scene 创建与失败结算 | 将 native 创建结果接入导航，并联动 current containment、输入/focus、publication 与 cleanup |
| BSN | owned `BsnSceneFactory`、`BsnBuildError`、`DefaultOutlet`；已反转依赖，由 ECS optional bridge 消费 | 公开注册、原生 realization 与导航闭环尚未启用 |
| Facade | 导航、snapshots、旧 driver/journeys；真实独立 Core/View/ECS consumer | 稳定 BSN 入口、真实 BSN consumer、普通嵌套示例及旧 API 退出尚未完成 |

ECS 当前主链仍是 `EcsViewDriver → FrameContentWriter/ContentPlan → RealizedFrame`。其构建时内容名单与 authored UI component 写法不等于 native Scene 的 current containment/样式保留合同。后台 reaper/fairness、公开 audit/probe 和 ECS/facade test-support features 已移除；调度仅公开 `RouterInputSet::{Refresh,Ready}` 与 `RouterScheduleSet::PublishSnapshots`，中间 lifecycle 调度为 crate 私有，必要故障 seam 只在私测中编译。

ECS 的私有 BSN preparation 已调用真实官方 `ScenePatch::load/resolve`：一次 factory 调用消费 fresh Scene，检查 runtime、cached inclusion 和重复引用，准备保持 ChildOf 顺序的声明图及 attempt 唯一匿名引用。成功与失败结果均携带 dependency handles，供后续执行/结算保活。此代码尚未接入导航；`AppBsnExt` 要与能够成功执行的 root realization 同时启用，不注册一个只能返回“未实现”的 driver，也不把 Scene 放进可克隆的 `ContentPlan`/replay artifact。私测的直接 Scene spawn 只验证解析 payload 与声明组件，不证明 Router ownership、publication、输入或 cleanup。

新增私有 `presentation::native_scene` 消费同一 prepared Scene：先批量预备 registry 容量和 seal，再登记 shell 与全部声明 Entity，之后才写私有组件；引用 seed 与 shell wrapper 交由官方 executor 执行。成功 adoption 检查实际关系与其它 live owner，逐次复验临时标记移除产生的变化，退役 descendant 创建期 claims，并返回最后的真实内容闭包快照；该快照不是永久 ownership 名单。失败时只删除有 exact 证明的叶节点，保留 protected closure，并在每次关系/删除写入后复验剩余目标与隔离 shell；合法声明/普通容器交错不再误判为关系环。无法证明隔离时仍返回明确 `Unisolated` 残留证据，不能当成 inert 或 publication 成功。此单元尚未接入前台 owner、mount、输入/focus 或通用 cleanup，不闭合 BSN 产品路径。

原生 pointer 的既有 hover/press/click/drag/祖先缓存与 `PickingInteraction` 已在三个正常 publication 入口及 authority isolation 的同一 exclusive 调用内撤销。正常发布通过 exact resource Entity 联合查询预借资源/组件，新增 pointer tail 只做原位修改；历史目标或资源 Entity 被禁用也不会漏清，正常 presentation 的默认查询过滤不因此放宽。post-commit 的观察 partition 不再被全量用作撤权目标，protected/app 状态保持。`Refresh` 与 `generate_hovermap` 之后、`update_interactions`/`pointer_events` 之前的定点过滤继续防止旧或新非活动命中派发。此接线仍以现有内容名单为基础；native current containment、Scene staging/AutoFocus 与 idle host-loss 的联动仍未闭合。

三个成功发布路径（normal reconciliation、Back target、projectionless fallback）现已将 World preparation 及外层 `RouterNavigation` 资源 scope 的退出放到最终复验之前，再按 exact resource Entity 联合借用常驻 navigation/registry/gate/focus/pointer/presentation。原 active/cleanup owner 在 Core publication 前安装，发布后不再通过资源重新插入补装 owner。Back 最终复验比较完整 leaving 与 RetainEmpty focus 来源；来源漂移沿现有 pending rejection 同调用归还原 active，再由正常调度清理与 fallback，不让临时 owner 存放位置导致下一次 Refresh 误撤有效权限。仅 focus 值变化仍可发布，等价 gate 变化保持原顺序与 tick。此迁移不宣称其它 terminal、cleanup、no-target 或 snapshot 路径的资源边界全部闭合，也不替代 native current-containment 接线。

无 active 的初始 unbound RouterFallback 在发布前失去 host 时，Core 现在接纳原 failed Task source 进入现有 authority settlement，保持 `HostLost` 而非协议错误；有 active 的 fallback 不由此窄分支接纳。Core 保留其合法逻辑 roots 选择，ECS 则提交实际 pending 的完整物理义务。无候选观察也已复用完整 permit outcome 处理，避免丢失 Issued owner 后重新注册永远 Outstanding。上述修复不等于完全 idle 的 host-loss 主动检测已实现。

**待确认的 focus 集成边界**：官方 `InputFocus` 没有直接按 target 过滤待派发变化队列的公开入口。Scene apply 后整体恢复快照虽可抑制 pending `AutoFocus`，也会撤销同次 Scene 对其它有效目标的合法 focus 改写；目前不采用该无条件恢复方案。若选择构建期 focus 变化不生效、应用在 Scene 执行之外显式设置焦点，需要先明确接受这一限制，不把它默认为既有合同。

`QuarantineLeaseTable` 仍承担前台 cleanup/host 无法安全销毁的私有残留 ownership；verified、unresolved、authority-lost 转移、stale projection 和输入/focus 隔离仍使用它。旧残留不持有 terminal 请求的前台 capability，也不会因 blocker 消失自动回收；Entity/metadata 可能长期保留，不能清空 table 冒充完成 cleanup。

facade 合同验证已迁往独立 consumer 与语义依赖检查；ECS 遗漏的旧文档 catalogue 和两项 AST API 清单也已退出，私有源码形状断言仍待阶段 E 合并。现有 consumer 从独立 workspace 运行合法装配，并校验实际私有身份/authority 入口与 session 单次消费；默认 consumer 覆盖 LF/CRLF。`bsn-wiring` 仍使用迁移 renderer，只证明 feature 接线，不是 native Scene 的产品验证。

## 必须保留的行为

- 基础/嵌套路由、稳定遍历与 specificity 冲突诊断；不同 leaf 的最高优先级并列为 `MatchConflict`。
- common-prefix 保留、suffix 替换；search 变化重建整链，fragment-only 推进最深 normal frame generation，等价 location 可零物理 mutation。
- push/replace/back 成功后统一提交 history；`NoBackEntry` 是无 request 的同步拒绝，运行时无目标是 `BackTargetUnavailable`。
- boundary/error/fallback 与 host loss/re-registration；无 binding nested Push 失败 cleanup 后进入 `RouterFallback`，不恢复旧 Terminal 语义。
- exact identity、线性 owner、publication-last、owned-only cleanup、stale 拒绝、非活动输入/focus 隔离和 app-owned host 保持。

以上是已有行为与迁移保留项，不宣称当前旧输入实现已经满足全部 native BSN 输入情形。

**idle host-loss 已闭合**：所有请求终态后的 registration removal/host despawn 会主动处理，包括 Ready 无 active 的绑定；真实 retired grant 建立请求独立的 owner，旧请求/history 保持。重新接纳将原 grant/candidate/staged slot 交给唯一 pending holder，全部 staging/activation 与外层资源重插后再复验实际旧残留、候选和新 slot，以常驻资源借用完成确认；真实拒绝原因与物理清理证据分开保留。两个真实 hook 回归、17 项定向、冻结版本 workspace/feature 验证及独立/交叉复审通过；native Scene 尚未接入，不能将该证据推广为其输入/cleanup 已验收。

## 历史与候选证据

Root、Nested、Reconciliation、History、Recovery 的完成记录见 [历史索引](superpowers/README.md)。Recovery 验证候选为 `494ae6b`；结构治理范围 `211874a..2f2cb42` 保存其行为，历史门禁只证明当时范围。

其后六个候选 `c8a9672..0799098` 同时包含 reaper/audit/probe 外扩面与必要前台修复；逐块保留/删除映射在实施 plan，不按整提交或整个 input 文件回退。

新方案已获得官方 Scene/UI/picking 的临时机制探针证据，包括失败时未挂接产物和缓存 Click 的真实反例、声明图预登记 wrapper 与局部 pointer 过滤的可行性。探针不在产品 runtime 中，不证明 ownership 漂移、完整输入/focus、Windows 或 MVP gate 已通过。

## 尚未完成

生产 BSN root/nested/error 路径、旧 renderer API 退出、独立 BSN consumer、MVP 全量验证、最终审查和 main 集成均仍需完成。旧阶段 PASS、规范修订或注册类型存在不能替代这些结果。
