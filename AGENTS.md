# AGENTS.md — Bevy UI Router

## 1. 适用范围

本文件适用于通用 Bevy UI Router 的协作开发。

具体设计以已确认的需求、设计文档或公共 API 约定为准。

实现应保持 Router 的通用性、可组合性和边界清晰。不要引入特定应用、业务场景、品牌、数据服务或宿主工程假设。

## 2. 优先级

1. 明确的用户任务和已确认设计优先。
2. 本文件约束优先于临时实现偏好。
3. 现有公共 API 和已发布语义优先于内部实现便利。

只有当冲突会影响范围、安全边界、生命周期语义或公共 API 时，才需要先澄清；小的实现细节应按现有约束直接推进。

## 3. 核心硬约束

- Router 只管理 router-owned subtree，不能销毁、替换或无理由重建 app-owned host。
- core 不绑定具体平台、主题、文案、权限、数据服务或应用 shell 模型。
- 平台、工具或高级集成能力必须通过 adapter 或明确设计决策接入，不能假设为 core 默认能力。
- frame/root 身份与 generation 必须可追踪，至少能关联到 Router、host、slot、frame identity 和 generation。
- 用于 admission 或 authority 的 revision 必须绑定显式实例身份，不能只依赖 change tick、查询时序或可复用数值
- ownership seal 和 authority ID 一经消费，不得因失败、删除或 quarantine 而复用；registry 变更必须校验当前 exact Entity、owner、seal 和预期 lifecycle
- lifecycle 或角色转换必须分别冻结并校验 current 与 next；不得从提交后的结构反推 current，`next=None` 是合法 no-op，只参与校验，不得进入 mutation plan
- retained-empty、slot-only 或其他稀疏拓扑不得用 frame/slot 数量关系或数组位置推导 authority、ownership 或 cleanup obligation；必须使用 parent、outlet、slot identity、mount binding 和 exact claim；允许 lifecycle 必须按 Entity 自身 role 与 binding 分别判定，不能取关联 frame/slot 的宽泛并集
- Router-created Entity 必须先登记 ownership，再写 component 或 relationship；可能触发 hook/observer 的写入完成后必须复验，drift 时保持 inert 并 fail closed
- authority preflight 必须用携带原 owner 的类型化结果区分 ready、pending rejection、authority loss 和可重试准备失败，不能用同一个空值或 retry 分支混合不同生命周期语义
- 可见状态切换必须在 publication 前完成全部校验、分配、排序、owner 组合、claim rebind 和 cleanup reservation；publication 是最后语义步骤，执行阶段只允许预备好的不可失败原位 mutation，返回后不得再安装 owner 或执行可失败工作
- lifecycle capability 必须线性持有并单次移动或消费
- snapshot、fingerprint 与 projection 只能校验，不能重建 owner
- 拒绝路径必须返还原 owner
- 延迟执行或 one-shot lifecycle action 必须冻结完整 source projection，消费前从当前 authority 重建并要求 exact equality，逐元素仍有效不足以授权 mutation 或 publication
- boundary evidence 在 freeze、classify、settle 间不得降为 Entity、bool、空值或通用错误，必须保留 source、exact expectation、observed state、关联 obligation 与 owner
- 只有可证明未来可完成的容量或前置事实不足可以 retry，永久矛盾必须有界 fail closed
- lifecycle mutation/focus set 必须来自完整 observed relationship closure 和显式 role classification
- exact entering、leaving、pending cleanup 或 authority-invalid current 可以按 active plan 成为 authorized target
- role classification 先确定 authorized target，剩余 preserved、non-target current、app-owned、foreign 与 other-authority non-target closure 必须保持为 protected，non-target current 只包含 authorized target role 之外的 current Entity
- authorized target closure 必须与每个 non-target protected closure 不相交，且 mutation 只能落在 allowed write set 内
- 可能触发 hook/observer 的 staging、cleanup 或 recovery 写入后，必须重建并复验 target/protected partition
- failure cleanup 的 target/obligation manifest 与 completion 后 lifecycle continuation 必须独立冻结和校验；continuation 只来自原始 failure/authority、attempt 与 branch 分类，不能由 preserved active、obligation 数量或物理 disposition 反推
- atomic publication tail 不得包含会触发 hook/observer 的写入、query 或可失败 validation
- 不持有 `&mut World` 跨 `await`。
- 异步结果 apply 前必须执行 stale check；身份不匹配必须丢弃，不能产生可见状态变化。
- pending scene 不能视为 active UI；active UI 只能来自已完成 resolve/apply 且通过身份校验的结果。
- resolve/apply 失败必须清理 router-owned 局部状态，并通过明确错误通道表达失败。
- 失败清理不能触碰 app-owned host。
- 一旦实现 hidden/retained subtree，就必须隔离输入并释放 focus；该 subtree 不能继续接收 pointer、keyboard、gamepad input 或保留 active focus。
- 不依赖 Bevy query 未定义顺序；需要稳定顺序时使用明确 key、index 或排序规则。
- scene-local observer 可以处理局部 UI 行为，但不能成为 route lifecycle 的权威状态源。
- adapter 只能通过公开 API 影响 Router，不能伪造 generation/frame identity，不能绕过 lifecycle 直接切换 active frame。
- 外部声明、加载或增量更新机制可以作为 integration layer 研究或接入，但不能成为 routing state source。
- 未看到命令成功输出，不能声称验证通过。

## 4. 实现建议

- 保留 common-prefix frame 可作为默认策略建议；是否 remount 应由明确的 key、index、参数或策略决定。
- 优先让 Router core 只接收抽象后的命令、参数和策略，把平台事件、宿主环境和工具集成放在 adapter 层。
- 使用 Bevy relationship API 做 ownership check 和 hierarchy traversal，避免把临时调试遍历固化为 runtime API。
- 将异步任务携带的数据限制为可安全移动的句柄、快照或请求参数，World mutation 留在 apply 阶段。
- 局部状态更新应尽量精确；不要用整 host 重建规避 ownership、focus 或 lifecycle 问题。
- 错误处理应区分 resolve 失败、apply 失败、stale 结果和 adapter 失败，避免伪造成成功状态。
- 对外 API 保持小而稳定；内部 generation、frame identity 和 lifecycle 细节不应暴露给 adapter 伪造。

## 5. 非目标

- 本文件不是架构说明、设计规格、路线图、测试矩阵或发布流程。
- Router core 不是平台框架、应用 shell、主题系统、文案系统、权限系统或数据服务层。
- 不因为未来可能需要某项集成能力，就提前实现未被需求或设计决策覆盖的功能。
- 不在本文件中维护完整类型模型、错误枚举、路由元数据清单或能力对照表。
- 不把特定声明格式、资源加载方式、开发工具链或宿主环境写成 core 默认行为。

## 6. 验证要求

- 按变更触及区域选择验证，不维护与当前变更无关的长测试矩阵。
- 只要涉及 Rust 代码，至少运行相关格式检查、静态检查或说明未运行原因。
- 涉及 ownership 时，验证 Router、host、slot、frame identity 和 generation 标记完整。
- 涉及 admission、registry 或 staging 时，验证同 tick identity 变化、历史 seal/stale claim 拒绝和 post-write observer drift 不会越权写入
- 涉及 lifecycle 或角色转换时，验证 current 与 next 独立绑定，且 no-op 不产生 presentation、registry、cleanup 或其他物理 mutation
- 涉及 retained-empty、slot-only 或稀疏分支时，验证 exact binding 与各 Entity 自身 role 的允许 lifecycle，不以集合数量或数组位置代替拓扑事实，也不共享关联 frame/slot 的 lifecycle 联合集合
- 涉及 authority preflight 时，验证各类结果保持独立语义并返还原 owner，永久 drift 不会退化为无界 retry
- 涉及 lifecycle capability 或 terminal evidence 时，验证拒绝、重试与 completion error 后 capability 仍唯一可追踪，terminal 后同一 scope 不再持有 outstanding action、ticket、capability 或 lifecycle owner
- 涉及延迟 action 时，验证消费时对完整 source projection exact refreeze
- 涉及 boundary evidence 时，验证永久 drift 不进入无界 retry，typed evidence 与 owner 不在 helper 边界丢失
- 涉及 isolation、cleanup 或 recovery 时，验证 lifecycle mutation/focus set 来自完整 observed relationship closure 和显式 role classification，authorized target 与 non-target protected partition 正确且 mutation 保持在 allowed write set 内
- 涉及可能触发 hook/observer 的 staging、cleanup 或 recovery 写入时，验证每次写入后重建并复验 target/protected partition
- 涉及 failure cleanup 或 continuation 时，分别验证 cleanup scope 与 settlement 后 lifecycle branch，覆盖 preserved active 与 zero/nonzero obligations
- 涉及 lifecycle、failure 或 terminal 分类变化时，除 focused 与完整 crate 验证外，必须重跑受影响且已闭合的最高层 public journey、facade 或 consumer gate；局部分类测试不得改写既有公共合同
- 涉及 test-only fault seam 时，seam 只能改变生产 predicate 的真实输入事实；测试必须经该 predicate 导出结果，并在移除或短路该判定时失效，不能直接注入 classification、terminal 或 lifecycle 结果
- 涉及 atomic publication 时，验证 `Ready` 前完成全部 preparation，atomic publication tail 没有 hook/observer-triggering write、query、可失败 validation 或结构写入
- 涉及异步逻辑时，验证 stale check 覆盖 router、frame、generation 和 task 身份。
- 涉及 hidden/retained subtree 时，验证输入隔离与 focus 释放。
- 涉及 resolve/apply 时，验证失败会清理 router-owned 局部状态，且不触碰 app-owned host。
- 涉及排序或匹配时，验证没有依赖未定义的 query iteration order。
- 涉及 adapter 时，验证 adapter 只通过公开 API 影响 Router。
- 文档变更应检查是否仍是轻量护栏，且没有残留特定项目、路径、品牌或业务上下文。
- 无法运行的验证项必须在最终报告中明确列出。

## 7. 共享工作区提交纪律

- 多代理直接在同一工作区协作时，同一时刻只能有一个明确的 index/提交负责人；负责人提交期间，其他代理不得暂存或提交。
- 只用精确文件列表执行 `git add`，禁止在共享工作区使用 `git add -A`、`git add .` 或隐式包含他人文件的提交命令。
- 提交前必须检查 staged 文件集合；发现职责外文件时停止提交并交还负责人处理，不能顺手纳入、回退或改写他人变更。
