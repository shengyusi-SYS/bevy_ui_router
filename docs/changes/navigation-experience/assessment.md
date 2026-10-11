# 路由长期演进：职责与机制评估

状态：长期方向与职责评估；实例专题已进入实施，当前输入与实例等价迁移已完成，其余方向的交付状态见[路线图](../../roadmap.md)。以已暴露的通用问题选择长期方向，消费者当前可接受和短期实现成本不能作为架构收口依据。本文维护选型理由，不重复专题 spec/plan；现行行为以[导航](../../contracts/navigation.md)、[输入与焦点](../../contracts/input-focus.md)和[生命周期](../../contracts/lifecycle.md)合同为准。

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

本专题的[设计](../instance-coordination/design.md)与[实施计划](../instance-coordination/plan.md)统一维护实例身份、exact 父依赖、当前输入、容量、混合准备和验收；本评估只保留选型理由。

目标匹配链、存活实例、当前已发布输入和 history entry 分别建模。构建输入是一次执行的快照，同一实例需要新的只读输入接缝；只放宽复用比较会留下过期数据。保留现行 params/search/fragment 默认行为，先解决模型和输入的职责分离。

父 P 允许保留、子 C 不允许时，离场保留 P 及可用 outlet、清理 C；返回 P/D 应复用 P 并正常匹配 D。库存按实例持有原 owner/payload 与 exact 父依赖，不递归打包成不可拆整体；缓存父准备新子期间仍 inert。全组 opt-in、子不保留连带父 fresh、内部库存无条件退休已退出候选公共语义。exact whole-branch hit 只能是统一模型下的内部优化。

选择统一实例协调的收益是布局状态可持续、子页可独立变化，既有 publication/cleanup 只需消费逐实例计划；代价集中于混合来源、原 owner 返还与依赖淘汰。继续增加 leaf/整组快捷分支较快，却会重复这部分责任并固化父子耦合。首批单链、default outlet、同 host/exact 父是交付范围，不作为长期身份模型。

业务会话结束归应用，实例失效和受控清理归 Router。公开实例逐出命令另行设计，不因当前消费者不用而排除，也不要求应用私自删除缓存 Entity。

### 公开的通用输入资格接缝

长期目标是在 Bevy/UI 的候选采集/选择及 dispatch 目标生成前消费 route-neutral 资格，继续使用原生排序、TabGroup、modal 和 bubbling。候选不合格不自动剪掉其全部后代；事件起始目标的过滤也不能误拦合法 host 上的冒泡。

Router 提供允许、禁止、范围外的自身资格，通用 UI 结合控件与应用政策。导航与分发共用资格事实，同时明确当前焦点失效、窗口 fallback 和调度；不以事后清 focus 代替分发前判断。

优先围绕公开 hook 设计和实现，不要求消费者采用特定 Bevy fork。官方 0.19.1 缺少过滤入口，循环调用 navigate 的薄 adapter 只能是可替换迁移桥：它反复采集/排序候选，还依赖 doc(hidden) helper 和私有插件 glue 的替代接线，不能作为长期完成状态。

本次核查的官方 [0.20.0](https://github.com/bevyengine/bevy/releases/tag/v0.20.0) 已让 [Tab 跳过隐藏 UI](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_input_focus/src/tab_navigation.rs)，但仍未提供 Router 资格 hook；[dispatch 和焦点队列](https://github.com/bevyengine/bevy/blob/v0.20.0/crates/bevy_input_focus/src/lib.rs)也未补齐所需公开接缝。它超出当前 0.19 支持范围，单纯升级不足以交付本项。

若正式接缝尚不可用且交付需要迁移桥，将兼容实现隔离在一个模块，接口服从同一资格合同，并用真实插件旅程验证等价行为。是否投入这段过渡工作须单独决定；不因较短工期默认先做，也不同时建设两条路线却只计算一条预算。

### 统一的访问级滚动恢复意图

长期模型在成功 Back publication 时按 opt-in scope 冻结目标 entry 原记录和归属，建立恢复意图；容器绑定与内容就绪是两个独立条件。marker 声明容器/key，不能决定访问意图是否存在。

已存在容器和晚出现容器共用一套意图、绑定、就绪、取消和结算机制。意图冻结值与 live sampling 分开；可复制标识仅用于内部单次消费，不新增 lifecycle owner。新 publication、目标失效或应用取消撤销旧意图，导航终态不等待内容。

自动首布局恢复与显式 ready 可作为不同执行策略；ready 后在正常 layout 前准备位置，layout 后单次裁剪。默认行为和应用滚动优先的具体语义在专题合同明确，不猜测稳定、不额外跑 layout、不无限追赶增长。

稳定真实容器本来就适合页面结构时，前置接入是优化；若必须伪造范围、额外留 shell、阻塞导航或自行补恢复，则是在规避已知缺口。首布局范围不足和容器延迟出现都已暴露，应纳入整体设计。实现可以先开放早容器 ready，再开放晚绑定；不能先以 marker flag 为中心，之后补第二套恢复状态机。loader、虚拟范围和条目锚定仍归其它模块。

## 规划与投入依据

实例协调沿已完成的输入与实例迁移继续交付父子独立保留，并退出被替代的 leaf 专用路径。通用输入公开接缝先核实上游可行性，访问级滚动意图独立设计；不把整条路线图捆成一次发布前置。迁移桥与长期完成分别验收。

整体主线、依赖关系、分项风险与预算统一归[路线图](../../roadmap.md#工作量风险与-roi)。实例模型设计不等于实现预算；消费者接入核查计入所属专题，应用迁移与业务政策由应用单独估算。预算不能以重复计算共享设计/验收放大，也不能以省去必要门禁缩小。

ROI 同时考虑用户收益、重复维护、后续扩展和迁移成本。输入 hidden-helper 桥较快但有维护债务；exact blob 较快但会固化父子耦合；统一意图把必要复杂度前移，避免应用重复恢复。真实缺口外的功能仍需证据，长期正确不等于一次实现所有设想。

Rust 实施按[验证指南](../../development/verification.md)重跑相称完整 crate 和高层旅程。现行架构图继续描述已实现机制，落地后更新，不把候选图当作当前运行证据。性能优化由代表预算和 profile 决定。

## 消费者协调的作用

此前消费者接受官方薄 adapter、exact 全组方案和暂不增加若干 API，只证明当时路径的可接受性，不能作为长期架构已确定的结论。消费者设计仍 reviewing，所有公共 API/spec 均需独立审阅。

后续协调应确认揭示的问题、可观察行为、接入事实和迁移成本。消费者不需要提供所有未来反例，库也不能要求消费者以重写页面或扭曲路由表达迁就内部限制。反馈只归入通用设计，不复制具体页面、数据、会话或 UI 结构。
