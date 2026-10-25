# 历史性能数据

[English](README.en.md) | 简体中文

保留五组使用完整提交源码运行的公开 benchmark 数据。运行方法和各模式计时边界见[性能测量](../../performance.md)。CSV 保存样本汇总，每组的来源如下。

| Dataset / 数据 | Rows / 行 | Source / 源码 |
| --- | --- | --- |
| [Keep-alive](performance-keep-alive-2026-10-08.csv) | 96 | `1070143dc257bd35b7617cc527127fdfc0cf5a57` |
| [Parent/child / 父子库存](performance-instances-2026-10-11.csv) | 168 | `00ba8be819143e7472b32cdf80ff74fc6e5a8062` |
| [Idle / 空闲更新](performance-idle-2026-10-17.csv) | 192 | before `b3bd15397737f4e0cd59bc951f4a73c3888df76b` → after `d731ccbeabe1232f71d1727c8f5e061e091fb854` |
| [Steady / 稳态执行](performance-steady-2026-10-17.csv) | 192 | before `d731ccbeabe1232f71d1727c8f5e061e091fb854` → after `a5b16d0348aba65b361b478ed5995e2102b07734` |
| [Initial dynamic page / 初始动态页面](performance-initial-steady-2026-10-18.csv) | 40 | before `2037699a1463f997cc506bbc43da94d5f09526c4` → after `9c222bb636b76bea62adc5ec30402a306ea133d6` |

## 测量环境

这些数据使用 Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`、AMD Ryzen 9 9950X、24 个可见逻辑 CPU；Rust 为 `1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0，Bevy 为 0.19.1。使用 `bsn` feature 与默认 bench 优化 profile：opt-level 3、debuginfo 0、debug assertions/overflow checks 关闭。CPU affinity 和频率由系统管理。

原测量 `Cargo.lock` SHA-256：`1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`。二进制身份如下：

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

## 维护

历史文件保留完整行和质量标记。新的对照使用新的数据文件，并记录源码、工具链、二进制、命令和环境。旧候选补丁、阶段 profile 和临时窗口探针通过 Git 历史追溯。
