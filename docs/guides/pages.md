# 页面状态与滚动

[English](pages.en.md) | 简体中文

完成[双页窗口教程](integration.md)后，按需要使用下面的功能。代码沿用教程中的 `page_content`，类型从 `bevy::prelude::*` 和 `bevy_ui_router::prelude::*` 导入。

## 读取当前路径与参数

用 `RouteInputs` 读取页面当前的路径和参数。创建页面的闭包只收到**创建时**的值；布局复用或保留页面重新出现后，当前值可能已经不同。

下面的系统更新带有 `RouteLabel` 标记的文字。先在页面内需要显示路径的 `Text` 实体上添加该标记：

```rust
#[derive(Component, Clone, Default)]
struct RouteLabel;

fn update_route_labels(
    inputs: RouteInputs,
    mut labels: Query<(Entity, &mut Text), With<RouteLabel>>,
) {
    for (entity, mut text) in &mut labels {
        let Some(input) = inputs.current(entity) else { continue };
        text.set_if_neq(Text::new(input.location().path()));
    }
}
```

注册系统时，让它读取本轮导航完成切换后的状态：

```rust
app.add_systems(
    Update,
    update_route_labels.after(RouterScheduleSet::PublishSnapshots),
);
```

对于 `page("items/:id", ...)`，在该页面内用 `input.params().get("id")` 读取参数；query 和 fragment 分别用 `input.location().search()`、`.fragment()` 读取。参数属于当前这一层路由，外层布局不会自动获得子页的参数。

`current(entity)` 只为活动页面返回输入。只想判断页面是否活动时，使用 `RouteActivity::is_active(entity)`。

## 保留页面状态

假设设置页里有输入框，离开再回来时希望保留已输入的内容。将教程中的设置页声明改为：

```rust
page("settings", |_| Ok(page_content("Settings"))).keep_alive()
```

再给 `BsnRouter` 添加 `.keep_alive_capacity(2)`。两个配置都需要：前者选择哪些页面允许保留，后者限制非活动页面的数量，默认容量为 0。命中保留页面时，原 Entity 和组件继续使用，创建页面的闭包不会再次调用。

这个例子的外层布局一直存在，因此设置页可以在它下面保留。嵌套更深时，父布局和子页分别选择是否保留，也分别占容量；父布局被清理后，它下面的保留页面会一起清理。直接挂在应用容器下、没有父路由布局的根页面不保留。

在同一布局内切换子页时，共用布局本来就会复用，不需要为此开启 `.keep_alive()`。完整的容量和淘汰规则见[实例保留参考](../contracts/navigation.md#实例保留)。

### 查询参数改变时也保留

默认情况下，地址中的查询参数（query）改变会重建页面。比如从 `/items?sort=name` 切到 `/items?sort=date`，希望保留页面并自己更新列表，可以在对应的 `page(...)` 上加 `.preserve_on_search()`。

从外层布局到目标页，想继续复用的每一层都要设置它。当前页面和保留页面遵守同一规则。复用后用本页前面的 `RouteInputs` 更新内容。路径参数改变、或最深页面的片段（fragment）改变，仍可能重建；详细判定见[导航参考](../contracts/navigation.md#匹配与-location-更新)。

## 动态内容与重新激活

数据加载使用应用自己的 Bevy 任务和系统：先创建页面或加载提示，再把完成的数据写入页面。

保留页面仍在 World 中，普通 Query 和后台任务不会因它离场而自动暂停。页面系统可以先调用 `inputs.current(entity)`，返回 `None` 就跳过本次更新。异步结果应用前，检查它是否仍对应当前页面和数据版本；需要跨 `await` 使用的路径或参数，先复制为普通值。

带延迟加载的完整流程见[嵌套示例](../../crates/bevy_ui_router/examples/bsn_nested.rs)。

## 滚动恢复

Router 可以在 Back 时恢复**那一次访问**的滚动位置。同一页面被访问多次时，各次位置分别保存。

在**路由页面内已有的滚动容器**上添加标记：

```rust
commands.entity(scroll_container).insert(RouteScroll::new("body"));
```

`scroll_container` 是该容器的 Entity。它应已有 `ScrollPosition`，并在 `Node` 中设置 `overflow: Overflow::scroll_y()`（或需要的横向滚动）。容器的内容、尺寸和滚轮处理沿用应用已有的实现。

`"body"` 是容器名称，在同一页面内保持唯一且稳定；多个容器可以分别用 `"body"`、`"sidebar"`。默认在返回后的首个正常布局尝试恢复一次，位置会被裁剪到可滚动范围。Push/Replace 不会套用 Back 的历史位置。

### 等内容加载完再恢复

如果列表要等数据回来，把标记换成显式恢复：

```rust
RouteScroll::new("body")
    .with_restore_policy(RouteScrollRestorePolicy::Explicit)
```

应用创建好内容后再发送 ready。这里用自己的 `PageState` 记录加载状态：

```rust
#[derive(Component)]
struct PageState {
    content_ready: bool,
}

fn allow_restore(
    restores: RouteScrollRestorations,
    mut commands: Commands,
    pages: Query<(Entity, &PageState)>,
) {
    for (entity, page) in &pages {
        if page.content_ready
            && let Some(restore) = restores.pending(entity, "body")
        {
            commands.ready_route_scroll(restore.id());
        }
    }
}
```

给页面内的 Entity 添加 `PageState`，将 `allow_restore` 注册到 `Update`；创建好内容后把 `content_ready` 设为 `true`。查询时可以使用页面内的 Entity，不要求滚动容器已经存在。

如果等待期间用户主动滚动，在应用的滚轮处理器中用查询到的 ID 取消这次恢复：

```rust
commands.cancel_route_scroll(restore.id());
```

即使滚到边界、位置没有变化，也应取消，避免稍后突然跳回旧位置。ready/cancel 都使用刚查询到的 ID。恢复只执行一次，随后内容增长不会继续追赶旧位置。

<details>
<summary>自定义调度时的执行顺序</summary>

- 新建 UI 并应用 Commands：在 `Update` 或 `UiSystems::Prepare` 之前，使相机等信息完成传播。
- 发送 ready/cancel：在 `RouterScrollSet::Prepare` 之前。
- 主动覆盖本次恢复位置：在 `RouterScrollSet::Prepare` 之后、Layout 之前。
- 读取布局裁剪后的实际位置：在正常 Layout 之后。

需要正常 UI layout 和有效相机。导航完成不表示本轮布局已完成；详细行为见[滚动恢复参考](../contracts/navigation.md#按访问恢复滚动)。

</details>

## 输入与焦点接入

普通鼠标交互使用 Bevy UI picking，Router 会让非活动页面退出命中，并取消离场页面已开始的手势。

页面控件自己的键盘或手柄处理器，用 `RouterInputGate::allows_keyboard(entity)` / `allows_gamepad(entity)` 判断目标当前能否接收输入。将处理器放在 `PreUpdate` 的 `RouterInputSet::Ready` 之后；动态新增、隐藏或移出内容的操作放在输入刷新之前。应用级快捷键使用应用自己的处理逻辑。

需要焦点时安装 `InputFocusPlugin`，并选择首焦点。Router 负责在页面离场时释放焦点。若使用 `TabNavigationPlugin` 或 `InputDispatchPlugin`，还需在目标选择和事件派发时检查上述输入资格；当前不会自动接好这两条路径。

可参考[窗口示例](../../examples/windowed/src/main.rs)中的页面键盘处理器，交互检查步骤见[调试指南](debugging.md)。
