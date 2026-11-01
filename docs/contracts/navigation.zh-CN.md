# 导航与观察合同

[中文](navigation.zh-CN.md) | [English](navigation.md)

本页说明页面如何匹配、何时复用，以及导航、访问记录和滚动恢复的结果。物理提交与失败结算见[生命周期](lifecycle.zh-CN.md)，view 执行见[Scene](scene.zh-CN.md)。

## 匹配与 location 更新

matcher 选择完整匹配分支，按整条分支累计的 specificity 依次比较：消耗 path 段数更多、静态段更多、参数段更多、index 更多、wildcard 更少、pathless 更少。比较单位是整条分支，遍历顺序稳定。

最高优先级相同但 leaf 不同的候选返回 `MatchConflict`。例如同层 `a/:x` 与 `:x/b` 对 `/a/b` 的分支计数相同，会冲突。声明顺序只决定稳定遍历和诊断顺序。非法定义在构造或安装阶段报错，无完整匹配产生 route failure。

pattern 的静态段精确匹配，`:name` 匹配一个段，末尾 `*name` 捕获剩余段且允许为空；index 仅在没有剩余段时匹配，pathless 不消耗段。pattern 去除前导 `/`，内部空段和非根尾随 `/` 非法。capture 名须非空、只含 ASCII 字母数字或下划线，同 pattern 不得重名。params 按 route-local 作用域传递，各层读取自身参数。

`RouteLocation::new(path)` 只处理 path：规范化为唯一前导 `/`、去除空段和非根尾随 `/`，不分离其中的 `?`/`#`。`with_search` / `with_fragment` 原样保存传入值。

`RouteLocation::from_address` 以及 `From<&str>` / `From<String>` 是内部地址入口：第一个 `#` 分出 fragment，其前第一个 `?` 分出 search，去掉这两个分隔符后沿用原 path 规范化。`/items?` 的 search 是空串，`/items` 的 search 是 None。它不做 URL decoding、不消解 `.`/`..`、不解释外部 URL 的 scheme/authority，也不使用当前页面作为基址；`?tab=info` 指向根路径。结构化 location 不会再次解析。

search 按缺省/存在及原始字符串精确比较：`None`、空串、是否带 `?`、键值顺序、重复值顺序和百分号编码的差别均不等价，不做 query 参数归一化。fragment 同样比较存储的原始可选字符串。

| 导航变化 | view/lifecycle 行为 |
| --- | --- |
| 同祖先切换子页 | 保留不变 prefix；suffix 默认 fresh，符合下述 keep-alive 条件时逐实例复用 |
| 某段 params 或复用身份变化 | 从该段起替换，保留此前 prefix |
| raw search 变化 | 默认从 root 重建；连续 opt-in 的相同身份 prefix 可保留，在首个默认节点或身份变化处开始替换 |
| 仅 fragment 变化 | 保留祖先，重建最深 normal frame；frame/slot 身份保持，generation 推进 |
| canonical path/search/fragment 全部等价 | normal presentation 保持原结构，history 仍按命令语义提交；显式滚动恢复可写入位置 |

新建或重建 view 得到不可变的 `RouteViewInput` factory 快照。保留布局和命中的库存实例不重新调用 factory，其局部内容维持原状态；历史 factory input 不随导航更新。应用通过 `RouteInputs::current(Entity)` 读取所属 active frame 的本次已发布输入。

`BsnRoute::preserve_on_search()`（或底层 `RouteNodeOptions::with_preserve_on_search(true)`）显式允许该节点在 raw search 变化时复用，默认 false。它同时用于当前 prefix 和下述库存命中，不归一化 query，也不忽略 params、route/view/slot/key 或父 scope 身份。各层独立选择；父布局开启不使默认子页自动复用。同时改变 search 和 fragment 时，若整链原可复用，仍保留祖先并替换最深 frame、推进其 generation。需要读取新 query 的保留内容使用 `RouteInputs::current`。

`RouteInputRef` 提供 `route()`、`params()`、`location()`、`origin()`、`failure()` 五个只读 getters；search/fragment 经 `location()` 读取。每层 params 仅属于该 route，完整 location 属于本次 presentation。normal frame 的 failure 为 None；route-error frame 使用错误边界 route/local params 和原 failure，正常前缀不继承 failure；router fallback 的 route 为 None、params 为空、failure 为原失败。error/fallback location 使用失败请求位置，成功 history 保持原值。

`RouteInputs::current` 已包含活动资格判断；读取当前输入无需先调用 `RouteActivity`。每次 publication 同时切换目标各层输入，包括 preserved 和缓存命中的内容。pending/stale/rejected 工作不改旧 active 输入，非活动内容不能读取当前值；发布后即可读取新值，不等请求 terminal。结构 no-op 不为输入更新增加物理写入。Entity 查询范围与借用边界见[页面活动查询](input-focus.zh-CN.md#页面活动查询)。

active error presentation 不走 normal 等价导航优化。复用判定包含路由树来源（provenance）、route/slot/explicit key 等完整身份。

实现见 [matcher](../../crates/bevy_ui_router_core/src/matcher.rs)、[branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs) 与 [location](../../crates/bevy_ui_router_core/src/location.rs)。

## 实例保留

`BsnRoute::keep_alive()` 为 route 显式启用保留，`BsnRouter::keep_alive_capacity(n)` 限制每个 Router 的非活动实例数，默认 0；分别等价于底层 `RouteNodeOptions::with_keep_alive(true)` 和 `RouterConfig::with_keep_alive_capacity(n)`。active 实例不占容量，未 opt-in 或容量为 0 时沿用 fresh/cleanup 路径。

normal 页面和布局可分别保留，exact parent frame/generation、outlet、slot binding 与 host incarnation 必须有效；父实例可以 active 或 retained。直接挂在 host 根部、没有 normal 父 frame 的页面，以及 error/fallback、失败或 quarantine 内容不进入库存。单条目标链、default outlet 和同 Router/host 的范围保持。

例如 `R/P/C → R/Q → R/P/D`：P 开启保留、C 未开启时，离场保存原 P 及其 outlet、清理 C；返回复用原 P，由 matcher 选择 D。P、C 都开启时，C 可在非活动 P 内独立保留；返回 P/D 不会自动恢复旧 C。父实例未保留或被淘汰时，其依赖库存一并进入 child-first 清理。

实例优先选择符合复用政策的当前内容，再选择 exact 父作用域内的库存，最后 fresh；多个合法库存候选选择最近成功离场者。父复用身份不以旧子目标或旧完整 location 决定。

命中使用该父作用域内的 route/view 复用身份、params 和 raw search；该节点显式开启 `preserve_on_search` 时允许 search 不同。最深目标仍要求 raw fragment 精确匹配。命中保留原 Entity、局部组件状态和内容 handles，跳过 factory、Scene resolve/apply；本次请求使用新的执行身份并完成 publication。未命中创建 fresh Scene。

库存按成功离场维护简单 LRU，失败不刷新顺序。命中实例移出非活动库存，各层独立计算保留条件；超出容量时选最旧候选，并受控清理其依赖闭包，所以实际剩余数可以小于容量。parent/outlet/slot 或 host scope 失效后，原实例经[生命周期结算](lifecycle.zh-CN.md#非活动实例与长期-owner)。

同次离场按子到父加入 LRU，所以父子都允许保留而容量为 1 时可留下父、清理子；本次选中的实例及其父依赖不作为容量淘汰目标。

keep-alive 本身不改变输入更新规则：默认 raw search 变化仍重建整条分支，稳定布局需要单独 opt-in `preserve_on_search` 才能保持父 scope；仅 fragment 变化仍重建最深 normal frame。

history entry 标识一次访问，缓存标识页面实例。不同 entry 可以命中同一个 Entity；Push/Replace 的新 entry 与 Back 的既有 entry 语义保持。显式滚动仍按目标 entry 恢复。

## 请求、history 与 presentation

`RouterNavigation::{push,replace,back}` 是即时入队入口，返回真实 request ID 或 `NavigationEnqueueError`。`commands.router(handle)` 提供延迟 Push/Replace/Back，在 Bevy 应用 Commands 时调用同一入口；它可与 `RouteInputs`、应用组件 Query 在同一系统中使用，不持有可变导航资源借用。

Commands 的字符串目标转换为 `RouteLocation::from_address`；传入结构化 location 则保留原值。它不返回同步 request ID，也不保证本轮发布。入队失败默认调用 Bevy warning handler，`.with_error_handler(handler)` 允许应用选择处理器。`NavigationCommandError` 保留 Router、`NavigationKind` 和原 `NavigationEnqueueError`；被拒绝的入队没有 request ID，已入队请求的执行失败仍从 snapshots 观察。

`BsnRouter::initial(location)`（或 `RouterConfig::with_initial_location`）保存一次性默认位置。首次 host Ready 时，通过原 Push 入口提交；此前成功入队的显式 Push/Replace/Back 消费尚未提交的默认意图，失败的显式入队不消费。默认意图尝试入队一次，失败报告 warning；已入队后服从原队列顺序，完成或失败都不自动重试，host 重接不重放。没有配置 initial 时，Ready 可以尚无页面。

同 Router 的命令按内部 sequence 串行处理。push/replace 在入队时确定 requested location；back 在轮到运行时冻结前一个成功 history entry，避免排队期间的旧目标成为执行权限。

- push 成功追加 entry，并截断 forward 分支。
- replace 成功以新身份替换当前 entry；空 history 时建立首项。
- back 成功才移动 cursor；失败保持 cursor 和原 history。

入队时已发布 `can_back=false`，即时入口返回 `NoBackEntry`，不生成 request；Commands 经自己的错误 handler 报告同一结果。已成功入队但执行时无目标，请求失败为 `BackTargetUnavailable`，不调用 view，不改变 UI/history。

normal success 统一发布 route presentation 与 history。error view/fallback 可以成为可见 presentation，但原导航仍失败，history 不推进。未启用滚动恢复的等价 location 保持零物理写，不取消 push/replace/back 的 history 语义。

presentation/history publication 和 request terminal 是两个时刻。发布后可仍处于 `PostCommitCleanup`；旧责任结算后才进入终态。读取 active presentation 可确认当前页面。

publication 后 cleanup 或 host loss 不回滚已经提交的 history，也不改写此次发布确定的原始成功/失败分类。重试通过新的显式命令完成，不自动回放旧 request。

上述事务语义属于生产 Router。独立 Core 的 `MemoryHistory` 是纯数据容器，其 `back()` 立即移动 cursor；它自身不执行 view、publication 或失败回滚。生产 history 权限由 Core 的 [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs) 协议持有。

## 按访问恢复滚动

`RouteScroll::new(key)` 显式标记带有 `Node`、`ScrollPosition` 的容器。key 在一个 route frame 内唯一且稳定；不同 frame 可重复。未标记容器保持应用行为，同 frame 的冲突 key 全部排除并给出有界诊断。

离开 normal 页面时保存最后一次有效布局显示的二维逻辑像素位置，关联 Core canonical history entry 和 route 复用身份。同一 exact frame、Entity 和 key 仍有效而暂缺布局时保留最近见证；去标记、失去归属或替换 Entity 会清除该容器的 live 见证。重复 URL 的不同访问独立，Replace 使用新 entry。normal 被 error/fallback 替换时仍保存原位置，失败呈现不覆盖成功访问记录；失败保留原页面时不恢复或重置。

成功 Back 为目标 entry 的已保存 frame/key 冻结恢复意图，包括 cache hit 与仍存活的 preserved prefix；容器可以已经存在或随后创建。Push/Replace 保持存活或缓存复用实例的当前值，fresh Scene 使用初值。目标没有记录时保持当前值。导航终态不等待内容就绪。

意图的 opaque ID 在每次 publication 独立创建；同 entry、frame 或 Entity 再次 Back 也不复用。`RouteScrollRestorations::pending(scope_entity, key)` 从该 exact active frame 的任意内容查询冻结位置及 ID，容器尚不存在也可查询。意图只绑定一次唯一容器，记录其原 Entity、frame、key、策略与位置；已绑定容器失效、key/策略改变或被替换时消费意图，不跟随替代 Entity。

`RouteScroll::new(key)` 默认 `Automatic`：绑定后的首个正常布局尝试一次。`with_restore_policy(RouteScrollRestorePolicy::Explicit)` 等待应用经 `Commands` 的 `ready_route_scroll(id)` 允许尝试，ready 可先于晚容器出现。新 publication 撤销旧意图；仅入队、未发布准备或保留当前内容的失败不取消它。旧或已消费 ID 无效果。

等待时冻结目标与 entry 原记录保持，实际显示见证继续更新；采样和离场保存不能用初值覆盖仍有效意图的原记录。应用改变已绑定 `ScrollPosition` 则取消恢复，后续按实际显示采样；同 Update 立即离场也先复验该意图。应用在执行后改写位置，布局后不再用恢复结果覆盖。等值赋值与绑定前的应用意图不能由数值推断，可用 `cancel_route_scroll(id)` 明确取消；取消不回滚已经写入或应用自己的值。

请求值统一在 `PostUpdate` 的 `RouterScrollSet::Prepare` 写入，该 set 在 `UiSystems::Content` 后、`UiSystems::Layout` 前；publication 后、Prepare 前读取组件可能仍为原值。正常布局后复验当前完整 branch 归属，单次回写实际裁剪位置并消费意图，内容随后增长不追赶旧目标。缺少实际布局或有效相机时不保存伪造位置，本次尝试放弃并保留绑定时原值，给出有界诊断；不额外运行布局或按固定帧重试。

记录随 canonical entry 回收：Replace、forward 截断及 Router 移除释放对应记录；host loss 撤销意图，保留仍在 history 中的访问记录。接入方式见[滚动指南](../guides/pages.zh-CN.md#滚动恢复)。

## Snapshots 与诊断

request snapshot 提供自身 requested/matched/status、trace 和 failure。Router snapshot 分开提供 running request、active presentation、availability、history/can-back 与 cleanup/diagnostics。

queued、running 和 last-terminal 分别观察；error presentation 的 requested location 与成功 history entry 分别保存。Back 未运行前的 requested target 可以尚未确定。

公开稳定读取边界为 `Update` 中 `RouterScheduleSet::PublishSnapshots` 之后，可读本轮 snapshots。内部 Observe/Drive/Commit/Cleanup 顺序由 ECS 持有，应用通过公开命令发起导航。

Router 发布 `RouterSnapshots` 时仅在可观察值变化后标记资源 changed；应用可用 Bevy change detection 跳过无变化快照的投影。该信号覆盖所有 Router 与请求，不代表动态内容归属、activity 或滚动变化；需要当前归属时仍使用 `RouteInputs` / `RouteActivity`。

诊断定位 Router/request、route（若匹配）、失败来源与阶段，区分恢复失败和已提交后的异常。仅在语义转移时记录，不按 polling 次数制造事件。

每个 Router 保留最近 64 个 terminal 请求，queued/running 不参与淘汰。exact domain/Router 已签发但淘汰的请求为 `RequestLookup::NotRetained`；跨 App/Router、未来或错误配对身份为 `Unknown`。可读取 `earliest_retained_sequence` 判断保留范围。淘汰不改变 authority。

实现见 [snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs)，检查命令见[验证指南](../development/verification.zh-CN.md)。
