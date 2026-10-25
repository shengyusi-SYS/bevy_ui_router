# 窗口调试

[中文](debugging.md) | [English](debugging.en.md)

[窗口示例](../../examples/windowed/src/main.rs)提供 Bevy Remote Protocol（BRP）调试入口，可以向示例窗口发送输入、读取布局和状态、保存渲染截图。输入在应用内处理，系统鼠标和键盘焦点保持原位。

## 启动和观察

在仓库根目录启动：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features ui-debug
```

在另一个终端，从同一目录运行 Python client：

```sh
python3 examples/windowed/ui_debug.py snapshot
python3 examples/windowed/ui_debug.py find "Layout counter"
python3 examples/windowed/ui_debug.py capture router-ui.png
```

运行需要桌面显示和图形驱动，Python client 使用标准库。`ui-debug` 在启用该 feature 的 debug 构建中监听 `127.0.0.1:15703`，并让窗口持续更新以处理请求；release 构建保持普通运行方式。调试窗口启动时不请求窗口焦点。

`snapshot` 提供以下信息：

| 字段 | 含义 |
| --- | --- |
| `windows`、`scale_factor` | 窗口 Entity 和显示缩放 |
| `center_physical`、`size_physical` | 实际节点位置和尺寸，以物理像素表示 |
| `router_active` | 内容是否属于当前活动页面 |
| `pointer_allowed`、`keyboard_allowed` | 当前输入资格 |
| `route_location`、`route_search`、`route_fragment` | 分开的 path、search、fragment 原字符串 |
| `route_id` | 所属 frame 的 route；fallback 为 null |
| `scroll_requested`、`scroll_actual` | 请求位置与布局裁剪后的实际位置 |
| `pending_restore` | 当前 scope 中 `list` 的待恢复位置，容器创建前也可查询 |
| `focus` | 当前焦点 Entity |

非活动内容的路由字段为 null。`find` 按名称或显示文本筛选节点；查找当前页面时再选择 `router_active=true`，因为保留页面的 Entity 仍然存在。

点击使用窗口内的**逻辑坐标**。将目标的 `center_physical` 除以该窗口的 `scale_factor`，再传给 `click`。例如，查询得到逻辑位置为 `(100, 120)` 时：

```sh
python3 examples/windowed/ui_debug.py click 100 120
python3 examples/windowed/ui_debug.py key Digit2 --logical '{"Character":"2"}'
python3 examples/windowed/ui_debug.py key Space
```

client 在按下与释放之间等待应用帧推进。截图通过 `router/capture` 和 `router/capture_status` 获取 PNG。使用 `--url` 选择端点，完整参数见 `python3 examples/windowed/ui_debug.py --help`。

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

终端和状态栏提供请求、目标 Entity、history cursor 与 focus。页面 keyboard handler 在 `RouterInputSet::Ready` 后查询 gate。

## 滚动和持续手势

列表使用真实 `MouseWheel` 消息。先把 cursor 移到 `overview-scroll`、`settings-scroll` 或已加载的 `delayed-scroll` 区域，再通过 `router/input` 发送事件：

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

把示意的 `123` 换成 snapshot 中的窗口 Entity；负 `y` 向下滚动。传递方式是 `python3 examples/windowed/ui_debug.py rpc router/input '<JSON>'`。示例的 scroll observer 修改 Bevy `ScrollPosition`，Router 的 marker 负责按访问保存位置。

持续手势分两次发送 `MouseButtonInput`，使用相同窗口、`button: "Left"`，并将 `state` 分别设为 `Pressed`、`Released`。在两次调用之间切换页面并等待帧推进，即可检查离场撤销。普通点击使用 `click`；消息编码见 [client](../../examples/windowed/ui_debug.py) 的 `Bridge.event` 和 `Bridge.click`。

## 排查问题

| 现象 | 检查方法 |
| --- | --- |
| client 无法连接 | 确认以 `--features ui-debug` 启动 debug 构建，进程仍在运行，端点为 `127.0.0.1:15703` |
| 点击落在错误位置 | 用新 snapshot 定位活动节点，按窗口缩放把物理坐标换成逻辑坐标 |
| Back 后位置未恢复 | 确认 Back 成功且该次访问已有记录，再查看 `router_active`、`pending_restore` 和两个滚动位置字段 |
| 显式恢复一直等待 | 内容准备好后发送 Ready；确认 key 唯一、容器有 `ScrollPosition` 和 scroll overflow |
| `pending_restore` 已消失 | 检查是否已恢复、用户已滚动、应用已取消，或绑定已失效 |
| 有布局数据但截图为空 | 检查窗口、camera/render target 和 renderer；等待帧推进后再次截图 |

重复 key 或缺少有效 layout/camera 时，终端警告会带上首次发现的 Router、frame、generation 和 key。每个 Router 每类警告显示一次。恢复规则见[导航合同](../contracts/navigation.md#按访问恢复滚动)。

调试输入经过 Bevy `WindowEvent` 和 typed message 管线，截图来自 renderer。需要检查系统事件、真实设备或原生平台行为时，在对应环境执行同一交互步骤，并记录运行环境。自动化轮询、持续更新和截图会增加运行开销；性能测量使用[性能指南](../performance.md)中的专用命令。完整测试入口见[验证指南](../development/verification.md)。
