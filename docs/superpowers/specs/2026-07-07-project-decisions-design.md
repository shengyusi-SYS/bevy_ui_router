# Bevy UI Router Project Decisions Design

**Date:** 2026-07-07  
**Status:** Confirmed planning decisions  
**Scope:** Project-level architecture and MVP planning decisions made before implementation.

---

## Purpose

This document records the confirmed project decisions for the generic Bevy UI Router. It is not an implementation plan and does not authorize code changes by itself.

The goal is to keep the first implementation focused while preserving the architecture required by the existing design documents:

- Router core remains generic, testable, and independent from Bevy, BSN, platform APIs, application services, themes, copy, permissions, and host application assumptions.
- Bevy ECS integration owns entity, schedule, host, slot, visibility, pickability, focus, and ownership concerns.
- BSN is the first concrete view adapter, not router core.
- Platform behavior enters through router-neutral intent and location abstractions.
- App-specific conventions stay outside core through extensions or integration profiles.

---

## Confirmed Decisions

### 1. Workspace Shape

Use a multi-crate Cargo workspace.

Confirmed shape:

- `bevy_ui_router_core`
- `bevy_ui_router_view`
- `bevy_ui_router_ecs`
- `bevy_ui_router_bsn`
- `bevy_ui_router_platform`
- `bevy_ui_router`
- examples and test support crates or packages as needed

Rationale:

- Crate boundaries enforce the most important architectural rule: core must not depend on Bevy, BSN, platform APIs, or application-specific concepts.
- Independent crates make contract tests easier to target.
- Future renderers, platform adapters, and integration profiles can evolve without rewriting core.

Trade-off:

- The workspace has more initial setup than a single crate, and feature coordination must be maintained carefully.

### 2. Public Entry Point

Keep `bevy_ui_router` as a facade crate.

The facade crate should re-export commonly used APIs from the lower-level crates and use features to control optional integrations.

Recommended default facade surface:

- core routing types
- view adapter traits and fake/test renderer support
- ECS integration plugin and host/slot components

Recommended opt-in facade features:

- BSN adapter
- platform adapter helpers
- examples or dev-only utilities

Rationale:

- Users get a simple entry point.
- Internal crate boundaries remain explicit and testable.
- Optional integrations can remain feature-gated and avoid pulling unstable or platform-specific dependencies into every build.

### 3. Planning Scope

Plan the complete Phase 0-6 roadmap, but make only Phase 0-3 executable in the first detailed implementation plan.

Phase 0-3 should cover:

- contract freeze
- route tree and branch model
- frame state and error boundary
- view adapter interface and immediate BSN adapter

Phase 4-6 should remain roadmap-level until the earlier contracts are stable:

- loader, guard, and revalidation
- cache, modal, and transition runtime
- asset-driven BSN and dev tools

Rationale:

- The project needs a complete architectural direction, but non-MVP systems should not be implemented or over-specified before the core lifecycle is proven.

### 4. MVP API Style

Use a builder-first public API for route tree declaration.

MVP should provide a clear hand-written API such as a `RouteTree` or route builder. The underlying data structures should remain directly testable, but users should not need to construct every internal field manually.

Rationale:

- Builder APIs are explicit, testable, and easier to stabilize than macros.
- The API remains usable before derive macros, codegen, or route DSLs exist.
- It avoids locking the project into syntax-level decisions too early.

Non-MVP:

- route declaration macros
- derive-based typed route generation
- file-system route generation

### 5. Typed Routes And Typed Search

Do not include typed route or typed search APIs in the MVP executable scope.

MVP should use:

- raw `RouteLocation`
- string path segments
- raw or string-based params and search
- stable params/search fingerprints for branch comparison
- extension points for future typed codecs

Typed route/search support moves to Phase 4 or later.

Rationale:

- The first implementation should stabilize matcher, history, reconciliation, frame identity, generation, and diagnostics before introducing schema and generic codec design.
- Future typed APIs should layer on top of the raw boundary rather than replacing it.

### 6. View Adapter Strategy

Use a dual-track view adapter strategy in Phase 0-3.

Track A: fake/test renderer

- Used for contract tests.
- Must prove lifecycle, branch reconciliation, failure cleanup, diagnostics, and ownership rules without depending on BSN.

Track B: immediate BSN adapter

- Implemented in `bevy_ui_router_bsn`.
- Feature-gated or independently isolated from core.
- Targets immediate code-driven BSN scene apply.
- Validates the real Bevy/BSN integration path without allowing BSN API instability to shape core semantics.

Rationale:

- The fake/test renderer gives stable router contract coverage.
- The BSN adapter exposes real integration risk early.
- A failure in BSN integration must not require changing core routing semantics.

### 7. Bevy Dependency Strategy

Keep `bevy_ui_router_core` free of Bevy dependencies.

Temporarily pin Bevy-dependent crates to the current custom Bevy 0.19 git branch used by the repository:

- `bevy_ui_router_ecs`
- `bevy_ui_router_bsn`
- examples that require Bevy

The roadmap must explicitly include migration to upstream Bevy 0.19+ or the official BSN API when available.

Rationale:

- The project can target the current BSN environment while preventing experimental Bevy dependencies from leaking into core.
- It keeps a clear path to future reproducible builds and public release constraints.

Risk:

- CI, external development, and publishability may be affected while Bevy-dependent crates use a custom git dependency.

### 8. Validation Strategy

Use dual-track validation.

Every Phase 0-3 milestone must include focused contract tests. Phase 3 must additionally provide a neutral minimal nested routes example that exercises both:

- fake/test renderer path
- immediate BSN adapter path

Required test emphasis:

- matcher behavior
- branch reconciliation and common-prefix retention
- frame identity and generation
- route-owned subtree cleanup
- failure cleanup that never touches app-owned host entities
- error attribution and nearest boundary behavior
- diagnostics snapshots
- stable ordering that does not rely on undefined Bevy query iteration order

Rationale:

- Tests protect the framework contracts.
- Examples prove the public API and integration shape are usable.

---

## Architecture Boundary Summary

### `bevy_ui_router_core`

Owns router semantics only:

- `RouteLocation`
- `RouteTree` and builder API
- matcher
- `MatchedBranch`
- memory history
- branch diff
- frame identity and generation
- frame lifecycle model
- error taxonomy
- diagnostics snapshot types
- extension points for future typed route/search and app profiles

Must not depend on:

- Bevy ECS types
- BSN
- platform APIs
- concrete UI widgets
- app-specific services, permissions, themes, copy, or shell models

### `bevy_ui_router_view`

Owns renderer-agnostic view contracts:

- view factory contract
- render plan or view descriptor contract
- apply result and failure cleanup contract
- fake/test renderer for contract tests

It may reference router identities from core but must not own history, matching, or lifecycle authority.

### `bevy_ui_router_ecs`

Owns Bevy integration:

- plugin setup
- schedule sets
- `RouteHost`
- `RouteSlot`
- route-owned frame root markers
- entity ownership and cleanup
- hierarchy mutation
- visibility, pickability, and focus finalization
- diagnostics publication as Bevy resources or events

It must not allow adapters to bypass core lifecycle or forge frame identity/generation.

### `bevy_ui_router_bsn`

Owns BSN rendering integration:

- code-driven BSN route views
- immediate BSN scene apply
- scene resolve/apply error mapping
- cleanup of partially created router-owned output

It must not own:

- route matching
- history
- branch reconciliation
- frame lifecycle authority
- guard, loader, action, revalidation, back, or platform policy

### `bevy_ui_router_platform`

Owns router-neutral platform boundary types:

- `BackIntent`
- `BackPolicy`
- `BackCapabilitySnapshot`
- location source abstractions
- router instance targeting for future multi-window or multi-surface integration

MVP should keep this crate minimal and avoid concrete platform packages.

---

## Data Flow

The intended lifecycle is:

```text
RouteLocation
  -> core matcher
  -> MatchedBranch
  -> branch diff
  -> frame plan with router, host, slot, frame identity, generation
  -> ECS creates or updates router-owned frame roots
  -> view renderer applies into RouteSlot
  -> ECS finalizes visibility, pickability, and focus
  -> core commits active rendered branch
  -> diagnostics snapshot
```

Rules:

- Requested location, matched branch, and active rendered branch remain separate.
- Pending or partially applied scenes are not active UI.
- A frame becomes active only after view apply and ECS finalization succeed.
- Stale async or deferred results must be rejected before producing visible state changes.
- Failure cleanup is limited to router-owned local state and subtree.
- App-owned hosts are never destroyed, replaced, or rebuilt by the router.

---

## Phase Roadmap

### Phase 0: Contract Freeze

Purpose:

- Convert the highest-risk lifecycle rules into executable contracts before adding renderer complexity.

Executable scope:

- workspace and facade structure
- core crate isolation
- raw `RouteLocation`
- memory history behavior for `push`, `replace`, and `back`
- initial frame identity and generation model
- route-owned versus app-owned boundary tests
- diagnostics snapshot shape

Exit criteria:

- Core compiles without Bevy.
- Contract tests cover navigation history, frame identity, generation, ownership boundary, and diagnostics basics.
- No public API requires BSN or app-specific concepts.

### Phase 1: Route Tree And Branch Model

Purpose:

- Establish route declaration and matching.

Executable scope:

- route builder API
- route ids
- static segments
- params
- catch-all segments
- index routes
- nested routes
- pathless layouts
- named slot identity in the model
- `MatchedBranch`
- common-prefix branch diff

Exit criteria:

- Matcher tests cover static, param, catch-all, index, pathless, not-found, and conflict behavior.
- Branch diff tests prove common-prefix retention and changed suffix replacement.
- Matching order is deterministic and does not depend on insertion or query iteration ambiguity unless explicitly defined.

### Phase 2: Frame State And Error Boundary

Purpose:

- Separate location, matched branch, and active rendered branch, then make failure behavior explicit.

Executable scope:

- route frame records
- minimal statuses for MVP
- error taxonomy
- nearest error boundary resolution
- frame generation increment rules
- diagnostics for frame state and error attribution

Exit criteria:

- Failed build/apply paths enter explicit error state.
- Error boundary behavior replaces only the intended router-owned frame region.
- App-owned host cleanup is forbidden by tests.
- Diagnostics identify router, host, slot, frame identity, generation, route id, and error source.

### Phase 3: View Adapter Interface And BSN Adapter

Purpose:

- Prove renderer-agnostic application and the first real renderer integration.

Executable scope:

- `bevy_ui_router_view` renderer contract
- fake/test renderer
- ECS route host and slot integration
- frame root markers
- hierarchy apply and cleanup
- visibility, pickability, and focus finalization hooks
- immediate code-driven BSN adapter behind isolated dependency boundary
- minimal nested routes example

Exit criteria:

- Fake renderer tests prove apply success, apply failure cleanup, and active commit ordering.
- BSN adapter demonstrates immediate scene apply in a neutral example.
- Frame is not marked active before renderer apply and ECS finalization complete.
- BSN failures map into route view/apply errors.

### Phase 4: Loader, Guard, And Revalidate

Roadmap scope:

- typed params/search exploration
- sync guard first, pending guard reserved
- route-scoped task contract
- async result identity and stale checks
- loader/action/revalidation dependency model
- redirect loop detection

Entry condition:

- Phase 0-3 contracts are stable enough that async results can target clear frame identities and generations.

### Phase 5: Cache, Modal, And Transition

Roadmap scope:

- route-local serializable state
- retained entity cache contract
- modal/drawer frame kind
- pause/resume lifecycle
- transition policy
- back policy composition
- hidden subtree input isolation and focus release

Entry condition:

- Ownership, cleanup, focus, visibility, and pickability finalization are already covered by tests.

### Phase 6: Asset-Driven BSN And DevTools

Roadmap scope:

- `.bsn` asset route views
- queued/asset scene lifecycle
- hot reload by rebuilding view frames while preserving allowed route state
- diagnostics overlay
- navigation trace replay

Entry condition:

- Immediate BSN adapter is stable and queued scene pending/apply/active boundaries are explicitly modeled.

---

## Known Risks

- The custom Bevy 0.19 git dependency may make CI and external reproduction fragile until migration to upstream Bevy or official BSN API is possible.
- BSN API instability can affect `bevy_ui_router_bsn`, but must not reshape `bevy_ui_router_core`.
- Builder API design can become too broad if it tries to cover typed routes, macros, profiles, and file routing before the raw route model is stable.
- Examples can accidentally introduce app-specific assumptions; examples must use neutral page names and generic UI.
- Retained entity cache, transition, and async loader semantics are important, but implementing them before lifecycle and ownership contracts are stable would increase risk.

---

## Non-Goals For MVP

- typed route/search public API
- derive macros or route DSL macros
- async loader/action/revalidation implementation
- pending async guard implementation
- retained entity cache implementation
- transition animation runtime
- file-based BSN route scenes
- hot reload
- platform-specific back or deep-link packages
- app-specific integration profiles
- business-specific metadata, permissions, services, copy, themes, or shell policies

---

## Review Notes

This document records decisions already confirmed during planning. It intentionally leaves detailed task steps to a later implementation plan.

Before moving to implementation planning, review whether the confirmed decisions still match the intended project direction:

- multi-crate workspace with facade crate
- Phase 0-6 roadmap with Phase 0-3 executable detail
- builder-first MVP API
- raw route/search in MVP
- fake renderer plus BSN adapter dual track
- Bevy dependency isolated to ECS/BSN crates
- dual-track validation through tests and neutral examples
