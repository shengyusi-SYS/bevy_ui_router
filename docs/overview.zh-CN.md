# 产品概览

[English](overview.md) | 简体中文

Bevy UI Router 把页面组织、切换和返回集中到一个路由系统。应用提供页面 Scene 和显示它们的 host，Router 根据路由树选择内容，并维护当前页面与访问记录。

## 页面、布局与访问

假设应用有 `/projects` 和 `/projects/:id`。父布局可以放导航栏，子页面显示项目列表或详情。切换子页时，共用父布局和其中的局部状态保持连续。`RouteInputs` 让复用布局读到新的路径、该布局自身的 route-local 参数和查询信息。

`push` 增加一次访问，`replace` 替换当前访问，`back` 回到上一项记录。访问记录保存在内存中。一次访问和一个页面实例分开管理：多次访问可以复用同一个页面，滚动位置仍按各次访问分别保存。

| 概念 | 含义 |
| --- | --- |
| Route | 路由树中的一项，描述匹配规则和视图 |
| Scene / factory | 页面构建内容 / 每次构建时生成新 Scene 的函数 |
| Host | 应用提供的 UI 容器 |
| Outlet | 布局中放置子页面的位置 |
| Frame | Router 管理的一层页面或布局实例 |
| History entry | 一次成功导航形成的访问记录 |
| Publication | 页面准备完成并成为当前路由呈现的时刻 |

## 已有能力

- 静态路径、路径参数、末尾 wildcard、默认子页和无路径布局。
- 单条嵌套链，每层布局一个 default outlet；公共前缀复用、变化部分替换。
- `push`、`replace`、`back`，以及当前状态、请求结果和诊断快照。
- 同步、代码构建的 Bevy Scene/BSN 页面、路由错误页面和 Router fallback。
- 布局与子页分别选择 keep-alive，按容量和父依赖管理非活动实例；默认容量为 0。
- 按访问记录恢复二维滚动位置，支持晚出现的容器、内容 ready、取消和应用主动滚动。
- 页面活动查询、原生 pointer 隔离、离场手势撤销和焦点释放；键盘/手柄处理器通过显式 gate 接入。
- 可选的[过渡生命周期](guides/transitions.zh-CN.md)：首次可见前准备、退出存续和完成/取消；具体动画由应用实现。
- 可选桌面鼠标后退键和 Android 传统 Back 输入，由应用选择窗口对应的 Router。

页面数据加载由普通 Bevy 任务和系统完成；任务返回数据，系统在 World 中安装内容。窗口、主题、相机、业务数据和后台工作策略由应用配置。

## 选择接入层

推荐通过 `bevy_ui_router` 的 `bsn` feature 接入 Bevy UI。Core 管理路由决策，View 提供与渲染器无关的视图合同，ECS 执行实体变更，BSN 构建 Scene，Platform 传递平台返回意图。默认 `view` feature 和 Core/View 使用独立于 Bevy 的类型。

应用按[接入指南中的版本对应表](guides/integration.zh-CN.md#bevy-版本要求)选择 Router 版本；工具和平台验证范围见[验证指南](development/verification.zh-CN.md)。

浏览器 URL/history、异步 Scene 加载、typed routes 和多出口等后续方向见[路线图](roadmap.zh-CN.md)。开始使用时，阅读[接入指南](guides/integration.zh-CN.md)；查询具体规则时，阅读[行为合同](contracts/README.zh-CN.md)。
