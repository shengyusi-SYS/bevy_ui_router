# Historical performance data

English | [简体中文](README.md)

Public benchmark data is retained with complete source commits. See [performance measurement](../../performance.en.md) for commands and timing boundaries. Benchmark CSVs contain summaries; window data retains individual samples.

| Dataset / 数据 | Rows / 行 | Source / 源码 |
| --- | --- | --- |
| [Keep-alive](performance-keep-alive-2026-10-08.csv) | 96 | `1070143dc257bd35b7617cc527127fdfc0cf5a57` |
| [Parent/child / 父子库存](performance-instances-2026-10-11.csv) | 168 | `00ba8be819143e7472b32cdf80ff74fc6e5a8062` |
| [Idle / 空闲更新](performance-idle-2026-10-17.csv) | 192 | before `b3bd15397737f4e0cd59bc951f4a73c3888df76b` → after `d731ccbeabe1232f71d1727c8f5e061e091fb854` |
| [Steady / 稳态执行](performance-steady-2026-10-17.csv) | 192 | before `d731ccbeabe1232f71d1727c8f5e061e091fb854` → after `a5b16d0348aba65b361b478ed5995e2102b07734` |
| [Initial dynamic page / 初始动态页面](performance-initial-steady-2026-10-18.csv) | 40 | before `2037699a1463f997cc506bbc43da94d5f09526c4` → after `9c222bb636b76bea62adc5ec30402a306ea133d6` |
| [Transitions / 过渡全路径](performance-transitions-2026-10-25.csv) | 750 | before `9ac27b8c3f154b9ac5a524470ae8f540c21b2237` → after `7e12ae72654acbe6649eaee832c4670d28d9be9e` |
| [Window / 窗口过渡](performance-transition-window-2026-10-25.csv) | 4800 | Same sources; optimized window build without BRP |

## Measurement environment

These runs used Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`, an AMD Ryzen 9 9950X, and 24 visible logical CPUs; Rust `1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0, and Bevy 0.19.1. Builds used the `bsn` feature and the default optimized bench profile: opt-level 3, debuginfo 0, debug assertions and overflow checks disabled. CPU affinity and frequency were managed by the system.

The first five datasets used `Cargo.lock` SHA-256 `1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`. Their binary identities follow; transition sources are listed separately below.

| Dataset / 数据 | Binary SHA-256 / 二进制 SHA-256 |
| --- | --- |
| Keep-alive | `7cc3e143254610b5c790564fc885f81d1c34d6c17811ae345c460327d34f7fef` |
| Parent/child | `bcd55bee6885f4911709d2e6163c0f8fa24e94a6f1b93718aefc813b4820e619` |
| Idle before | `5fef98b47f41029b3f37b29870531dbfa6db9d43a27ad1f8a39a9f2dc7f0784e` |
| Idle after / Steady before | `68322ed332e32f940c357787198deeff67cefa76f77f1d5d919433638e6ace81` |
| Steady after | `b2c8411f7ab816d533d4bc92ccc10c87997853cf0b6d4d319a5e15a08b4ece1a` |
| Initial before | `2669913ec81e8d6dc7a963faff2b672d39d5e420dcc56ef0977d961f62ccc509` |
| Initial after | `d3b3c50dc2b046cc50778f1acf674e53b945b48f14354972ab766cbe62e9da5a` |

## Run order and comparison scope

- **Keep-alive**: two rounds of sequential flat → nested runs, with 24 rows per group and 128 samples per scenario. All summaries are retained without filtering.
- **Parent/child inventory**: 24 rows per group, 128 samples, and 64 warmup navigations. Rounds 3/4 are complete sequential flat → nested pairs, marked `uncontended_pair`. Early overlapping runs are `contended`; the unmatched group is `unpaired`. All 168 rows are retained; comparisons use complete sequential pairs.
- **Idle updates**: two before rounds → build after → two after rounds, all flat with content sizes 1/32/128. Each group has 48 rows, retaining slower groups and outliers.
- **Steady execution**: after building, run before 1 → after 1 → before 2 → after 2. Each group has 48 rows. After adds two fixed focus-scheduling entities; the control group is unchanged.
- **Initial dynamic page**: before 1 → build after → after 1 → before 2 → after 2. Nested content with 128/1024 dynamic nodes, 64 warmup updates, 128 samples per scenario, and 10 rows per group; one request and one history entry.

For sequential pairs, builds and measurements were separate, with windows, renderers, and instrumentation closed. Host load affects wall-clock time; compare control groups and variation across rounds as well. Each idle sample averages 32 updates. These summary percentiles support group comparisons; per-frame latency and heap memory use separate measurements.

## Transition measurements: 2026-10-25

These runs use the same machine above, Bevy 0.19.1, the default optimized profile, `CARGO_INCREMENTAL=0`, and two build jobs. Root lock SHA-256: `0fc9c7ada9ed0bc2dd0f580cd6e4bbada45de8d8c4c5629a9d71ba1601f3aad5`; window lock: `cd1fe4460a631e586c745ec840e3c87b0ce5a9a1638621e3e6ad1935d591bbab`. The 1060×720 window uses X11/WSLg, Vulkan, Mesa 26.0.8 llvmpipe (LLVM 21.1.8), without BRP or screenshots.

| Variant | Source | Benchmark binary SHA-256 |
| --- | --- | --- |
| `before` | `9ac27b8c3f154b9ac5a524470ae8f540c21b2237` | `c09bba2e7fcc86b86cc0435f1420b218e620390601938abdfc0889d4064bed84` |
| `input_batch` | `a8682de49aa7ca659bf0382ffea7ae6530ae6242` | `f079ec75010f12a769a795edf4be6dc2d2f6ab747de550da8ff6c4262fdc8207` |
| `after` | `7e12ae72654acbe6649eaee832c4670d28d9be9e` | `8c2e8fd7e0bfcee863f6881c4173686f441981cbd0705860439a2757f89c2e2e` |

Window binary SHA-256: `before` is `80196bd253515200e9f36c03615e3b9983d685bc6f427e4f9074494fb0543ef9`; `after` is `c50cd11662ce2def4ef37e980a05be59e4ddd617c52f2869a41975c38c52f292`. Sources match the table above.

All formal timing runs were serial and separate from builds. Headless order: `before 1 → input_batch 1 → before 2 → input_batch 2`, followed by before/input_batch comparisons for nested/128 and no-UI/128, then `after 1 → before 3 → after 2`. Each full flat group has 90 rows, 16 samples, and 32/128/1024 nodes. Each focused group has 30 rows. Only `ui=1` includes normal layout and candidate projection. All operations use Replace to keep a one-entry history. See the [method](../../performance.en.md#full-animation-path) for warmup and completion boundaries.

Window order: `before 1 → before 2 → after 1 → after 2`. Each effect warms 60 qualified updates, then collects 120 main and 120 render samples; five modes produce 1200 rows per run. Application time is real, so sampled animation progress and navigation counts can vary with update intervals; this is not a fixed-work CPU comparison. CSV `elapsed_ns` is wall-clock; `update_interval_ns` records main-loop update intervals only.

All rows and outliers are retained. For 1024-node cache hits with held exit, headless p50 falls from 1.009–1.022 ms/update to 0.492–0.534 ms/update; each sample averages 32 updates. With 1024 fresh nodes and transitions disabled, the publication update includes immediate cleanup; p50 falls from 392–403 ms to 199–214 ms. The changes batch input eligibility/ancestor observations and index registry reads within each immutable adoption validation. All original checks and post-write revalidation remain.

Small cases and controls vary; some `after 1` controls are slower, so not every difference is attributable to the changes. Window idle and animation timings move together as well. Window data verifies real scheduling and submission behavior; it does not establish a hardware GPU or presentation-FPS improvement. A cleanup EntityRef-read candidate saved only about 5%, below this run's 10% retention threshold, and was reverted. No cross-write cache or reduced revalidation was added.

## Maintenance

Historical files retain all rows and quality annotations. Store new comparisons in new files with source, toolchain, binary, commands, and environment. Previous candidate patches, phase profiles, and temporary window probes remain available in Git history.
