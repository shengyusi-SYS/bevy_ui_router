# Roadmap

**更新时间**：2026-09-25。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。

## Now

- 官方 native BSN、输入/focus、公开导航/恢复与推荐示例已完成。旧公开机制和冗余私有 fixture 已按真实调用者清理，Core/ECS/public 独立及交叉审查、冻结源码全量验证通过；本地 main 已快进集成，合并后 smoke 通过。
- 补充独立窗口示例、六包打包配置、打包消费者验证和单 Linux CI。Linux/WSLg 真实窗口、软件渲染和 OS 注入键鼠交互已验证；图形示例独立 workspace，避免污染 headless 测试。
- 最小公开 API 性能基线已完成两轮优化构建测量，见[结果与边界](performance.md)。功能验收结论不等于性能就绪。
- 按实测定位完成两项局部简化：cleanup 每轮只读采集共享未结算集合；合并 seal 发行/注册并删除历史账本。完整回归及两轮相同基准通过，128 子节点导航 p50 从约 37–39 ms 降至 17.6–18.3 ms；32/128 子节点原有约 50% 的连续使用退化未再出现。
- 后续 profile 选择分类器中的两项关系组件集合，将 HashSet 简化为固定数组。完整验证及两轮正式基准通过；128 子节点导航 p50 进一步降至 16.3–17.0 ms，按同场景 p50 均值约减少 7.2%，32 子节点约减少 5.6%。空闲与恢复未见改善，详见[完整结果](performance.md#2026-09-25-分类器局部简化)。

## Next

- 若目标应用需要约 128 个子节点的单帧切换，评估剩余 cleanup 成本：当前约 16–17 ms 尚不含 UI 布局与渲染，p95 可超过 60 Hz 帧预算。按实际页面规模决定是否继续局部优化，不直接扩展全局缓存、索引或性能平台。
- 按实际使用反馈维护已验收的 MVP；按所有者选择暂不授予开源许可证，先完成发布准备。正式发布前确定许可与版本。Windows 原生、最低 Rust、物理 GPU 加速和硬件设备输入未验证，不为这些未运行项扩张 MVP 功能或测试矩阵。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。

统一 spec/plan 与范围评审已完成；旧多文档/AST gate 已替换为实际 consumer、LF/CRLF 对照和语义依赖检查，历史协议原位归档。native 工作区已有真实运行路径，仍不能将历史基线或局部通过视为 BSN MVP 验收。
