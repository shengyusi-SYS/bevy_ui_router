# 发布与后续范围

功能范围以 [专题合同](contracts/README.md) 为准；已有运行证据及平台限制见 [验证指南](development/verification.md)。已完成的 MVP 计划不重新启用；新发现的问题按下述优先级处理。

## 当前优先级

生产参考反馈只用于发现通用场景；问题必须在本项目公开入口独立验证，不引入参考应用的页面、业务状态或返回逻辑。

### 先修复保留与不保留页面混用

同一有效父布局下，A leaf opt-in 保留、B leaf 不保留时，A→Push B→Back A 或 Push A 已在本项目最小 Scene 中复现停滞于 `ApplyingFrame`；两页都保留的对照可完成。这是现有导航合同内的缺陷，修复及验收优先于发布和新增能力。

根因定位在 ECS：普通离场 cleanup 沿用 fresh successor 的历史挂载假设，无法接受仍在共享 slot 中的 retained successor；准备拒绝又未形成有界结算。局部整理应独立冻结当前 exact mount 事实和完成本轮清理后的 next prior，保持 target/protected 分区、原 owner 与原 history 交接。只绕过 manifest 拒绝仍可能把已删除 sibling 留进 next prior，不能算修复。

完成条件是混用场景的 Back/Push、命中后的下一次 fresh 导航均经普通 `App::update` 收敛，并保持实例、history、factory 调用和输入/focus 语义；保留双缓存对照。沿现有 preparation/publication/cleanup 管线修复，不新增缓存状态机，也不要求所有 route 开启保留。按[验证指南](development/verification.md)执行受影响完整 crate 和 public journey/consumer。

### 再确定延迟滚动恢复的最小合同

当前滚动恢复只选择 publication 前已存在的标记容器，并在首个正常布局后单次结算。Back 后才附加 marker 不补恢复已独立验证；随后内容增长不回跳也是[现行合同](contracts/navigation.md#按访问恢复滚动)，属于接入窗口与能力范围。

稳定容器可提前声明时沿用现有接入。确需延迟创建时，下一步先设计显式就绪后的单次恢复：冻结本次 entry 的原记录，区分实例归属、容器就绪与真实 layout，明确应用滚动意图优先及离开/取消边界。它不阻塞导航请求，不自动猜测高度稳定，也不扩成 loader、无限轮询或业务状态仓库；设计确认前不改变默认行为。

raw search 变化重建整链、仅同父 normal leaf 可保留仍是明确范围；此次反馈不足以直接改写复用规则或增加整分支缓存。性能优化继续由代表页面预算决定，不先于已复现的导航阻塞。

## 首次发布

当前已有同步 BSN、嵌套导航、history、错误恢复、输入/focus、独立消费者、窗口示例和打包检查。仓库尚未正式发布 crates.io；具体许可证和发布版本尚待确定，当前不授予开源许可证。

发布时需要完成：

1. 确定授权方式与发布版本，按决定补齐包元数据。
2. 对最终发布树执行验证指南中的功能、feature、打包消费者检查；窗口冒烟覆盖实际发布的示例。
3. 核对发行说明中的能力与限制，然后按 crate 依赖顺序发布。上传是单独的发布动作。

Windows 原生、最低 Rust、物理 GPU 与硬件输入尚未验证。它们是现有证据边界；仅在实际目标环境或已发现的平台差异要求时追加验证，不据此扩张功能或建立无目的的平台矩阵。

## 按实际需求选择

[性能记录](performance.md)显示大内容页面仍有逐写复验成本。下一项是否优化，应由代表页面的实际帧预算和新 profile 决定；现有小窗口示例不证明复杂页面性能就绪。

未交付能力统一列于 [当前产品范围](overview.md#核心需求与当前范围)，目前未排期，不构成首次发布的隐式前置。

增加能力前先证明消费者需求，说明与现有生命周期合同的关系，再确定最小实现及验收。

按 history entry 的显式滚动恢复和同父 leaf 保留已交付，并具有组合、独立 consumer、窗口与性能证据；新发现的混用缺陷以上述修复项为准。行为归[导航合同](contracts/navigation.md)，方法和边界归[验证](development/verification.md)与[性能记录](performance.md)。已完成的设计/实施材料退出默认文档树，由 Git 追溯。

query 更新时的布局保留与标准键盘插件接线，分别按消费者需求确定最小范围；它们不作为本轮交付或首次发布的隐式前置。浏览器 URL/history adapter、异步 factory 与过渡动画仍未排期。
