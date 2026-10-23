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

## 当前 history entry 读取

滚动观察与恢复查询现在直接读取 Core 的当前 entry key。此前为取得一个 key，先构造包含全部 history keys 的投影；新入口仍从同一个 canonical history cursor 读取，不引入缓存或第二个状态来源，将这一步从 O(H) 复制与分配降为 O(1)。完整 history 投影继续服务需要全部 keys 的调用方。

采用现有 `steady`、nested、每场景 128 样本，按 before→after→after→before 完成两轮串行优化对照，完整 40 行及构建身份见[数据说明](data/performance/README.md#当前-history-entry-读取)。夹具只有一项 history，因此不用于宣称长 history 的实际收益。带 marker 的 128 内容节点场景，两轮 idle 批均值 p50 分别降低约 0.6–1.8%；1024 节点中，两层 frame 第二轮由 177.769 增至 178.560 µs，其余配对下降。小幅变化可能混有运行波动，不能认领稳定的整帧加速。

保留依据是读取接口与调用需求一致、确定消除了完整投影分配，改动局部且无新状态。同类审查还将 Back 入口与两处 host admission 判断中的 `snapshot().availability()` 改为已有 `availability()` 借用，避免为了一个枚举克隆完整 history、匹配、诊断与 host trace；这三处没有独立耗时收益结论。既有滚动、history、生命周期与独立 consumer 回归仍验证实际行为；此对照不含 UI layout、picking、窗口或连续滚动输入，不代表 FPS。

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

当前代码中，正常清理在每次真实删除后重新检查完整 target/protected 事实，见 [removal plan 与执行](../crates/bevy_ui_router_ecs/src/cleanup.rs)。此前清理分类、普通内容临时登记和历史 seal 列表造成的重复工作已减去；逐写复验仍保留。历史耗时支持优先定位清理，但不能证明当前版本各步骤的占比，也不能仅靠几个数据点推导全库复杂度。

## 清理与输入准备热点

2026-10-24 在独立源码与 target 上给既有 flat keep-alive 夹具加入临时 scope 计时，每场景 32 样本，记录逐 update 耗时。初次定位后进一步拆开保护复验；两次全部阶段的记录、可重放差异与构建身份见[诊断数据](data/performance/README.md#清理与输入准备-2026-10-24)。下表取细分诊断的 128 内容节点组，为对应 scope 累计耗时占计时 update 总耗时的比例。

| 路径 | 默认关闭 / fresh | 满容量 LRU | 缓存命中 |
| --- | --- | --- | --- |
| removal settlement（含以下清理子项） | 25.0% | 39.2% | 不执行 |
| protected 字段复验 | 13.4% | 30.1% | 不执行 |
| 构建已删除 receipt 集合 | 2.4% | 1.8% | 不执行 |
| target 复验 | 6.1% | 4.8% | 不执行 |
| 输入权限准备 | 0.6% | 0.5% | 11.0% |

scope 是含子调用的 wall-clock，不能相加；探针也有开销。这是热点定位，不是优化前后比较或完整 CPU 归因。初次诊断的 focus 收集约占 fresh/LRU 更新的 0.2%，不足以支持独立优化。

据此尝试在一次 protected 比较中只解析一次 `EntityRef`，其后从同一只读引用读取各组件；每次写入后仍完整重验。无插桩产物四轮配对后**撤回**：首轮 baseline 连未受影响的 idle 都明显变慢，整对标记不稳定；其余三轮 128 节点 LRU p50 变化为 −3.0%、−0.1%、−7.9%，最后一轮 idle 同时减少 5.5%。没有稳定证据支持增加该实现。全部 192 行和候选差异保留，不以首轮近 50% 的变化宣称收益。

输入准备另测了两版：完整版本将准备与共用撤权中的查找改为临时 `EntityHashSet`；收窄版只改导航增删准备，恢复原撤权函数。每版均有两轮 flat/nested、128 样本的完整对照。[全部 384 行及候选来源](data/performance/README.md#输入准备候选-2026-10-24)均保留。完整版本在 128 节点命中时 p50 降低 7.9–12.7%，但 LRU 增加 5.9–7.1%；收窄版三个稳定配对的 LRU 增加 8.1–10.0%，最后一对因整体波动标记不稳定。**两版均撤回**，包括其专用新增测试，不以局部命中收益换取淘汰回退。

独立诊断中，完整版本的输入准备从约 45 µs 降至 9 µs；但其它 scope 也变化，无法由此确定正式 LRU 回退的原因，不归因为某个 allocator 或编译器行为，也不继续叠加阈值或特殊分支。补测的 presentation 全 World 访问在小 World 基线中约占 LRU 的 0.25%、命中的 3.3%；该夹具最多 441 个活实体，只支持暂缓本轮改造，不能据此排除无关 World 实体增多后的收益。

| 方向与最小边界 | 工作量、风险与决策 |
| --- | --- |
| 输入准备使用操作内实体集合 | 两版均已测并撤回；命中改善，但 LRU 稳定回退。保留原实现，暂不继续更换容器 |
| presentation 与 pointer 改用 Bevy 定向实体查询 | 已按[规模对照](#按目标读取-presentation-与-pointer)采用；保留原权限判定与实时观察，消除无关 World 实体的扫描成本 |
| Registered target 借用组件 ID，避免临时排序 manifest | 0.5–1 人日，中风险；target 全部复验仅约 5–6%，其中普通内容不走该分支。收益上限有限，暂缓 |
| 完整 ledger 的 receipt 索引 | 0.5–1 人日，中风险；现有 plan 不覆盖已结算项，另建索引引入维护负担，而对应成本仅约 2%，不采用 |
| focus 临时容量、一次比较内 EntityRef 合并 | 低成本；前者占比低，后者对照无稳定收益，不继续 |
| 滚动 publication 全记录克隆与线性匹配 | 0.5–2 人日；现有基线只有少量 history/容器，未证明扩大 prepared delta 的收益，暂缓 |
| 滚动 Prepare/Settle 的窄 Query/Res 迁移 | 2–4 人日，中风险；沿用[已有调度取舍](#窄读取方案的取舍)，尚无稳定整帧收益依据 |

Core matcher 的分支克隆与最大深度重算需要更大真实路由表的证据；当前夹具不能支持扩大匹配器重构。快照 revision 或全局索引会增加同步状态，现有等值不写与窄读取已处理明确浪费。测试已删除重复 consumer 和源码守卫，继续缩减将触及独立行为保护；尚无构建瓶颈证据，不新增构建框架。

这些方向均可使用标准库和现有 Bevy 能力，不需增加依赖。目标观察与保护复验分别承担不同职责，不能为了减少循环合成一个更弱的判定；观察只能在一次只读操作内复用，实际写入后重新读取。

当前没有真实页面预算支持新增全局缓存、分帧销毁、第二套 runtime、全平台 benchmark 或更多容器微优化。60 Hz 的 16.7 ms 是整个帧的参考预算，不是 Router 可独占的预算。

## 完整导航阶段与 Scene 声明拓扑

在 `11e8935` 的独立副本补齐十个系统阶段、Scene 创建/接管和 BSN prepare/resolve 计时，覆盖 keep-alive 与父子库存、flat/nested、每场景 32 样本。[全部 7,944 个逐 update 记录、探针与身份](data/performance/README.md#完整导航阶段-2026-10-24)包含初始化、预热、重置和退出；其中正式 measure 为 2,304 行。导航另记录 enqueue、每次 update、累计 update 边界及原请求完成时间。

以下为 128 内容预算下，scope 累计耗时占对应完整导航累计耗时的范围。父子库存的父与子各有 128 个内容节点。Scene 与父循环属于 Drive 内的嵌套项，不能相加；父循环包含期望关系构造、实际 Children 读取和比较。

| 路径 | 完整导航均值 | Scene 创建占比 | 声明父循环占比 | SettleCleanup 占比 |
| --- | --- | --- | --- | --- |
| leaf 默认关闭 / fresh | 7.61–7.92 ms | 49–50% | 21–22% | 37–40% |
| leaf 首次 miss | 4.35–4.37 ms | 87–89% | 38–39% | 约 0.2% |
| leaf 满容量 LRU | 9.94–10.04 ms | 39% | 17% | 49–51% |
| 原父、新子 | 7.80–7.99 ms | 47–48% | 20–21% | 32–33% |
| 父子均 fresh | 19.12–19.40 ms | 39–40% | 17–18% | 47–49% |
| 父子依赖淘汰 | 23.89–24.29 ms | 32–33% | 14% | 55–56% |

父子均 fresh 和依赖淘汰仍跨三个 update；本次依赖淘汰最大单次 update 为 19.46 / 20.07 ms。命中不创建 Scene，成本主要集中在 Commit（leaf 约 67%、父子约 73–74%）。这些是带探针的阶段诊断，不能当作正式版本收益、屏幕延迟或持续帧率。

声明拓扑中有可直接消除的重复工作：原实现每次释放节点标记后，重新为每个父声明扫描全部声明并构造 expected children。逐写后的实时身份、关系和完整闭包复验仍有必要；声明关系在这次 realization 内不变，可以一次准备有序邻接表及 exact Entity 索引。该边界使用已有 Bevy `EntityHashMap`，不增加跨帧缓存或第二套 authority。

该局部方案完成两轮 flat/nested、keep-alive/父子库存的无插桩配对，每场景 128 样本，[全部 384 行及完整源码身份](data/performance/README.md#scene-声明拓扑对照-2026-10-24)保留。样本数、update、活实体、活动/非活动页和 factory 计数逐项相同。128 内容预算的结果如下，分位数范围为四组各自的 p50，不是合并样本分位数。

| 路径 | before p50 | candidate p50 | 配对变化 |
| --- | --- | --- | --- |
| leaf 默认关闭 / fresh | 7.44–7.51 ms | 5.71–6.09 ms | −18.4% 至 −24.0% |
| leaf 首次 miss | 4.18–4.39 ms | 2.46–2.57 ms | −38.9% 至 −43.8% |
| leaf 满容量 LRU | 9.57–9.96 ms | 8.16–8.54 ms | −10.8% 至 −15.9% |
| 父子均 fresh | 19.10–19.65 ms | 16.02–16.93 ms | −13.5% 至 −16.1% |
| 原父、新子 | 7.69–7.95 ms | 5.97–6.24 ms | −20.7% 至 −23.6% |
| 父子依赖淘汰 | 23.46–24.18 ms | 21.51–22.94 ms | −5.1% 至 −9.1% |

代价也保留：leaf 命中 p50 从 0.381–0.395 ms 变为 0.398–0.422 ms，各对增加 4–27 µs（1.1%–6.9%）；父子命中变化 −0.5% 至 +4.0%。命中没有执行 Scene 创建，原因尚未确定，不能归因为拓扑查询本身或声明其完全不受影响。小页面和 idle 未显示一致改善。选择这项局部减法的依据是中等内容量 fresh/miss/淘汰路径稳定节省约 1–3 ms，且去掉声明关系的重复构造；不宣称每条路径均变快。

阶段覆盖比此前局部清理/输入诊断更完整，但尚未覆盖 heap 分配与释放、多 Router 耦合、大 history/路由表和异步等待。无关 World 实体另作下述有界诊断。单个小夹具的低占比不等于其它方向没有优化价值；也不能从实体数稳定推导堆内存稳定。

## 无关 World 实体与清理准备

继续在 `11e8935` 独立副本给原探针增加 removal plan / receipt batch 两个 scope，并添加默认关闭的普通背景实体开关。两轮按 0→2048→8192、8192→2048→0 串行运行 flat keep-alive，每场景 32 样本；背景实体只有 Node 与独立 marker，不归 Router，初始化和退出均验证完整保留。[全部 11,832 行、补丁与身份](data/performance/README.md#world-规模与清理准备-2026-10-24)包含父子库存的一次补充清理诊断。

固定 128 个内容节点时，缓存命中的两个诊断均值如下。数据带插桩，只说明规模耦合，不是优化效果。

| 无关实体数 | presentation 准备 | 完整导航 |
| --- | --- | --- |
| 0 | 13.4 / 13.6 µs | 408 / 403 µs |
| 2048 | 42.0 / 42.6 µs | 472 / 474 µs |
| 8192 | 133.0 / 132.4 µs | 714 / 683 µs |

原 `prepare_presentation_access` 的全可选组件查询会遍历整个 World，然后线性查找 target。实际 mutation 只需要 target、pointer targets 和明确的资源 Entity；现有 Bevy 唯一实体迭代可把遍历限制在这个集合，同时保持默认过滤、disabled pointer、目标缺失/重复拒绝及原 target 顺序。输入撤权的 `prepare_pointer_mutation` 也有相同的全 World 查询结构：反序组的 InputRefresh / InputFilter 从约 53 / 6 µs 增至 129 / 82 µs。完整导航增量不能全部归因于 presentation；两个职责应独立验证。

清理方面，128 父子依赖淘汰的 removal plan 准备约 1.21 ms，其中 receipt batch 约 0.66 ms；leaf LRU 对应约 0.46 / 0.24 ms。源码确认同一只读准备区间为每个 target 重复比较完整 issued attempt，再创建单项 receipt 去重集合。一次准备整批 receipt、再分别线性消费，是可核验的局部方向；它与已撤回的 EntityRef 读取合并不同，也不能据此省去删除后的完整 World 复验。该补充诊断只有一个父子运行，不把所有 SettleCleanup 剩余成本归到 receipt。

背景实验只改变实体数量，未覆盖 archetype 多样性、应用系统或 MT 竞争；不因此引入新的运行框架或全局索引。

## 按目标读取 presentation 与 pointer

两处准备使用现有 Bevy `iter_many_unique_mut`，只借用已授权 targets、pointer targets 与 exact resource Entity。presentation 同时用操作内 Entity 索引替代逐实体线性查找；缺失/重复 target 仍拒绝，返回顺序仍按原 target 顺序，disabled pointer、只读准备与 publication tail 的边界不变。没有跨帧缓存，也没有更换原 pointer membership 集合。

在 `6b20ed9` 上分别比较 presentation、pointer 和组合，背景数0/8192、flat、每场景128样本，各两轮反序运行。[完整384行与重放来源](data/performance/README.md#定向查询-2026-10-24)保留全部结果。8192个无关实体、128内容节点时，组合结果为：

| 路径 | before p50 | 组合 p50 | 配对变化 |
| --- | --- | --- | --- |
| fresh | 6.054 / 6.065 ms | 5.744 / 5.804 ms | −5.1% / −4.3% |
| 首次 miss | 2.727 / 2.714 ms | 2.429 / 2.491 ms | −10.9% / −8.2% |
| 缓存命中 | 739 / 722 µs | 387 / 375 µs | −47.6% / −48.1% |
| 满容量 LRU | 8.608 / 8.519 ms | 8.207 / 8.250 ms | −4.7% / −3.2% |

两项单独修改也减少该规模下的导航成本；组合使命中接近无背景的水平。背景为0时，128节点命中变化−3.3% / +2.0%，LRU约持平；1节点第二轮命中与LRU分别增加8.0% / 14.4%，部分idle增加约1–6%，不能宣称小场景没有开销。小World的128节点命中第二轮p95也从461增至564µs，全部记录保留。采用依据是消除不相关World规模造成的稳定线性成本，改动限于局部访问准备；未据此增加阈值分支或归因未解释的波动。

## History 源校验的分配减法

Core 的 canonical history 源比较原来先构造 owned snapshot，再复制 locations；成功校验后立即丢弃。现在直接借用既有 cursor/entries，完整比较 incarnation、revision、cursor、entry IDs 和 locations；只有需要保存源或返回失败证据时才构造 owned 值。capability source 与 prepared next 继续独立持有，比较仍逐项核对，不增加 revision 缓存或更改 history 行为。

在独立 Core 副本以固定版本分配计数库测量 1/64/256 条 history，每项预热4次、记录16次，完整 [288 行与可重放来源](data/performance/README.md#history-分配-2026-10-24)保留。下表为每次操作的分配次数，组内16次结果一致；不是完整 ECS 导航计时。

| 操作 | 1 entry：before→after | 64 entries：before→after | 256 entries：before→after |
| --- | --- | --- | --- |
| 冻结完整 owned source | 5→3 | 131→66 | 515→258 |
| 成功重新校验 NoTarget source | 5→0 | 131→0 | 515→0 |
| Replace 的 freeze→prepare→source match→publish | 21→7 | 525→133 | 2061→517 |

256 entries 的 Replace 链累计分配字节从219,196降至65,054；最终区间净分配保持相同。owned source 的最终持有字节仍为32,512，减少的是临时副本；失败仍返回原 capability 和完整 observed evidence。该局部优化不声称整导航耗时按相同比例下降。

计数仅覆盖当前线程、使用非嵌套区间。Replace 会释放区间外创建的输入及旧状态，因此净值可为负，不能解释为进程驻留内存或泄漏。计数依赖与 ignored probe 只保留为诊断补丁，不进入生产 Cargo 配置。

## 窗口诊断的边界

[窗口 CSV](data/performance/performance-window-2026-09-27.csv)来自简单交互示例、两轮优化构建的临时诊断副本：暖机后 BRP 发送真实 WindowEvent/KeyboardInput，交替导航，采样期间不截图或轮询。每次在输入帧观察成功、正确路径、正尺寸布局与相同 layout Entity；最终截图只验证最终页面。

| 指标 | 两轮 p50（ms） | 两轮 p95（ms） |
| --- | --- | --- |
| 切换帧 First→Last 主世界经过时间 | 5.342 / 4.872 | 6.040 / 6.081 |
| 同批距输入至少四帧的对照 | 3.836 / 3.577 | 4.798 / 4.392 |
| 切换帧之后 First→First 间隔 | 10.323 / 10.004 | 12.809 / 11.056 |

该观察使用 Linux/WSLg llvmpipe 软件渲染和 continuous 更新，未测硬件 GPU。First→Last 不含 render 子应用/GPU 呈现；记录工作在结束时间戳后，仍影响下一帧间隔。CSV 行的 interval 对应前一帧之后，切换后的间隔需读 offset=1，不能读输入行 offset=0。

这不是复杂页面验收、屏幕首帧延迟、FPS 承诺或严格 Router 开销。当前可复现的窗口交互方法见[调试指南](guides/debugging.md)，不需要把临时测量探针变成生产 API。
