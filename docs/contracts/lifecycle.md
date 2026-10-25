# 生命周期与清理合同

[中文](lifecycle.md) | [English](lifecycle.en.md)

本页说明 host 接纳、执行身份、实体责任、页面发布和失败恢复。导航结果见[导航](navigation.md)，创建期责任见[Scene](scene.md)，输入撤权见[输入与焦点](input-focus.md)。

## Host 接纳与重新注册

应用创建和布置 host。Router 管理 host 与 top slot 的授权挂载边；host 本身、样式和其它应用子节点继续由应用管理。

合法 host 及完整 `ChildOf` 祖先链的每个 Entity 都须存在、带 `Node` 且关系无环。接纳后保持原祖先链；reparent host/祖先或移除祖先 `Node` 会使原绑定失效。普通样式修改保持该拓扑身份。

每次 registration 有独立 incarnation。RouterHandle、exact Entity、registration 实例与 host epoch 共同确定接纳身份，同 Entity 同 tick 移除再插入也创建新实例。

首次接纳需要唯一合法候选。已有合法 host 时，额外候选被拒绝，当前绑定继续工作。旧前台责任结算后可以重新注册，建立新 epoch 并接收新命令。

## Ownership 与 publication

执行身份关联 Router、host、slot、frame identity 和 generation，延迟完成还匹配 task。registry 写入验证 exact Entity、owner、seal 和预期 lifecycle。已消费 authority ID 与 ownership seal 永久退役。

Router-created Entity 先登记 ownership，再写可能触发 hook/observer 的 component/relationship，写后复验。事实发生漂移时保持内容 inert 并停止发布。

lifecycle capability 是唯一持有的执行凭据，单次移动或消费。拒绝、retry 和 completion error 返回原 owner；snapshot、fingerprint、projection 保存校验信息。

current 与 next 独立冻结和校验。`next=None` 只做校验，物理 mutation plan 为空。延迟或 one-shot action 冻结完整 source projection，消费时与当前 authority 的完整投影精确比较。

pending shell 用 `Display::None` 隔离，同时保留普通内容 authored 值。resolve/apply 成功、身份与 adoption 验证通过后，进入 publication 准备。

发布前完成校验、必要分配、排序、owner 组合与安装、claim rebind、cleanup reservation 及 input/focus preparation。随后在同一 exclusive 调用中执行 publication tail：只对已准备、已借用状态做不可失败的原位更新。query、可失败 validation、结构写入和 hook/observer-triggering 写入都在 tail 前完成。Core 最后发布 presentation/history。

该时刻同时切换 Router 状态和输入资格，old/new 只有一方为 Router-input-active。普通系统可能看到 staging Entity；Router 的回退范围是受管实体，应用资源副作用由应用负责。

旧或不匹配结果只结算自己的 pending 责任，当前请求保持原身份与结果。异步执行只跨 await 携带可移动数据，World mutation 留在 apply；当前 BSN 使用同步 Scene 创建。

## 可选过渡存续

`RouteTransitionDriver` 的 prepare 在首次可见前运行；presented 表示新路由已发布。应用负责效果、时间与重绘，Router 负责精确展示表面、完成/取消和实例处置。接口见[过渡指南](../guides/transitions.md)。

旧页在退出存续期间可以可见，但不具有 activity、当前输入或 focus。发布时确定 retain/dispose/retire 分区；释放时才冻结动态内容的实际清理清单。旧页在原退出 owner 交接完成后成为可命中的 Live cache。进入动画可在请求终态后继续；旧页存续未释放时请求仍承担退出责任。

连续请求按原顺序执行：持有退出页时，新请求先同步取消旧会话并完成退出交接；只有进入动画时，实际改变视图才取消原会话，等价导航不重启动画。完成命令按不透明会话身份匹配，旧命令不结算新会话。取消回调返回前必须停止应用写入。

## 非活动实例与长期 owner

符合[实例保留](navigation.md#实例保留)条件的离场实例，逐一移交 Core 签发的独立、不可 Clone 的长期 owner。ECS 保存原内容和实例自有 slots，父子分别承担责任。移交和其余清理完成后，原请求可以终结。

retained shell 保留在原 child slot 下，与 active sibling 共存，具有自己的 retained role/claim。成为可命中库存后，它隐藏且不参与布局。合法 protected 实例需匹配完整 scope 和原 owner；命中前分别复验库存、current 与 next。拒绝、取消或清理错误时，原 owner 继续由对应 holder 持有或返还。

容量淘汰、父 scope 失效及 host loss 通过 cleanup owner 结算。已退休库存保留原 normal cleanup token，live owner 在受控移交时绑定对应 token，每份责任只移交一次。host 重新接纳等待原 settlement；新 incarnation 从新内容开始，history 按原规则保留。

库存自身失效和共享 authority loss 分开判断。共享父/slot/host 有效，且失效实例及依赖已安全清理时，同一 request 使用原 continuation，从失效位置开始 fresh，期间合法 active 页面保持。新子失败先清理本次 fresh 产物，再独立结算失效库存。

安全删除条件不成立时停止删除，保留明确责任。若停止的依赖清理仍有存活目标且 host authority 已失效，原 token、receipts、owners 和内容转入私有 residual 责任，前台请求以 `AuthorityLost` 终结，Router 保持 unavailable。物理清理仍记录为未完成；已删除或可证明为空且 inert 的原 shell 使用对应受控结算路径。

普通新内容失败保留仍合法父 scope 的 active 和其它库存。error boundary 保留其上方合法 scope；fallback 或父 scope 替换会结算相应离场库存。

## 失败、恢复与 host loss

build、resolve、apply、依赖未就绪和 mount 失败保留可定位的来源与阶段。公共 failure 提供分类，详细诊断提供上下文。

| 情况 | Presentation | History / request |
| --- | --- | --- |
| normal success | 新 route | 提交 history；结算后 Succeeded |
| match failure | fallback 成功则 Error | history 保持；Failed(original) |
| build/resolve/apply/entering mount failure | 最近可用 boundary error view，否则 fallback | history 保持；Failed(original) |
| 恢复自身失败 | 保留合法旧 presentation，或 None | history 保持；Failed(RecoveryFailed)，记录原始与恢复失败 |
| 发布前 host loss | settlement 后 None | 保留旧 history；Failed(HostLost)，Unavailable |
| 发布后 host loss | 随后清空 active | 保留已发布 history 与原结果，附 diagnostic，Unavailable |

boundary 从失败 route 向祖先寻找，保留严格位于 boundary 上方的合法 prefix，错误 view 替代 boundary/suffix。恢复尝试获得新身份，每次失败只尝试一次恢复。无 error binding 的 nested Push 失败在 pending cleanup 后进入 router fallback。

committed authority loss 隔离并清空失效 active，按 host 实际状态结算，已有 history 保持。请求结果按损失在 publication 前后分别分类。

host loss 也覆盖已无运行请求的 idle Router，以及 Ready 但尚无 active 的绑定。原 host grant 单次退役并创建独立 settlement owner；已终结请求保持终态。

无 active 时仍结算 top slot 与授权挂载边。原边安全结算后可重新接纳，新 host 等待新导航，旧请求和 history 保留。

cleanup target/obligation manifest 与 completion 后的 continuation 独立冻结。continuation 来自原 failure/authority、attempt 和 branch 分类；物理删除数量仅反映清理进度。

## Cleanup 与内容边界

Router 管理自建结构和授权 frame 内容边界。挂入此边界的普通内容随页面管理，借用的应用实体放在边界之外；移出的普通内容由应用接手。自建 shell/root/slot 的责任仍由原 owner 持有。

创建期登记 exact allocation obligations，覆盖未挂接产物；成功 adoption 后切换为 current-containment 管理。cleanup ledger 与唯一 owner 管理两类义务：

- `Registered`：结构或尚未 adopt 的产物，使用自身 exact claim。
- `ContainedContent`：普通内容，冻结 exact Entity、source root/shell 和关系，由原 frame owner 提供 authority。

普通内容退出使用上述关系证据，保持现有 registry 记录、marker 和 seal。root claim 与普通 child 的登记身份分别校验。

先撤销目标输入/focus，再按 child-frame→parent-frame、内容 leaf→root 清理。每次操作明确 target、protected closure 和 allowed write set。应用内容、foreign、preserved 及其它 authority 的 non-target 内容保持 protected；target 与所有 protected closure 必须分离。

冻结后新挂入实体不扩张原清理权限。内容缺失、关系变化、历史创建义务和实际残留分别判断。操作内可以冻结角色、删除顺序和 receipt；每次真实写入后重新验证 source、剩余关系及完整 target/protected partition。

合法无关组件或样式变化保持有效；需要匹配的是相关身份、关系、隔离、输入和实际写集。只有证明目的、接受角色和失败语义一致时，验证 primitive 才可共用；只读 preservation 或零目标检查允许合法 protected extras。

无法证明安全删除时停止破坏性操作并记录来源。已证明隔离的残留归 private inert residual；`Unisolated` 保持其未隔离分类。永久矛盾有界终止，容量或可完成前置条件不足可保留原 owner 重试。

terminal 前消费原前台 scope 的全部 action、ticket 和 capability，私有 residual 由独立责任持有。当前残留按原结算路径管理。未完成 apply/settlement 保留依赖；终结后的 inert residual 释放原 attempt 的依赖持有，实体上强 Handle 按 Bevy 规则继续保活。

实现见 [cleanup](../../crates/bevy_ui_router_ecs/src/cleanup.rs) 与 [registry](../../crates/bevy_ui_router_ecs/src/registry.rs)。
