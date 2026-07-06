# Bevy UI Router 规划审查处置记录

**日期:** 2026-07-07
**关联文档:** `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
**状态:** 第一轮审查已评估并修订主规划文档

---

## 1. 流程说明

主规划文档先由负责人编写，再交给多个子代理进行只读审查。子代理按职责方向划分，不指定逐项检查清单，不允许直接修改文档。

负责人不直接接受审查结论，而是先做归因和取舍：

- 是否有事实依据。
- 是否触及项目硬约束。
- 是否是局部措辞问题。
- 是否暴露共同根因。
- 是否需要推翻已确认决策或重规划。

只有评估后确认为合理的问题，才进入文档修订。

---

## 2. 第一轮子代理职责

第一轮包含五个审查方向：

- 架构与模块边界审查。
- 生命周期、状态机、ownership 与 focus 审查。
- Public API、extension point 与后续演进审查。
- 测试、验证与交付风险审查。
- 事实性审查。

全部审查均为只读，没有子代理直接修改文件。

---

## 3. 负责人评估

第一轮审查没有发现需要推翻已确认方向的问题。multi-crate workspace、core 独立、BSN adapter 隔离、builder-first API、raw location/search MVP、Phase 0-6 路线图仍然成立。

审查结果显示三个共同根因：

1. 跨层公共类型归属没有足够明确。
2. 生命周期缺少事务模型、pre-apply stale gate、commit-time recheck 和 cleanup authority。
3. 验证策略缺少“不变量 -> 阶段 -> 测试资产 -> 验证命令 -> 退出证据”的追踪链。

因此第一轮修订不推翻架构，而是把主文档升级为更严格的合同文档。

---

## 4. 已修订的问题

### 4.1 公共类型与依赖边界

处置：接受并修订。

修订内容：

- 将线性依赖图替换为 allowed-dependency matrix。
- 明确 `BackIntent`、`BackPolicy`、`BackCapabilitySnapshot` 归 core。
- 明确 platform crate 只把平台事件翻译成 core public API 输入。
- 明确 core `RouteTree` 只保存 renderer-neutral view binding key，不保存 view factory、Bevy component、BSN scene 或闭包。
- 增加 public type ownership table。
- 把 app integration profile 表述收窄为 core 外组合模式。

### 4.2 Facade Default Feature

处置：接受并修订。

修订内容：

- 区分 facade 可导出能力与 default 实际启用依赖。
- 当前规划基线改为 default 不拉入 Bevy。
- ECS、BSN、platform 通过显式 feature opt-in。
- Phase 0 可推翻该基线，但必须记录理由。

### 4.3 当前仓库事实

处置：接受并修订。

修订内容：

- 在主文档开头增加当前仓库事实。
- 明确当前仓库仍是单 crate，尚未拆分 workspace。
- 明确当前根 crate 直接依赖自定义 Bevy git 分支。
- 明确 workspace 结构是目标结构，不是当前已存在代码。

### 4.4 Stale Check 时序

处置：接受并修订。

修订内容：

- 将原本单一的 commit 前校验拆成两层。
- 增加 pre-apply stale/identity gate。
- 增加 pre-commit identity/generation recheck。
- 明确 stale/deferred/queued 结果在写入 World 前必须被拦截。

### 4.5 Transaction 与旧 Active UI

处置：接受并修订。

修订内容：

- 增加 navigation transaction 模型。
- 明确旧 active branch 在新 branch 或 error boundary 成功 commit 前保持可回退。
- 明确失败 cleanup 只清理本次 transaction 创建的 partial output。
- 明确 pending overwrite 使旧 transaction 失效。

### 4.6 Frame Id 与 Generation 规则

处置：接受并修订。

修订内容：

- 增加 frame id reuse rules。
- 增加 generation increment rules。
- 明确 stale check 必须比较 frame id 与 generation。
- 明确 adapter 不能自行构造 frame id/generation。

### 4.7 Cleanup Authority

处置：接受并修订。

修订内容：

- 将 adapter “执行 partial cleanup”的表述改为报告 partial output 和 disposal hints。
- 真正 ECS subtree cleanup 只能由 Router/ECS cleanup API 基于 marker 执行。

### 4.8 Error Taxonomy

处置：接受并修订。

修订内容：

- 增加 `HostOrSlotUnavailable`、`EcsFinalizeFailed`、`CleanupFailed` 或等价错误来源。
- 明确 taxonomy 是对早期文档的收敛结果。
- 明确 error boundary 不隐式修改 history。

### 4.9 Phase 0-3 验证链

处置：接受并修订。

修订内容：

- 增加 minimum verification matrix。
- 增加 test asset plan。
- 将 minimal fake apply-result/test harness 前移到 Phase 2。
- 将 Phase 3 拆成 Phase 3A-3D 小里程碑。
- 补充 pending overwrite、missing host/slot、adapter identity compile-fail 或可见性边界测试。

### 4.10 Guard 范围

处置：接受并修订。

修订内容：

- Phase 0-3 默认只预留 `GuardRejected` error kind。
- 同步 guard 是否实现留到 Phase 0 冻结。
- pending async guard 保持非 MVP。

---

## 5. 延期或未处理的问题

以下问题不阻塞当前规划文档通过，但需要后续处理：

- `docs/README.md` 中指向 `AGENTS.md` 的相对链接可能需要改为 `../AGENTS.md`。
- Bevy 0.19 BSN 的真实 API 状态无法从本地仓库事实确认，当前只能依据项目文档和 `Cargo.toml` 记录。
- CI 与 crates.io 发布风险当前没有本地配置可验证，保留为风险判断。
- RouteStateStore 与 retained entity cache 的完整类型模型延期到 Phase 5；Phase 0-3 只保留术语和 identity 预留。

---

## 6. 第二轮审查结论

第二轮仍由五个方向进行只读审查，目标是确认第一轮 must-fix 是否收敛，不重新发散完整规划。

第二轮结论：

- 架构与模块边界方向通过，无阻塞问题。
- 生命周期、状态机、ownership 与 focus 方向通过，无阻塞问题。
- Public API、extension point 与后续演进方向通过，无阻塞问题。
- 测试、验证与交付风险方向通过，无阻塞问题。
- 事实性审查方向通过，无阻塞问题。

第二轮仅提出几处非阻塞措辞建议，负责人评估后已做小修：

- 将 platform crate 描述从“提供边界类型”改为“提供 adapter glue 和 helper”。
- 将 BSN adapter cleanup 表述改为“请求或触发受控 ECS cleanup API”。
- 将 view crate 的 factory 表述收窄为 view binding registry 或等价 contract。
- 将早期 failure cleanup 范围同步为 transaction 限定。
- 将 platform capability 表述改为读取 core capability snapshot。

---

## 7. 当前结论

第一轮审查结果合理，且问题有共同根因，但不要求推翻架构。已按共同根因修订主规划文档。

第二轮已确认第一轮 must-fix 收敛，当前主规划文档可以进入“是否需要整理重写”的评估步骤。

---

## 8. 整理重写验收

第二轮通过后，五个审查方向均建议进行整理型重写。共同理由是：主规划内容已经正确，但审查后累积了较多补丁式结构，不够适合作为后续 implementation plan 的直接输入。

负责人采纳该建议，但没有重新打开技术决策。整理重写要求是：

- 不改变已通过审查的技术语义。
- 保留 all must-fix 结论。
- 把 Phase 0-3 整理成 implementation-plan 友好的结构。
- 将审查过程保留在本记录中，主文档只保留规划正文。

子代理先新建候选重写文档，负责人随后验收：

- 全局不变量未丢失。
- allowed-dependency matrix 和 public type ownership table 未丢失。
- pre-apply gate、pre-commit recheck、navigation transaction、old active fallback、cleanup authority 和 frame generation 规则未丢失。
- Phase 0-3 均包含目标、前置条件、范围、非范围、产物、测试资产、最低验证和退出标准。
- Phase 4-6 仍保持 roadmap 级别。

验收通过后，候选重写版已替换原主规划文档。
