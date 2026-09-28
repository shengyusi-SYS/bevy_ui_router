# Roadmap

**更新时间**：2026-09-28。范围与验收归 [统一 MVP spec](superpowers/specs/2026-09-07-router-mvp-design.md)，具体依赖和证据归 [实施 plan](superpowers/plans/2026-09-07-router-mvp-implementation.md)。当前结论以 spec 文件头与 plan 的嵌套 Back 修复节为准。

## Now

- 9/28 修复 query 变化、祖先重建后的嵌套 Back 挂载与有界失败处理，官方 Bevy 基线的完整回归通过。补充真实 `App::update` 调度的组合场景，历史验收不再代替新组合的验证。
- 官方 native BSN、输入/focus、公开导航/恢复与推荐示例已有集成基线。旧公开机制和冗余私有 fixture 已按真实调用者清理；此前独立/交叉审查、冻结版本全量验证和合并后 smoke 均为对应版本及已运行场景的历史证据。
- 补充独立窗口示例、六包打包配置、打包消费者验证和单 Linux CI。Linux/WSLg 真实窗口、软件渲染和 OS 注入键鼠交互已验证；图形示例独立 workspace，避免污染 headless 测试。
- 最小公开 API 性能基线已完成两轮优化构建测量，见[结果与边界](performance.md)。功能验收结论不等于性能就绪。
- 按实测定位完成两项局部简化：cleanup 每轮只读采集共享未结算集合；合并 seal 发行/注册并删除历史账本。完整回归及两轮相同基准通过，128 子节点导航 p50 从约 37–39 ms 降至 17.6–18.3 ms；32/128 子节点原有约 50% 的连续使用退化未再出现。
- 后续 profile 选择分类器中的两项关系组件集合，将 HashSet 简化为固定数组。完整验证及两轮正式基准通过；128 子节点导航 p50 进一步降至 16.3–17.0 ms，按同场景 p50 均值约减少 7.2%，32 子节点约减少 5.6%。空闲与恢复未见改善，详见[完整结果](performance.md#2026-09-25-分类器局部简化)。

- 同一 ledger 中区分真实登记结构与普通内容授权，删除普通内容退出时的临时注册/marker 和逃逸修补，贯通异常/残留出口并修复真实销毁回调导致的永久 cleanup。两轮正式结果：32 子节点正常导航 p50 均值降低约 12.3%，128 子节点降低约 22.6%；完整数据和边界见[性能记录](performance.md#2026-09-27-内容清理迁移正式结果)。

## Next

- P4 操作内清理计划已完成完整回归、三轮原样性能对照及 BRP 渲染/input 验收，保留逐写完整复验。相对 P2，32/128 子节点正常 p50 均值再降低 21.8%/35.6%，128 子节点为 8.0–9.2 ms；小页面和恢复未证明收益。该时间仍不含布局与渲染，不能宣布大型页面性能就绪。后续按实际产品页面帧预算和新 profile 决定，不自动扩展全局缓存、分帧调度或全库重写。
- 本次 Back 功能阻塞及相关回归已闭合；后续按通用使用反馈和实测帧预算决定是否进一步工作。本项目独立开源发布，正式发布前确定许可证与版本。Windows 原生、最低 Rust、物理 GPU 加速和硬件设备输入未验证，不为这些未运行项扩张 MVP 功能或测试矩阵。

## Post-MVP

- queued/streaming/file/cached-asset BSN、hot reload。
- multiple/named/dynamic outlets、retained cache、transition。
- typed route/search、loader/action/async guard/revalidation、平台 adapter。
- 只有真实需求成立后再安排多 Router 产品旅程、后台回收/fairness、公开 authority audit/probe。

## 历史定位

Root、Nested、Reconciliation、History、Recovery 与结构治理是可复用基线，详细完成证据见 [历史索引](superpowers/README.md)。不重跑旧开发计划，也不将旧“Authority closure → MVP”路线延续为新目标。BSN adapter 和普通嵌套示例属于本次 MVP，不再列入 MVP 后。

统一 spec/plan 与范围评审已完成；旧多文档/AST gate 已替换为实际 consumer、LF/CRLF 对照和语义依赖检查，历史协议原位归档。native 工作区已有真实运行路径，仍不能将历史基线或局部通过视为 BSN MVP 验收。
