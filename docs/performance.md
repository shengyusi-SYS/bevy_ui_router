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

## Keep-alive 对照

使用同一 benchmark 的独立模式；不改变默认模式的 48 行 CSV，也不把旧 `retained_requests` 列当作缓存实例数：

```sh
ROUTER_BENCH_MODE=keep_alive cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > keep-alive-flat.csv
ROUTER_BENCH_MODE=keep_alive ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > keep-alive-nested.csv
```

行为 smoke 使用相同环境变量与 `cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test`。正式计时样本数由 `ROUTER_BENCH_SAMPLES` 控制，smoke 固定为 2；正式比较至少重复两轮，同一轮记录 flat/nested 的完整输出和构建身份。每种形状输出 24 行，覆盖相同的 1/32/128 内容预算。

| 场景前缀 | 真实工作与计时外检查 |
| --- | --- |
| `disabled` | 默认容量 0、路由不 opt-in，a/b 连续 Replace；每次 factory 调用一次，旧 Page 消失 |
| `miss` | 容量 2，a→b 首次 Replace miss；每次样本前在计时外离开父 scope 再建立 a，避免混入容量淘汰；一次 factory 调用、一个 inactive Page |
| `hit` | 容量 2，预热后 a/b 连续 Replace 往返；原 Page Entity 复用，计时操作不调用 factory |
| `lru` | 容量 2，a/b/c/d 四页循环 Replace；容量满后每次 fresh 构建并淘汰最旧 inactive 实例，其余两个实例保留 |

每组输出 `*_replace` 与 `*_idle`。导航计时仍包含 enqueue 到请求终态、`running_request=None` 和 foreground cleanup 清空；idle 沿用每样本 32 次 update 的均值。所有场景使用同一个内容 builder 和 `/:section` 父路由，父参数在往返期间不变；初始化、预热、miss 的父 scope 重置和详细断言在计时外。

独立 CSV 报告 `capacity`、`live_entities`、`active_pages`、`inactive_pages` 与 `factory_builds`。页面及内容的 activity census 通过公共 `RouteActivity` 读取，另核对输入资格、稳定实体总数、history 仅一项、父布局/工具栏身份与局部状态。`factory_builds` 只累计计时导航中的 factory 调用，不包含初始化、预热或 miss 重置；对应 idle 为 0。每组结束再次离开父 scope，确认原 Page/Content Entity 与旧父布局均已释放，app-owned host 仍存在。

这些列观察 authored 页面、内容和活实体；不测 heap，也不暴露或推定 Core 内部 owner 数量。导航与 idle 的差异包含相应驻留内容和公开 Router 机制，不能单凭时间差归因到某个内部步骤。该模式同样没有 UI layout、GPU、窗口或 I/O。

## 父子库存对照

父子库存采用独立 `instances` 模式：

```sh
ROUTER_BENCH_MODE=instances cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > instances-flat.csv
ROUTER_BENCH_MODE=instances ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > instances-nested.csv
```

每组 24 行、正式每场景 128 样本；至少两轮 flat/nested。预热 64 次导航，让公开终态请求保留达到稳定数量，计时外继续核对；避免把请求增长期混入稳态。行为 smoke 将 `cargo bench` 换为 `cargo test` 并加 `-- --test`。父和子**各自**含 1/32/128 个 authored 内容节点，活动内容总数为两倍，不能与旧 leaf 模式按相同数字直接计算性能变化。

| 场景前缀 | 连续 Replace 的真实工作 |
| --- | --- |
| `disabled` | 容量 0，父子均 fresh，旧父子清理 |
| `parent_fresh_child` | 容量 2，仅父保留；原父 Entity/局部状态复用，子每次 fresh |
| `parent_child_hit` | 容量 2，父子均保留；预热后原父子命中，factory 均不调用 |
| `dependency_eviction` | 容量 2，三对父子循环；满容量后每次 fresh，淘汰一组有 exact 父依赖的旧父子 |

同样报告导航与 idle；计时沿用 enqueue 至终态及前台 cleanup 清空的边界。计时外核对公共 activity、输入资格、history、factory 次数、原 Entity/父局部状态、库存数量和稳定活实体数，场景结束用真实 fallback 路径释放全部父子及内容。输出父/子独立内容预算、非活动实例和 factory 计数；不测 heap 或内部 owner 数。普通基础插件和内容 builder 与其它模式相同，不含 UI layout、GPU、窗口或异步 I/O。

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

## Keep-alive 实测

2026-10-08 在同机顺序完成两轮 flat/nested 优化构建对照，每场景 128 样本，共四组完整结果。运行前已关闭窗口和 renderer；源码、工具链、环境及全部 CSV 见[数据身份](data/performance/README.md#keep-alive-2026-10-08)。下表为四组逐操作 p50 范围，单位 ms；p95、最大值、idle 与实例数量保留在[完整数据](data/performance/performance-keep-alive-2026-10-08.csv)。

| 内容子节点 | disabled Replace | 首次 miss | 连续 hit | 满容量 LRU |
| --- | --- | --- | --- | --- |
| 1 | 0.184–0.196 | 0.147–0.156 | 0.108–0.114 | 0.238–0.254 |
| 32 | 1.019–1.090 | 0.440–0.454 | 0.210–0.212 | 1.308–1.370 |
| 128 | 7.466–7.706 | 4.026–4.177 | 0.580–0.659 | 10.017–10.241 |

128 子节点时，活实体数在默认关闭时稳定为 179，miss/hit 为 309，满容量 LRU 为 439，非活动页为 0/1/2；每组离开父 scope 后旧 Page/Content/布局均释放，app-owned host 保留。满容量 idle p50 为 0.225–0.237 ms，默认关闭为 0.143–0.154 ms。计数证明本夹具驻留有界，不能由此宣称堆内存或内部 owner 数量无泄漏。

命中省去 factory/apply 并复用实体，连续往返收益明确。首次 miss 保留旧页，默认关闭则立即清理旧页，两者职责不同；不能把时间差称为 Scene 构建优化。满容量淘汰包含回收成本，128 子节点仍约 10 ms 且不含 layout/render，复杂页面性能尚不能据此宣布就绪。后续优先按代表页面预算定位清理与逐写复验成本，避免以更换 LRU 容器或新增 runtime 代替实际热点分析。

## 父子库存实测

2026-10-11 在同一优化构建完成两轮完整串行 flat/nested，采用数据的每个场景 128 样本。完整源码、环境、统计质量和全部输出归[数据身份](data/performance/README.md#父子库存-2026-10-11)。下表为四组逐操作 p50 范围，单位 ms；父与子各自采用同一内容预算，不能和旧 leaf 表按相同数字计算版本收益。

| 父/子各自内容节点 | 均 fresh 并清理旧父子 | 原父、新子 | 原父子命中 | 满容量依赖淘汰 |
| --- | --- | --- | --- | --- |
| 1 | 0.527–0.534 | 0.306–0.366 | 0.218–0.223 | 0.616–0.633 |
| 32 | 2.645–2.850 | 1.284–1.386 | 0.484–0.535 | 3.198–3.471 |
| 128 | 19.515–20.561 | 8.299–8.895 | 1.685–1.847 | 25.221–26.767 |

128 预算的依赖淘汰 p95 为 26.154–27.865 ms；完整命中 p95 为 1.744–1.913 ms，仍保留一次 5.705 ms 最大值。均 fresh 与依赖淘汰为 3 update，原父/新子与完整命中为 1 update。活实体数分别稳定为 309/441/571/571，非活动父/子为 0/0、1/0、1/1、1/1；最后的真实 fallback 路径释放所有父子及内容。完整命中的 factory 为零，原父/新子仅构建子。

复用减少真实构建和销毁，收益成立；驻留有界也成立，但不证明 heap 无泄漏。256 个活动 authored 内容节点的 fresh/依赖淘汰已经超出整个 60 Hz 帧的参考预算，且尚不含 layout/render；不能宣布复杂页面性能就绪。下一步只按真实页面预算，对切换与清理路径作限额 profile，再决定是否深化操作内 removal/protection 模块；本测量不能把全部耗时归因于逐写复验，也不支持更换 LRU、放松归属证明或新增 runtime。

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
