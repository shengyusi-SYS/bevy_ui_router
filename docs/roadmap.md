# Roadmap

**更新时间**：2026-09-22。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## Now

- 候选精简、独立迁移基线及 idle host-loss 已完成。官方 native BSN 联动、失败 owner 交接与输入/focus 已通过独立及交叉审查、真实反例修正和冻结验证；转入剩余组合验收与旧机制退出，不扩大产品范围。

## Next

- 对照统一 spec 收口剩余真实 BSN 组合与公共 snapshot 观察边界；复用已有内部 default outlet、history、恢复和 host/stale 证据，不重做已完成的联动迁移。
- 完成示例和旧内容模型/API 退出，删除已被替代的过渡入口与重复事实，保留 renderer-neutral 独立验证；不新增 runtime 或通用事务框架。
- 通过推荐 facade 的独立 BSN consumer、示例、全量验证与独立交叉审查后集成 main；完成前不标记 MVP PASS。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。

统一 spec/plan 与范围评审已完成；旧多文档/AST gate 已替换为实际 consumer、LF/CRLF 对照和语义依赖检查，历史协议原位归档。native 工作区已有真实运行路径，仍不能将历史基线或局部通过视为 BSN MVP 验收。
