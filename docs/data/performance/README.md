# Historical performance data

English | [简体中文](README.zh-CN.md)

Public benchmark data is retained with complete source commits. See [performance measurement](../../performance.md) for commands and timing boundaries. Benchmark CSVs contain summaries; window data retains individual samples.

| Dataset / 数据 | Rows / 行 | Source / 源码 |
| --- | --- | --- |
| [Keep-alive](performance-keep-alive-2026-10-08.csv) | 96 | `1070143dc257bd35b7617cc527127fdfc0cf5a57` |
| [Parent/child / 父子库存](performance-instances-2026-10-11.csv) | 168 | `00ba8be819143e7472b32cdf80ff74fc6e5a8062` |
| [Idle / 空闲更新](performance-idle-2026-10-17.csv) | 192 | before `b3bd15397737f4e0cd59bc951f4a73c3888df76b` → after `d731ccbeabe1232f71d1727c8f5e061e091fb854` |
| [Steady / 稳态执行](performance-steady-2026-10-17.csv) | 192 | before `d731ccbeabe1232f71d1727c8f5e061e091fb854` → after `a5b16d0348aba65b361b478ed5995e2102b07734` |
| [Initial dynamic page / 初始动态页面](performance-initial-steady-2026-10-18.csv) | 40 | before `2037699a1463f997cc506bbc43da94d5f09526c4` → after `9c222bb636b76bea62adc5ec30402a306ea133d6` |
| [Transitions / 过渡全路径](performance-transitions-2026-10-25.csv) | 750 | before `9ac27b8c3f154b9ac5a524470ae8f540c21b2237` → after `7e12ae72654acbe6649eaee832c4670d28d9be9e` |
| [Window / 窗口过渡](performance-transition-window-2026-10-25.csv) | 4800 | Same sources; optimized window build without BRP |
| [Retained steady updates](performance-retained-steady-2026-11-12.csv) | 372 | `c96836592f553a3e2b8c898063280e013f1764a1` → `bd937055bd5cf218b5c398e89a55a5437f7818b2` |
| [Observation controls](performance-observation-controls-2026-11-12.csv) | 194 | Same sources; ordinary navigation and initial dynamic pages |
| [Public query consumers](performance-public-queries-2026-11-12.csv) | 240 | `4ad3c77f9bdc0b38159f03ae5b141bcd91520a88` → `96787c790507cca1ee3b8e474edafcf75dbbe7fc` |
| [Completed-request maintenance and retained hits](performance-maintenance-2026-11-14.csv) | 80 | `aaca8de7bdfaa7c874ab00b5fc6f4c63a921f576` → `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79` |
| [Shared ordinary ancestors](performance-query-ancestors-2026-11-14.csv) | 280 | `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79` → `f409d6f1f9295086c56dbbf0a818cf4c9fcc0ea8` |

## Measurement environment

This environment describes datasets through October 2026. The November measurements have a separate record below.

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

All formal timing runs were serial and separate from builds. Headless order: `before 1 → input_batch 1 → before 2 → input_batch 2`, followed by before/input_batch comparisons for nested/128 and no-UI/128, then `after 1 → before 3 → after 2`. Each full flat group has 90 rows, 16 samples, and 32/128/1024 nodes. Each focused group has 30 rows. Only `ui=1` includes normal layout and candidate projection. All operations use Replace to keep a one-entry history. See the [method](../../performance.md#full-animation-path) for warmup and completion boundaries.

Window order: `before 1 → before 2 → after 1 → after 2`. Each effect warms 60 qualified updates, then collects 120 main and 120 render samples; five modes produce 1200 rows per run. Application time is real, so sampled animation progress and navigation counts can vary with update intervals; this is not a fixed-work CPU comparison. CSV `elapsed_ns` is wall-clock; `update_interval_ns` records main-loop update intervals only.

All rows and outliers are retained. For 1024-node cache hits with held exit, headless p50 falls from 1.009–1.022 ms/update to 0.492–0.534 ms/update; each sample averages 32 updates. With 1024 fresh nodes and transitions disabled, the publication update includes immediate cleanup; p50 falls from 392–403 ms to 199–214 ms. The changes batch input eligibility/ancestor observations and index registry reads within each immutable adoption validation. All original checks and post-write revalidation remain.

Small cases and controls vary; some `after 1` controls are slower, so not every difference is attributable to the changes. Window idle and animation timings move together as well. Window data verifies real scheduling and submission behavior; it does not establish a hardware GPU or presentation-FPS improvement. A cleanup EntityRef-read candidate saved only about 5%, below this run's 10% retention threshold, and was reverted. No cross-write cache or reduced revalidation was added.

## Retained steady updates: 2026-11-12

The new `retained_steady` mode keeps the same active page Entity, a 128-node shared parent, and a 32-node active leaf. It accumulates 0/1/4 retained siblings with 32/1024 dynamic descendants each. Idle and requested-scroll-position updates are measured separately; factories, activity, input eligibility, history, layout geometry, and eventual scope cleanup are checked outside timing. `ui=1` installs real layout and picking systems without a renderer or injected pointer input. Navigation request records number 1/3/9 across inventory levels; compare the same row across variants, where that work is identical.

These runs used Bevy 0.20.1, the Rust/CPU/OS listed above, `bsn`, the default optimized bench profile, and two build jobs. Root lock SHA-256: `f34d67d2d70b85a1c113d08f321f018e4c39d72f6c5de6a0b6fa1bf62f6cf227`. CPU affinity and frequency remained system-managed. No device, renderer, GPU, or power measurement is included.

| Variant | Production source | Benchmark binary SHA-256 |
| --- | --- | --- |
| `before` | `c96836592f553a3e2b8c898063280e013f1764a1` | `30e5654fc1f1600bf6c38fab2cc3863a75e22d7774fa63e16f81fc1c67a6cce3` |
| `shared_observation` | `891ee53be78da05a0a782131e08d376739294f8f` | `27aee61efc82381005b7967250361894e664f3f224fa018f7b93a2c1cc677086` |
| `entity_hash` | `2282510fc9d14253297c0d6f701bf28ac806a6d7` | `7ea6283be4ced98d5d8b3ad58bff5ebb7617c130a0d687474ab3d7409ca6c204` |
| `after` | `bd937055bd5cf218b5c398e89a55a5437f7818b2` | `4b46b752bbeae9900ccc744d4303e17dea752c487958dcbd37bf8cab35cca9ec` |

Build with `CARGO_BUILD_JOBS=2 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked --no-run`, then keep each executable fixed while comparing. Commands and flags are in the [measurement guide](../../performance.md#retained-siblings-with-fixed-active-content). Nested formal runs use 32 samples of 32 updates, with 64 warmup updates; flat and ordinary control runs use 16 samples. The later paired initial-page controls use 32. Each percentile describes batch-average CPU wall time, not a per-frame presentation percentile.

All timing was serial and separate from builds. Initial order was two `before` router/UI pairs, then shared-observation runs with a `before` UI replay. The hash candidate was compared with a shared-observation replay. Two final `before → entity_hash` pairs ran separately for UI and router-only modes, followed by two `after` router/UI pairs and ordinary controls. The CSV retains every committed-variant row, including early hash runs marked `variable_round`; this marks observed timing variation, not a proven external cause. Later stable replays support the conclusion without deleting those rows.

For nested content with 1024 descendants per retained page, final idle p50 ranges in microseconds are:

| Mode / retained pages | Before replays | After |
| --- | ---: | ---: |
| Router-only / 0 | 55.8–56.7 | 43.1–43.9 |
| Router-only / 4 | 650.4–659.7 | 192.8–200.5 |
| UI / 0 | 90.5–93.2 | 76.9–78.1 |
| UI / 4 | 1182.6–1204.4 | 717.4–737.9 |

The corresponding UI / 4 scroll-update p50 falls from 1191.3–1194.1 to 715.2–739.3 microseconds. Controls do not establish a universal speedup: ordinary initial-page cases vary by a few percent, and UI totals still include Bevy work. The lazy outlet count avoids an unnecessary full leaf scan introduced by eager fact preparation. Content vectors and membership sets are shared without copying within the observation; allocation counts and bytes were not instrumented, so no numeric allocation reduction is claimed.

The retained changes share parent content and identical preservation proofs inside one immutable World borrow, index registry reads, and use Bevy's Entity hash set. Role-specific checks and fresh observations after writes remain. An exploratory single-entry memo replacement did not produce a repeatable CPU benefit and was reverted. Remaining retained content still receives one complete preservation observation per required phase; cross-frame invalidation machinery and speculative multi-router optimization were deferred. These results establish library CPU improvements, not application FPS or battery gains.

## Public query consumers: 2026-11-12

The `public_queries` fixture has 32 or 1024 active dynamic descendants and the same number of retained descendants. It measures resident consumer systems after publication, including their application-component writes and normal Router updates. `per_target` queries each descendant; `same_root` repeats the active-root query the same number of times. Scroll rows measure a missing intent. See the [method](../../performance.md#application-query-consumers).

These runs use the November environment above, Bevy 0.20.1, `bsn`, the same root lock, optimized bench profile, and two build jobs. Each row has 32 samples averaging four updates, after 16 warmup updates. No renderer, UI layout, pointer injection, device measurement, or heap-allocation counter is included.

| Variant | Source | Binary SHA-256 |
| --- | --- | --- |
| Before fixture | `4ad3c77f9bdc0b38159f03ae5b141bcd91520a88` | `d1e9e135720d4873570863246facf4e23661fb1e1e8ef1c18566566858c3b632` |
| Shared reader | `96787c790507cca1ee3b8e474edafcf75dbbe7fc` | `c897092c27c575ad5110063671971b866d748c9cb7296e79d3316d1f4070306b` |

Build with the existing `cargo bench ... --no-run` command, then run the fixed executable with `ROUTER_BENCH_MODE=public_queries` and `ROUTER_BENCH_SHAPE=flat|nested`. Builds and timings were serial. Run order was before flat/nested rounds 1–2, exploratory reader work, before replay 3, then shipped-reader round 1 → before replay 4 → shipped-reader round 2. All before and shipped-reader rows remain in the CSV. Preliminary uncommitted reader experiments are separate from this dataset. Some flat control and repeated-root rows vary substantially; no cause is established.

The primary comparison uses old entry points and the shared reader **inside the shipped binary**, with identical content, writes, and run conditions. The before fixture has four run conditions; the shipped fixture has five, so cross-binary controls also include that small scheduling difference. Per-update p50 ranges in microseconds across the two final rounds are:

| Shape / per-target queries | Empty loop | Individual activity | Shared activity | Individual inputs | Shared inputs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Flat / 2048 | 158.2–160.9 | 1383.2–1508.7 | 349.3–350.1 | 1389.3–1427.8 | 351.2–353.2 |
| Nested / 2048 | 153.1–156.2 | 1488.8–1498.4 | 650.2–653.4 | 1489.3–1506.7 | 652.3–659.0 |
| Nested / 64 | 18.1–18.2 | 57.0–57.1 | 34.1–34.2 | 56.9–57.1 | 34.1–34.4 |

For nested repeated-root activity, p50 changes from 2096.4–2122.3 to 162.1–164.8 microseconds. Nested per-target scroll misses change from 1483.0–1497.5 to 667.8–669.7 microseconds. These gains require creating one reader outside the loop; existing individual methods remain available. They establish CPU cost for this consumer workload, not isolated method latency or application FPS. Small-list special casing did not establish a repeatable additional gain and was removed.

## Completed-request maintenance and retained hits: 2026-11-14

The `maintenance` fixture separates historical trace size from live content, then measures warmed retained navigation. See the [method](../../performance.md#closed-traces-and-warmed-cache-hits). These runs use the November environment, Bevy 0.20.1, the same root lock, `bsn`, the default optimized bench profile, and two build jobs. Each row has 32 samples. No renderer, UI layout, pointer injection, device measurement, or allocation counter is included.

| Variant | Source | Binary SHA-256 |
| --- | --- | --- |
| `before` | `aaca8de7bdfaa7c874ab00b5fc6f4c63a921f576` | `c3a958db4124f354e8a15c2c0f77245e71dabbb959bad6732f25a112f9b7a459` |
| `prepared_publication` | `3cda2f803bacec6714044d26ea0a594df681ce87` | `8cb51b4d925194232509de93ed61957d302cb51b11801877585ce0d70c4bd2e1` |
| `local_input_index` | `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79` | `162efbaf516e0c399b318964a49cf41348753616b8f4701145f84111bbe26846` |

Build and freeze executables with the existing `cargo bench ... --no-run` command, then select `ROUTER_BENCH_MODE=maintenance` and `ROUTER_BENCH_SHAPE=flat|nested`. Builds and timing runs were separate and serial. Run order was before rounds 1–2 → prepared-publication round 1 → before round 3 → prepared-publication round 2 → local-input-index round 1 → prepared-publication round 3 → local-input-index round 2. Each round ran flat then nested. All 80 rows, including variable small cases, are retained.

With 54 live entities, one history entry, and 64 terminal requests, idle p50 previously grew from 14.8–16.3 µs at 768 trace events to 35.9–36.8 µs at 8960 events. The final variant spans 13.0–14.6 µs across all trace sizes. Reusing exact-request closed traces removes that payload-dependent comparison; late diagnostics still update independently. These idle samples average 32 updates each.

For two warmed pages with 1024 dynamic descendants each, complete Replace-hit p50 changes from 5.81–5.92 ms to 3.59–3.62 ms with prepared input changes and absent-marker capture skipping, then to 1.05–1.09 ms with a local input-membership index during retained validation. Each operation takes one update; entities, history, and factories stay fixed. The combined result is about 82% lower CPU wall time for this workload. The three changes in the middle variant were measured together, so their individual contributions are not inferred. Small 32-node cases vary and do not establish a general speedup.

The input gate keeps its ordered projection and checks the complete source again when binding prepared values. The retained index exists only during one immutable validation, at the original role-specific predicate. Neither change removes post-write proofs or adds persistent cache invalidation. These measurements describe library CPU work, not application FPS.

## Shared ordinary ancestors: 2026-11-14

The existing `public_queries` fixture, November environment, Bevy 0.20.1, root lock, `bsn`, and optimized profile remain as described above. Each row has 32 samples averaging four updates. The before source is `91fbcf3fbbf8dfebd005c5a9c89d675d4db7df79`, binary SHA-256 `162efbaf516e0c399b318964a49cf41348753616b8f4701145f84111bbe26846`. The shipped reader source is `f409d6f1f9295086c56dbbf0a818cf4c9fcc0ea8`, binary SHA-256 `9c565eb3042804d785db0c5c020c50d981fec40bfb632cfdd03a64673917d29d`.

Run fixed executables with `ROUTER_BENCH_MODE=public_queries` and `ROUTER_BENCH_SHAPE=flat|nested`. Builds and timings were separate and serial. Order: before round 1 → exploratory reader work → before round 2 → shipped reader round 1 → before round 3 → shipped reader round 2, flat then nested each round. All 280 before and shipped-reader rows are retained. An uncommitted first candidate reduced nested cost but added flat overhead; it was replaced by recording only ordinary nodes in the existing cycle set, avoiding terminal registered-node insertion/removal and duplicate target insertion.

For 2048 distinct targets, p50 ranges in microseconds are:

| Shape / API | Before | Shared ancestors |
| --- | ---: | ---: |
| Nested / activity | 640.7–650.6 | 349.9–351.1 |
| Nested / current inputs | 643.9–652.7 | 347.5–349.1 |
| Nested / scroll miss | 638.8–650.7 | 356.9–358.0 |
| Flat / activity | 340.1–355.4 | 348.7–348.9 |
| Flat / current inputs | 342.3–347.6 | 349.6–351.0 |
| Flat / scroll miss | 346.6–356.2 | 355.4–362.1 |

Nested activity totals fall about 45%; flat input and scroll cases retain a small overhead and establish no additional gain. Repeated-root queries and empty-loop controls stay near their existing cost. The reader shares completed resolutions only for ordinary ancestors within one immutable borrow. Registered boundaries still distinguish direct targets from inherited content, including negative results and query order. This does not add a persistent index or change the public API.

## Maintenance

Historical files retain all rows and quality annotations. Store new comparisons in new files with source, toolchain, binary, commands, and environment. Previous candidate patches, phase profiles, and temporary window probes remain available in Git history.
