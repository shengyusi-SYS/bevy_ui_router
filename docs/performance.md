# Router 性能基线

这是 MVP 的小规模诊断工具，不定义新的 API 合同或性能 SLA。实现位于 [`router_baseline.rs`](../crates/bevy_ui_router/benches/router_baseline.rs)，只调用 facade 公开入口。方法依据见[同类项目调研](06-Router-performance-benchmarks.md)。

## 运行

从仓库根目录执行，默认每个场景 128 个样本：

```sh
cargo +stable bench -p bevy_ui_router --bench router_baseline --features bsn --locked > baseline.csv
```

需要更多样本时设置环境变量 `ROUTER_BENCH_SAMPLES`（正整数），例如 Bash：

```sh
ROUTER_BENCH_SAMPLES=512 cargo +stable bench -p bevy_ui_router --bench router_baseline --features bsn --locked > baseline.csv
```

只验证夹具和断言、不给出性能结论：

```sh
cargo +stable test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

CI 运行 smoke 检查，不做耗时门禁。正式比较使用优化构建，固定机器、依赖、工具链、feature 与构建参数，避免同时编译或运行其他重负载；至少重复运行两次。结果写到不同文件，以免覆盖基线。不要把 debug smoke 与 bench 数据比较。

## 测量范围

夹具安装 `TaskPoolPlugin`、`AssetPlugin`、`ScenePlugin`，注册真实 BSN 工厂与嵌套 Router，创建 app-owned host、布局、工具栏、内部 outlet 和页面。通过 `RouterNavigation` 提交请求，以普通 `App::update` 驱动，并经 `RouterSnapshots` 判断完成。页面分别包含 1、32、128 个平铺 Node 子节点；路由树固定为一个父路由和两个子路由，不测路由表规模。

没有安装 UI layout、原生 picking、GPU renderer、窗口、I/O 或异步加载。这里的时间是 `Instant` 测得的 **CPU 侧工作经过时间（wall-clock）**，包括调度和抢占，并不是线程 CPU 计数器，也不是屏幕显示延迟。运行时输入授权刷新在内，真实设备输入管线不在内。

| CSV 场景 | 初始状态与计时边界 |
| --- | --- |
| `idle_control` | 同样 authored Node 内容、同样基础插件，不安装 Router；不含 Router wrappers。预热 64 帧后测普通 update，作为应用调度对照，不能精确分解单个 Router 系统 |
| `idle_one_request` | 首次导航后，无请求提交；一个保留请求，history 一项 |
| `idle_retained` | 32 轮 replace b / replace a 后，无请求提交；实际验证发生请求淘汰，与上一项保持相同当前页面和一项 history。差值是整个 Router 的保留状态影响，不是某个系统的独立计时 |
| `push` / `replace` / `back` | 每轮从 a 出发，push b、replace c、back a；history 始终至多两项。计时从提交请求前到请求终结、无 running request 且 foreground cleanup 清空；包含公开完成轮询 |
| `failure` / `recover` | 工厂主动返回 Build 错误，发布 fallback，再 replace a；失败不得改 history，恢复后 fallback 实体消失。单独预热 32 轮，不代表所有 Apply/异步失败类型 |
| `soak_cycle_operations` | 新夹具正常导航预热后，连续八批相同的 push/replace/back，每批报告所有三种操作的样本分布；检查活实体数和实际保留请求数稳定。不能将混合分布解读为某种操作的 p95 |

导航样本不计 App/路由初始化、目标字符串构造、详细断言和 CSV 输出；BSN 构建、内部校验、publication、旧页面清理和 snapshot 发布都在计时内。每次正常导航在计时后验证目标页面、history、旧实体消失、父布局与工具栏状态保留、输入授权、内容数量和活实体数。失败和恢复分别验证错误来源、fallback 授权及 history/实体约束。32 次 update 的完成上限是夹具防挂起断言，不是性能阈值。

空闲帧每个样本连续运行 32 个 update，再除以 32；其 p50/p95 是**批次均值**分布，不能当成逐帧尾延迟。导航的 p50/p95 使用逐次操作的 nearest-rank 分位数。`max_us` 保留调度噪声，不自动剔除离群值；`min_updates/max_updates` 记录完成所需 update 次数。CSV 的数量列在计时外读取，表示场景组结束时的稳定状态，不是峰值；failure 行的数量在对应恢复完成后读取。时间单位为微秒。

重复使用仅检查公开可见的实体、history 和请求保留，**不等于已测堆分配或证明无泄漏**。夹具自身保留本次运行的请求 ID 以经公开接口统计保留数量，这些测试记录不属于 Router 存活内存。需要诊断内存时再单独使用分配 profiler，避免插桩污染耗时结果。

## 如何使用结果

先比较同场景的绝对耗时、update 数和重复运行波动；再观察节点数量、请求保留及连续使用的趋势。只有稳定、相关的成本出现后，才对相应 Router 系统做 profiling，决定局部重构或专项微基准。不要以一次测量推断复杂度、以空闲调度对照推断全应用 FPS，或为追求数字删掉 lifecycle 校验。

本工具不增加全平台性能矩阵、跨框架排名、共享 CI 百分比阈值或性能服务。测量范围之外的 GPU、真实输入、异步等待、路由数量及多个 Router 并发成本未验证。

## 2026-09-24 初始结果

两轮完整优化构建运行均正常退出，所有行为断言通过。每轮每个普通场景 128 个样本；每个 soak 批次 128 轮 push/replace/back，共 384 个操作样本。两轮共 96 行汇总数据，保存于[原始 CSV](performance-baseline-2026-09-24.csv)；保留了所有最大值，没有剔除调度离群值。

补充验证：优化构建与 debug 构建的 `--test` smoke 均完整通过，`public_navigation_journeys` 36 项测试通过（0 failed / 0 ignored）。独立审查核对了公开入口、计时边界、状态可比性和 CSV 汇总。CI 步骤已配置并在本地执行对应命令，远端 CI 未触发。

环境与复现信息：

- Linux WSL2，`6.18.33.2-microsoft-standard-WSL2`，`x86_64-unknown-linux-gnu`；宿主报告 AMD Ryzen 9 9950X，当前环境可见 24 个逻辑 CPU。未固定 CPU affinity/频率，也未测其他机器或平台。
- `rustc 1.98.1 (48a229cea 2026-09-01)`，LLVM 22.1.8，官方 Bevy 0.19.1；Cargo 默认优化 `bench` profile，没有自定义优化参数。
- 运行时代码基于 `741070c`，此次只增加 benchmark、CI smoke 与文档，没有性能修补。
- `Cargo.lock` SHA-256：`1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`。
- benchmark 源码 SHA-256：`12f68e697757957e6afd13c6d7eea3552e4e7a97cee7b10fe95025bc5ce35dbd`。
- 两轮分别执行 `CARGO_BUILD_JOBS=2 cargo +stable bench -p bevy_ui_router --bench router_baseline --features bsn --locked --offline`，在构建完成后顺序运行，未同时执行其他 Cargo 验证。两轮使用同一可执行文件。

下表为两轮 p50 的范围；导航范围同时涵盖 push、replace、back，单位均为毫秒。空闲数据是批次均值的 p50：

| 页面子节点数 | 无 Router 空闲对照 | 一个保留请求的空闲帧 | 64 个保留请求的空闲帧 | 正常导航 | 正常导航 p95 范围 |
| --- | --- | --- | --- | --- | --- |
| 1 | 约 0.0034 | 0.0168–0.0174 | 0.0294–0.0298 | 0.178–0.252 | 0.258–0.353 |
| 32 | 约 0.0035 | 0.0384–0.0391 | 0.0618–0.0639 | 2.201–2.610 | 2.386–3.087 |
| 128 | 约 0.0034 | 0.0999–0.1016 | 0.1521–0.1552 | 37.006–38.591 | 38.880–40.807 |

正常导航均在一个 update 完成；Build 失败到 fallback 需要两个 update，恢复需要三个。128 节点的失败 p50 为 34.253–35.960 ms，恢复 p50 为 6.961–7.562 ms；恢复路径比正常切换便宜，但这些操作完成的工作不同，不能混成一个导航指标。

连续使用八批的平均操作耗时如下。每个大小分别保持相同活实体数（52 / 83 / 179）、两项 history 与 64 个保留请求，所有旧页面清理及布局保留断言均通过：

| 子节点数 | 第一轮：第 1 批 → 第 8 批 | 第二轮：第 1 批 → 第 8 批 |
| --- | --- | --- |
| 1 | 0.225 → 0.257 ms（+14.2%） | 0.218 → 0.247 ms（+13.5%） |
| 32 | 2.477 → 3.717 ms（+50.0%） | 2.186 → 3.338 ms（+52.7%） |
| 128 | 37.904 → 56.119 ms（+48.1%） | 36.803 → 55.761 ms（+51.5%） |

**结论：当前不能把功能验收通过等同于性能就绪。** 128 个平铺子节点的单次导航从提交到一次 update 完成已花费约 37–39 ms，尚不含 UI 布局和渲染；对要求流畅帧率的应用，这是应优先处理的实际风险。连续使用的首末批差异在两轮均出现，值得继续定位。环境噪声仍可影响具体数字；这些数据不构成跨平台 SLA、复杂度证明或堆内存泄漏结论。

源码检查提供了两个后续 profiling 候选，**尚未通过采样或对照实验量化各自贡献**：

1. [`RouteEntityRegistry::register`](../crates/bevy_ui_router_ecs/src/registry.rs) 对持续追加的 `consumed_seal_sequences: Vec<u64>` 调用 `contains`。删除 live record 不会删除历史 seal。BSN 离场清理也逐个注册内容实体，因此实体数、history、保留请求稳定并不能保证这个内部历史集合稳定。
2. [`prepare_content_cleanup_inner`](../crates/bevy_ui_router_ecs/src/presentation/native_frame.rs) 在每个内容节点登记、写入后重新验证 protected 集合和完整 cleanup projection；其中 registry record 查询为线性搜索。正常切换和 back 都经由此生产清理路径，存在随内容规模放大的重复工作。

下一步应先定位上述历史查询与 cleanup 重复验证的成本，评估是否能在保留 exact identity、单次消费和写后复验语义的前提下简化内部结构。当前证据不支持优先优化 matcher，也不支持先引入跨平台性能平台或随意减少校验。
