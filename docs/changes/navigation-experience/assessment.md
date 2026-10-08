# 路由长期演进：职责与机制评估

状态：候选评估，未实施。用户已明确要求以暴露出的通用问题选择长期方向，消费者当前可接受和短期实现成本不能作为架构收口依据。本文不是已确认的 API/spec 或实施计划；现行行为仍以[导航](../../contracts/navigation.md)、[输入与焦点](../../contracts/input-focus.md)和[生命周期](../../contracts/lifecycle.md)合同为准。

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

### 实例身份与当前路由输入

分别定义目标匹配链、页面实例身份、当前已发布输入和 history entry。构建输入继续是一次执行的快照；存活实例另有受控的当前只读输入消费方式，不重放 factory，不回滚任意应用状态。

Core 决定复用、输入更新或替换，View/ECS 落实输入可见性与物理行为。输入和活动资格随合法 publication 生效，pending 数据不冒充当前输入；业务需要保留的元数据快照仍由应用按自身语义持有。

现有 params 从变化 frame 起替换、raw search 整链重建、cached/preserved factory input 不更新的规则保持，直到专题设计明确替代语义。当前没有新 query 请求不构成跳过此模型的理由；也不据此先造 query 解析、归一化或任意复用策略。先设计身份/输入接缝，再裁决具体变化的默认行为。

### 存活实例库存与目标链协调

长期模型以实例及 exact 父 binding 管理库存，matcher 独占目标链。Core 对目标各段选择当前实例复用、库存激活或 fresh，再准备离场保留与清理；复用的父实例不能因目标子页改变而自动换身份。

父 P 允许保留、子 C 不保留时，离场清理 C，保留 P 及可用 outlet；返回 P/D 时复用 P，按正常匹配为 D 选择命中或 fresh。父真正销毁时，其依赖子实例不能跨父复活；子淘汰不应迫使父重建。内部 inactive 库存是否存活取决于父实例、exact scope、自身政策和容量，而非父是否仍在 active chain。

库存可平铺保存原 owner/payload 和父依赖，不必递归打包成不可拆整体。一次操作提前选择、准备并移交原 owners；失败返还原责任。current 仍冻结真实 active，next 显式描述库存激活和 fresh 层，不把缓存伪装成 preserved current。新子页准备期间，缓存父仍 inert。

容量和淘汰必须明确依赖关系：子实例可独立淘汰，父淘汰选择其依赖 closure。计费、LRU 与 owner 粒度由专题设计裁决；不能为简单计数强制所有后代同一政策。清理只写 target，保留父及其它实例保持 protected。

首批仍可限单条匹配链、单 default outlet、同 Router/host 和有效外部 parent/outlet/slot；不扩跨父重挂载、跨 host 或任意 UI 树框架。exact whole-branch hit 可以是内部里程碑或优化，不能作为唯一实例身份。全组 opt-in、子不保留连带父 fresh、内部库存无条件退休退出拟议公共语义。

设计验证聚焦 P/C → Q → P/D、P(true)/C(false)、内部子缓存往返及父淘汰/host loss，重点证明混合激活/fresh 的 owner 返还和局部清理。现有单 leaf 行为须回归；新语义不以增加一套专用整分支状态机实现。

业务会话由应用结束，实例失效和受控清理由 Router 负责，两者分别设计。当前只有 push/replace/back 公共导航，没有实例逐出命令；是否增加接口应由通用失效责任与公开合同决定，不能仅以某消费者当前不用而排除，也不能要求应用私自删除缓存 Entity。

### 公开的通用输入资格接缝

长期目标是在 Bevy/UI 的候选采集/选择及 dispatch 目标生成前消费 route-neutral 资格，继续使用原生排序、TabGroup、modal 和 bubbling。候选不合格不自动剪掉其全部后代；事件起始目标的过滤也不能误拦合法 host 上的冒泡。

Router 提供允许、禁止、范围外的自身资格，通用 UI 结合控件与应用政策。导航与分发共用资格事实，同时明确当前焦点失效、窗口 fallback 和调度；不以事后清 focus 代替分发前判断。

优先围绕公开 hook 设计和实现，不要求消费者采用特定 Bevy fork。官方 0.19.1 缺少过滤入口，循环调用 navigate 的薄 adapter 只能是可替换迁移桥：它反复采集/排序候选，还依赖 doc(hidden) helper 和私有插件 glue 的替代接线，不能作为长期完成状态。

若正式接缝尚不可用且交付需要迁移桥，将兼容实现隔离在一个模块，接口服从同一资格合同，并用真实插件旅程验证等价行为。是否投入这段过渡工作须单独决定；不因较短工期默认先做，也不同时建设两条路线却只计算一条预算。

### 统一的访问级滚动恢复意图

长期模型在成功 Back publication 时按 opt-in scope 冻结目标 entry 原记录和归属，建立恢复意图；容器绑定与内容就绪是两个独立条件。marker 声明容器/key，不能决定访问意图是否存在。

已存在容器和晚出现容器共用一套意图、绑定、就绪、取消和结算机制。意图冻结值与 live sampling 分开；可复制标识仅用于内部单次消费，不新增 lifecycle owner。新 publication、目标失效或应用取消撤销旧意图，导航终态不等待内容。

自动首布局恢复与显式 ready 可作为不同执行策略；ready 后在正常 layout 前准备位置，layout 后单次裁剪。默认行为和应用滚动优先的具体语义在专题合同明确，不猜测稳定、不额外跑 layout、不无限追赶增长。

稳定真实容器本来就适合页面结构时，前置接入是优化；若必须伪造范围、额外留 shell、阻塞导航或自行补恢复，则是在规避已知缺口。首布局范围不足和容器延迟出现都已暴露，应纳入整体设计。实现可以先开放早容器 ready，再开放晚绑定；不能先以 marker flag 为中心，之后补第二套恢复状态机。loader、虚拟范围和条目锚定仍归其它模块。

## 设计顺序、工作量与 ROI

先完成共享实例/输入模型以及保留协调的关键设计轨迹，再按专题形成 design/plan。通用输入公开接缝和访问级滚动意图可独立设计；实现只在依赖明确后并行。每次交付必须是长期模型的一部分，迁移桥与长期完成分别验收。

旧的分支 8–12 人日、主线 17–26 人日对应整组 exact blob 子集，不再作为新方向预算。当前估算如下，未形成实施承诺：

| 工作 | 当前规划依据 |
| --- | --- |
| 实例身份/输入、库存协调、owner/容量/失败分叉 | 先预留 2–4 人日进行模型设计、关键轨迹验证和交叉审查；据选定方案重估实现 |
| 通用输入公开接缝与 Router 桥 | 初估整体工程投入 8–14 人日，不含上游等待；若另外需要迁移桥，明确复用与额外成本 |
| 统一访问级滚动意图 | 初估完整最小能力 6–9 人日；可分早容器 5–7、晚绑定后续 1–2，同一机制 |
| 消费者接入核查 | 0.5–1 人日用于确认事实和验收，不用于回避已经暴露的一般问题 |
| 应用迁移与业务政策 | 由应用单独估算，不计为路由库能力；通用 UI 与 Router 工作分别分账 |

ROI 同时考虑用户收益、重复维护、后续扩展和迁移成本。输入 hidden-helper 桥较快但有维护债务；exact blob 较快但会固化父子耦合；统一意图把必要复杂度前移，避免应用重复恢复。真实缺口外的功能仍需证据，长期正确不等于一次实现所有设想。

Rust 实施按[验证指南](../../development/verification.md)重跑相称完整 crate 和高层旅程。现行架构图继续描述已实现机制，落地后更新，不把候选图当作当前运行证据。性能优化由代表预算和 profile 决定。

## 消费者协调的作用

此前消费者接受官方薄 adapter、exact 全组方案和暂不增加若干 API，只证明当时路径的可接受性，不能作为长期架构已确定的结论。消费者设计仍 reviewing，所有公共 API/spec 均需独立审阅。

后续协调应确认揭示的问题、可观察行为、接入事实和迁移成本。消费者不需要提供所有未来反例，库也不能要求消费者以重写页面或扭曲路由表达迁就内部限制。反馈只归入通用设计，不复制具体页面、数据、会话或 UI 结构。
