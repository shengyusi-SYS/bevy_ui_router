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
| ECS：host、identity、staging、publication、cleanup | [ECS public API](../../crates/bevy_ui_router_ecs/tests/public_api.rs)、`src/navigation_tests.rs`、`src/cleanup_tests.rs` | 公开导航旅程的旧内容清理、布局保留、失效缓存部分清理后的 host loss/readmission 与后续请求 |
| BSN：factory、resolve/apply、outlet、containment | [BSN facade tests](../../crates/bevy_ui_router/tests/bsn_facade_features.rs) | [BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs) 及两个原样示例 |
| 输入与焦点 | [原生 pointer 测试](../../crates/bevy_ui_router/tests/bsn_native_input.rs)、[原生 focus 测试](../../crates/bevy_ui_router/tests/bsn_native_focus.rs) | 窗口中的点击/Space、跨页手势与独立目标保护 |
| 保留页与普通页交接 | [公开保留旅程](../../crates/bevy_ui_router/tests/retained_navigation.rs) | 父子独立 opt-in、原父/新子、内部库存与 LRU 依赖淘汰；OwnInvalid、fresh 失败、部分清理 host loss 后原请求终结与残留责任 |
| entry 滚动恢复 | [公开滚动旅程](../../crates/bevy_ui_router/tests/scroll_restoration.rs) | 正常 UI layout 的早晚绑定、显式 ready、二维裁剪、取消与应用优先、未完成记录及旧 ID 失效；保留既有失败、host 重接纳、原父/fresh 子 Back 与当前输入组合 |
| 页面活动资格 | [公开活动旅程](../../crates/bevy_ui_router/tests/public_activity.rs) | 非交互内容、可变应用 Query、缓存失活/恢复、动态后代和 exact 父链/host registration |
| facade 与 feature/依赖边界 | [独立 consumer harness](../../crates/bevy_ui_router/tests/facade_consumers.rs)、[依赖合同](../../crates/bevy_ui_router/tests/dependency_contract.rs) | 最小依赖的独立应用运行与逐 case 编译拒绝 |

私有测试可以覆盖内部写入窗口、stale owner 和故障注入；公开旅程证明推荐入口与普通调度能够兑现行为。二者互补，不能以一个 snapshot 或私有 probe 替代完整用户路径。

独立 consumer 按不同 feature 配置编译运行，并直接消费推荐示例的原样源码，使用显式依赖，避免借用仓库 dev-dependency 的 feature 合并。编译拒绝逐 case 断言错误类别、目标符号及 consumer source，防止无关错误掩盖权限边界失效。依赖规则的 LF/CRLF 输入由解析单测覆盖，不重复运行整套 consumer，也不代表 Windows 原生验证。

## 示例与窗口

两个有限次 headless 演示：

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

quick start 验证最小 Scene runtime 和请求结果；nested 示例执行真实 UI 布局，跨 query 保留原父布局并更新当前输入，默认子页重建。应用任务只返回普通数据，主线程安装内容后 ready 当前滚动意图，Back 的 fresh 子页在正常布局后恢复二维位置；离开布局后返回新子页，核对原父 Entity、工具栏状态、活动资格和新的当前输入。两者均不打开操作系统窗口或启用 GPU renderer；独立 consumer 原样编译运行两者。

窗口应用是独立 workspace，图形依赖与 headless 库验证分开：

```sh
cargo fmt --manifest-path examples/windowed/Cargo.toml --all --check
cargo fetch --manifest-path examples/windowed/Cargo.toml --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --all-features --locked
cargo run --locked --manifest-path examples/windowed/Cargo.toml
```

窗口示例容量为 3，布局 A、Overview、Settings 独立开启保留，Delayed 和 Outside 不保留。它演示原父/新子与当前输入、自动 entry 滚动，以及 Back 到 fresh Delayed 后晚创建列表、ready、取消和应用滚动优先。quick start 演示默认关闭，nested 为父布局开启容量 1，子页保持不保留。

Linux 示例选择 X11；CI 安装 `libxkbcommon-dev`、`libx11-dev`、`libxi-dev`、`libxcursor-dev`、`libxrandr-dev`。窗口交互验证 push/replace/back、布局计数保留、页面点击与 Space、旧焦点释放、跨页按住/释放、Build failure/fallback 和恢复。应用内 BRP 自动化见 [调试指南](../guides/debugging.md)。

原生 `TabNavigationPlugin` / `InputDispatchPlugin` 不在示例接线中；键盘通过 Ready 后显式消费 gate。引擎消息注入、OS 事件与物理设备输入是不同证据，不能混写。

## 性能夹具与发布产物

CI 只检查性能夹具行为，不设耗时门禁：

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=instances cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=steady ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

正式时间比较使用 [性能基线](../performance.md) 的优化构建方法，不能使用 smoke 的 debug 时间。

发布预检需要完整 checkout 和 Python 3.12+：

```sh
cargo package --workspace
python3 scripts/check-packaged-consumer.py
```

第一步解压并验证六个库包；[产物 consumer](../../scripts/check-packaged-consumer.py) 只把 Router 依赖指向这些解压产物，核对实际来源后运行包内 BSN consumer。Bevy 来自官方 crates.io，consumer 固定 0.19.1。归档文件的规范化时间戳可能让共享 target 复用同版本旧编译产物；脚本刷新解压源码时间戳，确保实际编译本次包内容，并保留 registry 依赖缓存。标准打包需要可用的 registry；已有离线多包验证曾遇到 Cargo checksum 错误，不能用 `--no-verify` 将它转成通过。确需验证未提交改动时，先审阅实际 diff，再显式使用 `--allow-dirty` 并准确记录源码状态。

## 已有证据与未验证范围

当前执行入口参数收窄后的树通过 workspace 全特性 1,423 项测试、ECS 无默认特性 653 项测试、workspace 全目标/全特性静态检查和格式检查；独立 consumer 在完整测试中实际运行。安装回归确认 Collect 与 Ready 均非独占系统；正常调度验证 Hover 的 Commands/observer 结果在 Ready 后可见，observer 发起的真实请求在 Collect 产生 action。

已有稳态回归继续覆盖动态 native membership、完整 slot 边界、禁用重复 marker、retained/active 身份、focus、输入拒绝、公开资源 change detection 与后续访问滚动恢复；原生 focus 覆盖晚安装、外部初始化和原调度条件/顺序。稳定快照保持等值不写，终态后的诊断仍可更新。观察只在连续只读阶段复用，保留异步轮询、host 失效、正常滚动采样和写后复验。独立窗口全目标/全特性编译及 `steady` nested smoke 的既有证据绑定观察优化源码 `6a0ab58f704e878efd7494c5f66ba92ea3cf956f`；本轮未重跑窗口或打包，也未测量两处参数收窄的独立收益。已有计时归[性能说明](../performance.md)。

跨 search 复用源码 `a737b4d` 的公开回归覆盖 current/retained 命中及 Push/Replace/Back 与 fragment 的组合。扩展后的 nested 示例进一步验证应用任务安装内容、当前输入与显式滚动 ready，已实际运行并由独立 consumer 原样消费；对应变更通过全特性 ECS、公开保留/滚动/活动旅程、workspace 全目标/全特性静态检查和格式检查。窗口诊断字段通过独立 workspace 全目标/全特性编译与格式检查，未重跑窗口交互或打包。

原生输入扩展候选的补丁与重建入口保存在提交 `8ac20ab`，可用 `git show 8ac20ab:scripts/native-input-candidate/README.md` 查询。候选曾通过 UI 协议、完整 ECS crate 与原生 Tab/dispatch、焦点、pointer、保留和滚动公开旅程；未完成独立下游装配、打包、窗口、硬件输入或性能验收。该方案需要引擎 patch，现已延期并退出工作树，结果不代表当前官方依赖树交付了原生自动接线。

历史验收不能证明所有生产组合。route opt-in 与容量须独立配置；组合回归覆盖混用 Back/Push、淘汰后的后续普通导航、非目标缓存自身失效、暂缺布局后正常离场，以及部分清理中途 host loss 后的同 host 重新接纳与后续 fresh 导航。authority 隔离还须核对 native 内容根的 authored Visibility、Pickable 与应用局部状态保持。不能把跳过失败用例的检查称为全量通过。

窗口源码 `042ee400892e83f8957307752586eb7e98a83c00` 已通过独立 workspace 格式、默认和全特性全目标静态检查，并在 Linux/WSLg、960×640、软件渲染、debug `ui-debug` 中完成组合验收。通过应用内 BRP 的真实 WindowEvent 与 typed messages，验证原父 Entity/计数返回新子且读取新输入、leaf 原 Entity/计数及不同 entry 位置、焦点与已开始手势撤权、replace/失败 history/fallback 后 fresh 恢复。fresh Delayed 返回时容器尚无而意图存在，先 ready 再 load 恢复；取消后 load 保持初值，应用主动滚动取消等待目标。renderer 截图另核对父/新子与晚容器恢复的最终呈现。

父子独立库存源码 `05b373007e1d4fcc240cecdc9fab743a81323b60` 已通过 workspace 全特性、ECS 无默认特性、公开保留/滚动组合及独立 consumer；最后补入的滚动旅程与 nested 示例另经相关 facade 测试和实际运行。性能夹具源码 `dc3ef59e57434f2730fb7a5958e0f660d8294f7a` 通过格式、workspace 全目标/全特性静态检查及行为检查；两轮完整串行优化结果归[父子库存实测](../performance.md#父子库存实测)。

滚动统一意图源码 `69615784830b3b4f425f6e7991fd4a503b31c26c` 已通过格式、workspace 全目标/全特性静态检查、未跳过用例的全特性测试、ECS 无默认特性及独立 consumer。nested 原样消费与实际运行在正常 UI 布局中验证 fresh 子页 Back 后 ready。该提交树执行 `cargo package --workspace --locked`，六个包均验证成功；产物 consumer 复编译解压 Router 源码，核对实际来源后运行公开 BSN 旅程成功。该轮窗口收口仅修改独立示例和文档，库源码仍为该验证树；未上传，最终发布树按后续变更复核。

以下是截至 2026-09-28 的历史运行证据，不是此次文档整理重新运行的结果。固定来源为提交 `6de4233e2c636daa5892807b05af8a19d6e2cc0f` 中的 `docs/superpowers/plans/2026-09-07-router-mvp-implementation.md`（交付、官方版本与嵌套 Back 验证）和 `docs/current-architecture.md`（最终交付状态）；用 `git show <该提交>:<上述路径>` 查询，不恢复为当前计划。性能证据另见[数据身份](../data/performance/README.md)。

已有本地 workspace/独立 consumer、headless Scene/UI/picking/focus、打包产物 consumer 和 Linux/WSLg 窗口交互证据。依赖声明支持官方 Bevy 0.19.x，0.19.0 与 0.19.1 已运行相应回归；默认 lock 与主要 consumer 固定 0.19.1。这不代表已验证未来 patch 版本。

窗口证据来自 Linux/WSLg、llvmpipe 软件渲染；未运行 Windows 原生、最低 Rust 工具链、物理 GPU 加速与硬件设备输入。没有已证实的库级平台限制时，不增加与问题无关的平台矩阵。CI 配置存在不等于远端运行成功；本地成功不能冒充远端状态。

验证结果绑定实际源码、命令、feature 和环境；失败与未运行项如实保留，不以历史测试总数作当前验收证据。发布版本和许可证尚待确定，打包检查不会上传包。
