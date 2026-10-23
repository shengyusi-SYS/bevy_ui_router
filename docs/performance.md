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

## 初始动态页面对照

现有夹具的 `steady` 模式只测首次导航后的空闲路径，避免以反复导航或大量请求记录解释初始页面成本：

```sh
ROUTER_BENCH_MODE=steady ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > initial-steady.csv
```

每组 10 行：128/1024 个普通内容节点各测无 Router 控制、两层及三层 native frame 的初始页，以及同页增加一个普通 `RouteScroll + ScrollPosition` 后的空闲更新。三层仅增加透明 pathless layout，内容预算相同；内容通过公开 World 在 publication 后动态创建。仅一条请求、一项 history，不使用 keep-alive。`nested` 沿用四层内容树；`flat` 可按需作形状对照。普通样本数和 32 update 批均值沿用现有方法。

这个对照隔离路由归属和输入/滚动观察，不安装 UI layout、camera、picking 或 renderer；它不能测布局采样、视觉滚动或整应用开销。行为 smoke 将 `cargo bench` 换为 `cargo test` 并加 `-- --test`。

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

本节根 workspace 的 headless benchmark 默认命令没有启用 `bevy_ecs/multi_threaded`；其 CPU 结果不能证明多线程 executor 的调度等待已消除。窗口 consumer 独立启用 MT，比较前须核对实际 compiler artifact 的 features。

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

## 私有 focus 调度实测

2026-10-18 用真实窗口示例的同一份优化二进制，在运行前仅切换 `NativeFocusDrainBoundary` 的 Multi/Single executor；保留原 focus pipe、其它 schedules、UI layout 和 renderer。按 Multi→Single→Single→Multi 串行完成两轮，每组预热 128 帧、记录 512 帧，共 2048 个逐帧样本。源码、实际 MT features、二进制 hash、重建方法和全部数据见[数据身份](data/performance/README.md#focus-调度-2026-10-18)。

| 指标 | Multi 第 1 / 2 轮 | Single 第 1 / 2 轮 |
| --- | --- | --- |
| 私有边界 p50（µs） | 7.730 / 8.070 | 5.410 / 6.020 |
| 私有边界 p95（µs） | 13.021 / 12.780 | 9.480 / 10.830 |
| Main executor p50（ms） | 3.021 / 3.100 | 2.980 / 3.079 |
| Main executor p95（ms） | 3.641 / 3.681 | 3.489 / 3.738 |

边界 p50 两轮分别减少 2.32 / 2.05 µs。Main 的变化仍混有运行波动，第二轮 p95 反向，不能归因出稳定的整帧收益；Single 边界最大值 49.381 µs 也高于 Multi 的 35.311 µs，全部保留。这支持为单个 exclusive system 的私有 schedule 明确选择 Single，改动收益小、实现成本低，不构成整体调度重构的依据。

边界由两个固定 Single 探针夹住完整 private Schedule，绝对值还含固定探针开销。Main 原本就是 Single，测量委托其 executor；不含 Main 外层初始化、render 子应用或呈现。四组首末与跨组页面、活实体、正尺寸布局节点及全部滚动坐标一致，原官方 drain 确被一个正常参数 pipe 替换。该窗口使用 WSLg 软件渲染且处于空闲页，没有连续滚动输入；结果不能用于硬件 GPU、移动设备、滚动 FPS 或纯 Router 总成本结论。

## 空闲滚动 Prepare 节点诊断

2026-10-18 在真实 MT 窗口上，用同一优化 binary 对照完整管线与仅移除空闲 Prepare 节点。保留正常 Settle 的完整归属复验、布局采样、focus 和其它 UI 系统；夹具只执行初始 Push，无 Back/恢复意图。每组预热 128 帧、记录 512 帧，顺序 full→without→without→full；[完整 2048 行及可重建来源](data/performance/README.md#空闲-prepare-2026-10-18)保留全部样本。

| 指标 | 完整管线第 1 / 2 轮 | 移除空闲 Prepare 第 1 / 2 轮 |
| --- | --- | --- |
| PostUpdate executor p50（ms） | 1.573 / 1.600 | 1.528 / 1.558 |
| PostUpdate executor p95（ms） | 1.952 / 1.993 | 1.955 / 1.989 |
| Main executor p50（ms） | 2.910 / 2.987 | 2.919 / 2.964 |
| Main executor p95（ms） | 3.475 / 3.584 | 3.491 / 3.642 |

PostUpdate p50 两轮分别减少 44.656 / 41.992 µs，但 Main p50 一轮增加 8.994 µs、一轮减少 22.520 µs，Main p95 均未改善；尚无稳定主世界收益。该差值包括整个 exclusive 节点的派发、`World::flush`、同步及其它 ready 系统的调度变化，不能拆称纯 barrier 时间，也不是 typed 方案收益的严格上限。

这是固定位置的空闲诊断，没有连续滚动输入。两侧核对路由/history、Ready、marker 身份/原始位置，以及活实体、Node 和正尺寸布局节点的计数稳定；没有比较完整几何或实体集合。计时委托原 Multi PostUpdate 与 Single Main executor，不含外层 schedule 初始化和 render 子应用/呈现。环境仍为 WSLg 软件渲染。生产中没有删除或跳过 Prepare，临时窗口副本已移除。

### 窄读取方案的取舍

Bevy 0.19.1 的 Multi executor 先 `can_run` 再 `should_run`；exclusive 系统即使最终 `run_if=false`，也要先等正在运行的系统结束。`ParamSet<(&World, …)>` 则声明读取全部数据，仍与所有写入冲突。这两条路线都不能替代真正窄访问。

正确的窄 reader 应让现有 World 入口与 Query/Res 入口共享同一套只读判定；同次普通系统内完成观察、数值结算和记录采样，不新增队列、跨帧证明缓存或 authority 来源。访问必须保留 host/slot/manifest、native outlet 唯一性、retained 的输入/focus 保护及 legacy 值验证。结构读取须覆盖原 World 可见的 disabling 实体，正常 eligible/layout 查询则保持 Bevy 过滤；manifest 可用 Entities/Archetypes/Components metadata，不必读取全部应用组件值。基础机制来自现有 Bevy，无需增加依赖。

这项迁移跨多个只读证明模块，粗估 **2–4 人日、风险中等**；不能靠只改 scroll 的参数完成。当前诊断没有建立足以抵偿迁移成本的整体运行收益，因此延期跨层窄读取迁移，也不补 `run_if` 或全 World 参数中转。局部读取减法可以独立评估。后续只有在同一 MT 消费入口确认相关等待占比后，再比较完整最小候选；本诊断不否定其它页面的收益，也不为发布增加平台门禁。

## Retained 权限读取对照

2026-10-18 对 retained 权限相交、Settle 查询元数据缓存及其首次调用初始化做三组独立 MT 对照。使用现有 keep-alive 模式、flat、1/32/128 内容预算，每场景 128 样本；导航预热 32 个循环，idle 另预热 64 次 update 后按每样本 32 update 采样。源码、二进制身份与全部 288 行见[数据说明](data/performance/README.md#retained-读取-2026-10-18)。本组显式启用 MT，不与旧默认非 MT 结果直接比较。

最终只保留 retained 权限相交：完整 shell 闭包仍检查实体存在、去重和子级回链，再复用已有集合与 gate 权限及两种 focus 判交。这部分检查由逐后代扫描权限的 O(R×A) 改为 O(R+A)，R 为闭包大小、A 为权限数量；其它 authority/内容证明保持原样。没有新增集合、跨帧缓存、公共 API 或调度层。

查询缓存组合的两轮 `disabled_replace`（128 内容节点）p50 分别增加 356.076 / 301.647 µs；改为首次 Settle 初始化后仍增加 364.138 / 986.490 µs。预热早于计时，不能用初始化时机直接解释稳态差异；具体原因未确认，也不能据此推导 QueryState 普遍较慢。本轮撤下整个 eligible/marker 缓存及其额外过滤器元数据维护，保留既有滚动布局 SystemState。

仅权限相交的第 2 轮采样期间存在外部并行 Cargo 编译，整对 48 行标为 `contended_pair`，不作收益估算。未重叠该已知干扰区间的第 1 轮同样有利有弊：下表为 LRU idle 的批均值分位数，单位 µs，不能解释为单帧尾延迟。

| 内容节点 | before p50 | after p50 | before p95 | after p95 |
| --- | --- | --- | --- | --- |
| 1 | 508.840 | 511.142 | 522.843 | 537.589 |
| 32 | 529.589 | 606.523 | 549.670 | 651.320 |
| 128 | 612.026 | 599.131 | 641.085 | 632.848 |

128 节点 LRU idle p50 减少 12.895 µs（约 2.1%），32 节点反而增加 76.934 µs（约 14.5%）。同一有效配对的 128 节点导航也混合：disabled/miss/LRU Replace 的 p50 分别增加 231.466 / 21.360 / 201.095 µs，hit 减少 19.200 µs。受干扰轮次不能用来否定这些不利结果。

采用依据是低维护成本、可直接证明的重复扫描减法，**整体 CPU 收益尚未证实**；不认领普遍加速，不为此继续扩大计时矩阵。数据包含全部较慢组与最大值，未合并不同运行的分位数。该 headless 夹具没有 UI layout、picking、窗口、GPU 或持续滚动输入，不能用于滚动 FPS 或实际应用帧率结论；跨层窄读取迁移仍按上节取舍延期。

## 初始动态页面实测

2026-10-18 对同一只读阶段的 native 内容证明复用、Entity 集合与滚动 marker 查找做两轮 before/after 优化构建对照。使用上述 `steady` 模式、nested 四层内容树，每场景 128 样本；完整 40 行、源码与二进制身份见[数据说明](data/performance/README.md#初始动态页面-2026-10-18)。下表为 1024 个动态内容节点时 idle 批均值 p50 的两轮范围，单位 µs。

| native frame 层数 | 普通滚动 marker | before p50 | after p50 |
| --- | --- | --- | --- |
| 2 | 无 | 303.556–307.139 | 144.119–144.275 |
| 2 | 1 个 | 477.428–493.098 | 182.472–186.057 |
| 3 | 无 | 311.020–316.746 | 149.579–150.385 |
| 3 | 1 个 | 493.244–512.030 | 198.233–199.410 |

按各轮配对计算，1024 节点无 marker 的 p50 降低约 52–53%，带 marker 降低约 60–62%；128 节点的四种 Router 场景降低约 46–58%。无 Router 对照 p50 保持在 3.4–3.6 µs。每组样本数、update、实体与请求数量一致；仅一条请求，收益无需依赖缓存或历史记录积累。增加一个透明 layout 的成本小于本夹具的重复内容观察成本，但不能由此判断真实应用布局的成本。

优化复用当前 World 借用内已经证明的内容和切口事实，仍分别检查值、claims 与完整 focus 范围；滚动候选先判重复 key，再筛布局资格。动态内容在下一次观察重新读取，正常采样和 publication 边界保持。这组结果只支持库内 CPU 侧观察成本下降：夹具没有 UI layout、原生 picking、窗口或 GPU，也没有持续视觉滚动，不能解释设备 FPS 或承诺相同比例的帧率改善。

## 稳态执行实测

2026-10-17 继续对空目标隔离、空工作资源作用域、内容观察复用和 focus 安装核查做两轮交替 before/after 对照。完整 192 行、源码和二进制身份见[数据说明](data/performance/README.md#稳态执行-2026-10-17)。沿用同一 flat 夹具、1/32/128 内容预算和每普通场景 128 样本；下表为 128 内容节点时 idle 批均值 p50 的两轮范围，单位 µs。

| 场景 | before p50 | after p50 |
| --- | --- | --- |
| 无 Router 的调度对照 | 3.453–3.456 | 3.451–3.487 |
| 1 条终态请求记录 | 68.123–73.720 | 51.101–51.788 |
| 64 条终态请求记录 | 102.330–104.315 | 85.672–90.309 |

按各轮配对计算，两种 Router 空闲场景的 p50 分别降低约 24–31% 和 12–18%。页面预算、history、请求保留数量和 update 边界保持一致；Router 组总实体由 179 增为 181，来自固定的 focus 核查资源与 observer，不随请求或 update 增长。计时未通过降频或停止滚动采样取得收益，全部导航与连续操作数据也保留在 CSV 中，不据此推导所有导航或尾延迟普遍改善。

这些时间不含 UI layout、原生 picking、窗口、GPU 或 I/O，也不能推导整应用 CPU 或功耗变化。新增真实布局、滚动、focus 与 retained 共存用例通过的是[行为验证](development/verification.md#已有证据与未验证范围)，没有对应场景的计时结论。

## 空闲更新实测

2026-10-17 对输入/滚动只读观察与快照增量更新进行两轮 before/after 优化构建对照。使用相同公开 `router_baseline`、flat 形状、1/32/128 内容预算和每普通场景 128 样本；完整 192 行、源码与构建身份见[数据说明](data/performance/README.md#空闲更新-2026-10-17)。下表是 128 内容节点时两轮 idle 批均值 p50 的范围，单位 µs，不是单帧分位数。

| 场景 | before p50 | after p50 |
| --- | --- | --- |
| 无 Router 的调度对照 | 3.407–3.471 | 3.378–4.380 |
| 1 条终态请求记录 | 98.553–99.344 | 67.897–70.859 |
| 64 条终态请求记录 | 150.040–154.012 | 103.592–104.451 |

两种 Router 场景均为 179 个活实体、一项 history，before/after 数量与 update 边界保持相同；64 是请求记录数，不是缓存页面数。该预算下空闲 p50 下降约三成。未更改 lifecycle、真实写后复验或正常滚动采样，也未通过降低更新频率取得结果。

1/32 节点的结果不一致，首轮 after 的调度对照和其它场景也有较大波动；全部输出与离群值保留，不据此宣称小页面、导航或尾延迟普遍改善。该夹具不含 UI layout、原生 picking、窗口、renderer 或 I/O，不能推导整应用 CPU、持续重绘或功耗的改善幅度。

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

复用减少真实构建和销毁，收益成立；驻留有界也成立，但不证明 heap 无泄漏。256 个活动 authored 内容节点的 fresh/依赖淘汰累计耗时不低，且尚不含 layout/render；这些操作跨 3 次 update，不能拿总耗时断言单帧已超过 60 Hz 的 16.7 ms 参考预算，也不能宣布复杂页面性能就绪。下一步只按真实页面预算，测最慢单次 update 与切换/清理热点占比，再决定是否深化操作内 removal/protection 模块；本测量不能把全部耗时归因于逐写复验，也不支持更换 LRU、放松归属证明或新增 runtime。

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
