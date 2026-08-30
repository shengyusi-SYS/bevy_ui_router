# Root Push SDD Progress

Plan: `docs/superpowers/plans/2026-07-11-root-push-implementation-plan.md`
Start commit: `b56ce1c`

Task 1: complete (commits f4cbee7..2c5f989, spec review approved, quality review approved)
Task 2: complete (commits 12c1aff..543c4b7, spec review approved, quality review approved)
Task 3: complete (commits 0532589..4287fa3, spec review approved, quality review approved)
Task 4: complete (implementation 35e631b..52d65fe; cleanup layering correction a88c273..666bb1f; factual authority review approved; lifecycle quality review approved after fix)
Task 5: complete (commit 0656be7; spec review approved; quality review approved)
Task 6: complete (commit 367aa52; spec review approved after fix; quality review approved after fix)
Task 7: complete (commit f5d4798; independent factual review approved; ECS 77 tests approved)
Task 8: complete (commit 1bd1c4a; independent safety review approved after exact-registration and exact-seal fixes; ECS 96 tests approved)
Task 9: complete (atomic workspace cutover; factual architecture review approved; lifecycle review approved after authority-invalid Active isolation fixes; workspace/Core/View/ECS/facade/fmt gates approved)
Task 10: complete (commit ba40f56; cleanup/fallback/diagnostics/repeated-push/public Root journey implemented; Core/ECS/journey/workspace/fmt gates approved; authority, snapshot, facade and lifecycle reviews approved after fixes)
Task 11: complete (commit 8ff82dc; Root-profile facade, isolated consumer matrix, public signature closure, authority audit and exact regression catalogue implemented; spec and quality review approved after fixes)
Task 12: complete (Root completion gate; authority, adequacy, manifest and facade reviews approved after fixes; workspace check, nextest 442/442, Cargo tests/doctests, facade fixed gates, authority 19/19/19 and format/diff gates approved)

# Nested Push SDD Progress

Plan: `docs/superpowers/plans/2026-07-16-nested-push-implementation-plan.md`
Start commit: `eab1267`

Task 1: complete (commit 42de3c9; spec review approved; quality review approved; content 11/11, compile-fail 12/12 and Root journeys 4/4 verified)
Task 2: complete (commit 6c0a151; spec review approved after foreign/stale fix; quality review approved; mount 7/7, ids 10/10, integration 1/1 and Core 170/170 verified)
Task 3: complete (plan boundary correction 4e6ccd1; implementation f8199f1; independent re-review approved after workspace consumer and fallback cleanup fixes; workspace check, ECS 132/132, Core 185/185, View and format gates verified)
Task 4: complete (plan boundary corrections fe14965..1461144; implementation b4ed8bd; independent review approved after read-only artifact classification and exact frame-task context fixes; Core 219/219, ECS 132/132, Root journeys 4/4, workspace and format gates verified)
Task 5: complete (Core session 036a6ad; test fixture 6e71c36; facade scanner prerequisite 572a1f1; ECS staging 9122939; authority and lifecycle re-reviews approved; Core 228/228, ECS 166/166, facade 46/46 and 40/40, checks and format gates verified)
Task 6: complete (commit 46608ae; spec, authority and lifecycle reviews approved after fixes; Core 246/246, ECS 212/212, test-support 221/221, Root journeys 4/4, facade 46/46 and 40/40, authority 20/20/20, workspace and format gates verified)
Task 7: complete (commit 3c9150e; spec/lifecycle and authority/Matrix reviews approved after fixes; ECS 259/259, test-support 270/270, workspace 656/656, Root journeys 4/4, authority 20/20/20, workspace check and format gates verified)
Task 8: complete (commit 5e8869c; spec/public and authority/Matrix re-reviews approved after fixes; Core 247/247, public journeys 5/5, authority audit 2/2, facade 46/46 and 40/40, authority 23/23/23, workspace nextest 659/659, workspace check and format gates verified)
Task 9: complete (evidence fix 702e36e; completion record a977bd0; four review responsibilities and final lifecycle/physical review approved; Core 301, View 15, ECS 273, journeys 5/5, audit 2/2, facade 46/46 and 40/40, authority 23/23/23, workspace nextest 659/659, doctest 28, check/format/diff gates verified)

# Reconciliation SDD Progress

Plan: `docs/superpowers/plans/2026-07-18-reconciliation-implementation-plan.md`
Contract: `docs/reconciliation-contract-profile.md`
Start commit: `0201d49`

Task 1: complete (commit 228c5bf, lifecycle and scope reviews approved after mount issuer and release cleanup fixes, reconciliation 14/14, release mount drift 2/2, Core 290/290, doctest 28/28, workspace check and format/diff gates verified)
Task 2: complete (commit c627a9e, specification and lifecycle reviews approved after preserved-mount projection fix, reconciliation 26/26, integration 44/44, Core 275 unit plus all boundary and doctest gates, workspace check and format/diff gates verified, ECS 258/273 with 15 expected Task 3/4 splice-transition failures recorded)
Task 3: complete (commit 6f8e734, specification and authority reviews approved after closure-cut, required-use, duplicate-authority, lifecycle/occupancy and anchor-alias fixes, branch transaction 20/20, mount 32/32, registry 13/13, ECS all-target check and format/diff gates verified, full ECS 278/293 with the same 15 expected Task 4 runtime failures)
Task 4: complete (plan boundary corrections b899a4c..f47e6af; implementation 9415d03; review fixes 4f4e4dc; lifecycle and authority re-reviews approved after RetainEmpty lifecycle, exact leaving cleanup, focus release, publish-last and no-op role fixes; ECS 311/311, authority 23/23/23, workspace check and format/diff gates verified)
Task 5: complete (commit ddce26d; specification, architecture and lifecycle re-reviews approved after exact boundary classification, typed isolation and ledger-reserve fixes; Core 284/284 plus 28 doctests, ECS 377/377, workspace all-targets/all-features check and format/diff gates verified)
Task 6: complete (plan gate 9d7d2f0; implementation e5a26e6; specification and quality re-reviews approved after exact role, audit, cleanup and child-slot authority evidence fixes; focused 29/1/6/1, ECS all-features 381/381, authority 23/23/23 and format/diff gates verified)
Task 7: complete (public evidence gate a694acb; cleanup/continuation guardrail 9307d2f; Root fallback Core/ECS prerequisite plans 10dc5f4..81d2657; prerequisite correction 2e5cb3d with specification, authority and atomicity re-reviews approved; public evidence 5806c59 with specification and quality re-reviews approved after change-tick, boundary-target and cleanup-order evidence fixes; focused 3/3, facade journeys 7/7, authority audit 3/3, facade 46/46 and 40/40, authority 23/23/23, format and diff gates verified)
Task 8: complete (commit 2fa5332; specification, Core lifecycle, ECS physical authority and public facade reviews approved after factual-boundary fixes; final factual docs re-review approved; Core 286 unit plus 28 doctests, View 15, ECS 401 unit plus all integration targets, journeys 7/7, audit 3/3, facade 46/46 and 40/40, authority 23/23/23, workspace nextest 838/838, workspace doctest 28/28, check/format/diff and clean-tree gates verified)

# MVP Documentation Guardrails Progress

Plan: `docs/superpowers/plans/2026-07-20-mvp-documentation-guardrails-update-plan.md`
Start commit: `1844380`

Task 1: complete (commit bcef428; specification and quality review approved; generic guardrail search, private-identifier exclusion and diff check verified)
Task 2: complete (commits bb4fabe..7eefc02; factual and quality re-review approved after completed-step checkbox reconciliation; checkpoint and diff checks verified)
Task 3: complete (commits a737923..b40c3b9; normative, factual and cross-module architecture reviews approved after target-partition and atomic-tail fixes; final range diff and clean-worktree gates verified)

# History SDD Progress

Plan: `docs/superpowers/plans/2026-07-20-history-implementation-plan.md`
Contract: `docs/superpowers/specs/2026-07-20-history-design.md`
Start commit: `e9d77f6`

Task 1: complete (commit a80de43; specification and quality re-review PASS after strict section-bounded profile, final-MVP vocabulary and claim-evidence fixes; facade allowlist 54/54, compile-fail 46/46, authority catalogue 5/5, authority 23/23/23 and format/diff gates verified)
Task 2: complete (commit d287f5c; independent canonical review PASS with no findings; facade allowlist 54/54, compile-fail 46/46, authority catalogue 5/5, targeted terminology and diff gates verified)
Task 3: complete (commit 7052b88; contract, lifecycle quality and authority/atomicity re-reviews PASS; Core 309, ECS 402, public replace 5/5, authority audit 4/4, ecs-only journeys 12/12, facade 54/54 and 46/46, workspace check and format/diff gates verified)
Task 4: complete (commits 0bd4579..f478fd6; Isolated receipt/completion and public Back closure passed independent code/contract reviews with Critical/Important/Minor 0; Core 365/365, ECS back 143/143, authority catalogue 15/15, public C1 1/1, public Back audit 2/2, facade allowlist 54/54, facade compile-fail 47/47, workspace check and format/diff gates verified)
Task 5: complete (commit e001345; public C2/C3 failure and queued-Back journeys, ABA/cross-incarnation/stale replay, pre-commit Host/AuthorityLost settlement, per-hook Pickable refreeze and PendingRejected owner/evidence closure; independent public/Core/ECS reviews closed with no remaining Critical/Important; Core 393/393, ECS 554/554, public journeys/audit 24/24, authority 24/24/24, workspace check and format/diff gates verified)
Task 6: complete (documentation close commit 1f3b055; exact committed tree verified with tracked worktree/index clean and commit diff check exit 0; History implementation be25dbe through e001345 inclusive; final contract activation sync f0fcc2e; History PASS with Recovery next; facade profile/compile-fail 101/101, workspace nextest 1108/1108 run 46cea8dc-76b7-4198-8ac4-2192716fc60c, doctest 28/28, all-target/all-feature check, format and diff gates verified; Recovery, Authority closure and MVP PASS remain open)

# Recovery SDD Progress

Plan: `docs/superpowers/plans/2026-08-12-recovery-implementation-plan.md`
Contract: `docs/superpowers/specs/2026-08-12-recovery-design.md`
Start commit: `11765b4`

Task 1: complete (commit 2adbe0c; strict Recovery profile/stage-union activation, current exclusions 10, History active deltas 3, Recovery planned deltas 3, Recovery active authority 0, runtime catalogue 24; parser and canonical re-reviews approved with Critical/Important/Minor 0 after indented-table, L+A+H, source-bound admission and identity-exhaustion fixes; facade allowlist 61/61, compile-fail 47/47, authority catalogue 18/18, authority verifier 24/24/24, both all-target checks, format and diff gates verified; Recovery runtime semantics, full Journey G, Authority closure and MVP PASS remain open)
Task 2: complete (R1 final `fbadf43`, merged to `main`; canonical failure-cleanup owner/ECS continuation, post-hook refreeze, public Journey D and profile activation; no-binding nested Push remains cleanup → `RouterFallback`; independent Core/ECS/public reviews closed with no Critical/Important; workspace 1153/1153 and Core doctest 28/28 verified; CRLF profile fixture made newline-agnostic).
Task 3: complete (R2 `d889942`; post-commit `L + A + H` compound settlement and Journey G Recovery-owned post-commit foreground rows; independent reviews Critical 0 / Important 0; authority 26/26/26 and committed-candidate gates verified).
Task 4: complete (R3 `21a0184`; stable unavailable, source-bound admission, provisional settlement, fresh/same-host epoch re-admission, Scenario E sealed stale-output cleanup and R3 activation; six global active deltas, three Recovery authority rows, catalogue 27).
Task 5: complete on verified implementation candidate `494ae6b` (R4 fixes `f8768bf`, `a42dce9`, `494ae6b`; final Core/ECS/public reviews Critical 0 / Important 0; one non-blocking mismatch-projection Minor retained; facade 61/61 and 47/47, public journeys 29/29, audit 10/10, catalogue integration 18/18, authority 27/27/27, workspace nextest 1222/1222 run `5462ad56-859f-4bbe-89a2-7ad7f1b3eae2`, Core doctest 28/28, workspace check/format/diff/tracking/trailing-whitespace gates verified). Recovery PASS is limited to Journey D, Scenario E and Journey G Recovery-owned foreground/readmission rows; actual old-lease reap, full Journey G, Authority closure and MVP PASS remain open. The newer overall objective makes behavior/API-preserving structural governance the next stage before Authority closure.

# Structural Governance Progress

Structural governance: complete on candidate `2f2cb42` (range `211874a..2f2cb42`; Core engine tests/post-commit, ECS cleanup tests/capacity and ECS navigation tests/authority-isolation extracted into stable child modules; behavior, public API, test names, features and filter entry points preserved; facade allowlist 61/61 and compile-fail 47/47, public journeys 29/29, audit 10/10, authority catalogue integration 18/18, authority 27/27/27, workspace nextest 1222/1222 with 0 skipped, doctest 28/28, all-target/all-feature check, format/diff/clean-tree gates verified; final independent review Critical 0 / Important 0 / Minor 0). Authority closure is now active; no new phase spec/plan is required.
