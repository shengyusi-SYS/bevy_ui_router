# Roadmap

> **Current-stage status:** Recovery PASS at verified implementation candidate `494ae6b`. The completed profile keeps eight Authority-owned exclusions, six global active deltas, three Recovery authority rows, and catalogue 27. Journey D and Scenario E are complete; only Journey G's Recovery-owned foreground/readmission rows are complete. Background old-lease reap, full Journey G, Authority closure, and MVP PASS remain open.

**状态**: Phase 3B.1 `Recovery PASS` + structural governance complete；Now = Authority closure

**更新时间**: 2026-08-31

## 已完成

- [x] **Root push**
  - facade → `App::update` → snapshot 的真实垂直切片
  - cleanup、fallback、diagnostics、重复 push、公开旅程与 Root facade completion gate
  - exact Bevy revision、tracked lockfile、物理 UI/picking、authority catalogue 与独立复审

详细边界见 [Root Push Contract Profile](root-push-contract-profile.md) 与 [Root Push Review Log](superpowers/plans/2026-07-11-root-push-review-log.md)

- [x] **Nested push**
  - 真实 parent outlet、sealed child mount 与 root-to-leaf apply
  - whole-branch preflight、原子 publication、frame-local input/focus 与 exact cleanup
  - Journey A、opaque outlet/content audit 与 Root 20 + Nested 3 authority gate

详细边界见 [Nested Push Contract Profile](nested-push-contract-profile.md) 与 [Nested Push Implementation Plan](superpowers/plans/2026-07-16-nested-push-implementation-plan.md)

- [x] **Reconciliation**
  - Core logical roles、ECS physical splice、exact cleanup 与 scoped pre-commit authority settlement
  - Journey B、nested fragment、equivalent physical no-op 与 common-prefix-preserving suffix splice
  - Task 7 prerequisite `2e5cb3d`，public evidence `5806c59`，Task 8 focused/full gates 与四路独立复审

- [x] **History**
  - 推荐 facade `replace`、late-bound relative `back`、canonical history identity 与 publication-last history commit
  - C1–C3 成功/失败/queued Back 公开旅程，pre-commit authority settlement、ABA/cross-incarnation/stale replay 与 exact cleanup owner closure
  - implementation `be25dbe` 至 `e001345`，contract activation `f0fcc2e`，authority catalogue 24，workspace nextest 1108/1108

- [x] **Recovery**
  - boundary-bound error presentation、post-commit `L + A + H` compound settlement、stable unavailable、fresh-epoch re-admission 与 sealed stale-output cleanup
  - Journey D、Scenario E 与 Journey G Recovery-owned foreground/readmission rows；无 binding nested Push 保持 cleanup → `RouterFallback`
  - R2–R4 range `d889942..494ae6b`，six active deltas，authority 27/27/27，workspace nextest 1222/1222

- [x] **结构治理**
  - Core `engine::post_commit`、ECS `cleanup::capacity` 与 `navigation::authority_isolation` 已形成稳定协议 child module；巨型内联测试已机械迁移
  - 提交范围 `211874a..2f2cb42`；行为、公共 API、测试名、feature 与过滤入口不变
  - facade closed-world、authority 27/27/27、workspace nextest 1222/1222、doctest 28/28、全目标检查和最终独立复审通过

## Now

1. **Authority closure**
   - multi-router stress、完整 opaque audit、lease authority、background reaper 与 cleanup replay
   - `RouterInputProbes`、真实 input isolation journey 与 old-epoch quarantine 安全收敛
   - 新能力进入独立 quarantine、reaper、audit、input-probe 模块，不继续堆入 `navigation.rs`

## Next

1. **MVP gate**
   - facade journeys、authority regressions、feature matrix、workspace tests 与独立 factual review

Authority closure、`RouterInputProbes`、完整 Journey G 与 MVP gate 仍未完成；结构治理完成不等于完整 Journey G 或 MVP PASS。

## MVP 后

- immediate BSN adapter
- 按独立 variant 审查后增加文本、图片等 primitive
- 真实 BSN nested journey 示例

## 暂停

- queued/streaming BSN、file scene、hot reload
- parallel named outlet、retained cache、transition、loader/action/revalidation

暂停项不作为当前切片的前置假设
