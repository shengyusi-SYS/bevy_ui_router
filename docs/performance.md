# Performance measurement

English | [简体中文](performance.zh-CN.md)

Run public-API benchmarks to measure CPU-side navigation, cleanup, and idle updates. The main entry is [router_baseline.rs](../crates/bevy_ui_router/benches/router_baseline.rs); [bsn_content_control.rs](../crates/bevy_ui_router/benches/bsn_content_control.rs) provides a direct Scene-construction control.

## Run and compare

Run optimized builds from the repository root:

```sh
cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > flat.csv
ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > nested.csv
cargo bench -p bevy_ui_router --bench bsn_content_control --features bsn --locked > control.csv
```

Ordinary scenarios use 128 samples by default; `retained_steady`, `public_queries`, `maintenance`, and `history` use 32. Set a positive `ROUTER_BENCH_SAMPLES` to adjust this. `ROUTER_BENCH_SHAPE` accepts `flat` (default) or `nested`; both use the same content-node budget, with the latter forming a shallow four-level tree.

Fix the machine, toolchain, dependencies, features, and optimization settings, and repeat at least twice. Build and measure sequentially, preserving all paired output. Compare absolute p50/p95, update counts, and variation for equivalent work before choosing a path to profile.

## Benchmark modes

| `ROUTER_BENCH_MODE` | Work measured | Content size |
| --- | --- | --- |
| Unset | Idle, push/replace/back, failure/recovery, repeated navigation | 1/32/128 nodes per page |
| `keep_alive` | Retention disabled, initial miss, warmed hit, LRU eviction, and corresponding idle updates | 1/32/128 nodes per page |
| `instances` | Fresh parent/child, retained parent/fresh child, parent/child hits, dependency eviction | 1/32/128 nodes each for parent and child |
| `steady` | Dynamic content after initial navigation, comparing two/three frames, ordinary scroll markers, and a no-Router control | 128/1024 nodes |
| `retained_steady` | The same active leaf and shared parent with 0/1/4 retained siblings; idle and scroll-position updates | Parent 128, active leaf 32; each retained leaf 32/1024 dynamic nodes |
| `public_queries` | Resident application consumers: empty loop, individual entry points, shared reader; repeated root and distinct targets | 32/1024 active descendants and the same number of retained descendants |
| `maintenance` | Idle updates after completed cleanup, and warmed retained Replace hits | 0/32/128 retired descendants per request with no current descendants; 32/1024 descendants per retained page |
| `history` | Same-page Replace with fixed visit history, comparing saved scroll records with an unmarked control | 1/128/1024 visits; parent 128 and active leaf 32 dynamic descendants |
| `transitions` | Disabled, enter-only, and held exit; publication, animation updates, release, finish, and idle | 32/128/1024 nodes per page |

### Full animation path

```sh
ROUTER_BENCH_MODE=transitions ROUTER_BENCH_UI=1 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > transitions-ui.csv
ROUTER_BENCH_MODE=transitions cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > transitions-router.csv
```

Both variants use the public driver and completion commands. `ROUTER_BENCH_UI=1` adds real UI layout, UiStack, and picking systems, asserting valid page geometry and camera input, without a renderer. Omit it to observe Router and basic scheduling. `ROUTER_BENCH_CONTENT=32|128|1024` selects one content size; `ROUTER_BENCH_SHAPE=nested` exercises shared ancestor paths.

Fresh creation and warmed cache hits are measured separately. Sixty-four short Replace navigations warm request eviction and lifecycle paths, followed by idle updates. Each measured sample executes 32 animation updates and 32 post-finish updates; the disabled control performs the same update count. `holding_update` and `idle_update` are batch means per update; other phases measure one operation. Publication ends when the new route becomes active, while held-exit cleanup is charged to release. Publication alone cannot establish total navigation cost.

The window example provides finite automatic sampling:

```sh
ROUTER_TRANSITION_PERF=/tmp/transitions-window.csv cargo run --release --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
```

It warms and samples idle, slide/scale-rotation, and enter-only/held-exit modes, then writes CSV once on completion. Main World timing spans `First` to `Last`. Render timing brackets submission in `RenderSystems::Render`, including present scheduling/waits and possibly pipeline queue work in the same set. Update intervals are not presentation FPS, and these CPU measurements are not GPU execution times. Close BRP, screenshot capture, and other builds during comparisons; fix the window, renderer, and machine conditions.

### Keep-alive comparison

```sh
ROUTER_BENCH_MODE=keep_alive cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > keep-alive.csv
```

Disabled retention creates a page and cleans up its predecessor on every navigation. Miss resets the parent scope before each sample. Hit reuses the original page entity after warmup. LRU cycles through four pages with capacity two, creating fresh pages and evicting old instances. CSV output records capacity, live entities, active/inactive pages, and factory calls. Each group exits the parent scope and verifies that pages, content, and the old layout are released while the host remains.

### Parent/child inventory comparison

```sh
ROUTER_BENCH_MODE=instances ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > instances.csv
```

Warm up with 64 navigations to bring terminal request records to stable capacity. Parents and children select retention independently. Dependency eviction cycles through three parent/child pairs at capacity two. Untimed checks cover entities, parent state, activity, input eligibility, history, and factory calls. Each group releases parent/child content through fallback.

### Initial dynamic page comparison

```sh
ROUTER_BENCH_MODE=steady ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > steady.csv
```

Complete one navigation and create content through the public World after publication. A transparent pathless layout supplies the three-frame control. An ordinary `RouteScroll + ScrollPosition` marker measures the idle cost of scroll observation.

### Retained siblings with fixed active content

```sh
ROUTER_BENCH_MODE=retained_steady ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > retained-steady-router.csv
ROUTER_BENCH_MODE=retained_steady ROUTER_BENCH_SHAPE=nested ROUTER_BENCH_UI=1 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > retained-steady-ui.csv
```

The shared parent has 128 ordinary dynamic descendants and the active leaf has 32. Within each fixture, visit sibling pages and return to the original active leaf to accumulate 0, 1, and 4 retained siblings, each with 32 or 1024 dynamic descendants. All descendants are added through the public World after publication. The active leaf and shared parent entities stay identical as inventory grows. Both flat and nested shapes keep these node budgets unchanged.

Each stage warms 64 updates, then measures idle updates and updates changing only the active `ScrollPosition`. Each sample averages 32 updates; the scroll row includes the component write. `ROUTER_BENCH_UI=1` installs normal layout, UiStack, picking systems, and camera input, and checks real scroll overflow and the computed displayed position. `0` or an unset value measures Router without UI layout. No pointer is injected. CSV records node budgets, retained pages, live entities, request records, factory builds, and measured scroll writes. Untimed assertions verify activity/input eligibility, unchanged entities and counts, no factory rebuild during sampling, and complete content cleanup when leaving the parent scope.

### Application query consumers

```sh
ROUTER_BENCH_MODE=public_queries cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > queries-flat.csv
ROUTER_BENCH_MODE=public_queries ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > queries-nested.csv
```

A resident application system runs after `PublishSnapshots` with a mutable application-component query. Every update queries either the same active root repeatedly or each active/retained dynamic descendant. The shared `RouteQueries` consumer creates one reader per system invocation; the individual-entry consumers perform the same application writes and target selection. An empty-loop control includes the same writes and run conditions. Scroll queries measure a missing intent; successful pending lookups are covered by the public scroll journeys.

Each fixture keeps one active page, one retained sibling, two history entries, and fixed entities/factory counts. Default sampling is 32 samples of four updates after 16 warmup updates; `ROUTER_BENCH_BATCH=1` samples individual updates. CSV `frames_per_sample` records the actual batch. Percentiles describe those sample means, including normal Router scheduling and the consumer, not isolated method latency. Activity, inputs, and inactive rejection are checked outside timing. This headless mode includes no UI layout or pointer input.

### Closed traces and warmed cache hits

```sh
ROUTER_BENCH_MODE=maintenance cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > maintenance-flat.csv
ROUTER_BENCH_MODE=maintenance ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > maintenance-nested.csv
```

`terminal_history_idle` completes 64 Replace navigations, retiring 0, 32, or 128 dynamic descendants each time. The current page has no content descendants, history has one entry, and the latest 64 terminal requests remain. After 64 idle warmup updates, each sample averages 32 ordinary updates. CSV records trace-event and diagnostic counts outside timing, separating historical payload size from the current entity budget.

`retained_hit` creates two pages and adds 32 or 1024 dynamic descendants to each after publication. It warms 64 alternating Replace hits, then times complete alternating navigations with one active and one retained page. The page entities and factory count stay unchanged; the group finally leaves the parent scope and checks content release. Both groups are headless and perform content census queries outside timing.

`sample_scope` distinguishes `mean_update` from `complete_navigation`. In navigation rows, `updates_per_sample=0` denotes operation-scoped sampling; `min_updates` and `max_updates` give the actual update range. Trace counts, current content, history, and factories are checked outside the measured samples.

### Scroll records across visits

```sh
ROUTER_BENCH_MODE=history cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > history-flat.csv
ROUTER_BENCH_MODE=history ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > history-nested.csv
```

This mode always installs normal UI layout, picking systems, and camera input without a renderer. Repeated Push to the same URL creates 1, 128, or 1024 visits while reusing one page and its parent. Each visit samples a distinct integral scroll position through real layout. The marked and unmarked groups keep the same dynamic content budget, entities, and factory count.

After 64 Replace warmups, each sample times one complete same-URL Replace while keeping the history length fixed. Timing includes enqueue, ordinary updates and UI layout, publication, and terminal completion with no foreground cleanup. This measures the complete navigation cost as saved visit records grow. The unmarked control includes the same canonical history work.

Untimed checks verify real displayed positions, history, unchanged entities, and no factory rebuild. At most two Back navigations check that marked pages restore different earlier visit positions while unmarked pages keep their current value. The one-visit control adds a second visit only after timing for this check. CSV records the marker flag, history and entity counts, update range, factory calls, and Back checks; each group finally releases its content.

## Timing boundaries

The Router fixture installs TaskPool, Asset, and Scene runtime and advances through ordinary `App::update`, navigation APIs, and snapshots. Navigation timing starts before enqueue and ends at a terminal request, no running request, and completed foreground cleanup. It includes building, resolve/apply, input authorization refresh, publication, and old-content cleanup. Initialization, target-string construction, detailed assertions, and CSV output are untimed.

| Metric | Interpretation |
| --- | --- |
| Navigation p50/p95 | `Instant` wall-clock for each complete operation, using nearest-rank percentiles |
| Idle p50/p95 | Percentiles of sample means, with each sample averaging 32 consecutive updates |
| Entity and request counts | Stable state at the end of a scenario group; failure rows are read after their corresponding recovery |
| Update limit | An assertion that each step settles within 32 updates |

Ordinary modes measure headless Router and basic scheduling. The `transitions` and `retained_steady` UI options, and the `history` mode, additionally cover normal layout and input candidate projection; without injected pointers they do not measure actual hit testing. Use the window mode above for submission; GPU execution, actual presentation, and I/O require their corresponding environment. The root workspace and separate window application can use different executors/features, so record the actual build configuration before comparisons.

The direct BSN control keeps the host/layout/outlet and measures Scene construction, resolve/apply, old-page deletion, and ordinary updates. It shows engine construction cost; Router benchmarks additionally perform history, authorization, isolation, and recovery work.

## Behavior checks

Extended checks run the benchmark fixtures' behavior assertions without timing thresholds:

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=retained_steady ROUTER_BENCH_SHAPE=nested ROUTER_BENCH_UI=1 cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=public_queries ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=maintenance ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=history ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

`instances`, `steady`, and `transitions` also support `-- --test`. The `retained_steady` smoke check keeps both workloads and 0/4 retained siblings with 32 descendants each, using two samples of four updates. `maintenance` smoke retains 64 terminal records, retiring 0/32 descendants and using 32 descendants per cached page; it takes two samples, with two updates per idle sample. `history` smoke uses 1/32 visits, both marker groups, and two samples through real UI layout. Add `ROUTER_BENCH_UI=1 ROUTER_BENCH_CONTENT=32` to check the transition fixture through real layout with a small content budget. Use the `cargo bench` commands above for timing.

## Save and read results

Record the source revision, lockfile hash, Rust/Bevy versions, machine and OS, features, build profile, binary hash, command, and complete output. Preserve interference and outlier annotations. Paired comparisons use equal content budgets and work. Establish stable entity and record counts before investigating sustained growth with a heap profiler.

The [historical data index](data/performance/README.md) retains measurements with full source identities for inspecting fixture evolution and output formats. Results apply to their listed sources; rerun the current checkout rather than treating historical values as current guarantees.
