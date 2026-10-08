# 导航体验扩展：职责与选型评估

状态：候选评估，未实施。用于后续设计与消费者协调，不是已确认的 API/spec 或实施计划。现行行为仍以[导航](../../contracts/navigation.md)、[输入与焦点](../../contracts/input-focus.md)和[生命周期](../../contracts/lifecycle.md)合同为准；执行顺序归[路线图](../../roadmap.md)。

## 目标与职责

目标是连续的嵌套导航、必要的页面状态与访问位置恢复，以及非活动内容撤权。消费者路径提供需求证据，不决定库的内部协议；具体业务、页面、会话和结果处理不进入本项目。

采用路由权威、通用 UI 能力与应用政策分层协作。Router 全包输入与列表会持续扩张职责；只留下 path/history 则会把访问恢复、实例生命周期与撤权协调推回应用。当前 Core/View/ECS/BSN 分工保留，按实际责任深化已有模块，不先建新 runtime 或拆新 crate。

| 职责 | 归属 | 接缝 |
| --- | --- | --- |
| 匹配、请求、history、实例身份、保留范围与淘汰决策 | Router Core | 唯一路由决策来源；缓存不选择新的导航目标 |
| 物理保留、publication、cleanup、路由活动资格及离场撤权 | Router ECS | 沿现有 owner 与执行管线完成；应用不承担清理协议 |
| 通用实例保活、输入候选算法、分组/modal、布局和裁剪 | Bevy/UI runtime | Router 提供保留政策和资格；现有 ECS 可承接物理桥，不为形式独立预建框架 |
| entry 滚动记录、恢复意图与失效 | Router ECS，消费 Core entry 事实 | 应用标记容器并报告 ready/cancel；UI 提供实际布局 |
| 内容加载、虚拟范围、业务会话、草稿、结果与后台工作政策 | 应用及相应数据/控件模块 | 应用提供事实并消费 RouteActivity；不复制路由或滚动记录表 |

## Web 参考策略

以下是对官方文档的适配判断，不要求 Bevy 复制某个 Web 框架的全部行为。

- 先保持公共布局，再选择保活。Next.js 的共享 layout 保留状态；本项目继续 common-prefix 复用，不能用缓存补偿不必要的重建。[Next.js Layout](https://nextjs.org/docs/app/api-reference/file-conventions/layout)
- 分离实例身份与当前输入。Vue Router 可复用组件并响应 params 更新；本项目现有 factory input 不会原位更新，未来复用扩展须同时设计输入消费，不能仅放宽比较。[Vue Router 参数更新](https://router.vuejs.org/guide/essentials/dynamic-matching.html)
- 保活机制可以组合。Vue 在 RouterView 渲染的组件外组合 KeepAlive，并提供容量和激活/失活行为。路由保留政策属于 Router，通用树保活属于 UI runtime；当前无需抽出第二套框架。[RouterView](https://router.vuejs.org/guide/advanced/router-view-slot)、[KeepAlive](https://vuejs.org/guide/built-ins/keep-alive.html)
- 滚动按访问恢复，并提供执行时机原语。React Router 默认使用 location key，Vue Router 允许异步提供位置；本项目采用 entry 记录与显式就绪，不因此引入 loader。[React Router 滚动恢复](https://reactrouter.com/api/components/ScrollRestoration)、[Vue Router 滚动行为](https://router.vuejs.org/guide/advanced/scroll-behavior)
- 焦点政策与路由撤权分开。React Router 将导航后的焦点选择交给应用，modal 约束由通用 UI 模式定义；Router 只保障其离场内容撤权。[React Router 焦点管理](https://reactrouter.com/how-to/accessibility)、[W3C Modal](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)
- URL、实例状态与业务状态不混为一体。需要导航语义的筛选或排序可以放 search，不为绕过当前重建限制强塞 path；业务草稿不以缓存 Entity 作为唯一持有者。[React Router 状态管理](https://reactrouter.com/explanation/state-management)

React Activity 隐藏时清理 Effects，是其 runtime 提供的能力。本项目的 RouteActivity 只报告资格，不承诺自动暂停任意 ECS 系统。[React Activity](https://react.dev/reference/react/Activity)

## 候选工作包

### 原生输入接线与资格桥

已有公开 API 探针确认未经适配的原生 Tab 候选包含失活缓存页，但没有闭合完整插件输入旅程。锁定的官方 Bevy 0.19.1 缺少 Tab handler 的资格过滤入口；必须先确定安装和调度，不能仅补 gate 查询。

短期候选是小型 opt-in adapter：保留官方 InputDispatchPlugin，在资格准备后分发；替代 stock Tab 接线，循环调用官方 navigate 筛选最终目标。保留必要点击获焦与错误恢复，候选和分发共用允许、禁止、Router 范围外的分类。不改 authored TabIndex，不依赖 observer 偶然顺序，不安装两套 Tab handler。

TabNavigation 虽可调用但带 doc(hidden)，需隔离这处版本依赖。先用真实 KeyboardInput、官方插件和普通 App::update 建立正向收键、失活拒绝、合法范围外目标、modal 和撤焦旅程，再验证 adapter 与既有 pending focus 队列的组合。

另一条路线是在 Bevy/UI 内增加 route-neutral predicate hook，由 Router 桥接资格。该路线需要通用层的设计与维护，不让 Router 强制依赖消费者 fork。两条路线择一；当前预算不包含同时建设或等待上游合入。

### 按需要增加显式就绪滚动

先核查 publication 前真实容器和正确滚动总范围是否可用。若足够，直接消费现有恢复；若局部调整稳定 shell 的成本低于扩展，可选择局部接入调整，但不要求应用全面重写 builder。

确需等待内容时，publication 冻结目标 entry 的原位置与目标归属，应用/列表报告 ready 或 cancel。恢复标识只是可复制、内部单次消费的意图标识，不拥有 lifecycle 或 history publication 权限。不能 ready 时才读取 live records，因为等待期间正常采样可能已覆盖原值。

ready 后在正常 layout 前准备位置，layout 后复用单次裁剪结算；后发应用滚动优先，新 publication 或目标失效撤销旧意图。导航和请求终态不等待内容；不猜测高度稳定、不额外跑 layout、不无限轮询。

晚创建容器只有在代表路径确实需要时加入：publication 前声明恢复 key，之后绑定 exact 容器。仅添加 marker ready flag 不足以补足这项能力。虚拟范围、条目锚定和数据完整判断归应用/控件模块。

### 有限嵌套分支保留

首版候选仅保留同一有效共同父 frame/outlet/slot/host 作用域内、完全匹配的多 frame suffix。组内当前页面全部明确 opt-in，否则整个离场 suffix fresh；返回另一子分支也 fresh。现有单 leaf 继续原规则。

容量按非活动 frame 计费，整组 LRU；组内旧 inactive leaf 库存单独退休，不递归聚合进保留组。沿原 owner/ledger 机制清理库存时，保留组属于 protected，不能重放包含该组的整份父退出删除计划。

Core projection/hit、ECS splice/claim rebind/activation 和 cleanup 都有单 frame 假设，需局部扩展既有完成管线，不能只删除 deepest 判断。代表验证覆盖多层往返、leaf/branch 混用、淘汰后 fresh、父/host 失效和整组输入撤权；部分命中、排除子页后局部重建、递归子缓存、跨 host 缓存不进入首版。

不保留的页面正常离场后由库清理，业务会话由应用结束；fresh 不等于重建已结束的业务会话。目前公开导航是 push/replace/back，没有公开的显式实例失效命令。只有真实路径仍需要逐出已缓存实例时，再设计最小公开合同。

### 参数与输入更新

资源身份先使用已有 path params。需要导航/history 语义的 query 不应为规避重建而改成 path；出现真实需求时，优先评估实例身份与当前只读输入分离，再决定局部更新或重建范围。当前 raw search 整链重建及 cached/preserved factory input 不更新的合同保持，暂不增加通用 query 复用策略。

## 顺序、工作量与 ROI

下表为熟悉仓库的开发者所需人日初估，含相称审查与验证，不是已排期承诺。按责任分摊，联合验收不在多个预算重复计算。

| 工作包 | Router | 通用 UI / 应用 / 联合工作 | 合计及 ROI |
| --- | --- | --- | --- |
| 原生输入薄 adapter | 资格桥与调度 1–2 | 通用接线 1–2；联合插件/consumer/文档 2–3 | 4–7；高，补真实接入缺口 |
| 现有滚动接入核查 | — | 代表容器/范围核查 0.5–1 | 0.5–1；高，可能省去扩展 |
| 必要的显式就绪恢复 | 冻结意图及结算 3–4 | ready/cancel 接入与消费旅程 1–2 | 4–6；真实等待需求下高 |
| 最小分支保留 | 设计、Core/ECS、回归与文档 8–12 | 应用消费另计 | 8–12；多层往返频繁时较高 |
| 晚容器支持 | 必要时再细分 | 沿用同一恢复机制 | 增量 2–3；条件性 |

若显式就绪确有必要，主线跨层合计约 17–26 人日，不能全部记作路由库开发。应用页面迁移、参数映射及业务闭环由消费者计划单独估算；通用上游 hook 不计入薄 adapter 预算。

先并行做原生输入复现和滚动接入核查，再分别收口最小合同。按必要性推进显式就绪，之后独立交付最小分支保留；晚容器单独决定。消费者中不依赖新能力的工作可独立推进。

每个实施专题分别形成 design/plan，不建一份覆盖所有领域的执行状态机。Rust 变更沿[验证指南](../../development/verification.md)选择检查；现行架构图继续描述已实现机制，落地后再更新。性能优化由代表页面预算决定，不以增加缓存代替 profile。

## 消费者协调项

协调只确认行为需要和接入代价，不让消费者 spec 决定内部 owner/API，也不自动授权实施。

1. 哪些代表场景已有稳定容器与正确范围，哪些确需内容就绪或晚容器；接入改动是否局部可承受。
2. 官方薄输入 adapter 的明确安装方案能否满足现有 UI，哪些通用 hook 确实需要另做；一次性页面能否用不保留和正常离场表达退出。
3. exact branch 全组 opt-in/fresh 的边界是否满足往返体验；是否存在不能由现有参数消费表达的真实 query 更新需求。

消费者反馈回到上述通用选型后再更新评估，不复制其页面、数据、会话或 UI 结构。
