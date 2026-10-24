# 输入与焦点合同

本页定义应用消费输入的边界、非活动内容撤权和焦点操作顺序。调度与执行位置见[运行机制](../architecture/runtime.md#bevy-调度与输入)，source owner 与发布边界见[生命周期](lifecycle.md)。

## 输入资格、pointer 与 focus

标准 UI pointer 使用原生 picking；keyboard/gamepad 的 target-addressed 集成显式消费 `RouterInputGate`。应用在 `PreUpdate` 的 `RouterInputSet::Ready` 后消费输入，在 `Refresh` 前提交动态拓扑变动。

Refresh 根据 active roots 的当前 frame-local Children closure 计算权限，下一 Ready 边界接纳新增内容、撤销移出内容。无 World 的 gate 是该边界快照，不是任意时刻实时 ancestry 查询。

原生 UI 输入资格以 `ComputedNode` 和 authored visibility 识别可交互内容，不要求 `Node`；例如 `ScrollbarThumb` 的几何由控件计算，不参与普通布局。所属 scope、发布和非活动撤权规则相同。

Router 生命周期切换在同次 publication/隔离内撤权，包括 active→retained，不能等下一帧。hidden、pending、leaving、failed、retained/residual 内容不保留集成输入或 active focus；其它有效目标不被无理由清除。

原生 pointer 撤权同时清除目标的当前/前次 hover、press/click/drag 和祖先缓存，保留其它目标状态。在真实 hover 生成后、dispatch 前过滤非活动 owned scope；不能只依赖 Display、最新命中或修改 Pickable，也不能全局清空 pointer 状态。

原生 `TabNavigationPlugin`/`InputDispatchPlugin` 不会因 Router 安装而自动遵守 gate；使用它们须另接目标过滤。手动触发事件、绕过 gate 读原始设备输入或直接改 World 属于可信应用代码责任。

pending Scene 的 `InputFocus::set`/AutoFocus 只形成候选，不授予权限。只保留最后未被后发操作覆盖的候选；原 owner 成功发布且目标仍属于该 source 的可获焦 active 内容时，同次 publication 兑现。

后发合法 set/clear 优先，包括同值 set、已空时 clear；后发 pending 候选也替代前一个。候选失败、stale、取消、隐藏或移出 source 时丢弃，不复活早先候选，不重放整段 pending focus 历史。

无显式意图时，不自动选择首控件或恢复历史焦点；缓存重新激活也不恢复旧焦点。无操作的可变借用不是 focus 操作。保持应用原 focus queue、cause 与延后事件顺序；不能向从未 active 的 pending/failed 目标派发延后 FocusGained/Lost，也不能仅 clear focus 就宣称派发已隔离。

## 页面活动查询

facade/prelude 的 `RouteActivity` SystemParam 提供 `is_active(Entity)`，用于应用决定页面 timer、Query 或其他后台工作是否执行。它读取已发布 active assignment、最近已登记边界的 exact claim、同一 host registration incarnation 和当前内容挂载/containment；cached、pending、leaving、failed、quarantine 与无关 app-owned host 返回 false，不能借活动父布局获得资格。

`RouteInputs::current(Entity)` 与 `RouteActivity` 共用 exact active-frame 解析，返回该 frame 的借用 `RouteInputRef`；不存在有效活动归属时返回 None。两者还核对 Core 为 Ready 且 exact frame 仍在逻辑 active 集合中，不把尚未清理的物理内容当作活动页面。遇到最近已登记的非活动或失效边界即拒绝，不继续向 active 祖先借资格；其它 Router 的有效内容按自身 scope 读取。

普通原生页面后代和非交互 Entity 也可查询，目标无需附加路由组件。两个参数只读取 Router 元数据与相关层级组件，可与应用自己的 `Query<(Entity, &mut T)>` 共用；本轮 publication 后的活动状态与输入可在 `RouterScheduleSet::PublishSnapshots` 之后读取。输入引用仅在 SystemParam 借用期间有效，跨 World mutation 或 await 前须复制普通数据；不提供 lifecycle 或 publication 权限。字段和发布语义见[导航合同](navigation.md#匹配与-location-更新)。

activity 独立于 authored visibility 和输入资格，也不是完整 frame-local 内容健康检查。Router 不会因隐藏缓存而暂停应用系统、observer 或任意副作用；应用选择是否执行自己的工作。pointer/keyboard/gamepad 仍遵守上面的输入边界，不以 activity 查询替代 gate。

实现见 [input](../../crates/bevy_ui_router_ecs/src/input.rs)、[native focus](../../crates/bevy_ui_router_ecs/src/native_focus.rs)、[activity](../../crates/bevy_ui_router_ecs/src/activity.rs)、[当前输入](../../crates/bevy_ui_router_ecs/src/route_inputs.rs) 与 [公开调度](../../crates/bevy_ui_router_ecs/src/plugin.rs)。
