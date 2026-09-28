# Bevy UI Router MVP Implementation Plan

## 原生内容清理减法规划（2026-09-25；2026-09-27 完成 P1–P4）

用户已批准执行本规划。2026-09-27 已完成 P1：有限纵向原型通过，正常导航两轮对照显示 128 子节点同场景 p50 均值降低约 24.9%，32 子节点降低约 13.5%，1 子节点无改善。P2 完整迁移与 P3 冻结版本验收已完成；正式 128 子节点正常导航 p50 均值降低约 22.6%，32 子节点降低约 12.3%。设计依据为统一 spec 第 13 节；历史已完成阶段不重开，现有合同不因实施启动自动失效。

**目标**：在保留当前公开行为和真实清理责任的前提下，减少普通 BSN 内容退出时的转换、结构写入与重复证明。性能依据为 `55911d6` 及现有正式基线；不将“测试通过”当作性能就绪，不预设通用毫秒 SLA。

**推荐路线**：先用同内容引擎对照及有界原型验证，再在同一 ledger/owner 模型内区分登记结构和普通内容授权，最后完成行为、成本与发布使用验证。Core/matcher/history 和 Scene 创建期未挂接产物的责任模型保持现有语义。

### 投入、风险和收益排序

以下为一名熟悉 Rust/Bevy 的工程师集中工作的工程日量级，包含调试和审查，不是模型运行时间或交付承诺。阶段互相依赖，不能按代理数量等比分摊。

| 阶段 | 工作量 | 风险 | 收益 / ROI |
| --- | --- | --- | --- |
| P1 对照、接口盘点与有界原型 | 1–2 日 | 低至中；原型隔离，未证明前不接生产 | 最高确定性 ROI：用有限投入决定是否值得迁移 |
| P2 同一 ledger 的内容授权重构 | 3–5 日 | 中高；涉及 owner、义务数量/digest、异常移交与输入消费者 | 最高潜在收益；取消额外逐节点注册/marker 写及对应复验，实际幅度由 P1 决定 |
| P3 完整回归、正式基准和收尾 | 2–3 日 | 中；可能揭示 P2 漏迁的角色或回调事实 | 必需；防止用跳过工作、误清理或隐藏延迟换取提速 |
| P4 操作内清理计划（条件项） | 另加 3–6 日，含复验 | 中高；预期状态更新与各角色语义必须准确 | P2 后重复分类仍为主热点时，直接减少分类/证据重建；不提前承诺收益 |

推荐第一轮总量暂估 **6–10 工程日**；第一笔投入限制为 P1 的 1–2 日。若 P4 也值得实施，合计约 **9–16 工程日**，它不自动成为必做项。若 P1 暴露跨 Core/多协议扩张或无法保留必要行为，不把估算自动加码为无限重构。继续微调原分类器虽然便宜，但不改变重复次数；全库重写的范围与收益无法由现有证据支持，不安排工期。

### P1：证明值得替换的最小路径

**主要文件与职责**：`benches/router_baseline.rs` 保留现有公开导航测量；同目录的直接 BSN 对照（若保留为独立 bench，同时更新 facade `Cargo.toml`）只量化同内容创建/删除成本；`presentation/native_frame.rs`、`presentation.rs`、`cleanup.rs` 与 `branch_transaction.rs` 用于隔离原型和消费者盘点。运行结果归 `docs/performance.md`，不另建性能平台。

- [x] 固定依赖、机器、优化配置、场景内容、请求完成终点和成功断言。直接对照使用相同 Scene 内容与基础 plugins，分别清楚记录 resolve/apply/despawn/update 的计时范围；它是成本参照，不是 Router 等价实现。
- [x] 核对 `CleanupObligation.claim`、`RealizedFrame::cleanup_members/obligation_count`、`PreparedCleanupLedger` 的 digest/count、各 `CleanupOwner` 及 `NativeCleanupContent` 全部真实消费者。列出必须改动的唯一数据来源，排除以 root claim 伪装 child record、按存活数量推断完成或从快照重建 owner。
- [x] 隔离验证普通已 adopt 内容的只读准备和原 owner 消费：不为删除新插 marker，不新增普通 child registry record，仍保持确切目标、叶到根顺序与真实 despawn 后复验。以正常 replace、动态 child、真实销毁 hook 移走剩余节点/改变 protected 关系、原始未挂接 apply failure 为最小正反证据。至少贯通一条真实残留/authority-loss 移交及完成出口，证明 `quarantine_residual`、`AuthorityLostResidualProjectionFact`、`FrozenCleanupTerminalAuthority` 不需要伪造 child claim 或重新登记普通内容。
- [x] 临时计数记录准备期注册/结构写次数、分类/关系遍历次数和剩余分段时间；无插桩版本同机交替比较。计时必须完成相同公开工作，不能将 cleanup 移到计时外。原型缺失的异常路径明确标记，不能把该数字当生产收益。
- [x] 输出 go/no-go：证明 owner/完成接缝可保持单一、没有扩张权限，并且成本下降足以支持迁移。只有个位数百分比或噪声收益、需要第二套正常状态机、必须取消真实写后复验时停止原路线；保留已证明有价值的独立小改动建议。

2026-09-27 已完成的独立产物：直接 BSN 对照两轮优化运行和 smoke 均通过，128 子节点 p50 50.77–52.65 µs；未改运行时代码的 Router 重测仍为 16.39–17.09 ms。数据与边界归 `docs/performance.md`。补齐公开 facade 中仍在页面内的动态 child/grandchild 清理断言，完整 `bsn_facade_features` 6 项通过；主工作区 fmt 和 workspace all-target/all-feature check 通过。以上不证明新的内容清理实现已通过。

随后新增普通应用 `On<Despawn>` observer 回归，在原始 `55911d6` 运行时代码上复现：首个内容节点删除时，回调将剩余节点移至应用父节点并设置应用焦点；32 次普通 update 后仍为 `Running(PostCommitCleanup)`，`foreground_pending=true`，回调恰好执行一次。该反例补足旧 marker-insert 逃逸测试的时序空白；该回归在原版本失败，在 P2 最终实现通过；原 owner/receipt 经同一终结通道有界收敛，没有增加重试次数或提前清空前台状态。

隔离原型进展：同一 ledger 中两种目标身份已贯通 normal leaving 和逃逸 tombstone/receipt；真实 source/protected 漂移沿原永久失败接收器保留完整 ledger/原 Leaving owner，前台有界结束。5 条新增机制探针通过，完整公开 facade 7 项通过（包括上述在原始版本失败的 observer 反例）。完整 ECS 诊断 666 通过、8 失败：3 项是显式未迁移的 authority builder，5 项依赖旧 marker/逐 child 登记过程。原型不是生产实现，也不能将这次诊断称为完整回归通过；两轮优化交替对照、独立计数构建和独立复核完成，P1 决定 go；范围和原始数据见性能记录。原型仍每次执行 8,775 次分类（128 子节点），不可将转换减少等同于分类次数减少。

消费者盘点确认：`RealizedFrame.entities/claims` 和 branch claim rebind 只应表示真正注册的结构，不能把普通内容加入后继续沿用 claims 数量等式；ledger 的完整义务数量、digest、receipt 则必须包含两类目标。native capture、normal/failed/authority 三类构造、classify/settle、focus、残留移交和 terminal 需要同一目标身份表达。`AuthorityLostResidualProjectionFact.retired_claim`、registry tombstone 与 `FrozenCleanupTerminalAuthority` 的逐目标 claim 耦合是实际迁移工作，不是必须逐节点登记的产品要求。未发现方案存在语义上的必然矛盾，P1 仅证明有限纵向接缝；P2 已迁移这些生产消费者并通过完整工作区。

P1 产物是决定和证据；临时原型不直接作为成品合入。正式保留的 benchmark 增量独立验证，原生产样本与旧 CSV 不改写。

### P2：一次迁移完整的内容清理责任

**修改边界**：`presentation/native_frame.rs` 负责只读内容准备及角色事实；`presentation.rs`、`branch_transaction.rs` 负责原 frame/branch owner 和清理成员移交；`cleanup.rs` 负责两种授权证据、统一分类/执行/receipt/完成；`registry.rs` 仅保留实际登记 Entity 的 exact retirement。`frame_local.rs`、`input.rs`、`native_focus.rs`、`navigation/authority_isolation.rs` 与实际导航接收器迁移 marker 相关消费者。仅在职责明确且能局部封装时提取私有内容清理模块，不机械拆分全库。

- [x] 在同一 ledger 内区分 Registered / ContainedContent 证据，原 `CleanupOwner` 继续线性持有实际权限；root/shell/slot、未 adopt 创建期产物继续原 exact registry 方式。内容义务的 Entity、source、关系、顺序、digest/count 和 receipt 全部进入真实完成校验。
- [x] 成组退出普通后代的 cleanup 注册/marker 插入、只为该转换存在的 manifest CAS、逐 marker retry 和逃逸标记修补。迁移全部读取者后才能删除旧机制；不把私有 `NativeCleanupContent` observer 回归原样复制成新产品接口。
- [x] 固定开始消费时的目标和 source；回调后重建观察用于验证与分类，不扩大原删除范围。保留提前移出内容、执行中移出内容、已缺失目标、child-frame barrier、保护对象关系改变的不同结果及原 owner。内容归还应用后不得继续保留其 Router 输入/焦点管理权限。
- [x] 对普通内容和 app-owned protected 状态按实际授权目的校验，允许无关正常组件/样式变化；同 tick 身份替换、真实关系/隔离变化、错误 owner 和跨 frame/source 仍拒绝。共享只读事实与借用比较，不共享不同角色的失败结论，不跨 World 写入复用有效性判断。
- [x] 贯通所有使用已 adopt 内容的 leaving、failed branch 中已实现 frame、host/authority loss、Back 和残留移交通道。原创建失败的未挂接义务继续保留；permanent/retry/source/原错误与发布后结果保持原分类。只读准备失败返还完整原 owner，永久停止不得新增无限等待前台 holder。
- [x] terminal/quarantine 证明区分实际 child Entity、仍由原 owner 持有的 source shell/root 及其隔离状态；digest/count 与真实 structural claim 集合分别验证，不能简单删除旧 `claims.len() == obligation_count()` 检查而不提供对应一致性。无 registry record 不代表无清理义务，也不能用 root record 填补 child record。
- [x] 保留输入与 focus 的真实授权来源和撤权范围；页面离场先撤权，历史手势不重新派发，移出内容和其它有效目标不被错误清焦。异常残留仍保持已有隔离责任，不把取消 marker 当成取消残留管理。
- [x] 每个可独立验证的提交完成受影响测试和独立审查；正常 adopted 内容只有一条生产执行路线，不以新快速路径长期回退旧正常路径。Core 只接收原语义的 opaque 物理完成结果，不新增 Router 模式或公开 API。

### P3：在同一冻结版本上验收和收口

- [x] 现有 public journeys、独立 facade consumer、原样嵌套示例与 native input/focus 回归全部保留必要行为；新回归同时覆盖合法应用 hook 和确实改变权限/关系的反例。旧私有结构断言随实现退出，不能以减少断言掩盖用户结果变化。
- [x] 根代理独占 Cargo/rustc，执行 fmt、workspace all-target/all-feature check、完整 workspace tests/doctests、完整 ECS BSN/no-default，以及受影响最高层 facade/consumer。提交前检查精确文件集合；保留当前两份 manifest 的用户改动，不顺手纳入。
- [x] 两轮原样无插桩正式 benchmark 对比正常、失败/恢复、空闲与 soak；报告 p50/p95、实际 update 数、实体/history/请求保持及规模增长。若减少的仅是子阶段耗时而总导航未改善，不宣称产品提速；不从临时原型外推最终收益。
- [x] 使用现有窗口示例检查导航、布局状态、fallback、焦点与旧手势撤权。使用 Bevy BRP 调试：示例独立 opt-in feature 在进程内投递 Bevy 输入消息并由渲染器截图，避免全局 OS 输入/焦点操作。该检查验证真实引擎管线，不冒充 OS/硬件输入或物理 GPU 性能；不新增平台测试矩阵。
- [x] 独立实现/合同审查关闭实际问题后，更新 spec/plan 状态、current facts、性能记录和 roadmap。确认成本仍是否逼近整帧、收益是否与复杂度相称；尚未解决的热点如实记录，不自动开展下一轮优化。

验证入口沿用：`cargo +stable fmt --all -- --check`、`cargo +stable check --workspace --all-targets --all-features --locked --offline`、`cargo +stable test --workspace --all-features --locked --offline`、`cargo +stable test -p bevy_ui_router_ecs --features bsn --locked --offline`、`cargo +stable test -p bevy_ui_router_ecs --no-default-features --locked --offline`；正式 benchmark 命令和环境使用 `docs/performance.md`。依赖来源保持当前 lock，未运行结果不预写 PASS。

### P2/P3 交付与验收

- 普通已 adopt 内容不再为退出登记 record 或插 marker/seal/lifecycle，capture 使用真实保留容器；删除转换专属 retry/cursor、逃逸修补与未被读取的 cleanup failure 副本。真实结构与创建期未挂接产物保持自己的 exact claim；普通内容仍计入统一完整 count/digest/receipt，不以登记结构数量推导清理完成。
- leaving、failed、authority/host-loss、Back、postcommit、quarantine/terminal 和输入/focus 消费者全部迁移。普通残留进入同一现有 lease，分别保留原 source/receipt 和终结时的隔离身份；没有新正常 fast/fallback runtime、Core/public API 或 owner 类型扩张。独立实现/合同复核无未关闭阻断。
- 新真实 hook/observer 回归覆盖动态 child/grandchild、执行中逃逸和应用 focus、逃逸旁仍 contained 的残留、protected/source 漂移、祖先变为登记边界，以及允许无关组件/Node 样式变化。原版本永久 Running 的公开反例在最终版本通过；永久停止保留原 ledger/owner，前台有界结束。
- fmt、workspace all-target/all-feature check 通过；隔离实现与同步后的主工作区各完成全量 1,226 项单元/集成及 28 项 doctest，0 failed/ignored。ECS/BSN 最终 673+1 包含于完整工作区；无 BSN 的 ECS 单独 619+1 通过。主工作区保留用户的两份版本范围 manifest，lock 仍解析 Bevy 0.19.1。共享 target 曾重用含 worktree 路径的 Core consumer 测试产物，清理对应编译缓存后主工作区全量复验通过，未修改产品代码掩盖环境失败。
- 原样无插桩正式 benchmark 两轮各 48 行全部断言通过，正常/失败/恢复分别保持 1/2/3 update。正式性能、连续使用、精确 hash 与原始 CSV 归性能记录；128 子节点 p50 为 12.586–13.328 ms，尚不含布局/渲染。有限 P1 的约 24.9% 不作为最终收益。
- 用户要求避免其它窗口干扰后，停止全局 X11 输入方式，实现示例专用 BRP feature、窗口内消息和 renderer PNG 截图。实际 Linux/WSLg Vulkan/llvmpipe 验证布局 Entity/计数保持、push/replace/back、页面点击/Space、focus 释放、旧页面手势撤权、Build fallback 不改 history 及恢复。新悬停目标允许收到正常 release，旧目标不能收到 release/click，新页计数保持 0；未扩张为“所有新目标都禁止 release”。本轮不将进程内消息验证称为 OS/硬件验证。默认及调试示例构建/检查、CI 同命令的双 benchmark smoke 经实际运行，远端 CI 未触发。
- 临时原型 bench 已退出，保留直接 BSN 对照和正式基线；spec、current facts、roadmap、性能记录与 README 已同步。P1–P3 至此收口；P4 保留为需重新 profile 与收益门槛的条件候选，本轮没有自动继续。未提交、发布或改变许可证决定。

### P4：仅在剩余重复分类仍值得处理时统一操作内计划

2026-09-27 用户继续授权推进 P4。当前 P2 冻结实现的隔离诊断确认：128 子节点场景中 collect/classify 占 settlement 的主要部分，当前 focus closure 的重复查找亦显著；决定验证单一 operation-local removal plan 候选。插桩仅在隔离副本，原样正式 benchmark 不改。

- [x] 对当前 P2 而非历史原型重新 profile，保留未插桩 P2 对照可执行文件。
- [x] 替换普通删除循环：一次接纳角色/目标/实际图与动作，逐真实写后重新核对完整相关事实；保留原失败分类和 owner/receipt，不留快慢双实现。
- [x] 用真实回调验证允许的组件/样式与 sibling 重排，以及关系/source/foreign 边界变化的拒绝；独立审查当前图与删除顺序。
- [x] 完整相关门禁、原样优化对照与 BRP 真实渲染/input 验收；收益不足或需要扩大需求时收口候选。

P4 已完成并合入主工作区，随后按用户指令提交为 `b0dbc3b`。初始及逐写直接图比较保留全部 accepted target、source、registry/lease、protected、mount 与 focus 事实，漂移继续走原 ledger 分类/终结；不重建 owner，不保留旧普通循环作为另一 runtime。实际图叶序与每实体 receipt 有真实重挂/Despawn 回归，普通组件/样式及 sibling 重排有真实回调正向回归；独立交叉审查无必须修正项。

冻结候选通过完整主工作区 1,229 项单元/集成与 28 项 doctest，ECS/BSN 676+1（已包含）、无 BSN 620+1；fmt、workspace all-target/all-feature check、原基准与直接 BSN 对照 smoke、窗口默认/可选调试配置检查及构建通过。三轮原样优化交替对照共 288 行全部断言通过：相对当前 P2，32/128 子节点正常 p50 均值分别降低 21.8%/35.6%，1 子节点无收益，恢复未改善；完整分位数和未删结果归性能记录。

主工作区窗口使用 Bevy BRP 进程内 WindowEvent/typed message 输入和 renderer 截图完成真实 picking、Space、focus、Push/Replace/Back、布局状态保持、已按下手势跨页撤权、失败 history/fallback 与恢复验证。查看初始/失败/恢复图片，日志断言全部通过，随后关闭本次测试窗口。环境仍为 Linux/WSLg、llvmpipe 软件渲染；没有全局键鼠注入，不把结果表述为 Windows 原生或硬件输入/GPU 验收。

以下为 P4 原进入门槛，现已满足：P3 后重新 profile。只有普通真实删除阶段仍主导总时间、总导航仍缺乏实际余量，并且收益足以支持额外迁移时采用。本阶段估算包含受影响的完整回归、独立审查和正式基准复测，不以新计划无限续期。

主要接缝为 `CleanupLedger::settle_with_failure_source`、`collect_evidence` 及只读 partition 比较。冻结一次 accepted roles 与 leaf-to-root 删除列表；每次实际 mutation 后按已完成删除更新预期关系，再重建并比较完整相关 partition。证据失效即停止，不能因为已有删除列表继续写。重点是避免反复生成每个剩余目标的完整分类/分配，不是跨写缓存结论或每批末尾才验证。

可参考既有 `PreparedFailureCleanupRemoval` 的准备/逐写检查分工，但只有目的、角色与失败语义相同时才共享代码。验收继续覆盖正常允许的回调与 target/protected 关系漂移；不把普通组件变化重新解释为 ownership 变化，不根据目标自身 hook flags 跳过 parent/relationship 回调复验。若最终只带来噪声级收益或形成另一套通用状态机，停止该候选。

本轮不安排 Core/history 重写、全局缓存或 registry 索引、分帧清理调度、创建期协议重做、后台 reaper 或新公开配置。只有新证据能重新打开相应问题。发布仍按已有决定先准备、暂不授权开源许可证或正式发布。

## 官方 Bevy 版本范围与独立发布（2026-09-28）

依赖声明统一为 `0.19`（`>=0.19.0, <0.20.0`），不新增旧版本适配分支。默认 workspace 与窗口示例 lock 保持官方 `0.19.1`；独立 facade/打包 consumer 的精确 `0.19.1` 约束是该测试夹具的固定基线，不是库的最低版本要求。

最低版本验证在隔离源码副本进行：将 Bevy 依赖固定为 `=0.19.0`，对默认 lock 中全部 47 个 Bevy 包补充不参与本机编译的精确版本约束，重新解析并核对它们全部来自 crates.io、版本均为 `0.19.0`。这避免只降低顶层版本、实际混用 `0.19.1` 子包。隔离副本使用独立 target，未污染生产 lock 或构建产物。

官方 `0.19.0` 的 facade all-features 库编译、ECS 完整 676 + 1 项，以及 BSN facade 8、原生 focus 19、原生 input 18、公开导航 43 项均通过。focus 的实际运行验证覆盖 Reflect 观察和官方焦点事件管线，不能只用编译成功证明兼容。默认 `0.19.1` 的最终完整工作区复验通过 1,238 项单元/集成与 28 项 doctest，0 failed / 0 ignored，workspace 和独立窗口示例的 all-target/all-feature check、格式检查均通过；本轮没有以任一私有 fork 作为官方兼容性证据，也没有新增平台矩阵或硬件验收。

本库文档仅保留通用缺陷、仓库内可复现验证和设计经验；外部应用的业务、日志与迁移状态已移出。项目面向独立开源发布，许可证另行确定。包含跨项目记录的未推送提交与这次清理合并，原始历史和工作区差异仅备份在仓库外，不发布这些中间提交。既有性能记录中的旧提交 ID 保留为测量时源码标识，不再作为发布历史中的可检出节点。

## 嵌套 Back 挂载与失败收敛修复（2026-09-28，基线 b0dbc3b）

**本次 Back 阻塞已修复。** 官方 Bevy 基线的公开 `history_back_restores_nested_location_after_search_rebuild` 在修复前独立复现了嵌套分支随 query 重建后 Back 无法结束：请求保持 Running(ApplyingFrame(0))，最后 trace 为 MountRequested。修复后完整回归通过。问题属于通用路由行为，不依赖外部项目、私有 Bevy fork 或业务页面。

本节仅记录本库实现、可复现证据与可复用经验。外部应用的迁移、页面交互、日志和测试结果由相应项目维护，不纳入本库验收状态。

根因：`dispatch_pending_back_target` 不接 initial MountRequest；applied 后的 MountRequest 被存为 `AppliedMountRequestPending` 而没有继续执行者。历史 plan 保留的阶段性 safe-stop 在当前实现及内部成功测试中仍存在。公共嵌套 Back 只改变 leaf 参数、保留 slot；search 用例仅 Replace，组合覆盖遗漏。独立只读交叉审查已确认这些路径与现行 MVP §4 合同冲突；不是新增需求，也不能用调整 query 语义或 Replace 冒充 Back 修复。

修复范围与验证顺序：

- [x] 在本库官方依赖基线上复现，补公开失败回归，更正验收结论。
- [x] initial/applied MountRequest 统一进入受控 target mount continuation，复用已有物理挂载与 Core 算法；完整保留 history、continuation、pending owner。成功返回真实 FrameTask；拒绝、容量耗尽、写后失败转入对应 target cleanup，不能遗留 owner 或落入无关 generic failure 路径。
- [x] 迁移内部“合法请求永久 safe-stop”阶段测试，保留单次消费、mismatch 返还与零写证明；补 initial mount、连续两层挂载、缺失 outlet/真实 apply 失败的有界终结。既有 preserved prefix 状态与 identity 保留必须继续成立。
- [x] 相关 Core/ECS 完整 crate、最高层公开旅程、仓库内独立 facade consumer、fmt/check 与独立交叉复核通过；功能验证不替代性能验收。

实现与经验：initial/applied MountRequest 现在沿同一物理挂载执行，原 history/continuation 继续持有；失败桥只在 Core 确认 exact receiver 后消费 continuation，完整物理 branch 交给既有 manifest/cleanup。嵌套失败不再套用单层零残留分类。成功报告原先使用的 root driver-failure freeze 在 cleanup-only 移交后已无消费者，连同专属阻断 holder 删除；挂载、commit、cleanup 各自的真实准备和写前/写后验证保留。

普通 `App::update` 补充暴露两个单层假设：进入中的父页面可以与旧页面合法共槽；等待 MountRequest 的分支必须仍可被下一帧输入 Refresh 观察。父页面校验复用既有 exact reconciliation binding，Core 只读接口借用真实 held/pending MountRequest 对应的 projection，不消费 action、伪造 task 或新增路由状态来源。新增公开旅程覆盖 query 全分支重建、连续两层挂载、保留 root 并重建参数化 middle/leaf、保留父页面局部状态，以及缺失 outlet、真实 apply/build 失败的有界终结与重试。私有阶段测试改为最终成功/失败、单次报告、owner 释放和应用 host 保全断言。

本项目经验：阶段性 safe-stop 必须标记为未交付，不能被永久接受停滞的测试转化为完成证据；覆盖按公开操作 × 分支形态组合检查，并使用真实 schedule 穿过 Refresh/drive/settle，连续手动调用 drive 不能替代。复用证明前先核对其目的和角色，失去消费者的中间证明应删除。现阶段不支持将问题归因于最近的 cleanup 性能迁移，也不据此重写整个 Core/history。

验证结果：Back 定向 ECS 84 项、完整 workspace all-features 1,238 项单元/集成及 28 项 doctest 全部通过，0 failed / 0 ignored；其中公开导航旅程 43 项、Core 单元 433 项、ECS 单元 676 项，独立 facade consumer gate 同样通过。ECS no-default 620 + 1 项通过；fmt、workspace all-target/all-feature check 和 diff 检查均 exit 0，保留既有 warnings。沿用上文 2 jobs、关闭 incremental 和 dev/test debug info 的 offline/locked 配置。新增参数化 middle 回归曾错误要求叶页面继承父参数，已依据现有 route-local params 合同改为逐帧验证，未改变产品语义。最终 runtime 经独立静态复核未发现新增阻断。

## P4 后性能验证（2026-09-27，基线 b0dbc3b）

用户已批准限定范围：重新 profile 当前实现、补相同节点数的平铺/普通嵌套对照、测量真实窗口切换帧，再按证据至多选择一个主要热点。不新增平台矩阵、全局缓存、分帧清理或产品能力。保留两份用户原有依赖版本修改；主代理独占 Cargo 与正式计时。

- [x] 隔离诊断当前 cleanup 的分类、计划准备、receipt、逐写目标/protected/mount/focus 检查及输入刷新，计数与时间分开，嵌套时间不相加。
- [x] 原 benchmark 默认 flat 不变，增加显式 nested 形状；同 authored Entity/Node/Content 数量、同生产导航及断言（关系组件分布随拓扑改变），计时外校验实际深度和数量。原样 flat 与 nested 各至少两轮，保留全部结果。
- [x] 一次性窗口探针在优化构建中通过 BRP 启动定向引擎输入批次，采样阶段不做 RPC 轮询/截图。分清主世界经过时间、帧间隔与不可据此推断的 GPU/显示延迟，保存实际环境和行为断言。
- [x] 依据新 profile 选择零或一个实现候选；只有无插桩总时间有稳定收益且维持原语义时合入，并运行相称完整回归与独立复核。其余候选只记录，不连续叠加。

结果见[性能记录](../../performance.md#2026-09-27-p4-后性能验证)及其原始 CSV。protected 事实读取成为剩余主要热点，但隔离的 EntityRef/借用 claim 比较候选未显示可保留收益，决定不合入；生产 runtime 保持 `b0dbc3b`。flat/nested 各三轮，两轮配对受宿主机其他构建干扰并明确标记，只有第一轮用于描述性比较，稳定形状差异仍未证明。32/128 节点 nested 真实深度为 4；真实窗口两轮共 80 次切换成功，保留全部 960 帧，未使用全局键鼠或窗口焦点操作。

本轮交付限于形状 benchmark、nested CI smoke 和结果记录。共享 target 诊断产物复用问题已通过强制重编译与保存核对后的二进制修正，早期无效输出不进入正式数据。候选与临时窗口探针均未进入主源码，临时 release 窗口产物也已清理。已运行主 workspace fmt、all-target/all-feature check、两种形状优化 smoke 与完整 benchmark 的行为断言，均 exit 0；继续验证时也按 CI 环境执行本地 test profile 的 flat Router + 直接 BSN smoke、nested Router smoke，均 exit 0；未改生产 Rust，不重复冒称本轮运行了完整单测、远端 CI 或跨平台矩阵。既有 warnings 保留。后续只在宿主机空闲时复测形状/soak 的稳定性，或以真实产品页面帧预算触发进一步设计工作。

## 分类器剩余热点局部简化（2026-09-25）

用户确认在 `f44089c` 上重新 profile，并按证据选择一项局部优化。延续已有合同、公开基准和验证范围；主代理独占 Cargo/rustc，独立代理只读复核，保留已有两份 manifest 改动。

- [x] 隔离副本重新计时：128 子节点 settlement 约 8 ms，其中分类约 6.4 ms；native cleanup preparation 约 5 ms。两个单变量插桩对照显示，分类时仅有两项的关系组件 HashSet 改为固定数组，比复用 registry 查询的整体收益更高；独立确认成员判断等价，不跨 World 写入边界。
- [x] 无插桩交替对照确认收益后，仅落实关系组件固定数组；保持原提前返回、normalized/foreign/missing component 判定、写后复验及所有行为回归，不同时加入 registry 索引或其它优化。
- [x] 冻结源码执行完整受影响 crate、最高层公开使用/consumer gate、格式/静态检查及两轮原样完整 benchmark，独立审查并更新性能记录。

结果：生产代码仅替换分类函数的一处两元素集合及三处成员判断；独立审查确认 `None`、提前返回和全部写后复验语义不变。`cargo +stable fmt --all -- --check`、workspace all-target/all-feature check、完整 workspace 1,220 项单元/集成与 28 项 doctest，以及无 BSN 的 ECS 619 + 1 项测试均通过，0 failed / 0 ignored；ECS/BSN 单独的 669 + 1 项测试亦通过。check/test 沿用 2 个构建任务、关闭 incremental 与 dev/test debug info。

两轮原样完整 benchmark 均 exit 0，断言通过，原始 96 行及分位数/soak/未改善场景统一归[性能记录](../../performance.md#2026-09-25-分类器局部简化)。128 子节点正式导航 p50 为 16.3–17.0 ms，相较前轮 p50 均值约减少 7.2%；不将临时短对照的约 14–15% 替代正式收益。未修改 benchmark、Cargo.lock、公开 API 或平台矩阵，未采纳其它优化候选。

## 性能问题局部修正（2026-09-25）

用户已确认先做两项有运行证据的局部简化：同轮 cleanup classification 共享未结算实体集合；合并 seal 发行/注册，删除 lifetime consumed-seal 账本。沿用现有 spec 和公开 API，不增加全局缓存、Entity 索引或跨平台性能矩阵。

- [x] 每轮只读 evidence collection 复用惰性集合；保留单实体分类入口及每次 World/receipt 写入后的重新采集，独立运行完整 ECS/BSN 与 public navigation/input/focus 回归和优化构建基准。
- [x] 唯一内部注册入口分配身份并返回 exact claim，迁移生产调用和有效测试；保持失败后身份不复用、容量预留、domain/owner/lifecycle/CAS 与写后复验，删除被替代的历史账本及旧私有协议测试。
- [x] 独立审查两项改动；最终源码运行受影响完整 crate、最高层 facade/consumer gate、格式/静态检查，并以相同 fixture 的两轮完整基准复测。原始记录与解释更新到性能文档。

主代理独占 Cargo/rustc 与 index；保留已有 ECS/BSN manifest 的版本范围改动，不纳入本轮重构范围。

### 局部修正结果与验证

- cleanup 集合只在同一只读 `collect_evidence` 中共享，独立分类及 World/receipt 写入后的重新采集保持不变。融合注册使生产调用无法提交旧 seal，成功直接得到 exact claim；失败序号仍消耗，删除 lifetime 历史列表而保留 domain、单调序号及 exact record 校验。
- 迁移生产调用及真实 drift fixture；旧私有 seal 注入协议测试由新入口下的不复用、stale/foreign claim/CAS 拒绝测试替代，补注册后删除、失败尝试及同 tick registry 替换的 projection 回归。独立审查覆盖两项改动和全部测试迁移，未发现阻断。
- Rust 1.98.1 下 `cargo +stable fmt --all -- --check`、`git diff --check`、`cargo +stable check --workspace --all-targets --all-features --locked --offline` 通过。完整 `cargo +stable test --workspace --all-features --locked --offline`：1,220 项单元/集成及 28 项 doctest；ECS `--features bsn`：669 + 1 项；ECS `--no-default-features`：619 + 1 项，均 0 failed / 0 ignored。完整 workspace 包含最高层 public journey、原生输入/focus 和独立消费者 gate。check/test 使用 2 个构建任务、关闭 incremental 与 dev/test debug info，既有 warnings 未作无关清零。
- 两轮默认 128 样本优化 benchmark 均正常退出，全部行为断言通过，fixture 与锁文件 hash 未变；结果及精确测量边界统一记录在[性能文档](../../performance.md#2026-09-25-局部修正)和[原始 CSV](../../performance-baseline-2026-09-25.csv)。128 子节点导航约减少 53%；32/128 子节点原先约 50% 的连续使用退化未再出现，但约 18 ms 的大页面 CPU 成本仍在。未重跑窗口/GPU或新增跨平台矩阵，也未把该项余量视为已解决。

## MVP 发布准备（2026-09-24）

用户确认推进窗口示例、打包和最小发布检查；许可证明确选择暂不授予，先完成准备。因此本轮不发布 crates.io、不打正式 release tag。沿用现有 spec，不扩展 Router 能力或跨平台矩阵。

- [x] 独立 `examples/windowed` 应用使用公开 facade、官方 BSN、Winit 和 UI renderer，实际窗口验收 push/replace/back、共享布局状态、构建失败/fallback、恢复以及输入/focus 撤权。窗口图形依赖不加入库的 feature。
- [x] 为六个库补内部依赖版本及描述/repository/readme，验证真实打包产物和独立消费者；不把 `--no-verify` 或仍引用工作区源码的编译当作包验证。
- [x] 单 Linux CI 固化 fmt、all-target/all-feature check、完整 workspace/consumer 测试；更新使用与发布说明，记录实际运行环境及尚未验证项。

Cargo/rustc 验证和 index 仍由主代理独占；独立代理负责元数据与 CI，主代理负责窗口示例、实际交互、产物验证及文档收尾。

### 发布准备结果

- 窗口应用使用独立 workspace/lock，根 workspace 显式 exclude；CI 分开解析和构建，避免 Winit/render feature 合并到 headless 测试。原库的 runtime 和 feature 合同未扩张。
- Linux/WSLg X11 窗口实际显示正常页和错误 fallback。通过 OS 注入的鼠标/键盘输入验证：布局计数从 0 到 1 后跨 push/replace/back 保留；页面点击与 Space 正向计数；切页后 focus 清空，新页无焦点时 Space 不生效；按住旧页按钮并切页后，旧 Entity 不再收到 release/click，新页计数保持 0。Build failure 保持 history，随后普通导航恢复。软件渲染和 OS 事件不冒充物理 GPU/硬件验证。
- 六包内部依赖均有版本，补 description/repository/readme；共享 README 的仓库链接改为绝对 URL，避免解压包中的相对链接失效。按用户选择无 license 字段/文件，也未发布。
- 标准在线 `cargo +stable package --workspace --allow-dirty` 对六包逐个解压构建通过。stable 1.98.1 与 nightly 的离线多包验证均出现临时 registry 的 `no hash listed` 内部错误；没有用 `--no-verify` 掩盖。新增产物消费者脚本只 patch 六个 Router 包到实际解压目录，核对完整来源后执行包内原样 BSN journey（含 96 次替换），已通过。
- 稳定版执行发现 Core 一条 compile-fail 断言依赖 nightly 完整措辞；保留 E0423 与目标类型验证，删除易变句子。原非法构造仍被拒绝，修正后完整 workspace 重跑通过：Rust 1.98.1，fmt、all-target/all-feature check、1,217 项单元/集成测试及 28 项 doctest，0 failed / 0 ignored。
- 元数据/CI、窗口公开入口/插件/input gate、产物消费者来源分别独立复核，未发现新增阻断。远端 CI 尚未触发；没有把本地命令成功写成远端成功。
- 最终独立窗口命令 `cargo +stable check --manifest-path examples/windowed/Cargo.toml --all-targets --locked --offline` 与对应 `build` 通过；该可执行文件再次完成上述完整 OS 交互序列。实际后端为 Vulkan / llvmpipe（CPU 软件渲染）。更新后的六个打包产物及其独立消费者复验均通过；本轮不提交/推送正式 release，不运行其它平台或最低 Rust 矩阵。

## 上线前真实使用审查（2026-09-23，本轮审查与局部重构完成）

本轮由用户明确要求全面核查并修正、清理、补充。沿用统一 spec；此前 Linux/headless MVP 通过不等于上线审查完成。以完整公开能力、生命周期分支和应用集成路径为审查单位，不以测试数量、抽样或未执行的旧文件代替证据。

- [x] Core/View：逐模块核对定义/匹配、输入更新、history、请求队列与保留、诊断、identity、owner、恢复及 publication 的合同与真实正反验证。
- [x] ECS/BSN：逐入口核对安装、host/outlet、官方 Scene preparation/apply/adoption、成功切换、partial failure、cleanup、stale、host loss/readmission 和长期重复使用。
- [x] 输入/focus：完整核对原生 pointer 与历史手势、pending/failed/residual、动态内容、官方焦点队列及应用在 Ready 后消费 keyboard/gamepad gate；补真实 BSN 负向路径和独立有效目标对照。
- [x] Facade/示例/依赖：核对所有 feature 的正向使用及禁用边界、原样独立 consumer、逐步布局/状态保留、文档命令与 runtime 前置；迁移有效旧断言后删除失效测试、脚本和无调用 fixture。
- [x] 修正直接注入容量结果的 seam，使错误来自真实 reservation predicate；通过定向故障敏感性验证证明移除关键判定会使回归失败。
- [x] 真实环境：按用户澄清面向全平台，但没有明显平台限制时不新增跨平台测试矩阵；静态核对平台限制，重点验证真实应用插件组合。未执行的最低 Rust、窗口/GPU 与设备输入不借 headless 结果替代。
- [x] 对新增发现逐项复现/修正/回归，并做独立交叉复核；同一最终源码重跑 fmt、workspace all-target check、完整 tests/doctests、ECS 无 BSN、facade feature/consumer 门禁。

主代理独占 Cargo/rustc、index 和提交。审查代理按 Core/View、ECS/BSN、输入/focus 划分；文件修改单独指定 owner。审查结果与命令证据在本节归集，不新增第二份 spec 或固定测试 catalogue。

### 本轮减法与局部重构

用户已确认先复盘根因再实施。保留路由、history、布局复用、失败恢复、owned-only 和输入撤权合同；不把可信应用代码当作恶意 World 沙箱，不以理论计数器耗尽驱动新协议。

- [x] 用公开 BSN Scene 的应用组件 hook 验证 app-owned host 的无关组件变化；按实际写入授权收窄 ordinary failure cleanup 的 protected 校验。另以公开 Children observer 复现 host 接纳误拒绝，删除 staging 的全 World 组件/实体冻结及资源载体白名单，保留相关关系、目标写入范围和 exact ownership 拒绝。
- [x] 整理 ordinary failure cleanup 的推进结果，区分完成、真实可重试等待和永久停止；停止物理写入不能替代请求终结，残留责任不能继续占据前台请求。
- [x] 在真实路径支持下复用请求级终结准备与 owner 移交；删除被替代的永久 pending holder，不逐个新增 Back/refreeze 专属恢复 API，也不一次重写所有状态机。
- [x] 更新旧测试中无意义的组件清单冻结和永久 Running 预期，保留写入范围、host 状态、原错误来源、history 与后续请求行为；同一版本运行受影响完整 crate 和 public consumer/journey 门禁。

已撤下未实现的 Back 专用接口草稿。删除私有 shell 组件及极端计数器测试仅作为内部健壮性证据，不能代替公开应用故障或自动成为上线阻断。

### 本轮结果与验证

- 根因修正：可信 Scene 销毁 hook 给 host 添加普通组件后，仍按原 Build failure 发布 fallback，随后导航成功；host Children observer 修改、新建、删除无关应用实体后，接纳与导航正常。没有把新应用实体登记为 router-owned，也没有扩大清理写入集合。
- 生命周期减法：ordinary ledger 合并永久停止原因并保留原证据，删除额外的永久等待 holder；拒绝移交时返回原 ledger。Core 四条终结路径复用私有记录准备/安装，不合并分支 source/owner/continuation。已发布 normal 保持 Succeeded，已发布 error 保持原失败，history 不被清理故障改写。
- 长期使用与边界：host trace 保留最近 64 条，exact 注册去重独立保存；slot 耗尽不再无限重发；seal 耗尽在分配前拒绝。计数极限属于低成本健壮性修复，不据此夸大常见上线风险。
- 示例与测试：嵌套示例每次导航检查布局/状态保留；独立 BSN consumer 连续 96 次替换检查存活实体和 history。补原生 pending/failed/residual、动态内容、Back/fallback 手势和应用 Ready 后输入消费。删除未注册旧测试、25 个无引用 fixture、失效 catalogue 脚本及无合同依据的源码形状扫描；必要 authority 边界由真实编译拒绝验证。
- 测试有效性：临时绕过真实 verified/unresolved reservation、owner CAS、staging reservation 后，相关回归均实际失败；还原后完整门禁通过。没有保留 mutant，也没有用注入 terminal/classification 代替生产判断。
- 独立审查：Core/View、普通 ECS/BSN、输入/focus、specialized Back/no-target/target-permanent/PostCommit、capacity 与 facade/示例分别核对，并对本轮终结分类与 host 写入范围交叉复核。未根据私有 receiver/batch 破坏探针新增恢复协议；未证明正常应用可达的既有 inert 分支不作为本轮扩张实现的理由。

最终同一源码验证（成功命令输出已核对，0 failed / 0 ignored）：

- `cargo fmt --all -- --check`，以及独立 consumer 源文件的 Rust 2024 格式检查。
- `cargo check --workspace --all-targets --all-features --locked --offline`。
- `cargo test --workspace --all-features --locked --offline`：1,217 项单元/集成测试、28 项 doctest；其中 Core 431、ECS 666，独立 consumer 与两个原样示例均通过。
- `cargo test -p bevy_ui_router_ecs --no-default-features --locked --offline`：616 项单元测试与 1 项公开 API 测试。
- facade 的无默认 feature、默认、仅 ecs、bsn 四组 `cargo check` 均通过；独立 consumer 同时验证启用组合及禁用模块拒绝。

验证环境为 Linux/headless 与真实 Bevy Scene/UI/picking/focus 管线；未运行窗口/GPU、真实设备、最低 Rust 或其它操作系统原生执行。未发现本项目 runtime 的显式平台专属限制，按用户要求没有增加跨平台测试矩阵。既有 warnings 未作无关清零。

> **For agentic workers:** 执行时使用 `subagent-driven-development` 或 `executing-plans`，按下述交付结果推进与审查。用户已授权统一规划、合理调度子代理和安排提交；不为每个小修复重复征求流程选择，不机械套用 TDD 或另立 spec。

**Goal:** 完成可从稳定 facade 使用的官方同步 BSN Router MVP，保留完整基础/嵌套路由、history、恢复和 owned-only/input/focus 语义。

**Architecture:** 复用 renderer-neutral Core/View；BSN 提供 one-shot Scene factory 与 outlet 声明，ECS optional bridge 独占 World、staging、publication 和 cleanup。复用 shell/content-root 两层结构，Scene 内容采用 current containment，不延续旧 data-only component manifest。

**Tech Stack:** Rust 2024、官方 crates.io Bevy `0.19.1`、Bevy ECS/UI/picking/input-focus/Scene、Cargo workspace。

**Spec:** [统一 MVP design spec](../specs/2026-09-07-router-mvp-design.md)。实现者同时读取 spec 和本计划；产品合同归 spec，本文件只拥有迁移顺序与证据。

**状态**：MVP 已在本地 `main` 完成集成与既定验收（2026-09-23）。阶段 A–E 的代码、审查、最终同版本验证和合并后 smoke 均完成。官方 BSN、native 导航/恢复、输入/focus、同更新 snapshots 与推荐示例已实现并验证；三处接收端缺陷和最终私有 fixture 减法已分别复审。成功 focus 合同保持 spec §7.1；较早检查点和取舍见第 7 节。

## 全局约束

- 官方 crates.io Bevy `0.19.x`，默认锁文件为 `0.19.1`，最低 `0.19.0` 的补充验证见本计划版本范围节；无 Bevy fork、patch 或 source replacement，Core/View 和 facade 默认功能不依赖 Bevy。
- 旧 data-only renderer API 无兼容义务；不取消 renderer-neutral 边界、最小独立测试替身或既有导航行为。
- 每个 layout 一个 default outlet，可位于内容根或内部容器；不实现 multiple/named/dynamic outlets。
- Router 只管理自身结构、Scene 创建期声明图分配义务及成功 adoption 后的当前 containment，不销毁或重建 app-owned host，不回滚任意应用副作用。
- publication-last、exact identity/stale 检查、线性 owner、失败清理和输入/focus 隔离持续有效。
- 无 binding 的 nested Push 失败保持 cleanup 后 RouterFallback；不能为迁移恢复旧 Terminal 分类。
- 不把 public audit/probe、background reaper/fairness、固定 catalogue 或 hostile World 沙箱作为 MVP 前置。
- 全程一个 index/提交负责人、一个 Cargo/rustc 负责人；子代理默认只读审查，授权实现时明确文件所有权。

## 1. 基线与实施方式

本轮起点：`sys/structural-governance`，`0799098`。选择在当前候选上前向迁移，`1712be4` 仅作为六个候选提交之前的比较点；不整体 reset/revert，不重新执行已完成的 Recovery 开发。

进入 runtime 阶段前，用 `git status --short`、`git branch --show-current`、`git worktree list` 复核实时环境，遵循 worktree skill 的已有隔离检查。用户变更不能纳入或回退。出现新并行实现需求时再选择独立 worktree；不为纯文档审查复制工作区。

迁移起点的 Core/View 作为锚点。当时 ECS 主路径使用 `EcsViewDriver -> FrameContentWriter/ContentPlan -> RealizedFrame`，输入名单来自构建时输出，不能直接覆盖动态 native Scene；当前接线进度见上方状态，不把起点描述当作最新事实。六个候选含可删产品外扩张，也含必须保留的前台安全修复。

### 收口时的执行取舍（2026-09-23）

- 保留产品范围和 Core/View 分层；当前调整的是 ECS 操作职责与交付组织，不另开架构重写、第二份 spec 或全面安全整顿。已确认的创建期分配义务、adoption 后 containment、线性 owner 和输入隔离不能因精简消失。
- native BSN、idle admission、current containment、输入/focus 和公开导航/恢复已有接受的联动基线，推荐示例也已交付。阶段 C/D 的证据现已归集；各检查点证据见执行记录，集成前总体验收仍开放。
- 最终审查的三处接收端缺陷已闭合：统一实际事件与去重记录的保留责任、冻结原 attempt 的结果、使隔离后预期符合真实 shell 写集。修正通过完整 Core/ECS/facade/workspace 测试、下一普通更新对照及独立增量复审；不新增通用事务框架，不以日志位置代替 authority，不放宽 source refreeze。
- 旧公开 writer/report 和专属 replay 已退出。已按核实清单清理无调用者 helper、私有替身冗余和脆弱源码形状断言，保留真实 no-BSN 管线、owner 返还和物理输入回归；不把所有 ContentPlan、静态检查或 warnings 清零设为完成条件。
- 审查单元覆盖完整用户路径和最后有副作用的边界，包含同调用与下一普通更新；循环内运行定向验证，交付冻结后运行受影响完整 crate/最高层 gate，修复采用增量复审。独立/交叉意见须核对真实路径；不以评估文档数、测试数或局部绿色推算产品完成度。
- 核心范围和已确认的 AutoFocus 策略不重定。当前仅在真实修复触及的职责内消除重复知识，不整体替换 Core/RouterEntry，不增加后台回收、公开审计或未来 renderer。最终以同一冻结版本的完整验证、Core/ECS/public 独立与交叉审查、spec 结果及 main smoke 收口。

本轮按以下三个主要交付单元组织，全部已闭合：

1. **native 公共验收（本轮交付已闭合）**：公开 snapshot 同更新观察由 `a153520` 闭合；原 public journey fixture 已整体迁到官方 BSN，补齐 factory input/等价与 search/fragment、排队及失败 Back、boundary/build/resolve 恢复和两处真实 host/outlet 缺口。旧 writer 专属测试责任由现有私测保留，不另建永久平行 suite；全 MVP 验收已在第 10 节归集。
2. **示例与旧机制退出**：旧公开 writer/report 注册与导出已撤下，外部迁移 consumer 已改为明确的 ECS-only 安装验证，Data 注册及变体仅在私有单元测试中编译；仅服务旧延迟回放的状态、投影、cleanup 包装和无用静态 helper 已成组退出。推荐 quick start/普通内部 outlet 示例已交付，并作为独立 consumer 运行。私有替身按已核实的真实调用者和行为替代完成最终边界清理；真实 no-BSN 管线仍在，不以删除所有 ContentPlan 代码为先决条件。
3. **最终集成**：spec 验收证据、Core/ECS/public 独立与交叉审查、全量验证、main 快进合并和合并后 smoke 已完成。后续新增生产问题须有真实可达证据，才安排局部修正。

这些单元不等于固定提交次数或工时承诺。已识别的多余复杂度是旧公开内容模型与专属测试回放协议，二者已退出；不据剩余符号数启动新一轮整体重构。Core/View 分层、创建/cleanup 的真实 FrameOutput role、执行依赖保活、原 failure owner 与残留责任按调用者保留。阶段 checklist 仍按完整验收勾选，不因局部通过整阶段打勾，也不将尚未勾选误报为功能从未实现。

### 1.1 候选行为 retention map

| 候选提交 | 迁移决定 | 不能丢失的结果 / 验证落点 |
| --- | --- | --- |
| `c8a9672` | 移除 background reaper、fairness 与公开调度前置 | 保留前台 cleanup/host 仍使用的 private residual owner，直到有等价替代；terminal 与残留隔离分开 |
| `9c33bfb` | 移除 public audit DTO、投影与专属 consumer | 导航/host snapshots 仍能表达请求结果；私有必要 owner 检查不因 DTO 删除而消失 |
| `433f344` | 移除 `input_probe.rs` 和 probe 接线 | 保留 production input gate，测试迁往真实输入路径和内部回归 |
| `03d8f46` | 移除 public probe/test-support 和中间调度依赖 | 保留真实 UI/picking 正向命中、非活动拒绝、keyboard/gamepad gate 和 focus 释放 |
| `df5c7b2` | 逐块保留实际安全修复，删除只为 probe/audit 服务的同步 | 不按 `input.rs` 整文件回退；受保护 subtree 与 focus 不被误伤 |
| `0799098` | 保留关系漂移和 ownership-cycle 修复结果，允许改写私有机制 | cleanup 写入后 drift 停止破坏性操作；永久矛盾有界收敛；原 owner 不丢失；旧 epoch 不发布 |

此表不要求保留全部旧测试形状。删除旧断言前，为同一用户结果或不变量指出替代测试；只验证已延期机制的测试可以随机制删除。

### 1.2 关键文件与职责

| 文件/模块 | 本次职责 |
| --- | --- |
| `crates/bevy_ui_router_core/src/{engine,branch,request,view_input,history}.rs` 及其现有模块 | 保留决策、history/fragment/search/recovery；除新证据所需修复外不做整体重写 |
| `crates/bevy_ui_router_view/src/` | 保留 key/descriptor/只读 contract；清除旧公开 report/writer 依赖时维持独立测试 |
| `crates/bevy_ui_router_bsn/{Cargo.toml,src/lib.rs}`；可拆出 `src/factory.rs`、`src/outlet.rs` | factory、`BsnBuildError`、`DefaultOutlet`，无 ECS 依赖 |
| `crates/bevy_ui_router_ecs/src/bsn.rs`（新增） | optional App extension/注册包装、官方 Scene 执行与错误映射 |
| `crates/bevy_ui_router_ecs/src/presentation/native_scene.rs`（新增） | 声明图创建期登记、私有 wrapper apply、adoption 与同次失败结算；不替代导航/publication |
| `crates/bevy_ui_router_ecs/src/{app_ext,driver,content,presentation,frame_local,mount}.rs` | store 分派、Scene-writable root、frame-local containment、outlet binding；旧 writer 退出 |
| `crates/bevy_ui_router_ecs/src/{components,registry,cleanup,input,plugin,branch_transaction,lifecycle_preparation,navigation}.rs` | owned structure、隔离、publication、前台 cleanup/residual 与稳定调度 |
| `crates/bevy_ui_router_ecs/src/{audit,input_probe,test_support}.rs` 与 `registry/quarantine_reaper.rs` | 移除外扩 surface；只保留有真实前台用途或私有测试用途的最小部分 |
| `crates/bevy_ui_router/{Cargo.toml,src/lib.rs,tests/}` | facade feature/导出、独立 consumer、public journeys、删除旧 golden/probe 依赖 |
| `README.md`（新增）、`examples` 对应 crate 内示例、`docs/{README,current-architecture,roadmap}.md`、`AGENTS.md` | 推荐装配、事实/路线、轻量护栏；单一规范来源 |

文件拆分以实际职责和可审查性为准；以上新增私有文件名可在阶段开始时调整并同步本表，不因此另开架构需求。

## 2. 阶段 A：统一合同和可执行验证基线

**交付结果**：一份完整 spec、一份本计划；旧文档不再定义新增 MVP 范围。现有行为验证仍可运行，不把“删规范”变成“删合同”。

**涉及文件**：本 spec/plan、`AGENTS.md`、`docs/README.md`、`docs/roadmap.md`、`docs/current-architecture.md`；旧 Target/Matrix/Fixture/Allowlist/Golden/Journeys/Goals；`crates/bevy_ui_router/tests/support/facade_contract_cases.rs` 及相关 facade/dependency tests。

- [x] 完成两份独立 spec/plan 审查，交换原报告自由交叉评价；逐条核对代码与用户裁决。matcher/Back 文案与 pointer 撤权合同已修正；Scene 失败产物问题作为下项前置保留，不以审查数量或一致意见替代技术判断。
- [x] 完成 spec 第 12 节的窄可行性验证：选择官方 resolve 后的直接 ChildOf 图预分配/登记、私有 shell wrapper 和独立 attempt obligations，不预挂 staging 树。探针覆盖匿名首 child、深层部分失败、根模板前向引用失败、root/child 双向引用、用户 resolve 整体替换、正常 Children 顺序与空 Children 差异。cached Scene inclusion 在 UI 分配前拒绝；机制结论已落实到 spec/阶段 C。
- [x] 迁移基础路由、参数/search/fragment、single outlet 位置、独立 renderer 验证、错误恢复、history、host/input/focus 语义；收口复审及交叉复核无剩余合同阻断项，spec 标为 active，旧协议目录在索引中标为迁移/历史材料。
- [x] 重写 AGENTS 为长期护栏，保留身份、线性 owner、publication-last、局部 revalidation、owned-only、真实验证和共享工作区纪律；更新索引、current facts、roadmap 和历史定位。协议步骤、测试目录和已延期产品机制归 spec/历史，不按机械行数缩减。
- [x] 移除多份 Markdown/Golden 与源码互证 API/能力的 gate，包括专为 AST 闭世界扫描器维护的 mutation fixtures；替换为三类实际 Core/View/ECS 程序、六组独立 workspace 配置和六个真实权限边界拒绝点。普通导航/恢复/输入行为测试保留；旧 driver 仅为迁移 renderer，阶段 C 用真实 BSN 替代，独立验证能力长期保留。
- [x] 已修 Recovery profile 的 CRLF 回归在迁移基线和依赖迁移提交中重跑成功。无用途的 profile parser 整体退出后，不为留旧测试继续维护它；LF/CRLF 对照迁到同源 consumer TOML/Rust 写入→Cargo 运行路径，使用 Rust `Path`/`Command`，不拼接平台 shell。独立评估确认这是回归责任迁移，不是恢复 LF 假设；Windows 原生验证仍为 NOT_RUN。
- [x] 将依赖检查从精确私有 manifest/feature 列表改为语义约束：默认 facade 无 Bevy；Core/View 的 default/all-features 正常与构建依赖图均保持 renderer-neutral；官方 release crates 全部按包名选择并核对 `0.19.1` 与 crates.io。workspace/现有 consumer 共用 lock 检查；manifest 与实际 Cargo 配置检查 Bevy fork、patch、replacement 和路径覆盖，不限制普通依赖的合法配置。BSN→ECS 的现存错误方向仍在阶段 C 反转，Cargo 本身拒绝依赖环；这里不以依赖 gate 通过宣称 BSN 接线完成。
- [x] 有效合同已迁入经审查的统一 spec，旧文档 parser 引用已消除。Target/Matrix/Fixture/Allowlist/Golden/Journeys/Goals 和 Root/Nested profiles 原位归档，显式声明旧状态/清单不约束当前开发；保留路径和正文以免破坏历史引用。History/Recovery 完成记录继续作为历史证据，不以旧 catalogue 数量约束新设计。

**验证**：相关 facade/dependency/text tests；`cargo test --workspace --all-features` 作为迁移前基线，原样记录失败和环境限制。无 Rust 改动的合同提交仅要求 diff、引用和一致性检查；不能把文档审查写成 runtime PASS。

**提交边界**：统一 spec/护栏/索引为合同提交；本 plan 单独提交。测试合同迁移和历史材料删除另成可审查提交，不与 runtime 行为混提。必要决定全部迁入并提交后删除临时 `2026-09-07-mvp-reevaluation.md`，其证据可从 Git/任务历史追溯。

## 3. 阶段 B：精简候选，保持可用的独立基线

**交付结果**：不依赖 public audit/probe/reaper 的导航和输入基础；Core/View 与真实 ECS 路径仍可独立测试。

**涉及文件**：retention map 的 ECS 文件、`plugin.rs`、facade exports/features、`public_navigation_{journeys,authority_audit,input_isolation}.rs`、`test_support_features.rs`。

- [x] 按 retention map 移除公开 audit/probe、background reaper 调度、fairness 和仅服务它们的测试。先列出 private residual owner 的前台 callers，保留其实际义务，不能整块删除 `QuarantineLeaseTable` 来掩盖 outstanding ownership。
- [x] 仅公开 `RouterInputSet::{Refresh,Ready}` 和 `RouterScheduleSet::PublishSnapshots`。旧 public journey 中必须在内部暂停/注入的故障回归迁回 ECS 私有 tests；外部 consumer 只执行普通 `App::update` 和稳定入口。
- [x] 保留最小 fake renderer 驱动生产 Core/ECS 逻辑，先明确 fixture 控制的真实 build/apply 事实；不能注入 classification、terminal 或 publication 结论。旧 data-only executor 可暂为迁移工具，阶段 C/D 替代后退出，测试独立性本身长期保留。
- [x] 重跑 root/nested/reconciliation/history/recovery 的已有 public 行为与相应 ECS crate。若最高层回归失败，沿生产路径确定 cause；不得通过恢复旧 Terminal、删目标断言或增 retry 次数过关。

保留的迁移 renderer 只控制构建成功、构建/部分 apply 失败、缺失/重复 outlet、旧 session handle 与真实关系环等输入事实；实际 writer 拒绝、Core 分支分类、cleanup、publication 和请求终态继续走生产路径。它不证明官方 Scene 行为，阶段 C/D 将替换生产入口。

**验证入口**：`cargo test -p bevy_ui_router_core`、`cargo test -p bevy_ui_router_view`、`cargo test -p bevy_ui_router_ecs --all-features`、`cargo test -p bevy_ui_router --features ecs --test public_navigation_journeys`。重命名/迁移测试目标时同步命令，不固定原文件数量。

**提交边界**：先删除外扩 surface/调度与对应依赖，再提交必要私有回归迁移；每个提交保持可编译可审查，不批量 revert 六个 candidate。

## 4. 阶段 C：官方 BSN root 垂直切片

**交付结果**：独立应用从 facade 注册 owned Scene，在 app-owned host 上成功展示 root；真实 build/resolve/apply 失败清理 pending，不发布 partial UI；旧 active/input/focus 和 history 符合 spec。

**接口**：实现 spec 第 3 节的 `AppBsnExt::register_bsn_view`、`BsnSceneFactory`、`BsnBuildError` 和 `DefaultOutlet`。ECS store 按同一 `ViewDriverKey` 命名空间选择 native 或内部 fake executor，completion 仍由 ECS 消费真实执行结果后推进 Core。

**涉及文件**：BSN crate、新 `ecs/src/bsn.rs`、ECS/ facade Cargo 与 exports；`app_ext.rs`、`presentation.rs`、`frame_local.rs`、`input.rs`、`cleanup.rs`、`mount.rs`、`navigation.rs` 及对应 tests。

- [x] 先反转 BSN 的错误依赖，再添加 ECS optional `bsn` bridge 和 facade feature 联动。以真实 `Box<dyn Scene>` 单次消费编译，测试 factory 每个 effect 恰好调用一次、重复 key 拒绝与 normal/error/fallback 安装校验。
- [x] 在含 BSN binding 的 Router 安装入口检查 `AssetServer`、`Assets<ScenePatch>`；在调用前再次检查 runtime 资源。缺失分别产生 `MissingSceneRuntime` 安装错误或有来源的运行 failure，不调用缺失资源会 panic 的官方入口。
- [x] 将现有 shell/content-target 改为 Router shell + Scene root，删除 authored 内容的封闭 component/构建输出清单假设。Router 结构 identity/seal 继续 exact 校验。按 spec 第 12 节使用官方 ScenePatch load/resolve，保活 dependency handles，预检直接 ChildOf 图及 cached/reference/capacity 边界，先登记 exact allocation obligations 再写组件；匿名引用使用本 attempt 唯一身份，不能复制探针固定编号。
- [x] 将只含引用 seed 与原 resolved root 的私有 wrapper 应用到本次新建 inert shell；原 root 的引用仍指向 content target，wrapper 自身不携带它们。原模板和关系由官方 executor 按顺序执行，诊断去除最外层私有 wrapper 错误。返回同一 exclusive 调用内完成 adoption 或失败隔离，覆盖尚未挂接的 root/children；验证成功后才交接到 current containment，不延后到普通调度。
- [x] 将 Scene content traversal、input/focus scope、cleanup scope 和 mount validation 联动迁移。不能仅新增 BSN wrapper 后把 Scene 偷渡回 cloneable `ContentPlan`，也不能让一个 scope 采用 current containment 而另一个仍使用永久扁平名单。
- [x] 以 shell `Display::None` 作为非活动布局隔离基础；明确 `Refresh/Ready`、原生 UI 布局、picking dispatch 与 Router publication 的调度关系。验证 active 正向命中，并覆盖离场/host loss 后、下一次布局前的输入窗口；若缓存几何仍允许旧目标命中，先补齐最小 production 隔离机制，不能用多跑空帧掩盖缺口。
- [x] 将 authorized target 的原生 hover/previous-hover/press/click/drag/祖先缓存撤销纳入 isolation preparation，并在真实 dispatch 前过滤非活动 owned scope；保留 app/其它有效目标的输入状态。真实按下/拖拽后离场或 host loss、旧目标作为 residual 存活、继续 Move/Release 都不得再触发其 observer；不能仅测试新命中，也不能误将 `PostHover` 排在官方 `pointer_events` 前。
- [x] `RouterInputGate` 每个输入边界从当前 active frame-local Children closure 导出；publication 同步撤销旧权限、安装新权限并释放目标 focus。动态 child 添加/移出、显式 `Visibility::Visible`/自定义 `Pickable`、保留 app focus 都有真实对照；不覆盖 authored presentation 值。
- [x] 以真实 `AutoFocus` 的 staging 成功/失败对照验证原有效 focus、当前非活动 focus 与延后的 FocusGained/Lost；明确推荐路径与原生 Tab/dispatch 的接线边界，不把完整原生 Tab 支持偷偷加入 MVP。普通 typed 方法没有队列过滤接口，但官方 Reflect 可安全读取实际队列；已验证的追加候选保留原队列、合法 clear/set 及 cause，无需整体回滚或提前派发事件。按 spec §7.1 接入成功意图，显式启用所需 feature，验证固定版本 schema、真实追加来源、目标分类和跨 drain 后续操作；接入官方处理边界时保持应用初始化、排序和事件派发时序。源码/来源不匹配不能伪装成已隔离，不新增构建期 focus 禁令或全历史重放。
- [x] 所有 allocation/query/claim/cleanup/input preparation 在 `Ready` 之前完成，publication tail 只执行已准备的不可失败原位 mutation；successful presentation/history 是最后语义步骤。检查范围包括外层资源 scope 的退出：官方 Bevy 0.19.1 的 `resource_scope` 会取出并重新插入 Resource component，不能把它当作无副作用借用。三个成功发布路径已迁往 exact resource Entity 联合借用；native owner/内容边界接入后仍须验证完整 preparation 与 publication，不能将此检查点推广为其它 terminal/cleanup/no-target/snapshot 路径全部闭合。失败留原 owner，stale 丢弃而不发布。
- [x] 建立真实 BSN root consumer 和 build/resolve/apply error 回归；外部系统在 `RouterScheduleSet::PublishSnapshots` 后读取同帧 snapshot。resolve/apply 故障使用官方 Scene/template 的实际失败，不制造假的 Router report。host sentinel/关系、旧 active、history、pending cleanup、真实 partial subtree、focus 和 request failure 来源已有实际验证。

**已落实的发布迁移依赖**：无已提交 active 的 RouterFallback 在最终复验发现 host loss 时，原 Core authority settlement 因缺少 reconciliation 拒绝，错误进入协议 fail-close 而非 `HostLost`。独立评估及交叉复核后复用已消费的 failed Task source、原 HostLease 与 authority cleanup，仅为 unbound fallback 且 Core active kind 为 None 增加窄接纳分支；既有身份、Applying、action、retirement、lease 和 exact roots 校验继续复用，不伪造 reconciliation 或新增清理协议。Core 保留 Task source 的两种合法 roots 选择；当前已 realized 的 ECS caller 必须据真实 pending owner 提交包含 target 的完整 manifest，不能按物理数量反推逻辑 roots。拒绝返还原 task/lease；host loss 与 parent-only rejection 分别验证 `HostLost`、`RecoveryFailed`。完整 Core/ECS 和最高层 public/consumer 验收结果见执行记录；此窄修不代表有旧 active 的全部 projectionless host-loss、idle host loss 或其它 terminal/snapshot 资源边界已闭合。

同一回归进一步确认，无候选的空闲观察只匹配 permit 签发结果中的 `Ineligible`，会丢弃实际 `Issued` owner，令之后重新注册永久停在 `Outstanding`。本检查点复用既有 candidate rejection/pre-staging unavailable 结算处理全部返回类型，保留首次 `AwaitingHost` 的无候选语义；验证先经历无候选空闲更新再重新注册、无悬空 admission owner、旧请求不重放和新命令可用。这是既定 host/re-registration 合同的实际依赖，不通过移动注册时序绕过；不因此宣称阶段 D 的完全 idle host-loss 检测已完成。

**可行性证据，非产品 gate**：隔离临时 workspace 的官方 `0.19.1` 探针已验证 owned Scene 应用到既有 root、内部 outlet、resolve error、真实 UI layout/picking 的 shell 隔离，以及局部 pointer 缓存过滤；也复现未挂接 Node 和隐藏后缓存 Click 两个缺口。新增探针验证声明图分配、外层 wrapper、命名引用和失败产物追踪，合计 10 passed / 0 failed。此结果包含反例测试，不是 10 项产品能力通过；尚未证明 Router owner/observer drift、依赖 handle 保活、原子切换、完整手势撤销、keyboard/gamepad/focus 或 Windows。产品 tests 必须重新覆盖这些范围，不能引用临时 `/tmp` 路径作为持久依赖，也不能复制探针的直接 despawn 或计数检查作为权限证明。

**验证入口**：`cargo test -p bevy_ui_router_bsn`、`cargo test -p bevy_ui_router_ecs --all-features`、新增 facade `bsn` 集成 test 与独立 workspace consumer；涉及 lifecycle 的变化同时重跑最高层导航/恢复 journeys。

**提交边界**：依赖/合同与私有官方 Scene 解析准备可以先形成检查点；公开注册与能够成功执行的 native root realization 一起启用，避免安装接受一个只能失败的 driver。shell/current-containment/input/cleanup 的一致性作为一个联动迁移审查单元，可以分内部检查点但不能宣称仅注册成功就完成 BSN。暂留迁移 executor 必须标注用途/退出阶段，不新增兼容包装。

## 5. 阶段 D：嵌套布局、history 与恢复闭环

**交付结果**：普通带侧栏/工具栏的 layout 通过内部 default outlet 展示 child，leaf 导航保留 layout 状态；完整基础路由、history、错误恢复和 host 重新注册从推荐 facade 可达。

**涉及文件**：`bsn/src/outlet.rs`（若拆分）、ECS `mount.rs`、`presentation.rs`、`branch_transaction.rs`、`navigation.rs`、`cleanup.rs`、必要 Core 修复；facade public BSN journeys、`crates/bevy_ui_router/examples/bsn_nested.rs`（新增）、`README.md`。

- [x] 在本 frame content closure 内解析单 outlet，停止于 child slot；支持 root 与内部 Node anchor，leaf 无 outlet 合法。需要 child 时缺失/重复/非 Node/失效声明明确失败，不默认挑选、不 fallback 到 root。
- [x] 补齐已确认的 idle host-loss 缺口：所有请求终态后的 registration removal/host despawn 在普通 `App::update` 下主动失效，不依赖新请求或 cleanup-in-flight；包括 Ready 无 active 的已绑定 host。验证旧成功结果/history 不回滚、owned-only cleanup、输入/focus 撤权和重新注册。原 grant 的请求独立 settlement、成功 retained 责任、容量/public snapshot 与 staging/资源重插后的最终接纳复验已通过独立/交叉审查和全量验证；不代表尚未接线的 native Scene 已完成相同验收。
- [x] preserved parent 复验 exact anchor/slot/generation，child slot 独立于 marker；测试 layout、anchor、slot 和局部应用状态在 leaf 切换时保持。把 entering mount rejection 和 committed authority loss 分开，覆盖稀疏/保留 prefix，不能由 frame/slot 数量反推权限。
- [x] 从 BSN factory 观察实际 route-local params、raw search、fragment、origin/failure。覆盖 index/pathless/静态/params/wildcard、同优先级不同 leaf 的 `MatchConflict`、参数变化的 suffix、search 整链重建、fragment-only deepest normal frame generation 推进、等价 location 的零物理 mutation。
- [x] 完成 push/replace/back 的成功、失败 back 和串行入队结果；区分 `NoBackEntry` 同步拒绝（无 request）和入队后的 `BackTargetUnavailable`。back 在执行时选择目标，失败不改 cursor，错误 presentation 不写成功 history。
- [x] 完成最近 boundary/error view、fallback、恢复自身失败、无 binding nested Push cleanup 后 RouterFallback；分别验证原 failure continuation 和实际 cleanup scope，不按 preserved active 或 obligation 数量选择分支。
- [x] 验证 publication 前后 host loss 的不同结果、同 Entity registration 移除重插、重复候选、旧 epoch/task stale、重新注册仅接受新命令。保留最小 RouterHandle scope 回归，不扩成 multi-router stress。
- [x] 将普通嵌套示例和短小 quick start 做成实际编译/运行消费者。运行环境明确安装 Asset/Scene/UI/picking/focus 所需插件，不能依赖本 workspace dev feature 偶然统一。

**验证**：真实 BSN public journeys、ECS focused + 完整 crate、受影响 Core/View tests。故障只能通过实际生产输入或私有 seam 改变 predicate 的事实输入；最高层 consumer 不依赖 owner crate、test-support、中间 schedule、audit/probe。

**提交边界**：outlet/nested 保留语义、history/input 更新、recovery/host 组合可形成独立审查提交；具体顺序可随依赖调整，不能每个检查点重写整份 spec。

## 6. 阶段 E：清理、独立审查与集成

**交付结果**：新 MVP 推荐路径完整、旧 data-only 公开面与临时桥退出，验证证据和事实文档一致，满足条件后集成 main。

- [x] 删除不再被必要内部 fixture 使用的旧 writer/report/ContentPlan/outlet executor 及其 public 导出、兼容 consumer 和 golden。保留最小 fake renderer；删的是旧 API 形状，不是独立验证能力。

  公开面退出已通过独立审查和验证：移除两层 exports 与公开 Data 注册，将剩余单测注册收为私有 test-only 扩展；迁移 ECS-only consumer，移除旧 session 专属拒绝行、旧类型导出断言和未注册的 root 测试源。专属延迟回放 owner/投影/调度/cleanup 包装、无用静态 helper 及对应旧形状断言也已成组退出。最后按真实调用者删除六文件中的无用私有 helper、writer-only despawn 和纯源码形状测试，保留真实未知 handle、owner、claim、post-write drift 回归；完整 no-BSN ECS 行为与 native public/独立 consumer 仍通过。不要求全删 ContentPlan。
- [x] 合并重复测试，移除固定 catalogue、Markdown 循环解析、全量 hostile-source 扫描的维护前置。保留能防止 authority 暴露的少量 compile-fail 和实际 consumer 验证，不把迁移承诺变成未经证明的删除。
- [x] 对照 spec 第 10 节逐行附上实际 gate/结果；当前架构只记实现事实，roadmap 只留 Now/Next/Post-MVP，旧文档不再同时称自己 canonical。
- [x] 执行下列最终验证；独立 consumer 的 target/workspace 与产品测试隔离，同一共享 target 不并行 Cargo。

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo test --workspace --doc --all-features --locked
cargo check -p bevy_ui_router --no-default-features --locked
cargo check -p bevy_ui_router --locked
cargo check -p bevy_ui_router --no-default-features --features ecs --locked
cargo check -p bevy_ui_router --no-default-features --features bsn --locked
cargo test -p bevy_ui_router --features bsn --test bsn_facade_features --locked
```

- [x] 独立 workspace facade BSN consumer 在自身 lock 上核对官方解析图与 feature 前置，运行真实 root/nested/error；应用只依赖 facade 与实际需要的官方 Bevy crates。LF/CRLF 对照已运行；Windows 原生验证保持 NOT_RUN。
- [x] 安排 Core、ECS、public/consumer 三视角独立审查；复杂关键问题用两个代理自由交叉评价。主代理核实证据并修正，重跑受影响最高层 gate；最终 fixture 差异另经独立审查。
- [x] 仅在 spec 验收结果、独立审查与全量验证齐全后记录 MVP completion；已检查 main 和唯一 worktree，快进集成到 `7a34fcc`，合并后公开旅程 36 项及独立 facade consumer（含示例）再次通过。

**提交纪律**：仅精确 `git add <files>`，提交前检查 `git diff --cached --name-status` 和 staged diff；不包含他人文件。需要远端 push/PR 或扩大动作范围时按当前用户授权判断，不因“MVP 完成”自动发布包。

## 7. 执行记录与调整规则

本 spec/plan 已完成两份独立审查与交叉复核：matcher/Back 的公共语义简述已修正；pointer 历史撤权与 Scene 创建期义务均有明确设计处置。第三位代理只读核查公开 Scene graph/reference/wrapper 接口，主代理独立编译并运行机制探针。修订后两位再次收口审查并交换报告交叉评价，均确认无合同提交/开始实现的阻断项；owner/observer drift、dependency handle 保活、完整手势和 focus 验证继续属于阶段 C 的产品责任。

合同提交 `4bfbc4f` 仅修订 spec、工程护栏、索引、路线和当前事实，未改 runtime；双路独立复审与交叉复核 GO，7 份当前文档的本地链接检查及 staged diff 检查通过。临时探针 `cargo test --offline` 为 10 passed / 0 failed，格式检查成功。

迁移前 `cargo test --workspace --all-features --locked` 已完成，exit 0，包含 Recovery CRLF 回归、29 项 public navigation journeys、完整 Core/ECS/View 与 28 项 Core doctests。命令仍报告现有 unused/deprecation 等 warnings，未作为本轮文档工作顺手修复；Windows 原生验证未运行。此结果是旧 runtime 的迁移基线，不是生产 BSN 或新 MVP PASS。下一工作是阶段 A 的测试合同迁移与阶段 B 候选精简，保留已有行为证据。

阶段 A 的依赖测试迁移已落地：新增共享 source/version policy 和独立 dependency contract，移除 Core/View/ECS 的精确私有 manifest 清单及未注册的重复 facade 测试；尚保留旧 Markdown/Golden consumer gate，未变更 runtime。独立审查的配置覆盖、qualified replacement 与非默认 feature 三项漏检已修正，定向复审无阻断项。`cargo test -p bevy_ui_router --test dependency_contract --locked` 为 7 passed；`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，后者包含 29 项 public journeys、418 项 Core 和 620 项 ECS 单元测试及现有 consumer/CRLF 回归。现有 warnings 与 Windows NOT_RUN 保持如实记录；阶段 A 仍待真实 consumer 替换和旧文档整理。

阶段 A consumer 迁移与归档（`e0d52c4`）：上项依赖迁移已提交为 `3f8ff83`；本次删除 facade 的旧 Markdown/Golden/AST harness 后，六组真实正向 consumer 和六个单独拒绝点通过。迁移范围和新实现分别获独立只读审查；主代理按建议改用真实内部 entry/publication 入口、完整 handle 构造和 session 二次消费，不以缺字段或其它编译故障冒充权限拒绝。`cargo test -p bevy_ui_router --test facade_consumers --offline --locked`、格式检查及显式 consumer fixture 格式检查、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0。全量保留 29 项 public journeys、418 项 Core / 620 项 ECS 单元测试和 28 项 Core doctests。旧协议原位归档、当前事实/路线同步；当时记录“阶段 A 交付”，但其解析依赖清理结论有下述遗漏。Windows NOT_RUN，现有 warnings 未顺手处理；`bsn-wiring` 不证明 native Scene。

阶段 A 补漏：阶段 B 的调用者核查发现 ECS `authority_regression_catalogue` 仍解析已归档文档，之前“旧 parser 引用已消除”的范围判断不完整。独立只读核查确认其 18 项测试全部是文档/AST/源码形状扫描，没有 runtime 或独立 compile consumer；现已删除该文件及 `app_ext`/`snapshots` 两项同类 exact API shape gate、失去消费者的 ECS dev `syn`/`toml` 依赖。真实注册、snapshot retention、authority/cleanup/navigation 回归及外部正向/拒绝 consumer 保留。其它私有源码形状断言的合并仍按阶段 E 处理，不以本次补漏宣称所有静态测试已退出。

该补漏独立复核无阻断项，与阶段 B reaper 候选在同一工作区完成 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked`，均 exit 0；其中仍有 29 项 public journeys、独立 consumer 和完整 Core/ECS/View 回归。测试/依赖补漏单独提交，不含 runtime 修改；reaper 删除及其行为验证另记下一检查点。Windows NOT_RUN，现有 warnings 未顺手处理。

阶段 B reaper 检查点（本记录所在提交）：阶段 A 补漏已单独提交为 `d7155d9`。本次删除后台 reaper/fairness、专用 observation/evidence/游标和 `ReapQuarantines` 调度，保留前台 host、cleanup、presentation、input 与 stale-output 使用的 private residual owner、exact transfer 和 terminal provenance。旧 lease/fresh epoch 回归改走普通 `App::update`：旧实现会回收 claim，按新边界预期失败；删除后验证同一旧 owner/claims、Hidden/IGNORE/deny-all/三 modality 拒绝和 fresh epoch 不变。插件安装保留首次系统数量与重复安装列表一致性检查；facade 仅移除后台 GC 断言，并把 cleanup 采样点移到 Settle 后。

两位独立只读审查并交叉复核，首次安装测试缺口已补，定向复审无阻断项。补后 focused 回归、`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0；全量包括 597 项 ECS 单元测试、418 项 Core 单元测试、29 项 public journeys、独立 consumer 和 28 项 Core doctests。7 份当前文档的本地引用检查通过。Windows NOT_RUN，现有 warnings 未顺手处理。此检查点不完成阶段 B：public audit/probe/test-support 外扩面和其余公开内部调度仍待迁移，生产 BSN 与 MVP 集成尚未完成。

阶段 B surface/调度收口（`0493e48`）：reaper 检查点已提交为 `7e00b99`。本次删除公开 audit/probe DTO、计数端点、专属 projection/inventory/tracked-slot traversal、ECS/facade test-support features 和中间 public 调度标签。真正供私测使用的 back-capacity 顺序、delayed-report replay、host-admission 和 commit-seal seam 收为 `cfg(test)`；Core dev test-support 与前台 residual owner 保留。迁移关系如下：

| 原验证职责 | 当前验证落点 |
| --- | --- |
| audit consumer 中的 Back 同步拒绝、forward-tail 截断 | public journeys 的 `history_back_no_entry_is_synchronous_and_preserves_snapshots`、`history_back_then_push_truncates_forward_tail` |
| probe 计数之外的输入/focus 行为 | public nested replace 的旧目标撤权、preserved layout/new leaf 正向 gate、focus 释放；ECS 真实 picking 和生命周期输入私测保留 |
| commit seal 后 registration loss | 新增 ECS `host_registration_loss_after_commit_seal_cancels_publication_and_settles_owner`，有/无 active 两行，真实 registration 变化、取消 publication、原物理 owner 结算及严格 HostLost |
| post-commit cleanup、same-host retired edge、stale report 精确窗口 | 保留 ECS `post_commit_host_loss_enters_single_compound_without_rolling_back_history`、`same_host_readmission_waits_for_retired_edge_and_old_slot_disposition`、sealed/ownerless stale-output 与旧 lease/fresh epoch 私测；public 只验证明确 queued-loss、history 保留、re-admission/显式 restore 与 foreign 内容保护 |

普通 `App::update` 返回时本 fixture 已完成 cleanup，不能以“已观察 publication”推断仍处于内部窗口。测试迁移暴露完全 idle 的 registration loss 不启动恢复；它是既存 MVP 缺口，已列入阶段 D 和 current architecture，未通过新增前台请求冒充修复。两位独立审查及交叉复核确认上述分层迁移无剩余阶段 B 阻断项；同一编译意见合并处理，最后修订重新验证。

最终 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0；包含 582 项 ECS / 418 项 Core 单元测试、30 项 public journeys、独立 consumer 和 28 项 Core doctests。精确 seal focused 回归也通过。Windows NOT_RUN，现有 warnings 保留。阶段 B 的独立迁移基线完成；生产 BSN、idle host loss、MVP 验收和 main 集成未完成。

阶段 C 解析准备检查点（`9ee02ec`）：反转 BSN/ECS 依赖，新增官方 Scene 依赖与 optional bridge，提供 `BsnSceneFactory`、`BsnBuildError`、`DefaultOutlet` 及 facade 导出。私有 preparation 消费真实 owned Scene、检查必需 runtime、保留原引用与 ChildOf 顺序、拒绝 cached inclusion/重复引用，并准备 attempt 唯一匿名引用。resolve 与图预检失败也携带 dependencies 到后续结算；没有把 Scene 放进旧可克隆 artifact。公开注册和 native realization 尚未接入，阶段 C 各交付项仍保持未闭合。

该检查点的双路独立审查及交叉复核无阻断项；按意见补齐失败 dependency lifetime。`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，含 592 项 ECS（新增 10 项 preparation）、418 项 Core、30 项 public journeys、8 项 dependency contract、独立 consumer 和 28 项 Core doctests。另行 `cargo test -p bevy_ui_router_bsn --offline --locked` 与 facade `--no-default-features --features bsn` check 通过；BSN crate 自身尚无独立 tests，行为测试在 ECS 与 facade。facade 的官方 Scene outlet 声明测试不等于 Router native gate。7 份当前文档的本地链接检查通过，现有 warnings 保留，Windows NOT_RUN。下一依赖仍是 exact 创建期义务、wrapper apply、current-containment/input/cleanup 联动与真实 root consumer；consumer 同时补普通非 Scene asset handle 的正向用法，不能仅靠现有 ScenePatch dependency 用例代替。

阶段 C pointer 派发前撤权检查点（`09642d1`）：真实 UI/picking 测试复现旧页面离场后继续收到 Drag。修复在 gate mutation preparation 预备历史撤权目标，`Refresh` 清既有缓存，并在 `generate_hovermap` 后、`update_interactions`/`pointer_events` 前按目标过滤新命中、前次 hover、press/click/drag 与祖先缓存，复位已有 `PickingInteraction`；保留 app-owned 和其它有效目标。原子 apply 只替换已准备的 gate 数据，没有把查询/分配放进 publication tail。

两份独立审查及交叉复核无剩余阻断项。按审查修正 active 祖先误分类与 interaction 残留，并让真实回归在旧目标移除父链和 seal 后仍验证历史撤权；它同时验证有效祖先与独立 app-owned 交互保持。最终 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，包含 593 项 ECS、418 项 Core、30 项 public journeys、独立 consumer 与 28 项 Core doctests；focused 原生手势回归也通过。现有 warnings 保留，Windows NOT_RUN。

本检查点不闭合阶段 C 的 pointer 整项：publication 同一 exclusive 调用内直接清 Bevy 缓存、native current-containment 目标、Scene failure/AutoFocus 及 host-loss 接线仍在后续。下一实现继续声明图 exact 创建义务与 wrapper apply，同步迁移 adoption/input/cleanup；生产路径核查确认可复用 exact registry retirement 和局部 cleanup 证据，但不能直接复用旧 closed manifest 或永久输出名单。相关映射仅为临时实现依据，未记作 native apply 验收。

阶段 C 私有 native 创建检查点（本记录所在提交）：新增批量 registration/seal 预备与完整声明图先登记，传播解析 attempt，使用官方引用 seed/wrapper apply；成功后校验 live adoption、逐项释放 descendant 临时标记并退役 exact claims，返回最终真实闭包。失败结算区分 exact 叶节点删除与 protected 残留，保留 authored 组件值；写后复验全体剩余目标、关系和 shell 隔离。独立审查发现的合法声明/普通容器交错误判、遗漏 live quarantine/unresolved owner，以及 marker hook 后闭包/结算问题已修正。普通 Image 的成功/失败 dependency handles 也由实际 preparation 路径验证。

本次新增 13 项 native 私测覆盖双向引用与首写前完整登记、部分 Node 写入失败及原错误深度、合法交错拓扑、真实 hook 漂移、普通组件正向、既存 residual owner 保护和 marker-release 后结果；三项 registry batch 测试另验证完整容量、owner 冲突与 seal 永不复用。用例必须先建立真实触发窗口：官方 scratch 写入会替换提前附加的 Children，hook 不能在正常 root 挂接时提前触发；Entity 活数量使用 `count_spawned`，不是索引容量 `len`。

修正后两名独立代理复审并交叉评价，无剩余私有检查点阻断项；主代理核实对应生产路径与实际测试。定向 native 验证为 13 passed；`cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked` 均 exit 0，包含 609 项 ECS、418 项 Core、30 项 public journeys、8 项 dependency contract、独立 consumer 和 28 项 Core doctests。registry batch 另有 no-default-features 定向与完整 ECS 验证。现有 warnings 保留，Windows NOT_RUN。公开注册、导航/前台 owner、mount、输入/focus、统一 cleanup 及最高层 BSN consumer 均未闭合，`Unisolated` 仅为错误证据而非已隔离结果；下一步接入这些相互依赖的 production consumers，阶段 C 各产品交付项不因此打勾。

阶段 C pointer 同调用撤权检查点（本记录所在提交）：三个正常 publication consumer 与 authority isolation 均直接清理已授权目标的原生缓存和已有 interaction。新增资源借用采用 exact resource Entity 联合 query，不使用会 take/reinsert 的 `resource_scope`；默认禁用目标/资源仍可撤销历史输入，但正常 presentation 的查询资格不放宽。post-commit 原观察 partition 收窄为 owned obligations，并以实际历史撤权及当前 exact claims 授权 native 写入，保留 protected/app 状态。真实双 pointer 管线、Replace/Back/fallback/host-loss consumer、拒绝准备、无 gate、禁用实体和无操作 resource tick/observer 对照均由生产路径或真实输入事实验证。

两名独立代理复审并交叉评价，无剩余本检查点阻断，主代理核实了生产路径与运行输出。最终 all-features 与 no-default-features pointer 定向各为 7 passed；fmt、workspace all-targets/all-features check 和 workspace all-features test 均 exit 0，包含 615 项 ECS、418 项 Core、30 项 public journeys、独立 consumer 和 28 项 Core doctests。既有 warnings 保留，Windows NOT_RUN。此结果不闭合 native current-containment、Scene focus、idle host loss，或旧 gate/focus/registry 资源 scope 的整个 publication 边界；这些义务继续保留在阶段 C/D，不因 pointer 检查点通过而打勾。spec §7 创建总览同时校准为 §12 已选定的 factory/resolve/预检后才分配 UI 的顺序。

阶段 C 常驻资源 publication 检查点（2026-09-12，本记录所在提交）：normal reconciliation、Back target、projectionless fallback 在 World preparation 及外层资源 scope 退出后重新验证原来源，最终借用 exact resource Entity 上的常驻资源。原 active/cleanup owner 在 Core publication 前安装，发布后不再重新插入资源或补装 owner；registry index plan 在可变准备后重建。保留真实 Core commit seal 后 registration loss 的回归窗口，不用更早的失败替代。初始无 active fallback 的窄 HostLost 接纳及无候选 Issued permit 丢失修复一并落地，保持原 task/lease、完整物理义务和重新注册语义。

独立审查与交叉复核发现并修正 Back 最终 focus 来源漏验，以及等价 gate 因顺序改变而误写 tick。完整来源包含 leaving 与 RetainEmpty，来源漂移拒绝，单纯 focus 值变化仍允许发布。进一步用普通 `App::update` 复现了测试第二次手动 commit 曾掩盖的调度缺口：原 active 暂存于 splice，下一 Refresh 误撤 gate，后续 fallback 误变 AuthorityLost。最终在同调用复用原 pending-rejection consumer 归还 active；测试不再额外手动推进，验证原 entering owner、Apply/fallback、history 保持、app focus 与前台 owner 清空。最终两位原代理分别定向复审并交叉评价，无剩余本检查点阻断；主代理核实了生产路径、修复增量和实际命令输出。

最终冻结源码的 `cargo fmt --all -- --check`、`cargo check --workspace --all-targets --all-features --locked`、`cargo test --workspace --all-features --locked`、`cargo test --workspace --doc --all-features --locked` 均 exit 0。全量包含 420 项 Core、627 项 ECS、30 项 public journeys、独立 facade consumer、8 项 dependency contract、12 项 Core compile-fail 与 28 项 Core doctests。另行完整 ECS `--no-default-features` 为 604 项单元测试及 public API 通过；阶段 E 列明的 facade minimal/default/ecs/bsn 四组 check 与 `bsn_facade_features` test 均 exit 0。定向 Back focus 3 项、publication 21 项（含 RetainEmpty/资源 observer/初始 fallback）、gate 13 项、真实 pointer consumer 7 项均通过。既有 warnings 保留，Windows 原生验证 NOT_RUN。此结果不是完整 BSN 或 MVP 验收：下一依赖仍是 native store/执行与 RealizedFrame owner 转移、current containment/mount/input/focus/cleanup 的联动、真实 facade consumer，以及阶段 D/E 的其余闭环与 main 集成。

阶段 D idle host-loss 检查点（2026-09-21，本记录所在提交）：以真实 retired host grant 建立请求独立 settlement，覆盖已终态 active 与 Ready-empty、registration removal/despawn/同 Entity 重插，保留旧请求/history并仅接受新命令。容量错误保留 retry 和原 owner；receipt 完成与当前残留隔离分开；cleanup snapshot 反映实际 holder/receipt。重新接纳将唯一 pending owner 跨 staging/activation 和三层资源重插传递，最后复验后常驻确认；真实 retained 拒绝原因与新 slot 清理分开。只接纳三个 exact resource carrier 的预期重插，按每个 Router 完整操作依次确认，未引入全局事务或 reaper。

两位原审查代理完成独立增量复审与交叉复核，无剩余本任务阻断；主代理核实生产路径和真实日志。17 项 idle 定向和既有跨 Router 对照通过。最终冻结源码的 workspace all-targets/all-features check、完整 workspace test（含 422 Core、645 ECS、35 public journeys、consumer/compile-fail/View 与 doctest）、workspace doctest、完整 ECS no-default（622 单元测试）、四组 facade feature check、BSN feature test 和 fmt 均 exit 0；既有 warnings 保留，Windows NOT_RUN。这是旧/独立 renderer 路径上的 host 恢复交付，不是 native BSN 或 MVP 完成；下一工作回到阶段 C 联动接线。

同期 focus 可行性核查纠正了工程预设：Rust 字段私有不等于官方 Reflect 不可用。独立评估及交叉核对后，固定官方 0.19.1 的独立探针以显式 Reflect feature 读取实际操作后缀，用原 Clone 和公开 set/clear 保存原队列及其它有效目标，4 项测试通过，含真实 Scene AutoFocus 后成功/模板失败与延后事件。它仅证明追加来源下的局部机制；schema/source 不符明确拒绝而非伪造隔离。尚未接入 Router，不改变成功 AutoFocus 策略或扩大任意资源回滚合同，也不再预设必须牺牲合法构建期 focus 行为。

阶段 C native 联动检查点（2026-09-22，本记录所在提交；Core 前置为 `e7445a2`）：公开 factory 与真实 FrameTask 接通，原 adoption/失败产物沿既有 owner 和 Back/cleanup/terminal 接收端移动。physical anchor 与 authority source 分开，普通 authored 内容按 current containment；输入/focus 共享当前资格但保留原 source-frame 归属证明。官方 clipping 缓存采用统一观测，不再产生散落挂载豁免。真实 AutoFocus 时序、动态隐藏、移入 preserved 内容、HostLost/重接纳与当前 typed cleanup 证据均有生产路径回归。两位独立审查者完成交叉及增量复核，无本检查点剩余 finding。

最终冻结版本的 workspace all-target/all-feature check、完整 workspace test、workspace doctest、facade core/default/ecs/bsn checks、fmt/diff 均以 `--locked --offline`（适用命令）运行并 exit 0；包括 424 Core、663 ECS、19 focus、13 input、35 public journeys、独立 LF/CRLF consumer 与 28 Core doctests。完整 ECS no-BSN 为 624+1 PASS；最后的 BSN-only source-membership 修正未改其共享源码。既有 warnings 保留，Windows NOT_RUN。下一步按现有 spec 核对剩余组合和公共观察边界，推进示例/旧入口退出；不得以本记录代替最终 MVP/main 验收。

公开 snapshot 观察边界检查点（2026-09-22，本记录所在提交）：在原独立 BSN consumer 中添加普通 `Update` 应用系统，仅按公开 `PublishSnapshots` 标签排序，逐次要求新观察并对照本轮更新后的完整 request/router snapshot。将同一时刻的真实 native Entity 与输入授权作为独立发布证据，避免延后一轮的旧快照仍靠 polling 通过；既有嵌套 Push/Replace/Back、实际 Apply 失败、Error presentation 和 history 保持都经此边界观察。无生产代码或公开 API 修改。

实现与独立范围审查通过；主代理再次执行 `cargo test -p bevy_ui_router --test facade_consumers --locked --offline`（含独立 LF/CRLF BSN）、workspace fmt、consumer 显式 rustfmt check 和 diff check，均 exit 0。本次 test-only 改动不重复宣称全量 workspace runtime 验收，已有 warnings 和 Windows NOT_RUN 保留。阶段 C 最后一项的同更新观察部分已闭合，build/resolve 公开恢复组合仍随下一 native public journey 迁移完成，故不将整项提前勾选。

native 公共旅程检查点（2026-09-22，本记录所在提交）：原 fixture 已迁往真实官方 BSN，覆盖 factory input/location 更新、内部 outlet/局部状态保持、history/Back、真实 build/resolve/部分 apply 与恢复、host loss/readmission。替换的是旧公开 writer 模型和固定物理计数断言，不是既有行为责任；专属 session/cycle 输入的必要私有回归保留。实际迁移发现并修正两处 native 接收缺口：缺失根不再触发新内容获取，但保留原 cleanup 义务；零分配失败仍保留与原 task/current outlet 绑定的 authority-loss 证据，复用现有 settlement。已分配 child 的同类 native 边界也经原 exact failed-source 接收器验证。Core 与公开 API 未改，RecoveryFailed 的两份来源由同一请求的公开 diagnostics 验证，不强加摘要字符串格式。

两份独立审查及交叉复核无剩余问题。最终冻结源码的 public journeys 为 36 passed，完整 ECS 为 668 + 1，完整 facade（含 focus/input 与 LF/CRLF 独立 consumer）通过；workspace all-target/all-feature check、fmt 与 diff check 通过。主代理另行执行完整 workspace all-feature tests（含 424 Core、668 ECS、36 public journeys、28 Core doctests）及 ECS no-default-features（624 + 1），均 exit 0。现有 warnings 和 Windows NOT_RUN 保留。上述勾选只对应已有证据的交付项，其余已实现联动及 matcher/host 私有证据在最终 spec 验收归集时核对，不因尚未归集而重复开发；示例、旧公开入口/回放机制退出、最终审查和 main 集成仍开放。

阶段 E 公开入口退出（本记录所在提交）：两位独立评估及双向交叉复核支持按现有范围做减法，无需整体重写 Core/View 或增加安全需求。已撤旧公开 writer/report 与 Data 注册，保留 test-only 私有注册和同一真实执行管线；ECS-only 外部 consumer 只验证安装/资源/更新，不冒称渲染。删除未注册的旧 root 测试源、session 专属拒绝行和旧类型导出断言；原 native journeys、输入/focus、身份/authority 拒绝保留。该冻结改动经独立 spec/quality 审查无阻断项，主代理核对实际调用者、日志和哈希。

本检查点验证均 exit 0：完整 ECS all-features（668 + 1）和 no-default（624 + 1）、完整 facade（85，含 native journeys/focus/input、独立 LF/CRLF consumer）、workspace all-target/all-feature check、四 facade feature checks、格式与 diff。主代理另行重跑完整 workspace all-feature tests（含 Core 424、ECS 668、public journeys 36、Core doctests 28）与 workspace/consumer 格式检查。现有及公开面撤销后显现的 unused 私有协议 warnings 如实保留，Windows 原生仍 NOT_RUN。下一步是成组退出 replay/static helper 并收窄必要替身、完成示例，随后最终验收与 main 集成；不是本检查点即完成全部旧机制或 MVP。

阶段 E 专属回放退出（本记录所在提交）：在 `0d8cb48` 基线上，成组删除只由测试捕获入口驱动的旧输出 owner、延迟释放状态、全 store/snapshot 投影、调度和 cleanup 专属包装，并删除无消费者的静态 helper、四项形状断言和三项旧协议专属测试。共享 cleanup 中移除的回调在所有剩余调用者均为常真；真实 partition/refreeze、失败来源和原 owner 保留。stale 零写入、原 task 重试、同 host 重新接纳、旧残留隔离与 hook 后停止写入仍由生产路径回归覆盖。两位独立审查者完成双向交叉复核，无可行动 finding；主代理核实调用者、测试责任与最后 import-only 变更。

本检查点完整 ECS all-features（661 + 1）、no-default（617 + 1）、完整 facade（85，含 36 native journeys、19 focus、13 input 和独立 LF/CRLF consumer）、workspace all-target/all-feature check 均 exit 0。主代理另行重跑 `cargo test --workspace --all-features --locked --offline`，含 Core 424、ECS 661 与 Core doctests 28，exit 0；workspace/独立 consumer 格式检查及 diff 检查通过，产品冻结哈希一致。既有 warnings 与 Windows 原生 NOT_RUN 不变。下一交付是推荐 quick start/普通内部 outlet 示例及最后 fixture 边界核查，随后归集完整验收、最终审查验证和 main 集成；本次不新增产品需求或通用事务框架。

阶段 D 示例交付（2026-09-22，本记录所在提交）：新增 README、短小 BSN quick start 与普通内部 outlet 嵌套示例。后者以真实 UI 布局执行 Push overview → Push settings → Replace help → Back overview，验证 history、旧页面销毁及 layout/sidebar/toolbar/slot 的 exact Entity 和状态保持。现有独立 consumer harness 直接编译运行这两份源文件，显式配置官方依赖与插件，不复制第二份示例实现、不增加源码/Markdown 解析 gate。示例是有限 headless 程序，不承诺窗口/GPU、真实设备输入或自动 Tab/dispatch 集成。

独立 spec/quality 审查无可行动 finding，主代理核对冻结文件与实际输出。两个示例运行、完整 facade（85，含独立 LF/CRLF consumer）、workspace all-target/all-feature check 均 exit 0；主代理另行执行完整 `cargo test --workspace --all-features --locked --offline`，含 Core 424、ECS 661、public journeys 36、Core doctests 28，exit 0。主代理 workspace fmt、consumer/示例显式 rustfmt、diff 与冻结哈希检查通过。既有 warnings 保留，最低 Rust 版本执行、Windows 原生及窗口/GPU 验证 NOT_RUN。下一步先确认最终 Core 审查的两个具体候选，再完成有界 fixture 清理、最终交叉审查与 main 集成；本提交不是 MVP 验收完成。

最终审查修正（2026-09-22，本记录所在提交，前置 `a45cbc2`）：真实运行确认并修正三处旧来源假设。Router 混合诊断现在按实际保留事件关联去重记录，live/idle/prepared/refresh 共用有限追加与淘汰；发布后的请求结果来自原 attempt，error presentation 保留原失败而非改为成功；native authority isolation 保留 Scene authored 内容观察，仅更新真实隔离写集。完整 refreeze、protected partition 和线性 owner 不放宽。实际混合日志与 prepare/refresh 回归通过；正常、fallback、bound-error 的真实 publication 后 host loss 通过，并在一次普通更新后保持公开快照与单次 terminal/diagnostic。恢复旧 Core 判定时两个错误行明确失败，正常行仍通过。

Core/ECS/public 三视角独立审查及交叉已完成，修正经 Core/ECS 冻结增量复审通过；下一普通更新的补证也已复审。生产冻结版本完整 Core（428 单测及 integration/28 doctests）、ECS all-features（665 + 1）/no-default（617 + 1）、完整 facade（含 36 native journeys、19 focus、13 input、独立 LF/CRLF consumer 与示例）、workspace all-target/all-feature check 均 exit 0；主代理完整 workspace all-features test exit 0。最后仅扩展原测试尾部，实施者及主代理定向三项均通过，fmt/diff 与冻结哈希核对通过，不冒称再次运行全部 suite。按独立证据归集核对，阶段 C/D 剩余项现已勾选；阶段 E 的有界 fixture 清理、最终同版本门禁及 main 集成仍开放。既有 warnings 与 Windows/最低 Rust/GPU/硬件 NOT_RUN 保留。

最终私有 fixture 减法（2026-09-23）：在 `e35a832` 上按真实调用者和测试责任删除六文件中的无用 helper、writer-only despawn、纯源码形状检查，共 9 行新增、1035 行删除；真实未知 handle、三个故障的写前阻断、terminal owner、registry seal/claim 与写后漂移回归保留。独立 spec/quality 差异审查无 finding。实施者完整 ECS all-features（660 + 1）/no-default（612 + 1）、facade（85）及 workspace check 均 exit 0。主代理按六文件冻结哈希重跑最终同版本门禁：workspace fmt、显式 consumer/示例格式、all-target/all-feature check、workspace all-feature tests（Core 428、ECS 660、public journeys 36、Core compile-fail 12、独立 LF/CRLF consumer、Core doctests 28）、独立 doctests、facade 四 feature 组合与 BSN feature test、diff check 全部 exit 0。首次全量运行因临时构建空间耗尽而失败，环境恢复后同一源码完整重跑通过；失败未被记作产品通过。现有 warnings 仍在，Windows 原生、最低 Rust 执行、GPU/窗口及真实硬件输入 NOT_RUN。下一步为 main 集成和合并后 smoke。

本地集成（2026-09-23）：最终私有 fixture 减法与验收记录提交为 `7a34fcc`。检查当前分支工作区干净、`main=c67aa318`、merge-base 相同且只有一个 checkout 后，执行 `git switch main` 与 `git merge --ff-only sys/structural-governance`，本地 main 快进到 `7a34fcc`。合并后 `cargo test -p bevy_ui_router --all-features --test public_navigation_journeys --test facade_consumers --locked --offline` exit 0，36 项公开旅程和独立 facade consumer（含原样 quick start、嵌套示例及 LF/CRLF 对照）均通过。验收范围为已运行的 Linux/headless 环境；现有 warnings 保留，Windows 原生、最低 Rust 执行、GPU/窗口及真实硬件输入 NOT_RUN。未执行远端 push 或包发布。

每个阶段结束只补充：交付 commit、实际命令与结果、审查结论/未决项、下一依赖；临时构建路径、完整日志和细碎排查过程不固化成长期经验。阶段内私有文件布局和检查点可调整，影响公共语义、范围或信任边界才需要重新确认。

复用经验按本项目范围保存：无兼容义务不等于取消架构边界；单 outlet 限制数量而非位置；真实 input 测试先证明正向管线；失败 continuation 不由 cleanup 物理结果反推；已验证实现事实与设计/探针证据分开。不要将其它项目记忆并入本项目。
