# 验证与发布检查

[中文](verification.md) | [English](verification.en.md)

本页列出开发、集成和发布前的检查命令。行为定义见[专题合同](../contracts/README.md)，装配步骤见[集成指南](../guides/integration.md)。在仓库根目录执行命令，并记录所用源码、feature、工具链和运行环境。

## 按变更风险选择检查

| 变更 | 检查内容 |
| --- | --- |
| 文档 | 中英文内容对应、文件与锚点链接、命令与实际 target、发布内容扫描 |
| Core/View 局部逻辑 | 格式、受影响 crate 测试、对应公共行为；公共 API 变化加独立 consumer |
| ECS/BSN 安装、Scene 或 outlet | 受影响 crate 与 facade 测试，实际 `App::update` 和官方 Scene 路径 |
| lifecycle、failure、terminal、cleanup 或 publication | 受影响完整 crate、公开导航/恢复旅程、独立 facade consumer；覆盖 owner 收敛、拒绝、写后变化与后续请求 |
| 输入/focus | 正常命中、非活动拒绝、已开始手势撤销、焦点释放、其它有效目标继续交互 |
| feature、依赖、示例、发布配置 | 依赖检查、最小 consumer、示例、窗口编译；发布配置还检查打包产物 |
| 性能 | 对应行为测试和同机优化构建比较，方法见[性能指南](../performance.md) |

测试通过生产入口观察结果。需要构造特殊情况时，让 test-only seam 改变生产判断实际读取的输入事实，再由正常逻辑得出结果。组合行为一起验证，例如 query 导致重建后再 Back，或清理期间 host 失效后重新注册。

## 自动检查入口

使用满足实际解析依赖的 `rust-version` 要求的 Rust 工具链；项目采用 2024 edition。安装 rustfmt 后执行：

```sh
cargo fetch --locked
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
```

这些是 [CI](../../.github/workflows/ci.yml) 的主检查。独立 consumer 在自己的 workspace 中离线解析依赖，`cargo fetch` 提前准备 registry 和依赖缓存。共享 target 由一位验证负责人顺序运行 Cargo/rustc。

CI 的 `--locked` 复现已提交的依赖快照；版本要求见[接入指南](../guides/integration.md#bevy-版本要求)。独立 consumer 使用 `0.19` 范围重新生成自己的 lockfile，不复用仓库的 Bevy 选择。维护依赖快照时用 `cargo update` 解析兼容更新，再运行相应检查。

全特性与最小 feature 各有检查用途。修改 ECS 的可选 BSN 接线时，补充基础 ECS 路径：

```sh
cargo test -p bevy_ui_router_ecs --no-default-features --locked
```

开发声明、Commands 或动画适配时，可先运行对应旅程：

```sh
cargo test -p bevy_ui_router --features bsn --test public_usage --locked
cargo test -p bevy_ui_router_ecs --features bsn,tween transition::tween --locked
```

这些定向检查不代替上表要求的完整 crate 和 consumer 验证。

修改返回 adapter 时，运行单独输入源和组合路由测试：

```sh
cargo test -p bevy_ui_router_platform --features desktop-back --locked
cargo test -p bevy_ui_router_platform --features android-back --locked
cargo test -p bevy_ui_router --features bsn,desktop-back,android-back --test platform_back --locked
```

只检查公共 feature/依赖与独立消费者时：

```sh
cargo test -p bevy_ui_router --test dependency_contract --test facade_consumers --locked
```

consumer harness 分别使用无默认、默认 `view`、显式 `view`、`ecs`、`bsn`、`platform`、`desktop-back` 和 `android-back`。每个应用声明自己的最小依赖；两个推荐示例也会以原样源码编译运行。编译拒绝用例按错误类别、目标符号和源码位置检查私有 API 及 feature 边界。

仓库内链接相对文档所在目录解析。六个 crate 共用根 README；发布到 crates.io 前还需核对其页面中的链接，不能仅用本地链接检查代替。

文档与发布内容检查：

```sh
python3 scripts/check-docs.py
python3 scripts/check-release-content.py --output /tmp/router-release-audit.json
```

发布内容脚本按规则扫描当前文件，并输出脱敏报告。需要同时检查 Git 历史时加 `--history`；额外项目词汇可放在本机文本文件中，通过 `--term-file` 传入。检查报告中的匹配项后再决定处理方式，敏感原文保留在本机审阅。

使用 [Gitleaks](https://github.com/gitleaks/gitleaks) 扫描提交历史中的凭据：

```sh
gitleaks git . --redact --log-opts="--all --full-history" --report-format json --report-path /tmp/router-gitleaks.json
```

## 从模块验证到用户行为

| 主题 | 测试入口 | 覆盖的操作 |
| --- | --- | --- |
| Core 导航 | [Core tests](../../crates/bevy_ui_router_core/tests)、[公开导航旅程](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs) | 路由匹配、嵌套切换、参数/query/fragment、history、失败与恢复 |
| View | [公共 API](../../crates/bevy_ui_router_view/tests/public_api.rs)、[独立 view consumer](../../crates/bevy_ui_router/tests/consumers/view.rs) | 路由输入和 renderer-neutral 绑定 |
| ECS 生命周期 | [公共 API](../../crates/bevy_ui_router_ecs/tests/public_api.rs)、[导航测试](../../crates/bevy_ui_router_ecs/src/navigation_tests.rs)、[清理测试](../../crates/bevy_ui_router_ecs/src/cleanup_tests.rs) | host 注册、身份检查、准备、发布、局部清理和重新接纳 |
| 推荐接入 | [公开使用旅程](../../crates/bevy_ui_router/tests/public_usage.rs)、[声明安装](../../crates/bevy_ui_router_ecs/src/bsn/install/tests.rs)、[Commands](../../crates/bevy_ui_router_ecs/src/navigation_commands.rs) | 嵌套声明、当前输入与可变 Query、延迟导航错误、initial 优先级与 host 重接、多 Router 隔离 |
| BSN | [facade 测试](../../crates/bevy_ui_router/tests/bsn_facade_features.rs)、[独立 BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs) | factory、Scene 应用、outlet 与内容归属 |
| 平台返回 | [返回旅程](../../crates/bevy_ui_router/tests/platform_back.rs)、[桌面 consumer](../../crates/bevy_ui_router/tests/consumers/desktop_back.rs)、[Android consumer](../../crates/bevy_ui_router/tests/consumers/android_back.rs) | 按窗口传递意图、过滤重复、双源通道、弹层优先与定向 Back |
| 输入与焦点 | [pointer](../../crates/bevy_ui_router/tests/bsn_native_input.rs)、[focus](../../crates/bevy_ui_router/tests/bsn_native_focus.rs) | 活动命中、离场撤销、已开始手势和其它有效目标 |
| 保留实例 | [保留旅程](../../crates/bevy_ui_router/tests/retained_navigation.rs) | 父子独立保留、原父/新子、容量淘汰、失效清理和后续导航 |
| 路由过渡 | [过渡旅程](../../crates/bevy_ui_router/tests/route_transitions.rs)、[原生重叠输入](../../crates/bevy_ui_router_ecs/src/input/native_ui_candidates/tests.rs) | prepare/current 输入、首帧样式、enter-only、动态退出、父子保留、Back、连续导航、host 失效与重新接入 |
| Tween 适配 | [适配旅程](../../crates/bevy_ui_router_ecs/src/transition/tween/tests.rs) | 发布后零起点、独立时钟、最后写入后的退出释放、取消停止写入与最终重绘 |
| 滚动恢复 | [滚动旅程](../../crates/bevy_ui_router/tests/scroll_restoration.rs) | 真实布局、早晚容器、ready/cancel、二维裁剪、用户滚动优先、旧 ID，以及过渡与缓存淘汰中的多次访问恢复 |
| 页面系统 | [活动旅程](../../crates/bevy_ui_router/tests/public_activity.rs) | 可变应用 Query、保留页失活/恢复、动态后代与精确父链 |
| 集成边界 | [consumer harness](../../crates/bevy_ui_router/tests/facade_consumers.rs)、[依赖合同](../../crates/bevy_ui_router/tests/dependency_contract.rs) | 各 feature 的可见 API、最小依赖和依赖方向 |

内部测试覆盖具体写入窗口和故障条件，公开旅程串起推荐入口与正常调度。开发中同时关注两层，让局部修改和用户操作保持一致。

## 示例与窗口

运行两个无窗口示例：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

quick start 检查声明安装、最小 Scene runtime 和初始页面。nested 使用真实 UI layout，组合共享布局、query 更新、应用任务、当前路由输入和显式滚动恢复。两者完成有限次更新后退出。

窗口应用有独立 workspace，分别检查默认和全部可选 feature（包括 `ui-debug` 与 `tween`）：

```sh
cargo fmt --manifest-path examples/windowed/Cargo.toml --all --check
cargo fetch --manifest-path examples/windowed/Cargo.toml --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --all-features --locked
cargo run --locked --manifest-path examples/windowed/Cargo.toml
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features tween --bin tween_transitions
```

Tween 示例用同一准备回调定义平移和缩放旋转，可切换退出持有、对比保留计数和 Back；手工 driver 示例展示直接使用生命周期接口的方式。具体合同见[过渡指南](../guides/transitions.md)。

Linux 构建采用 X11；CI 安装 `libxkbcommon-dev`、`libx11-dev`、`libxi-dev`、`libxcursor-dev` 和 `libxrandr-dev`。交互检查包括 push/replace/back、布局与页面计数、不同访问的滚动位置、晚到内容、点击和 Space、焦点释放、跨页手势、错误 fallback 和恢复。具体步骤与 BRP 命令见[窗口调试](../guides/debugging.md)。

窗口示例的键盘处理器在 `RouterInputSet::Ready` 后显式查询 gate；应用接入原生 Tab/Dispatch 时，还需验证其目标选择和派发路径。Android manifest 接线见[系统返回指南](../guides/platform-back.md#android-传统返回)。

## 性能夹具与发布产物

性能夹具先检查行为，CI 使用前三条命令；实例与稳态改动可补充后两条：

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=instances cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=steady ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

正式耗时比较使用[性能指南](../performance.md)的优化构建方式，记录同机相同场景的结果。

打包检查使用完整 checkout、可用的 registry 和 Python 3.12+：

```sh
cargo package --workspace
python3 scripts/check-packaged-consumer.py
```

`cargo package` 生成并验证六个库包。[打包 consumer](../../scripts/check-packaged-consumer.py) 解压这些 `.crate`，把 Router 依赖指向解压目录，核对实际包来源后运行包内 BSN 应用；Bevy 按 `0.19` 兼容范围独立解析。脚本刷新解压源码的时间戳，使共享 target 重新编译本次包内容，继续复用 registry 依赖缓存。

保留上述联网打包命令。Cargo `1.98.1` 和 `1.99.0-nightly (eb98b54bc)` 的 `package --workspace --offline` 已在无外部依赖的双 crate 工程中复现临时 registry 的 `no hash listed` 错误，索引内校验和存在且与包一致；同一 nightly 去掉 `--offline` 后，最小工程及本项目六包内置校验通过。相关上游症状见 [Cargo #14396](https://github.com/rust-lang/cargo/issues/14396)，具体内部触发分支尚未确认。严格离线时，`--no-verify` 生成包再运行打包 consumer 是独立产物验证，不能记作 Cargo 内置校验通过。

检查未提交改动时，先审阅 diff，再显式使用 `cargo package --workspace --allow-dirty` 并记录源码状态。包列表可用 `cargo package -p bevy_ui_router --list` 查看；逐包核对 README、源码、示例和测试所需文件，再运行上面的真实打包检查。

## 已有证据与未验证范围

下表概括仓库历史验证记录，便于选择后续检查。每次提交的实际结果以该次命令输出为准。

| 环境或能力 | 已有记录 | 后续检查 |
| --- | --- | --- |
| Bevy `0.19` 系列 | 历史记录覆盖 `0.19.0` / `0.19.1` 的 workspace、公开旅程和独立 consumer 回归 | 依赖更新后按变更范围验证实际解析结果 |
| Headless Scene/UI | 导航、布局、picking、focus、保留、活动与滚动组合 | 按功能改动选择完整操作链 |
| 打包产物 | 六个库包和解压后的 BSN consumer | 最终发布树重新打包并运行 consumer |
| Linux/WSLg 窗口 | llvmpipe 软件渲染；BRP 输入和 renderer 截图覆盖导航、保留、滚动、焦点、手势、fallback 和应用自定义过渡 | 窗口交互改动后按调试指南复测 |
| Android | 最小 `android-back` 的 `aarch64-linux-android` 编译、Bevy 消息与调度测试 | APK、设备和系统返回行为待实测 |
| Windows 原生、物理 GPU、硬件输入 | 待验证 | 在目标平台记录系统、设备、命令和交互结果 |
| Rust 最低版本 | 依赖声明要求已核对 | 最低工具链运行待验证 |

旧运行日志通过 `git log -- docs/development/verification.md` 查询。新结果记录本次运行的成功、失败和待执行项；远端 CI 状态从实际 workflow run 查询。
