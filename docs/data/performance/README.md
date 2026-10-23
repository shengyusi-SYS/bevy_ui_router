# 性能测量数据

这里保存测量数据；历史 CSV 原样迁移，新测量注明完整源码与环境身份，不混作现行合同。运行方法、统计边界和结果解读见[性能说明](../../performance.md)。窗口与调度诊断文件为逐帧记录，其余是样本汇总，不能从汇总分位数还原或合并原始样本。

## 数据身份

| 数据 | 数据行 | 用途 | 源码/比较身份 |
| --- | --- | --- | --- |
| [performance-baseline-2026-09-24.csv](performance-baseline-2026-09-24.csv) | 96 | 初始 Router 基线；连续使用退化 | 741070c；优化前 |
| [performance-baseline-2026-09-25-classification.csv](performance-baseline-2026-09-25-classification.csv) | 96 | 分类器小集合改为固定数组 | f44089c + 固定数组修改 |
| [performance-baseline-2026-09-25.csv](performance-baseline-2026-09-25.csv) | 96 | 共享单轮 evidence 集合、seal 注册减法 | 087b99c + 对应局部修正 |
| [performance-baseline-2026-09-27.csv](performance-baseline-2026-09-27.csv) | 48 | 旧版本单轮重测 | 55911d6；48 行 |
| [performance-bsn-control-2026-09-27.csv](performance-bsn-control-2026-09-27.csv) | 54 | 直接引擎创建/删除对照，无 Router 权限语义 | 同一旧依赖/工具链；两轮，batch 0–8 |
| [performance-cleanup-2026-09-27.csv](performance-cleanup-2026-09-27.csv) | 96 | 正式 Registered/ContainedContent 迁移 | 55911d6 + 内容迁移；两轮完整基准 |
| [performance-cleanup-plan-2026-09-27.csv](performance-cleanup-plan-2026-09-27.csv) | 288 | 操作内 removal plan，三轮配对 | base=内容迁移版本；candidate=已采用 removal plan，采用实现提交为 b0dbc3b；测量前局部源码未完整冻结 |
| [performance-cleanup-prototype-2026-09-27.csv](performance-cleanup-prototype-2026-09-27.csv) | 180 | baseline 与有限 normal 原型 | 55911d6 对照；prototype 无 failure/recover，不能视为完整生产版本 |
| [performance-shape-2026-09-27.csv](performance-shape-2026-09-27.csv) | 576 | flat/nested 与读取合并候选，三轮 | b0dbc3b 基线；candidate 未采用；仅 run 1 uncontended_pair 可比较 |
| [performance-window-2026-09-27.csv](performance-window-2026-09-27.csv) | 960 | 简单窗口逐帧诊断，非 Router headless 基准 | b0dbc3b；临时 release BRP/采样副本，两轮 |
| [performance-keep-alive-2026-10-08.csv](performance-keep-alive-2026-10-08.csv) | 96 | 默认关闭、miss/hit、idle、容量淘汰；两轮 flat/nested | `3d6ca2c62a4f1011eb9449e25f1ed237b4c8f35e`；完整已提交 crate/benchmark 源码 |
| [performance-instances-2026-10-11.csv](performance-instances-2026-10-11.csv) | 168 | 父子 fresh、原父/新子、父子命中和依赖淘汰；其中 96 行为两轮完整串行配对 | `dc3ef59e57434f2730fb7a5958e0f660d8294f7a`；完整已提交 crate/benchmark 源码 |
| [performance-idle-2026-10-17.csv](performance-idle-2026-10-17.csv) | 192 | 输入/滚动观察与快照减法；before/after 各两轮完整 flat | before=`a6c0ef29d3c8d70532ed8e6ed7dcb4d8469e791e`；after=`9d80af9be3f20c170b6ee63140924de10dcaf906` |
| [performance-steady-2026-10-17.csv](performance-steady-2026-10-17.csv) | 192 | 空工作执行减法、内容观察复用与 focus 冷路径；两轮交替 before/after flat | before=`9d80af9be3f20c170b6ee63140924de10dcaf906`；after=`e59a3ea0c8a7d6f56fe9dd8a4935fb6beeac3ed1` |
| [performance-initial-steady-2026-10-18.csv](performance-initial-steady-2026-10-18.csv) | 40 | 一条初始请求、动态内容、两层/三层 native frame 与普通滚动 marker；两轮 before/after nested | before=`dab9bf0b042b0c86db6228c82013116500025ac5`；after=`6a0ab58f704e878efd7494c5f66ba92ea3cf956f` |
| [performance-focus-schedule-2026-10-18.csv](performance-focus-schedule-2026-10-18.csv) | 2048 | 真实窗口空闲时私有 focus schedule 的 Multi/Single 对照；两轮逐帧 | `316a6d98e1451da9ef20c2f795683e1dae1fda47` + 完整[探针源码](focus-schedule-probe-2026-10-18.rs)与[窗口差异](focus-schedule-probe-2026-10-18.patch) |
| [performance-scroll-prepare-2026-10-18.csv](performance-scroll-prepare-2026-10-18.csv) | 2048 | 完整 MT 窗口与移除空闲 Prepare 节点的诊断；两轮逐帧，非采用候选 | `3fc65c333cc498dfcf05f975476485a4049aefe2` + 完整[探针](scroll-prepare-probe-2026-10-18.rs)与[差异](scroll-prepare-probe-2026-10-18.patch) |
| [performance-retained-read-2026-10-18.csv](performance-retained-read-2026-10-18.csv) | 288 | retained 权限相交与查询缓存的三组 MT 对照；各两轮，含撤回候选及最后一轮受干扰配对 | 基线 `58e24d2`；组合 `b655f14`、懒初始化 `95de4eb`；仅权限相交 `b7a5140`，完整身份见[本组说明](#retained-读取-2026-10-18) |

上述日期和 revision 是测量身份；部分局部提交在后续整理历史时已合并，不承诺其短 SHA 在当前分支仍可检出。`b0dbc3b7486e71cd41e5a8cd05e84ac031b10ce0` 当前仍可解析，但不在当前分支祖先链内。完整历史测量说明固定保存于 `6de4233e2c636daa5892807b05af8a19d6e2cc0f:docs/performance.md`，可用 `git show` 查询；它记录比较方法，不补足未保存的源码。

只有带可恢复完整源码身份的测量才能重建旧精确树。表中的“revision + 局部修改”、临时 prototype、shape candidate 和 release BRP/采样副本未保留完整源码差异，不能保证精确复现；二进制 hash 也不能补足这一缺口。当前实现不能借旧记录宣称重新计时，新的对照须绑定新的完整源码/lock/工具链身份。

原型、正式迁移、removal plan 以及读取候选分别比较，不能互相替代。shape 的第二/三轮保留 `contended_pair` 标记，整组不用于收益估算；没有删掉较慢配对或最大值。原型只覆盖有限 normal cleanup，其结果不证明完整 failure/authority/residual 通道。

## Keep-alive 2026-10-08

使用完整提交 `3d6ca2c62a4f1011eb9449e25f1ed237b4c8f35e` 的 crate、lock 和 benchmark；编译与计时期间这些文件没有未提交修改。命令为[独立 keep-alive 模式](../../performance.md#keep-alive-对照)，round 1/2 均按 flat→nested 顺序；每组 24 行，128 样本，未过滤离群值。CSV 增加 `round` / `shape` 两列，保留各组全部原始汇总字段。

环境为 Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 可见逻辑 CPU，`rustc 1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0，Bevy 0.19.1。默认优化 bench profile，构建并发 4；窗口已关闭、无并行 Cargo/rustc 或本任务的渲染/采样，未固定 CPU affinity/频率。方法为 CPU 侧 wall-clock，不含 layout/GPU/I/O；idle 是 32 update 批次均值的分布。

`Cargo.lock` SHA-256 为 `1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`；实测 benchmark 二进制 SHA-256 为 `7cc3e143254610b5c790564fc885f81d1c34d6c17811ae345c460327d34f7fef`。版本与历史测量工具链不同，不将二者时间差解释为单一改动收益。

## 父子库存 2026-10-11

源码为完整提交 `dc3ef59e57434f2730fb7a5958e0f660d8294f7a`，生产实现包含父子独立库存。使用[父子库存模式](../../performance.md#父子库存对照)，先按 Cargo `compiler-artifact` 输出确认实际优化 executable，再直接顺序运行同一二进制；每组 24 行、128 样本、64 次导航预热，全部行为断言启用。父与子各自有 1/32/128 个内容节点。

`round=3/4` 的 flat→nested 为两个完整串行配对，标记 `uncontended_pair`，用于结果估算。早期 round 1 nested 与 round 2 flat 因启动重叠，整组标记 `contended`；round 1 flat 标记 `unpaired`，不混入配对范围。所有 168 行及最大值原样保留，没有从较慢组或组内样本中挑选结果；汇总分位数不能还原原始逐操作样本。

Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 可见逻辑 CPU；`rustc 1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0、官方 Bevy 0.19.1、默认 bench profile（opt-level 3，debug assertions 关闭）、bsn feature。本任务正式配对期间没有并行 Cargo/rustc、窗口或 renderer；未固定 CPU affinity/频率，也未控制宿主全部负载。计时为 CPU 侧 wall-clock，不含 layout/GPU/I/O；idle 为 32 update 批次均值。

`Cargo.lock` SHA-256：`1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`；实际 benchmark executable SHA-256：`bcd55bee6885f4911709d2e6163c0f8fa24e94a6f1b93718aefc813b4820e619`。运行前后核对二进制一致，crate/lock/benchmark 无未提交修改。旧 keep-alive 数据的内容预算与请求预热不同，不能将时间差解释为父子实现本身的版本回归。

## 空闲更新 2026-10-17

before 为完整提交 `a6c0ef29d3c8d70532ed8e6ed7dcb4d8469e791e`；after 为 `9d80af9be3f20c170b6ee63140924de10dcaf906`，包含 `ecfb01f` 的输入/滚动观察减法与后续快照优化。benchmark 源码、features、lock 和计时边界相同，两份优化 executable 分别保留后直接运行，运行前后核对 hash。after 构建及计时期间 crate/lock/benchmark 无未提交修改。

顺序为 before round 1→before round 2→after 构建→after round 1→after round 2，编译与计时不重叠；没有本任务的并行 renderer、窗口或插桩。每组完整 48 行，flat、1/32/128 内容预算，每普通场景 128 样本；idle 为每样本 32 次 update 的均值，soak 每批为 384 次操作。CSV 仅附加 `variant` / `round` / `shape`，保留全部原始汇总值；各组活实体、请求保留数和 min/max update 边界逐项一致。

Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 可见逻辑 CPU；`rustc 1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0，官方 Bevy 0.19.1，bsn feature，bench profile 为 opt-level 3、debuginfo 0、debug assertions 与 overflow checks 关闭。未固定 CPU affinity/频率，也未控制宿主全部负载。首轮 after 波动较大，对照与导航尾部也变慢；两轮全部保留，未过滤较慢组或离群值，不用小规模或导航结果推断普遍收益。

`Cargo.lock` SHA-256：`1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`。

| executable | SHA-256 |
| --- | --- |
| before | `5fef98b47f41029b3f37b29870531dbfa6db9d43a27ad1f8a39a9f2dc7f0784e` |
| after | `68322ed332e32f940c357787198deeff67cefa76f77f1d5d919433638e6ace81` |

## 稳态执行 2026-10-17

before 为完整提交 `9d80af9be3f20c170b6ee63140924de10dcaf906` 的保留优化 executable；after 为 `e59a3ea0c8a7d6f56fe9dd8a4935fb6beeac3ed1`，包含 `b873705` 的 focus 调度核查。benchmark、features、lock 与计时边界不变；after 构建与计时期间 crate/lock/benchmark 无未提交修改。两份 executable 在运行前后核对 hash，优化构建由 Cargo 的 compiler-artifact 确认实际文件。

先完成 after 构建，再按 before round 1→after round 1→before round 2→after round 2 串行运行。每组完整 48 行，flat、1/32/128 内容预算，每普通场景 128 样本；idle 为每样本 32 update 的均值。CSV 附加 `variant` / `round` / `shape`，保留全部汇总值、较慢组及离群值。请求保留数、样本数和 min/max update 边界逐项一致。Router 组总实体固定增加 2，对应 focus 核查资源与 `ScheduleBuilt` observer；无 Router 的控制组不变。新增基础实体不随页面预算、请求或 update 增长。

环境仍为 Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 可见逻辑 CPU，`rustc 1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0、官方 Bevy 0.19.1；features 为 default/bsn/ecs/view，bench profile 为 opt-level 3、debuginfo 0、debug assertions 与 overflow checks 关闭。没有本任务的并行 Cargo/rustc、renderer、窗口或插桩；未固定 CPU affinity/频率，未控制宿主全部负载。

这组计时仍不含 UI layout、原生 picking、窗口、GPU 或 I/O。真实布局、滚动、focus 与 retained 共存的新增公开用例是行为验证，不能替代这些场景的性能数据。`Cargo.lock` SHA-256 为 `1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`。

| executable | SHA-256 |
| --- | --- |
| before | `68322ed332e32f940c357787198deeff67cefa76f77f1d5d919433638e6ace81` |
| after | `b2c8411f7ab816d533d4bc92ccc10c87997853cf0b6d4d319a5e15a08b4ece1a` |

## 初始动态页面 2026-10-18

before 为完整提交 `dab9bf0b042b0c86db6228c82013116500025ac5`；after 为 `6a0ab58f704e878efd7494c5f66ba92ea3cf956f`。两份优化 executable 均在源码冻结后构建，由 Cargo compiler-artifact 确认并独立保存，计时直接运行保留文件，运行前后核对 hash。after 构建及计时期间 crate/lock/benchmark 无未提交修改；benchmark、features、lock 和计时边界相同。

顺序为 before round 1→after 构建→after round 1→before round 2→after round 2，编译与计时不重叠。采用[初始动态页面模式](../../performance.md#初始动态页面对照)，nested 四层内容树，128/1024 个动态内容节点，每组 10 行、每场景 128 样本、64 update 预热，每个样本为 32 update 均值。内容在首次 publication 后创建；仅一条请求、一项 history，不保留非活动页面。两层/三层 native frame 的差别是一个透明 pathless layout，内容预算相同。

CSV 仅附加 `variant` / `round` / `shape`，完整保留两轮 40 行、较慢组与最大值；未过滤样本。各组样本数、min/max update、活实体和保留请求数量逐项一致。1024 内容节点时控制组为 1059 实体，两层/三层 Router 组为 1077/1080；本次没有新增基础实体。

环境为 Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 可见逻辑 CPU，`rustc 1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0、官方 Bevy 0.19.1；features 为 default/bsn/ecs/view，bench profile 为 opt-level 3、debuginfo 0、debug assertions 与 overflow checks 关闭。没有本任务的并行 Cargo/rustc、窗口或 renderer；未固定 CPU affinity/频率，未控制宿主全部负载。

这是 CPU 侧 wall-clock 的路由观察对照，不含 UI layout、原生 picking、窗口、GPU 或 I/O。普通 marker 没有布局几何，预期会给出缺布局诊断；真实布局与滚动恢复另由公开行为测试验证。它不测持续视觉滚动，也不能用于设备 FPS 归因。`Cargo.lock` SHA-256 为 `1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`。

| executable | SHA-256 |
| --- | --- |
| before | `2669913ec81e8d6dc7a963faff2b672d39d5e420dcc56ef0977d961f62ccc509` |
| after | `d3b3c50dc2b046cc50778f1acf674e53b945b48f14354972ab766cbe62e9da5a` |

## 历史测量共同环境与统计

- Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`，`x86_64-unknown-linux-gnu`；AMD Ryzen 9 9950X，环境可见 24 逻辑 CPU。
- `rustc 1.98.1 (48a229cea 2026-09-01)`，LLVM 22.1.8，官方 Bevy 0.19.1；默认优化 bench profile，未固定 CPU affinity/频率。
- Router 每普通场景 128 样本，soak 每批 384 混合操作；完整一轮 48 行。直接 BSN 每批 128 replace，batch 0 为初始分布、1–8 为连续使用，不能直接与 Router 混合 soak 分布比较。
- 导航/直接 BSN 为逐操作 nearest-rank 分位数；idle 是 32 update 均值的分布。不同运行的 p50 均值不是合并样本分位数。
- 正式计时不含插桩；受宿主高负载干扰的形状配对已单独标记。没有以 debug smoke 数字代替优化结果。
- window 额外环境：Linux/WSLg，Vulkan llvmpipe（Mesa 26.0.8、LLVM 21.1.8），960×640 简单页面、continuous 更新。每轮暖机 120 帧，40 次交替 push，相隔 12 帧；没有硬件 GPU 或真实设备性能结论。

## 测量构建身份

历史测量的 `Cargo.lock` SHA-256：

```text
1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8
```

初始基线至 removal plan 的 Router benchmark SHA-256：

```text
12f68e697757957e6afd13c6d7eea3552e4e7a97cee7b10fe95025bc5ce35dbd
```

形状测量的 benchmark SHA-256：

```text
8c10b384191ee02304212075e8a0697efe5b2e5b538dd17a7d3f56ea64afe042
```

形状测量已核对的 base/candidate 二进制 SHA-256（用于识别当时产物，不是可重复构建承诺）：

```text
base      fa2e63a4f89db794577aa04a43050ddd07e0a2605f4713d730881e35ea542f0b
candidate 8d82769f351ecd8ca269f116179fc5ccc13e281fbc4bb34f95c8e1413c2504bf
```

这些 hash 不能单独还原整个 runtime；必须结合表中源码身份与 Git 中原测量说明。共享 target 曾复用诊断产物的输出已作废，未计入这些 CSV；正式比较使用核对后的独立产物。

## Focus 调度 2026-10-18

完整生产源码固定为 `316a6d98e1451da9ef20c2f795683e1dae1fda47`；[身份记录](focus-schedule-identity-2026-10-18.json)保存工具链、lock/source/probe/实测 binary 的 SHA-256、实际 compiler artifacts、运行顺序和四组相同的完整 shape。生产代码在采样期间与该提交一致。Linux WSL2 / Ryzen 9 9950X / 24 可见逻辑 CPU，Bevy 0.19.1；ECS 与 tasks 的实际 artifact 均启用 `multi_threaded`，opt-level=3、无 debug assertions。WSLg renderer 报告软件渲染，未固定 affinity/频率。

[探针](focus-schedule-probe-2026-10-18.rs)附加到该提交的窗口主文件，保持正式输入、focus、layout 和 renderer；启动前仅切换私有 executor。提前初始化 PostUpdate 记录官方 drain TypeId，结束时要求只移除该系统并新增一个非 exclusive pipe。采样前后与四组间均核对 Ready、无 running request、`/workspace/overview`、3 个 frame、41 个 Node/正尺寸布局节点、282 个活实体及全部 41 个 `ScrollPosition` 的坐标和 Entity；后者是组件数，不是滚动容器数。坐标均为零，未施加连续滚动输入。

每组 128 帧预热、512 个逐帧样本；运行顺序为 round 1 Multi→Single，round 2 Single→Multi，无并行本任务编译/其它采样。CSV 按该运行顺序保留全部 2048 行与离群值。`boundary_us` 包含完整私有 Schedule 和两个固定 Single 探针的边界开销；`main_executor_us` 委托 Bevy 原有 Single Main executor，不含其外层初始化和 render 子应用/呈现。记录、形状断言和输出在该 Main 计时结束后。每轮用 nearest-rank 计算分位数，结果及限制归[性能说明](../../performance.md#私有-focus-调度实测)。

在独立目录检出该源码提交后，可从保存的两个文件重建临时 binary；探针只作本次诊断来源，不加入默认示例或 CI：

```sh
mkdir -p examples/windowed/src/bin
cp docs/data/performance/focus-schedule-probe-2026-10-18.rs /tmp/router-focus-schedule-probe-module.rs
patch -o examples/windowed/src/bin/focus_schedule_probe.rs examples/windowed/src/main.rs < docs/data/performance/focus-schedule-probe-2026-10-18.patch
CARGO_BUILD_JOBS=4 cargo build --offline --locked --release --manifest-path examples/windowed/Cargo.toml --bin focus_schedule_probe --message-format=json > artifacts.jsonl
ROUTER_FOCUS_PROBE_EXECUTOR=multi examples/windowed/target/release/focus_schedule_probe > multi1.log 2>&1
ROUTER_FOCUS_PROBE_EXECUTOR=single examples/windowed/target/release/focus_schedule_probe > single1.log 2>&1
ROUTER_FOCUS_PROBE_EXECUTOR=single examples/windowed/target/release/focus_schedule_probe > single2.log 2>&1
ROUTER_FOCUS_PROBE_EXECUTOR=multi examples/windowed/target/release/focus_schedule_probe > multi2.log 2>&1
```

生产提交早于本组数据文件；重建时须将本组保存的探针与差异文件带入该检出目录。四次运行应使用同一个已核对 hash 的优化 binary，并在构建结束后串行采样。临时窗口副本已从当前默认源码树移除，差异重建结果与实测副本逐字节一致。

## 空闲 Prepare 2026-10-18

生产来源固定为 `3fc65c333cc498dfcf05f975476485a4049aefe2`，采样期间生产源码、lock 与该提交一致。[身份记录](scroll-prepare-identity-2026-10-18.json)保存实际 opt-level=3、MT compiler artifacts、工具链、source/lock/probe/binary/CSV SHA-256、运行顺序和四组相同的 shape。Linux WSL2 / Ryzen 9 9950X / 24 可见逻辑 CPU，Bevy 0.19.1，WSLg 软件渲染；未固定 affinity/频率。采样期间没有本任务的并行构建或其它采样。

[探针源码](scroll-prepare-probe-2026-10-18.rs)与[窗口差异](scroll-prepare-probe-2026-10-18.patch)完整保存。`full` 保持生产管线；`without_empty_prepare` 用 `RemoveSystemsOnly` 移除 Prepare set 的唯一系统，断言数量为 1，并保留 set 的 Content→Layout 顺序。其它 UI 系统、Settle 和私有 focus schedule 均保留。窗口只做初始 Push，无 Back；首末核对 history 长度 1、Ready、无 running，marker 的公开恢复查询为 None 作交叉证据。此查询单独不能排除无容器意图；无 pending 的主要依据是夹具导航创建路径。

四组首末及跨组相同的证据限于 `/workspace/overview`、3 个 frame、41 个 Node/正尺寸布局节点、282 个活实体的计数、唯一 marker 的 Entity/key、全部 41 个原始 `ScrollPosition` 的 Entity/坐标；没有记录完整几何或活实体集合。正常 focus processor 被唯一非 exclusive pipe 替换；PostUpdate 总系统数 full=81、without=80。位置固定，没有持续滚动输入。

每组预热 128 帧、记录 512 帧，按 full1→without1→without2→full2 运行同一 binary。CSV 的 `post_executor_us` 委托原 Multi PostUpdate executor，`main_executor_us` 委托原 Single Main executor；不含各自外层初始化及 render 子应用/呈现。Main 包含 Post 计时结果写入的固定开销；形状检查、记录与输出在 Main 计时结束后。保留全部 2048 行和离群值，各组分位数按 nearest-rank；解释归[性能说明](../../performance.md#空闲滚动-prepare-节点诊断)。

此对照只用于节点移除诊断：差值同时含派发、flush、同步和其它 ready 系统调度变化，不是纯 barrier、typed 实现收益或其严格上限。没有生产改动，也没有降低正常 Settle 采样频率。

在独立目录检出上述生产来源，并带入本组保存的探针与差异文件后重建：

```sh
mkdir -p examples/windowed/src/bin
cp docs/data/performance/scroll-prepare-probe-2026-10-18.rs /tmp/router-scroll-schedule-probe-module.rs
patch -o examples/windowed/src/bin/scroll_schedule_probe.rs examples/windowed/src/main.rs < docs/data/performance/scroll-prepare-probe-2026-10-18.patch
CARGO_BUILD_JOBS=4 cargo build --offline --locked --release --manifest-path examples/windowed/Cargo.toml --bin scroll_schedule_probe --message-format=json > artifacts.jsonl
ROUTER_SCROLL_PROBE=full examples/windowed/target/release/scroll_schedule_probe > full1.log 2>&1
ROUTER_SCROLL_PROBE=without_empty_prepare examples/windowed/target/release/scroll_schedule_probe > without1.log 2>&1
ROUTER_SCROLL_PROBE=without_empty_prepare examples/windowed/target/release/scroll_schedule_probe > without2.log 2>&1
ROUTER_SCROLL_PROBE=full examples/windowed/target/release/scroll_schedule_probe > full2.log 2>&1
```

差异重建结果与实测窗口副本逐字节相同；该临时副本已从默认示例源码树移除。四组应在构建结束后串行运行，先核对同一优化 binary 的 hash。

## Retained 读取 2026-10-18

使用现有 keep-alive 模式，flat、1/32/128 内容预算，每场景 128 样本；导航预热 32 个循环，idle 另预热 64 次 update，每个 idle 样本为 32 update 均值。未改 benchmark 或计时边界。环境沿用[初始动态页面](#初始动态页面-2026-10-18)的 Linux/WSL2、CPU、工具链、官方 Bevy 0.19.1 和 lock；本组额外显式启用 `bevy_internal/multi_threaded`，不能与旧默认非 MT 数据直接作版本收益比较。

| CSV series / variant | 完整源码提交 | 实测 executable SHA-256 |
| --- | --- | --- |
| 全部 / before | `58e24d26cd85c142b83080252591c4b41d2cc828` | `5ccd15dd4d9c9bcdfea7a2b96c9ce396a01a698c1ae636afa75493d8f1677a62` |
| eager / after | `b655f1458ae056257e19465ea092888870701ab2` | `d22686682435b859f49cab0a5ad14965eada9abf231873bba74e917916f12ad8` |
| lazy / after | `95de4eb0f28256effbd01c6666fa4aa3d82ac362` | `b7f28524dffa1ecb5d1946528635c650dd000fd18aa5ce69052350d3ce97875b` |
| retained / after | `b7a5140aa28a403b6aabed78e58afc82151bafe5` | `02f25d32ce7dfa82cfba7ce13b1f5231b998724568c6067346a6a0ba90f5c294` |

`eager` 同时包含 retained 权限相交和 Settle eligible/marker 查询元数据缓存；`lazy` 仅将缓存初始化移到首次 Settle；`retained` 撤回整个查询缓存，只保留权限相交。三组依次运行，各组都是 before 1→after 1→after 2→before 2；CSV 的 `run_order` 为全局顺序，`round` 为配对编号。每次构建结束后保存独立 executable，再直接串行运行，不复用 target 路径作为版本身份。

构建命令如下；从对应输出的 `compiler-artifact` 取 `router_baseline` executable，复制并核对 hash，再以 `ROUTER_BENCH_MODE=keep_alive ROUTER_BENCH_SHAPE=flat` 运行该文件：

```sh
CARGO_BUILD_JOBS=4 cargo bench --offline --locked -p bevy_ui_router --bench router_baseline --features bsn,bevy_internal/multi_threaded --no-run --message-format=json > artifacts.jsonl
```

四份产物的完整 compiler-artifact features/profile 投影相同，`bevy_ecs` 均含 `multi_threaded`；bench 为 opt-level 3、debuginfo 0、debug assertions 与 overflow checks 关闭。crate/benchmark/lock 在各自构建和计时期间没有未提交修改；本任务自身没有并行 Cargo/rustc、renderer 或窗口。未固定 CPU affinity/频率，也未控制宿主全部负载。

已确认采样期间存在外部并行 Cargo 编译，区间为 2026-10-17 UTC 20:25:35–20:25:43、20:25:49–20:26:13。文件时间确认与 `retained` 第 2 轮重叠：after CSV 从创建至末写为 20:25:10.515–20:25:46.323，before 为 20:25:46.343–20:26:22.095。整个配对 48 行均标为 `contended_pair`，不用于收益估算；没有按单行猜测或裁剪受影响样本，也没有重跑追加矩阵。其余组未与该已知区间重叠，标为 `uncontended_pair`，不表示控制了宿主全部负载。

每次完整输出 24 行，全部 288 行保留，含受干扰轮次、较慢导航、尾部和最大值；不删除候选或挑选较快轮次。三组各轮的样本数、min/max update、实体、活动/非活动页面和 factory 次数逐项一致。结果是 CPU wall-clock 汇总，不含 UI layout、picking、窗口、GPU 或持续视觉滚动；没有合并不同运行的分位数。采用结论与未确定归因见[性能说明](../../performance.md#retained-权限读取对照)。

## 当前 history entry 读取

[完整 40 行](performance-current-history-2026-10-23.csv)使用既有 `steady` 模式、nested、每场景 128 样本，包含无 Router、两层/三层 frame、有/无普通滚动 marker 的所有组。固定顺序为 before 1→after 1→after 2→before 2；没有删去较慢组或离群值。两侧均只有一条请求、一项 history，实体和 update 数相同，idle 分位数是每 32 次 update 均值的分布。

[构建身份](current-history-identity-2026-10-23.json)记录基线提交 `b3672c6`、候选完整源码 hash、lock、工具链、实际 binary 和 compiler-artifact features/profile，以及每组输出 hash 和时间。候选仅增加受控的当前 key 读取并替换滚动三处完整投影调用，未改基准。使用两个单独保存的无插桩 executable 运行，采样期间没有本任务的并行构建。

环境为 Linux WSL2、AMD Ryzen 9 9950X、24 可见逻辑 CPU，Rust 1.99.0-nightly、官方 Bevy 0.19.1，opt-level 3、无 debug assertions。实际 `bevy_ecs` artifact 未启用 `multi_threaded`；没有 UI layout、picking、GPU、窗口或持续滚动。未固定 CPU affinity/频率，也未控制宿主全部负载。此结果只约束该小 history 夹具，不证明长 history、MT 调度或整应用帧率收益。

## 清理与输入准备 2026-10-24

诊断基线为 `b3672c6`，临时插桩只存在于独立源码副本与 target，未进入生产模块。第一次的[逐 update 记录](cleanup-profile-2026-10-24.csv)、[完整补丁](cleanup-profile-2026-10-24.patch)、[身份](cleanup-profile-identity-2026-10-24.json)覆盖六个 scope；第二次的[细分记录](cleanup-profile-detail-2026-10-24.csv)、[完整补丁](cleanup-profile-detail-2026-10-24.patch)、[身份](cleanup-profile-detail-identity-2026-10-24.json)另分离 removed receipt 集合与 protected 字段比较。每次各 1,572 行，保留 setup、warmup、measure、reset、teardown 全部阶段，其中 measure 各 384 行。第二份补丁独立应用于相同基线，不叠加第一份。

从基线的独立检出应用对应补丁后，以独立 `CARGO_TARGET_DIR` 优化构建 `cargo bench -p bevy_ui_router --bench router_baseline --features bsn --no-run --locked`；由 compiler-artifact 找到 executable，再运行 `ROUTER_BENCH_PROFILE_SCOPES=1 ROUTER_BENCH_MODE=keep_alive ROUTER_BENCH_SAMPLES=32 <executable>`。数据在请求终态后统一输出；scope 为同步调用的 inclusive wall-clock，嵌套项不能求和，插桩成本仍在总耗时内，两组总时间不作回归比较。

后续 EntityRef 合并候选的[全部 192 行](performance-cleanup-entity-read-2026-10-24.csv)、[未采用差异](cleanup-entity-read-candidate-2026-10-24.patch)和[构建身份](cleanup-entity-read-identity-2026-10-24.json)绑定 `a0679dd8086ef20baaae7fd868adc3fa34d12369`；不含探针。两个保存的 executable 按 before 1→after 1→after 2→before 2，再 before 3→after 3→after 4→before 4 串行运行。每组 flat、128 样本、完整 24 行；样本数、update、活实体、活动/非活动页及 factory 次数逐项一致。

首轮 before 的导航及未受修改影响的 idle 广泛变慢，原因未证实，整对标为 `unstable_pair`。其余配对和全部较慢值均保留，未按单行筛选；没有本任务的并行编译、窗口或其它采样，也未控制宿主全部负载。补充两轮用于判断这种整组变化是否掩盖候选收益，最终候选已撤回，结论见[性能说明](../../performance.md#清理与输入准备热点)。

共同环境为 Linux WSL2、Ryzen 9 9950X、24 可见逻辑 CPU、Rust 1.99.0-nightly、Bevy 0.19.1，bench opt-level 3、无 debug assertions，未固定 affinity/频率。实际 ECS features 不含 `multi_threaded`；这些夹具不含 UI layout、picking、窗口、GPU 或 I/O。CSV 为墙钟而非线程 CPU 时间；idle 仍是 32 update 批均值的分布。

## 输入准备候选 2026-10-24

[完整 384 行](performance-input-preparation-2026-10-24.csv)记录 `full_set` 与 `narrow_set` 两个候选，每个候选均先 flat、再 nested，各自按 before 1→after 1→after 2→before 2 顺序执行。每次完整 24 行、每场景 128 样本；没有本任务的并行编译。每对样本数、update 数、实体、活动/非活动页和 factory 次数一致。`narrow_set` 的 nested 第 2 轮两侧出现整体变慢，整对标为 `unstable_pair`，原因未证实；所有输出和较慢结果保留。

[身份记录](input-preparation-identity-2026-10-24.json)保存实际二进制、完整差异、lock、features 与 profile。before 为 `a0679dd8086ef20baaae7fd868adc3fa34d12369` 保存的无插桩产物；两个 candidate 均为 `3fe3c9f` 加[完整集合差异](input-candidate-2026-10-24.patch)或[收窄差异](input-narrow-candidate-2026-10-24.patch)。这两个基线提交之间仅增加诊断文档，生产源码相同；三处 availability 窄读取没有混入候选。两个输入候选均未采用，源码与专用新增测试已撤回。

为解释热点而保留的[追加诊断 3,144 行](publication-profile-2026-10-24.csv)分 baseline 与完整集合候选，各 1,572 行，保留全部阶段；两者都基于 `b3672c6`，与正式对照的源码基线不同。先在独立检出应用[九 scope 补丁](publication-profile-2026-10-24.patch)获得 baseline；完整集合诊断再应用[输入差异](publication-profile-input-2026-10-24.patch)。两个补丁顺序重放与实测源码逐字节一致，[身份](publication-profile-identity-2026-10-24.json)绑定数据、差异和二进制。构建与运行方法沿用上一节的独立 target / 32 样本诊断命令。

追加 scope 包括 `prepare_presentation_access`；诊断确认局部输入准备变快，但其它 scope 的变化没有建立因果解释，不能代替无插桩的完整路径对照。共同环境及不含 MT/layout/picking/GPU/I/O 的边界沿用上一节。最终取舍见[性能说明](../../performance.md#清理与输入准备热点)。

## 完整导航阶段 2026-10-24

[全部 7,944 行](navigation-stages-2026-10-24.csv)、[独立探针补丁](navigation-stages-probe-2026-10-24.patch)与[身份](navigation-stages-identity-2026-10-24.json)绑定完整基线 `11e89350b1abeb82eb82b1a0aea6d50ac0151ae6`。补丁在该基线重放后，11 个变化文件逐字节与实测副本相同；生产库不含探针。顺序为 keep_alive flat、keep_alive nested、instances flat、instances nested，每场景 32 个计时样本，保留所有阶段及离群值，measure 共 2,304 行。

独立 target 构建 `cargo bench -p bevy_ui_router --bench router_baseline --features bsn --no-run --locked --message-format=json`，以 compiler-artifact 定位并保存 executable；用 `ROUTER_BENCH_PROFILE_SCOPES=1 ROUTER_BENCH_SAMPLES=32 ROUTER_BENCH_MODE=<mode> ROUTER_BENCH_SHAPE=<shape>` 直接运行。身份记录包含准确构建参数、源码/数据/二进制 hash、lock、Rust 和实际 ECS features。

十个系统阶段分别为 ObserveAuthority、Collect、Drive、Commit、SettleCleanup、PublishSnapshots、InputRefresh、InputFilter、ScrollPrepare、ScrollSettle；另十五个细分 scope 与阶段嵌套，inclusive 时间不能相加。父 children 循环同时包含不可变期望的构造、实际读取与比较，不代表纯构造时间。仅非 MT 构建使用 thread-local 聚合。

每个导航记录 enqueue、逐 update、从原起点计算的累计时间与原 completion 边界，数据在请求终态后统一输出。completion 在同一导航的多行中重复，汇总时按 nav_id 去重。标准 benchmark 的断言、CSV 和计时边界保留；探针有额外成本，不能拿插桩总时间替代无插桩对照。idle 沿用原批均值，没有新增逐帧记录。

环境沿用本日 Linux WSL2 / Ryzen 9 9950X / Rust 1.99.0-nightly / Bevy 0.19.1，24 可见逻辑 CPU；采样串行且无本任务的并行构建，未固定 affinity、频率或控制宿主全部负载。不含 MT、UI layout、picking、renderer 或 I/O；结果解读见[完整阶段说明](../../performance.md#完整导航阶段与-scene-声明拓扑)。

## Scene 声明拓扑对照 2026-10-24

[完整 384 行](performance-native-topology-2026-10-24.csv)、[候选完整差异](native-topology-candidate-2026-10-24.patch)和[构建身份](native-topology-identity-2026-10-24.json)对应基线 `11e8935`。candidate 仅修改 native Scene 的操作内声明拓扑准备及复用，保留全部 World 实时复验，并增加一个真实 marker hook 重排 siblings 的回归；没有改 benchmark。

先 keep_alive、再 instances，每个模式先 flat 后 nested；每种组合均按 before 1→candidate 1→candidate 2→before 2 运行，每场景 128 样本，每次完整 24 行。CSV 合并两种模式的原始列，不适用列留空，所有较慢组和最大值保留。各配对非计时列逐项相同，无选择性删除或合并分位数。

使用无插桩优化构建及独立保存的两个 executable，实际路径来自 compiler-artifact。导出基线与主工作区复用 target 时，显式清理 ECS/facade 的 release 产物后重新构建 candidate，避免跨源码目录复用旧产物；记录确认 ECS 源码重新编译，两份 binary hash 不同。采样期间没有本任务的并行构建。

环境、工具链、features 与本日完整阶段诊断一致，实际 ECS 未开启 `multi_threaded`。headless wall-clock 不含 layout、原生输入、renderer 或异步 I/O。结果包含命中路径的小幅回退，取舍见[性能说明](../../performance.md#完整导航阶段与-scene-声明拓扑)。

## World 规模与清理准备 2026-10-24

[全部 11,832 行](world-scaling-2026-10-24.csv)、[完整独立补丁](world-scaling-probe-2026-10-24.patch)和[身份](world-scaling-identity-2026-10-24.json)仍绑定 `11e8935`，不包含后来采用的 Scene 拓扑优化。补丁独立重放后11个变化文件与实测副本逐字节一致，不叠加前述探针。共有27个 inclusive scope，新增 removal plan 准备与 receipt batch 准备；后者嵌套于前者。

先运行 flat instances、背景数0的一次清理诊断，再用同一 executable 运行 flat keep_alive：0→2048→8192、8192→2048→0，每场景32样本，标准CSV的每组24行及全部行为断言保留。背景开关为 `ROUTER_BENCH_UNRELATED_ENTITIES`，默认0；其余构建和 profile 环境变量与完整阶段诊断相同。采样串行且无本任务的并行构建。

背景实体只带普通 Node 与独立 marker，不含 ChildOf、Router host 或 Content。初始化断言精确增加N个，退出再次验证数量、Node和无归属状态；这些附加检查不在计时循环内。跨规模配对的非计时列完全一致，`live_entities` 精确相差N，原CSV继续报告真实总数。数据新增 run 与 unrelated_entities；按 run/mode/背景数/nav_id 对 completion 去重。

环境与非MT、headless、wall-clock边界沿用本日完整阶段诊断。固定内容预算下的背景规模对照不代表任意 archetype 或应用负载；[结果及最小优化边界](../../performance.md#无关-world-实体与清理准备)分别讨论 presentation、输入撤权和 receipt，未把全部增量归于单一scope。

## 定向查询 2026-10-24

[全部384行](performance-bounded-query-2026-10-24.csv)、[背景夹具差异](bounded-query-fixture-2026-10-24.patch)、[组合候选差异](bounded-query-candidate-2026-10-24.patch)与[构建身份](bounded-query-identity-2026-10-24.json)绑定 `6b20ed9`。先应用夹具，再按身份中的 `candidate_include` 用 `git apply --include=<path>` 选取 presentation 两个文件、pointer 文件或全部文件；四个版本均已逐字节重放核验。合入主源码前仅另作 rustfmt 格式化。

无插桩，原导航终态边界、idle批次、CSV与行为断言不变；背景实体的精确数量及无归属检查位于初始化/退出，不在采样内。使用实际 compiler-artifact 保存四个 executable，确认每个候选的ECS重新编译。各背景数按 baseline1→presentation1→pointer1→combined1→combined2→pointer2→presentation2→baseline2 串行运行，先0后8192；每次flat keep_alive、128样本、完整24行。

组内全部非计时列一致，`live_entities`继续包含背景实体；所有尾部和较慢结果保留。环境沿用本日WSL2、非MT优化构建与headless边界，没有本任务的并行构建，不含layout/picking/GPU/I/O，也未控制宿主全部负载。采用理由及小场景代价见[性能说明](../../performance.md#按目标读取-presentation-与-pointer)。

## Receipt 批量准备 2026-10-24

[完整192行](performance-receipt-batch-2026-10-24.csv)、[未采用差异](receipt-batch-candidate-2026-10-24.patch)与[构建身份](receipt-batch-identity-2026-10-24.json)绑定 `822eed4`，只改 cleanup 的操作内 receipt 准备，未进入主源码。补丁重放与实测候选逐字节相同；源码基线已含定向查询优化，没有改 benchmark。

先flat keep_alive，再flat instances，各按before1→candidate1→candidate2→before2运行，每场景128样本，每次完整24行；非计时列逐项一致，行为断言通过。两份无插桩executable来自重新编译的实际artifact，全部慢值与尾部保留。构建参数、环境及非MT/headless边界沿用本日定向查询对照；没有本任务的并行构建。静态复核不代替crate回归，本候选未运行完整crate测试；[未采用原因](../../performance.md#无关-world-实体与清理准备)基于完整路径收益不足。

## 多 Router 2026-10-24

[192行](multi-router-2026-10-24.csv)、[六文件独立探针](multi-router-probe-2026-10-24.patch)和[身份](multi-router-identity-2026-10-24.json)绑定 `6b20ed9`，早于定向查询优化。patch重放逐字节一致，Cargo配置未改。构建原router_baseline后，以 `ROUTER_BENCH_MODE=multi_router ROUTER_BENCH_SAMPLES=32 <executable> --bench` 运行；已确认构建及运行退出0，全部边界保护断言通过。

固定A/B两个Router，均两层native frame、每页128节点；A有两个预热页面，B有64条终态请求与一项history。三个阶段各预热64update再采32次：无marker空闲、仅A有普通marker空闲、A命中导航/B静止。计数使用固定TLS双槽，没有探针HashMap分配；只测Settle归属观察、借用请求索引构建，并分别计数两类公开快照重建。A保留原完成边界与行为断言，B状态检查在阶段结束及A退出后进行。

外层update/completion时间在A/B两行重复，统计按phase/sample去重；inclusive scope不可与外层求和。探针只适用于实际非MT构建，并验证必经A scope被捕获；没有UI layout、窗口或滚动恢复计时。单次运行支持成本定位，不作为正式版本收益对照；环境和工具链沿用本日headless诊断。[结论](../../performance.md#多-router-的观察与快照)区分临时索引、真实快照重建和滚动观察。

## History 分配 2026-10-24

[全部288行](history-allocations-2026-10-24.csv)、[探针与临时 lock 差异](history-allocation-probe-2026-10-24.patch)、[生产候选差异](history-borrow-candidate-2026-10-24.patch)及[身份](history-allocations-identity-2026-10-24.json)绑定 `c63a1c5`。先应用探针得到 before，再应用候选得到 after；顺序重放与实测四个文件逐字节相同。只在独立副本的 Core dev-dependency 加入固定 `allocation-counter = "=0.8.1"`，生产 Cargo/lock 未改变。

优化构建 `cargo test -p bevy_ui_router_core --release --no-run --locked --message-format=json`，独立 target、构建并发2、关闭 incremental；取 compiler-artifact 的 test executable 保存后运行 `history_authority::tests::history_allocation_diagnostic --ignored --exact --nocapture --test-threads=1`。两个 binary、完整源码、lock 和 Rust 身份均记录。

使用现有 HistoryAuthorityFixture，经正常 Push/Back capability 构建1/64/256条 history，在真实 cursor 0 下分别测 owned freeze、成功 NoTarget 源校验及完整 Replace 链，每项4次预热、16次样本。Replace 的 binding/target 构造与打印在区间外；真实 capability 的创建、prepare、source match、publish 在同一区间。owned freeze 返回值在区间外 drop，便于同时观察临时总分配与最终 owned 输出。

依照[计数库官方说明](https://docs.rs/allocation-counter/0.8.1/allocation_counter/fn.measure.html)，仅当前线程计数；全部区间不嵌套。`current` 是区间分配减释放，Replace 包含对旧 history/外部输入的释放，因此为负不代表错误，也不能据此宣称进程 heap 或长期释放正常。本次比较分配次数与字节，不比较工具替换 allocator 后的 wall-clock。原始每次记录均保留，不设置 CI 分配阈值；[结论](../../performance.md#history-源校验的分配减法)限于这三条 Core 路径。
