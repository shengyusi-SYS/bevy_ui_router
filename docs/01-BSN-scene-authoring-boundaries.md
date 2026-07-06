# 01 — BSN Scene Authoring Boundaries

**日期**: 2026-07-06  
**目标**: 说明 Bevy 0.19 BSN 对通用 Bevy UI Router 的价值、边界和风险。

---

## 1. BSN 是什么

BSN 是 Bevy 0.19 中面向 scene authoring 的声明式能力，用于描述、组合、解析并生成 ECS entity、component、relationship 与 asset dependency。

它适合回答：

- 一个 UI 子树由哪些 entity 和 component 组成。
- 子节点、relationship、局部 observer 如何声明。
- 多段 scene 如何组合、patch、复用。
- 依赖 asset 的 scene 何时可以 resolve、apply 或 spawn。

它不负责回答：

- 当前 location 应匹配哪条 route。
- history 如何 push、replace、back、forward。
- loader、guard、action、revalidation 如何调度。
- route error boundary、transition、cache、diagnostics 如何管理。

结论：BSN 应作为 Router 的 view adapter 或 scene authoring layer，而不是 Router core。

---

## 2. 适合 Router 借用的能力

BSN 适合承担 Router 中的视图生成职责：

- route view 返回 code-driven `bsn!` scene。
- layout、slot、outlet 的 UI 结构通过 scene composition 表达。
- `SceneList` 表达多个 root 或 relationship list。
- `Children` 等 relationship 声明嵌套 UI 子树。
- patch 用于 spawn/resolve 阶段的字段级组合。
- `SceneComponent` 封装复杂 widget。
- scene resolve/apply error 映射为 view build/apply failure。
- queued spawn 作为后续 asset scene 能力基础。

合理边界：

```text
MatchedBranch
  -> Router 创建 route-owned host/frame
  -> BSN adapter 构建 scene description
  -> Renderer apply/spawn scene
  -> Router 提交 frame lifecycle 状态
```

---

## 3. 不适合由 BSN 承担的职责

BSN 不应承担：

- route tree 定义与 matcher。
- location source 与 history stack。
- push、replace、back、forward 语义。
- branch reconciliation 与 common prefix layout 保留。
- route frame ownership、generation、cleanup。
- async loader、action、guard、revalidation。
- stale result 检查与 cancellation policy。
- error boundary 与 fallback selection。
- transition、focus、visibility、pickability 收敛。
- route-local serializable state。
- retained entity subtree cache。
- diagnostics 与 contract testing。

这些职责应由 Router core、Bevy ECS integration 或 platform adapter 管理。

---

## 4. `spawn_scene` 与 `queue_spawn_scene`

`spawn_scene` 是立即 resolve + spawn 的路径。它适合首版 code-driven `bsn!` route view，因为失败可以同步映射为 view apply error，成功后也更容易提交 route frame 为 active。

`queue_spawn_scene` 会先返回一个 entity，再等待依赖加载，并在后续 schedule 中 apply scene。这个返回值只代表临时 entity 已创建，不代表 route view 已可见、可交互或已完成挂载。

因此 Router 必须显式区分：

- `Resolving`：scene 还在等待依赖或 apply。
- `Applying`：正在写入 ECS World。
- `Active`：view 已挂载，visibility、focus、pickability 等状态已收敛。
- `Error`：resolve/apply 失败，进入 fallback 或 boundary。
- `Leaving`：旧 frame 正在退出、隐藏、清理或缓存。

使用 queued spawn 时还必须绑定 frame id、generation 或 load token。任何异步完成结果写回前，都要确认目标 route frame 仍然有效，避免旧 scene 覆盖新 route。

---

## 5. Patch / Template / Observer / SceneComponent 风险

BSN patch 是 spawn/resolve 阶段的描述合并，不是 runtime reconciliation。它不能替代 branch diff、局部 UI 更新或 retained subtree cache。

Template 能访问 spawn context、resource、当前 entity 与 scene entity reference，但不应承载 async loader 逻辑。异步任务不应持有 `&mut World` 跨 await，结果应通过明确的 apply phase 写回，并做 generation 校验。

Scene 内声明 observer 适合局部 UI callback，但不适合承载 route lifecycle 副作用。guard、loader、action、retry、revalidation 等逻辑应集中在 route module 或 Router task contract 中，便于取消、测试和诊断。

`SceneComponent` 适合封装 widget，但不能替代 route frame root 的所有权标记。Router 仍需要 owner marker、frame id、generation、slot 信息与 cleanup 策略。

---

## 6. 首版策略

首版应采用 code-driven `bsn!`：

- 类型检查直接来自 Rust。
- failure cleanup 更容易验证。
- 不需要引入 `.bsn` asset loader 假设。
- 不需要提前定义 hot reload 的 entity replacement 策略。

asset scene 与 hot reload 可作为后续能力：

- route-level lazy scene asset。
- scene dependency pending 状态。
- asset missing / resolve error 映射。
- hot reload 后重建 view，同时保留 route state。
- 明确旧 frame 的隐藏、替换、销毁或缓存策略。

hot reload 不应被理解为“对旧 entity tree 直接 patch”。更稳妥的模型是重新构建 view frame，并由 Router 决定哪些 route state 或 cache 可以保留。

---

## 7. BSN Adapter 设计原则

1. Router core 不依赖 BSN。
2. BSN adapter 只负责 view description 到 ECS scene 的转换。
3. Route root 由 Router 创建并标记。
4. Frame lifecycle 由 Router 提交。
5. Branch reconciliation 保留 common prefix layout。
6. Async 结果必须 stale-safe。
7. Error 要按来源分类。
8. App-specific metadata 通过 extension 注入。
9. Diagnostics 是核心能力。
10. BSN 是首选 view adapter，不是唯一 view adapter。
