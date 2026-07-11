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
