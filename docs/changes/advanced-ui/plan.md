# 页面保留与滚动恢复实施计划

**状态：设计已由用户确认，实施中。** 行为与范围以[设计](design.md)为准；本页只维护交付依赖、实施接缝与验收进度。

按结果顺序交付。Core 决定 history 与 lifecycle，ECS 持有实体、滚动数据和物理 owner；沿用现有 publication/cleanup 协议。只在实际改动处拆分私有模块，不整体重写 navigation。默认关闭新增能力时保持现有行为。

闲置 host loss 的完整缓存移交已闭合；接下来单独提交局部结构收敛，再扩展缓存策略：

- Core 在 `engine/retained.rs` 统一 retained 库存核对和 preserve/retire 分区；normal、idle 与失败入口复用这些准备，保留各自已有 history/host owner 和 continuation，不为每种失败增加独立缓存状态机。
- ECS 在 `retained.rs` 统一原 owner/physical payload 的准备、移交与返还；选定内容进入既有 cleanup manifest/physical forest。retained publication 编排移入私有模块，主 navigation 仅保留流程分派；不用机械拆文件代替职责收敛。
- 验证围绕用户旅程及关键 owner 不变量，覆盖一次代表性准备拒绝和真实写后变化。相同语义不叠加多份低层反例矩阵；已有行为回归不因重组而删除。LRU 先用标准容器，避免手写链表或引入不能减少生命周期复杂度的依赖。

## 1. Canonical history 的受控只读交接

- [x] 从现有私有 `HistoryEntryId` 投影 opaque entry key；在 integration 提供 canonical entries 和已准备 publication 的 source/target/保留 entries/Back 恢复事实。投影不签发 authority，不向 ECS 开放 history mutation。
- [x] Push 新建、Replace 换身份、Back 选择既有身份；error/fallback 无成功目标 entry。所有交接事实来自原 history capability 和已准备 publication，不能从提交后 cursor、URL 或 Entity 重建。
- [x] 回归重复 URL、等价 Replace、Back、forward 截断、失败不推进、跨 Router 身份与放弃 preparation 不改变 history。

实现入口：Core `history_authority.rs`、`engine.rs`、`integration.rs`。验证先运行相关 history/integration 回归，再运行完整 Core crate。

## 2. 独立可用的 entry 滚动恢复

- [x] ECS 新增私有 `scroll.rs`，提供显式容器 marker；记录关联 entry、route 复用身份与 frame-local key，位置采用实际布局的逻辑像素。公开 marker 经 ECS 与 facade/prelude 导出。
- [x] 在 normal 来源失去有效内容前捕获位置；覆盖 normal、Back、projectionless error/fallback 的 publication 路径。失败保留原页面时不恢复、不建立成功目标记录。
- [x] Back 对 fresh 与 preserved prefix 准备恢复；Push/Replace 保持实例当前值。先冻结 exact frame/generation、容器归属与写集合，再准备写入；publication tail 只执行已借用的不可失败原位 mutation，不插入组件或执行查询。
- [x] 在正常 `UiSystems::Layout` 后单次结算裁剪结果；失效请求丢弃，不通过固定延迟、额外 layout 或无限 polling 等待。缺少实际 layout、冲突 key 和非法归属给出有界诊断。
- [x] 按 canonical entry 保留集合回收记录；host loss 保留仍有效的访问记录。只为实际 restoration 请求保存短期 exact Entity 映射。
- [x] 公共旅程使用真实 UI layout 验证二维/多容器、重复访问、preserved prefix、等价 Back、缩放、内容缩短再增长、缺省/冲突 key、失败来源与 host 重新接纳。

实现入口：ECS `navigation.rs` 的 normal/Back/projectionless publication、`presentation.rs` 的受控借用、`plugin.rs` 的布局后调度及新 `scroll.rs`；facade `lib.rs` 和公共旅程。此阶段独立完成，不等待缓存。

## 3. Retained 共存机制验证

- [x] 沿现有 slot/mount/frame-local/registry 边界验证独立 retained role 与 exact claim 能在同一 child slot 下和 active 共存；原请求不持有长期 outstanding capability。
- [ ] 覆盖 active 正向输入、retained 拒绝、无关缓存 protected、hook 写后 drift 拒绝和 parent cleanup 责任交接。机制证据必须经过生产 predicate，不能直接注入分类结论。
- [ ] 若探针揭示设计不可行，先收敛到有证据的 owner/角色协议再继续完整缓存；不通过放宽所有 child-set 检查获得通过。

实现入口：Core `frame.rs`/`engine.rs`/`integration.rs`，ECS `registry.rs`、`mount.rs`、`frame_local.rs`、`presentation.rs`、`input.rs`、`cleanup.rs` 与 `branch_transaction.rs`。此阶段独立审查后进入缓存实现。

已闭合连续多个 inactive leaf 与 active 共存、撤销输入/focus、离开父布局以及闲置 host 丢失后清理并重新接纳的完整旅程。当前先收敛库存/移交准备，再补前台失败与 authority 失效时的结算，之后完成命中、LRU 与组合验收。

## 4. 同父 leaf 缓存与完整责任交接

- [ ] Route 显式启用 keep-alive，Router 设置 inactive leaf 容量；默认关闭，容量零禁用。Core 决定 retain/reactivate/dispose，ECS 私有模块持有长期 retained owner 与实体。
- [ ] 按 exact parent frame/generation、outlet、slot binding、host incarnation 和 route/view/params/raw search/raw fragment 判定；只缓存 normal leaf，保持 fragment 替换与 search 重建合同。
- [ ] 离场同次撤销 pointer 历史 hover/已开始手势、keyboard/gamepad 和 focus，再隐藏并退出布局。提供一个最小活动资格查询入口，应用可据此决定后台工作。
- [ ] 命中保留 Entity、局部组件及 handles，不重复 factory/apply；在 Ready 前完成 claim rebind、owner 组合、输入及恢复准备。其他缓存始终 protected。
- [ ] 按失活时刻维护 LRU；淘汰、parent/outlet/slot 失效、host loss/readmission 经明确 cleanup owner 结算。cache 自身失效与 current authority loss 分别处理；准备或写后发布失败返还原 owner 或有界隔离清理。
- [ ] 验证 A→B→A 同 Entity/状态、factory 次数、禁用/LRU/淘汰重建、稀疏 binding、多缓存共存、动态内容、命中失败、parent/host loss、terminal owner 收敛与后续合法请求。

实现入口沿第 3 阶段接缝扩展，缓存策略和物理 holder 按职责拆私有模块；公开配置与活动查询经 facade/独立 consumer 验证。

## 5. 组合、真实输入与交付

- [ ] 同 key 多 entry 各自保存位置，分别覆盖 cache hit、miss 与淘汰；证明共享 Entity 不会覆盖另一 entry 的位置。
- [ ] 通过真实 pointer/显式 gate 管线验证正向命中、失活拒绝、已开始手势撤权、focus 释放、重新激活以及其他有效目标保留。
- [ ] 扩展示例与独立 consumer，经应用内 BRP 验证窗口；实际记录默认关闭、miss/hit、idle、淘汰和连续往返的性能及活实体/retained owner 稳定性。计时包含 cleanup，不将 hit 收益冒充 miss 优化。
- [ ] 将已交付行为归入 overview、所属专题合同、架构与接入/验证指南；保留未运行环境的准确边界。计划完成后退出默认文档树，由 Git 追溯。

## 验证与协作

每个语义结果先建立能暴露缺失行为的回归，再实现并独立审查。Rust 变更执行格式、静态检查与相称测试；lifecycle/failure/terminal 变化运行受影响完整 crate 和最高层 gate。

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo test -p bevy_ui_router_ecs --no-default-features --locked
```

公开旅程、输入/focus、facade/独立 consumer 的真实 targets 与独立窗口命令见[验证指南](../../development/verification.md)。共享 Cargo target 同时只有一个验证负责人；提交由主代理负责，使用精确文件列表并检查 staged diff。

query 更新、标准键盘插件接线和代表页面 miss 性能在本轮完成后分别按需求评估；浏览器、异步 factory、动画与整分支缓存仍在本轮范围外。
