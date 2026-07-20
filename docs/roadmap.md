# Roadmap

**状态**: Phase 3B.1 `Reconciliation PASS`

**更新时间**: 2026-07-20

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

## Now

1. **History stage spec/plan and complete matrix**
   - 定义 `replace`、`back`、zero-entering、成功后 history commit 与失败保持语义
   - 在同一阶段 spec/plan 中冻结完整 role/topology/physical-drift/terminal matrix，不新增第二份状态源
   - spec/plan 与该完整 matrix 分别通过独立复审后，才开始 History runtime implementation

## Next

1. **Recovery**
   - view/mount failure、error presentation、post-commit host loss 与 stale replay
2. **Authority closure**
   - multi-router stress、完整 opaque audit、lease authority、background reaper 与 cleanup replay
3. **MVP gate**
   - facade journeys、authority regressions、feature matrix、workspace tests 与独立 factual review

完整 History runtime、Recovery、Authority closure、`RouterInputProbes` 与 MVP 均仍未完成；Reconciliation PASS 不等于 MVP PASS

## MVP 后

- immediate BSN adapter
- 按独立 variant 审查后增加文本、图片等 primitive
- 真实 BSN nested journey 示例

## 暂停

- queued/streaming BSN、file scene、hot reload
- parallel named outlet、retained cache、transition、loader/action/revalidation

暂停项不作为当前切片的前置假设
