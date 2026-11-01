# Verification and release checks

[中文](verification.md) | [English](verification.en.md)

This page lists checks for development, integration, and release preparation. The [contracts](../contracts/README.en.md) define behavior, and the [integration guide](../guides/integration.en.md) explains setup. Run commands from the repository root and record the source state, features, toolchain, and environment.

## Choose checks for the change

| Change | Checks |
| --- | --- |
| Documentation | Matching Chinese/English content, file and anchor links, commands and actual targets, release-content scan |
| Local Core/View logic | Formatting, affected crate tests, corresponding public behavior; add standalone consumers for public API changes |
| ECS/BSN setup, Scenes, or outlets | Affected crates and facade tests, real `App::update`, and the official Scene path |
| Lifecycle, failure, terminal states, cleanup, or publication | Full affected crates, public navigation/recovery journeys, and standalone facade consumers; cover ownership settlement, rejection, changes after writes, and subsequent requests |
| Input/focus | Active hits, inactive rejection, revoking started gestures, focus release, and continued interaction with other valid targets |
| Features, dependencies, examples, or package configuration | Dependency checks, minimal consumers, examples, and window compilation; package configuration also needs archive verification |
| Performance | Relevant behavior tests and same-machine optimized-build comparisons; see [performance](../performance.en.md) |

Observe results through production entry points. To construct special conditions, let test-only seams change facts read by the production predicate, then let normal logic produce the outcome. Test combined behavior together, such as Back after a query-triggered rebuild, or host re-registration after losing the host during cleanup.

## Automated checks

Use a Rust toolchain satisfying the `rust-version` requirements of the resolved dependencies. This project uses edition 2024. Install rustfmt, then run:

```sh
cargo fetch --locked
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
```

These are the main checks in [CI](../../.github/workflows/ci.yml). Standalone consumers resolve dependencies offline in their own workspaces; `cargo fetch` prepares the registry and dependency cache. Assign one validation owner to run Cargo/rustc sequentially against a shared target directory.

CI uses `--locked` to reproduce the committed dependency snapshot; see [integration](../guides/integration.en.md#bevy-version-requirements) for version requirements. Standalone consumers generate their own lockfiles from the `0.19` range instead of reusing the repository's Bevy selection. When maintaining the dependency snapshot, use `cargo update` to resolve compatible updates, then run the relevant checks.

All-feature and minimal-feature checks serve different purposes. When changing optional BSN wiring in ECS, also test the base ECS path:

```sh
cargo test -p bevy_ui_router_ecs --no-default-features --locked
```

While developing declarations, Commands, or tween integration, start with their focused journeys:

```sh
cargo test -p bevy_ui_router --features bsn --test public_usage --locked
cargo test -p bevy_ui_router_ecs --features bsn,tween transition::tween --locked
```

These focused checks do not replace the full crate and consumer checks required by the table above.

When changing Back adapters, test each source independently and the combined navigation path:

```sh
cargo test -p bevy_ui_router_platform --features desktop-back --locked
cargo test -p bevy_ui_router_platform --features android-back --locked
cargo test -p bevy_ui_router --features bsn,desktop-back,android-back --test platform_back --locked
```

To run just the public feature/dependency checks and standalone consumers:

```sh
cargo test -p bevy_ui_router --test dependency_contract --test facade_consumers --locked
```

The consumer harness separately exercises no defaults, default `view`, explicit `view`, `ecs`, `bsn`, `platform`, `desktop-back`, and `android-back`. Each application declares its minimal dependencies, and both recommended examples compile and run from their unchanged source. Compile-rejection cases check the error category, target symbol, and source location for private API and feature boundaries.

Repository links resolve relative to each document. All six crates share the root README; before publishing to crates.io, also check links on its rendered page rather than relying only on local link validation.

Check documentation and release content with:

```sh
python3 scripts/check-docs.py
python3 scripts/check-release-content.py --output /tmp/router-release-audit.json
```

The release-content script scans current files with rules and writes a redacted report. Add `--history` to include Git history. Supply extra project terms from a local text file with `--term-file`. Review matched items before choosing a remedy, keeping sensitive original text in the local review environment.

Use [Gitleaks](https://github.com/gitleaks/gitleaks) to scan committed history for credentials:

```sh
gitleaks git . --redact --log-opts="--all --full-history" --report-format json --report-path /tmp/router-gitleaks.json
```

## Connect module tests to user behavior

| Topic | Tests | Operations covered |
| --- | --- | --- |
| Core navigation | [Core tests](../../crates/bevy_ui_router_core/tests), [public navigation journeys](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs) | Matching, nested navigation, parameters/query/fragment, history, failure, and recovery |
| View | [Public API](../../crates/bevy_ui_router_view/tests/public_api.rs), [standalone view consumer](../../crates/bevy_ui_router/tests/consumers/view.rs) | Route inputs and renderer-neutral binding |
| ECS lifecycle | [Public API](../../crates/bevy_ui_router_ecs/tests/public_api.rs), [navigation tests](../../crates/bevy_ui_router_ecs/src/navigation_tests.rs), [cleanup tests](../../crates/bevy_ui_router_ecs/src/cleanup_tests.rs) | Host registration, identity checks, preparation, publication, local cleanup, and readmission |
| Recommended setup | [Public usage journeys](../../crates/bevy_ui_router/tests/public_usage.rs), [declaration installation](../../crates/bevy_ui_router_ecs/src/bsn/install/tests.rs), [Commands](../../crates/bevy_ui_router_ecs/src/navigation_commands.rs) | Nested declarations, current inputs with mutable queries, deferred navigation errors, initial priority and host re-registration, independent Routers |
| BSN | [Facade tests](../../crates/bevy_ui_router/tests/bsn_facade_features.rs), [standalone BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs) | Factories, applying Scenes, outlets, and content ownership |
| Platform Back | [Back journeys](../../crates/bevy_ui_router/tests/platform_back.rs), [desktop consumer](../../crates/bevy_ui_router/tests/consumers/desktop_back.rs), [Android consumer](../../crates/bevy_ui_router/tests/consumers/android_back.rs) | Per-window intent, repeat filtering, shared source channel, overlay priority, and targeted Back |
| Input and focus | [Pointer](../../crates/bevy_ui_router/tests/bsn_native_input.rs), [focus](../../crates/bevy_ui_router/tests/bsn_native_focus.rs) | Active hits, departure revocation, started gestures, and other valid targets |
| Retained instances | [Retention journeys](../../crates/bevy_ui_router/tests/retained_navigation.rs) | Independent parent/child retention, retained parent with a new child, eviction, invalidation cleanup, and subsequent navigation |
| Route transitions | [Transition journeys](../../crates/bevy_ui_router/tests/route_transitions.rs), [native overlapping input](../../crates/bevy_ui_router_ecs/src/input/native_ui_candidates/tests.rs) | prepare/current inputs, initial styles, enter-only, dynamic exits, nested retention, Back, queued navigation, host loss/readmission |
| Tween adapter | [Adapter journeys](../../crates/bevy_ui_router_ecs/src/transition/tween/tests.rs) | Zero start after publication, independent clock, exit release after the final write, stopping writers on cancellation, and final redraw |
| Scroll restoration | [Scroll journeys](../../crates/bevy_ui_router/tests/scroll_restoration.rs) | Real layout, early/late containers, ready/cancel, two-dimensional clamping, user scrolling priority, stale IDs, and repeated visits with transitions and cache eviction |
| Page systems | [Activity journeys](../../crates/bevy_ui_router/tests/public_activity.rs) | Mutable application queries, retained-page deactivation/reactivation, dynamic descendants, and exact parent chains |
| Integration boundaries | [Consumer harness](../../crates/bevy_ui_router/tests/facade_consumers.rs), [dependency contract](../../crates/bevy_ui_router/tests/dependency_contract.rs) | Visible APIs for each feature, minimal dependencies, and dependency direction |

Internal tests cover precise write windows and failure conditions. Public journeys connect recommended entry points with normal scheduling. Use both layers to keep local changes aligned with user operations.

## Examples and window checks

Run the two headless examples:

```sh
cargo run --locked -p bevy_ui_router --features bsn --example bsn_quick_start
cargo run --locked -p bevy_ui_router --features bsn --example bsn_nested
```

The quick start checks declaration installation, minimal Scene runtime, and the initial page. The nested example uses real UI layout and combines shared layouts, query updates, application tasks, current route inputs, and explicit scroll restoration. Both exit after a finite set of updates.

The windowed application has its own workspace. Check defaults and all optional features separately, including `ui-debug` and `tween`:

```sh
cargo fmt --manifest-path examples/windowed/Cargo.toml --all --check
cargo fetch --manifest-path examples/windowed/Cargo.toml --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --locked
cargo check --manifest-path examples/windowed/Cargo.toml --all-targets --all-features --locked
cargo run --locked --manifest-path examples/windowed/Cargo.toml
cargo run --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
cargo run --locked --manifest-path examples/windowed/Cargo.toml --features tween --bin tween_transitions
```

The Tween example defines slide and scale/rotation effects through one preparation callback, with exit-hold switching, retained counters, and Back. The manual driver example demonstrates direct lifecycle integration. See the [transition guide](../guides/transitions.en.md) for the contract.

The Linux build uses X11. CI installs `libxkbcommon-dev`, `libx11-dev`, `libxi-dev`, `libxcursor-dev`, and `libxrandr-dev`. Interaction checks cover push/replace/back, layout and page counters, positions for separate visits, delayed content, clicks and Space, focus release, gestures across navigation, error fallback, and recovery. See [window debugging](../guides/debugging.en.md) for steps and BRP commands.

The example's keyboard handler explicitly queries the gate after `RouterInputSet::Ready`. Applications integrating native Tab/Dispatch also need to verify target selection and dispatch. Android manifest setup is in the [system Back guide](../guides/platform-back.en.md#android-traditional-back).

## Performance fixtures and packages

First check benchmark behavior. CI runs the first three commands; instance and steady-state changes can also use the last two:

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=instances cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=steady ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

For timing comparisons, use the optimized builds in the [performance guide](../performance.en.md) and record the same workload on the same machine.

Package checks require a complete checkout, an available registry, and Python 3.12+:

```sh
cargo package --workspace
python3 scripts/check-packaged-consumer.py
```

`cargo package` creates and verifies the six library packages. The [packaged consumer](../../scripts/check-packaged-consumer.py) extracts these `.crate` archives, points Router dependencies to the extracted directories, checks their actual sources, and runs the BSN application included in the package. Bevy resolves independently within the `0.19` compatibility range. The script refreshes extracted source timestamps so a shared target recompiles the current package contents while retaining the registry dependency cache.

Keep the online packaging command above. Cargo `1.98.1` and `1.99.0-nightly (eb98b54bc)` reproduce a temporary-registry `no hash listed` error with `package --workspace --offline` in a two-crate workspace with no external dependencies, even though the index checksum exists and matches the archive. With the same nightly, removing `--offline` passes built-in verification for both that fixture and this project's six packages. See [Cargo #14396](https://github.com/rust-lang/cargo/issues/14396) for a related upstream symptom; the exact internal trigger remains unconfirmed. When strictly offline, generating archives with `--no-verify` and running the packaged consumer provides independent artifact validation, not a passing Cargo built-in verification.

To check uncommitted changes, review the diff, explicitly use `cargo package --workspace --allow-dirty`, and record the source state. Inspect a package's file list with `cargo package -p bevy_ui_router --list`. Review each package's README, source, examples, and required test files, then run the full package checks above.

## Existing evidence and remaining coverage

This table summarizes historical verification recorded in the repository and helps select future checks. Each revision's actual result comes from its command output.

| Environment or capability | Existing record | Next checks |
| --- | --- | --- |
| Bevy `0.19` series | Historical workspace, public-journey, and standalone-consumer regressions cover `0.19.0` / `0.19.1` | Validate the resolved dependencies according to the scope of each update |
| Headless Scene/UI | Combined navigation, layout, picking, focus, retention, activity, and scrolling | Select complete journeys for the changed behavior |
| Package archives | Six library packages and the extracted BSN consumer | Repackage the final release tree and run its consumer |
| Linux/WSLg window | llvmpipe software rendering; BRP input and renderer screenshots cover navigation, retention, scrolling, focus, gestures, fallback, and application transitions | Repeat the debugging steps after interaction changes |
| Android | Minimal `android-back` compilation for `aarch64-linux-android`, plus Bevy message/scheduling tests | APK, device, and system Back checks remain to be run |
| Native Windows, physical GPU, hardware input | Awaiting verification | Record system, device, commands, and interaction results on the target platform |
| Minimum Rust version | Dependency requirements checked | Minimum-toolchain execution remains to be run |

Find older execution logs with `git log -- docs/development/verification.md`. Record each new run's successes, failures, and pending checks. Read remote CI status from the actual workflow run.
