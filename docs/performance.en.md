# Performance measurement

English | [简体中文](performance.md)

Run public-API benchmarks to measure CPU-side navigation, cleanup, and idle updates. The main entry is [router_baseline.rs](../crates/bevy_ui_router/benches/router_baseline.rs); [bsn_content_control.rs](../crates/bevy_ui_router/benches/bsn_content_control.rs) provides a direct Scene-construction control.

## Run and compare

Run optimized builds from the repository root:

```sh
cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > flat.csv
ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > nested.csv
cargo bench -p bevy_ui_router --bench bsn_content_control --features bsn --locked > control.csv
```

Ordinary scenarios use 128 samples by default. Set a positive `ROUTER_BENCH_SAMPLES` to adjust this. `ROUTER_BENCH_SHAPE` accepts `flat` (default) or `nested`; both use the same content-node budget, with the latter forming a shallow four-level tree.

Fix the machine, toolchain, dependencies, features, and optimization settings, and repeat at least twice. Build and measure sequentially, preserving all paired output. Compare absolute p50/p95, update counts, and variation for equivalent work before choosing a path to profile.

## Benchmark modes

| `ROUTER_BENCH_MODE` | Work measured | Content size |
| --- | --- | --- |
| Unset | Idle, push/replace/back, failure/recovery, repeated navigation | 1/32/128 nodes per page |
| `keep_alive` | Retention disabled, initial miss, warmed hit, LRU eviction, and corresponding idle updates | 1/32/128 nodes per page |
| `instances` | Fresh parent/child, retained parent/fresh child, parent/child hits, dependency eviction | 1/32/128 nodes each for parent and child |
| `steady` | Dynamic content after initial navigation, comparing two/three frames, ordinary scroll markers, and a no-Router control | 128/1024 nodes |
| `transitions` | Disabled, enter-only, and held exit; publication, animation updates, release, finish, and idle | 32/128/1024 nodes per page |

### Full animation path

```sh
ROUTER_BENCH_MODE=transitions ROUTER_BENCH_UI=1 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > transitions-ui.csv
ROUTER_BENCH_MODE=transitions cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > transitions-router.csv
```

Both variants use the public driver and completion commands. `ROUTER_BENCH_UI=1` adds real UI layout, UiStack, and picking systems, asserting valid page geometry and camera input, without a renderer. Omit it to observe Router and basic scheduling. `ROUTER_BENCH_CONTENT=32|128|1024` selects one content size; `ROUTER_BENCH_SHAPE=nested` exercises shared ancestor paths.

Fresh creation and warmed cache hits are measured separately. Sixty-four short navigations warm request eviction and lifecycle paths, followed by idle updates. Each measured sample executes 32 animation updates and 32 post-finish updates; the disabled control performs the same update count. `holding_update` and `idle_update` are batch means per update; other phases measure one operation. Publication ends when the new route becomes active, while held-exit cleanup is charged to release. Publication alone cannot establish total navigation cost.

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

## Timing boundaries

The Router fixture installs TaskPool, Asset, and Scene runtime and advances through ordinary `App::update`, navigation APIs, and snapshots. Navigation timing starts before enqueue and ends at a terminal request, no running request, and completed foreground cleanup. It includes building, resolve/apply, input authorization refresh, publication, and old-content cleanup. Initialization, target-string construction, detailed assertions, and CSV output are untimed.

| Metric | Interpretation |
| --- | --- |
| Navigation p50/p95 | `Instant` wall-clock for each complete operation, using nearest-rank percentiles |
| Idle p50/p95 | Percentiles of sample means, with each sample averaging 32 consecutive updates |
| Entity and request counts | Stable state at the end of a scenario group; failure rows are read after their corresponding recovery |
| Update limit | An assertion that each step settles within 32 updates |

These headless CPU-side measurements cover Router and basic scheduling. Measure UI layout, native picking, window presentation, GPU, and I/O in their application scenarios. The root workspace and separate window application can use different executors/features, so record the actual build configuration before comparisons.

The direct BSN control keeps the host/layout/outlet and measures Scene construction, resolve/apply, old-page deletion, and ordinary updates. It shows engine construction cost; Router benchmarks additionally perform history, authorization, isolation, and recovery work.

## Behavior checks

CI runs the benchmark fixtures' behavior assertions:

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

`instances` and `steady` also support `-- --test`. Use the `cargo bench` commands above for timing.

## Save and read results

Record the source revision, lockfile hash, Rust/Bevy versions, machine and OS, features, build profile, binary hash, command, and complete output. Preserve interference and outlier annotations. Paired comparisons use equal content budgets and work. Establish stable entity and record counts before investigating sustained growth with a heap profiler.

The [historical data index](data/performance/README.en.md) retains five measurements with full source identities for inspecting fixture evolution and output formats. Run the current checkout to obtain current performance results. Past candidate patches and phase probes remain in Git history.
