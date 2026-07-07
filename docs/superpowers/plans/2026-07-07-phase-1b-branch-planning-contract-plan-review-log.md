# Phase 1B Branch Planning Contract Plan Review Log

## Round 1

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** all four reviewers requested changes.

**Common root cause assessment:**

The draft correctly aimed for a data-only Phase 1B contract, but it mixed three layers that must stay separate:

- segment reuse authority for future frame preservation;
- diagnostic/declaration metadata such as labels and error-boundary candidates;
- route-tree provenance needed to decide whether two tree-local `RouteId` values can be compared.

The draft also carried roadmap-level acceptance language that was broader than its executable tests, especially for raw search fingerprint fixtures and branch-diff scenarios.

**Accepted review findings:**

- Mark the current repository state as blocked because Phase 1A is only planned, not implemented.
- Split segment reuse identity from diagnostic/declaration metadata.
- Replace `RouteSegmentFingerprint` with `RouteSegmentReuseKey` for diff reuse decisions.
- Keep diagnostic labels, route metadata, and error-boundary declarations out of reuse keys and branch fingerprints.
- Add `RouteBranchProvenance` and require branch diff to fail closed across different route-tree provenance.
- Add structural fingerprint encoding to avoid separator collisions.
- Expand raw search tests for leading `?`, repeated keys, `a=` vs `a`, percent-case preservation, and fragment exclusion.
- Treat app payload exclusion as a current API boundary because `RouteLocation` has no payload or extension field.
- Expand branch-diff fixtures for sibling, child/depth, layout/provenance, slot/provenance, explicit-key/provenance, search, params, and suffix ordering.
- Add compile-fail boundary coverage for fingerprint and matched-branch constructors.
- Add a coverage matrix mapping semantics to tests and commands.
- Clarify that every filtered `cargo test` result is invalid if it reports `running 0 tests`.

**Rejected or deferred findings:**

- Do not make error-boundary metadata drive reuse. Error-boundary behavior belongs to Phase 2.
- Do not add app payload fields in Phase 1B just to test payload exclusion. Future payload/extension surfaces must add their own fingerprint tests.

**Revision summary:**

- Added current blocked status and stricter precondition wording.
- Added segment reuse key / provenance semantics.
- Updated public contract and task snippets for `RouteSegmentReuseKey` and `RouteBranchProvenance`.
- Expanded fingerprint and branch-diff fixtures.
- Added public-constructor compile-fail boundary checks.
- Added an explicit coverage matrix.

## Round 2

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** all four reviewers requested changes.

**Common root cause assessment:**

Round 1 separated the right concepts, but several executable examples still contradicted the intended authority model. The biggest issue was that ordinary branch-diff fixtures accidentally compared branches from separately built route trees, while also expecting same-tree common-prefix reuse. The same mismatch appeared in search handling: the roadmap treats raw search as part of branch/frame comparison, but the draft still let search-only changes keep a common prefix.

The correct architectural fix is to make same-source and cross-source comparisons explicit in the plan. Same-source fixtures compare branches produced by one matcher. Cross-source fixtures prove fail-closed behavior even when independent trees are structurally identical. Raw search changes fail closed by default in Phase 1B until a future reviewed route policy narrows that behavior.

**Accepted review findings:**

- Define `RouteBranchProvenance` as a core-internal source identity, not a structural fingerprint.
- Require structurally identical independent route trees to fail closed by provenance.
- Fix ordinary branch-diff fixtures to use the same matcher when expecting a nonzero common prefix.
- Make raw search fingerprint differences produce full leave/enter under the Phase 1B default policy.
- Strengthen metadata exclusion tests so diagnostic labels and error-boundary declarations are excluded from both `RouteSegmentReuseKey` and `RouteBranchFingerprint`.
- Add multi-element suffix-order coverage for leaf-to-root leaving and root-to-leaf entering.
- Add off-branch / unknown-route negative coverage for nearest error-boundary lookup.
- Expand compile-fail constructor coverage for segment reuse keys, branch provenance, params fingerprints, matched routes, and matched branches.
- Require nonzero doctest evidence for constructor-boundary checks.

**Rejected or deferred findings:**

- Do not add per-route search-reuse policy in Phase 1B. The default fail-closed behavior is simpler and matches the current roadmap; narrowed reuse belongs to a future reviewed design.
- Do not treat provenance as a route-tree structural hash. That would make independently constructed but identical trees appear interchangeable and would weaken tree-local route id boundaries.
- Do not add same-provenance route-tree hot-update fixtures for slot or explicit-key changes in Phase 1B. There is no reviewed provenance-preserving update path yet, so Phase 1B only covers those declarations as reuse-key inputs and cross-provenance fail-closed cases.

**Revision summary:**

- Updated branch provenance, search-change, branch diff, and acceptance semantics.
- Revised branch-diff test snippets to separate same-matcher common-prefix cases from cross-provenance fail-closed cases.
- Added structurally identical cross-provenance and multi-element suffix ordering fixtures.
- Strengthened matched-branch metadata exclusion, error-boundary negative cases, and constructor-boundary doctests.

## Round 3

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** architecture, API/lifecycle, and test/risk reviewers requested changes. Fact/source reviewer passed.

**Common root cause assessment:**

The scope and dependency facts were now correct, but several tests and snippets still used proxy evidence instead of direct invariants. Provenance separation, reuse-key encoding, metadata exclusion, public export boundaries, diagnostics, and Phase 1A gate checks all needed to be expressed as directly observable contracts.

There was also a small public-contract drift: snippets copied future lifecycle vocabulary from earlier phases into Phase 1B. Apply/stale/adapter/finalize/cleanup-style variants already exist in the current repository as broad Phase 0 contracts; resolve and transaction remain future vocabulary. Phase 1B must not add, reinterpret, or rely on those later lifecycle error categories.

**Accepted review findings:**

- Remove provenance from `RouteSegmentReuseKey` and `RouteBranchFingerprint`; keep it as a branch-level comparability gate only.
- Require typed length-prefixed structural encoding for segment reuse keys and branch fingerprints.
- Add explicit-key separator coverage so identity safety does not rely on delimiter restrictions.
- Strengthen independent-tree tests to show fingerprints can be equal while provenance still forces fail-closed diff behavior.
- Add direct local-param fingerprint coverage for matched branches with ancestor and leaf params.
- Replace metadata/boundary string-absence checks with equality checks proving metadata-only changes do not alter reuse keys or branch fingerprints.
- Replace weak Phase 1A gate evidence with source-only symbol checks and nonzero Phase 1A test commands.
- Add positive facade/prelude import coverage for public read-only contract types, so compile-fail constructor doctests cannot pass merely because exports are missing.
- Replace free-form branch-planning diagnostic messages with a structured `BranchPlanningDiagnostic` payload and fixed redacted message.
- Clarify that Phase 1B only adds `InvalidRouteDeclaration` if missing, preserves existing Phase 0 error variants, and does not add or reinterpret later resolve/apply/stale/adapter/transaction/finalization/cleanup lifecycle categories.

**Rejected or deferred findings:**

- Do not add same-provenance route-tree hot-update diff fixtures for slot or explicit-key changes. There is still no reviewed provenance-preserving update path in Phase 1B; those declarations are covered as reuse-key inputs and cross-provenance fail-closed cases.
- Do not restrict explicit keys to avoid separators solely for fingerprint safety. The accepted fix is structural encoding; user-facing key validation can remain a separate declaration-design decision.

**Revision summary:**

- Tightened global constraints, semantics, public contract, coverage matrix, and acceptance criteria around provenance, structural encoding, diagnostics, and Phase 1A gate evidence.
- Added direct fingerprint/reuse-key encoding, local-param, metadata equality, public export, and structured diagnostic fixtures.
- Replaced future-lifecycle error snippets with a local declaration-error-only Phase 1B change.

## Round 4

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** API/lifecycle reviewer passed. Architecture, fact/source, and test/risk reviewers requested changes.

**Common root cause assessment:**

The algorithm and lifecycle boundaries were now sound, but the document still had two planning-document risks:

- Identity-like information was made public for observability, especially `RouteBranchProvenance`.
- Some snippets described local incremental edits as if Phase 1B owned full existing public types such as `RouteErrorKind` or diagnostics exports.

The appropriate fix is to shrink Phase 1B's public surface and make implementation snippets explicitly incremental. Provenance remains internal and can be observed externally only through `BranchDiff::provenance_changed()`. Existing diagnostics and error API surfaces must be preserved rather than restated as Phase 1B-owned contracts.

**Accepted review findings:**

- Remove `RouteBranchProvenance` and `MatchedBranch::provenance()` from the public contract and facade/prelude export tests.
- Keep provenance type, constructors, debug/string access, and matched-branch accessor core-internal.
- Export only `BranchPlanningDiagnostic` as new diagnostics prelude surface; do not restate `DiagnosticIdentity`, `DiagnosticSnapshot`, `DiagnosticEvent`, or `DiagnosticEventKind` as Phase 1B export work.
- Preserve existing `DiagnosticEventKind` / `DiagnosticEvent` derives when adding branch-planning fields.
- Replace the full-looking `RouteErrorKind` enum snippet with an incremental instruction that only adds `InvalidRouteDeclaration` if missing and preserves all existing variants.
- Make `RouteError::invalid_route_declaration` `pub(crate)` because Phase 1B only needs it for core declaration validation.
- Add direct assertion that matched route metadata is readable.
- Split search-change fail-closed coverage from fragment-only neutrality at the branch-diff layer.
- Add positive facade imports for support types used by compile-fail doctests.
- Update matched-route constructor compile-fail coverage to call the current full internal constructor signature and require future updates if that signature changes.

**Rejected or deferred findings:**

- Do not expose provenance publicly as a diagnostic string. The public diagnostic should report `provenance_changed`, not the source token itself.
- Do not make Phase 1B own full error or diagnostics API rewrites. It only adds the local branch-planning pieces it needs.

**Revision summary:**

- Narrowed public provenance, diagnostics export, and error-helper scope.
- Added branch-level fragment-only coverage, matched metadata readability, and support-type facade import coverage.
- Reframed error and diagnostics snippets as local additive changes that preserve existing public API shape.

## Round 5

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** all four reviewers requested changes.

**Common root cause assessment:**

All reviewers converged on the same remaining issue: the prose said provenance was core-internal, but the implementation snippet still made `RouteBranchProvenance` a public type through the public `fingerprint` module. The compile-fail doctest only checked absence from `prelude`, so it did not prove absence from the public module path.

A second, smaller issue was that the diagnostics snippet still used a full-looking enum/struct definition even though Round 4 had established that diagnostics changes must be additive and preserve existing public API shape.

**Accepted review findings:**

- Make `RouteBranchProvenance` `pub(crate)` in the implementation snippet.
- Remove public provenance `as_str()` and `Debug` exposure from the snippet.
- Add a compile-fail check for the full module path `bevy_ui_router_core::fingerprint::RouteBranchProvenance`, not only `prelude`.
- Change Task 7 Step 1 to an explicit RED diagnostics test and add `running 0 tests` invalid evidence wording.
- Replace full-looking diagnostics enum/struct snippets with additive instructions: add `BranchPlanned`, add a `branch_planning` field, and preserve all existing variants, fields, derives, and accessors.

**Revision summary:**

- Provenance is now core-internal by Rust visibility in the plan snippets, not merely absent from `prelude`.
- Diagnostics implementation guidance is now explicitly additive.
- RED/coverage wording for diagnostics now matches the plan's global evidence rules.

## Round 6

**Reviewed document:** `docs/superpowers/plans/2026-07-07-phase-1b-branch-planning-contract-plan.md`

**Reviewer directions:**

- Architecture and module-boundary review.
- API and lifecycle authority boundary review.
- Fact and source review.
- Test, risk, and execution feasibility review.

**Results:** architecture, API/lifecycle, and fact/source reviewers passed. Test/risk reviewer requested changes.

**Common root cause assessment:**

The public API and lifecycle boundaries now passed review. The remaining issues were executable-test details introduced by the narrower provenance and structured diagnostics decisions:

- `RouteBranchProvenance` no longer exposes `Debug`, so tests must not use `assert_ne!` on provenance values.
- `BranchPlanningDiagnostic` had multiple public accessors, but the RED test directly exercised only part of them.

**Accepted review findings:**

- Replace provenance `assert_ne!` checks with `assert!(a != b)` to avoid requiring `Debug` on an internal authority token.
- Add direct assertions for `BranchPlanningDiagnostic::leaving_len`, `entering_len`, `search_changed`, and `branch_fingerprint_changed`.

**Revision summary:**

- Updated provenance inequality assertions to avoid expanding provenance observability.
- Expanded the diagnostics RED test to exercise every public payload accessor.
