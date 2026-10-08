# 验证与发布检查

本页说明检查入口、证据范围和如何选择验证，不维护固定测试清单或阶段执行日志。产品行为归 [专题合同](../contracts/README.md)，模块边界见 [架构](../architecture/README.md)，应用接线见 [集成指南](../guides/integration.md)。

## 按变更风险选择检查

| 变更 | 所需验证 |
| --- | --- |
| 文档整理 | 本地文件和锚点链接、命令与实际 target 一致、合同没有遗漏或互相冲突；不因修改文档重复运行全部 runtime 测试 |
| Core/View 局部逻辑 | 格式、相关 crate 测试及对应公共行为；公共 API 变化同时运行独立 consumer |
| ECS/BSN 安装、Scene 或 outlet | 受影响 crate 与 facade 测试，真实 `App::update` 及官方 Scene 路径 |
| lifecycle、failure、terminal、cleanup 或 publication | 受影响完整 crate、公开导航/恢复旅程和独立 facade consumer；包含 owner 收敛、拒绝、写后 drift 与后续请求 |
| 输入/focus | 真实管线正向命中、非活动拒绝、已开始手势撤权、focus 释放及其它有效目标保留 |
| feature、依赖、示例或发布配置 | 依赖合同、独立 consumer、实际示例、独立窗口编译；发布配置还需验证打包产物 |
| 性能修改 | 相同语义检查和优化构建同机对照；方法与数据归 [性能基线](../performance.md) |

回归应经生产入口导出结论。私有 seam 可以改变生产 predicate 的输入事实，不能直接注入“成功”“终结”或生命周期分类。新增组合行为要重新验证：分别通过的 search 重建与 Back 测试，曾遗漏两者组合后的 child mount 停滞。

## 自动检查入口

从仓库根目录执行，与 [CI](../../.github/workflows/ci.yml) 的主检查一致：

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
```

首次执行先运行 `cargo fetch --locked`。独立 consumer 会离线解析自己的 workspace，所需 registry 数据与依赖必须已经可用。共享 target 同时只由一个 Cargo/rustc 验证负责人使用。

修改 ECS 可选 BSN 接线时补充不含 BSN 的路径：

```sh
cargo test -p bevy_ui_router_ecs --no-default-features --locked
```

全特性检查不能代替最小 feature 检查。独立 consumer 和依赖合同负责验证默认、无默认、`view`、`ecs`、`bsn` 的真实依赖与可见 API；无需另抄一份 Markdown export allowlist。

## 从模块验证到用户行为

| 责任 | 直接验证入口 | 高层证据 |
| --- | --- | --- |
| Core：匹配、branch diff、请求、history | [Core tests](../../crates/bevy_ui_router_core/tests)、`src/engine_tests.rs` | [公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs) 的嵌套切换、参数/search/fragment、Back、失败及恢复 |
| View：renderer-neutral 输入与 binding | [View public API](../../crates/bevy_ui_router_view/tests/public_api.rs) | [view consumer](../../crates/bevy_ui_router/tests/consumers/view.rs) 的独立使用 |
| ECS：host、identity、staging、publication、cleanup | [ECS public API](../../crates/bevy_ui_router_ecs/tests/public_api.rs)、`src/navigation_tests.rs`、`src/cleanup_tests.rs` | 公开导航旅程的旧内容清理、布局保留、host loss/readmission 与后续请求 |
| BSN：factory、resolve/apply、outlet、containment | [BSN facade tests](../../crates/bevy_ui_router/tests/bsn_facade_features.rs) | [BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs) 及两个原样示例 |
| 输入与焦点 | [原生 pointer 测试](../../crates/bevy_ui_router/tests/bsn_native_input.rs)、[原生 focus 测试](../../crates/bevy_ui_router/tests/bsn_native_focus.rs) | 窗口中的点击/Space、跨页手势与独立目标保护 |
| entry 滚动恢复 | [公开滚动旅程](../../crates/bevy_ui_router/tests/scroll_restoration.rs) | 正常 UI layout 的二维位置、缩放/裁剪、preserved prefix、失败来源、host 重接纳及 stale settlement 拒绝 |
| 页面活动资格 | [公开活动旅程](../../crates/bevy_ui_router/tests/public_activity.rs) | 非交互内容、可变应用 Query、缓存失活/恢复、动态后代和 exact 父链/host registration |
| facade 与 feature/依赖边界 | [独立 consumer harness](../../crates/bevy_ui_router/tests/facade_consumers.rs)、[依赖合同](../../crates/bevy_ui_router/tests/dependency_contract.rs) | 最小依赖的独立应用运行与逐 case 编译拒绝 |

私有测试可以覆盖内部写入窗口、stale owner 和故障注入；公开旅程证明推荐入口与普通调度能够兑现行为。二者互补，不能以一个 snapshot 或私有 probe 替代完整用户路径。

独立 consumer 直接编译并执行推荐示例的原样源码，使用显式依赖，避免借用仓库 dev-dependency 的 feature 合并。编译拒绝逐 case 断言错误类别、目标符号及 consumer source，防止无关错误掩盖权限边界失效。LF/CRLF 覆盖文本输入，不代表 Windows 原生验证。

## 示例与窗口

两个有限次 headless 演示：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

quick start 验证最小 Scene runtime 和请求结果；nested 示例还执行真实 UI 布局并检查父布局/工具栏状态保留。两者均不打开操作系统窗口或启用 GPU renderer。

窗口应用是独立 workspace，图形依赖与 headless 库验证分开：

```sh
cargo fmt --manifest-path examples/windowed/Cargo.toml --all --check
cargo fetch --manifest-path examples/windowed/Cargo.toml --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --all-features --locked
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

Linux 示例选择 X11；CI 安装 `libxkbcommon-dev`、`libx11-dev`、`libxi-dev`、`libxcursor-dev`、`libxrandr-dev`。窗口交互验证 push/replace/back、布局计数保留、页面点击与 Space、旧焦点释放、跨页按住/释放、Build failure/fallback 和恢复。应用内 BRP 自动化见 [调试指南](../guides/debugging.md)。

原生 `TabNavigationPlugin` / `InputDispatchPlugin` 不在示例接线中；键盘通过 Ready 后显式消费 gate。引擎消息注入、OS 事件与物理设备输入是不同证据，不能混写。

## 性能夹具与发布产物

CI 只检查性能夹具行为，不设耗时门禁：

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

正式时间比较使用 [性能基线](../performance.md) 的优化构建方法，不能使用 smoke 的 debug 时间。

发布预检需要完整 checkout 和 Python 3.12+：

```sh
cargo package --workspace
python3 scripts/check-packaged-consumer.py
```

第一步解压并验证六个库包；[产物 consumer](../../scripts/check-packaged-consumer.py) 只把 Router 依赖指向这些解压产物，核对实际来源后运行包内 BSN consumer。Bevy 来自官方 crates.io，consumer 固定 0.19.1。标准打包需要可用的 registry；已有离线多包验证曾遇到 Cargo checksum 错误，不能用 `--no-verify` 将它转成通过。确需验证未提交改动时，先审阅实际 diff，再显式使用 `--allow-dirty` 并准确记录源码状态。

## 已有证据与未验证范围

以下是截至 2026-09-28 的历史运行证据，不是此次文档整理重新运行的结果。固定来源为提交 `6de4233e2c636daa5892807b05af8a19d6e2cc0f` 中的 `docs/superpowers/plans/2026-09-07-router-mvp-implementation.md`（交付、官方版本与嵌套 Back 验证）和 `docs/current-architecture.md`（最终交付状态）；用 `git show <该提交>:<上述路径>` 查询，不恢复为当前计划。性能证据另见[数据身份](../data/performance/README.md)。

已有本地 workspace/独立 consumer、headless Scene/UI/picking/focus、打包产物 consumer 和 Linux/WSLg 窗口交互证据。依赖声明支持官方 Bevy 0.19.x，0.19.0 与 0.19.1 已运行相应回归；默认 lock 与主要 consumer 固定 0.19.1。这不代表已验证未来 patch 版本。

窗口证据来自 Linux/WSLg、llvmpipe 软件渲染；未运行 Windows 原生、最低 Rust 工具链、物理 GPU 加速与硬件设备输入。没有已证实的库级平台限制时，不增加与问题无关的平台矩阵。CI 配置存在不等于远端运行成功；本地成功不能冒充远端状态。

验证结果绑定实际源码、命令、feature 和环境；失败与未运行项如实保留，不以历史测试总数作当前验收证据。发布版本和许可证尚待确定，打包检查不会上传包。
