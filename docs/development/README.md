# 功能开发入口

已有行为从[专题合同](../contracts/README.md)查询，模块关系见[架构总览](../architecture/README.md)，执行顺序见[运行机制](../architecture/runtime.md)，owner 交接及失败分流见[生命周期执行协议](../architecture/lifecycle.md)。本页帮助按改动范围进入相关上下文；完整检查命令归[验证指南](verification.md)。

## 按改动定位

| 开发主题 | 先读合同 | 主要实现 | 行为验证入口 |
| --- | --- | --- | --- |
| 匹配、params/search/fragment、prefix 复用 | [导航](../contracts/navigation.md#匹配与-location-更新) | Core [matcher](../../crates/bevy_ui_router_core/src/matcher.rs)、[branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs) | Core 回归与[公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs) |
| push/replace/back、history、snapshots | [请求/history](../contracts/navigation.md#请求history-与-presentation)、[观察](../contracts/navigation.md#snapshots-与诊断) | Core [engine](../../crates/bevy_ui_router_core/src/engine.rs)、[history](../../crates/bevy_ui_router_core/src/history.rs)，ECS [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs)、[snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs) | [公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs)，包含重建后的嵌套 Back |
| 父子独立 keep-alive、依赖/LRU、库存失效 | [保留范围](../contracts/navigation.md#实例保留)、[长期 owner](../contracts/lifecycle.md#非活动实例与长期-owner) | Core [retained](../../crates/bevy_ui_router_core/src/engine/retained.rs)，ECS [retained](../../crates/bevy_ui_router_ecs/src/retained.rs)、[retained miss](../../crates/bevy_ui_router_ecs/src/navigation/retained_miss.rs) | [Core retained 回归](../../crates/bevy_ui_router_core/src/integration_retained_tests.rs)、[native 回归](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs)、公开导航旅程 |
| entry 滚动恢复 | [滚动](../contracts/navigation.md#按访问恢复滚动) | Core [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs)，ECS [scroll](../../crates/bevy_ui_router_ecs/src/scroll.rs) | [真实布局与组合滚动旅程](../../crates/bevy_ui_router/tests/scroll_restoration.rs) |
| 页面后台工作活动资格 | [RouteActivity](../contracts/input-focus.md#页面活动查询) | ECS [activity](../../crates/bevy_ui_router_ecs/src/activity.rs) | [公开活动旅程](../../crates/bevy_ui_router/tests/public_activity.rs)及[独立 BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs) |
| Scene/factory、运行装配、default outlet | [Scene 与挂载](../contracts/scene.md) | [BSN 合同](../../crates/bevy_ui_router_bsn/src/lib.rs)、[ECS bridge](../../crates/bevy_ui_router_ecs/src/bsn.rs)、[native Scene](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs)、[mount](../../crates/bevy_ui_router_ecs/src/mount.rs) | [BSN facade](../../crates/bevy_ui_router/tests/bsn_facade_features.rs)及[独立 BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs) |
| host、identity、publication、failure/recovery、cleanup | [生命周期与清理](../contracts/lifecycle.md) | Core [integration](../../crates/bevy_ui_router_core/src/integration.rs)、[engine](../../crates/bevy_ui_router_core/src/engine.rs)，ECS [registry](../../crates/bevy_ui_router_ecs/src/registry.rs)、[navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs)、[cleanup](../../crates/bevy_ui_router_ecs/src/cleanup.rs) | ECS 完整 crate、[native 回归](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs)及公开旅程/consumer |
| pointer、keyboard/gamepad gate、focus | [输入与焦点](../contracts/input-focus.md)及[调度](../architecture/runtime.md#bevy-调度与输入) | ECS [input](../../crates/bevy_ui_router_ecs/src/input.rs)、[native focus](../../crates/bevy_ui_router_ecs/src/native_focus.rs)、[plugin](../../crates/bevy_ui_router_ecs/src/plugin.rs) | [原生 pointer](../../crates/bevy_ui_router/tests/bsn_native_input.rs)、[原生 focus](../../crates/bevy_ui_router/tests/bsn_native_focus.rs) |
| facade、features、依赖与示例 | [共同边界](../contracts/README.md#共同职责边界)、[注册](../contracts/scene.md#注册与运行装配) | [facade](../../crates/bevy_ui_router/src/lib.rs)、[manifest](../../crates/bevy_ui_router/Cargo.toml)、[App extension](../../crates/bevy_ui_router_ecs/src/app_ext.rs) | [独立 consumer](../../crates/bevy_ui_router/tests/facade_consumers.rs)、[依赖合同](../../crates/bevy_ui_router/tests/dependency_contract.rs)、实际示例与打包 consumer |
| 性能问题 | [性能基线](../performance.md)与热点所属专题 | 先 profile 对应生产路径，再选择修改 | 同语义 benchmark 对照与相应功能 gate |

这是定位地图，验证要求按变更风险选择。涉及跨层行为时沿依赖阅读：例如改变导航复用会影响 Scene 重建、frame identity 和焦点；改变 outlet 会影响 mount 拒绝与失败清理。无需为每个任务加载所有历史背景。

## 新增功能与文档演进

先说明用户问题、期望行为及当前实现缺口，再决定影响哪些模块和专题。新能力可以改变当前范围，但需要明确替换或扩展哪些合同，不能把旧方案、理论能力或内部实现便利当成需求。

复杂功能可以组织为 `docs/changes/<feature>/design.md` 与 `plan.md`；独立子问题可拆分设计，多个功能不必塞入一份全局 spec。设计说明行为、边界、理由与验收，计划安排交付依赖、验证和审查。简单修复直接维护相关代码与基线，不为模板创建空文档。

当前实例收口专题：[实例协调与当前路由输入](../changes/instance-coordination/design.md)，执行拆分见[计划](../changes/instance-coordination/plan.md)。下一个实施专题为[访问级滚动意图设计](../changes/scroll-intent/design.md)与[计划](../changes/scroll-intent/plan.md)，尚未交付；输入正式接缝从[长期评估](../changes/navigation-experience/assessment.md)进入。

未落地设计在页首标明草案、已确认或实施中及影响范围，链接相关当前合同。方案描述的是待改变部分，不重复抄写整个基线，也不能静默覆盖现行行为。当前没有需要恢复执行的旧 MVP 阶段计划。

交付后按内容归位：

- 产品能力和范围变动更新概览。
- 用户行为与跨层不变量更新所属专题合同。
- 模块关系或执行机制变动更新架构。
- 装配/交互变化更新指南，验证入口变化更新验证文档。
- 可复用原因写入工程经验；测量证据保留环境和适用范围。

已完成计划和过程日志退出默认文档树，由 Git 追溯。保留一个当前答案意味着同一规则有清楚的维护位置，不意味着整个项目只能有一个 spec 或 plan。

## 修改前后的核对

修改前确认实际调用路径与相关回归，区分合同变化和内部重构；需要选择方案时先读[工程经验](design-lessons.md)中对应原因。修改后检验跨主题链接与受影响高层行为，按[验证指南](verification.md#按变更风险选择检查)执行相称检查。

文档里的设计、现有实现和运行证据分别成立；新增设计或绿色局部测试不能自动证明生产入口已接通。协作与提交纪律见 [AGENTS.md](../../AGENTS.md)。
