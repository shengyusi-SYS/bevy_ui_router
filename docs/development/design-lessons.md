# 设计理由与经验

本页只提炼 Bevy UI Router 可复用的因果与取舍。现行行为归[专题合同](../contracts/README.md)，实现关系归[架构](../architecture/README.md)，协作护栏归 [AGENTS.md](../../AGENTS.md)。经验帮助选择调查方向，不能代替当前代码和运行证据。

## 决策集中，执行按职责拆分

路由、请求、history 与逻辑 lifecycle 相互关联，由 Core 统一决定；ECS 落实 World 写入，View 保持 renderer-neutral，adapter 使用受控合同构建内容。单一决策来源避免平台回调、Scene observer 或缓存各自推进路由。

模块按稳定协议及其 owner 拆分，编排层只组合阶段与资源借用。拆 crate、提取文件或引入依赖，应该减少理解和修改成本；行数、理论扩展能力和审查次数本身不能证明需要重写架构。

优先使用 Bevy 的 Scene、布局与输入管线，以及标准容器。简单 LRU 不需要新 runtime；第三方依赖只有覆盖真实责任、减少自维护成本时才值得引入。共享工具也须有相同的证明目的、角色分区与失败语义，不能只因代码相似合并。

## 访问、页面实例与物理责任分别建模

history entry 标识一次访问，页面实例保存 Entity 与局部状态，owner 承担物理责任；关系见[架构图](../architecture/README.md#访问实例与责任)。同一页面可服务多次访问，滚动位置必须按 entry 保存。采样缺失与身份失效分开：缺布局保留仍有效的 exact 容器见证，离场再净化记录，避免同 key 新实体继承旧值。长期 retained owner 可以超出原请求寿命，原请求仍应结清自己的前台 capability。

等待恢复时只保护冻结目标与 entry 原记录，实际显示见证继续更新；否则取消或同 Update 离场会丢失来源。publication 可能早于布局前执行，离场也必须复验原绑定、策略和应用值。使用正常布局与一次结算，不用帧数或范围增长猜测 ready。

嵌套路由保留 common prefix，服务的是布局与局部状态连续性；整 host 重建会改变这一用户行为。path、route-local params、search 和 fragment 的复用语义不同，不能仅按 route id 或 location 变化统一重建。

active branch 因而不再等于完整生命周期库存。父 scope 退出、fallback 和 host loss 都要检查原 retained owner，而不是只递归删除当前 active；数量和树位置也不能代替 exact binding。

缓存复用的是已实现页面，fresh Scene 仍是一次执行的构建输入。已有页面不应通过克隆 Scene 或重放旧 task 恢复；依赖保活、前台 capability 和残留隔离责任各有自己的结束边界。

目标链与本次 fresh 创建义务独立：复用父实例不决定子目标，库存前缀不能进入 fresh apply/失败根集合。父子各持原 payload，以 exact 父依赖协调；整组 opt-in 或全局 Fresh/Retained 分类会让正常提交与失败续接反复分叉。

## 共用完成路径，保留不同的失败语义

Back 的成功目标既可能 fresh，也可能 retained。共享接收器若仍假设所有成功结果都 fresh，就会遗漏合法目标；应让它们进入既有 publication 管线，携带原 history authority、执行身份与 continuation，而不是补一套缓存 Back 状态机。

重新激活、非目标保护与安全处置回答不同问题。缺少原 outlet 声明可能阻止激活，不自动撤销受保护身份或阻止按原 owner 安全处置；各路径分别证明当前 authority、角色、目标写集与原义务，不能把任意 drift 都降成 miss。

cleanup 决定物理义务如何处置，continuation 决定原请求如何结束或恢复。后者来自原失败与 attempt，不能由保留了旧 active、剩余实体数或删除结果反推。停止删除不等于责任已结清：部分完成路径须保存原 receipt 与失效来源，重新证明剩余原义务。可以完成原 token 的 inert shell 结算与仅终结前台、保有未完成物理责任的 authority residual 是不同结果，不能伪造 completion。仅真实容量或可完成前置不足可以重试；永久矛盾必须有界结算。

物理准备的拒绝必须进入有明确 owner 的 classification/continuation；不能在独立 current/next preflight 之前以空准备结果无限等待。共用同一生产 predicate 时可以删除重复前置校验，保留发布前完整检查及必要写后复验。

## 线性 owner 与 preparation 边界不能靠快照补救

snapshot、projection 和 fingerprint 提供事实校验，不能重建 capability。会签发 owner 的接口也不是只读查询；所有 typed outcome 都必须完整消费或返还，否则一次空闲观察也可能遗失唯一 owner。

current/next 独立冻结，延迟 action 核对完整 source projection。当前挂载证据与清理后的下一轮记录也分别准备；全部选定删除义务都须反映到下一轮 prior。逐元素有效不能证明仍属于同一次操作；稀疏 slot 和 retained sibling 必须按自身 role/binding 判断。

Core 接纳线性责任前，先完成该协议要求的容量预留和 owner 交接准备。缓存自身失效再继续 fresh 的路径，还需提前备妥同次 removal/receipt 包，避免接纳后卡在物理准备；普通 cleanup 的实际 receipt 则随其 attempt 准备。publication 前完成所有可失败工作，tail 只提交预备状态，不能返回后补装 owner。

Bevy 的便利借用 API 也要检查实际副作用。资源 take/reinsert、scope 退出和 drop 可能改变 tick 或触发 hook；审查原子边界时要包含外层包装，不能只检查函数正文。需要只读 preparation 时使用适合该边界的联合借用。

## 保护内容边界，按真实写入复验

可信 Scene/template/observer 可以修改应用资源。Router 只管理自有结构与授权内容边界，不承诺回滚任意应用副作用。cleanup 的相关保护从实际清理单元和授权挂载范围展开，不能上溯整个应用树、冻结无关 UI。原 host 的注册观察单独保护：它证明操作期间事实保持，不要求已经进入 authority-loss 结算的来源重新有效；原 authority、owner 与 continuation 仍由各自协议判定。

创建期先登记 exact allocation obligations，覆盖尚未挂接的产物；成功 adoption 后按当前 containment 管理普通内容。普通组件新增不是 ownership 转移，普通内容也不必临时转成永久登记结构。

relationship 与组件写入可能同步触发 hook/observer，正常 UI propagation 也会改变派生组件。每次相关写后重建 target/protected partition，再验证授权写集和保留值；不以全局白名单、旧快照或遍历顺序掩盖角色差异。只读 preservation 不应套用 mutation 的封闭 child-set 假设。视觉隔离与 lifecycle 写入也按冻结的实体 role 分开，避免把 native 内容根当作 Router shell 改写应用状态。

## 活动资格、输入权限与后台工作各有边界

隐藏 retained 页面不等于暂停应用 timer、Query 或 observer。应用用最小的 `RouteActivity` 查询决定自己的工作；查询须核对最近登记边界和完整父级挂载链，不能让 retained child 继承 active parent 的资格。activity 与 authored visibility、输入 gate 分开。

接入真实 Bevy system 时检查资源借用。只读活动/状态展示与可变导航编排混在一个 system 会造成冲突；按职责拆成有明确顺序的 system，通常比用复杂借用包装更清晰。

不可见也不等于历史手势失效。撤权必须经过原生 picking、当前/前次 hover 和 press/click/drag 状态，并保护其它有效目标。focus 的延后候选与后发 set/clear 还包含操作顺序，最终值相等不能证明中间没有被覆盖。

## 验证沿真实用户旅程闭合

私有测试定位精确 owner、drift 和调度边界；独立 consumer 使用自己的 feature 图证明 facade 装配；普通 `App::update` 的组合旅程证明实际收敛。各项单独通过，仍可能漏掉 fresh/hit Back、父 scope 替换和恢复的交接问题；不要用额外内部调用跳过真实刷新边界。

输入测试先证明正向命中，再建立真实按下/拖拽并验证撤权，重新激活后还要证明新的输入可用。负向无事件本身可能只是插件或调度未接通。

fault seam 只改变生产 predicate 的真实输入。编译拒绝核对目标诊断来源；删除旧机制时先迁移必要行为，不保留只证明源码形状、固定清单或多份 Markdown 彼此同步的验证。反例按已确认范围、可达性和影响选择。

打包产物的来源校验与实际重新编译是两件事。归档时间戳规范化后，共享 target 可能复用同版本旧产物；解压消费时刷新源码时间戳，让 Cargo 编译本次字节内容，同时保留 registry 依赖缓存。

设计审查、headless/crate、独立 consumer、窗口及平台原生验证分别报告。工具可用、命令成功和产品行为通过是不同事实，未执行项不能由其它层级推断。

## 性能先对齐工作与证据

性能减法先消除重复采集、反复登记和重复分类；操作内共享计划不能省去真实写后失效的复验。容器或读取方式是否值得优化，以配对测量决定。

命中省去构建，首次 miss 保留旧页，关闭缓存则清理旧页，满容量淘汰又包含回收；这些工作不能仅凭耗时差归因到 Scene 优化。计时应覆盖请求终态、无 running 和前台 cleanup 清空，同时观察驻留内容有界及退出后的释放；活实体计数不能证明 heap 或内部 owner 无泄漏。

预热须覆盖计时路径的有界记录及其淘汰稳态；内容数量稳定，不代表请求快照成本已经稳定。否则样本数会改变增长期与稳态的混合比例。

测量绑定源码、二进制、环境和完整配对数据，隔离插桩及并行构建影响。跨 update 的整次操作累计耗时不能当作单帧耗时；限额 profile 应分别观察最慢 update 与热点占比。headless 导航、窗口主世界时间和屏幕呈现回答不同问题；方法及数据归[性能基线](../performance.md)。

## 文档只维护当前答案

每条规则有明确归属，跨主题用链接。复杂功能按职责组织设计/计划，小修复不机械新建文档；交付后必要语义归位，已完成计划和日志由 Git 追溯。经验只保留可复用结论，不复制阶段状态、提交、测试计数、临时路径或其它项目内容。
