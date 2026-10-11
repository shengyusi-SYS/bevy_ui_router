# 生命周期与清理合同

本页定义 host 接纳、identity/owner、publication、失败恢复和物理责任结算。导航结果见[导航](navigation.md)，创建期产物来源见[Scene](scene.md)，撤权见[输入与焦点](input-focus.md)。

## Host 接纳与重新注册

host 由应用创建和布置。Router 只管理 host 与 top slot 的授权挂载边，不删除、替换、重建 host，不改写它的 UI 样式与其它 app-owned children。

合法 host 及其完整 `ChildOf` 祖先链上的每个 Entity 必须存在、有 `Node`，且关系无环。接纳后保持 exact parent ancestry；reparent host/祖先或撤销祖先 `Node` 会使原 host authority 失效，不能作为透明布局更新。普通样式修改不改变该拓扑身份。

每次 host registration 有独立 incarnation。RouterHandle、exact Entity、registration 实例和 host epoch 共同决定 admission；同 Entity 同 tick 移除再插入也不是原实例。

首次没有唯一合法候选时不 Ready；已有合法 host 时拒绝额外候选、保留当前绑定。重新注册在旧前台 settlement 后建立新 epoch，仅接受新命令。

## Ownership 与 publication

执行身份至少关联 Router、host、slot、frame identity、generation；延迟完成还需核对 task。registry mutation 验证 exact Entity、owner、seal 和预期 lifecycle。

authority ID 与 ownership seal 消费后不复用。Router-created Entity 先登记 ownership，再写可能触发 hook/observer 的 component/relationship，写后重新验证；drift 时保持 inert 并停止不安全发布。

lifecycle capability 线性持有，单次移动或消费。拒绝、retry、completion error 返还原 owner；snapshot、fingerprint 和 projection 只用于比较，不能重建 owner。

current 与 next 独立冻结和校验，不能从提交后结构反推 current。`next=None` 是只校验 no-op，不进入物理 mutation plan。延迟/one-shot action 冻结完整 source projection，消费时要求 exact equality。

pending 不等于 active。resolve/apply 成功且身份与 adoption 校验通过，才可准备 publication。pending shell 使用 `Display::None` 隔离，不只依赖 parent Visibility，也不覆盖普通内容 authored 值。

发布前完成全部校验、必要分配、排序、owner 组合/安装、claim rebind、cleanup reservation 和 input/focus preparation。

同一 exclusive 调用内的 publication tail 只对预备并预借的状态原位赋值，不包含 query、可失败 validation、结构写入、hook/observer-triggering 写入或后置 owner 补装。Core 发布 presentation/history 是最后语义步骤。

统一发布保证 Router 状态与输入活性在线性化点一致：old/new 不同时 Router-input-active，pending 不被报告为 active。普通应用系统仍可看到 staging Entity；任意应用资源副作用不属于 Router rollback。

旧或不匹配结果拒绝 publication，并结算自己的 pending 责任，不能把当前请求伪造成成功或失败。不持有 `&mut World` 跨 await；当前 BSN 不提供异步加载产品能力。

## 非活动实例与长期 owner

符合[实例保留](navigation.md#实例保留)的离场实例逐一移交 Core 签发的独立、不可 Clone 的长期 owner；ECS 持有原物理内容和实例自有 slots，父子各自持有责任。原请求在移交及其余清理完成后可以终结，库存不挂在旧请求的 task、ticket 或 lifecycle capability 上。

retained shell 留在原 child slot 下，具有独立 retained role/claim，与当前 active sibling 共存。它隐藏且不参与布局；只有与完整 scope 和原 owner 匹配的实例才是合法 protected 内容，任意 sibling 不能借此获得授权。命中前复验原缓存、当前 active 与独立 next，原 owner 在拒绝、取消或清理错误时仍须返还或由明确责任继续持有。

容量淘汰、父 scope 失效及 host loss 都通过 cleanup owner 结算缓存。原 normal cleanup 已接收的退休责任保留原 token；仍由缓存持有的 live owner 只在受控移交时绑定相应 cleanup token，不能重复移交，也不能先递归销毁其他 owner 的内容。host 重新接纳仍等待原 settlement；缓存不跨新的 host incarnation 复活，history 按原规则保留。

库存自身失效与当前 authority loss 分开判断。只有共享父/slot/host 仍合法、原失效实例及依赖安全清理完成后，才以同一个 request 和原 continuation 从失效位置起 fresh；期间合法 active 页面保持。新子失败先清理本次 fresh 产物，再独立结算失效库存，不把库存实例记成新建实体。

无法证明安全清理时停止物理删除并保留明确责任。若停止的依赖清理仍有存活目标，且真实 host authority 已失效，原 token、receipt、owners 和物理内容移交现有私有残留责任；原前台请求以 `AuthorityLost` 终结，Router 保持 unavailable，不伪造 cleanup completion 或承诺重新接纳。已全部删除或可证明为空且 inert 的原 shell，仍按既有受控结算路径处理。不承诺任意 World 修改都能自动恢复为 miss。

普通新内容失败不无理由清空仍合法父 scope 的 active 或其他缓存；error boundary 只保留其上方仍合法的 scope，fallback/父 scope 替换则结算离场库存。撤权和应用后台工作边界见[输入与活动资格](input-focus.md)。

## 失败、恢复与 host loss

build、resolve、apply、依赖未就绪和 mount 失败保留可定位来源与阶段。公共 failure 分类和详细诊断分工，不将所有来源降为通用错误，也不承诺直接暴露内部 Bevy error 对象。

| 情况 | Presentation | History / request |
| --- | --- | --- |
| normal success | 新 route | 提交 history；完成结算后 Succeeded |
| match failure | fallback 成功则 Error | history 不推进；Failed(original) |
| build/resolve/apply/entering mount failure | 最近可用 boundary error view，否则 fallback | history 不推进；Failed(original) |
| 恢复自身失败 | 保留仍合法旧 presentation，或 None | history 不推进；Failed(RecoveryFailed)，保留原始与恢复失败 |
| 发布前 host loss | settlement 后 None | 保留旧 history；Failed(HostLost)，Unavailable |
| 发布后 host loss | 随后清空 active | 保留已发布 history 与原请求结果，附 diagnostic，Unavailable |

boundary 从失败 route 向祖先寻找，保留严格位于 boundary 之上的合法 prefix，错误 view 替代 boundary/suffix。新恢复尝试有新身份，不继承原 capability。

恢复只尝试一次，失败后不递归创建 error view。无 error binding 的 nested Push 失败在 pending cleanup 后进入 router fallback。

committed authority loss 隔离并清空失效 active，按实际 host 状态收敛，不回滚既有 history；请求结果按损失发生在 publication 前后分别分类。

host loss 同样覆盖所有请求终态后的 idle Router，以及 Ready 但尚无 active 的绑定。真实 host grant 单次退役建立独立 settlement owner，不伪造 request 或重开旧 terminal 请求。

无 active 也要结算 top slot 与授权挂载边。旧请求和 history 保持；原边安全结算后重新接纳，新 host 不自动恢复旧页面。

cleanup target/obligation manifest 与完成后的 continuation 独立冻结。continuation 来自原 failure/authority、attempt 和 branch 分类，不由旧 active、义务数量或实际删除数量反推。

## Cleanup 与内容边界

Router 管理自己的结构，以及已授权 frame 内容边界。应用不要把 borrowed/app-owned Entity 挂入该边界；普通内容移出后由应用负责。自建 shell/root/slot 的 owner 不因意外脱离而消失。

创建期使用 exact allocation obligations，覆盖尚未挂接的产物；成功 adoption 后交接为 current-containment 管理，不能用一次 Children 扫描代替创建期责任。

一个 cleanup ledger 与线性 owner 承担两类义务：Registered 使用结构或未 adopt 产物自身的 exact claim；ContainedContent 使用 exact Entity、source root/shell 和冻结关系，authority 仍由原 frame owner 持有。

普通内容退出不临时增加 registry record/marker/seal。root claim 不能冒充普通 child 的独立登记，关系证据不能替代或重建 owner。

先撤销目标输入/focus，再按 child-frame→parent-frame、内容 leaf→root 清理。明确 target、protected closure 和 allowed write set；app-owned、foreign、preserved 及其它 authority 的 non-target 内容保持 protected。

target 与 protected closure 必须不相交。冻结后的新挂入 Entity 不自动扩张旧清理权限；内容缺失、关系变化、创建义务及实际残留分别判断，不以当前 closure 为空推导历史义务已完成。

操作内可冻结角色、删除顺序与 receipt；每次真实写后仍重建并验证 source、剩余关系和相关完整 target/protected partition。不得沿旧快照继续破坏性操作，也不以目标自身 hook flag 推断没有回调。

合法无关应用组件或样式变化不单独构成 authority loss；相关身份、关系、隔离、输入与实际写集仍需匹配。保护内容不等于冻结整个 World 或全部组件集合。

无法证明安全删除时停止破坏性操作，保留明确来源。只有已经证明隔离的残留可以描述为 private inert residual；`Unisolated` 保留明确的未隔离状态，不能冒充隔离成功。永久矛盾有界 fail closed；只有可证明未来能完成的容量/前置不足可 retry。

terminal 前原前台 scope 的 outstanding action、ticket 和 capability 全部消费。私有残留隔离责任独立，不持有 terminal 请求的前台 owner，不承诺后台最终 GC。

未完成 apply/settlement 保留原依赖；已终结 inert residual 不承诺保活原 attempt dependencies 至实体销毁。已应用组件的强 Handle 按 Bevy 自身规则保活。

实现见 [cleanup](../../crates/bevy_ui_router_ecs/src/cleanup.rs) 与 [registry](../../crates/bevy_ui_router_ecs/src/registry.rs)。
