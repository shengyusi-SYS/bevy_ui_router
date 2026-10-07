# 产品概览

本文说明核心需求与当前范围。系统关系见[架构总览](architecture/README.md)，行为细节按[专题合同](contracts/README.md)查阅，发布安排见[路线图](roadmap.md)。

## 核心需求与当前范围

提供独立、通用的 Bevy UI Router。应用声明路由树和 view，绑定自己拥有的 UI host，通过 push、replace、back 展示嵌套页面，保留公共布局及局部状态。

成功导航统一发布 presentation 与 history；失败提供明确结果和有限恢复；非活动 Router 内容不保留集成输入权限或焦点。

当前 MVP 交付：

- builder-first 路由树，静态/参数/末尾 wildcard 匹配，index route、pathless layout，以及 raw location/search/fragment。
- 单条嵌套路由链，每个 layout 一个 default outlet，common-prefix 保留、suffix 替换和 memory history。
- 显式滚动容器按 history entry 保存二维位置，Back 恢复并由正常 UI 布局裁剪。
- 官方同步、代码驱动的 Bevy Scene/BSN adapter，以及 renderer-neutral 的 Core/View 边界。
- route-bound error view、router fallback、host loss/re-registration、stale 拒绝、局部 cleanup、输入与 focus 隔离。
- facade、可运行示例、独立 consumer 和相称的跨层验证。

依赖采用官方 crates.io Bevy `0.19`，允许 `>=0.19.0, <0.20.0`；已验证的版本和环境归[验证说明](development/verification.md)。Core/View 与 facade 默认功能不依赖 Bevy。

当前不交付 typed route/search、codegen、loader/action/async guard/revalidation、transition、retained cache、multiple/named/dynamic outlets、queued/streaming/file/cached-asset BSN、hot reload、平台特定 adapter、后台 reaper 或公开 authority audit/probe。

不引入业务、品牌、主题、数据服务、权限或应用 shell 假设。多 Router 保留 handle 命名空间隔离，但完整多 Router 产品旅程不是 MVP 前置。

## 扩展边界

当前范围是开发基线，可以随已确认功能演进。新增功能先说明消费者问题、可观察行为及影响的专题，避免把历史未实现方案或理论能力自动升级为需求。未来设计与实施入口见[开发指南](development/README.md)。
