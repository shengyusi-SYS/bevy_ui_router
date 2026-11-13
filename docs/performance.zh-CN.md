# 性能测量

[English](performance.md) | 简体中文

用公开 API 运行基准，测量导航、清理和空闲更新的 CPU 侧耗时。入口为 [router_baseline.rs](../crates/bevy_ui_router/benches/router_baseline.rs)，直接 Scene 构建对照为 [bsn_content_control.rs](../crates/bevy_ui_router/benches/bsn_content_control.rs)。

## 运行与比较

在仓库根目录运行优化构建：

```sh
cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > flat.csv
ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > nested.csv
cargo bench -p bevy_ui_router --bench bsn_content_control --features bsn --locked > control.csv
```

默认每个普通场景采样 128 次，`retained_steady`、`public_queries`、`maintenance` 与 `history` 为 32 次，用正整数 `ROUTER_BENCH_SAMPLES` 调整。`ROUTER_BENCH_SHAPE` 可选 `flat`（默认）或 `nested`；两者内容节点数相同，后者形成四层浅树。

固定机器、工具链、依赖、features 和优化参数，至少重复两轮。串行完成构建与计时，保存全部配对输出。先比较相同工作的绝对 p50/p95、update 次数及波动，再选择需要 profile 的路径。

## 基准模式

| `ROUTER_BENCH_MODE` | 测量内容 | 内容规模 |
| --- | --- | --- |
| 不设置 | 空闲、push/replace/back、失败与恢复、连续导航 | 每页 1/32/128 节点 |
| `keep_alive` | 关闭保留、首次 miss、预热命中、LRU 淘汰，以及各自的空闲更新 | 每页 1/32/128 节点 |
| `instances` | 父子 fresh、原父/新子、父子命中、父依赖淘汰 | 父和子各自 1/32/128 节点 |
| `steady` | 首次导航后的动态内容，比较两层/三层 frame、普通滚动 marker 和无 Router 控制 | 128/1024 节点 |
| `retained_steady` | 固定活动页和共同父布局，积累 0/1/4 个保留兄弟页；空闲与滚动位置更新 | 父布局 128、活动页 32；每个保留页 32/1024 个动态节点 |
| `public_queries` | 常驻应用消费者：空循环、单次入口、共享 reader；重复根与逐实体查询 | 32/1024 个活动后代及同量保留后代 |
| `maintenance` | 清理完成后的空闲更新，以及预热后的缓存 Replace 命中 | 每请求退休 0/32/128 个后代，当前无内容后代；每个保留页 32/1024 个后代 |
| `history` | 固定访问历史的同页 Replace，比较有滚动记录与无 marker 的对照 | 1/128/1024 次访问；父布局 128、活动页 32 个动态后代 |
| `transitions` | 关闭过渡、仅进入、持有退出；分别测发布、逐帧更新、释放、完成与空闲 | 每页 32/128/1024 节点 |

### 动画全路径

```sh
ROUTER_BENCH_MODE=transitions ROUTER_BENCH_UI=1 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > transitions-ui.csv
ROUTER_BENCH_MODE=transitions cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > transitions-router.csv
```

两种运行均使用公开 driver 和完成命令。`ROUTER_BENCH_UI=1` 增加真实 UI layout、UiStack 和 picking 系统，并检查页面几何与相机有效；不创建 renderer。省略该开关可观察 Router 与基础调度。`ROUTER_BENCH_CONTENT=32|128|1024` 可限定内容规模，`ROUTER_BENCH_SHAPE=nested` 可检查共享父链。

fresh 与预热后的缓存命中分别测量。64 次短 Replace 导航预热请求淘汰与生命周期，随后预热空闲更新；每个正式样本固定执行 32 次动画 update 和 32 次结束后 update。关闭过渡的控制也执行相同 update 数量。`holding_update` 和 `idle_update` 是每批 update 的平均耗时，其余列为一次阶段耗时。发布计时在新路由生效时结束，持有退出的清理工作记入 release；只比较发布耗时不能推断导航总成本。

窗口示例提供有限的自动采样模式：

```sh
ROUTER_TRANSITION_PERF=/tmp/transitions-window.csv cargo run --release --locked --manifest-path examples/windowed/Cargo.toml --bin transitions
```

它对空闲、平移/缩放旋转、只进入/持有退出分别预热并采样，结束后一次性写 CSV。主 World 计时覆盖 `First` 到 `Last`；渲染计时位于 `RenderSystems::Render` 中，包含提交与 present 调度/等待，可能包含同 set 的 pipeline queue 处理。update 间隔不代表实际呈现 FPS，这些 CPU 计时也不代表 GPU 执行时间。正式对照关闭 BRP、截屏和其它构建，固定窗口、渲染后端及机器条件。

### Keep-alive 对照

```sh
ROUTER_BENCH_MODE=keep_alive cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > keep-alive.csv
```

关闭保留时每次创建并清理旧页；miss 每次样本前重建父作用域；hit 预热后复用原页面 Entity；LRU 在容量 2、四页循环中创建新页并淘汰旧实例。CSV 记录容量、活实体、活动/非活动页面和 factory 次数。每组退出父作用域后验证页面、内容及旧布局释放，host 保留。

### 父子库存对照

```sh
ROUTER_BENCH_MODE=instances ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > instances.csv
```

预热 64 次导航，使终态请求记录进入稳定容量。父子分别选择保留；依赖淘汰使用容量 2 和三对父子循环。计时外检查 Entity、父布局状态、activity、输入资格、history 和 factory 次数。每组通过 fallback 释放父子内容。

### 初始动态页面对照

```sh
ROUTER_BENCH_MODE=steady ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > steady.csv
```

只完成一次导航，在 publication 后通过公开 World 创建内容。透明 pathless layout 构成三层 frame 对照；普通 `RouteScroll + ScrollPosition` marker 用于观察滚动接线的空闲成本。

### 固定活动内容的保留兄弟页对照

```sh
ROUTER_BENCH_MODE=retained_steady ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > retained-steady-router.csv
ROUTER_BENCH_MODE=retained_steady ROUTER_BENCH_SHAPE=nested ROUTER_BENCH_UI=1 cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > retained-steady-ui.csv
```

共同父布局固定有 128 个普通动态后代，活动页固定有 32 个。每个夹具依次访问兄弟页并返回原活动页，积累 0、1、4 个保留页；每个保留页分别使用 32 或 1024 个动态后代。全部后代在 publication 后通过公开 World 添加。库存增长时，活动页和共同父布局保持原 Entity；flat 和 nested 使用相同节点预算。

每个阶段预热 64 次 update，随后分别测量空闲更新和仅改变活动页 `ScrollPosition` 的更新。每个样本取 32 次 update 的均值，滚动行包含 component 写入。`ROUTER_BENCH_UI=1` 安装正常 layout、UiStack、picking 系统及相机输入，并检查真实滚动溢出和计算后的显示位置；`0` 或不设置时测量无 UI layout 的 Router。夹具不注入 pointer。CSV 记录节点预算、保留页、活实体、请求记录、factory 构建和正式样本的滚动写入数。计时外断言检查 activity/输入资格、Entity 和数量保持不变、采样期间无 factory 重建，以及退出父作用域时完整清理内容。

### 应用查询消费者

```sh
ROUTER_BENCH_MODE=public_queries cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > queries-flat.csv
ROUTER_BENCH_MODE=public_queries ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > queries-nested.csv
```

常驻应用系统在 `PublishSnapshots` 后运行，同时持有可变应用组件 Query。每次 update 重复查询同一活动根，或逐个查询活动／保留页的动态后代。共享 `RouteQueries` 消费者每次系统调用只创建一个 reader；单次入口消费者执行相同的目标选择和应用组件写入。空循环对照包含同样的写入与运行条件。滚动查询测量没有待恢复意图的情况；pending 正向命中由公开滚动 journey 验证。

每个夹具固定一个活动页、一个保留页、两条 history，实体数与 factory 次数保持不变。默认预热 16 次 update，随后取 32 个样本，每个样本平均 4 次 update；`ROUTER_BENCH_BATCH=1` 可逐 update 采样。CSV 的 `frames_per_sample` 记录实际批量。分位数包含正常 Router 调度与应用消费者，表示样本均值，不是独立方法耗时。activity、当前输入与非活动拒绝在计时外验证。本模式无 UI layout 和 pointer 输入。

### 已关闭 trace 与预热缓存命中

```sh
ROUTER_BENCH_MODE=maintenance cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > maintenance-flat.csv
ROUTER_BENCH_MODE=maintenance ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > maintenance-nested.csv
```

`terminal_history_idle` 完成 64 次 Replace，每次退休 0、32 或 128 个动态后代。当前页面没有内容后代，history 只有一条记录，保留最新 64 条终态请求。再预热 64 次空闲更新后，每个样本平均 32 次普通 update。CSV 在计时外统计 trace 事件与诊断数量，区分历史数据体积和当前实体预算。

`retained_hit` 创建两个页面，在 publication 后给每页追加 32 或 1024 个动态后代。先预热 64 次交替 Replace 命中，再测交替导航的完整耗时，保持一个活动页和一个保留页。页面 Entity 与 factory 次数不变，场景组最后退出父作用域并检查内容释放。两组均为 headless，内容 census 查询在计时外执行。

`sample_scope` 区分 `mean_update` 和 `complete_navigation`。导航行的 `updates_per_sample=0` 表示按完整操作采样，实际更新范围见 `min_updates` 和 `max_updates`。trace 数量、当前内容、history 和 factory 检查均在测量样本之外。

### 多次访问的滚动记录

```sh
ROUTER_BENCH_MODE=history cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > history-flat.csv
ROUTER_BENCH_MODE=history ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > history-nested.csv
```

此模式始终安装正常 UI layout、picking 系统和相机输入，不创建 renderer。同一 URL 重复 Push 形成 1、128 或 1024 次访问，复用同一页面和父布局；每次访问通过真实布局采样不同的整数滚动位置。有 marker 与无 marker 两组保持相同的动态内容预算、Entity 和 factory 次数。

预热 64 次 Replace 后，每个样本测量一次完整的同 URL Replace，history 长度固定。计时包含 enqueue、普通 update 和 UI layout、publication，以及请求终态和前台 cleanup 清空；用于观察保存的访问记录增长时的完整导航成本。无 marker 对照包含相同的 canonical history 工作。

计时外检查实际显示位置、history、Entity 不变和无 factory 重建；最多两次 Back 验证有 marker 页面恢复不同旧访问的位置，无 marker 页面保留当前值。单次访问的对照仅在计时后增加第二次访问来完成此检查。CSV 记录 marker 开关、history 和实体数量、update 范围、factory 次数及 Back 检查数；每组最后释放内容。

## 计时边界

Router 夹具安装 TaskPool、Asset 和 Scene runtime，通过普通 `App::update`、导航 API 和快照推进。导航从 enqueue 前计时到请求终态、无 running request 且前台 cleanup 清空，包含构建、resolve/apply、输入授权刷新、publication 和旧内容清理。初始化、目标字符串构造、详细断言与 CSV 输出在计时外。

| 指标 | 读法 |
| --- | --- |
| 导航 p50/p95 | 每次完整操作的 `Instant` wall-clock，采用 nearest-rank 分位数 |
| 空闲 p50/p95 | 每个样本连续 32 次 update 的均值，再对这些均值取分位数 |
| 实体和请求数量 | 场景组完成后读取的稳定状态；failure 行在对应 recover 完成后读取 |
| update 上限 | 每步 32 次更新的防挂起断言 |

普通模式是 headless CPU 侧测量，覆盖 Router 与基础调度；`transitions` 和 `retained_steady` 的 UI 开关，以及 `history` 模式额外覆盖正常布局和输入候选投影，未注入 pointer 时不测实际命中。窗口提交另用上述窗口模式，GPU 执行、实际呈现和 I/O 需在对应环境单独测量。根 workspace 与独立窗口可能使用不同 executor/features，比较前记录实际构建配置。

直接 BSN 对照保留 host/layout/outlet，测 Scene 构造、resolve/apply、旧页删除和普通 update。它用于观察引擎构建成本；Router 基准还包含 history、授权、隔离和恢复工作。

## 行为检查

扩展检查运行基准夹具的行为断言，不设置耗时阈值：

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=retained_steady ROUTER_BENCH_SHAPE=nested ROUTER_BENCH_UI=1 cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=public_queries ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=maintenance ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=history ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

`instances`、`steady` 和 `transitions` 同样支持 `-- --test`。`retained_steady` 的 smoke 保留两种更新工作负载，以及 0/4 个各有 32 个后代的保留页，每组为两个样本、每样本四次 update。`maintenance` smoke 保持 64 条终态记录，退休 0/32 个后代、每个缓存页含 32 个后代，取两个样本，空闲样本各平均两次 update。`history` smoke 使用 1/32 次访问、有无 marker 两组，以及经过真实 UI layout 的两个样本。检查过渡夹具时可加 `ROUTER_BENCH_UI=1 ROUTER_BENCH_CONTENT=32`，以较小内容验证真实布局路径。正式计时使用上面的 `cargo bench` 命令。

## 保存与阅读结果

每次测量保存源码 revision、lock hash、Rust/Bevy 版本、机器与 OS、features、构建 profile、二进制 hash、命令及完整输出。保留干扰和离群值标记，配对比较使用相同内容预算与工作量。先确认实体及记录进入稳定容量，再调查需要 heap profile 的持续增长。

[历史数据索引](data/performance/README.zh-CN.md)保留带完整源码身份的测量，便于检查夹具演进和数据格式。性能结论对应所列源码；评估当前 checkout 时重新运行，避免把历史数值视为当前保证。
