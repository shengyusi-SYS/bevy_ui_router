# 输入与焦点合同

[中文](input-focus.zh-CN.md) | [English](input-focus.md)

Router 让已发布页面接收输入，并在内容离场时撤销手势与焦点。应用的输入处理、页面后台工作和窗口返回分别使用下列接口。执行顺序见[运行机制](../architecture/runtime.zh-CN.md#bevy-调度与输入)，发布规则见[生命周期](lifecycle.zh-CN.md)。

## 输入资格、pointer 与 focus

标准 UI pointer 使用原生 picking；目标定向 keyboard/gamepad handler 在 `PreUpdate` 的 `RouterInputSet::Ready` 后读取 `RouterInputGate`。应用在 `Refresh` 前提交动态拓扑变化，新增和移出内容会在下一 Ready 边界更新资格。gate 保存该边界的权限快照。

原生 UI 资格根据 `ComputedNode` 和 authored visibility 判断，允许几何由控件计算、没有 `Node` 的内容，例如 `ScrollbarThumb`。所属 scope、发布和非活动撤权规则保持一致。

生命周期切换在同次 publication/隔离内撤权，包括 active→retained。hidden、pending、leaving、failed、retained/residual 内容退出集成输入和 active focus；其它有效目标继续工作。

pointer 撤权清除目标的当前/前次 hover、press/click/drag 和祖先缓存，保留其它目标状态。真实 hover 生成后、dispatch 前再次过滤非活动 owned scope，因此已开始的旧手势也随离场取消。

原生 `TabNavigationPlugin`/`InputDispatchPlugin` 的目标过滤需要应用接入。应用自定义派发、原始设备读取和直接 World 修改也由应用安排。

pending Scene 的 `InputFocus::set`/AutoFocus 保存为候选。原 owner 成功发布且目标仍属于该 source 的可获焦 active 内容时，同次 publication 兑现最后有效候选。

后发合法 set/clear 优先，包括同值 set、已空时 clear；后发 pending 候选也替代前一个。候选失败、stale、取消、隐藏或移出 source 时丢弃。无显式意图时保持现有焦点策略，重新激活缓存也等待新意图。无操作的可变借用不算一次 focus 操作。

焦点处理保留应用原 queue、cause 和延后事件顺序；从未 active 的 pending/failed 目标被排除在延后 FocusGained/Lost 派发之外。

## 页面活动查询

`RouteQueries::read()` 返回借用的 `RouteQueryReader`。遍历页面 Entity 前创建一次读取器，让活动、当前输入和待恢复滚动查询在其中共享归属与层级观察。

- `is_active(Entity)` 帮助应用决定页面 timer、查询或后台工作是否执行。
- `current(Entity)` 返回借用的 `RouteInputRef`，无有效活动归属时返回 None，已包含活动资格检查。
- `pending(Entity, key)` 返回活动 frame 下对应 key 的未完成滚动恢复，容器尚未存在时也可查询。详见[滚动恢复](navigation.zh-CN.md#按访问恢复滚动)。

共享 resolver 核对已发布 active assignment、最近登记边界的 exact claim、host registration incarnation 和当前 containment。Core 必须为 Ready，exact frame 必须在逻辑 active 集合中。最近的已登记边界失效或非活动时直接拒绝；cached、pending、leaving、failed、quarantine 及无关应用 host 没有活动结果。其它 Router 的有效内容按自己的 scope 查询。

普通页面后代和非交互 Entity 无需额外路由组件。`RouteQueries` 只读 Router 元数据和相关层级，可与应用 `Query<(Entity, &mut T)>` 及 Commands 共用。在 `RouterScheduleSet::PublishSnapshots` 后可读取本轮发布结果。观察事实只在读取器的不可变借用期内共享；World mutation 后再次 `read()` 会读取新事实，同 tick 也如此。输入引用借用读取器，跨 World mutation 或 await 前复制所需普通数据。字段含义见[导航合同](navigation.zh-CN.md#匹配与-location-更新)。

`RouteActivity::is_active`、`RouteInputs::current` 和 `RouteScrollRestorations::pending` 仍通过同一 resolver 提供；循环或混合查询使用一个 `RouteQueryReader`。

activity 与可见性、输入资格分别判断。应用通过它控制自己的系统和后台工作；输入处理仍使用 gate。隐藏缓存中的系统、observer 和其他应用逻辑继续按应用自身调度执行。

实现见[公开查询](../../crates/bevy_ui_router_ecs/src/route_queries.rs)、[input](../../crates/bevy_ui_router_ecs/src/input.rs)、[native focus](../../crates/bevy_ui_router_ecs/src/native_focus.rs) 和[调度](../../crates/bevy_ui_router_ecs/src/plugin.rs)。

## 窗口级返回输入

facade 或 Platform 的 `desktop-back` / `android-back` features 提供 `DesktopBackInputPlugin` / `AndroidBackInputPlugin`。两者在 `PreUpdate`、Bevy `InputSystems` 之后的 `PlatformBackInputSet` 中产生 `PlatformBackRequested { window }`。

| 来源 | 转换为返回消息的输入 |
| --- | --- |
| 桌面鼠标 | `MouseButtonInput` 的 `Back + Pressed` |
| Android 传统按键 | `KeyboardInput` 逻辑键 `BrowserBack` / `GoBack`，`Pressed` 且 `repeat == false` |

每个来源保留原窗口与自身消息顺序；多个来源之间使用实际调度顺序。Released、其它键和 Android 重复按键被忽略，原始输入继续供其它 reader 使用。

应用用一个处理器选择 window→RouterHandle 映射、本地 UI 优先级和无 history 时的动作，再调用 `RouterNavigation::back(router)` 处理入队结果。返回属于窗口级意图，可以在任意鼠标位置触发。`BackPolicy` 等类型供应用表达策略；实际决策由这个处理器执行。

推荐把处理器放在同一 `PreUpdate` 的 `.after(PlatformBackInputSet)`，让获准的返回进入后续 `Update` 导航。`MessageReader` 各自有游标，因此本地处理和 Router 派发应放在同一条决策路径。应用保存安装得到的 handle 并明确关联窗口。`DesktopBackRequested` / `DesktopBackInputSet` 是共享类型的兼容名称。

插件幂等注册原始与返回消息通道。设备输入由应用已有的 Bevy 后端提供，也可注入标准消息测试。各 feature 只启用所需 mouse 或 keyboard 输入；应用自行安装 Router ECS、Scene、窗口和平台运行插件。

Android adapter 使用传统按键消息；现代 callback 与预测返回手势属于后续能力。Bevy 消息中没有 Android 取消标志，应用需明确处理无 history 时的退出策略。Android manifest 兼容开关见[系统返回指南](../guides/platform-back.zh-CN.md#android-传统返回)。
