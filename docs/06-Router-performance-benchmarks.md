# Router 性能测试参考

核查日期：2026-09-24。本文记录一手源码证据与可借鉴的方法，不是本项目新增验收合同。未安装或运行下列 Web 项目的 benchmark，没有复现其性能数字，也不据此比较框架快慢。

## Web 路由的实际实践

### TanStack Router：按责任分层

核查版本：`ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7`。

- **建树微基准**：`route-tree-construction.bench.ts` 用 100、1,000、10,000 条静态、动态、嵌套、混合路由，分别测“已有新对象的处理与初始化”和“对象创建加处理与初始化”；在计时外验证路由数量与可匹配结果。这些规模是该仓库的 fixture，不是本项目应照搬的需求。[源码](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/packages/router-core/tests/route-tree-construction.bench.ts)
- **匹配微基准**：`matching-interpolation.bench.ts` 覆盖参数、可选段、splat、编码与 Link 预热状态，轮换输入，计时后用 checksum 验证实际完成的工作。[源码](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/packages/router-core/tests/matching-interpolation.bench.ts)
- **客户端导航**：React fixture 挂载真实 `RouterProvider`、Link 和订阅者；Vitest 在 `jsdom`、production 模式中执行导航循环，另有 flame profile 入口。它覆盖 Router 与 React 更新成本，但没有真实浏览器 layout/paint，不能解读为用户可见帧时间。入口为 `CI=1 NX_DAEMON=false pnpm nx run @benchmarks/client-nav:test:perf:react --outputStyle=stream --skipRemoteCache`。[说明](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/README.md)、[fixture](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/react/app.tsx)、[配置](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/react/vite.config.ts)、[循环](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/react/speed.bench.ts)
- **SSR 与包体积各自独立**：SSR 对构建后的 server handler 发请求，要求确定性输入并消费 response body；场景隔离，以免新增路由改变原有基线。包体积另测输出 JS 的 raw/gzip/brotli 大小，不混成运行时性能。[SSR 说明](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/ssr/README.md)、[体积说明](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/bundle-size/README.md)

### React Router / Remix：完整请求与具体优化问题

核查 React Router 官方仓库版本：`2d9827bc09bc562647553f8abf026823e3bfd9dd`。以下证据针对该仓库，不推断所有历史 Remix 版本的测试覆盖。

- `playground/performance` 有可运行的 HTTP 压测脚本。先在该目录执行 `pnpm build && pnpm start`，另一个终端执行 `node scripts/bench.mjs [baseUrl] [requests] [concurrency]`。它轮换静态、动态、深层和 404 路径，预热后报告吞吐、平均延迟及 p50/p95/p99。[入口](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/playground/performance/package.json)、[脚本](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/playground/performance/scripts/bench.mjs)
- **测量边界需要读实现**：虽然注释称“route matching benchmark”，它实际计时 `fetch()`，包含请求栈成本；没有消费响应 body，异常也加入延迟样本。因而它不是隔离的 matcher 耗时，也不是成功完成整页响应的严格基准，不能原样用作正确性或性能硬门禁。[同一脚本](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/playground/performance/scripts/bench.mjs)
- `scripts/bench/passthrough-requests.bench.mjs` 用 Vitest 对比 `passThroughRequests` 两种设置下的 document、loader、action handler 开销，针对重复创建 Request 的具体优化；其 mock 返回空页面响应，并未测真实 React HTML 渲染。这展示了“已有具体问题，再写小基准”的用法。[源码](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/scripts/bench/passthrough-requests.bench.mjs)

### Vue Router：确认体积检查，运行时基准覆盖尚未确认

核查版本：`287507b1861558407562b3145443d22785c6b0b8`。检查官方仓库递归文件树的 `bench`/`perf` 路径、根与 router package scripts 后，本次没有确认独立运行时 benchmark harness；这不等于项目从来不做性能测试，也不排除其他分支或临时专项探针。

已确认 `build:size` 构建 size-check fixtures，根脚本 `size` 执行 `scripts/check-size.mjs`，统计产物大小、gzip、brotli。体积检查并非匹配、导航或渲染计时。[package scripts](https://github.com/vuejs/router/blob/287507b1861558407562b3145443d22785c6b0b8/packages/router/package.json)、[根入口](https://github.com/vuejs/router/blob/287507b1861558407562b3145443d22785c6b0b8/package.json)、[体积脚本](https://github.com/vuejs/router/blob/287507b1861558407562b3145443d22785c6b0b8/scripts/check-size.mjs)

## 可借鉴的方法与边界

以下是由上述实现提炼的方法判断，不是各框架共同规定的标准：

1. 先声明问题与计时边界。建树、匹配、导航、UI 更新、完整响应、内存保留各自回答不同问题，不必一次建立完整矩阵。
2. 先保证做了相同且正确的工作，再比较耗时。验证完成状态或计数，区分缓存、预热、初次创建；不把 skipped work 算作优化。
3. setup 是否计时取决于问题。测稳定导航时可排除建树；测首次启动时不能把初始化移出计时。
4. 使用确定性 fixture、固定环境和同机基线，报告绝对成本与波动。Web 项目的路由规模、SSR、包体积或多框架矩阵不能直接变成本项目需求。
5. benchmark 的存在不证明任何库快或慢；正确性测试通过，也不等于已验证性能。无需为了“同类项目都有”而一次建成持续性能平台。

## Bevy / Rust 的可借鉴实践

Bevy 官方独立 `benches` crate 使用 Criterion，并按 ECS、picking、scene、render 等责任拆分 bench target；这支持分层测量，而不是只看整个应用 FPS。[manifest](https://github.com/bevyengine/bevy/blob/main/benches/Cargo.toml)

Bevy profiling 指南要求使用优化构建，并提供系统 tracing、Tracy 与 Chrome/Perfetto 路径。先用 profile 判断时间落在哪里，再为需要反复比较的路径建立 benchmark，通常比先写大量微基准更合适。内存 tracing 自身有额外成本，应与普通耗时测量分开。[官方指南](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md)

Criterion 的 batched timing 可以把 fixture 初始化放在计时之外；输入被修改时，应控制每次样本的初始状态。导航本身的 cleanup 属于被测工作，不能当成 fixture teardown 排除。共享云 CI 的 wall-clock 波动较大，因此初期宜保存结果、同机比较，不设置任意百分比回归硬门禁。[timing loops](https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html)、[FAQ](https://bheisler.github.io/criterion.rs/book/faq.html)

## 本项目判断与 MVP 最小范围

以下是初次调研时源码检查形成的建议，当时没有执行性能测量，也未发现独立 benchmark harness。后续已按此范围补充实现与运行结果，见[性能基线](performance.md)；本节不以静态检查代替测量结论。

- `crates/bevy_ui_router_ecs/src/plugin.rs` 安排每帧输入授权刷新与快照发布。`input.rs::refresh_input_gate_system` 校验活动分支、收集输入事实并处理撤权；`snapshots.rs::publish` 遍历保留请求并投影、复制快照。因此无导航时的成本也值得测，不能只测 matcher。
- `crates/bevy_ui_router_core/src/matcher.rs::match_location` 是可独立测量的核心路径，但应在导航测量或 profile 显示其值得关注后，再增加专项微基准。
- `crates/bevy_ui_router/tests/consumers/bsn.rs` 已有 96 次 replace 后检查实体数、保留布局、history、请求和 cleanup 的行为回归。这证明相关行为与资源保留约束，不提供耗时、分配量或内存高水位证据。
- 既有 debug 窗口示例与 WSLg/llvmpipe 验收属于功能证据，不能充当原生 GPU 帧性能基线。

建议先建立一份小基线，范围限定为三个问题：

| 问题 | 最小场景 | 记录 |
| --- | --- | --- |
| 空闲帧是否有明显固定或随规模增长的成本？ | 固定 UI，活动 Router 不导航；改变受管内容规模和保留请求数量，分别测量 | 稳态 CPU 时间/帧；profile 中 Router 系统占比 |
| 用户导航实际需要多少工作？ | 经公开入口 push、replace、back；保留父布局、替换子页面，并包含一个失败后恢复场景 | 请求提交到终结且相关 cleanup 收敛的 CPU 时间与 update 数；独立逐次样本可统计 p50/p95 |
| 重复使用是否退化？ | 预热后分批重复 replace/back，保持有界 history，复用现有正确性断言 | 各批耗时趋势、实体与保留记录数量；必要时再分析分配和存活内存 |

这里的 CPU 导航终点不等于屏幕首次显示时间。异步等待、真实首帧与 GPU 渲染如需评估，应独立命名和计时。Criterion 的批次统计也不能直接当作逐次导航延迟的 p95。进程 RSS 不回到初始值可能来自 allocator 缓存，不能单凭它认定泄漏。

使用固定依赖、工具链、构建配置和同一台机器；预热后比较基线，报告绝对成本和波动。只选择少量贴近示例与预期使用的规模，不照搬 Web 项目的万级路由矩阵。现阶段不增加全平台 benchmark、专门性能服务或任意毫秒阈值，也不在测量前引入缓存、dirty tracking 或改动 lifecycle 校验。

**调研结论：值得在 MVP 发布准备中做一次小规模性能基线，但缺少 benchmark 本身不是已证实的上线阻断项。** 若真实规模下出现稳定的帧预算压力或随规模显著恶化，再定位原因、决定局部重构与持续回归项。后续测量确实出现页面切换成本和连续使用退化信号，具体证据与局限见[运行结果](performance.md#2026-09-24-初始结果)。
