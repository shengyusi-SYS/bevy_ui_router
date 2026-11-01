# 应用自定义路由过渡

[中文](transitions.md) | [English](transitions.en.md)

Router 默认不安装过渡 driver。应用决定效果、缓动、时长和叠放布局。使用可选 Tween 适配时，库协调控制器的开始、取消与完成；使用自己的动画系统时，实现下文的 `RouteTransitionDriver`。相关类型均从 `bevy_ui_router::prelude::*` 导入。

当前展示表面接入面向原生 Bevy UI / BSN 页面。

## 使用 Tween 适配

启用 Router 的 `bsn,tween` features，并添加 `bevy_tween = { version = "0.13", default-features = false }`。安装 `RouteTweenPlugin::default()`；应用仍提供 `TimePlugin`（`DefaultPlugins` 已包含）。运行完整示例：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features tween --bin tween_transitions
```

[窗口入口](../../examples/windowed/src/bin/tween_transitions.rs)使用统一路由声明、Commands 和保留计数。[应用效果](../../examples/windowed/src/bin/tween_transitions/effects.rs)提供平移、缩放旋转和进入动画模式，不维护 session、计时器或结束回调。 窗口按钮显示当前效果和退出动画模式；点击切换后，下一次导航使用新设置。

在 `BsnRouter` 上配置 `.transition_driver(RouteTweenDriver::new(prepare_effect))`。应用只需一个准备回调，例如：

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

`enter` 和 `exit` 接收第三方 `tween`、`sequence`、`parallel` 等有限时间线。添加 exit track 即要求保留退出页面；退出写入者全部完成后自动 release，全部 track 完成后自动 finish。没有 track 时跳过过渡。所有写入 leaving surface 的动画放在 exit track 中。

适配器在路由发布后从零开始计时，使用 `Time<Real>`；在 UI Prepare 前应用插值，最后一次写入后回收控制器。取消同步停止写入，连续导航沿原队列推进；按需更新窗口会在动画期间以及提交最终完成请求时请求重绘。

适配器支持 `EaseKind` / `EaseClosure` 和 boxed 组件插值，默认注册 `UiTransform`、`Node`、`ZIndex`。应用内容的其它组件可用 `RouteTweenPlugin::default().with_component::<T>()` 注册，通过 `with_closure` 或 boxed interpolator 操作。typed interpolator、resource/asset tween 和 tween event 不由该适配器注册；需要这些能力时使用手工 driver。控制器位于独立 Entity，保持原表面写入边界；应用额外资源副作用仍由应用管理。

如同时使用 `DefaultTweenPlugins`，先安装 `RouteTweenPlugin`，以便把私有时间上下文登记给上游 debug 插件。若上游 debug 已先安装，计时仍独立，但它可能误报未知时间上下文；适配器不会覆盖应用已有的 debug 配置。自定义时钟或特殊完成策略也使用下面的手工 driver。

## 运行手工 driver 示例

从仓库根目录运行：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
```

[窗口示例](../../examples/windowed/src/bin/transitions.rs)使用同一个 driver 接口实现平移和缩放旋转。按钮和键盘操作相同：

| 按键 | 操作 |
| --- | --- |
| `1` / `2` | 进入保留状态的 Notes / Gallery 页面 |
| `3` | 进入每次返回都重新创建的 Fresh 页面 |
| `B` | Back |
| `E` | 切换下一次过渡的效果 |
| `H` | 切换是否持有退出页面；关闭时仅播放进入动画 |
| `R` | 连续发出导航，观察旧动画取消 |

点击页面计数器后切换页面，可比较保留与重新创建的行为。[动画实现](../../examples/windowed/src/bin/transitions/animation.rs)使用官方 `Time<Real>`、`Timer`、`EasingCurve` 和 `UiTransform`；无需额外动画 runtime。窗口运行环境见[应用集成](integration.md#运行仓库示例)。

## 接入 driver

为每个 Router 注册一个应用 driver：

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
| `presented(world, id)` | 路由已发布，启动应用动画。此通知不表示进入动画已完成。 |
| `cancelled(world, id, reason)` | 在返回前停止该 session 的所有写入者，并清理应用动画状态。 |

每次调用 `prepare` 后都会收到 `presented` 或 `cancelled`；已 presented 的 session 也可能随后取消。`Skip` 会触发 `cancelled(..., Skipped)`，以便撤销准备状态。其它原因包括 `PreparationAborted`、`Superseded` 和 `AuthorityLost`。取消已发布的动画不会撤销已提交的路由与 history。

`RouteTransitionContext` 提供不透明 `id()`、Router、request、`NavigationKind`、`from()` / `to()`，以及 `entering()`、`leaving()`、`preserved()`。目标包含 exact `frame()`、应用内容根 `content()` 和可临时修改的 `surface()`。整页效果通常只处理 `is_outermost()` 的目标，避免同时变换父 frame 和嵌套子 frame；preserved frames 仅供读取上下文。

## 表面与动画状态

`surface()` 是既有 Router shell 的 Entity。授权期间可以修改它已有的 `Node` 布局字段（不含 `display`）、`UiTransform` 和 `ZIndex` 值。关系、组件集合、可见性、输入及身份仍由 Router 管理。重叠布局由应用在 `prepare` 中设置，Router 不添加动画 wrapper，也不替换应用 host。

把计时器、动画组件或第三方 animator 放在应用资源或独立的应用 Entity 上，显式指定目标 surface。不要向 Router shell 添加动画组件。应用也可动画化自己的 `content()`；这部分修改及资源副作用由应用自行收尾。

`cancelled` 必须同步停止写入，不能只设置一个等下一帧才生效的停止标记。回调排队的 Commands 会在 Router 继续之前 flush。结束或取消时，Router 恢复仍具有 exact ownership 的 shell 基线，并保留当前 Router 控制的 `display`；保存 Entity 不会延长写入授权。

示例把唯一动画写入者保存在资源中，在 `cancelled` 立即移除；每帧系统排在 `Update` 的 `RouterScheduleSet::PublishSnapshots` 之后，用正常 Bevy 组件写入推进效果。

应用也负责窗口重绘。Reactive winit 模式不会因计时器存在而持续更新；示例在动画运行期间发送 `RequestRedraw`。若应用采用按需更新，等待中的其它应用计时任务也需要安排唤醒。

## 退出释放与完成

`Run { hold_exit: true }` 让退出页面保持可见，延后它原定的清理或保留处置。退出页面已经非活动，不能继续接收路由输入或保留焦点。`Run { hold_exit: false }` 不延后退出处置，适合初次进入或仅播放进入动画；返回后不要继续写 leaving surfaces。

先停止对应目标的动画写入，再发送命令：

```rust
// 退出动画已停止；进入动画仍可继续。
commands.release_route_transition_exit(session_id.clone());

// 全部动画写入已停止；也释放尚未释放的退出页面。
commands.finish_route_transition(session_id);
```

`RouteTransitionId` 只标识这一次动画 session。重复或过期命令不会影响后续 session。示例先在退出动画结束时 release，再在进入动画结束时 finish。

active、当前输入与 history 在路由发布时更新。持有退出页面时，请求可继续处于 `PostCommitCleanup`；退出责任完成后请求即可终结，进入动画仍可继续。只进入的动画不延长请求生命周期。

若后续请求正在等待旧退出责任，Router 可以先取消旧 session 并完成退出处置，再推进队列；这一步不判断新目标是否复用旧表面。没有真实 entering/leaving 变化的等价导航不会新建动画 session，已有的只进入 session 可以继续。

职责划分参考 [Vue Router transitions](https://router.vuejs.org/guide/advanced/transitions.html) 和 [Motion presence](https://motion.dev/docs/react-animate-presence)：路由管理实例存续，应用组合具体效果；不照搬 DOM 或动画运行时。
