# 设计理由与经验

本页提炼本项目设计和修复中可复用的原因，帮助判断下一次改动该落在哪里。具体行为以 [专题合同](../contracts/README.md) 为准，当前实现关系见 [架构](../architecture/README.md)，协作规则归 [AGENTS.md](../../AGENTS.md)。

## 分层是为了让决定只有一个来源

路由匹配、请求顺序、history 和逻辑生命周期相互关联，交给 Core 统一决定，能避免 view 或平台回调各自推进状态。ECS 负责把决定落实为 World 写入；Scene factory 负责生成内容。拆 crate 的价值在责任和可测试边界，而不是文件数量。

View 保持 renderer-neutral，使 Core 的行为可以独立验证，也让未来 renderer 有明确接入点。当前只交付一个生产 BSN adapter，不为可能出现的 renderer 提前建立通用插件框架。

facade 的默认功能不拉入 Bevy runtime，应用显式选择 `ecs` / `bsn`。独立窗口 workspace 解决的是图形 feature 合并污染 headless 验证，而不是要求使用者采用某种应用 shell。

## 嵌套路由的价值在局部保留

用户需要的是切换子页时侧栏、工具栏和局部状态继续存在。common-prefix 保留与 suffix 替换直接服务这一需求；整 host 重建即使更易实现，也会改变用户行为。

“单 default outlet”限制承载数量，不限制它必须位于 Scene 根。内部容器上的 outlet 才能表达普通布局。声明 marker 与实际 child slot 分离，让布局 authoring 不必持有挂载权限。

path、route-local params、raw search 和 fragment 对复用的影响不同。判断复用时应遵守既有语义，而不是以“同一个 route id”或“location 变化”统一重建全部内容。

## 可信应用不需要 World 沙箱

Scene/template/observer 是应用代码，可以触发组件 hook、修改资源或创建自己的实体。Router 的事务只管理授权的路由内容与物理结构，不能承诺回滚任意应用副作用。

曾经冻结全 World 组件或 app-owned host 的无关组件，导致正常 Children observer 和销毁 hook 被误判成 authority drift。修复应围绕真实写入授权、关系和 exact ownership 收窄校验，而不是不断扩大白名单。

应用拥有 host；Router 创建的 shell/slot 与 Scene 普通内容承担不同责任。后者在受管 containment 内由 Router 管理，移出后由应用承担。把所有普通内容临时转换成登记结构，会增加写入、清理和重复证明成本。

## Scene 成功与路由发布是不同事件

同步代码驱动仍会执行 resolve/apply，依赖官方 Scene runtime；Cargo feature 不能证明 `AssetServer` 和 `Assets<ScenePatch>` 已经安装。

apply 可以在 relationship 完整建立前失败。事后扫描 Children 不能找到所有本次创建产物，因此创建期先登记 exact allocation obligations，再在成功 adoption 后移交给内容边界管理。

Scene patch 是构建期组合，不能替代 route reconciliation。局部 observer 适合按钮回调，不能成为 lifecycle 的第二权威。

pending 内容必须先隔离，成功完成整个发布准备后才可获得可见性和输入权。只使用父级 Visibility 曾不足以隔离原生 UI，Router 自控 shell 的 display 与原生输入缓存处置必须结合验证。

## capability 与事实投影不能互相替代

snapshot、projection 和 fingerprint 描述观察到的事实，不代表持有 lifecycle owner。拒绝或准备失败应返还原 owner；根据“看起来相同”的数据重建 owner 会产生重复消费和 stale replay 风险。

延迟 action 要比较完整 source projection。逐个元素分别有效，并不能证明它们仍属于同一次操作。current/next 独立冻结也避免在提交后从结构反推旧状态。

稀疏 topology 表明数量和数组位置不能代表权限：slot-only、空 retained 结构、preserved parent 与 entering child 都要按各自 binding 和 role 判定。

这些规则用于具体可达的生命周期问题；不应把理论计数器极限扩张成全新公开协议或常见上线风险。

## 失败清理不是另一套路由决策

cleanup 回答“哪些产物还需要处置”，continuation 回答“原请求接下来如何结束或恢复”。后者不能由剩余实体数、旧 active 是否保留或物理销毁结果反推。

真实容量或尚未满足的前置事实可以等待；永久身份或关系矛盾不会因为重试更多次消失。永久停止物理写入也不能让前台请求永久 Running，残留隔离责任应与前台终结分开。

不同路径可以复用 preparation 和 owner 移交工具，但 pending rejection、authority loss、已发布 normal 与 error 的来源和结果必须保持区别。减法的目标是删除重复机制，而不是抹平分支语义。

## 组件生命周期让旧快照很快失效

component、relationship 和销毁操作可能同步触发应用代码。目标图与 protected 图在一次操作开始时合法，不代表下一次破坏性写入时仍合法；写后复验是针对这一真实生产行为。

复用校验函数前先比较证明目的。只读 preservation 不应套用 mutation 的封闭 child-set 假设，否则合法的 protected extras 会被误拒绝。

原生输入同样存在历史状态。`Display::None` 阻止新命中，并不会自动撤销已经记录的 press/click/drag；撤权必须覆盖真实缓存和派发时序，并保护其它有效目标。

焦点也不只是一个当前 Entity。pending AutoFocus、应用操作和延后 FocusGained/Lost 有顺序，必须经官方队列验证；无条件 clear 会丢掉应用后来的有效操作。

## 正确性证据必须到达用户入口

私有测试可以精准制造 drift 和 stale owner，但不能证明 public facade 已接通。独立 consumer 使用自己的依赖图，原样执行示例，能够发现仓库 dev feature 合并掩盖的问题。

只验证各项功能不足以证明组合正确。search 引发祖先重建后的 Back 曾因 mount continuation 漏执行而停滞；补充普通 `App::update` 的完整嵌套旅程才揭示问题。

输入负向测试应先证明正向管线确实命中，再验证撤权。否则插件缺失、错误 schedule 或零派发也会让“旧目标没有收到事件”成立。

编译拒绝应核对目标错误来源；源码形状扫描、API 黄金文本和多份 Markdown 矩阵容易只证明彼此同步。它们曾增加维护成本，真实 consumer 和行为回归更接近所需边界。

## 性能减法先减少重复工作

最初的持续退化来自永久增长的 consumed seal 历史查询；把发行与注册合并到唯一入口后，单调身份仍成立，历史列表不再必要。

同一次只读采集重复构造未结算集合、普通内容退出时反复转换成 registry 结构、每个成员重新分类整批义务，都增加了与业务工作无关的成本。共享只读采集与操作内计划减少这些重复，同时保留真正写后失效的复验。

容器微优化并非总有收益。固定小集合替换哈希有实测收益；后续 EntityRef 读取合并没有显示值得保留的收益，候选被放弃。不能仅凭减少调用或分配就宣布更快。

同内容 BSN 对照能帮助区分引擎创建/删除与 Router 责任成本；headless 导航、小窗口和真实产品帧预算回答不同问题。方法及原始数据见 [性能基线](../performance.md)，类似路由实践见 [研究参考](../reference/router-benchmarks.md)。

插桩二进制曾被共享 target 复用，宿主并行构建也曾污染配对结果。测量身份与环境需要确认，受干扰数据不能挑选一半来比较；诊断 probe 不构成正式性能结论。

## 文档按职责维护当前答案

早期设计、阶段合同、固定 API 清单与执行日志同时留在导航中，会把历史方案误读成当前需求，并诱发为已删除机制继续补丁。

文档分别说明产品、架构、专题合同、集成、验证和测量。按功能建立多份设计或计划可以降低阅读成本；同一规则保持清楚归属，避免多处复制。经验记录可复用的因果与取舍，执行过程由 Git 和任务历史追溯；不要复制测试计数、临时路径或其它项目状态。

下一次重构应从可复现问题、真实使用成本和现有责任边界出发。代码规模、过往审查次数或理论能力列表本身，都不足以证明需要新增机制。
