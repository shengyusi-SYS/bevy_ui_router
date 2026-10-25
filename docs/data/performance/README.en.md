# Historical performance data

English | [简体中文](README.md)

Five public benchmark datasets are retained from runs using complete source commits. See [performance measurement](../../performance.en.md) for commands and timing boundaries. Each CSV contains sample summaries with the following source identity.

| Dataset / 数据 | Rows / 行 | Source / 源码 |
| --- | --- | --- |
| [Keep-alive](performance-keep-alive-2026-10-08.csv) | 96 | `1070143dc257bd35b7617cc527127fdfc0cf5a57` |
| [Parent/child / 父子库存](performance-instances-2026-10-11.csv) | 168 | `00ba8be819143e7472b32cdf80ff74fc6e5a8062` |
| [Idle / 空闲更新](performance-idle-2026-10-17.csv) | 192 | before `b3bd15397737f4e0cd59bc951f4a73c3888df76b` → after `d731ccbeabe1232f71d1727c8f5e061e091fb854` |
| [Steady / 稳态执行](performance-steady-2026-10-17.csv) | 192 | before `d731ccbeabe1232f71d1727c8f5e061e091fb854` → after `a5b16d0348aba65b361b478ed5995e2102b07734` |
| [Initial dynamic page / 初始动态页面](performance-initial-steady-2026-10-18.csv) | 40 | before `2037699a1463f997cc506bbc43da94d5f09526c4` → after `9c222bb636b76bea62adc5ec30402a306ea133d6` |

## Measurement environment

These runs used Linux WSL2 `6.18.33.2-microsoft-standard-WSL2`, an AMD Ryzen 9 9950X, and 24 visible logical CPUs; Rust `1.99.0-nightly (ba28ff76f 2026-08-13)` / LLVM 23.1.0, and Bevy 0.19.1. Builds used the `bsn` feature and the default optimized bench profile: opt-level 3, debuginfo 0, debug assertions and overflow checks disabled. CPU affinity and frequency were managed by the system.

The original measured `Cargo.lock` SHA-256 was `1cb3ee92ac2d388259b5511ef5867c67b85e7044f3b5d76752b9d1fb42205ce8`. Binary identities:

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

## Maintenance

Historical files retain all rows and quality annotations. Store new comparisons in new files with source, toolchain, binary, commands, and environment. Previous candidate patches, phase profiles, and temporary window probes remain available in Git history.
