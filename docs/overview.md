# 产品概览

本文说明核心需求与当前范围。系统关系见[架构总览](architecture/README.md)，行为细节按[专题合同](contracts/README.md)查阅，发布安排见[路线图](roadmap.md)。

## 核心需求与当前范围

提供独立、通用的 Bevy UI Router，让应用声明页面及其嵌套关系，由库统一管理页面跳转、返回和当前路由信息，减少业务页面重复维护导航逻辑。

从应用使用者的角度，核心需求是：

- **组织页面。** 用路由树描述页面、参数和共享布局，在应用自己的 UI host 中展示对应内容。
- **切换与返回。** 通过 push、replace、back 导航，统一管理当前展示与访问记录；普通子页切换保留可复用的公共布局。
- **获得正确上下文。** 页面能够读取当前路由和参数，复用布局也能看到本次导航的信息；错误呈现有明确结果。

keep-alive 和滚动恢复是按需启用的体验增强：前者让布局与子页独立保留离场状态，后者让返回时恢复该次访问的位置。异步页面、过渡动画等另按实际需求选题。

正确性底线是当前展示与路由输入相符，访问记录按导航结果更新，非活动内容不继续响应集成输入或保留焦点，失败不会误动应用拥有的 host。内部身份、owner 和 publication 协议用于落实这些行为，具体合同见[生命周期](contracts/lifecycle.md)。

输入目标是离场或缓存内容退出 Tab 候选、输入目标和有效焦点，已开始手势随离场撤销，应用公共控件及其它有效路由内容照常工作。当前原生 pointer 和显式 keyboard/gamepad gate 已接通，原生 Tab/Dispatch 尚未自动消费路由资格；现行接入见[输入合同](contracts/input-focus.md)，剩余交付见[路线图](roadmap.md)。

当前实现范围：

- builder-first 路由树，静态/参数/末尾 wildcard 匹配，index route、pathless layout，以及 raw location/search/fragment。
- 单条嵌套路由链，每个 layout 一个 default outlet，common-prefix 保留、suffix 替换和 memory history。
- 显式滚动容器按 history entry 保存二维位置，Back 统一恢复已有和晚出现容器；支持内容 ready、取消与应用滚动优先，由正常 UI 布局单次裁剪。
- 有 normal 父作用域的布局与页面可分别 opt-in 保留原 Entity 和局部状态；返回时仍由新路由选择子页。非活动容量默认 0，启用后按简单 LRU 与父依赖闭包淘汰。
- 当前已发布路由输入可只读查询，公共布局复用后仍可取得新 location 和自身 route-local params。
- 官方同步、代码驱动的 Bevy Scene/BSN adapter，以及 renderer-neutral 的 Core/View 边界。
- route-bound error view、router fallback、host loss/re-registration、stale 拒绝、局部 cleanup、输入与 focus 隔离。
- facade、可运行示例、独立 consumer 和相称的跨层验证。

依赖采用官方 crates.io Bevy `0.19`，允许 `>=0.19.0, <0.20.0`；已验证的版本和环境归[验证说明](development/verification.md)。Core/View 与 facade 默认功能不依赖 Bevy。

当前不支持 typed route/search、codegen、loader/action/async guard/revalidation、transition、跨父搬移或跨 host 缓存、multiple/named/dynamic outlets、queued/streaming/file/cached-asset BSN、hot reload、平台特定 adapter、后台 reaper 或公开 authority audit/probe。

实例保留与访问记录是两个维度：不同 history entry 可以复用同一页面，滚动仍按 entry 恢复。页面的应用系统可用 `RouteActivity` 查询当前活动资格；布局、窗口和后台任务由应用配置。行为细节见[导航合同](contracts/navigation.md)，窗口与性能证据以专门验证文档为准。

不引入业务、品牌、主题、数据服务、权限或应用 shell 假设。多 Router 保留 handle 命名空间隔离，但完整多 Router 产品旅程不是 MVP 前置。

## 扩展边界

当前范围是开发基线，可以随已确认功能演进。新增功能先说明消费者问题、可观察行为及影响的专题，避免把历史未实现方案或理论能力自动升级为需求。未来设计与实施入口见[开发指南](development/README.md)。
