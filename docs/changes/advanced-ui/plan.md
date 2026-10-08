# 页面保留与滚动恢复实施计划

**状态：设计已由用户确认，实施中。** 行为与范围以[设计](design.md)为准；本页只维护交付依赖、实施接缝与验收进度。

按结果顺序交付。Core 决定 history 与 lifecycle，ECS 持有实体、滚动数据和物理 owner；沿用现有 publication/cleanup 协议。只在实际改动处拆分私有模块，不整体重写 navigation。默认关闭新增能力时保持现有行为。

entry 滚动、A→B→A 命中、LRU 淘汰，以及闲置、normal 提交前、pending failure 和 postcommit 期间 host loss 的缓存结算已闭合。后续按以下顺序收尾，不扩张功能范围：

1. 完成缓存自身失效与一个活动资格查询入口。
2. 验证多 entry 滚动与缓存组合、真实输入/窗口和性能，更新合同及 consumer。

近期问题共同来自“当前物理分支就是完整生命周期库存”的旧假设。调整实施接缝，将完整库存选择、原 owner 移交和物理 manifest 组合集中到现有 retained/cleanup 职责；不同来源的 history、失败 continuation 和 cleanup receipt 仍由原合同持有：

- Core 在 `engine/retained.rs` 集中 retained 库存和移交 owner 的 exact 核对；normal 按 next scope、idle 按 exact host 明确分区，保留各自已有 history/host owner 和 continuation，不为每种失败增加独立缓存状态机。
- ECS 在 `retained.rs` 统一原 owner/physical payload 的选择、预分配、移交与返还；选定内容进入既有 cleanup manifest/physical forest。normal 与 projectionless publication 分别在私有模块编排，共用 publication holder，主 navigation 仅分派到对应入口。
- 验证围绕用户旅程及关键 owner 不变量，覆盖一次代表性准备拒绝和真实写后变化。相同语义不叠加多份低层反例矩阵；已有行为回归不因重组而删除。LRU 复用现有标准容器；本轮不引入新的状态机库、通用恢复或后台暂停机制。

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

独立 retained role 与 exact claim 已进入生产路径；连续多个 inactive leaf 与 active 共存、输入/focus 撤权、离开父布局、闲置 host 丢失及重新接纳已有完整旅程。长期 owner 不再挂在原请求上。这些机制已通过独立审查和完整 crate/public gate，后续只补命中、失效路径及组合行为暴露的缺口，不重做探针或另建反例矩阵。

## 4. 同父 leaf 缓存与完整责任交接

命中切片沿三处接缝落实：Core 以本次 task 冻结只读命中/next 事实，在 late prepare 中构造独立 next work 并沿原 history/publication 接受；ECS 短借用原缓存完成 mount/closure/claim/输入准备，产出只读准备包，接受后才移动原 payload；driver 在 factory/apply 前分派此路径。原 frame/Entity/slot 保留，拒绝返还原 task、plan 和 owners；缓存不提前转成 fresh PreparedInert。leaf 自身的空 child slot 由 Core 在 retain 时保存并在 hit 时恢复。

- [x] Route 显式启用 keep-alive，Router 设置 inactive leaf 容量；默认关闭，容量零禁用。Core 决定 retain/reactivate/dispose，ECS 私有模块持有长期 retained owner 与实体。
- [ ] 按 exact parent frame/generation、outlet、slot binding、host incarnation 和 route/view/params/raw search/raw fragment 判定；只缓存 normal leaf，保持 fragment 替换与 search 重建合同。
- [ ] 离场同次撤销 pointer 历史 hover/已开始手势、keyboard/gamepad 和 focus，再隐藏并退出布局。提供一个最小活动资格查询入口，应用可据此决定后台工作。
- [x] 命中决策发生在 fresh factory/Scene apply 前，消费本次 Core task 与原缓存 owner，保留 Entity、局部组件及 handles；在 Ready 前完成 claim rebind、owner 组合、输入及恢复准备。其他缓存始终 protected。
- [x] 按失活时刻维护 LRU；命中移出 inactive 库存，再失活时加入队尾。满容量淘汰最旧实例，原 owner 经既有 cleanup token 结算；其他缓存与 app-owned host 保留。
- [ ] 完成 parent/outlet/slot 与 cache 自身失效的剩余分类。cache 自身失效与 current authority loss 分别处理；准备或写后发布失败返还原 owner 或有界隔离清理，安全结算后可 fresh build。
- [ ] 验证 A→B→A 同 Entity/状态、factory 次数、禁用/LRU/淘汰重建、稀疏 binding、多缓存共存、动态内容、命中失败、parent/host loss、terminal owner 收敛与后续合法请求。

实现入口沿第 3 阶段接缝扩展，缓存策略和物理 holder 按职责拆私有模块；公开配置与活动查询经 facade/独立 consumer 验证。

命中已有通用 driver 连续往返及公开 Native Scene 旅程，证明原 Entity/自定义组件值保持，factory 与 Scene apply 均不重复。Core 覆盖自身空 slot 恢复及真实 source 准备拒绝返还。LRU 公开旅程串起多次 miss/hit、满容量淘汰与淘汰后重建，验证选定对象清理、其他缓存状态及非活动输入拒绝；Core 准备与复验共用分区算法，ECS 退休缓存按当前完整 slot children 冻结父边界，不沿用首次 mount 的历史 sibling 快照。

normal 提交前与 pending failure 期间 host loss 已验证完整库存、原 owners/payload 与 registry retirement 交给同一 authority batch，保留 history 和 app-owned host，新 epoch 可接纳且不重放中断请求。普通 pending cleanup 保留合法 active/cache 与原 failure continuation；Core 验证真实 trigger revision 变化时返还原 owners。

fallback 已验证旧父 scope 与缓存在同一普通 cleanup batch 退役，保留原 Apply 失败、history 与 host，后续导航成功。BoundError 验证 surviving parent 的 exact frame/slot 与缓存保留、非活动输入拒绝和后续合法请求，没有套用 fallback 的全退休政策。

postcommit 已由一次真实多层 scope 导航验证两类库存同时结算：已绑定原 normal token 的退休项留在原 ledger，Live 缓存及原 owners/payload 进入 authority 第二批，联合完成分别结算两个 exact token，保留已提交结果与 history；app-owned host 保留并可重新接纳。完整库存归入既有 receiver projection，首次执行与 retry 共用接线，拒绝沿原 holder 保有唯一 owner。延迟 proof 删除包只移除 exact 选定项，双 Router 旅程验证不会覆盖其他 Router 后续建立的缓存。

永久协议矛盾的 residual/transfer 保持原 owner；这类保存句柄的路径不算可恢复结算，也不扩展为新的自动恢复需求。

## 5. 组合、真实输入与交付

现有独立滚动和原生输入/focus 回归尚未启用 keep-alive，不作为以下组合验收的完成证据。活动资格也不能直接等同输入 gate：活动布局可能没有交互目标，查询应基于现有 active/claim 事实。

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
