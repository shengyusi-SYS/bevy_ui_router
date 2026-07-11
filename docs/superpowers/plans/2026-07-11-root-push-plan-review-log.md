# Root Push Plan Review Log

**Status:** Root push `Plan PASS`

**Reviewed artifacts:**

- `docs/root-push-contract-profile.md`
- `docs/superpowers/plans/2026-07-11-root-push-implementation-plan.md`
- Related `roadmap.md`, `target-architecture.md`, and `mvp-acceptance-journeys.md` sequencing clarifications

## Review Roles

| Role | Model | Direction |
| --- | --- | --- |
| Factual | `gpt-5.6-sol` high | Bevy revision, Cargo/features, commands, files, executable evidence |
| Architecture/lifecycle | `gpt-5.6-sol` high | authority cutover, identity, fallback, commit and cleanup uniqueness |
| Facade closure | `gpt-5.6-sol` high | stage profile, final golden relationship, owner visibility, capability leakage |
| Delivery/testing | `gpt-5.6-terra` high | task order, TDD, commit boundaries, consumer and regression gates |

## Review Rounds

### Round 1: Common Protocol Gap

The first draft mixed the final MVP facade contract with Root-only behavior and split work by crate before freezing cross-crate identity and commit semantics. The lead accepted this common root, rejected expanding Root to the whole MVP, and introduced a pure-subtraction Root contract profile, full `RootTaskIdentity`, explicit fallback attempts, preflighted non-fallible publication, and a single workspace authority cutover.

### Round 2: Transitive Closure Gap

Review found incomplete compile/task closure: cleanup was after cutover, excluded audit types still appeared in signatures, fixed test timing conflicted with vertical slices, and repeated public `push` lacked honest second-call behavior. The plan moved cleanup foundation into cutover, added exact signature exclusions, clarified stage-activated fixed targets/catalogues, and expanded Root to final-correct repeated single-root behavior while keeping child construction private.

### Round 3: Final Closure

The plan fixed owner inherent-method visibility, repeated-push history/cleanup branches, valid Cargo commands, independent feature consumers, executable authority regression comparison, and Root/Nested terminology. All four reviewers returned `PASS`; each explicitly distinguished plan approval from implementation or MVP approval.

## Structural Assessment

Two fresh reviewers independently returned `NO REWRITE`. They found the final-spec, stage-profile, and execution-runbook hierarchy usable. Accepted local cleanup added status/baseline metadata, README/Roadmap links, GREEN-only completion commands, explicit active target paths, and maintenance ownership. No whole-document rewrite was performed.

## Conclusion

Root push planning is `Plan PASS`. Implementation must begin at Task 1 and may claim only per-task completion until Task 12 evidence and independent implementation reviews pass.

## Implementation Clarification: Task 3

During Task 3, an independent factual checklist exposed two stage-plan transcription errors and one facade projection omission. The lead evaluated them against the documented authority order instead of accepting the findings mechanically:

- The blanket ban on public ID `raw()` access was narrowed to `RouterHandle` and `NavigationRequestId`; other read-only ID projections remain exactly as frozen by the associated-API golden, while every constructor and allocator stays crate-private.
- The final public `NavigationRequestSnapshot` remains ECS-owned. Task 3 produces only the Core-private request-record foundation used by the later ECS snapshot projection.
- Target Architecture consistently requires committed error presentation to expose its matched fingerprint, while Fixture/Golden omitted the accessor. The omission was corrected with `ActivePresentation::matched()` and explicit match-failure/post-match recovery assertions; no new runtime capability or Root-profile exception was introduced.

These are local consistency corrections. They do not change module authority, lifecycle semantics, task order, or the Root push scope. A `gpt-5.6-sol` high factual/facade re-review and a `gpt-5.6-terra` high implementation-sequencing re-review both returned `PASS` with no findings, so the plan remains `Plan PASS`.

## Implementation Clarification: Task 4 Cutover Boundary

Before Task 4 implementation, factual dependency inspection showed that its Step 5 accidentally applied the requirement “private and disconnected” to the legacy runtime rather than the new engine. Current production ECS/View callers still require the legacy Core modules, while the global protocol and Task 9 assign caller migration, export removal, source deletion, and new-engine wiring to one atomic workspace cutover.

The lead accepted the common root and corrected Task 4 so that `RouterEngine`/cleanup remain Core-private and disconnected through Task 8, while the legacy path retains only its existing visibility for unchanged live callers and remains the sole integrated publisher. Task 4 may not add legacy callers or exports. Task 9 still owns the single migration/removal/wiring commit, preventing both a broken intermediate workspace and a two-publisher state.

A `gpt-5.6-sol` high factual/architecture review and a `gpt-5.6-terra` high delivery review independently required the same correction. This changes no target architecture or Root capability; it only makes the reviewed cutover sequence executable.

Follow-up review found the same sequencing assumption in Tasks 5-8: View report removal, ECS bridge replacement, engine-backed command/snapshot behavior, and engine admission consumption were scheduled before the atomic cutover. The lead treated these as one shared root rather than four local findings. Tasks 5-8 now add only owner-crate private/disconnected foundations and authority-free final DTOs while every existing live caller remains unchanged. Task 9 exclusively owns cross-crate consumption, behavioral facade completion, caller migration, old-path removal, and lifecycle wiring. No adapter, alias, shim, or dual-write compatibility layer is introduced.

Closure review further required every Task 4-8 commit to compile all workspace reverse dependents, kept new definitions with legacy name collisions owner-private until Task 9, expanded the Task 9 file map to every deferred owner, and added a positive facade queue/snapshot RED/GREEN target to the Task 9 gate. The shared-workspace staging race was separately codified in `AGENTS.md` as a single-index-owner rule with exact-path staging only.

The final file-map check added the Core engine/cleanup/admission/lib owners needed to expose a sealed ECS-only integration path in the same commit that removes facade whole-crate aliasing, and named `view/testing.rs` explicitly for removal. This closes the cutover without creating a temporarily facade-reachable authority surface.

After these corrections, the `gpt-5.6-sol` high factual/architecture reviewer and `gpt-5.6-terra` high delivery reviewer both returned `PASS` for the Task 4-9 execution closure. The plan remains `Plan PASS`.
