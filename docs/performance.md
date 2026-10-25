# 性能测量

[English](performance.en.md) | 简体中文

用公开 API 运行基准，测量导航、清理和空闲更新的 CPU 侧耗时。入口为 [router_baseline.rs](../crates/bevy_ui_router/benches/router_baseline.rs)，直接 Scene 构建对照为 [bsn_content_control.rs](../crates/bevy_ui_router/benches/bsn_content_control.rs)。

## 运行与比较

在仓库根目录运行优化构建：

```sh
cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > flat.csv
ROUTER_BENCH_SHAPE=nested cargo bench -p bevy_ui_router --bench router_baseline --features bsn --locked > nested.csv
cargo bench -p bevy_ui_router --bench bsn_content_control --features bsn --locked > control.csv
```

默认每个普通场景采样 128 次，用正整数 `ROUTER_BENCH_SAMPLES` 调整。`ROUTER_BENCH_SHAPE` 可选 `flat`（默认）或 `nested`；两者内容节点数相同，后者形成四层浅树。

固定机器、工具链、依赖、features 和优化参数，至少重复两轮。串行完成构建与计时，保存全部配对输出。先比较相同工作的绝对 p50/p95、update 次数及波动，再选择需要 profile 的路径。

## 基准模式

| `ROUTER_BENCH_MODE` | 测量内容 | 内容规模 |
| --- | --- | --- |
| 不设置 | 空闲、push/replace/back、失败与恢复、连续导航 | 每页 1/32/128 节点 |
| `keep_alive` | 关闭保留、首次 miss、预热命中、LRU 淘汰，以及各自的空闲更新 | 每页 1/32/128 节点 |
| `instances` | 父子 fresh、原父/新子、父子命中、父依赖淘汰 | 父和子各自 1/32/128 节点 |
| `steady` | 首次导航后的动态内容，比较两层/三层 frame、普通滚动 marker 和无 Router 控制 | 128/1024 节点 |
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

## 计时边界

Router 夹具安装 TaskPool、Asset 和 Scene runtime，通过普通 `App::update`、导航 API 和快照推进。导航从 enqueue 前计时到请求终态、无 running request 且前台 cleanup 清空，包含构建、resolve/apply、输入授权刷新、publication 和旧内容清理。初始化、目标字符串构造、详细断言与 CSV 输出在计时外。

| 指标 | 读法 |
| --- | --- |
| 导航 p50/p95 | 每次完整操作的 `Instant` wall-clock，采用 nearest-rank 分位数 |
| 空闲 p50/p95 | 每个样本连续 32 次 update 的均值，再对这些均值取分位数 |
| 实体和请求数量 | 场景组完成后读取的稳定状态；failure 行在对应 recover 完成后读取 |
| update 上限 | 每步 32 次更新的防挂起断言 |

普通模式是 headless CPU 侧测量，覆盖 Router 与基础调度；`transitions` 的 UI 开关额外覆盖正常布局和输入候选投影，未注入 pointer 时不测实际命中。窗口提交另用上述窗口模式，GPU 执行、实际呈现和 I/O 需在对应环境单独测量。根 workspace 与独立窗口可能使用不同 executor/features，比较前记录实际构建配置。

直接 BSN 对照保留 host/layout/outlet，测 Scene 构造、resolve/apply、旧页删除和普通 update。它用于观察引擎构建成本；Router 基准还包含 history、授权、隔离和恢复工作。

## 行为检查

CI 运行基准夹具的行为断言：

```sh
cargo test -p bevy_ui_router --bench router_baseline --bench bsn_content_control --features bsn --locked -- --test
ROUTER_BENCH_SHAPE=nested cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
ROUTER_BENCH_MODE=keep_alive cargo test -p bevy_ui_router --bench router_baseline --features bsn --locked -- --test
```

`instances`、`steady` 和 `transitions` 同样支持 `-- --test`。检查过渡夹具时可加 `ROUTER_BENCH_UI=1 ROUTER_BENCH_CONTENT=32`，以较小内容验证真实布局路径。正式计时使用上面的 `cargo bench` 命令。

## 保存与阅读结果

每次测量保存源码 revision、lock hash、Rust/Bevy 版本、机器与 OS、features、构建 profile、二进制 hash、命令及完整输出。保留干扰和离群值标记，配对比较使用相同内容预算与工作量。先确认实体及记录进入稳定容量，再调查需要 heap profile 的持续增长。

[历史数据索引](data/performance/README.md)保留带完整源码身份的测量，便于检查夹具演进和数据格式。性能结论对应所列源码；评估当前 checkout 时重新运行，避免把历史数值视为当前保证。
