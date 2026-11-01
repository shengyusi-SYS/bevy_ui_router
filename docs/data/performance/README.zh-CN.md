# 历史性能数据

[English](README.md) | 简体中文

按完整提交源码保存公开 benchmark 数据。运行方法和各模式计时边界见[性能测量](../../performance.zh-CN.md)。CSV 保存样本汇总；窗口数据保存逐样本值。

| Dataset / 数据 | Rows / 行 | Source / 源码 |
| --- | --- | --- |
| [Keep-alive](performance-keep-alive-2026-10-08.csv) | 96 | `1070143dc257bd35b7617cc527127fdfc0cf5a57` |
| [Parent/child / 父子库存](performance-instances-2026-10-11.csv) | 168 | `00ba8be819143e7472b32cdf80ff74fc6e5a8062` |
| [Idle / 空闲更新](performance-idle-2026-10-17.csv) | 192 | before `b3bd15397737f4e0cd59bc951f4a73c3888df76b` → after `d731ccbeabe1232f71d1727c8f5e061e091fb854` |
| [Steady / 稳态执行](performance-steady-2026-10-17.csv) | 192 | before `d731ccbeabe1232f71d1727c8f5e061e091fb854` → after `a5b16d0348aba65b361b478ed5995e2102b07734` |
| [Initial dynamic page / 初始动态页面](performance-initial-steady-2026-10-18.csv) | 40 | before `2037699a1463f997cc506bbc43da94d5f09526c4` → after `9c222bb636b76bea62adc5ec30402a306ea133d6` |
| [Transitions / 过渡全路径](performance-transitions-2026-10-25.csv) | 750 | before `9ac27b8c3f154b9ac5a524470ae8f540c21b2237` → after `7e12ae72654acbe6649eaee832c4670d28d9be9e` |
| [Window / 窗口过渡](performance-transition-window-2026-10-25.csv) | 4800 | 同上；无 BRP 的优化窗口构建 |

## 测量环境

这些数据使用 Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 个可见逻辑 CPU；Rust 为 `1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0，Bevy 为 0.19.1。使用 `bsn` feature 与默认 bench 优化 profile：opt-level 3、debuginfo 0、debug assertions/overflow checks 关闭。CPU affinity 和频率由系统管理。

前五组测量的 `Cargo.lock` SHA-256：`1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`。这些二进制身份如下；过渡测量的来源另列于下文。

| Dataset / 数据 | Binary SHA-256 / 二进制 SHA-256 |
| --- | --- |
| Keep-alive | `7cc3e143254610b5c790564fc885f81d1c34d6c17811ae345c460327d34f7fef` |
| Parent/child | `bcd55bee6885f4911709d2e6163c0f8fa24e94a6f1b93718aefc813b4820e619` |
| Idle before | `5fef98b47f41029b3f37b29870531dbfa6db9d43a27ad1f8a39a9f2dc7f0784e` |
| Idle after / Steady before | `68322ed332e32f940c357787198deeff67cefa76f77f1d5d919433638e6ace81` |
| Steady after | `b2c8411f7ab816d533d4bc92ccc10c87997853cf0b6d4d319a5e15a08b4ece1a` |
| Initial before | `2669913ec81e8d6dc7a963faff2b672d39d5e420dcc56ef0977d961f62ccc509` |
| Initial after | `d3b3c50dc2b046cc50778f1acf674e53b945b48f14354972ab766cbe62e9da5a` |

## 运行顺序和比较范围

- **Keep-alive**：两轮各按 flat → nested 串行执行，每组 24 行、每场景 128 样本。记录未过滤的全部汇总。
- **父子库存**：每组 24 行、128 样本、64 次导航预热。round 3/4 是完整串行 flat → nested 配对，标记 `uncontended_pair`；早期重叠运行标记 `contended`，未配对组为 `unpaired`。保留全部 168 行，比较采用完整串行配对。
- **空闲更新**：before 两轮 → 构建 after → after 两轮，均为 flat、1/32/128 内容规模。每组 48 行；较慢组与离群值均保留。
- **稳态执行**：构建完成后按 before 1 → after 1 → before 2 → after 2 运行。每组 48 行；after 增加两个固定 focus 调度相关实体，控制组保持原样。
- **初始动态页面**：before 1 → 构建 after → after 1 → before 2 → after 2。nested、128/1024 个动态节点、64 update 预热、每场景 128 样本，每组 10 行；一条请求和一项 history。

串行配对期间构建和计时分开，关闭窗口、renderer 和插桩。宿主系统负载会影响 wall-clock，比较时一并查看控制组与各轮波动。idle 的每个样本是 32 update 均值；这些汇总分位数用于各组比较，逐帧延迟和堆内存另行测量。

## 过渡测量：2026-10-25

使用上面的同机环境、Bevy 0.19.1 与默认优化 profile，`CARGO_INCREMENTAL=0`、构建并发数 2。根 lock SHA-256 为 `0fc9c7ada9ed0bc2dd0f580cd6e4bbada45de8d8c4c5629a9d71ba1601f3aad5`；窗口 lock 为 `cd1fe4460a631e586c745ec840e3c87b0ce5a9a1638621e3e6ad1935d591bbab`。窗口使用 X11/WSLg、Vulkan、Mesa 26.0.8 llvmpipe（LLVM 21.1.8），1060×720，无 BRP/截屏。

| Variant | Source | Benchmark binary SHA-256 |
| --- | --- | --- |
| `before` | `9ac27b8c3f154b9ac5a524470ae8f540c21b2237` | `c09bba2e7fcc86b86cc0435f1420b218e620390601938abdfc0889d4064bed84` |
| `input_batch` | `a8682de49aa7ca659bf0382ffea7ae6530ae6242` | `f079ec75010f12a769a795edf4be6dc2d2f6ab747de550da8ff6c4262fdc8207` |
| `after` | `7e12ae72654acbe6649eaee832c4670d28d9be9e` | `8c2e8fd7e0bfcee863f6881c4173686f441981cbd0705860439a2757f89c2e2e` |

窗口 `before` 二进制 SHA-256 为 `80196bd253515200e9f36c03615e3b9983d685bc6f427e4f9074494fb0543ef9`，`after` 为 `c50cd11662ce2def4ef37e980a05be59e4ddd617c52f2869a41975c38c52f292`，源码对应上表。

所有正式计时均与构建分开串行运行。headless 运行顺序为 `before 1 → input_batch 1 → before 2 → input_batch 2`，再做 nested/128 和无 UI/128 的 before/input_batch 对照，最后 `after 1 → before 3 → after 2`。每个完整 flat 组 90 行，16 样本，32/128/1024 节点；两个单规模补充各 30 行。仅 `ui=1` 运行真实布局与候选投影；所有操作使用 Replace 保持单项 history。预热和完成边界见[方法](../../performance.zh-CN.md#动画全路径)。

窗口按 `before 1 → before 2 → after 1 → after 2` 独立运行。每种效果先预热 60 个合格 update，再收集 120 个 main 和 120 个 render 样本；五种模式共 1200 行/轮。应用按真实时间推进，采到的动画进度和导航次数可随更新间隔变化，不能当作固定工作量的 CPU 对照。CSV 的 `elapsed_ns` 是 wall-clock；`update_interval_ns` 只记录主循环更新间隔。

结果保留全部行与离群值。headless 的 1024 节点缓存命中、持有退出阶段，p50 从 1.009–1.022 ms/update 降到 0.492–0.534 ms/update；每个样本是 32 update 的均值。1024 节点 fresh、关闭过渡时，发布所在 update 包含立即清理，p50 从 392–403 ms 降到 199–214 ms。前者来自批次输入资格/祖先观察，后者来自单次不可变 adoption 校验中的登记表索引；全部原校验与逐写复验保留。

小场景和控制组存在波动，`after 1` 的部分控制行较慢，不能把所有差值归因于修改。窗口空闲与动画耗时也同步变化，因此本组窗口数据用于检查真实调度和提交行为，不支持硬件 GPU 或实际呈现 FPS 的提升结论。退出清理的 EntityRef 读取候选仅约 5% 收益，未达到本轮 10% 保留门槛，已撤销；未增加跨写缓存或减少复验。

## 维护

历史文件保留完整行和质量标记。新的对照使用新的数据文件，并记录源码、工具链、二进制、命令和环境。旧候选补丁、阶段 profile 和临时窗口探针通过 Git 历史追溯。
