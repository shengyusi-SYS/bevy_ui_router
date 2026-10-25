# 设计理由与经验

[English](design-lessons.en.md) | 简体中文

这里解释主要设计选择及其原因。当前行为见[行为合同](../contracts/README.md)，执行顺序见[运行机制](../architecture/runtime.md)。

## 决策集中，执行按职责拆分

路由、请求、history 和逻辑生命周期由 Core 统一决定，ECS 负责 World 中的物理操作，View 描述与渲染器无关的输入和视图。adapter 消费这些合同。这样，一次导航的目标、结果和访问记录来自同一决策链。

模块按稳定协议和 owner 拆分，编排层组合阶段与资源借用。只有证明目的、接受的实体角色和失败处理相同时，验证逻辑才适合共用。优先复用 Bevy 的 Scene、布局和输入管线，让 Router 聚焦路由责任。

## 访问、页面实例与物理责任分别建模

history entry 表示一次访问，实例保存 Entity 和局部状态，owner 持有物理执行责任。同一实例可以服务多次访问，所以滚动记录按 entry 保存。构建输入是一次执行的快照，复用实例通过 `RouteInputs` 读取当前输入。

嵌套导航保留公共布局，父和子分别选择 keep-alive。返回 P/D 时，库存决定能否复用 P，当前路由决定子目标 D。父子各自持有 payload 和精确的父依赖，既能保留工具栏状态，也能正常构建新的子页。

活动分支和存活实例库存分别维护。父作用域退出、fallback 和 host loss 会处理相关非活动 owner；原请求结束时结清自己的前台 capability，长期保留责任转交给 retained owner。

## 共用完成路径，保留不同的失败语义

fresh 和 retained 目标都进入同一 publication 管线，携带 history authority、执行身份和后续处理方式。激活、保护非目标和清理分别验证各自所需的事实。例如，缺少 outlet 可能阻止激活，而原 owner 仍可授权安全清理。

cleanup 回答物理义务如何处置，continuation 回答原请求如何结束或恢复。后者保存原失败、attempt 和 branch 分类。部分完成时继续持有原 receipt 与失效来源，区分可完成的 inert shell 和仍有物理责任的隔离残留。可以完成的容量或前置条件不足才进入重试；永久矛盾进入有界终结。

## 线性 owner 与准备阶段

owner 是一次持有、移动或消费的执行凭据。snapshot、projection 和 fingerprint 用来比较事实。current 与 next 分别冻结和校验；延迟 action 消费前重建完整来源投影并要求相等。稀疏 slot、保留的 sibling 和 cleanup target 按各自 role/binding 判断。

Core 接纳责任前，先准备所需容量与 owner 交接。publication 前完成校验、分配、排序、claim rebind、清理预留和输入/focus 准备，最后一步只提交已准备好的状态。审查这一边界时，也检查外层资源借用、take/reinsert、scope 退出和 drop 的副作用。

## 保护内容边界，按真实写入复验

Router 管理自己的结构和授权内容边界，应用拥有 host 与应用资源。创建阶段先登记精确 allocation obligation；成功接管后按当前 containment 管理普通内容。

组件和 relationship 写入可能同步触发 hook/observer。相关写入后重新遍历目标与受保护内容，校验 allowed write set。只读 preservation 允许合法的额外受保护内容；mutation proof 则验证实际删除或改写目标。两种证明分别维护自己的接受条件。

## 活动资格、输入与后台工作

应用通过 `RouteActivity` 决定页面系统是否继续工作。活动查询检查最近登记边界及完整父级挂载链；页面活动、应用 visibility 与输入 gate 分别表达各自用途。

离场撤权处理当前及历史 hover、press/click/drag 状态和 focus，同时保留其它有效目标。焦点候选还保留操作顺序：后来的 set/clear 会覆盖前面的候选，即使最终值相同。

只读状态展示与可变导航编排适合拆成有明确顺序的 system，让 Bevy 的资源借用关系清楚可见。

## 原生输入自动接线的后续方向

未来的 Bevy/UI 扩展入口应在候选加入、焦点准入和输入起始目标生成前消费通用活动资格，继续使用原生排序、TabGroup、modal 和 bubbling。候选拒绝后仍遍历后代；合法起始目标按原生规则冒泡；失效焦点按无焦点语义释放并回退窗口。

Router 桥需要区分允许、禁止和范围外。`RouterInputGate=false` 也可能表示范围外应用控件，因此全局 UI 策略需要单独处理范围外目标。最近登记的非活动或失效边界优先于活动父布局；不同 Router 按目标自身作用域判定。

焦点入口需要覆盖普通系统、World 和 AutoFocus，保持合法操作的同步结果、顺序和 cause。同值 set 与空 clear 也会覆盖旧候选。Scene apply 期间记录真实 task 和暂存内容归属；pending 意图随原 owner 成功发布后兑现，准备先于 publication，合法历史目标的释放事件保留。

这一方向需要公开、可维护且能随官方依赖接入的扩展点。接线完成时，现有依赖私有反射的焦点桥整体退出；当前应用继续使用显式 gate。计划状态归[路线图](../roadmap.md)。

## 滚动恢复与真实内容就绪

访问意图由成功 Back 和原 entry 记录决定，容器绑定与内容 ready 分开。早出现和晚出现的容器共享同一意图；等待期间冻结恢复目标，同时继续更新实际显示的采样见证。离场时复验原绑定、策略和应用值，以保留取消或同一更新内离场的正确来源。

自动首布局和显式 ready 分别服务即时内容与延迟内容。恢复通过正常 Bevy layout 裁剪并单次结算；应用负责加载、虚拟内容范围和主动滚动策略。

## 验证沿用户旅程组织

私有测试定位 owner、drift 和调度窗口；独立 consumer 验证实际 features 与 facade；普通 `App::update` 旅程覆盖完整行为。重点覆盖跨模块交接，例如 fresh/retained Back、父作用域替换和失败恢复。

输入测试先验证正向命中，再建立真实手势并撤权，重新激活后验证新输入可用。故障注入改变生产判定的真实输入事实。打包 consumer 核对依赖来源，并刷新解压源码时间戳，使共享 target 编译本次归档内容。

## 性能从相同工作开始比较

先减少重复采集和分类，在同一只读阶段复用完整事实。相关写入后重新验证。快照按完整可观察值比较，等值时保持资源 change detection 稳定；诊断仍能在请求终结后更新。

空工作在只读入口退出。布局查询与 World 结构查询各自保留所需过滤规则；membership 包含参与判重的禁用或缺组件 marker。调度核查跟随实际 graph build/change 事件，覆盖晚安装和外部重建。

预热覆盖有界记录的淘汰稳态，导航计时覆盖请求终态及前台 cleanup 完成。比较时固定工具链、features、executor 和内容规模，完整保存配对数据。headless 导航、完整 schedule 与窗口呈现分别测量，方法见[性能说明](../performance.md)。

## 文档维护

每条规则有明确归属，跨主题用链接。设计落地后把行为、实现和操作方法分别更新到合同、架构与指南；已完成的计划和实验日志通过 Git 追溯。
