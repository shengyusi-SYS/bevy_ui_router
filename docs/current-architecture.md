# Current Architecture

**类型**：实现事实与差距。**更新时间**：2026-09-07。**runtime 基线**：`sys/structural-governance` 的 `0799098`；后续合同和 consumer 迁移不代表 runtime 已完成迁移。

范围与验收以 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md) 为准，后续工作见 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## 当前事实

`Cargo.lock` 已跟踪，直接 Bevy 依赖使用官方 crates.io `0.19.1`。依赖升级和历史验证不证明 BSN MVP 已交付。

| 层 | 已有实现 | 新 MVP 差距 |
| --- | --- | --- |
| Core | builder/matching、请求顺序、branch diff、frame/generation、history、recovery 和唯一逻辑 publication | 保留既有行为；仅按新证据修正，不整体重写 |
| View | renderer-neutral key、registry、descriptor、effect/input 合同 | 清除旧公开 writer/report 耦合时保持独立性 |
| ECS | host/slot/frame binding、data-only driver、manifest、staging、cleanup、输入/focus、physical authority | 迁移到 Scene root/current containment，接入创建期责任、真实输入撤权与 BSN bridge |
| BSN | 空 prelude，当前仍依赖 ECS | 尚无 factory/Scene executor；须反转依赖并实现第一种生产 adapter |
| Facade | 导航、snapshots、旧 driver/journeys；真实独立 Core/View/ECS consumer | 稳定 BSN 入口、真实 BSN consumer、普通嵌套示例及旧 API 退出尚未完成 |

ECS 当前主链仍是 `EcsViewDriver → FrameContentWriter/ContentPlan → RealizedFrame`。其构建时内容名单与 authored UI component 写法不等于 native Scene 的 current containment/样式保留合同。公开中间调度、audit/probe、reaper 仍存在，尚未精简。

测试约束已迁往独立 consumer 与语义依赖检查，不再由 Markdown/Golden/AST 清单证明公共 API。现有 consumer 从独立 workspace 运行合法装配，并校验实际私有身份/authority 入口与 session 单次消费；默认 consumer 覆盖 LF/CRLF。`bsn-wiring` 仍使用迁移 renderer，只证明 feature 接线，不是 native Scene 的产品验证。

## 必须保留的行为

- 基础/嵌套路由、稳定遍历与 specificity 冲突诊断；不同 leaf 的最高优先级并列为 `MatchConflict`。
- common-prefix 保留、suffix 替换；search 变化重建整链，fragment-only 推进最深 normal frame generation，等价 location 可零物理 mutation。
- push/replace/back 成功后统一提交 history；`NoBackEntry` 是无 request 的同步拒绝，运行时无目标是 `BackTargetUnavailable`。
- boundary/error/fallback 与 host loss/re-registration；无 binding nested Push 失败 cleanup 后进入 `RouterFallback`，不恢复旧 Terminal 语义。
- exact identity、线性 owner、publication-last、owned-only cleanup、stale 拒绝、非活动输入/focus 隔离和 app-owned host 保持。

以上是已有行为与迁移保留项，不宣称当前旧输入实现已经满足全部 native BSN 输入情形。

## 历史与候选证据

Root、Nested、Reconciliation、History、Recovery 的完成记录见 [历史索引](superpowers/README.md)。Recovery 验证候选为 `494ae6b`；结构治理范围 `211874a..2f2cb42` 保存其行为，历史门禁只证明当时范围。

其后六个候选 `c8a9672..0799098` 同时包含 reaper/audit/probe 外扩面与必要前台修复；逐块保留/删除映射在实施 plan，不按整提交或整个 input 文件回退。

新方案已获得官方 Scene/UI/picking 的临时机制探针证据，包括失败时未挂接产物和缓存 Click 的真实反例、声明图预登记 wrapper 与局部 pointer 过滤的可行性。探针不在产品 runtime 中，不证明 ownership 漂移、完整输入/focus、Windows 或 MVP gate 已通过。

## 尚未完成

候选精简、生产 BSN root/nested/error 路径、旧 API 退出、独立 BSN consumer、MVP 全量验证、最终审查和 main 集成均仍需完成。旧阶段 PASS、规范修订或注册类型存在不能替代这些结果。
