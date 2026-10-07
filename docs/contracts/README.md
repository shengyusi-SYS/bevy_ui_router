# 专题合同

本目录按变化原因划分当前行为规范，各页共同构成产品合同。开发时先读相关主题，再沿链接检查相邻边界；文件数量不固定，同一条规则只维护一个归属。

| 主题 | 定义什么 | 改动时关注的关联 |
| --- | --- | --- |
| [导航与观察](navigation.md) | 匹配、location 复用、请求/history、snapshots | view 重建会触及 Scene；发布与终态顺序见 lifecycle |
| [Scene 与挂载](scene.md) | driver 注册、同步 Scene profile、声明图创建、default outlet | host/identity/失败责任见 lifecycle；focus 意图见 input-focus |
| [生命周期与清理](lifecycle.md) | host 接纳、identity/owner、publication、failure/recovery、cleanup | 导航结果见 navigation；撤权规则见 input-focus |
| [输入与焦点](input-focus.md) | Ready 边界、pointer 历史手势、focus 意图与派发 | Scene 创建与 lifecycle publication 提供 source 和生效时机 |

产品目标与当前能力见[概览](../overview.md)，实现关系见[架构](../architecture/README.md)，按功能定位源码与测试见[开发入口](../development/README.md)。本文与专题页描述当前合同；未实施方案和执行计划不混入这些文件。

## 共同职责边界

Core 独占匹配、请求顺序、逻辑 frame/generation、history、恢复选择和逻辑 publication。View 只提供 renderer-neutral 的 key、descriptor、registry 与只读 effect/input。

ECS 拥有 host/slot/frame 的物理绑定、Scene 执行编排、staging、输入/focus、publication preparation 和 cleanup。BSN 提供 Scene factory、普通 outlet 声明与 build error，不拥有导航或物理 authority。

Facade 组合常用 API 和 opt-in features，不导出可伪造的 owner、registry writer 或 commit/report 权限。新的 renderer integration 可以消费受控合同，当前不建设通用 renderer 插件框架。

公共签名由源码/rustdoc 提供，本文不维护导出符号黄金清单。修改合同需同时核对受影响行为、架构与验证；内部类型调整不能静默改变公共语义。
