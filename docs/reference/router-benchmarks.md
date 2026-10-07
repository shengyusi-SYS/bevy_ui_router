# 类似路由项目的性能测试经验

这是 2026-09-24 对一手源码的研究记录，保留当时核查的固定版本，不描述这些项目的最新状态。未运行其 benchmark，未复现性能数字，也不作跨框架排名。本项目的实际测量归 [性能基线](../performance.md)，本文不定义新验收要求。

## TanStack Router：按测量责任拆分

核查版本：`ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7`。

| 入口 | 实际测量与可借鉴点 |
| --- | --- |
| [route-tree-construction.bench.ts](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/packages/router-core/tests/route-tree-construction.bench.ts) | 静态、动态、嵌套及混合路由建树；区分处理既有对象与包含对象创建的初始化，并在计时外验证路由数和可匹配结果 |
| [matching-interpolation.bench.ts](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/packages/router-core/tests/matching-interpolation.bench.ts) | 参数、可选段、splat、编码与 Link 预热；轮换输入并消费 checksum，避免只测到无效工作 |
| [client-nav](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/README.md) | 真实 RouterProvider、Link 和订阅者的导航循环；[fixture](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/react/app.tsx) 与 [配置](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/react/vite.config.ts) 使用 production 模式和 jsdom，不包含真实浏览器 layout/paint |
| [SSR](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/ssr/README.md) / [bundle size](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/bundle-size/README.md) | 完整 server handler 请求及 response body 与 JS raw/gzip/brotli 体积分开测量；不同指标不合并为运行时性能 |

该项目使用的百级到万级路由规模是其 fixture，不能直接变成本项目需求。客户端循环实现见 [speed.bench.ts](https://github.com/TanStack/router/blob/ddad69a4a4b18c19e3fcc510ac1e21f9dab883a7/benchmarks/client-nav/react/speed.bench.ts)。

## React Router / Remix：读清计时边界

核查 React Router 官方仓库版本：`2d9827bc09bc562647553f8abf026823e3bfd9dd`；不推断所有历史 Remix 版本。

[playground/performance](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/playground/performance/package.json) 的 [bench.mjs](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/playground/performance/scripts/bench.mjs) 轮换静态、动态、深层和 404 路径，预热后报告吞吐与延迟分位数。虽然注释称 route matching benchmark，实际计时 `fetch()`，包含请求栈；未消费响应 body，异常也加入延迟样本。因此它既不是隔离 matcher 时间，也不是成功完成整页响应的严格基准。

[passthrough-requests.bench.mjs](https://github.com/remix-run/react-router/blob/2d9827bc09bc562647553f8abf026823e3bfd9dd/scripts/bench/passthrough-requests.bench.mjs) 针对重复创建 Request 的具体优化，对比 `passThroughRequests` 两种设置的 document、loader、action handler 成本。mock 返回空页面，不代表真实 React HTML 渲染；它展示了先发现问题、再建立专项微基准的做法。

## Vue Router：体积检查有明确用途

核查版本：`287507b1861558407562b3145443d22785c6b0b8`。

[package scripts](https://github.com/vuejs/router/blob/287507b1861558407562b3145443d22785c6b0b8/packages/router/package.json) 的 `build:size` 构建 size-check fixtures，[根入口](https://github.com/vuejs/router/blob/287507b1861558407562b3145443d22785c6b0b8/package.json) 与 [check-size.mjs](https://github.com/vuejs/router/blob/287507b1861558407562b3145443d22785c6b0b8/scripts/check-size.mjs) 检查产物大小、gzip、brotli。体积检查不能当成匹配、导航或渲染计时。

当时检查该版本文件树中的 bench/perf 路径及 package scripts，没有确认独立运行时 harness。这仅说明本次研究的证据边界，不等于项目从不测试性能。

## 可迁移的方法

以下是对上述实现的归纳，不是这些框架共同规定的标准：

1. 问题决定计时边界：建树、匹配、导航、UI 更新、完整响应和内存保留分别命名。
2. 确认相同且正确的工作：完成状态、校验值、预热、缓存与 setup 都应明确；减少实际工作不能冒充实现提速。
3. 固定 fixture、依赖、工具链和机器，报告绝对成本与波动。共享 CI 不宜直接使用任意百分比阈值。
4. 先 profile 真实热点，再为需要反复比较的路径写专项基准；不因为同类库有某项能力就扩充本项目矩阵。
5. 用户导航包含旧内容 cleanup，不能把它放到 teardown 排除；CPU 终结也不等于屏幕呈现。

当时同时参考了 Bevy 的 [benches manifest](https://github.com/bevyengine/bevy/blob/main/benches/Cargo.toml) 与 [profiling 指南](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md)，以及 Criterion 的 [timing loops](https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html) / [FAQ](https://bheisler.github.io/criterion.rs/book/faq.html)。这些动态链接供进一步阅读，没有固定到研究快照；不能据此断言当前版本的工具支持。可复用的方法是分层测量、采用优化构建、控制每个样本的初始状态，并把插桩开销与正式耗时分开。

本项目已实现公开 API 导航、空闲与有界重复使用基线，并保留直接 BSN 对照。其结论及局限只引用本仓库实际数据，不以 Web fixture、benchmark 的存在或正确性测试通过替代性能证据。
