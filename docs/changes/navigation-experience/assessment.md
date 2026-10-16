# 路由长期演进：职责与机制评估

状态：长期方向与职责评估；实例协调与访问级滚动已交付，原生输入正式接线尚未完成，剩余安排见[路线图](../../roadmap.md)。以已暴露的通用问题选择长期方向，消费者当前可接受和短期实现成本不能作为架构收口依据。本文维护选型理由，不重复专题 spec/plan；现行行为以[导航](../../contracts/navigation.md)、[输入与焦点](../../contracts/input-focus.md)和[生命周期](../../contracts/lifecycle.md)合同为准。

## 决策依据与职责

真实消费者路径用于揭示路由问题和建立验收，不能把一个应用的临时适配固化为公共语义。先选择职责清楚、覆盖已暴露一般问题的机制，再决定首批交付；未实现功能可以延后，已知错误的模型不能以“首版简单”为理由固定下来。

保留 Core/View/ECS/BSN 的分工，局部深化已有协议。长期方向不等于预建任意树缓存、完整输入框架或 loader，也不要求新 runtime/crate。现行实现限制和数据结构是假设检查对象，不自动成为产品边界。

| 职责 | 归属 | 长期接缝 |
| --- | --- | --- |
| 匹配、请求、history、实例身份和协调、保留/淘汰决策 | Router Core | 目标匹配链与存活实例库存分别建模，缓存不决定导航目标 |
| 物理实例、publication、cleanup、活动资格与离场撤权 | Router ECS | 持有原 owner/payload，消费 Core 计划；通用物理机制可在真实复用时下沉 UI runtime |
| 候选算法、分组/modal、输入分发、布局和裁剪 | Bevy/UI runtime | 公开、route-neutral 的资格接缝；Router 只桥接自身资格 |
| 当前只读路由输入、entry 滚动记录与恢复意图 | Router，按 Core/View/ECS 职责交接 | 当前输入不等于构建时快照；访问不等于页面实例；意图不依赖容器首次出现 |
| 加载、虚拟范围、业务会话、草稿、结果与后台工作政策 | 应用及相应数据/控件模块 | 提供事实并消费活动资格，不复制路由、实例库存或滚动表 |

## Web 参考策略

以下是对官方文档的适配判断，不要求复制某个框架的全部行为。

- 共享 layout 保留状态，先避免不必要的重建，再选择实例保留。[Next.js Layout](https://nextjs.org/docs/app/api-reference/file-conventions/layout)
- 同一实例可以响应新路由输入；身份判定和输入更新是两件事。本项目不能只放宽复用比较却继续消费过期 factory input。[Vue Router 参数更新](https://router.vuejs.org/guide/essentials/dynamic-matching.html)
- KeepAlive 与路由视图可以组合，通用保活机制和路由保留政策不必混在一个模块。[RouterView](https://router.vuejs.org/guide/advanced/router-view-slot)、[KeepAlive](https://vuejs.org/guide/built-ins/keep-alive.html)
- 滚动按访问选择记录，并允许应用控制执行时机；不因此将加载器并入 Router。[React Router 滚动恢复](https://reactrouter.com/api/components/ScrollRestoration)、[Vue Router 滚动行为](https://router.vuejs.org/guide/advanced/scroll-behavior)
- 导航后的焦点政策由应用决定，modal 和候选算法由通用 UI 层处理；路由负责其内容撤权。[React Router 焦点管理](https://reactrouter.com/how-to/accessibility)、[W3C Modal](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)
- 资源身份、可导航筛选和业务状态各有归属。需要 history 语义的筛选/排序可用 search，不为绕过整链重建硬塞 path。[React Router 状态管理](https://reactrouter.com/explanation/state-management)

React Activity 清理隐藏内容的 Effects 是其 runtime 能力，不等于 Bevy Router 可以自动暂停任意 ECS 系统。[React Activity](https://react.dev/reference/react/Activity)

## 长期机制与交付边界

### 实例协调与当前路由输入

实例身份、exact 父依赖、当前输入与容量归[导航合同](../../contracts/navigation.md#实例保留)，混合准备归[运行机制](../../architecture/runtime.md)；已完成设计与计划由 Git 追溯，本评估只保留选型理由。

目标匹配链、存活实例、当前已发布输入和 history entry 分别建模。构建输入是一次执行的快照，同一实例需要新的只读输入接缝；只放宽复用比较会留下过期数据。保留现行 params/search/fragment 默认行为，先解决模型和输入的职责分离。

父 P 允许保留、子 C 不允许时，离场保留 P 及可用 outlet、清理 C；返回 P/D 应复用 P 并正常匹配 D。库存按实例持有原 owner/payload 与 exact 父依赖，不递归打包成不可拆整体；缓存父准备新子期间仍 inert。全组 opt-in、子不保留连带父 fresh、内部库存无条件退休已退出候选公共语义。exact whole-branch hit 只能是统一模型下的内部优化。

选择统一实例协调的收益是布局状态可持续、子页可独立变化，既有 publication/cleanup 只需消费逐实例计划；代价集中于混合来源、原 owner 返还与依赖淘汰。继续增加 leaf/整组快捷分支较快，却会重复这部分责任并固化父子耦合。首批单链、default outlet、同 host/exact 父是交付范围，不作为长期身份模型。

业务会话结束归应用，实例失效和受控清理归 Router。公开实例逐出命令另行设计，不因当前消费者不用而排除，也不要求应用私自删除缓存 Entity。

### 公开的通用输入资格接缝

长期目标是在 Bevy/UI 的候选加入、AcquireFocus 接受及 dispatch 起始目标生成前消费 route-neutral 资格，继续使用原生排序、TabGroup、modal 和 bubbling。候选拒绝不剪掉其后代；合法起始目标继续原生冒泡，不逐祖先套用 Router gate。失效焦点按原无焦点语义释放并回退窗口。

Router 桥内部区分允许、禁止、范围外；通用 UI 再结合控件与应用政策，不要求公开新的路由身份或资格资源。`RouterInputGate=false` 也包含范围外应用控件，不能直接用作全局拒绝。最近已登记的非活动或失效边界拒绝，不能借活动父布局资格；不同 Router 按目标自身 scope 判定。动态拓扑沿用 Refresh/Ready 合同，原生 dispatch 排在 Ready 后，publication/隔离保持同次撤权。

焦点还需正式的操作准入/观察与官方事件处理接缝。[InputFocus 的 set/clear](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_input_focus/src/lib.rs) 立即修改 `get()` 读取的 current，而[后续事件处理](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_input_focus/src/gained_and_lost.rs)不修改它；只公开队列或在 drain 前过滤，仍不能闭合操作与读取之间的资格。入口须覆盖普通系统、World 和 AutoFocus 发起的操作，现有无 World 上下文的 setter 自身不能查询即时资格。合法操作保持同步兑现及原顺序/cause，同值 set 与空 clear 仍是覆盖旧候选的操作；pending 请求只由 Router 保存意图，不成为有效 focus 或先派发 FocusGained/Lost。

Router 在 Scene apply 期间需要真实 task 的来源事实及暂存内容归属，不能等 apply 完成后反推。原 owner 成功发布且最后意图仍有效时兑现，所需裁决与准备必须先于 publication tail；操作记录不授予或重建 owner。官方事件接缝保留有效操作的完整顺序及曾合法获焦目标的离场释放通知，不能以删除所有非活动目标事件代替这一区分。

上述是可采用接口的最低行为条件，不预定新的上游 API 形状。现有固定版本 [native focus 桥](../../../crates/bevy_ui_router_ecs/src/native_focus.rs)依赖私有字段反射与官方处理器替换；正式接线完成时应整体退休这段兼容责任，不能只在它旁边再加一层 adapter。

可采用接口须有公开合同、明确的依赖来源和维护责任。官方接口、最小上游扩展或独立 UI 扩展按同一行为标准评估；职责归属 UI 层不要求等待官方发布。先用最小可执行验证确认接缝能闭合，再选择一条生产接入路线；独立扩展应复用原生机制，不能把完整候选、modal 或分发算法复制进 Router。

官方 0.19.1 缺少过滤入口，循环调用 navigate 的薄 adapter 只能是可替换迁移桥：它反复采集/排序候选，还依赖 doc(hidden) helper 和私有插件 glue 的替代接线，不能作为长期完成状态。

本次核查的官方 [0.20.0](https://github.com/bevyengine/bevy/releases/tag/v0.20.0) 已让 [Tab 跳过隐藏 UI](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_input_focus/src/tab_navigation.rs)，但仍未提供 Router 资格 hook；[dispatch 和焦点队列](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_input_focus/src/lib.rs)也未补齐所需公开接缝。它超出当前 0.19 支持范围，单纯升级不足以交付本项。

官方 main 在固定版本 `0e93c7ab7d5ec016990dcb226ec544aee1636d70` 新增公开 [Focusable](https://github.com/bevyengine/bevy/blob/0e93c7ab7d5ec016990dcb226ec544aee1636d70/crates/bevy_input_focus/src/lib.rs#L65)，[Tab 候选采集](https://github.com/bevyengine/bevy/blob/0e93c7ab7d5ec016990dcb226ec544aee1636d70/crates/bevy_input_focus/src/tab_navigation.rs#L334)与 AcquireFocus 消费它，缺少组件不剪掉后代。这是未来候选接缝的部分进展；[dispatch](https://github.com/bevyengine/bevy/blob/0e93c7ab7d5ec016990dcb226ec544aee1636d70/crates/bevy_input_focus/src/lib.rs#L414)仍只验证 Entity 存活，[焦点队列](https://github.com/bevyengine/bevy/blob/0e93c7ab7d5ec016990dcb226ec544aee1636d70/crates/bevy_input_focus/src/gained_and_lost.rs#L64)仍无过滤。动态增删 Focusable 尚不能闭合直接设焦后的分发与排队焦点事件，当前不为此迁移整个 workspace。

候选验证可以局部修改 UI 依赖以证明最小接缝，产物和成功范围明确标为验证，不计作正式交付。实际改变支持的 Bevy 版本、依赖来源或消费者装配要求前，给出候选补丁、公开旅程及维护成本，完成依赖选择裁决。若另选迁移桥，兼容责任隔离在一个模块并服从同一资格合同，不同时建设两条生产路线。

可执行候选见[重建入口与采用成本](../../../scripts/native-input-candidate/README.md)。它已接通 World-aware 焦点操作、来源观察和预备发布，复用现有活动证明与原生算法；实际消费核查确认扩展需同时覆盖 `bevy_input_focus` 与 `bevy_ui`，不能只替换一个 crate。原生 Tab/dispatch 与既有焦点、pointer、保留和滚动公开旅程用于检验桥的可行性，完整 ECS crate 检验已有 ownership/失败边界。候选仍不改变当前支持来源，也不等于下游装配或窗口交付。

### 统一的访问级滚动恢复意图

统一意图已实现，绑定、ready、取消与执行规则归[导航合同](../../contracts/navigation.md#按访问恢复滚动)，接入见[集成指南](../../guides/integration.md#滚动恢复)。本评估保留选型理由。

访问意图由成功 Back 和原 entry 记录决定，容器绑定与内容就绪独立。marker 只声明容器/key，不能决定意图是否存在。早、晚容器使用同一机制，冻结值与 live sampling 分开，避免为晚创建再补第二套恢复状态机。

首布局范围不足和延迟创建已暴露，应一起纳入设计。自动首布局与显式 ready 是不同执行策略，加载、范围和应用滚动政策归应用；采用正常 Bevy layout，不把 loader 或虚拟条目锚定并入 Router。

稳定真实容器本来就适合页面结构时，前置接入是优化；伪造范围、额外留 shell 或阻塞导航会让应用承担路由恢复缺口，不作为通用接入要求。

## 规划与投入依据

实例协调和访问级滚动统一意图已交付，被替代的 leaf 专用路径已退出。通用输入先主动验证接缝及可采用路径，接口缺口不能直接成为无限等待的执行安排。验证、迁移桥与长期交付分别报告，不把整条路线图捆成一次发布前置。

整体主线、依赖关系、分项风险与预算统一归[路线图](../../roadmap.md#工作量风险与-roi)。实例模型设计不等于实现预算；消费者接入核查计入所属专题，应用迁移与业务政策由应用单独估算。预算不能以重复计算共享设计/验收放大，也不能以省去必要门禁缩小。

ROI 同时考虑用户收益、重复维护、后续扩展和迁移成本。输入 hidden-helper 桥较快但有维护债务；exact blob 较快但会固化父子耦合；统一意图把必要复杂度前移，避免应用重复恢复。真实缺口外的功能仍需证据，长期正确不等于一次实现所有设想。

Rust 实施按[验证指南](../../development/verification.md)重跑相称完整 crate 和高层旅程。现行架构图继续描述已实现机制，落地后更新，不把候选图当作当前运行证据。性能优化由代表预算和 profile 决定。

## 消费者协调的作用

此前消费者接受官方薄 adapter、exact 全组方案和暂不增加若干 API，只证明当时路径的可接受性，不能作为长期架构已确定的结论。消费者设计仍 reviewing，所有公共 API/spec 均需独立审阅。

后续协调应确认揭示的问题、可观察行为、接入事实和迁移成本。消费者不需要提供所有未来反例，库也不能要求消费者以重写页面或扭曲路由表达迁就内部限制。反馈只归入通用设计，不复制具体页面、数据、会话或 UI 结构。
