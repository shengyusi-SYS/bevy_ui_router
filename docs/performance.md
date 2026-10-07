# 性能基线

性能工具用于判断真实导航与清理成本，不定义 SLA。入口是公开 facade benchmark：[router_baseline.rs](../crates/bevy_ui_router/benches/router_baseline.rs) 与直接引擎对照 [bsn_content_control.rs](../crates/bevy_ui_router/benches/bsn_content_control.rs)。同类路由项目的方法阅读见[参考](reference/router-benchmarks.md)。

## 运行与比较

从仓库根目录执行优化构建：

```sh
cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > flat.csv
ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > nested.csv
cargo bench -p bevy_ui_router --bench bsn_content_control --features bsn --locked > control.csv
```

默认每个普通场景 128 个样本，可用 `ROUTER_BENCH_SAMPLES` 正整数调整。默认 shape 是 `flat`；`nested` 保持 authored 内容数相同，32/128 节点形成四层浅树，但关系组件分布不同。输出文件需区分形状和版本。

只检查行为夹具，不用于耗时结论：

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

固定机器、依赖、工具链、features 和优化参数，至少重复两轮；测量期间避免并行编译和其它重负载。插桩/profile 使用独立 target，返回正式计时前核对实际二进制身份、shape、样本数、三种规模和完整输出；Router 每轮 48 行汇总。保存全部配对与离群值，受干扰整组标记，不挑选较快结果。

先比较相同工作下的绝对 p50/p95、update 数与波动，再决定 profile 或重构。CI 仅运行上述 smoke，不设置共享机器上的耗时阈值。

## 计时边界

Router 夹具安装 TaskPool/Asset/Scene runtime，通过公开导航、普通 `App::update` 和 snapshots 驱动。路由树固定为父布局及两个子路由，页面含 1/32/128 个 authored `Node + Content` 子节点，另有布局、工具栏和内部 outlet。它不测路由表规模。

时间为 `Instant` 的 CPU 侧 wall-clock，包含调度抢占，不是线程 CPU 计数器或屏幕显示延迟。**不含 UI layout、原生 picking、GPU、窗口、I/O 或异步加载**；Router 输入授权刷新在内。

| 场景 | 计时工作与解释 |
| --- | --- |
| `idle_control` | 同样内容和基础插件，无 Router/wrappers；预热后普通 update 的调度对照 |
| `idle_one_request` | 首次导航后空闲，history 一项、保留请求一项 |
| `idle_retained` | 多轮 replace 后的相同页面与一项 history，验证请求淘汰确已发生 |
| `push` / `replace` / `back` | 从提交前到请求终态、无 running 且前台 cleanup 清空；每轮 a→push b→replace c→back a，history 至多两项 |
| `failure` / `recover` | factory 主动 Build error，发布 fallback，再 replace a；分别计时，失败不改 history |
| `soak_cycle_operations` | 独立夹具预热后连续八批 push/replace/back；每批统计三种操作的混合分布，并检查活实体/保留请求稳定 |

导航包含 factory 构建、resolve/apply、内部校验、publication、旧内容清理与公开完成轮询/snapshot。初始化、目标字符串构造、详细断言和 CSV 输出在计时外。不能把导航 cleanup 移到计时外当作 fixture teardown。

每步断言目标、history、旧内容消失、父布局/工具栏身份与局部状态、输入资格、内容数量及活实体。32 update 的完成上限是防挂起断言，不是时间阈值。有限 soak 稳定不证明全部长期行为或堆内存无泄漏。

导航分位数是逐操作 nearest-rank；idle 每个样本连续 32 update 后取均值，其 p95 是批次均值的分布，不能当成单帧 p95。多轮同场景 p50 的算术均值也不是合并样本分位数。CSV 数量列在场景组结束后、计时外读取，表示稳定状态而非峰值；failure 行的数量在对应 recover 完成后读取。

直接 BSN 对照保留 host/layout/toolbar/outlet，测 fresh Scene 构造、resolve/apply、旧页递归删除、flush 和普通 update。它没有 Router history、授权、隔离、失败/残留或写后复验，两者差值不构成严格 Router 开销归因。

## 已有结果与含义

以下是既有优化构建测量，环境/源码身份归[数据说明](data/performance/README.md)。这些数据绑定历史测量构建，不代表当前发布树的新结果。

最近采用的操作内 removal plan 对照，三轮 push/replace/back 的范围为：

| 页面子节点数 | 导航 p50（ms） | p95（ms） | 相对前一内容迁移版本的同场景 p50 均值变化 |
| --- | --- | --- | --- |
| 1 | 0.179–0.284 | 0.253–0.371 | +2.2%，无收益 |
| 32 | 1.068–1.251 | 1.137–1.351 | −21.8% |
| 128 | 7.976–9.176 | 8.199–10.656 | −35.6% |

[原始对照](data/performance/performance-cleanup-plan-2026-09-27.csv)中的 candidate 是已采用版本。正常、失败、恢复分别为 1/2/3 update。小页面和恢复未证明收益；128 节点仍约 8–9 ms 且不含布局/渲染，不能称大型页面性能就绪。

后续 flat/nested [形状对照](data/performance/performance-shape-2026-09-27.csv)只有第一轮配对无干扰：128 节点 normal p50 为 flat 8.552–9.170 ms、nested 8.129–8.924 ms。它没有出现数量级恶化，但不足以证明稳定形状差异或任意深树。第二/三轮 `contended_pair` 不用于收益估算。该文件的读取合并 candidate 未显示值得保留的收益，**未采用**。

最近定位的主要剩余工作是每次真实写后对完整 target/protected 事实的扫描。此前清理分类、普通内容临时登记和历史 seal 列表造成的重复工作已减去；逐写复验仍保留。性能增长不能仅靠几个点推导全库复杂度，后续应先测代表页面的真实切换帧及 cleanup 占比，再判断局部重构是否值得做。

当前没有真实页面预算支持新增全局缓存、分帧销毁、第二套 runtime、全平台 benchmark 或更多容器微优化。60 Hz 的 16.7 ms 是整个帧的参考预算，不是 Router 可独占的预算。

## 窗口诊断的边界

[窗口 CSV](data/performance/performance-window-2026-09-27.csv)来自简单交互示例、两轮优化构建的临时诊断副本：暖机后 BRP 发送真实 WindowEvent/KeyboardInput，交替导航，采样期间不截图或轮询。每次在输入帧观察成功、正确路径、正尺寸布局与相同 layout Entity；最终截图只验证最终页面。

| 指标 | 两轮 p50（ms） | 两轮 p95（ms） |
| --- | --- | --- |
| 切换帧 First→Last 主世界经过时间 | 5.342 / 4.872 | 6.040 / 6.081 |
| 同批距输入至少四帧的对照 | 3.836 / 3.577 | 4.798 / 4.392 |
| 切换帧之后 First→First 间隔 | 10.323 / 10.004 | 12.809 / 11.056 |

该观察使用 Linux/WSLg llvmpipe 软件渲染和 continuous 更新，未测硬件 GPU。First→Last 不含 render 子应用/GPU 呈现；记录工作在结束时间戳后，仍影响下一帧间隔。CSV 行的 interval 对应前一帧之后，切换后的间隔需读 offset=1，不能读输入行 offset=0。

这不是复杂页面验收、屏幕首帧延迟、FPS 承诺或严格 Router 开销。当前可复现的窗口交互方法见[调试指南](guides/debugging.md)，不需要把临时测量探针变成生产 API。
