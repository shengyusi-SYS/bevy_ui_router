# 功能开发入口

[中文](README.zh-CN.md) | [English](README.md)

从下面的功能地图找到相关合同、实现和测试。系统关系见[架构总览](../architecture/README.zh-CN.md)，执行顺序见[运行机制](../architecture/runtime.zh-CN.md)，完整命令见[验证指南](verification.zh-CN.md)。

## 按改动定位

| 开发主题 | 行为说明 | 主要实现 | 验证入口 |
| --- | --- | --- | --- |
| 声明安装、初始页面与 Commands 导航 | [集成指南](../guides/integration.zh-CN.md)、[导航合同](../contracts/navigation.zh-CN.md) | ECS [声明](../../crates/bevy_ui_router_ecs/src/bsn/declaration.rs)、[安装](../../crates/bevy_ui_router_ecs/src/bsn/install.rs)、[Commands](../../crates/bevy_ui_router_ecs/src/navigation_commands.rs) | [公开使用旅程](../../crates/bevy_ui_router/tests/public_usage.rs)、[声明安装测试](../../crates/bevy_ui_router_ecs/src/bsn/install/tests.rs) |
| 路径匹配、参数、query/fragment、共同前缀复用 | [导航](../contracts/navigation.zh-CN.md#匹配与-location-更新) | Core [matcher](../../crates/bevy_ui_router_core/src/matcher.rs)、[branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs) | Core 测试、[公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs) |
| push/replace/back、history、快照 | [导航与观察](../contracts/navigation.zh-CN.md) | Core [engine](../../crates/bevy_ui_router_core/src/engine.rs)、[history](../../crates/bevy_ui_router_core/src/history.rs)，ECS [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs)、[snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs) | [公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs)，包括重建后的嵌套 Back |
| 布局与页面保留、容量、LRU | [实例保留](../contracts/navigation.zh-CN.md#实例保留)、[保留实例生命周期](../contracts/lifecycle.zh-CN.md#非活动实例与长期-owner) | Core [retained](../../crates/bevy_ui_router_core/src/engine/retained.rs)，ECS [retained](../../crates/bevy_ui_router_ecs/src/retained.rs)、[retained miss](../../crates/bevy_ui_router_ecs/src/navigation/retained_miss.rs) | [Core 保留测试](../../crates/bevy_ui_router_core/src/integration_retained_tests.rs)、[公开保留旅程](../../crates/bevy_ui_router/tests/retained_navigation.rs) |
| 按访问恢复滚动 | [滚动恢复](../contracts/navigation.zh-CN.md#按访问恢复滚动) | Core [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs)，ECS [scroll](../../crates/bevy_ui_router_ecs/src/scroll.rs) | [真实布局与滚动旅程](../../crates/bevy_ui_router/tests/scroll_restoration.rs) |
| 页面系统活动资格与当前输入 | [页面活动](../contracts/input-focus.zh-CN.md#页面活动查询)、[导航](../contracts/navigation.zh-CN.md) | ECS [activity](../../crates/bevy_ui_router_ecs/src/activity.rs)、[route inputs](../../crates/bevy_ui_router_ecs/src/route_inputs.rs) | [活动旅程](../../crates/bevy_ui_router/tests/public_activity.rs)、[独立 BSN 应用](../../crates/bevy_ui_router/tests/consumers/bsn.rs) |
| Scene factory、插件装配、default outlet | [Scene 与挂载](../contracts/scene.zh-CN.md) | [BSN](../../crates/bevy_ui_router_bsn/src/lib.rs)、[ECS bridge](../../crates/bevy_ui_router_ecs/src/bsn.rs)、[native Scene](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs)、[mount](../../crates/bevy_ui_router_ecs/src/mount.rs) | [BSN facade 测试](../../crates/bevy_ui_router/tests/bsn_facade_features.rs)、[独立 BSN 应用](../../crates/bevy_ui_router/tests/consumers/bsn.rs) |
| host、身份、发布、失败恢复与清理 | [生命周期合同](../contracts/lifecycle.zh-CN.md)、[执行协议](../architecture/lifecycle.zh-CN.md) | Core [integration](../../crates/bevy_ui_router_core/src/integration.rs)，ECS [registry](../../crates/bevy_ui_router_ecs/src/registry.rs)、[navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs)、[cleanup](../../crates/bevy_ui_router_ecs/src/cleanup.rs) | 完整受影响 crate、[native 回归](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs)、公开旅程和独立应用 |
| 桌面鼠标与 Android 传统返回 | [窗口返回](../contracts/input-focus.zh-CN.md#窗口级返回输入) | Platform [back input](../../crates/bevy_ui_router_platform/src/back_input.rs) 和两个输入源，应用的返回处理器 | [返回旅程](../../crates/bevy_ui_router/tests/platform_back.rs)、最小依赖应用、窗口示例编译 |
| pointer、keyboard/gamepad gate、focus | [输入与焦点](../contracts/input-focus.zh-CN.md)、[调度](../architecture/runtime.zh-CN.md#bevy-调度与输入) | ECS [input](../../crates/bevy_ui_router_ecs/src/input.rs)、[native focus](../../crates/bevy_ui_router_ecs/src/native_focus.rs)、[plugin](../../crates/bevy_ui_router_ecs/src/plugin.rs) | [原生 pointer](../../crates/bevy_ui_router/tests/bsn_native_input.rs)、[原生 focus](../../crates/bevy_ui_router/tests/bsn_native_focus.rs)、窗口交互 |
| facade、features、依赖、示例与包内容 | [共同边界](../contracts/README.zh-CN.md#共同职责边界)、[Scene 注册](../contracts/scene.zh-CN.md#注册与运行装配) | [facade](../../crates/bevy_ui_router/src/lib.rs)、[manifest](../../crates/bevy_ui_router/Cargo.toml)、[App extension](../../crates/bevy_ui_router_ecs/src/app_ext.rs) | [独立应用](../../crates/bevy_ui_router/tests/facade_consumers.rs)、[依赖检查](../../crates/bevy_ui_router/tests/dependency_contract.rs)、示例及 Cargo 打包验证 |
| 路由过渡 | [过渡指南](../guides/transitions.zh-CN.md)、[生命周期](../contracts/lifecycle.zh-CN.md) | Core [exit](../../crates/bevy_ui_router_core/src/engine/exit.rs)、ECS [transition](../../crates/bevy_ui_router_ecs/src/transition.rs)、[Tween 适配](../../crates/bevy_ui_router_ecs/src/transition/tween.rs) 与 [退出处置](../../crates/bevy_ui_router_ecs/src/navigation/deferred_exit.rs) | [公开过渡旅程](../../crates/bevy_ui_router/tests/route_transitions.rs)、[Tween 适配测试](../../crates/bevy_ui_router_ecs/src/transition/tween/tests.rs)、真实 picking 与窗口效果 |
| 性能 | [性能指南](../performance.zh-CN.md)与对应功能合同 | 根据 profile 定位实际热点 | 同机 benchmark 比较和对应行为测试 |

跨层改动沿着行为读取相关主题。例如，导航复用会影响 Scene 重建、frame 身份和焦点；outlet 变动会影响挂载与失败清理。测试选择见[按变更风险选择检查](verification.zh-CN.md#按变更风险选择检查)。

## 新增功能与文档演进

从用户问题、期望行为和当前实现缺口开始，说明需要调整的模块和合同。复杂功能可放在 `docs/changes/<feature>/design.md` 与 `plan.md`：设计说明行为、边界和理由，计划安排交付顺序、检查和审查。简单修复直接更新相关代码、测试和文档。

未完成的设计在页首标注状态和影响范围，并链接当前合同。当前推进的主题见[路线图](../roadmap.zh-CN.md)。输入扩展等设计取舍见[工程经验](design-lessons.zh-CN.md)。

功能完成后，将内容更新到对应位置：

| 内容 | 维护位置 |
| --- | --- |
| 产品能力与适用场景 | [产品概览](../overview.zh-CN.md) |
| 用户可见行为与跨层约定 | [专题合同](../contracts/README.zh-CN.md) |
| 模块关系与执行机制 | [架构](../architecture/README.zh-CN.md) |
| 装配步骤、交互和排障 | [集成](../guides/integration.zh-CN.md)与[调试](../guides/debugging.zh-CN.md) |
| 检查命令和平台记录 | [验证指南](verification.zh-CN.md) |
| 可复用的原因与取舍 | [工程经验](design-lessons.zh-CN.md) |

每条规则在所属主题维护，中英文同步更新。已完成计划和过程日志通过 Git 历史查询，让默认文档树聚焦当前行为和正在开展的工作。

## 修改前后的核对

修改前跟踪真实调用路径，找出覆盖相关用户行为的测试。修改后检查受影响的完整操作链：例如一次导航应包括请求入队、页面显示、离场清理和后续导航。对失败路径，同时检查当前内容、输入/焦点及请求最终状态。

运行相称的格式、编译和测试检查；文档修改执行链接检查，命令见[验证指南](verification.zh-CN.md)。报告使用的命令、feature、源码状态和环境，并注明尚待执行的项目。共享工作区和提交纪律见 [AGENTS.md](../../AGENTS.md)。
