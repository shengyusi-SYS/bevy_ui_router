# Bevy UI Router 独立开发资料包

**日期**: 2026-07-06  
**用途**: 面向外部独立开发者的通用 Bevy UI Router 设计交接资料。  
**范围**: Bevy UI 路由框架、ECS 集成、BSN view adapter、平台 adapter、测试与交付边界。  

---

## 资料包目标

本文档集只描述一个通用 Bevy UI Router 框架。它不假设任何具体产品、宿主应用、业务领域、通信协议、主题系统、页面结构或迁移历史。

设计目标是：

1. 借鉴 Web Router 生态成熟模式，建立 route tree、nested outlet、layout route、typed navigation、error boundary 等核心合同。
2. 适配 Bevy ECS 的实体生命周期、schedule、relationship、focus、visibility、pickability 与 World 访问约束。
3. 将 BSN 作为首个 view adapter，而不是 Router core。
4. 保持 Router core 独立、可测试、renderer-agnostic，并允许应用通过 integration profile 注入自身约定。
5. 为外部开发者提供清晰的模块边界、MVP 范围、风险清单和测试矩阵。

---

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [AGENTS.md](AGENTS.md) | 纯 Bevy UI Router 开发环境的 agent 协作规范 |
| [01-BSN-scene-authoring-boundaries.md](01-BSN-scene-authoring-boundaries.md) | Bevy 0.19 BSN 的能力、边界、风险，以及作为 Router view adapter 的设计原则 |
| [02-Web-router-patterns-for-Bevy.md](02-Web-router-patterns-for-Bevy.md) | React Router、Remix、TanStack Router、Next、Vue Router、SvelteKit 可迁移模式和 Bevy 改造点 |
| [03-Core-architecture-and-MVP.md](03-Core-architecture-and-MVP.md) | 通用 Router core、Bevy ECS integration、view adapter、BSN adapter、platform adapter 的架构与 MVP |
| [04-Risks-tests-and-handoff.md](04-Risks-tests-and-handoff.md) | 风险、测试矩阵、阶段路线、交接边界和脱敏检查清单 |
| [05-Non-MVP-capabilities-in-Bevy.md](05-Non-MVP-capabilities-in-Bevy.md) | 非 MVP 能力在纯 Bevy 环境中的真实场景、必要性和架构预留方式 |

---

## 核心原则

1. **Router core 不依赖 BSN**  
   Core 只理解 route、location、branch、frame、state、cache、error 与 diagnostics。

2. **BSN 是 view adapter**  
   BSN 负责描述、resolve、apply Bevy scene，不负责 history、guard、loader、transition 或 back policy。

3. **Bevy ECS integration 是单独层**  
   `RouteHost`、`RouteSlot`、frame root marker、entity ownership、hierarchy mutation、visibility、pickability 和 focus 收敛都属于 ECS integration。

4. **应用约定通过 integration profile 注入**  
   主题、文案、权限、数据服务、持久 shell 策略、业务错误 payload 都不能进入 Router core。

5. **MVP 要小而可验证**  
   首版优先证明 route tree、matcher、branch reconciliation、frame lifecycle、BSN immediate apply、failure cleanup 和 diagnostics。

---

## 建议交付物

外部独立开发应至少产出：

- Public API draft
- State machine diagram
- Error taxonomy
- Test harness
- Minimal examples
- Adapter responsibility table
- Known non-goals
- Desensitization checklist
