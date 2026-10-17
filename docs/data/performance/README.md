# 性能测量数据

这里保存测量数据；历史 CSV 原样迁移，新测量注明完整源码与环境身份，不混作现行合同。运行方法、统计边界和结果解读见[性能说明](../../performance.md)。除窗口文件为逐帧记录外，其余是样本汇总，不能从汇总分位数还原或合并原始样本。

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
