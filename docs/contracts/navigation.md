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
| 同祖先切换 leaf | 保留不变 prefix；suffix 默认 fresh，符合下述 keep-alive 条件时复用原 leaf |
| 某段 params 或复用身份变化 | 从该段起替换，保留此前 prefix |
| raw search 变化 | 按上述精确比较重建整链 |
| 仅 fragment 变化 | 保留祖先，重建最深 normal frame；frame/slot 身份保持，generation 推进 |
| canonical path/search/fragment 全部等价 | normal presentation 保持原结构，history 仍按命令语义提交；显式滚动恢复可写入位置 |

新建或重建 view 得到本次 input。保留 layout 和命中的 cached leaf 不重新调用 factory，其局部内容维持原状态；active snapshot 的 location 更新不表示历史 factory input 被原位更新。

active error presentation 不走 normal 等价导航优化。复用判定包含路由树 provenance、route/slot/explicit key 等身份，不能只比较路径字符串。

实现见 [matcher](../../crates/bevy_ui_router_core/src/matcher.rs)、[branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs) 与 [location](../../crates/bevy_ui_router_core/src/location.rs)。

## 同父 leaf 保留

`RouteNodeOptions::with_keep_alive(true)` 为 route 显式启用保留；`RouterConfig::with_keep_alive_capacity(n)` 限制每个 Router 的非活动实例数，默认 0。active 实例不占容量，未 opt-in 或容量为 0 时沿用 fresh/cleanup 路径。

仅最深 normal leaf 可保留，而且 exact parent frame/generation、outlet、slot binding 与 host incarnation 必须处于仍有效的保留作用域。root 独立页面、带活动子分支的页面、error/fallback、失败或 quarantine 内容不进入缓存。leaf 自己声明的空 child slot 不等于活动子分支。

命中使用该父作用域内的 route/view 复用身份、params、raw search 和 raw fragment。命中保留原 Entity、局部组件状态和内容 handles，跳过 factory、Scene resolve/apply；本次请求仍沿合法的新执行与 publication 合同运行，不重放旧 task。未命中创建 fresh Scene。

缓存按失活时刻维护简单 LRU。命中实例移出非活动库存，原 active leaf 若可保留则成为最新失活项；超出容量时受控清理最旧非活动实例。parent/outlet/slot 或 host scope 失效时不能跨 scope 命中，旧责任须经[生命周期结算](lifecycle.md#非活动实例与长期-owner)。

keep-alive 不改变输入更新规则：raw search 变化仍重建整条分支并失效旧父 scope；仅 fragment 变化仍重建 deepest normal frame，不把此次重建解释为缓存命中。

history entry 标识一次访问，缓存标识页面实例。不同 entry 可以命中同一个 Entity；Push/Replace 的新 entry 与 Back 的既有 entry 语义保持。显式滚动仍按目标 entry 恢复，不按共享实例当前值冒充该次访问的记录。

## 请求、history 与 presentation

同 Router 的命令按内部 sequence 串行处理。push/replace 在入队时确定 requested location；back 在轮到运行时冻结前一个成功 history entry，避免排队期间的旧目标成为执行权限。

- push 成功追加 entry，并截断 forward 分支。
- replace 成功以新身份替换当前 entry；空 history 时建立首项。
- back 成功才移动 cursor；失败保持 cursor 和原 history。

入队时已发布 `can_back=false`，同步返回 `NoBackEntry`，不生成 request。已成功入队但执行时无目标，请求失败为 `BackTargetUnavailable`，不调用 view，不改变 UI/history。

normal success 统一发布 route presentation 与 history。error view/fallback 可以成为可见 presentation，但原导航仍失败，history 不推进。未启用滚动恢复的等价 location 保持零物理写，不取消 push/replace/back 的 history 语义。

presentation/history publication 和 request terminal 是两个时刻。发布后可仍处于 `PostCommitCleanup`；旧责任结算后才进入终态，不能据 `Running` 就推断新页面尚未发布。

publication 后 cleanup 或 host loss 不回滚已经提交的 history，也不改写此次发布确定的原始成功/失败分类。重试通过新的显式命令完成，不自动回放旧 request。

上述事务语义属于生产 Router。独立 Core 的 `MemoryHistory` 是纯数据容器，其 `back()` 立即移动 cursor；它自身不执行 view、publication 或失败回滚。生产 history 权限由 Core 的 [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs) 协议持有。

## 按访问恢复滚动

`RouteScroll::new(key)` 显式标记带有 `Node`、`ScrollPosition` 的容器。key 在一个 route frame 内唯一且稳定；不同 frame 可重复。未标记容器保持应用行为，同 frame 的冲突 key 全部排除并给出有界诊断。

离开 normal 页面时保存最后一次有效布局显示的二维逻辑像素位置，关联 Core canonical history entry 和 route 复用身份。重复 URL 的不同访问独立，Replace 使用新 entry。normal 被 error/fallback 替换时仍保存原位置，失败呈现不覆盖成功访问记录；失败保留原页面时不恢复或重置。

成功 Back 恢复目标 entry，包括 cache hit 与仍存活的 preserved prefix 容器；Push/Replace 保持存活或缓存复用实例的当前值，fresh Scene 使用初值。目标没有记录时保持当前值。位置在 publication 前准备，首个正常目标布局裁剪后，在 `UiSystems::Layout` 之后复验完整归属并单次回写实际位置；内容随后增长不会跳回旧请求值。

容器或 frame 失效时丢弃恢复请求。缺少实际 UI 布局或有效相机时不保存伪造位置，放弃本次恢复并保留原值、给出有界诊断；不额外运行布局或延迟轮询。首版不自动追赶异步增长或延迟创建的容器。

记录随 canonical entry 回收：Replace、forward 截断及 Router 移除释放对应记录；host loss 保留仍在 history 中的访问。接入方式见[集成指南](../guides/integration.md#滚动恢复)。

## Snapshots 与诊断

request snapshot 提供自身 requested/matched/status、trace 和 failure。Router snapshot 分开提供 running request、active presentation、availability、history/can-back 与 cleanup/diagnostics。

queued/last-terminal 不冒充 running；error presentation 的 requested location 不冒充成功 history entry。Back 未运行前的 requested target 可以尚未确定。

公开稳定读取边界为 `Update` 中 `RouterScheduleSet::PublishSnapshots` 之后，可读本轮 snapshots。内部 Observe/Drive/Commit/Cleanup 顺序由 ECS 持有，不成为应用推进 lifecycle 的 API。

诊断定位 Router/request、route（若匹配）、失败来源与阶段，区分恢复失败和已提交后的异常。仅在语义转移时记录，不按 polling 次数制造事件。

每个 Router 保留最近 64 个 terminal 请求，queued/running 不参与淘汰。exact domain/Router 已签发但淘汰的请求为 `RequestLookup::NotRetained`；跨 App/Router、未来或错误配对身份为 `Unknown`。可读取 `earliest_retained_sequence` 判断保留范围。淘汰不改变 authority。

snapshots 与诊断不公开 raw Entity 映射、owner 构造或全量 registry audit。

实现见 [snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs)。已运行验证证明的环境、尚未运行的平台及性能限制以专门文档为准，不从合同文字推导 PASS 或通用时延承诺。
