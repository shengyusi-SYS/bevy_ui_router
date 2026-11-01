# 添加过渡动画

[English](transitions.en.md) | 简体中文

给已有的 Bevy UI 页面加上切换动画。Router 负责让进入、退出页面在合适的时间存在，应用决定效果、时长和缓动。下面先使用 Tween 适配；已有动画系统可以直接接入后半部分的自定义驱动器。

## 1. 添加依赖和插件

在现有 `bevy_ui_router` 依赖上启用 `tween`，即 `features = ["bsn", "tween"]`，并添加：

```toml
[dependencies.bevy_tween]
version = "0.13"
default-features = false
```

沿用入门教程的导入，在 `add_bsn_router` 之前安装插件：

```rust
app.add_plugins(RouteTweenPlugin::default());
```

教程中的 `DefaultPlugins` 已包含动画需要的时间插件。

## 2. 连接路由并定义效果

在已有 `BsnRouter` 的声明链上，加上 `.transition_driver(RouteTweenDriver::new(prepare_effect))`，再实现下面的 `prepare_effect`。它让新页面在 240 毫秒内从右侧滑入。

`surface()` 返回 Router 为页面提供的外层动画节点。这里只修改它已有的 `UiTransform`；`is_outermost()` 避免父子页面重复移动。

```rust
use std::time::Duration;
use bevy_tween::{combinator::tween, prelude::*};

fn prepare_effect(
    world: &mut World,
    context: &RouteTransitionContext,
    tracks: &mut RouteTweenTracks,
) {
    for target in context.entering().iter().filter(|target| target.is_outermost()) {
        let surface = target.surface();
        world.get_mut::<UiTransform>(surface).unwrap().translation = Val2::px(24.0, 0.0);
        tracks.enter(tween(
            Duration::from_millis(240),
            EaseKind::CubicOut,
            surface.into_target().with_closure(|value: &mut UiTransform, progress, _| {
                value.translation = Val2::px(24.0 * (1.0 - progress), 0.0);
            }),
        ));
    }
}
```

`tracks.enter(...)` 播放进入动画；需要退出动画时，使用 `tracks.exit(...)`。两者可组合 `tween`、`sequence`、`parallel` 等有限时间线。退出页面的动画都放进 `exit`，库会等它们结束后再隐藏或清理页面。没有添加动画时直接切页。

适配器负责计时、取消、结束和按需窗口的重绘。动画可以修改外层节点已有的 `UiTransform`、`ZIndex` 和 `Node` 布局字段（不含 `display`）；外层节点的组件、父子关系、可见性和输入状态仍由 Router 管理。

## 3. 运行并调整效果

从仓库根目录进入窗口示例目录：

```sh
cd examples/windowed
cargo run --features tween --bin tween_transitions
```

点击页面按钮切换，点击 `Effect: …` 切换效果、`Exit: …` 开关退出动画，修改后在下一次导航生效。效果代码在 [effects.rs](../../examples/windowed/src/bin/tween_transitions/effects.rs)，完整接入在[示例入口](../../examples/windowed/src/bin/tween_transitions.rs)。

<details>
<summary>更多插值类型、自选插件与执行顺序</summary>

适配器支持 `EaseKind` / `EaseClosure` 和 boxed 组件插值，默认注册 `UiTransform`、`Node`、`ZIndex`。应用内容的其它组件可用 `RouteTweenPlugin::default().with_component::<T>()` 注册，通过 `with_closure` 或 boxed interpolator 操作。typed interpolator、resource/asset tween 和 tween event 需要下文的自定义驱动器。

如同时使用 `DefaultTweenPlugins`，先安装 `RouteTweenPlugin`，让上游 debug 插件识别路由动画的独立时钟。反过来安装不影响计时，但可能出现未知时间上下文的误报。自选 Bevy 插件时需提供 `TimePlugin`。

动画在页面切换后从零开始，使用 `Time<Real>`；在 UI Prepare 前应用插值，最后一次写入后回收控制器。取消会同步停止写入。按需更新的窗口在动画期间及提交完成请求时会收到重绘请求。需要自定义时钟或完成策略时，使用下文的驱动器接口。

</details>

## 接入已有动画系统

实现 `RouteTransitionDriver` 即可连接自己的动画系统。可以先在 `examples/windowed` 目录运行不依赖 Tween 的示例：

```sh
cargo run --bin transitions
```

[窗口示例](../../examples/windowed/src/bin/transitions.rs)使用这个接口实现平移和缩放旋转。按钮和键盘操作相同：

| 按键 | 操作 |
| --- | --- |
| `1` / `2` | 进入保留状态的 Notes / Gallery 页面 |
| `3` | 进入每次返回都重新创建的 Fresh 页面 |
| `B` | Back |
| `E` | 切换下一次过渡的效果 |
| `H` | 切换是否持有退出页面；关闭时仅播放进入动画 |
| `R` | 连续发出导航，观察旧动画取消 |

点击页面计数器后切换页面，可比较保留与重新创建的行为。[动画实现](../../examples/windowed/src/bin/transitions/animation.rs)使用官方 `Time<Real>`、`Timer`、`EasingCurve` 和 `UiTransform`；无需额外动画库。窗口运行环境见[应用集成](integration.md#运行仓库示例)。

### 注册驱动器

为 Router 注册驱动器：

```rust
let declaration = BsnRouter::new(root)
    .fallback(error_page)
    .transition_driver(MyTransitionDriver);
let router = app.add_bsn_router(declaration)?;
```

`MyTransitionDriver` 实现 `RouteTransitionDriver`。回调同步运行，可使用 `&mut World` 访问应用资源：

| 回调 | 应用职责 |
| --- | --- |
| `prepare(world, context)` | 设置首帧样式并记录动画状态，此时不要启动计时。返回 `Skip` 或 `Run { hold_exit }`。 |
| `presented(world, id)` | 页面已切换，启动应用动画。 |
| `cancelled(world, id, reason)` | 在返回前停止本次动画对组件的全部写入，并清理应用动画状态。 |

每次调用 `prepare` 后都会收到 `presented` 或 `cancelled`；已经开始的动画也可能随后取消。`Skip` 会触发 `cancelled(..., Skipped)`，以便撤销准备状态。其它原因包括 `PreparationAborted`、`Superseded` 和 `AuthorityLost`。取消已发布的动画不会撤销已提交的路由与 history。

`RouteTransitionContext` 提供本次动画的 `id()`、Router、导航请求、`NavigationKind` 与前后地址 `from()` / `to()`。用 `entering()` 和 `leaving()` 取得进入、退出的目标；`preserved()` 中继续共用的页面仅供读取。目标的 `frame()` 标识路由实例，`content()` 是应用内容根，`surface()` 是外层动画节点。整页效果通常只处理 `is_outermost()`，避免父子页面重复变换。

### 保存动画状态与处理取消

页面重叠布局在 `prepare` 中设置。

把计时器、动画组件或第三方动画控制器放在应用资源或独立 Entity 上，以外层节点为目标。也可以直接动画化自己的 `content()`。

`cancelled` 返回前应停止本次动画的所有写入。回调排队的 Commands 会在 Router 继续之前执行。结束或取消时，Router 恢复仍由它管理的外层节点原值，并保留当前的 `display`；之后不再继续修改这些节点。

示例把唯一动画写入者保存在资源中，在 `cancelled` 立即移除；每帧系统排在 `Update` 的 `RouterScheduleSet::PublishSnapshots` 之后，用正常 Bevy 组件写入推进效果。

使用 winit 按需更新模式时，在动画运行期间发送 `RequestRedraw`，让窗口持续更新。

### 退出动画结束后释放页面

`Run { hold_exit: true }` 让退出页面保持可见，延后它原定的清理或隐藏保留。退出页面已经非活动，不能继续接收路由输入或保留焦点。`Run { hold_exit: false }` 不等待退出动画，适合初次进入或仅播放进入动画；返回后不要继续修改退出页面的外层节点。

先停止对应目标的动画写入，再发送命令：

```rust
// 退出动画已停止；进入动画仍可继续。
commands.release_route_transition_exit(session_id.clone());

// 全部动画写入已停止；也释放尚未释放的退出页面。
commands.finish_route_transition(session_id);
```

用本次动画的 `RouteTransitionId` 发送命令：退出动画结束时释放页面，全部动画结束后完成过渡。

<details>
<summary>连续导航与请求完成时间</summary>

活动页面、当前参数与历史记录在页面切换时更新，不必等待动画结束。持有退出页面时，请求可处于 `PostCommitCleanup`；退出页面的清理或保留完成后，请求即可结束，进入动画仍可继续。只有进入动画时，不延长导航请求。

后续请求若正在等待旧退出动画，Router 可以先取消旧动画并处理退出页面，再推进队列。没有实际页面进入或退出的导航不会新建动画，已有的只进入动画可以继续。

</details>
