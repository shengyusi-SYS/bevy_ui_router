# 03 — Core Architecture And MVP

**日期**: 2026-07-06
**目标**: 定义通用 Bevy UI Router 的架构分层、核心概念、状态机、MVP 与模块边界。

---

## 1. Purpose

This document defines a generic Bevy UI Router architecture for independent framework development. It focuses on reusable routing concepts, Bevy ECS integration boundaries, view rendering adapters, BSN support, platform input adapters, and the minimum viable implementation scope.

The router must not assume any specific application domain, host layout, communication protocol, theme system, or data service.

---

## 2. Layered Architecture

```text
RouteLocation
  -> RouteTree
  -> Matcher
  -> NavigationPipeline
       guard
       redirect
       branch reconciliation
       view resolution
       view apply
       frame commit
       diagnostics
  -> Active RouteFrames
```

### 2.1 Router Core

Core owns framework-level routing semantics only:

- route definitions and route matching
- location and history abstraction
- matched branch calculation
- branch reconciliation
- route frame lifecycle
- guard and redirect outcomes
- open metadata and error contracts
- route-local state contracts
- retained entity cache contracts
- diagnostics snapshots

Core must not depend on Bevy ECS types, BSN, platform APIs, application services, or concrete UI components.

### 2.2 Bevy ECS Integration

ECS integration maps router concepts onto Bevy entities and schedules:

- router plugin setup
- schedule sets for navigation collection, branch commit, render apply, and frame finalization
- `RouteHost` and `RouteSlot` components
- route-owned frame root markers
- entity ownership and cleanup rules
- hierarchy mutation rules
- visibility, pickability, and focus convergence
- diagnostics publication as Bevy resources or events

This layer makes router mutations safe inside Bevy's World access model.

### 2.3 View Adapter

The view adapter boundary converts route view declarations into renderer-specific render plans.

Preferred split:

- route view factory produces a view description or render plan
- renderer applies the plan to a router-owned host or slot
- renderer reports partial output and disposal hints; Router/ECS performs cleanup through a controlled cleanup API
- router commits a frame only after apply and finalization succeed

This keeps routing state separate from scene construction details.

### 2.4 BSN Adapter

The BSN adapter is the first concrete view renderer, not Router core.

Responsibilities:

- support code-driven BSN route views
- map route props into BSN scene inputs
- apply BSN scenes under a `RouteSlot`
- report scene resolve or apply failures as router errors
- report partially created router-owned output and request cleanup through the controlled cleanup API

Deferred features:

- file-based scene routing
- hot reload
- asset chunking
- queued scene streaming beyond the immediate apply contract

### 2.5 Platform Adapters

Platform adapters produce router-neutral intents and location inputs:

- convert system back gestures or shortcut keys into `BackIntent`
- map deep links or custom URI inputs into `RouteLocation`
- expose per-window or per-surface location stores
- read `BackCapabilitySnapshot` to decide whether the host should intercept or delegate back behavior

Platform adapters must not embed application-specific permission, data loading, or view policy.

### 2.6 App Integration Profile

An app integration profile binds the generic router to app-specific conventions:

- metadata extensions
- guard dependencies
- data loading strategy
- mutation and revalidation channels
- rendering conventions
- persistent host layout policy
- diagnostics integration
- migration rules from an existing navigation system

These choices belong outside generic core API.

---

## 3. Core Concepts

### 3.1 RouteLocation

`RouteLocation` represents one navigation target. It should include:

- path
- typed or raw params
- query/search state
- optional fragment/hash
- optional custom payload
- router instance identifier

The router must distinguish requested location from rendered state. A location can be current before its matching branch is fully active.

### 3.2 RouteTree

`RouteTree` is the static route definition tree. It contains:

- path segments
- index routes
- pathless layout routes
- named slots
- route metadata
- guard definitions
- view declarations
- error boundary declarations

The route tree is declarative. Runtime frame state belongs elsewhere.

### 3.3 MatchedBranch

`MatchedBranch` is the ordered chain of route nodes produced by matching a `RouteLocation`.

It records:

- matched route ids
- extracted params
- parsed search state
- route metadata
- slot placement
- error boundary candidates

A matched branch is not the same thing as rendered UI.

### 3.4 RouteFrame

`RouteFrame` is one runtime instance of a matched branch segment. It carries:

- frame id
- route id
- generation number
- params/search fingerprint
- frame status
- slot identity
- root entity reference, in the ECS layer
- loader or render state, when enabled
- error state, if present

The generation number rejects stale async results.

### 3.5 RouteHost

`RouteHost` marks a root UI area managed by a router instance.

A host is owned by the application. The router may attach, remove, hide, or cache route-owned children under it, but it must not destroy the host itself.

### 3.6 RouteSlot

`RouteSlot` is a mount point for a branch segment or named outlet.

A layout route can preserve its slot while replacing only the child frame inside that slot. This is the basis for nested routing without full UI rebuilds.

### 3.7 BackIntent

`BackIntent` is a router-neutral request to move backward or close the most local navigational layer.

It may come from keyboard shortcuts, system gestures, controller buttons, window controls, or application UI. Core handles it through policy:

- close transient route layer
- pop route history
- block
- delegate to host
- request app exit or surface close

---

## 4. Core State Machine

The router should model frame lifecycle explicitly:

```rust
pub enum RouteFrameStatus {
    Matching,
    Guarding,
    Redirecting,
    Building,
    ResolvingView,
    ApplyingView,
    Entering,
    Active,
    Pausing,
    Leaving,
    Cached,
    Error,
}
```

Rules:

- `current location`, `matched branch`, and `active rendered branch` are separate.
- A frame is not `Active` until its view has been applied, mounted, and finalized.
- A failed frame enters `Error` without corrupting history unless policy explicitly replaces the location.
- Leaving and entering frames may coexist during transitions.
- Cached frames are inactive runtime objects, not serialized route state.

MVP can reduce phases to:

```text
Matching -> Building -> Active -> Leaving
                     -> Error
```

---

## 5. Branch Reconciliation

Navigation should reconcile old and new matched branches instead of replacing the whole UI tree.

Process:

1. Match the new `RouteLocation`.
2. Compare old and new branches by route id, slot identity, and params/search fingerprint.
3. Find the common prefix.
4. Preserve common prefix frames and their slots.
5. Leave changed suffix frames from leaf to root.
6. Enter new suffix frames from root to leaf.
7. Apply view changes inside affected slots only.
8. Commit the new active branch after apply and finalization succeed.

The reconciliation algorithm preserves stable layout frames and replaces only changed route-owned children.

Entity mutations should happen in clearly defined ECS phases. Despawn, spawn, reparent, hide, and cache operations for one reconciliation step should not be split across unrelated asynchronous callbacks.

---

## 6. RouteStateStore vs Retained Entity Cache

| Concern | RouteStateStore | Retained Entity Cache |
| --- | --- | --- |
| Purpose | Restore route-local UI state | Avoid rebuilding expensive runtime subtrees |
| Contents | Serializable state such as scroll, focus target, selected tab, form draft, filters | Bevy entity subtree and renderer-owned runtime state |
| Lifetime | Can follow history entries | Runtime only |
| Persistence | May be serialized | Must not be serialized |
| Capacity | Usually history-bound | Explicit cache policy and eviction |
| Restore mode | Rehydrate state into a new frame | Resume or reattach an existing subtree |

Cache policy should define:

- key strategy: frame instance, route plus params, or singleton
- hide strategy: disable input, hide visibility, or move under cache root
- eviction strategy
- pause/resume hooks
- cleanup behavior on final leave

---

## 7. RouteMeta

`RouteMeta` should remain open and application-extensible.

Core metadata should contain only router-algorithm policy:

```rust
pub struct RouteMeta {
    pub cache_policy: CachePolicy,
    pub transition_policy: TransitionPolicy,
    pub history_policy: HistoryPolicy,
    pub back_policy: BackPolicy,
    pub diagnostics_label: Option<String>,
    pub extensions: RouteMetaExtensions,
}
```

Application-specific metadata should live in typed extensions or an integration profile. Core should not hard-code text systems, permissions, layout enums, service dependencies, or theme conventions.

---

## 8. RouteError

`RouteError` should distinguish failure source while allowing application payloads.

Core error kinds:

```rust
pub enum RouteErrorKind {
    NotFound,
    GuardRejected,
    RedirectLoop,
    ViewBuildFailed,
    ViewApplyFailed,
    LoaderFailed,
    ActionFailed,
    StaleResult,
    Internal,
}
```

Recommended structure:

```rust
pub struct RouteError {
    pub kind: RouteErrorKind,
    pub route_id: Option<RouteId>,
    pub frame_id: Option<RouteFrameId>,
    pub message: String,
    pub source: Option<RouteErrorSource>,
    pub app_payload: Option<AppErrorPayload>,
}
```

Error boundaries should be resolved from the failing route upward. If a boundary exists, replace only that boundary's slot content. If not, use a router-level fallback view.

---

## 9. MVP Scope

The MVP should prove the router's core architecture with the smallest useful surface:

1. Route tree with nested routes, index routes, and pathless layouts.
2. Matcher from `RouteLocation` to `MatchedBranch`.
3. Router instance with memory-backed location history.
4. `push`, `replace`, and `back`.
5. `RouteHost`, `RouteSlot`, and route-owned frame root markers.
6. Branch reconciliation with common prefix preservation.
7. Minimal frame lifecycle and diagnostics.
8. Immediate BSN adapter for code-defined views.
9. Error fallback for match, guard, build, and apply failures.
10. Basic `BackIntent` handling without platform-specific coupling.
11. Test harness for matcher, reconciliation, ownership cleanup, failure cleanup, and diagnostics snapshots.

---

## 10. Non-MVP

Design for, but do not implement in the first version:

- async loader/action/revalidation pipeline
- pending async guards
- transition animation runtime
- retained entity cache implementation
- file-based BSN route scenes
- hot reload
- streamed or queued scene assets
- platform-specific adapter packages
- derive macros or route code generation
- multi-window persistence
- forward navigation
- app-specific integration profiles

---

## 11. First Release Module Boundaries

Suggested crate or module split:

```text
bevy_ui_router_core
  location
  route_tree
  matcher
  branch
  frame
  meta
  error
  state
  cache
  diagnostics

bevy_ui_router_ecs
  plugin
  schedules
  host_slot
  frame_root
  hierarchy
  focus
  visibility
  cleanup

bevy_ui_router_view
  view_descriptor
  render_plan
  renderer_trait
  fallback_view

bevy_ui_router_bsn
  bsn_view
  scene_apply
  scene_error_mapping

bevy_ui_router_platform
  back_intent
  location_sources
  capability_snapshot

bevy_ui_router_examples
  minimal_nested_routes
  error_boundary
  back_intent
  state_restore
```

The first release should keep core small, testable, and renderer-agnostic. Bevy ECS integration and BSN support should be real, but layered so future renderers or application profiles can be added without changing the routing model.
