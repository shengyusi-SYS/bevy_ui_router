# 原生输入接缝候选

这是依赖选择前的可执行候选，尚未应用到生产 workspace；不是另一条长期实现。选型理由归[输入评估](../../docs/changes/navigation-experience/assessment.md#公开的通用输入资格接缝)，现行能力归[输入合同](../../docs/contracts/input-focus.md)。采用后将代码归位并删除本候选，避免长期维护两份实现。

## 重建与验证

在仓库根目录运行：

```sh
python3 scripts/check-native-input-candidate.py
```

需要 Python 3.12+、Git、Rust 工具链及已缓存的官方 Bevy 0.19.1 源码/依赖。脚本校验修改文件的官方原文摘要，从 `sources.json` 固定的 Git 基线重新展开 Router，在全新的 `target/native-input-candidate-*` 目录应用两份补丁，再顺序运行格式、UI 接缝、完整 ECS crate 和公开组合旅程。日志留在该目录；`--prepare-only` 只重建供审阅。脚本不改变当前 checkout、index、分支或生产依赖，不联网、不上传。

- `ui.patch`：对官方 `bevy_input_focus` 和 `bevy_ui` 的扩展。
- `router.patch`：公开桥、旧反射/事件处理替换的退休及真实消费测试。
- `tests/`：独立 UI 接缝验证；不是 Router lifecycle owner 的测试替身。

## 已证明的合同

UI 通过同一即时资格谓词处理 Tab 候选、AcquireFocus、普通设焦和 dispatch 起始目标；排序、分组、modal、几何选择和合法目标的冒泡继续由 UI 负责。被拒请求不改变有效焦点；同值 set、空 clear 仍覆盖旧请求，内部撤权保留最后应用意图。

Router 在真实 Scene apply 时冻结普通 source 数据，原 owner 随 frame 移动；发布前再次核对原 source、完整操作身份、当前目标归属及应用资格。focus/FIFO 的全部准备与绑定在 Ready 前完成，tail 只替换已分配状态，退休状态在发布后释放。无实际修改时不改变焦点资源的 change tick。

活动证明复用 RouteActivity 的 exact host、claim、挂载链和实时层级；借出 RouterNavigation 时使用短暂的只读期望，不能由此重建 owner。撤权仍受原 target/protected 分区控制，不添加全局清焦补偿。

## 采用成本与证据边界

当前验证范围至少涉及两个配套 UI crate，其它第三方 setter 消费者仍需迁移评估。`InputFocus::set` 改走 World-aware `FocusWriter`/`set_focus`；依赖生命周期随之改变，`bevy_ui` 自动方向导航也要迁移。`FocusWriter` 声明整个 World 的只读访问，应用同时修改其它状态时需要有序拆分或 ParamSet；它会影响调度并行度。Router 借出期还要复制少量活动归属事实，尚未做代表性能对比。

候选验证保持 0.19.1，其可执行接入方式是消费方根 manifest 同时 patch 两个 crate。Cargo 不传播依赖库内的 `[patch]`，所以不能把库内 patch 当作下游已完成装配；正式依赖来源、版本维护及消费方要求仍需裁决。独立 UI 扩展若只能复制完整原生算法，没有消除维护责任；当前优先保留这份最小上游扩展作为可审阅接口提案，再决定维护来源。

已有证据覆盖官方焦点单测、同步操作/来源/准备协议、完整 ECS crate，以及真实 Router Tab/dispatch、焦点、pointer 手势、活动查询、保留、history/滚动公开旅程。当前生产 renderer 是 BSN；私有 Data executor 不构成新增 renderer 承诺。独立下游装配、打包、窗口、硬件输入和候选性能尚未验收，不以这些局部结果宣称原生输入正式交付。
