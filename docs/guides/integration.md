# 应用集成

[中文](integration.md) | [English](integration.en.md)

通过 `bevy_ui_router` 的 `bsn` feature，可以把 Bevy Scene 接到路由树上，由 Router 管理页面切换、共享布局和返回记录。本文从运行示例开始，再介绍保留状态、滚动、输入和平台返回的接法。

## 运行仓库示例

从仓库根目录运行：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

| 示例 | 内容 | 运行方式 |
| --- | --- | --- |
| [bsn_quick_start.rs](../../crates/bevy_ui_router/examples/bsn_quick_start.rs) | 声明页面、默认首屏、读取结果 | 无窗口，完成后退出 |
| [bsn_nested.rs](../../crates/bevy_ui_router/examples/bsn_nested.rs) | 嵌套布局、query 更新、应用异步内容、滚动恢复 | 无窗口，使用真实 UI 布局，完成后退出 |
| [windowed](../../examples/windowed/) | 可点击页面、状态保留、焦点、错误恢复 | 交互窗口 |
| [transitions](../../examples/windowed/src/bin/transitions.rs) | 应用自定义过渡、退出释放、快速连续导航 | 交互窗口 |

启动窗口示例：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin router_windowed
```

窗口示例自行配置窗口、相机、字体和样式。Linux 构建采用 X11，需要 X11/xkbcommon 开发库；运行需要桌面显示和图形驱动。输入自动化和截图见[窗口调试](debugging.md)。

路由过渡默认关闭。接入应用动画以及运行两种效果的独立示例，见[应用自定义路由过渡](transitions.md)。

## 在独立 Cargo 应用中使用

在 `bevy_ui_router` 仓库旁建立 `router-demo`，把 [quick start](../../crates/bevy_ui_router/examples/bsn_quick_start.rs) 复制为 `router-demo/src/main.rs`，再添加以下 `Cargo.toml`。根据自己的目录布局调整 `path`。

```toml
[package]
name = "router-demo"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy_ui_router = { path = "../bevy_ui_router/crates/bevy_ui_router", default-features = false, features = ["bsn"] }
bevy_app = { version = "0.19", default-features = false }
bevy_asset = { version = "0.19", default-features = false }
bevy_ecs = { version = "0.19", default-features = false }
bevy_scene = { version = "0.19", default-features = false }
bevy_ui = { version = "0.19", default-features = false }
```

在 `router-demo` 中执行 `cargo run`。项目使用 Rust 2024 edition，工具链与检查命令见[验证指南](../development/verification.md)。

复制嵌套示例时，再添加它使用的布局、相机与输入依赖：

```toml
bevy_input_focus = { version = "0.19", default-features = false }
bevy_internal = { version = "0.19", default-features = false, features = ["bevy_window", "bevy_camera", "bevy_ui", "bevy_picking"] }
```

## Bevy 版本要求

Bevy 依赖使用普通的 `version = "0.19"`，即 `>=0.19.0, <0.20.0`，由 Cargo 选择兼容补丁版本。Bevy 的 `0.x` 次版本升级需要单独适配；版本范围遵循 [Cargo 默认要求](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#default-requirements)。

仓库 `Cargo.lock` 和示例命令中的 `--locked` 用于复现本仓库构建，不固定消费方版本；应用通过自己的 manifest 和 lockfile 解析依赖。镜像、vendoring、Git/path 和 `[patch]` 等来源选择交给 Cargo，本库不额外扫描或禁止这些配置。

## 声明页面并启动

先安装应用需要的 Bevy 插件。最小无窗口应用使用 `TaskPoolPlugin`、`AssetPlugin` 和 `ScenePlugin`；可见 UI 还需要窗口、相机、UI/render 和输入插件。随后用 `page`、`layout`、`index` 直接关联路径与 Scene factory：

```rust
use bevy_app::App;
use bevy_scene::bsn;
use bevy_ui::Node;
use bevy_ui_router::prelude::*;

fn install_pages(app: &mut App) -> Result<RouterHandle, BsnRouterError> {
    let router = app.add_bsn_router(
        BsnRouter::new(
            layout(|_| Ok(bsn! { Node DefaultOutlet })).children([
                index(|_| Ok(bsn! { Node })),
                page("items/:id", |_| Ok(bsn! { Node })),
            ]),
        )
        .fallback(|_| Ok(bsn! { Node }))
        .initial("/"),
    )?;
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    Ok(router)
}
```

应用负责 host 的布局。`layout` 声明不消耗路径的布局，`index` 声明父路径下的默认页面；有路径的布局使用 `page(...).children(...)`。单页 Router 可直接使用 `BsnRouter::new(page(...))`，不会隐式增加布局。所有子路径相对父路由，前导 `/` 不会跳出父级。

factory 接收创建时的 `&RouteViewInput`，每次返回新 Scene 或 `BsnBuildError`。`fallback` 必须提供；某个路由的局部错误页用 `.error_view(factory)` 声明。安装统一生成并验证路由树和 view/driver 绑定，声明错误包含所在节点位置，不必逐项创建 key 或 unwrap。

`initial` 是一次性默认首屏。Router 等待首次 host Ready，再通过普通 Push 入队；之前已成功入队的显式导航会取消尚未提交的默认首屏。首次入队失败会报告 warning 并结束本次默认意图，host 重新接入不会重放已消费的首屏。窗口应用随后调用 `app.run()`；quick start 的有限更新循环用于无窗口验证。

`add_bsn_router` 会安装 Router 系统；也可预先安装 `BevyUiRouterPlugin`。遇到 `MissingSceneRuntime` 时，先安装 `AssetPlugin` 和 `ScenePlugin`，使 `AssetServer` 与 `Assets<ScenePatch>` 就绪。

需要自行组织 registry 的高级接入仍可使用 `register_bsn_view`、`RouteViewRegistry`、`RouteTree` 和 `add_ui_router(RouterConfig)`。它们共享同一验证与执行链；重复 key 保留原注册并报错。对应规则见 [Scene 合同](../contracts/scene.md)。

## 导航与结果观察

普通交互通过 `Commands` 发起导航，可与 `RouteInputs` 和应用组件 Query 在同一系统中使用：

```rust
use bevy_ecs::prelude::Commands;
use bevy_ui_router::prelude::*;

fn open_item(commands: &mut Commands, router: RouterHandle) {
    commands.router(router).push("/items/42?sort=name#details");
}
```

同样提供 `.replace(destination)` 和 `.back()`。命令在 Bevy 应用 deferred commands 时才进入原 Router 队列，不同步返回 request ID，也不保证本轮完成发布。入队失败默认通过 Bevy warning handler 报告；`.with_error_handler(handler)` 可选择自己的 `bevy_ecs::error::ErrorHandler`，从 `BevyError` downcast 到 `NavigationCommandError` 后读取 Router、操作和原 `NavigationEnqueueError`。

字符串通过 `RouteLocation::from_address` 分离 path、search 和 fragment，去掉分隔用的 `?`/`#`，保留原始内容及缺省/空串差别；不解析外部 URL、不解码、不根据当前页面解析相对地址。仍可显式构造位置：

```rust
let location = RouteLocation::new("/items/42")
    .with_search("sort=name")
    .with_fragment("details");
```

`new` 始终只接收 path；两个 setter 原样保留输入，包括调用方传入的 `?` 或 `#`。结构化位置传给 Commands 时不会重新解析。匹配与复用规则见[导航合同](../contracts/navigation.md)。

需要立即判断入队错误或保存 request ID 时，使用 `RouterNavigation::{push,replace,back}`；`Ok(request_id)` 只表示已入队。与当前页面查询一起使用时选择 Commands，避免同一系统同时借用 `RouteInputs` 和 `ResMut<RouterNavigation>`。平台返回处理器下文演示了即时入口。

`RouterSnapshots::request(handle, id)` 提供 `Succeeded` / `Failed` 等结果；`NotRetained` 表示完成记录已淘汰，`Unknown` 表示没有对应记录或 ID 属于另一个 Router。需要本轮快照的系统排在 `Update` 的 `RouterScheduleSet::PublishSnapshots` 之后。新页面和 history 已发布时，请求仍可能处于 `PostCommitCleanup`；界面状态和请求完成分别读取。

layout Scene 在承载子页面的 `Node` 上声明唯一 `DefaultOutlet`。切换相邻子页时，Router 复用共同父布局、outlet 和组件状态；需要独立存续的应用控件放在路由内容边界之外。Scene 和 host 要求见 [Scene 合同](../contracts/scene.md)与[生命周期合同](../contracts/lifecycle.md#host-接纳与重新注册)。

## 布局与页面保留

在 `page(...)` 或 `layout(...)` 声明上接 `.keep_alive()`，同时为 `BsnRouter` 配置 `.keep_alive_capacity(n)`：

| 配置 | 用途 |
| --- | --- |
| `BsnRoute::keep_alive()` | 离开后允许保留这个路由的 Entity 和局部状态，默认关闭。共同布局在当前分支内的复用照常进行。 |
| `BsnRouter::keep_alive_capacity(n)` | 最多保留多少个非活动实例，默认 0。父布局和子页分别计数。 |
| `BsnRoute::preserve_on_search()` | query 改变后允许复用，默认关闭。当前实例与保留实例采用相同规则，params 等身份条件仍参与判断。 |

离场保留需要三个条件：**该路由启用保留、Router 容量非零、原普通父实例及 outlet/slot 作用域仍有效**。父实例可以活动或保留。直接挂在 host 下的 root 没有这样的父实例，因此离场后清理；父实例被清理或淘汰时，依赖它的保留实例一起清理。容量 0 可以用于全局关闭保留。

父、子分别配置。只保留布局时，返回该布局会沿目标路由创建或复用子页。跨 query 导航需要复用多个层级时，逐层设置 `preserve_on_search`；Router 从第一个身份不匹配或未启用的节点起重建。search 与 fragment 同时变化时，还会应用最深 frame 的 fragment 重建规则。容量、LRU 和 history entry 的细节见[实例保留](../contracts/navigation.md#实例保留)。

## 动态内容与重新激活

页面可使用普通 Bevy 系统和任务加载数据：

1. factory 创建页面结构；它收到的 `RouteViewInput` 表示创建时的路由输入。
2. 页面发布后，通过 `RouteInputs::current` 读取当前位置，按应用的数据标识启动或更新任务。
3. 任务返回数据或句柄，由应用系统核对它对应的页面和数据版本，再修改 World。离场时继续任务还是取消任务，由应用决定。
4. 重新激活保留页面时，原 Entity 和组件状态继续使用，factory 不重新调用。`RouteInputs::current` 只返回活动页面的当前输入；只需活动布尔值时可用 `RouteActivity`。
5. 内容准备好后，如果需要恢复滚动，通知当前滚动恢复意图 ready。

完整组合见[嵌套示例](../../crates/bevy_ui_router/examples/bsn_nested.rs)。页面活动状态、当前输入和输入资格的查询方式见下文。

## 滚动恢复

在滚动容器上同时添加 `RouteScroll::new("body")`、Bevy 的 `ScrollPosition`，并在 `Node` 中设置 scroll overflow。`RouteScroll` 从 facade/prelude 导入。同一页面的多个容器使用不同稳定 key，如 `"body"` 和 `"sidebar"`；子页面可以使用与父布局相同的 key。

默认策略在容器唯一绑定后的首个正常布局恢复一次。需要等待数据或动态内容时，选择显式策略：

```rust
let marker = RouteScroll::new("body")
    .with_restore_policy(RouteScrollRestorePolicy::Explicit);
```

BSN 通过官方 `PatchFromTemplate` 修改组件，写法见[嵌套示例](../../crates/bevy_ui_router/examples/bsn_nested.rs)。内容准备好后，查询当前恢复意图并提交 ready：

```rust
fn allow_restore(
    restores: RouteScrollRestorations,
    mut commands: Commands,
    pages: Query<(Entity, &PageState)>,
) {
    for (scope, page) in &pages {
        if page.content_ready {
            if let Some(restore) = restores.pending(scope, "body") {
                commands.ready_route_scroll(restore.id());
            }
        }
    }
}
```

这里的 `PageState` 是应用自己的组件。`scope` 可以是当前活动 frame 内的任一内容 Entity，所以容器创建前也能查询恢复意图。`restore.position()` 是该次访问保存的二维逻辑像素位置。

取消恢复使用 `commands.cancel_route_scroll(restore.id())`。处理用户滚动时，可用容器的 `RouteScroll::key()` 查找并取消等待中的恢复；即使用户已滚到边界、位置数值未变，这样也能表达新的滚动意图。ready 触发一次恢复，后续内容增长沿用正常布局。每次 Back 都有自己的恢复 ID；命令携带此处查询得到的原 ID，不在执行时重新寻找当前意图。

安排系统时使用以下顺序：

| 工作 | 调度位置 |
| --- | --- |
| 创建 UI 结构并应用 deferred 写入 | `Update` 或 `UiSystems::Prepare` 之前，使相机等传播完成 |
| 发送 ready/cancel，修改已有组件值 | `RouterScrollSet::Prepare` 之前 |
| 应用主动覆盖本次恢复位置 | `RouterScrollSet::Prepare` 之后、Layout 之前 |
| 读取裁剪后的实际位置 | 正常 Layout 之后 |

应用安装正常 UI layout 并提供有效的 camera/render target。导航结果和布局结果按各自阶段读取。早晚出现的容器、保留页面和访问记录的行为见[按访问恢复滚动](../contracts/navigation.md#按访问恢复滚动)。

## 输入与焦点接入

`RouteInputs::current` 已包含活动资格判断，普通页面不必再查询一次 `RouteActivity`。它从 facade/prelude 导入，可和应用自己的可变组件查询及 Commands 一起使用：

```rust
fn update_route_labels(inputs: RouteInputs, mut labels: Query<(Entity, &mut Text)>) {
    for (entity, mut text) in &mut labels {
        if let Some(input) = inputs.current(entity)
            && text.0 != input.location().path()
        {
            text.0 = input.location().path().to_owned();
        }
    }
}

app.add_systems(
    Update,
    update_route_labels.after(RouterScheduleSet::PublishSnapshots),
);
```

只需判断活动状态而不读取路由输入时，使用 `RouteActivity::is_active(entity)`。每轮查询当前输入，让复用布局和重新激活页面读到新的 location；仅在文字变化时写入，减少 `Changed<Text>` 通知。读参数、改应用状态并导航的完整组合见[公开使用旅程](../../crates/bevy_ui_router/tests/public_usage.rs)。

`RouteInputRef::params()` 返回所属 frame 自己的参数，`location()` 包含完整 path/search/fragment。错误 frame 提供 `failure()` 和 `origin()`；正常前缀的 failure 为 `None`。fallback 的 `route()` 为 `None`、params 为空。非活动或归属失效的内容返回 `None`。需要跨 World 修改或 `await` 使用的字段，先复制为普通值。具体行为见[页面活动查询](../contracts/input-focus.md#页面活动查询)和[独立消费者示例](../../crates/bevy_ui_router/tests/consumers/bsn.rs)。

输入按目标使用专门的 gate：

- **Pointer：**使用 Bevy UI picking。Router 会隔离非活动内容，并在离场时撤销该目标的历史手势。新增、隐藏或移动内容的操作安排在输入刷新之前。
- **Keyboard/gamepad：**目标定向系统排在 `PreUpdate` 的 `RouterInputSet::Ready` 之后，按目标调用 `RouterInputGate::allows_keyboard` 或 `allows_gamepad`。全局快捷键由应用自己的规则处理。
- **Focus：**安装 `InputFocusPlugin`，由应用选择首焦点。Router 在发布和离场时维护受管目标的焦点资格。接入 `TabNavigationPlugin` / `InputDispatchPlugin` 时，应用还需将路由 gate 接到目标选择和派发路径；窗口示例采用显式 gated keyboard handler。

交互检查步骤见[窗口调试](debugging.md)，平台和设备验证记录见[验证指南](../development/verification.md)。

## 桌面鼠标返回

启用 facade 的 `desktop-back` feature，例如 `features = ["bsn", "desktop-back"]`，从 `bevy_ui_router::prelude::*` 或 `platform::prelude::*` 导入 adapter：

```rust
app.add_plugins(DesktopBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

应用在窗口 Entity 上保存 Router handle，并用一个处理器决定先关闭本地弹层还是执行导航：

```rust
#[derive(Component)]
struct WindowRouter(RouterHandle);

fn platform_back(
    mut requests: MessageReader<PlatformBackRequested>,
    windows: Query<&WindowRouter>,
    mut navigation: ResMut<RouterNavigation>,
) {
    for request in requests.read() {
        let Ok(target) = windows.get(request.window) else { continue };
        // 在这里先处理本地弹层，再决定是否导航。
        match navigation.back(target.0) {
            Ok(request_id) => { /* 保存 ID，通过 RouterSnapshots 观察完成。 */ }
            Err(error) => eprintln!("Back was not enqueued: {error:?}"),
        }
    }
}
```

`WindowRouter` 是应用组件，用 `commands.entity(window).insert(WindowRouter(router))` 绑定到窗口。它把输入的窗口和 Router host 联系起来。收到 `NoBackEntry` 时，由应用决定反馈或退出等宿主行为。成功入队的 Back 继续走普通 history、保留、滚动恢复和清理流程。

每个 `MessageReader` 都会读到消息，因此把本地 UI 优先级和导航决定集中到同一处理器。可运行接线见[窗口示例](../../examples/windowed/src/main.rs)；多窗口及弹层优先的用法见[返回旅程](../../crates/bevy_ui_router/tests/platform_back.rs)。

`desktop-back` 单独启用依赖 Bevy app/ECS/input；窗口与输入后端由应用提供。`platform` 和默认 facade 的依赖说明见[架构总览](../architecture/README.md)。

## Android 传统返回

在 Android Bevy 应用中启用 `android-back`，例如 `features = ["bsn", "android-back"]`，安装按键源，并复用上面的应用处理器：

```rust
app.add_plugins(AndroidBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

adapter 接收逻辑键 `BrowserBack` / `GoBack` 的首次按下，跳过重复和释放。Android 窗口后端与打包由应用配置。这个 feature 单独启用依赖 Bevy app/ECS/input 的 std、keyboard 功能。

在 Android 16 及以上、`targetSdk >= 36` 的应用中，传统 Back 键需要兼容配置。在现有 `AndroidManifest.xml` 的 `<application>` 或目标 `<activity>` 上设置：

```xml
<application android:enableOnBackInvokedCallback="false">
    <!-- 现有应用配置与 activity。 -->
</application>
```

该开关启用传统返回路径，同时关闭系统预测返回动画。详见 [Android 16 返回迁移说明](https://developer.android.com/about/versions/16/behavior-changes-16#predictive-back)。Activity 的设置优先于 application 默认值，见[按 Activity 配置](https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture#opt-out-at-an-activity-level)。

本 adapter 表达的是按键意图。Bevy 的 `KeyboardInput` 提供按下/释放信息，Android 的取消标志、预测返回进度和现代返回回调需要应用侧的专门集成。忽略意图或收到 `NoBackEntry` 后，宿主返回行为也由同一处理器明确选择。最小接线见 [Android consumer](../../crates/bevy_ui_router/tests/consumers/android_back.rs)，平台验证状态见[验证指南](../development/verification.md)。
