# Roadmap

**状态**: Phase 3B.1 `Reconciliation 实施中`

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

## Now

1. **Task 6: private physical reconciliation regressions**
2. **Task 7: public Journey B、fragment 与 facade evidence**
3. **Task 8: Reconciliation completion gate 与 factual records**

每个切片仍须从公开 facade 命令经过真实 `App::update` 到稳定 snapshot，不新增第二条 lifecycle publisher

## Next

1. **History**
   - replace、back、zero-entering 与成功后提交
2. **Recovery**
   - view/mount failure、error presentation、host loss 与 stale replay
3. **Authority closure**
   - multi-router stress、完整 opaque audit、lease authority、background reaper 与 cleanup replay
4. **MVP gate**
   - facade journeys、authority regressions、feature matrix、workspace tests 与独立 factual review

## MVP 后

- immediate BSN adapter
- 按独立 variant 审查后增加文本、图片等 primitive
- 真实 BSN nested journey 示例

## 暂停

- queued/streaming BSN、file scene、hot reload
- parallel named outlet、retained cache、transition、loader/action/revalidation

暂停项不作为当前切片的前置假设
