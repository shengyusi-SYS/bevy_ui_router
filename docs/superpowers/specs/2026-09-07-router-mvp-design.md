# Bevy UI Router 统一 MVP Design Spec

**日期**：2026-09-07

**状态**：Active design。总体目标与范围已获用户确认，完整修订稿经两份独立审查和交叉复核，结论为可开始实施；第 12 节最小机制已有官方依赖探针证据。合同生效不代表产品实现或 MVP 已通过验证。

**用途**：MVP 产品范围、架构、公共语义与验收结果的统一依据。实施顺序和当前事实分别由 implementation plan 与 current-architecture 记录。本文替代旧 Target、Matrix、Fixture、Allowlist、Golden、Journeys、Goals 的范围与重复规范地位；旧文件暂留供已有实现/测试迁移和历史追溯，不能反向扩张新 MVP。

## 1. 产品目标与范围

提供通用 Bevy UI Router。应用通过 facade 声明路由、注册 view、绑定自己拥有的 UI host，使用 push、replace、back 展示嵌套路由；成功导航统一提交 presentation 与 history，失败有明确结果，非活动 UI 不接收 Router 集成的输入或保留焦点。

MVP 交付官方同步、代码驱动 BSN adapter，同时保留 renderer-neutral 的 Core/View 边界和独立测试能力。旧 data-only renderer API 无兼容义务，可删除、替换或内部化；这不取消通用 adapter 边界、可组合布局和既有用户行为。

必须交付：

- 官方 crates.io Bevy `0.19.1`，无 Bevy fork、patch 或 source replacement；Core/View 和 facade 默认功能不依赖 Bevy。
- builder-first 路由树、静态/参数/末尾 wildcard 匹配、index route、pathless layout、raw location/search/fragment、route-local params。
- 单条嵌套路由链、每个 layout 的单个 default outlet、common-prefix 保留、suffix 替换、memory history。
- 官方 native Scene 的同步 factory/resolve/apply、内部容器上的 outlet 声明、基本错误诊断与 request/router snapshots。
- route-bound error view、router fallback、host loss/re-registration、stale 拒绝、router-owned cleanup、输入与焦点隔离。
- 最小独立测试替身、真实 BSN consumer、普通嵌套布局示例、相称的跨层验证和独立审查。

非目标：第二种生产 renderer、typed route/search、route codegen、loader/action/async guard/revalidation、transition、retained cache、multiple/named/dynamic outlets、queued/streaming/file/cached-asset BSN、hot reload、平台特定 adapter、multi-router 产品旅程、background reaper/fairness、public authority audit/probe/quarantine DTO。保留最小 RouterHandle 命名空间隔离回归。Post-MVP 项不会因为已有实现或测试而自动成为 MVP 前置。

## 2. 分层与依赖

| 层 | 拥有的职责 | 不拥有 |
| --- | --- | --- |
| Core | location/matcher、branch diff、请求顺序、frame/generation、history、成功/失败与恢复决定、基本诊断 | Bevy/Entity/Scene、物理 cleanup |
| View | renderer-neutral key、descriptor、只读 effect/input、view binding 合同 | World、Scene、lifecycle 权限 |
| ECS | host/slot/frame 物理 binding、renderer 执行编排、输入/焦点、staging、publication、cleanup | 路由匹配、第二套 history 或失败选择逻辑 |
| BSN | Scene factory contract、default outlet 声明、BSN 本地错误 | Core state、host/slot/frame 权限、独立导航 runtime |
| Facade | 常用 API 和 opt-in feature 组合 | 内部 owner、registry writer、可伪造的 report/commit token |

依赖采用 `BSN -> Core/View + 官方 Scene 所需 Bevy crates`、`ECS --optional bsn--> BSN`、`facade bsn -> ECS bridge + BSN`。移除当前空 BSN crate 对 ECS 的依赖，避免环。BSN 的 App extension 实现在 ECS 的 optional bridge 中，由 facade re-export；factory 和声明类型留在 BSN crate。

Core/View 的 effect、输入与完成/失败语义独立于 Scene。其他 renderer 可以通过新的受控 ECS integration 消费相同合同，无需改变 Core；MVP 不建设覆盖所有未来 renderer 的插件框架。独立测试替身使用相同 Core 决策和真实 ECS 执行路径，不能直接制造 publication 或 terminal 结果。

`default`/`view` 不拉入 Bevy；`ecs` 启用 ECS 集成；`bsn` 显式启用官方 Scene bridge。开发期测试工具不属于默认产品 surface。

## 3. 应用装配与 BSN API

沿用 `RouteTree::builder`、`RouteNodeOptions`、`RouteViewKey`、`ViewDriverKey`、`RouteViewRegistry`、`RouteViewDescriptor`、`RouterConfig`、`RouterHandle`、`RouterHostRegistration`、`RouterNavigation` 和 snapshots 的职责。与旧 data-only renderer 耦合的签名可以调整；导航、host 和失败语义按本文保持。

BSN 注册入口的目标签名如下；类型实现在所属 crate，facade 提供统一导入路径：

```rust,ignore
pub trait AppBsnExt {
    fn register_bsn_view<F, S>(
        &mut self,
        key: ViewDriverKey,
        factory: F,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError>
    where
        F: FnMut(&RouteViewInput) -> Result<S, BsnBuildError> + Send + Sync + 'static,
        S: bevy_scene::Scene;
}

// BSN crate 的可存储合同，供 ECS bridge 消费。
pub type BsnSceneFactory = Box<
    dyn FnMut(&RouteViewInput) -> Result<Box<dyn bevy_scene::Scene>, BsnBuildError>
        + Send + Sync,
>;

#[derive(Component, FromTemplate, Default, Clone, Copy)]
pub struct DefaultOutlet;
```

`BsnBuildError` 是可读 build failure，提供 `new(message)` 与错误格式化；不包含 authority。factory 的泛型返回值在注册时包装成 fresh `Box<dyn Scene>`，每个 effect 调用一次并单次消费；错误尝试也生成自己的新 Scene，不克隆或重放旧 Scene。

MVP 的 Scene 执行 profile 是直接代码组合的 native Scene、标准 `ChildOf` 声明图、正常 component/bundle/fallible templates，以及该图内的命名引用；不把它宣传为全部 Bevy Scene 资产/加载功能。cached Scene asset inclusion（包括已经加载的 cached patch）在分配 UI 前明确拒绝，不静默忽略；普通代码 Scene 的组合、模板失败和资源 handle 不因此被禁止。非 `ChildOf` 输出与模板主动创建的闭包外实体属于应用集成责任，不能作为隐式 Router UI 输出。

所有 renderer key 共用一个注册命名空间；重复 key 同步拒绝且不替换旧 factory。注册 factory 不能发布 UI。`add_ui_router` 同步校验 normal view、error view、fallback 和 driver binding；node 按声明顺序检查，normal view 缺失先于该 node 的 error view 缺失。MVP 不提供动态 unregister/rebind。

`RouteViewInput` 只读提供 requested location、matched route、route-local params、raw search、fragment、origin 与错误尝试的原始 failure。factory 没有 World、Commands、Entity、slot、frame/generation、report session 或 commit 权限。应用的 Scene/template/observer 仍可按官方能力运行可信应用代码。

### 3.1 必需的 Bevy runtime

官方 immediate `apply_scene` 读取 `AssetServer` 和 `Assets<ScenePatch>`；同步代码驱动不意味着无需 Scene runtime。应用先安装正常的 Bevy 插件，或在最小环境安装 `TaskPoolPlugin`、`AssetPlugin`、`ScenePlugin`。Router 不替应用选择 asset 路径、窗口、相机或渲染配置。

安装含 BSN binding 的 Router 时检查这些必要资源，缺失返回明确的 `MissingSceneRuntime` 安装错误；运行期间资源被撤销，则在调用 Scene 前返回有来源的 view failure，不能触发缺失资源 panic。Cargo feature 只控制编译，不能当成 runtime 就绪证据。实际 UI/picking/focus 插件由示例和集成说明明确列出。

### 3.2 推荐用法

应用建立 route tree 与 view registry，将 BSN 注册返回的 key 放入 `RouteViewDescriptor`，再调用 `add_ui_router`，在 app-owned `Node` 上添加 `RouterHostRegistration::new(handle)`。通过 `RouterNavigation::{push,replace,back}` 入队并取得 request id，通过 snapshots 观察结果。

Scene 内部的单 outlet 可以写为：

```rust,ignore
bsn! {
    Node { width: Val::Percent(100.0), height: Val::Percent(100.0) }
    Children [
        (Node { width: Val::Px(80.0) }),
        (Node { flex_grow: 1.0 }
            Children [
                (Node { height: Val::Px(32.0) }),
                (Node { flex_grow: 1.0 } DefaultOutlet)
            ])
    ]
}
```

示例表达固定侧栏、工具栏和内部页面区域；最终 quick start 必须作为实际 consumer 编译运行，本文片段本身不是通过证据。

## 4. 路由、location 与 history

路由树使用 builder API，允许 path、index、pathless 的已确认组合；matcher 按既有 specificity 规则选择完整匹配，不依赖查询顺序。最高优先级相同但 leaf 不同的候选返回 `MatchConflict`，不能按先声明者胜出；声明顺序只用于确定遍历和诊断顺序。非法定义在构造/安装阶段报错，无匹配产生 route failure。params 按 route-local 作用域传给 view。

path 规范化为唯一前导 `/`、去除空段与非根尾随 `/`；search 和 fragment 原样保存。保留以下现有输入更新语义：

| 导航变化 | view/lifecycle 行为 |
| --- | --- |
| 同一祖先下切换 leaf | 保留不变 prefix 的 frame、outlet、UI 状态，替换 suffix |
| 某段 route-local params 或复用 identity 改变 | 从该段起替换，保留之前的 prefix |
| raw search 改变 | 按既有 search fingerprint 规则重建整条链 |
| 只有 fragment 改变 | 保留祖先，重建最深 normal frame；frame/slot identity 保留，generation 推进 |
| canonical path/search/fragment 均等价 | 允许零物理 mutation；history 仍按本次命令语义提交 |

新建或重建 view 收到本次输入；保留的 layout 不重新调用 factory，其局部内容维持原状态。snapshot 的 active location 随导航提交，不据此声称保留 layout 的历史 factory input 已被原位更新。active error presentation 不走 normal 等价导航优化。

命令按 Router 内部 sequence 串行处理。push 在成功时追加 history 并截断 forward 分支；replace 在成功时替换当前位置，空 history 时建立首项；back 在轮到运行时冻结前一个成功 entry，成功才移动 cursor。入队时已发布的 `can_back=false` 则同步返回 `NoBackEntry`，不创建 request；已成功入队但轮到执行时无目标，则请求以 `BackTargetUnavailable` 失败，UI 和 history 保持且不调用 view。排队期间观察到的 can-back 不是执行权限。

普通导航成功才统一推进 active route presentation 和 history。错误 presentation 可以替换可见 UI，但原导航仍失败，history 不推进。重试通过新的显式命令完成，不自动回放旧请求。

## 5. Host、frame 与 ownership

app-owned host 由应用创建和布置；Router 可以管理它与自己 top slot 的授权挂载边，不删除、替换或重建 host，也不改写 host 的 UI 样式与其它 app-owned children。

每个 host registration 有独立 incarnation，与 RouterHandle、exact Entity 和 host epoch 一起决定 admission；移除再插入同一 component 也不是同一实例。重复候选的处理保持确定性：首次无唯一合法候选不 ready；已有合法 host 时拒绝额外候选，保留当前绑定。重新注册在旧前台 settlement 完成后建立新 epoch，只接受新命令。

物理默认结构选择复用现有两层 frame：

```text
app-owned host
└─ Router top slot
   └─ FrameShell              Router 的生命周期与隔离壳
      └─ FrameContentTarget   native Scene 应用的内容根
         └─ Scene content … DefaultOutlet
            └─ Router child slot
               └─ child FrameShell -> child content root
```

`FrameContentTarget` 从旧 Router-authored UI container 改为 Scene-authored root；Router 只保留私有 identity/binding 标记。公共 `Node`、`Visibility`、`Pickable` 等内容样式由 Scene 定义，生命周期切换不覆盖它们。FrameShell 上的布局和隔离组件由 Router 控制。没有必要仅为兼容旧 manifest 再增加一层 Entity。

Router 自建结构在写 component/relationship 之前登记 exact Router、host、slot、frame、generation 与 ownership；相关写入后重验。Scene 内容允许任意正常应用组件，完成建立后的内容使用当前标准 `Children` containment adoption，不套用旧 `SolidRect` 的封闭 component manifest。创建期按第 12 节登记本次声明图的 exact allocation obligations，覆盖尚未挂入该 closure 的产物；成功且 adoption 验证后才交接为 current-containment 管理。不能以一次 post-apply Children 扫描代替创建期责任。

Scene 内容的管理责任在其处于该 frame 内容边界内期间成立。应用不应把 borrowed/app-owned Entity 挂入该边界；内容 Entity 被应用移出后，后续管理由应用承担。Router 自建 shell、root、slot 的线性 owner 不会因意外脱离边界而消失，仍必须拒绝发布并完成相应安全 settlement。

每个 frame 的内容遍历遇到 child slot 时停止，嵌套 frame 使用自己的身份和清理顺序。验证 parent prefix、outlet 和 child frame 各自 role/binding，不以集合数量或数组位置推断权限。Snapshot/projection 仅供比较，不重建 owner；拒绝、重试与完成错误都必须保持原 owner 唯一可追踪。

## 6. Default outlet

每个需承载 child route 的 layout 声明一个 `DefaultOutlet`，可在内容根或内部 UI 容器。该普通声明组件不含任何 Router identity 或权限；ECS 只在当前 frame 内容 closure 内解析它，检验唯一性、Node 条件及 relationship，再建立私有 mount binding。

leaf 不要求 outlet。需要 child 时缺失、重复、非容器或失效声明产生明确 mount failure；不能默认挑第一个或退回 root。父 layout 保留时保留同一 outlet anchor 和 child slot，普通 leaf 导航只替换 child frame。声明与实际 slot 分离，不能靠移动 marker 动态切换活动挂载点。

消费 mount 前重新核对 parent/frame generation、anchor、slot 和关系：entering parent 的缺失/无效 outlet 属于 pending rejection；已提交 preserved parent 丢失有效 binding 属于 authority loss。两者不得混成同一 retry 或同一 fallback 路径。多命名 outlet、全 World 的 Name 搜索和动态 outlet placement 延后。

## 7. Scene 执行、切换与隔离

一个 view effect 的处理为：验证当前任务与 host → 调用 factory 并消费 owned Scene 完成 resolve/声明图预检 → 预分配并登记 inert shell/root 及声明图 → 官方 apply/adoption，重建内容 closure 并复验 root/binding → 必要时绑定 outlet/进入 child → 准备 publication。顺序与第 12 节一致，cached inclusion 等预检失败在 UI 分配前拒绝。中途失败进入对应 cleanup/recovery；旧或不匹配的任务不调用可见 publication。

执行器按第 12 节使用官方 resolve/apply 与私有 shell wrapper；不能用裸 `apply_scene` 加事后 Children 遍历替代创建期追踪。Scene root 和 child 可能在 relationship 建立前返回 apply error；apply 与其返回后的 adoption/失败隔离在同一 exclusive 调用内处理，不把未挂接的部分 UI 留到普通系统或下一输入/呈现边界。后续可延迟安全销毁，但必须先以本次 exact obligations 完成 inert 隔离，不能把“以后 cleanup”当成当前隔离。

Scene 成功返回不代表 active。pending shell 采用 `Node.display = Display::None`，使内容在原生 UI 布局/拾取处理后不参与呈现或命中；正常 active 恢复 Router 自控 shell 的 display。不能只用 parent `Visibility::Hidden`，也不能逐项覆盖 Scene authored 的 Visibility/Pickable。

发布前完成全部检查、必要分配、排序、owner 组合、claim rebind、输入集合与 cleanup obligation 预备。publication tail 在同一 exclusive 执行中，只对预建 Router 状态和组件值原位赋值；不包含 Scene apply、结构写入、observer/hook-triggering 写入、query 或可失败工作。Core 发布 presentation/history 是最后语义步骤，返回后不再补装 owner。

“统一发布”保证 Router 状态与输入活性在线性化点一致：old/new 不同时 Router-input-active，pending 不被报告为 active。普通应用系统可以看见 staging Entity，任意 Scene/template 对应用 resource 的副作用不属于 Router 事务 rollback。

### 7.1 输入与焦点

标准 UI pointer 使用原生 picking；键盘/gamepad 的 target-addressed Router 集成通过 `RouterInputGate`，应用系统在 `RouterInputSet::Ready` 后消费。`Refresh` 从当前 active content roots 的实际 frame-local Children closure 计算权限，替代现有构建时 SolidRect 列表；publication 在同一调用内更新权限，离场/失败立即撤销，并清除该目标闭包内的 Router/InputFocus 焦点。

应用动态增删内容在输入刷新前提交，下一 `Ready` 边界承认新 child、撤销移出 child。输入帧内的拓扑修改须遵守这个调度边界；无 World 的 gate 是该输入边界的快照，不能被描述成任意时刻的实时 ancestry 查询。Router 自己的生命周期变化不能等到下一帧才撤销权限。app-owned/其它 frame 的有效 focus 不应被无理由清除。

普通 Scene 局部 pointer observer 必须受到非活动子树的物理隔离；任意应用手动触发事件、绕过 gate 读取原始设备输入或直接改写全局 World，属于可信应用代码责任。原生 UI/picking 正向命中和非活动拒绝必须由真实 pipeline 证明，fake probe 不能替代。

原生 pointer 撤权还必须处理已开始的交互：离场、失败、host/authority loss 时，清除 authorized target 对应的当前/前次 hover、press/click/drag 和祖先缓存，保留其它有效目标状态。ECS 在真实 pointer dispatch 之前再次过滤非活动 owned scope；不能只依赖 `Display::None`、新的命中结果或修改 `Pickable`，也不能全局清空 pointer 状态。官方 `pointer_events` 属于 `PickingSystems::Hover` 的内部 chain，接线须以实际 `generate_hovermap`/dispatch 顺序为准，不能凭 `PostHover` 名称假定它早于派发。

本 MVP 不承诺直接安装原生 `TabNavigationPlugin`/`InputDispatchPlugin` 就自动遵守 Router gate。推荐 keyboard/gamepad 路径显式消费 gate；Scene apply 引发的 `AutoFocus` 需要同时验证非活动目标、原有效 focus 与延后 FocusGained/Lost 派发，不能仅调用 `clear()` 就声称隔离完成。若示例采用原生 Tab/dispatch，必须同时接入并验证相应目标过滤，不能把 gate 自测当成原生插件通过。

## 8. 失败、恢复与清理

BSN build error、官方 `ResolveSceneError`、`ApplySceneError`、依赖未就绪和 mount error 保留来源；未就绪在 immediate MVP 中失败，不暗中变成 queued/无限 retry。stale 结果丢弃并收敛自己的 pending 状态，不把当前请求伪造成成功或失败。

| 情况 | 可见 presentation | History | 请求结果 |
| --- | --- | --- | --- |
| normal success | 新 route | 按 push/replace/back 提交 | Succeeded |
| match failure | router fallback 成功则 Error | 不推进 | Failed(original) |
| build/resolve/apply/entering mount failure | 最近可用 boundary 的 error view，否则 router fallback | 不推进 | Failed(original) |
| error view/fallback 自身失败 | 保留仍合法旧 presentation，或 None | 不推进 | Failed(RecoveryFailed)，含原始与恢复失败 |
| pre-publication host loss | settlement 后 None | 保留旧 history | Failed(HostLost)，Unavailable |
| post-publication host loss | 已提交成功不回滚；随后 active 清空 | 保留已提交位置 | Succeeded + diagnostic，Unavailable |
| committed authority loss | 失效 active 隔离并清空，按有效 host 状态收敛 | 不回滚既有 history | 按损失发生前后的 publication 分类 |

boundary 从失败 route 向祖先查找；保留严格位于 boundary 之上的合法 prefix，错误 view 取代 boundary 及其 suffix。新错误尝试使用新身份，不能继承原 capability。恢复只尝试一次，失败后不递归创建 error view。无 binding 的 nested Push 失败在 pending cleanup 后进入 RouterFallback，不恢复旧 Terminal 语义。

失败 cleanup 的 target/obligation 与 cleanup 后继续哪个生命周期分支分别冻结。continuation 来自原始 failure/authority 与 attempt，不能由是否有旧 active、obligation 数量或是否真的删除实体反推。terminal 前前台 owner/action/ticket 必须消费完毕；永久矛盾有界 fail closed，只有可证明能完成的暂时前置不足才 retry。

清理先隔离输入/focus，再按 child-frame 到 parent-frame、内容叶到根顺序处理当前可证明 owned scope。当前关系、protected targets 与写入范围在会触发 observer 的操作后重新验证。不得因 root 曾 owned 就级联删除 app-owned/protected extras；不确定时停止破坏性操作，保留 private inert residual。独立 residual owner 可以继续负责隔离，但不持有已经 terminal 请求的前台 capability，不承诺后台最终 GC。

trusted Scene 不承担恶意脚本假设；Router 无需列举其全部应用组件或回滚 closure 外资源副作用。它仍必须正确处理同步失败、自己的 ownership/binding 漂移、host 丢失和正常 Scene 局部行为，不得以信任应用为理由跳过这些检查。

## 9. Snapshots 与诊断

request snapshot 提供请求自身 requested/matched/status 和可分类 failure；Router snapshot 分开提供 running request 摘要、active presentation、availability、history/can-back。queued/last-terminal 不能冒充 running，error presentation 的目标不能伪装成一次成功 history entry。

诊断至少能定位 Router/request、route（若已匹配）、失败来源与发生阶段，区分恢复失败和已提交后的异常。只在语义转移时记录，不以 polling 次数制造诊断。有限日志保留需让消费方区分 unknown 与已淘汰；不公开 raw internal Entity 映射、owner 构造或全量 registry audit。

公开调度保留 `RouterInputSet::{Refresh, Ready}` 和 `Update` 中的 `RouterScheduleSet::PublishSnapshots` 作为稳定入口；应用在后者之后读取本轮 snapshots。内部 Observe/Drive/Commit/Cleanup 的顺序由 ECS 持有，其细粒度窗口用于私有验证，不成为应用推进 lifecycle 的 API。

## 10. 验收与维护证据

| 用户结果/边界 | 必需的独立证据 |
| --- | --- |
| 官方可复现依赖与 feature 分层 | resolved lock source/version，无 patch/replacement；无默认 Bevy、无 Core/View Scene 依赖 |
| 基础路由可用 | builder、index/pathless/静态/params/wildcard、location/search/fragment、无匹配、同优先级 MatchConflict、确定顺序的 Core/View 测试 |
| Renderer 边界真实独立 | 最小 fake renderer 或等价替身驱动实际逻辑，不依赖 BSN、不伪造 lifecycle 结论 |
| 应用可直接使用 BSN | 独立临时 workspace 仅依赖 facade 与所需官方 Bevy crates，注册/运行 root、读取请求结果；短小 quick start |
| 普通嵌套 UI | 内部 default outlet 的侧栏/工具栏/内容区示例；leaf 切换保留 layout/anchor/slot 和局部状态 |
| 历史与更新一致 | push/replace/back、NoBackEntry 同步拒绝、入队后 BackTargetUnavailable、失败 back、等价 location、search 变化、fragment-only leaf remount |
| 恢复与 host | boundary/fallback、真实 build/resolve/apply/mount error、恢复再失败、publication 前后 host loss、重新注册 |
| Owned-only 与 stale | host 值/关系保持，frame generation 和 task stale 拒绝，稀疏 prefix、owner 单次消费、post-write 局部漂移回归 |
| 输入与焦点 | 原生 UI layout/picking 有真实正向命中；pending/leaving/failed/residual 拒绝，按下/拖拽跨离场后不再派发到旧目标；keyboard/gamepad gate、focus 释放、动态 child 刷新 |
| 跨层收敛 | 受影响 crate 与最高层 public journeys、feature 组合、workspace fmt/check/test/doctest；独立 Core/ECS/public 审查及交叉复核 |

测试断言用户行为和最小不变量，不再解析多份 Markdown 互相证明一致。既有有价值回归在替代验证落地后迁移或合并；不按固定测试数量或文件名验收。文本/consumer harness 兼容 LF/CRLF 和 Windows 路径，无法实际运行的平台验证必须明确记录。

完整 spec 保留必要设计理由；AGENTS 只保留工程护栏；plan 保存执行依赖、阶段结果、验证与提交边界。删除或归档旧规范前迁移必要语义和经验，并解除解析旧文档的测试依赖。实施完成后事实文档如实记录结果，MVP 只有在上述证据齐全且完成独立审查后才可激活和集成。

## 11. 设计依据与取舍

- 原始项目决策明确 Core/View 与 BSN 分离、builder-first、raw location 和独立 fake renderer；保留这些目的，放弃旧生产 data-only API 的形状。
- 单 outlet 允许内部位置声明，避免把布局限制误作 MVP 精简；multiple/dynamic outlet 另行设计。
- 两层 shell/content-root 复用已有结构，职责迁移允许 Scene 自由描述 UI。额外 root 层和每个 authored component 的 manifest 审计没有当前必要性。
- 正常前台 cleanup 和 private inert residual 足以表达 MVP 生命周期结果；background fairness/audit 产品化缺少当前消费需求。
- 官方 [`SceneBox`](https://docs.rs/bevy/0.19.1/bevy/scene/trait.SceneBox.html) 支持 `Box<dyn Scene>` 单次消费；[`apply_scene`](https://docs.rs/bevy/0.19.1/bevy/scene/trait.EntityWorldMutSceneExt.html) 应用到既有 Entity 并覆盖同类组件；[`Display::None`](https://docs.rs/bevy/0.19.1/bevy/ui/enum.Display.html#variant.None) 定义子树不布局/渲染。最终隔离合同仍以真实 UI/picking 验证为准。

本文的片段、类型设计和官方文档引用不等同于 Router 完成证据；实现及实际验证进度记录在配套 plan。

## 12. 官方 Scene 创建期责任与选定机制

两份独立审查与交叉复核确认总体分层可继续，未建议重做 Core；matcher/Back 的简述遗漏已按现有公共行为修正，原生 pointer 的历史交互撤权也已补入合同。

官方最小探针已经证明：related child 的 bundle 写入 `Node` 后，后续 bundle 返回 error，该 Entity 可以没有 `ChildOf`，且 despawn 根后仍存在。这证明失败产物不全在 root containment 内；不据此断言屏幕上已经出现可见泄漏。该普通失败不能归为恶意应用或任意资源副作用。

选择 **官方 resolve + 已登记声明图分配 + 私有 shell wrapper + attempt allocation obligations**，不预挂整棵树、不替换官方 relationship callbacks、不复制模板执行器：

1. 使用 `ScenePatch::load/resolve` 消费本次 owned Scene，保留 dependency handles 到 apply/settlement 完成；不重复 resolve，不克隆模板。取出本次未共享的 `ResolvedSceneRoot`。
2. 通过公开的 `get_or_insert_related_resolved_scenes::<ChildOf>()` 读取直接声明图；在 World 分配前检查不支持的 cached inclusion、引用重复/跨声明节点歧义和容量。root/child 的全部原命名引用仍属于原内容 Entity，匿名节点补本次内部唯一引用。
3. ECS 预分配并先登记 root/children 的 exact owner 与创建义务，不提前模拟其 `ChildOf` 写入；官方 executor 仍在模板成功后按原顺序写关系。分配记录不是根据 World 差集推导的 owner。
4. 新建私有 wrapper `ResolvedSceneRoot`，只含一个引用映射 seed template 和 `ChildOf[原 resolved root]`，应用到本次新建的 inert `FrameShell`。wrapper 不携带原 root 的命名引用；seed 将完整引用表绑定到已登记内容 Entity。随后官方执行原 Scene，原 root 复用 `FrameContentTarget`，无需增加物理层数。不能把 wrapper 应用到 preserved/active shell。
5. 成功后检查 root/child 的实际 adoption 与受管边界，再交接到 current containment；失败时本次分配记录继续覆盖尚未挂接的 exact Entity，在返回普通调度前完成隔离和可安全执行的清理。任何 protected closure/drift 仍按 owned-only 规则处理，不能无条件递归删除记录中的 Entity。

此 profile 有明确且有限的结构差异：声明 Entity 提前分配；公开遍历 getter 会使无子节点的 leaf 具有空 `Children` component，可能触发相应 hook。它不承诺和直接 `apply_scene` 的全部中间状态完全相同；应用的 Scene 组件值和正常 Children 声明顺序仍保留。wrapper 增加的最外层 `RelatedSceneError(ChildOf, index=0)` 只属于实现细节，诊断去除该层后保留原错误来源。

选择依据：官方 `0.19.1` 探针已覆盖匿名 child 首次失败、部分 Node 写入失败、成功 sibling 后深层失败、根模板提前引用后代再失败、原 Scene 整体替换 resolve target、root/child 双向命名引用、成功后的 Children 顺序、额外空 Children 的可观察性，以及公开 API 对 cached inclusion 的区分。预挂树方案需要干预官方关系写入，没有相对于独立创建义务的必要优势，因此不采用。

这些是机制可行性证据，不是完整 Router ownership、hook 漂移、输入隔离或 cleanup PASS。生产实现仍需第 10 节与 plan 阶段 C/D 的跨层验证；不把探针里的直接 despawn、固定编号或仅计数检查复制成产品权限证明。
