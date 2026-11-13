# Design rationale

English | [简体中文](design-lessons.zh-CN.md)

This page explains the main design choices and their reasons. See the [behavior contracts](../contracts/README.md) for current semantics and [runtime](../architecture/runtime.md) for execution order.

## Central decisions, separate execution responsibilities

Core decides routes, requests, history, and logical lifecycle. ECS performs physical World operations, while View describes renderer-neutral inputs and views. Adapters consume those contracts. A navigation's target, outcome, and history therefore follow one decision chain.

Modules follow stable protocols and owners; orchestration combines phases and resource borrows. Validation can be shared when its purpose, accepted entity roles, and failure handling agree. Reusing Bevy's Scene, layout, and input pipelines keeps Router focused on routing.

Simpler setup compiles declarations into the existing route tree and installation path, and forwards Commands to the existing navigation entry point. Animation adapters reuse third-party timelines and centralize their handoff to routing lifecycle; applications still define effects. This reduces caller setup knowledge while preserving a single source of routing decisions.

## Visits, instances, and physical responsibility

A history entry represents a visit, an instance preserves entities and local state, and an owner holds execution responsibility. One instance can serve several visits, so scroll records belong to entries. Build input is a snapshot of one execution; reused instances obtain current input through a `RouteQueries` reader's `current` method.

Nested navigation preserves shared layouts. Parents and children choose keep-alive independently. Returning to P/D lets inventory decide whether P can be reused while the current route selects D. Each holds its own payload and exact parent dependency, preserving toolbar state while allowing a fresh child page.

The active branch and live-instance inventory are separate. Parent exit, fallback, and host loss handle related inactive owners. A completed request settles its foreground capabilities, while a retained owner holds longer-lived responsibilities.

## One completion path, distinct failure meanings

Fresh and retained targets enter the same publication pipeline with history authority, execution identity, and continuation. Activation, non-target protection, and cleanup each validate the facts they require. A missing outlet can prevent activation while the original owner still authorizes safe cleanup.

Cleanup decides how to dispose of physical obligations; continuation decides how the original request ends or recovers. Continuation retains the original failure, attempt, and branch classification. Partial completion preserves the original receipt and loss evidence, distinguishing a completable inert shell from isolated residuals with outstanding physical responsibility. Retry is reserved for capacity or prerequisites that can become available; permanent contradictions settle in bounded steps.

## Linear owners and preparation

An owner is an execution capability held, moved, or consumed once. Snapshots, projections, and fingerprints compare facts. Current and next are frozen and validated separately. Before consuming a delayed action, execution rebuilds its complete source projection and requires equality. Sparse slots, retained siblings, and cleanup targets are classified by their own roles and bindings.

Prepare capacity and owner handoffs before Core accepts responsibility. Complete validation, allocation, ordering, claim rebinding, cleanup reservations, and input/focus preparation before publication. The final step commits prepared state. Reviewing that boundary includes the effects of outer resource borrows, take/reinsert operations, scope exits, and drops.

## Content boundaries and post-write validation

Router manages its own structures and authorized content boundaries; applications own hosts and application resources. Creation first records exact allocation obligations. Successful adoption then manages ordinary content through current containment.

Component and relationship writes can synchronously trigger hooks and observers. After relevant writes, rebuild the target and protected closures and validate the allowed write set. Read-only preservation accepts legitimate extra protected content; mutation proofs validate actual deletion or write targets. Each proof maintains its own acceptance conditions.

## Activity, input, and background work

Applications use a `RouteQueries` reader's `is_active` method to decide whether page systems keep working. Activity queries check the nearest registered boundary and the complete parent mounting chain. Page activity, authored visibility, and the input gate each serve a separate purpose.

Revocation handles current and previous hover, press/click/drag state, and focus while preserving other valid targets. Focus candidates also preserve operation order: later set/clear operations replace earlier candidates, even when the final value is equal.

Page systems can combine `RouteQueries`, mutable application queries, and Commands navigation without splitting one user action across systems. Call `read()` once before a loop so activity, inputs, and pending-scroll lookups share observations of the borrowed metadata. Use mutable `RouterNavigation` when an immediate request ID or synchronous enqueue-error handling is needed, arranging compatible resource borrows and scheduling.

## Future native input integration

A future Bevy/UI extension should consume generic activity eligibility before candidate collection, focus admission, and dispatch target selection, while retaining native ordering, TabGroup, modal behavior, and bubbling. Rejecting a candidate still allows traversal of its descendants. Valid targets use native bubbling. Invalid focus follows the unfocused path and falls back to the window.

The Router bridge needs allowed, denied, and out-of-scope outcomes. `RouterInputGate=false` can also describe an application control outside Router, so global UI policy handles out-of-scope targets separately. The nearest inactive or invalid registered boundary takes precedence over an active parent layout; different Routers classify a target by its own scope.

Focus entry points need to cover ordinary systems, World access, and AutoFocus, preserving synchronous results, order, and cause for valid operations. Setting the same value and clearing an empty value also supersede older candidates. Scene apply records the real task and staged-content ownership. Pending intent is fulfilled after successful publication by its original owner, with preparation before publication and release events retained for previously valid targets.

This direction needs public, maintainable extension points usable with official dependencies. Once adopted, it replaces the current focus bridge that depends on private reflection. Applications currently use the explicit input gate. Planning status belongs in the [roadmap](../roadmap.md).

## Scroll restoration and content readiness

Successful Back navigation and the source entry's record determine restoration intent. Container binding and content readiness are separate. Early and late containers use the same intent. While waiting, the restore target stays frozen and actual presentation sampling continues. Exit rechecks the original binding, policy, and application value to preserve the right source after cancellation or an exit in the same update.

Automatic first-layout and explicit-ready policies serve immediate and delayed content respectively. Restoration clamps through ordinary Bevy layout and settles once. Applications manage loading, virtual content ranges, and explicit scrolling policy.

## Verification follows user journeys

Private tests locate owner, drift, and scheduling boundaries. Independent consumers validate real features and facade assembly. Ordinary `App::update` journeys exercise complete behavior. Cross-module handoffs deserve particular coverage: fresh/retained Back, parent-scope replacement, and failure recovery.

Input tests first establish a positive hit, then begin a real gesture and revoke it, and finally verify new input after reactivation. Fault injection changes real inputs to production predicates.

## Compare performance for equivalent work

Remove repeated collection and classification by sharing complete World observations within the current immutable borrow, then revalidate after relevant writes. Immutable stored values, such as saved scroll positions, can share storage for their own lifetime; copy-on-write keeps prepared and current states isolated. Compare complete observable snapshot values and preserve resource change detection when equal; diagnostics can still update after request completion.

Empty work exits at read-only entry points. Layout and World-structure queries keep the filtering rules each needs. Membership includes disabled or incomplete markers involved in duplicate detection. Schedule checks follow actual graph build/change events, covering late installation and external rebuilds.

Warm up bounded-record eviction into steady state. Navigation timing includes request completion and foreground cleanup. Fix the toolchain, features, executor, and content size, and retain complete paired results. Measure headless navigation, full schedules, and window presentation separately; see [performance](../performance.md).

## Documentation maintenance

Give each rule one clear home and link across topics. After implementation, update behavior, mechanisms, and operations in contracts, architecture, and guides respectively. Completed plans and experiment logs remain available through Git history.
