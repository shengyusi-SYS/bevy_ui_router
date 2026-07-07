# Phase 1A Route Model And Matcher Contract Plan Review Log

## Round 1

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1-route-model-matcher-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** all four reviewers requested changes.

**Common root cause assessment:**

The draft planned a matcher-sized slice but named it as the full Phase 1 gate. That mismatch caused three linked problems:

- The scope omitted Phase 1 items already recorded in the roadmap: branch diff, raw params/search fingerprint, named slot placement, and error-boundary declaration inputs.
- The matcher examples were hand-authored before the params ownership model was frozen, so the tests mixed local params and accumulated params.
- The implementation snippets exposed more public surface than Phase 1A needs, especially `RouteTreeBuilder::Default`, `RouteNode::children`, and `RouteNode::declaration_index`.

**Accepted review findings:**

- Rename the document and scope to Phase 1A instead of presenting it as full Phase 1.
- Add an explicit Phase 1B handoff gate before Phase 2 can begin.
- Define `MatchedRoute::params()` as local route params only.
- Fix matcher fixture expectations so the matched branch includes every route node in the built tree.
- Add explicit not-found, root path, leading slash, index-vs-pathless, duplicate root, missing parent, and child order coverage.
- Require new module RED steps to compile the module before running Cargo, and treat `running 0 tests` as invalid evidence.
- Remove `RouteTreeBuilder::Default` from the planned implementation.
- Keep `RouteNode` out of the prelude and keep declaration order / child-list access internal.
- Reframe facade work as boundary verification only.
- Remove local absolute workspace path text from the plan.

**Rejected or deferred findings:**

- Do not expand this document into the full Phase 1 implementation plan. The reviewed split is safer because Phase 1B needs its own branch-planning contract review.
- Do not add accumulated branch params to Phase 1A. Branch-level params and search fingerprint belong to Phase 1B.

**Revision summary:**

- Moved the document to `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`.
- Added Phase 1A/Phase 1B split decision and Phase 1B handoff gate.
- Tightened public API and prelude surface.
- Repaired TDD evidence strategy and matcher/route-tree tests.
- Updated final verification and acceptance criteria to match Phase 1A.

## Round 2

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`

**Final results:**

- Architecture and module-boundary review: PASS.
- API and lifecycle authority boundary review: PASS.
- Fact and source review: PASS.
- Test, risk, and execution feasibility review: CHANGES_REQUESTED, then PASS after targeted wording fixes.

**Common root cause assessment for requested changes:**

The remaining issue was not Phase 1A architecture. It was test-step wording: the document used "failing" language for both true RED steps and for later regression/boundary coverage that may legitimately pass after earlier tasks.

**Accepted review findings:**

- Mark invalid-pattern tests as regression coverage, not a mandatory RED step.
- Mark facade export tests as boundary verification, not a mandatory RED step.
- Add a global rule distinguishing RED evidence from coverage and boundary-verification steps.
- Add acceptance criteria that only explicitly labeled RED steps require a failing command before implementation.

**Revision summary:**

- Updated Task 2 wording to "invalid-pattern regression coverage."
- Updated Task 9 wording to "facade boundary test."
- Clarified that coverage and boundary-verification steps must not manufacture failures when behavior already exists.
- A targeted test/risk review accepted these wording fixes with PASS.

## Rewrite Assessment

**Result:** NO_REWRITE_NEEDED.

**Assessment summary:**

The reviewed document is long because it includes task-level implementation detail required by the planning workflow, but its structure is clear enough to keep:

- Constraints and Phase 1A/1B split are at the top.
- Public contract and match semantics precede implementation tasks.
- Review gate and acceptance criteria remain separate from task details.
- Key review-driven decisions are already integrated into the body.

No rewrite candidate was created. The accepted document remains `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`.
