# 系统返回

[English](platform-back.md) | 简体中文

页面导航已经接好后，可以让鼠标后退键或 Android 返回键触发同一个 Back 操作。

## 桌面鼠标返回

在 `Cargo.toml` 中给 Router 添加 `desktop-back`，例如 `features = ["bsn", "desktop-back"]`。下面的代码沿用 `bevy::prelude::*` 和 `bevy_ui_router::prelude::*`。

先在已有的 App 中安装插件，并注册返回处理器：

```rust
app.add_plugins(DesktopBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

然后用一个应用组件，把 Router 关联到产生输入的窗口：

```rust
#[derive(Component)]
struct WindowRouter(RouterHandle);
```

在窗口创建后执行 `commands.entity(window).insert(WindowRouter(router))`。其中 `window` 是窗口的 Entity，`router` 是 `add_bsn_router` 返回的 handle。

最后添加处理器。先决定是否关闭本地弹层；如果这次返回已由弹层处理，就跳过导航：

```rust
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

收到 `NoBackEntry` 表示没有上一页，由应用决定提示或退出。成功的 Back 沿用已有的页面保留和滚动恢复设置。

弹层和页面返回应放在同一处理器中作一次决定，避免多个消息读取者分别执行返回。完整接线见[窗口示例](../../examples/windowed/src/main.rs)。

## Android 传统返回

在 Android Bevy 应用中添加 `android-back`，例如 `features = ["bsn", "android-back"]`。使用 `AndroidBackInputPlugin`，并复用上面的窗口绑定和 `platform_back` 处理器：

```rust
app.add_plugins(AndroidBackInputPlugin)
    .add_systems(PreUpdate, platform_back.after(PlatformBackInputSet));
```

这个插件接收传统返回按键。

在 Android 16 及以上、`targetSdk >= 36` 的应用中，传统 Back 键需要兼容配置。在现有 `AndroidManifest.xml` 的 `<application>` 或目标 `<activity>` 上设置：

```xml
<application android:enableOnBackInvokedCallback="false">
    <!-- 现有应用配置与 activity。 -->
</application>
```

该开关启用传统返回路径，同时关闭系统预测返回动画。详见 [Android 16 返回迁移说明](https://developer.android.com/about/versions/16/behavior-changes-16#predictive-back)。Activity 的设置优先于 application 默认值，见[按 Activity 配置](https://developer.android.com/guide/navigation/custom-back/predictive-back-gesture#opt-out-at-an-activity-level)。

现代返回回调和预测手势需要单独接入。忽略这条按键消息不会自动交还 Android 的默认返回行为。
