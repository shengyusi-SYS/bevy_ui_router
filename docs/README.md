# 文档与开发入口

文档围绕后续功能开发组织：先确认需求和模块边界，再按主题读取合同、定位实现与验证。当前基线由多个层级共同组成；同一条规则有明确归属，不限制 spec 的文件数量。

## 从哪里开始

| 目的 | 入口 | 职责 |
| --- | --- | --- |
| 了解核心需求与当前范围 | [产品概览](overview.md) | 产品要解决什么，当前交付到哪里 |
| 理解系统关系和定位模块 | [架构总览](architecture/README.md) | crate 依赖、职责、状态维度和源码地图 |
| 理解物理结构与执行顺序 | [运行机制](architecture/runtime.md) | frame/outlet 结构、导航时序、Bevy 调度 |
| 修改跨层生命周期机制 | [生命周期执行协议](architecture/lifecycle.md) | action/owner 交接、准备分流与内部模块定位 |
| 查阅当前行为和边界 | [专题合同](contracts/README.md) | 导航、Scene、生命周期、输入/focus，按变化原因划分 |
| 开发或修改功能 | [开发指南](development/README.md) | 按功能定位合同、源码、测试；组织新设计与计划 |
| 接入应用、运行示例 | [接入指南](guides/integration.md) | 实际装配步骤与可运行示例 |
| 调试窗口交互 | [窗口调试](guides/debugging.md) | 应用内 BRP 输入、布局查询和 renderer 截图 |
| 选择验证及发布检查 | [验证指南](development/verification.md) | 检查入口与已验证/未验证边界 |
| 分析性能 | [性能基线](performance.md) | 计时方法、已有结果与测量局限 |
| 理解设计原因和经验 | [工程经验](development/design-lessons.md) | 可复用因果与取舍；不另立产品要求 |
| 确认发布与后续安排 | [路线图](roadmap.md) | 当前待办与选题依据；不复制合同 |

首次了解项目可按“概览 → 架构 → 相关专题”阅读；修复或开发某项功能直接从开发指南进入，无需通读全部文档。

## 当前基线与未来设计

概览定义范围，架构解释当前实现，专题合同定义现行行为；使用指南与验证提供操作和证据入口。明确的用户决策与 [AGENTS.md](../AGENTS.md) 的优先级保持不变。

未来功能可按职责建立多份设计/spec 和实施计划，具体组织见[开发指南](development/README.md#新增功能与文档演进)。未实施方案明确标记状态并独立存放，不因进入仓库而成为当前合同。设计落地后同步相关主题，不把新行为散落在补充日志中。

公共 API 的精确签名以源码/rustdoc 为准，示例引用实际编译运行的源文件。跨主题通过链接说明依赖，不复制整份协议或建立互相证明的 Markdown/API 清单。

## 历史与参考

已完成阶段的 plan、review log、旧 Target/Matrix/Fixture/Allowlist/Golden/Profile 和过时研究已退出工作树。必要语义和经验归入对应主题；历史过程通过 `git log -- docs` 与 `git show <revision>:<path>` 追溯，不作为新开发的默认输入。

[参考研究](reference/router-benchmarks.md)是有版本边界的材料，不定义当前需求；[性能数据](data/performance/README.md)保留原型、候选与比较质量，不冒充当前源码的新验收。文档描述独立、通用的 Bevy UI Router，不混入其它项目的状态。
