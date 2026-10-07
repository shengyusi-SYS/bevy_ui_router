# 导航与观察合同

本页定义路由匹配、输入变化、请求/history 与可观察结果。物理提交与失败结算见[生命周期](lifecycle.md)，view 执行见[Scene](scene.md)。

## 匹配与 location 更新

matcher 选择完整匹配分支，按整条分支累计的 specificity 依次比较：消耗 path 段数更多、静态段更多、参数段更多、index 更多、wildcard 更少、pathless 更少。不是逐段从左到右比较，也不依赖 ECS query 顺序。

最高优先级相同但 leaf 不同的候选返回 `MatchConflict`，不能按先声明者胜出。例如同层 `a/:x` 与 `:x/b` 对 `/a/b` 的分支计数相同，会冲突。声明顺序只决定稳定遍历和诊断顺序。非法定义在构造或安装阶段报错，无完整匹配产生 route failure。

pattern 的静态段精确匹配，`:name` 匹配一个段，末尾 `*name` 捕获剩余段且允许为空；index 仅在没有剩余段时匹配，pathless 不消耗段。pattern 去除前导 `/`，内部空段和非根尾随 `/` 非法。capture 名须非空、只含 ASCII 字母数字或下划线，同 pattern 不得重名。params 按 route-local 作用域传递，不把祖先参数隐式合并到每个 view。

`RouteLocation` 的 path 规范化为唯一前导 `/`、去除空段和非根尾随 `/`；它不解析完整 URL，不从 path 分离 `?`/`#`、不做 URL decoding，也不消解 `.`/`..`。search 与 fragment 分别通过 `with_search` / `with_fragment` 传入并原样保存。

search 按缺省/存在及原始字符串精确比较：`None`、空串、是否带 `?`、键值顺序、重复值顺序和百分号编码的差别均不等价，不做 query 参数归一化。fragment 同样比较原始可选字符串；`#` 前缀不会自动去除。

| 导航变化 | view/lifecycle 行为 |
| --- | --- |
| 同祖先切换 leaf | 保留不变 prefix 的 frame、outlet 和 UI 状态，替换 suffix |
| 某段 params 或复用身份变化 | 从该段起替换，保留此前 prefix |
| raw search 变化 | 按上述精确比较重建整链 |
| 仅 fragment 变化 | 保留祖先，重建最深 normal frame；frame/slot 身份保持，generation 推进 |
| canonical path/search/fragment 全部等价 | normal presentation 可零物理 mutation，history 仍按命令语义提交 |

新建或重建 view 得到本次 input。保留 layout 不重新调用 factory，其局部内容维持原状态；active snapshot 的 location 更新不表示历史 factory input 被原位更新。

active error presentation 不走 normal 等价导航优化。复用判定包含路由树 provenance、route/slot/explicit key 等身份，不能只比较路径字符串。

实现见 [matcher](../../crates/bevy_ui_router_core/src/matcher.rs)、[branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs) 与 [location](../../crates/bevy_ui_router_core/src/location.rs)。

## 请求、history 与 presentation

同 Router 的命令按内部 sequence 串行处理。push/replace 在入队时确定 requested location；back 在轮到运行时冻结前一个成功 history entry，避免排队期间的旧目标成为执行权限。

- push 成功追加 entry，并截断 forward 分支。
- replace 成功替换当前 entry；空 history 时建立首项。
- back 成功才移动 cursor；失败保持 cursor 和原 history。

入队时已发布 `can_back=false`，同步返回 `NoBackEntry`，不生成 request。已成功入队但执行时无目标，请求失败为 `BackTargetUnavailable`，不调用 view，不改变 UI/history。

normal success 统一发布 route presentation 与 history。error view/fallback 可以成为可见 presentation，但原导航仍失败，history 不推进。等价 location 的零物理写不取消 push/replace/back 的 history 语义。

presentation/history publication 和 request terminal 是两个时刻。发布后可仍处于 `PostCommitCleanup`；旧责任结算后才进入终态，不能据 `Running` 就推断新页面尚未发布。

publication 后 cleanup 或 host loss 不回滚已经提交的 history，也不改写此次发布确定的原始成功/失败分类。重试通过新的显式命令完成，不自动回放旧 request。

上述事务语义属于生产 Router。独立 Core 的 `MemoryHistory` 是纯数据容器，其 `back()` 立即移动 cursor；它自身不执行 view、publication 或失败回滚。生产 history 权限由 Core 的 [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs) 协议持有。

## Snapshots 与诊断

request snapshot 提供自身 requested/matched/status、trace 和 failure。Router snapshot 分开提供 running request、active presentation、availability、history/can-back 与 cleanup/diagnostics。

queued/last-terminal 不冒充 running；error presentation 的 requested location 不冒充成功 history entry。Back 未运行前的 requested target 可以尚未确定。

公开稳定读取边界为 `Update` 中 `RouterScheduleSet::PublishSnapshots` 之后，可读本轮 snapshots。内部 Observe/Drive/Commit/Cleanup 顺序由 ECS 持有，不成为应用推进 lifecycle 的 API。

诊断定位 Router/request、route（若匹配）、失败来源与阶段，区分恢复失败和已提交后的异常。仅在语义转移时记录，不按 polling 次数制造事件。

每个 Router 保留最近 64 个 terminal 请求，queued/running 不参与淘汰。exact domain/Router 已签发但淘汰的请求为 `RequestLookup::NotRetained`；跨 App/Router、未来或错误配对身份为 `Unknown`。可读取 `earliest_retained_sequence` 判断保留范围。淘汰不改变 authority。

snapshots 与诊断不公开 raw Entity 映射、owner 构造或全量 registry audit。

实现见 [snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs)。已运行验证证明的环境、尚未运行的平台及性能限制以专门文档为准，不从合同文字推导 PASS 或通用时延承诺。
