# 窗口调试

[交互示例](../../examples/windowed/src/main.rs)提供独立应用内的 Bevy Remote Protocol（BRP）入口。用它发送输入、查询实际布局及获取 renderer 截图，避免操作系统鼠标、键盘和焦点影响其它窗口。此功能属于示例，不是 Router 库的 `devtools` 能力。

## 启动和观察

在第一个终端从仓库根目录启动窗口：

```sh
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features ui-debug
```

窗口启动后，在第二个终端从同一根目录运行 client：

```sh
python3 examples/windowed/ui_debug.py snapshot
python3 examples/windowed/ui_debug.py find "Layout counter"
python3 examples/windowed/ui_debug.py capture /tmp/router-ui.png
```

需要桌面显示与图形驱动，Python client 仅使用标准库。上述 `ui-debug` 模式不请求启动焦点；`ui-debug` 仅在 debug 构建显式启用时监听 `127.0.0.1:15703`，窗口持续更新以处理请求。标准 release 构建不会启用此调试入口。

`snapshot` 返回窗口 Entity、scale factor、实际 UI 布局与文本。`find` 按名称或显示文本检索当前节点。点击使用窗口内**逻辑坐标**；从 snapshot 取得的 `center_physical` 应除以对应窗口的 `scale_factor`：

```sh
python3 examples/windowed/ui_debug.py click 100 120
python3 examples/windowed/ui_debug.py key Digit2 --logical '{"Character":"2"}'
python3 examples/windowed/ui_debug.py key Space
```

上面的点击坐标仅示意，实际目标以查询结果为准。client 在按下/释放之间等待应用帧推进，截图通过 `router/capture` 和 `router/capture_status` 取得 PNG。普通调用可用 `--url` 指定端点；完整参数见 `python3 examples/windowed/ui_debug.py --help`。

## 用户行为检查

| 操作 | 观察结果 |
| --- | --- |
| Overview / Settings，或 `1` / `2` | Push 到对应页面，状态栏同步当前位置和 history |
| Replace（`R`）后 Back（`B`） | replace 不追加项，Back 返回前一成功项 |
| 增加 Layout counter，再切换子页面 | 同一 layout Entity 和计数保留；page counter 随新页面创建 |
| 点击 page counter 获焦，再按 Space | 当前 page 计数增加；切换后旧焦点释放，新页点击前不响应 Space |
| 按住页面按钮，按 `1`/`2` 切页，再释放 | 旧页面不接收 release/click；新页面与其它有效目标仍可交互 |
| Fail（`E`），再 Overview | 原 Build error 显示 fallback，失败不增加 history，后续显式导航恢复 |

fallback 会替换路由布局，跨 fallback 不承诺保留该布局的局部计数。终端输出和状态栏可核对请求、目标 Entity、history cursor 与 focus；页面键盘 handler 在 `RouterInputSet::Ready` 后检查 gate。

跨页按住手势通过 `rpc router/input` 分别发送 `MouseButtonInput`：参数中的 `window` 使用 snapshot 返回的真实窗口 Entity，`button` 为 `Left`，`state` 依次为 `Pressed`、`Released`。保持按下期间切换页面，等待帧推进后再释放；拆开发送只用于持续手势检查，普通交互优先使用 `click`/`key`。参数编码与消息结构见 [client](../../examples/windowed/ui_debug.py) 的 `Bridge.event` 和 `Bridge.click`。

## 证据边界

输入沿真实 Bevy `WindowEvent` 与 typed message 管线，截图来自 renderer；它们验证引擎交互和最终图像，不等同于 OS/硬件输入。snapshot 中的请求成功或正尺寸布局也不等于该时刻已呈现在屏幕上。

持续调试更新、BRP 轮询和截图会影响测量，不用于功耗或正式性能结论。窗口与 headless benchmark 分别说明，见[性能基线](../performance.md)；完整测试入口见[验证指南](../development/verification.md)。
