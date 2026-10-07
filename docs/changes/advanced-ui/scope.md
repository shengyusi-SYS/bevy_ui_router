# 高级 UI 能力：选型参考

**状态：前期研究参考，不是实施合同。** 当前已选择按 history entry 恢复滚动和同一保留父布局下的 leaf keep-alive，具体行为、边界和验收集中在[页面保留设计](design.md)。当前暂不支持浏览器；异步 factory、过渡动画未纳入本轮。

本页保留选型理由与未选方案的投入比较，不重复已选功能的技术合同。现行实现见[产品范围](../../overview.md)与[架构](../../architecture/README.md)。

## 方案比较

| 路线 | 做什么 | 取舍 |
| --- | --- | --- |
| 主要交给应用 | 页面状态存在应用资源；同步发布 skeleton；应用自写效果 | 适合少量场景，但跨 history 滚动恢复与真正的实例缓存会在消费者重复实现 |
| 按体验逐项扩展 | entry 滚动记录、受限实例 cache 分别交付，并验证组合 | 当前选择；保留同步默认路径，每项可独立使用与验收 |
| 通用页面 runtime | 任意状态快照、整分支 cache、并发 loaders、取消、转场组合 | 初期投入和组合风险最高，没有当前需求支持这一范围 |

滚动位置属于访问记录；缓存保存实体实例。两者不是同一个 key，也不需要通用组件序列化。缓存省去 build/apply，却增加存活实体与长期 cleanup 责任，收益需要测量。

## 未选能力的边界

页面内异步数据可以由同步 Scene 先显示 loading/skeleton，再由应用任务更新自己的内容；Router 导航成功不代表业务数据就绪。只有准备成功后才发布才需要 Router async factory。

当前 production BSN factory、resolve/apply 同步；已有 pending/task identity 不等于 Future 执行器。新增 async factory 需要拥有所有权的输入、job/poll、晚到结果拒绝和等待 owner 的结算；worker 不能携带 World 或 lifecycle 权限。现有 FIFO 意味着等待 A 时 B/Back 排队。取消/latest-wins 会改变请求终态和 history 行为，必须单独选择，不能只增加 abort handle。

入场效果可以由应用更新已发布页的视觉组件；初始值放在 Scene 中，避免首帧跳动。旧页持续可见的退出则需要新的 render-only 非输入角色：当前非 active shell 同时 Hidden/DisplayNone，仅延迟 cleanup 不会让它可见。

串行退出可以保留原 leaving owner 和 PostCommitCleanup，但会延迟后续导航；若要求旧效果未完即继续导航，则需要独立过渡 owner 和中断规则。两种行为不能混淆。keep-alive 与退出动画也不能同时持有同一份 owner。

不顺带建设 loader/action/guard/revalidation、modal、named outlet、prefetch、文件/queued/cached-asset Scene、平台 adapter 或通用插件框架。

## 投入参考

以下为熟悉仓库的工程师的专注工程日，包含相关回归、consumer、窗口及审查，不是模型耗时或承诺工期。已选项的投入与验收以[当前设计](design.md#5-交付顺序投入和验收)为准；下表仅用于未来重新选题。

| 候选 | 工作量 | 主要取舍 |
| --- | --- | --- |
| 页面内 loading/skeleton 示例 | 1–3 日 | 低投入验证是否无需 Router async；应用负责 stale 数据与业务错误 |
| 发布后入场效果示例 | 1–3 日 | 视觉效果由应用选择；不延迟导航成功或输入 |
| FIFO async Scene factory | 12–20 日 | 适用于准备好再切页，但须接受等待期间后续请求排队 |
| 显式取消或 latest-wins | 异步基础上另加 5–10 日 | 改变公共终态/history；中断 worker 不等于撤回 I/O 副作用 |
| 串行旧页退出 | 5–10 日 | 需接受效果期间请求和后续导航等待 |
| 任意 nested suffix cache | 约 3–6 周，需重估 | 跨父作用域 owner/slot 与 protected closure 扩大 |
| 并行快速退出效果 | 10–20+ 日，需重估 | 独立 owner、并发 closure 和中断规则 |

## 历史经验与参考

历史通过 `git show 6de4233:<path>` 查询，前期核查了原 Core 架构、Web 模式、非 MVP 能力研究、早期阶段规划和最终 MVP spec/plan。可复用结论是状态快照与实体 cache 分离、pending 与 active 分离、可见性与输入区别、未来能力需要真实 owner/cleanup。旧文档“必须预留”的清单及旧类型形状不成为新要求。

成熟 Web 实践仅用于拆解需求，不引入浏览器功能或复制 DOM runtime：

- [Vue KeepAlive](https://vuejs.org/guide/built-ins/keep-alive.html)：实例缓存、激活/失活和容量限制；不证明 Bevy 任意系统可以自动暂停。
- [Vue Router scroll](https://router.vuejs.org/guide/advanced/scroll-behavior.html)、[React Router ScrollRestoration](https://reactrouter.com/api/components/ScrollRestoration)：访问身份与恢复时序，不引入 sessionStorage 或默认 anchor 行为。
- [React Router pending UI](https://reactrouter.com/start/framework/pending-ui)、[race conditions](https://reactrouter.com/explanation/race-conditions)：等待反馈和取消是独立行为，不据此改变当前 FIFO。
- [React Router view transitions](https://reactrouter.com/how-to/view-transitions)、[Vue Transition](https://vuejs.org/guide/built-ins/transition.html)：不同进入/退出时序需分别选择；浏览器截图转场不等价于 Bevy 能力。
- [Bevy ScrollPosition](https://docs.rs/bevy/0.19.1/bevy/prelude/struct.ScrollPosition.html)与本地官方 0.19.1 源码：核对请求值、实际布局位置与单位；[异步任务示例](https://docs.rs/crate/bevy/0.19.1/source/examples/async_tasks/async_compute.rs)：后台准备与正常更新消费，不代表 Router 已接通 async。
