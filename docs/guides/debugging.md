# 导航与窗口调试

[中文](debugging.md) | [English](debugging.en.md)

先按症状检查路由状态，再按需要检查窗口布局和输入。

## 先定位导航问题

| 现象 | 先检查什么 |
| --- | --- |
| 安装报 `MissingSceneRuntime` | 在添加 Router 前安装 `AssetPlugin` 和 `ScenePlugin`；最小示例还安装 `TaskPoolPlugin` |
| 首屏没有出现 | 是否配置 `.initial(...)`、是否创建携带正确 `RouterHostRegistration` 的页面容器、App 是否持续更新；`AwaitingHost` 表示仍在等待页面容器 |
| 快照已有目标位置，但画面为空 | 检查应用容器和页面的布局尺寸、可见性、相机、渲染目标和 UI 渲染插件 |
| 导航调用没有返回请求 ID | Commands 不立即返回 ID；提交失败会输出警告。需要立即处理提交结果时使用 `RouterNavigation`，之后从快照观察切页结果 |
| 导航失败 | 用请求的 `RequestStatus::Failed`、`trace()`、`diagnostics()` 查看失败来源；当前显示错误页时也可读 `active().failure()` |
| 页面已切换，请求仍在 `PostCommitCleanup` | 页面和访问记录已经切换，但退出动画或清理尚未结束；检查 `cleanup()` 与退出动画是否完成 |
| Back 无法执行 | `NoBackEntry` 表示没有更早的成功记录；由应用决定此时的界面反馈或退出动作 |
| 页面状态被重建 | 核对页面的保留开关、容量、原父布局是否还在，以及路径参数、查询参数和片段是否改变；见[保留说明](pages.md#保留页面状态) |
| 滚动没有恢复 | 先确认 Back 成功且该次访问有记录，再查看 `RouteScrollRestorations::pending`、容器名称、布局、相机与恢复是否就绪或取消；见[滚动恢复](pages.md#滚动恢复) |

在 `RouterScheduleSet::PublishSnapshots` 之后读取本轮结果。下面的函数供应用在诊断按钮、错误处理或状态变化时按需调用，避免每帧重复输出：

```rust
use bevy_ui_router::prelude::*;

fn inspect_router(snapshots: &RouterSnapshots, router: RouterHandle) {
    let Some(snapshot) = snapshots.get(router) else { return };
    eprintln!("availability={:?}, location={:?}, phase={:?}, cleanup={:?}",
        snapshot.availability(), snapshot.active().location(),
        snapshot.pending_phase(), snapshot.cleanup());
    if let Some(failure) = snapshot.active().failure() {
        eprintln!("active error: {failure:?}");
    }
    for event in snapshot.diagnostics().events() {
        eprintln!("{:?}: {:?}", event.kind(), event.redacted_message());
    }
}
```

持有 `RouterNavigation` 返回的请求 ID 时，用 `snapshots.request(router, id)` 查询：`Found` 可读取 `status()`、`trace()` 和 `diagnostics()`；`NotRetained` 表示完成记录已被淘汰，`Unknown` 表示该 Router 中没有对应请求。`running_request()` 只表示正在执行的请求，结束后会变为 `None`。

## 启动和观察窗口

[窗口示例](../../examples/windowed/src/main.rs)提供 Bevy Remote Protocol（BRP）调试入口，可以发送输入、读取布局和状态、保存渲染截图。输入在应用内处理，系统鼠标和键盘焦点保持原位。

从仓库根目录进入窗口示例目录并启动：

```sh
cd examples/windowed
cargo run --features ui-debug
```

在另一个终端，也进入 `examples/windowed` 目录，再运行 Python 调试工具：

```sh
python3 ui_debug.py snapshot
python3 ui_debug.py find "Layout counter"
python3 ui_debug.py capture router-ui.png
```

Python 调试工具使用标准库，无需安装额外依赖。调试服务仅在 debug 构建中启用，默认地址为 `127.0.0.1:15703`。

<details>
<summary>snapshot 字段说明</summary>

| 字段 | 含义 |
| --- | --- |
| `windows`、`scale_factor` | 窗口 Entity 和显示缩放 |
| `center_physical`、`size_physical` | 实际节点位置和尺寸，以物理像素表示 |
| `router_active` | 内容是否属于当前活动页面 |
| `pointer_allowed`、`keyboard_allowed` | 当前输入资格 |
| `route_location`、`route_search`、`route_fragment` | 分别表示路径、查询参数和片段的原字符串 |
| `route_id` | 所属路由的标识；错误页为 `null` |
| `scroll_requested`、`scroll_actual` | 请求位置与布局裁剪后的实际位置 |
| `pending_restore` | 当前页面内 `list` 容器的待恢复位置，容器创建前也可查询 |
| `focus` | 当前焦点 Entity |

</details>

非活动内容的路由字段为 `null`。`find` 按名称或显示文本筛选节点；查找当前页面时再选择 `router_active=true`，因为保留页面的 Entity 仍然存在。

点击使用窗口内的**逻辑坐标**。将目标的 `center_physical` 除以该窗口的 `scale_factor`，再传给 `click`。例如，查询得到逻辑位置为 `(100, 120)` 时：

```sh
python3 ui_debug.py click 100 120
python3 ui_debug.py key Digit2
python3 ui_debug.py key Space
```

工具在按下与释放之间等待应用帧推进。截图通过 `router/capture` 和 `router/capture_status` 获取 PNG。使用 `--url` 选择端点，完整参数见 `python3 ui_debug.py --help`。

## 用户行为检查

| 操作 | 观察结果 |
| --- | --- |
| Overview / Settings，或 `1` / `2` | Push 到页面，状态栏更新位置和 history |
| Replace（`R`）后 Back（`B` 或鼠标后退侧键） | Replace 更新当前项，Back 返回前一成功项 |
| 增加 Layout counter 和 page count，在 Overview / Settings 往返 | 原 layout/page Entity 及各自计数保留，离场页面退出布局和输入 |
| 增加 Layout counter，Outside（`4`）后进入另一子页 | 布局 A 和计数保留，子页按新路由选择，工具栏显示新路径 |
| 在列表区域滚动，切页后 Back | 恢复该 history entry 的位置；重复访问同一页面分别保存位置 |
| Delayed（`3`）→ Load（`L`）→ 滚动 → Outside → Back | 返回新建的 Delayed 页面；列表创建前已有恢复意图 |
| 上一步返回后，Ready（`G`）再 Load | 首次正常布局恢复原位置；也可先 Load，待内容准备好后 Ready |
| 返回后 Cancel（`C`）再 Load，或等待 ready 时主动滚动 | 取消等待中的恢复，保留新列表初值或用户的新位置 |
| 点击 page counter 获焦，再按 Space | 当前页面计数增加；切页释放旧焦点，新页点击后取得焦点 |
| 按住页面按钮，按 `1`/`2` 切页，再释放 | 离场页面的手势撤销，当前页面与其它有效目标继续交互 |
| Fail（`E`），再 Overview | 显示 Build error fallback，失败保持 history，后续导航恢复页面 |

示例的保留容量为 3，布局 A、Overview 和 Settings 开启保留；Delayed 和 Outside 离场后清理。fallback 替换路由布局并清理旧缓存，恢复后得到新布局和页面。Load 演示稍后创建应用内容，Ready/Cancel 使用公开滚动命令。

终端和状态栏提供请求、目标 Entity、history cursor 与 focus。页面键盘处理器在 `RouterInputSet::Ready` 后检查输入资格。

## 滚动和持续手势

列表使用真实 `MouseWheel` 消息。先把窗口内光标移到 `overview-scroll`、`settings-scroll` 或已加载的 `delayed-scroll` 区域，再通过 `router/input` 发送事件：

```json
{
  "MouseWheel": {
    "window": 123,
    "unit": "Pixel",
    "x": 0.0,
    "y": -120.0,
    "phase": "Moved"
  }
}
```

把示意的 `123` 换成 `snapshot` 中的窗口 Entity；负 `y` 向下滚动。传递方式是 `python3 ui_debug.py rpc router/input '<JSON>'`。示例的滚轮处理器修改 Bevy `ScrollPosition`，Router 的滚动标记负责按访问保存位置。

持续手势分两次发送 `MouseButtonInput`，使用相同窗口、`button: "Left"`，并将 `state` 分别设为 `Pressed`、`Released`。在两次调用之间切换页面并等待帧推进，即可检查离场撤销。普通点击使用 `click`；消息编码见 [client](../../examples/windowed/ui_debug.py) 的 `Bridge.event` 和 `Bridge.click`。

## 排查问题

| 现象 | 检查方法 |
| --- | --- |
| 调试工具无法连接 | 确认以 `--features ui-debug` 启动 debug 构建，进程仍在运行，端点为 `127.0.0.1:15703` |
| 点击落在错误位置 | 用新的 `snapshot` 定位活动节点，按窗口缩放把物理坐标换成逻辑坐标 |
| Back 后位置未恢复 | 确认 Back 成功且该次访问已有记录，再查看 `router_active`、`pending_restore` 和两个滚动位置字段 |
| 显式恢复一直等待 | 内容准备好后发送 Ready；确认 key 唯一、容器有 `ScrollPosition` 并启用了滚动溢出 |
| `pending_restore` 已消失 | 检查是否已恢复、用户已滚动、应用已取消，或绑定已失效 |
| 有布局数据但截图为空 | 检查窗口、相机、渲染目标和渲染插件；等待帧推进后再次截图 |

容器名称重复或缺少有效布局、相机时，终端警告会带上首次发现的 Router、frame、generation 和 key。每个 Router 每类警告显示一次。恢复规则见[导航参考](../contracts/navigation.md#按访问恢复滚动)。

性能测量见[性能指南](../performance.md)，平台与完整测试入口见[验证指南](../development/verification.md)。
