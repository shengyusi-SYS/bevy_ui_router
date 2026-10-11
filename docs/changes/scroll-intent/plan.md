# 访问级滚动意图实施计划

状态：待实施，与[设计](design.md)配套；核心行为尚未交付。预算统一归[路线图](../../roadmap.md#工作量风险与-roi)，不重复计入最终窗口/组合与发布检查。沿 main，根代理独占 Cargo/index；独立代理只读交叉复核。

## 交付顺序

| 步骤 | 改动接缝与产出 | 验证和提交 |
| --- | --- | --- |
| 1. 分离意图与采样 | `ecs/src/scroll.rs`：从 `HistoryPublicationFacts` 冻结全部可恢复 frame/key 的目标记录；binding 可空，未完成 key 保留原 entry 值。`plugin.rs`：统一 Content 后/Layout 前准备、布局后结算。替换旧 Restoration，不保留平行路径 | 保持默认首布局结果和既有公开旅程；原布局前观察明确排在 Router Prepare 后。增加真实延迟内容/原记录不被初值覆盖回归，写入时序变化单独审查 |
| 2. ready、取消与晚绑定 | `scroll.rs` / 必要的私有子模块：只读 lookup、opaque ID、Commands ready/cancel、一次绑定及应用优先；`ecs/src/lib.rs`、facade `src/lib.rs` 导出。复用 active_scope，不改 Core 选路/history | 先建立早/晚容器、同 Entity 反复 Back、离开未完成恢复和应用优先的公开旅程；删除或不执行关键生产判断时相应回归应失败。公开接口和完整功能一起提交 |
| 3. 消费与文档收口 | 扩展独立 `tests/consumers/bsn.rs` 和真实布局示例；更新导航合同、guide、当前架构/流程及验证入口 | 对最终功能树运行相称完整门禁；窗口与打包纳入统一扩展树验收。实现完成后必要语义归合同，设计/计划退出默认树 |

步骤 1/2 是一个完整功能交付单元，不将只有 marker flag 而无统一意图的中间实现当成完成。不改变时序的内部数据拆分可以单独提交；新时序与完整接口一同交付，默认首布局结果和已有行为回归保持。新 API 名称按实施可行性审阅定型，不追加控制 Resource、ready 组件或生命周期 owner。

## 实施核对

- publication preparation 分配身份、冻结值与数据，tail 仅安装；失败准备不安装意图。
- lookup 不因晚 marker 缺失丢失已冻结请求；控制与执行消费原 ID，重复或旧 ID 不影响新意图。
- current host/frame/Entity/key 复验使用生产事实；采样、离场保存、实际裁剪与取消分开，禁止从初始值重建目标记录。
- 缺布局继续一次放弃，导航终态不等待 ready；应用 ready 不是布局成功结论。

局部运行 `cargo test -p bevy_ui_router --all-features --locked --test scroll_restoration --test retained_navigation --test facade_consumers`。最终执行格式、workspace all-targets/all-features 静态检查、完整 ECS（all-features 与 no-default-features）及 facade 全特性/consumer；若改动 Core publication/history，再运行完整 Core。共享 publication/failure 语义改变则扩大到 workspace 完整门禁，命令和证据边界归[验证指南](../../development/verification.md)。根代理核对 exact staged 文件后提交，不重复无新变化的成功检查。
