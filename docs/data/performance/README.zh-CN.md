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
| [保留页稳态更新](performance-retained-steady-2026-11-12.csv) | 372 | `c96836592f553a3e2b8c898063280e013f1764a1` → `bd937055bd5cf218b5c398e89a55a5437f7818b2` |
| [观察路径控制组](performance-observation-controls-2026-11-12.csv) | 194 | 同上；普通导航及初始动态页面 |
| [公开查询消费者](performance-public-queries-2026-11-12.csv) | 240 | `4ad3c77f9bdc0b38159f03ae5b141bcd91520a88` → `96787c790507cca1ee3b8e474edafcf75dbbe7fc` |
| [终结请求维护与缓存命中](performance-maintenance-2026-11-14.csv) | 80 | `aaca8de7bdfaa7c874ab00b5fc6f4c63a921f576` → `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79` |
| [普通祖先共享](performance-query-ancestors-2026-11-14.csv) | 280 | `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79` → `f409d6f1f9295086c56dbbf0a818cf4c9fcc0ea8` |

## 测量环境

本节环境对应截至 2026 年 10 月的数据；11 月测量另记于下文。

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

## 保留页稳态更新：2026-11-12

新增 `retained_steady` 模式保持同一活动页 Entity、128 节点的共同父布局和 32 节点的活动叶页，依次积累 0/1/4 个各有 32/1024 动态后代的保留兄弟页。分别测空闲更新及仅修改请求滚动位置的更新；factory、activity、输入资格、history、布局几何及退出作用域后的清理在计时外检查。`ui=1` 安装真实布局和 picking 系统，不创建 renderer、不注入 pointer。不同库存阶段分别有 1/3/9 条请求记录；跨版本比较相同行，保持该项工作量一致。

本次使用 Bevy 0.20.1，上述 Rust、CPU 和 OS，`bsn`、默认 bench 优化 profile 及两个构建 job。根 lock SHA-256：`f34d67d2d70b85a1c113d08f321f018e4c39d72f6c5de6a0b6fa1bf62f6cf227`。CPU affinity 和频率由系统管理。没有设备、renderer、GPU 或功耗测量。

| Variant | 生产源码 | Benchmark binary SHA-256 |
| --- | --- | --- |
| `before` | `c96836592f553a3e2b8c898063280e013f1764a1` | `30e5654fc1f1600bf6c38fab2cc3863a75e22d7774fa63e16f81fc1c67a6cce3` |
| `shared_observation` | `891ee53be78da05a0a782131e08d376739294f8f` | `27aee61efc82381005b7967250361894e664f3f224fa018f7b93a2c1cc677086` |
| `entity_hash` | `2282510fc9d14253297c0d6f701bf28ac806a6d7` | `7ea6283be4ced98d5d8b3ad58bff5ebb7617c130a0d687474ab3d7409ca6c204` |
| `after` | `bd937055bd5cf218b5c398e89a55a5437f7818b2` | `4b46b752bbeae9900ccc744d4303e17dea752c487958dcbd37bf8cab35cca9ec` |

先用 `CARGO_BUILD_JOBS=2 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked --no-run` 构建，比较时固定各版本的可执行文件。命令及参数见[测量指南](../../performance.zh-CN.md#固定活动内容的保留兄弟页对照)。nested 正式运行使用 32 样本、每样本 32 次 update，预热 64 次；flat 和普通控制组使用 16 样本，后期配对的初始页面控制组使用 32 样本。分位数描述 update 批次均值的 CPU wall time，不是逐帧呈现分位数。

计时均串行且与构建分开。先完成两轮 `before` router/UI 对照，再运行观察复用版并穿插 `before` UI 回测；Entity 集合候选与观察复用版回测对照。最后分别对 UI 和 router-only 完成两轮 `before → entity_hash`，随后运行两轮 `after` router/UI 及普通控制组。CSV 保留已提交版本的全部测量行，包括标记为 `variable_round` 的早期集合候选轮次；该标记表示观测到波动，不证明某个外部原因。后续稳定回测支撑结论，未删除这些行。

nested、每个保留页 1024 后代时，最终 idle p50 范围如下，单位为微秒：

| 模式 / 保留页数 | 修改前回测 | 修改后 |
| --- | ---: | ---: |
| Router-only / 0 | 55.8–56.7 | 43.1–43.9 |
| Router-only / 4 | 650.4–659.7 | 192.8–200.5 |
| UI / 0 | 90.5–93.2 | 76.9–78.1 |
| UI / 4 | 1182.6–1204.4 | 717.4–737.9 |

对应 UI / 4 的 scroll-update p50 从 1191.3–1194.1 降至 715.2–739.3 微秒。控制组不支持普遍加速结论：普通初始页面有几个百分点的波动，UI 总时间仍包含 Bevy 工作。按需 outlet 计数消除了提前准备 facts 时额外引入的完整叶页扫描。观察内共享内容 Vec 和 membership 集合，不复制大集合；未插桩测量分配次数或字节数，因此不宣称数值化分配收益。

保留的修改包括同次不可变 World 借用内的父内容和相同目的 preservation 复用、registry 读取索引及 Bevy Entity 集合。角色相关校验和写后重新观察保留。单条 memo 替代分桶 Vec 的探索候选没有产生可重复的 CPU 收益，已撤回。每个必要阶段仍完整观察保留内容一次；跨帧失效机制和未证实必要的多 Router 优化后置。本次证实的是库内 CPU 改善，不是应用 FPS 或电池收益。

## 公开查询消费者：2026-11-12

`public_queries` 夹具包含 32 或 1024 个活动动态后代，以及同量保留后代。计时覆盖 publication 后的常驻消费者系统，包括应用组件写入和普通 Router update。`per_target` 逐后代查询，`same_root` 以同样次数重复查询活动根。滚动行测量没有待恢复意图的情况，详见[测量方法](../../performance.zh-CN.md#应用查询消费者)。

环境沿用上文 November 记录，使用 Bevy 0.20.1、`bsn`、同一根 lock、优化 bench profile 与两个构建 job。每行预热 16 次 update，采样 32 次，每次平均 4 次 update。未包含 renderer、UI layout、pointer 注入、设备测量或 heap 分配计数。

| 变体 | 源码 | 二进制 SHA-256 |
| --- | --- | --- |
| 变更前夹具 | `4ad3c77f9bdc0b38159f03ae5b141bcd91520a88` | `d1e9e135720d4873570863246facf4e23661fb1e1e8ef1c18566566858c3b632` |
| 共享 reader | `96787c790507cca1ee3b8e474edafcf75dbbe7fc` | `c897092c27c575ad5110063671971b866d748c9cb7296e79d3316d1f4070306b` |

用现有 `cargo bench ... --no-run` 命令构建，再通过 `ROUTER_BENCH_MODE=public_queries` 与 `ROUTER_BENCH_SHAPE=flat|nested` 运行固定二进制。构建与测量串行进行。顺序为 before flat/nested 第 1–2 轮、探索性 reader 工作、before 第 3 轮回放，随后为最终 reader 第 1 轮 → before 第 4 轮回放 → 最终 reader 第 2 轮。CSV 保留全部 before 和最终 reader 行，未提交的初期 reader 试验另行保存。部分 flat 控制组与重复根行波动明显，尚未确定原因。

主要比较来自**最终二进制内**相同内容、写入和运行条件下的旧入口与共享 reader。before 夹具有 4 个运行条件，最终夹具有 5 个，跨二进制对照也包含这一小段调度差异。最终两轮的每 update p50 范围如下，单位微秒：

| 形状／逐实体查询数 | 空循环 | 单次 activity | 共享 activity | 单次 inputs | 共享 inputs |
| --- | ---: | ---: | ---: | ---: | ---: |
| 平铺／2048 | 158.2–160.9 | 1383.2–1508.7 | 349.3–350.1 | 1389.3–1427.8 | 351.2–353.2 |
| 嵌套／2048 | 153.1–156.2 | 1488.8–1498.4 | 650.2–653.4 | 1489.3–1506.7 | 652.3–659.0 |
| 嵌套／64 | 18.1–18.2 | 57.0–57.1 | 34.1–34.2 | 56.9–57.1 | 34.1–34.4 |

嵌套的重复根 activity p50 从 2096.4–2122.3 降至 162.1–164.8 微秒；嵌套逐实体滚动 miss 从 1483.0–1497.5 降至 667.8–669.7 微秒。收益要求在循环外创建一次 reader，原单次方法仍可使用。这些结果表示本消费者工作负载的 CPU 成本，不是独立方法耗时或应用 FPS。小列表特殊快路未得到可重复的额外收益，已移除。

## 终结请求维护与缓存命中：2026-11-14

`maintenance` 基准先分离历史 trace 大小与当前内容，再测预热后的缓存导航；见[方法](../../performance.zh-CN.md#已关闭-trace-与预热缓存命中)。使用上述 11 月环境、Bevy 0.20.1、同一根 lock、`bsn`、默认 bench 优化 profile 与两个构建任务，每行 32 样本。不含 renderer、UI layout、pointer 注入、设备测量或分配计数。

| Variant | Source | Binary SHA-256 |
| --- | --- | --- |
| `before` | `aaca8de7bdfaa7c874ab00b5fc6f4c63a921f576` | `c3a958db4124f354e8a15c2c0f77245e71dabbb959bad6732f25a112f9b7a459` |
| `prepared_publication` | `3cda2f803bacec6714044d26ea0a594df681ce87` | `8cb51b4d925194232509de93ed61957d302cb51b11801877585ce0d70c4bd2e1` |
| `local_input_index` | `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79` | `162efbaf516e0c399b318964a49cf41348753616b8f4701145f84111bbe26846` |

用既有 `cargo bench ... --no-run` 命令构建并固定二进制，再指定 `ROUTER_BENCH_MODE=maintenance`、`ROUTER_BENCH_SHAPE=flat|nested`。构建与计时分开串行执行，顺序为 before 第 1–2 轮 → prepared-publication 第 1 轮 → before 第 3 轮 → prepared-publication 第 2 轮 → local-input-index 第 1 轮 → prepared-publication 第 3 轮 → local-input-index 第 2 轮；每轮先 flat 后 nested。保留全部 80 行，包括波动较大的小规模场景。

固定 54 个存活实体、一个 history entry、64 个终结请求后，原 idle p50 随 trace 从 768 events 增至 8960 events，由 14.8–16.3 µs 增至 35.9–36.8 µs。最终版本各规模为 13.0–14.6 µs。按精确请求身份复用已闭合 trace，消除了与其长度相关的比较；晚到 diagnostics 仍独立更新。idle 每个样本平均 32 次更新。

两个页面各有 1024 个动态后代时，完整 Replace 命中的 p50 从 5.81–5.92 ms，经输入变更预备复用及无 marker 采集快路降至 3.59–3.62 ms，再经保留页验证的局部输入集合索引降至 1.05–1.09 ms。每次操作一次 update；实体、history 和工厂数固定。该负载合计约减少 82% CPU wall time。中间版本的三项改造一起测量，不推断各项独立贡献。32 节点小场景有波动，不能据此认定普遍加速。

Input gate 保留有序 projection，绑定预备值时复核完整 source；保留页索引只活于一次不可变校验，仍位于原有角色 predicate。没有删除写后证明或新增持久缓存失效机制。测量表示库内 CPU 工作，不是应用 FPS。

## 普通祖先共享：2026-11-14

沿用上述 `public_queries` 夹具、11 月环境、Bevy 0.20.1、根 lock、`bsn` 和优化 profile，每行 32 样本、每样本平均四次更新。before 源码为 `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79`，二进制 SHA-256 为 `162efbaf516e0c399b318964a49cf41348753616b8f4701145f84111bbe26846`；交付 reader 源码为 `f409d6f1f9295086c56dbbf0a818cf4c9fcc0ea8`，二进制为 `9c565eb3042804d785db0c5c020c50d981fec40bfb632cfdd03a64673917d29d`。

固定二进制后指定 `ROUTER_BENCH_MODE=public_queries`、`ROUTER_BENCH_SHAPE=flat|nested`。构建和计时分开串行执行，顺序为 before 第 1 轮 → reader 探索 → before 第 2 轮 → 交付 reader 第 1 轮 → before 第 3 轮 → 交付 reader 第 2 轮，每轮先 flat 后 nested。保留全部 280 行 before 和交付版本数据。未提交的首个候选改善了嵌套读取，但增加平铺成本；最终仅用现有环检测集合记录普通节点，省去注册终点的插入/移除和原目标的重复插入。

2048 个不同目标的 p50 范围，单位微秒：

| 结构／API | Before | 祖先共享 |
| --- | ---: | ---: |
| 嵌套／activity | 640.7–650.6 | 349.9–351.1 |
| 嵌套／当前输入 | 643.9–652.7 | 347.5–349.1 |
| 嵌套／scroll miss | 638.8–650.7 | 356.9–358.0 |
| 平铺／activity | 340.1–355.4 | 348.7–348.9 |
| 平铺／当前输入 | 342.3–347.6 | 349.6–351.0 |
| 平铺／scroll miss | 346.6–356.2 | 355.4–362.1 |

嵌套 activity 总耗时约减少 45%；平铺输入和滚动查询仍有少量开销，不认定额外收益。重复根查询和空循环控制组接近原有成本。reader 仅在一次不可变借用内共享完整解析后的普通祖先结果；注册边界仍区分直接目标和继承内容，包括负结果与查询顺序。没有新增持久索引或改变公开 API。

## 维护

历史文件保留完整行和质量标记。新的对照使用新的数据文件，并记录源码、工具链、二进制、命令和环境。旧候选补丁、阶段 profile 和临时窗口探针通过 Git 历史追溯。
