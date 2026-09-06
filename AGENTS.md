# AGENTS.md — Bevy UI Router

## 1. 定位与优先级

本文件是通用 Bevy UI Router 的长期协作护栏，不是第二份架构、类型协议、测试矩阵或路线图。当前设计与执行入口见 [文档索引](docs/README.md)。

1. 明确的用户任务和已确认设计优先；“不用兼容”取消旧 API 形状的兼容义务，不取消已有用户行为、通用性和架构边界。
2. 本文件优先于临时实现偏好；现有公共语义不能因内部实现便利被静默改变。
3. 只有范围、信任边界、生命周期或公共 API 的实质冲突需要先澄清；已授权范围内的小实现选择直接推进。

不引入特定业务、品牌、数据服务、主题、权限或应用 shell 假设。平台和声明/加载机制经 adapter 接入，不能成为 core 默认能力或第二个 routing state source。可信应用代码不是 hostile World 沙箱；不要为未成立的威胁扩张需求。

## 2. 架构与 ownership 护栏

- Core 独占路由、请求、history 与逻辑 lifecycle 决策；View 保持 renderer-neutral。ECS 拥有物理执行与 cleanup，adapter 只消费受控合同，不伪造 frame/generation、owner 或 publication。
- Router 只管理 router-owned 结构和设计明确授权的内容边界，不销毁、替换或无理由重建 app-owned host，不回滚任意应用资源副作用。scene-local observer 不是 lifecycle 权威。
- 身份至少关联 Router、host、slot、frame identity 与 generation；异步 apply 还要核对 task。admission/authority revision 绑定显式实例身份，不以 change tick、查询时序或可复用数值代替。
- ownership seal 与 authority ID 一经消费，不因失败、删除或 quarantine 而复用；registry mutation 校验 exact Entity、owner、seal 和预期 lifecycle。Router-created Entity 先登记 ownership，再写 component/relationship；可能触发 hook/observer 的写入后复验，drift 时保持 inert 并 fail closed。
- lifecycle capability 线性持有并单次移动或消费。拒绝、重试与 completion error 返还原 owner；snapshot、fingerprint、projection 只用于校验，不能重建 owner。terminal 后同一前台 scope 不留下 outstanding action、ticket 或 lifecycle capability；私有残留隔离责任与前台 owner 分开。
- current 与 next 独立冻结和校验，不从提交后结构反推 current。`next=None` 是只校验的 no-op，不进入任何物理 mutation plan。
- 稀疏拓扑按 parent、outlet、slot identity、mount binding 和 exact claim 判定；不按 frame/slot 数量或数组位置推导 authority、ownership 或 cleanup obligation。允许 lifecycle 按 Entity 自身 role/binding 判断，不取关联 frame/slot 的宽泛并集。
- 延迟/one-shot action 冻结完整 source projection；消费前从当前 authority 重建并要求 exact equality，逐元素有效不足以授权 mutation 或 publication。
- preflight 的 ready、pending rejection、authority loss、可重试准备失败保持独立类型语义和原 owner。boundary evidence 跨 freeze/classify/settle 保留 source、exact expectation、observed state、obligation 与 owner，不降为 bool、空值或通用错误。
- 只有可证明未来能完成的容量或前置事实不足可以 retry；永久矛盾必须有界 fail closed，不能用增加重试次数掩盖。

## 3. Mutation、publication 与输入

- lifecycle mutation/focus set 来自完整 observed relationship closure 和显式 role classification。active plan 可以授权 exact entering、leaving、pending cleanup 或 authority-invalid current。
- 先确定 authorized target；其余 preserved、non-target current、app-owned、foreign 与 other-authority non-target closure 保持 protected。target 与每个 protected closure 不相交，mutation 仅落在 allowed write set。
- 只有证明目的、accepted role partition 和 failure 语义相同时才复用验证 primitive。只读 preservation/zero-target proof 不得套用 mutation-oriented closed-world child-set 假设排除合法 protected extras。
- staging、cleanup、recovery 的 hook/observer-triggering 写入后重建并复验 target/protected partition；不能以旧快照继续破坏性操作。
- failure cleanup 的 target/obligation manifest 与 completion 后 continuation 分别冻结和校验。continuation 来自原 failure/authority、attempt 和 branch 分类，不由 preserved active、obligation 数量或物理 disposition 反推。
- publication 是最后语义步骤。此前完成全部校验、分配、排序、owner 组合、claim rebind、cleanup reservation 和输入/focus preparation；atomic tail 仅作预备好的不可失败原位 mutation，不含 query、可失败 validation、结构或 hook/observer-triggering 写入，返回后不再补装 owner。
- pending 不是 active UI；resolve/apply 成功且身份校验通过后才可发布。失败清理 router-owned 局部状态并保留明确错误来源，不触碰 app-owned host。
- hidden、pending、leaving、failed 或 retained 内容不能保留输入权限或 active focus。原生 pointer 隔离包括历史 hover 和已开始的手势；撤权按 target 范围进行，不清空其它有效目标的输入/focus。
- 不持有 `&mut World` 跨 `await`；异步数据只携带可移动句柄/输入，World mutation 留在 apply。stale 结果丢弃且不能产生可见 publication。
- 不依赖 Bevy query 未定义顺序；需要稳定顺序时使用明确 key/index/排序。精确更新局部状态，不用整 host 重建规避问题。

## 4. 规划、审查与验证

- MVP 使用一份完整 spec 和一份实施 plan：spec 记录合同、边界、理由和验收结果；plan 按可交付结果安排依赖、验证、审查及提交，阶段不必细化到每个动作。小修复不另建 spec，不机械套用 TDD。
- 先核实审查意见与真实生产路径，再按证据采纳。已授权的复杂重要问题使用独立子代理及交叉复核；审查一致不替代可行性和运行证据。
- 测试按变更风险选择，保护用户行为与关键不变量，不用多份 Markdown 表互相证明能力。删除旧机制时，保留必要行为回归或先落地等价替代。
- Rust 改动至少执行相关格式/静态检查并运行相称测试；lifecycle、failure 或 terminal 分类变化还须重跑受影响的完整 crate 和已闭合最高层 public journey/facade/consumer gate。
- ownership/staging 验证 exact identity、同 tick 实例变化、历史 seal/stale claim 与 post-write drift；角色转换验证 current/next 独立、no-op 零 mutation 和稀疏 binding；拒绝/重试/终结验证唯一 owner 与有界收敛。
- cleanup/recovery 验证完整 target/protected partition、allowed write set、写后复验及独立 continuation；publication 验证所有 preparation 在 Ready 前，tail 不含可失败或触发 hook 的工作。
- 输入验证须包含真实生产管线的正向命中、非活动拒绝、已开始手势撤权和 focus 释放；adapter 验证只能经受控公开入口影响 Router。
- test-only seam 只能改变生产 predicate 的真实输入事实，测试经该 predicate 导出结果；不能直接注入 classification、terminal 或 lifecycle 结论，移除关键判定时测试应失效。
- 未看到成功命令输出，不声称验证通过。设计审查、临时探针、crate 测试和 MVP 验收分别报告；未运行平台/验证项明确标记，不以 Linux 文本对照替代 Windows 原生验证。
- 文档保持职责单一；经验只记本项目可复用结论，不记录临时日志/次数/路径，不混入其它项目。旧合同先迁移必要语义与测试依赖，再删除或归档。

## 5. 共享工作区纪律

- 保留用户已有变更，不顺手纳入、回退或改写职责外文件。
- 同一时刻只有一个明确的 index/提交负责人；其提交期间，其他代理不得暂存或提交。
- 共享 workspace target 同一时刻只有一个 Cargo/rustc 验证负责人，其他代理静态审查或等待，避免构建产物竞争。
- 只用精确文件列表执行 `git add`，禁止 `git add -A`、`git add .` 或隐式包含他人文件的提交。
- 提交前检查 staged 文件集合和 diff；发现职责外文件停止提交并交还负责人处理。
