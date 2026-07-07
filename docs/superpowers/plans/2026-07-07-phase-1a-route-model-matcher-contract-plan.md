# Phase 1A Route Model And Matcher Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first core-only route pattern, route tree, deterministic matcher, and local matched-route params contract for Bevy UI Router.

**Architecture:** Phase 1A is the matcher slice of the broader Phase 1 route contract. It stays inside `bevy_ui_router_core` except for facade boundary verification, and it turns `RouteLocation` into a `MatchedBranch` through a declarative `RouteTree` without ECS, BSN, platform adapters, async tasks, frame lifecycle mutation, or app-owned host access.

**Tech Stack:** Rust 2024, workspace crates, zero runtime dependencies for `bevy_ui_router_core`, existing Cargo test and dependency-boundary checks.

## Global Constraints

- Router core remains renderer-agnostic and must not depend on Bevy ECS, BSN, platform APIs, application services, themes, permissions, data services, or application shell models.
- Phase 1A must not implement branch diff, raw params/search fingerprint, named slot placement, error-boundary selection, transaction runtime, async resolve/apply, frame lifecycle mutation, ECS host/slot behavior, retained subtree behavior, or BSN view application.
- Phase 1A is not the full Phase 1 gate. Phase 1B must complete branch planning inputs before Phase 2 transaction or frame-reuse work begins.
- `RouteId`, `RouteFrameId`, `RouteGeneration`, `DiagnosticIdentity`, and active/pending diagnostic constructors remain non-forgeable from external crates.
- Matching order must be deterministic and must not depend on Bevy query iteration order, filesystem order, hash iteration order, or child insertion accidents that are not encoded in an explicit key.
- Pending scenes are out of scope. A matched branch is not active UI.
- Match failures must use explicit error channels and must not be represented as a successful empty branch.
- The core manifest must keep dependency sections empty.
- Public API additions must be exported through `bevy_ui_router_core::prelude` only when they are stable contract surface for Phase 1A.
- Documentation and tests must keep the project generic and must not name a specific app, brand, host project, service, or business domain.
- Every RED step for a new module must wire the module into compilation before running Cargo. A command that exits 0 with `running 0 tests` is invalid RED evidence and must be treated as a plan execution failure.
- Coverage and boundary-verification steps are not RED steps unless explicitly labeled RED. They may pass immediately when earlier tasks already implemented the behavior, and execution must not introduce artificial failures to satisfy TDD theater.

---

## 1. Current Baseline

Phase 0 already provides these core contracts:

- `RouteLocation` in `crates/bevy_ui_router_core/src/location.rs`.
- Controlled id and generation types in `crates/bevy_ui_router_core/src/ids.rs`.
- Memory history in `crates/bevy_ui_router_core/src/history.rs`.
- Router-neutral back intent types in `crates/bevy_ui_router_core/src/back.rs`.
- Router-neutral error taxonomy in `crates/bevy_ui_router_core/src/error.rs`.
- Diagnostics identity and snapshot contracts in `crates/bevy_ui_router_core/src/diagnostics.rs`.
- Router-owned cleanup scope in `crates/bevy_ui_router_core/src/ownership.rs`.
- Dependency-boundary tests in `crates/bevy_ui_router_core/tests/dependency_boundary.rs`.

Phase 1A adds route declaration and matching only. It does not consume, apply, diff, fingerprint, or mutate rendered UI.

---

## 2. Phase Split Decision

Existing project roadmap documents describe Phase 1 as a wider gate that includes route tree, matcher, branch diff, raw params/search fingerprint, named slot placement, and error-boundary declaration inputs. This document intentionally splits that work:

- **Phase 1A:** route pattern grammar, route tree declaration, local params, deterministic matching, match errors, match diagnostics, and facade boundary verification.
- **Phase 1B:** branch diff, branch-level params/search fingerprint, named slot placement, route metadata/error-boundary declaration inputs, and the minimum branch planning contract needed before Phase 2 transaction lifecycle work.

Phase 2 must not begin from Phase 1A alone. It requires Phase 1B or a reviewed replacement plan that provides equivalent branch planning inputs.

If Phase 1A implementation uncovers that matcher semantics cannot be frozen without Phase 1B concepts, stop and revise the split instead of smuggling branch diff, slot placement, or error-boundary behavior into Phase 1A tasks.

### 2.1 Phase 1B Handoff Gate

The next planning document after Phase 1A must cover at least these Phase 1B contracts:

- Branch diff input and output shapes for common-prefix preservation.
- Branch-level raw params/search fingerprint used by future frame reuse and stale checks.
- Named slot placement as declaration data, not ECS mutation.
- Error-boundary declaration candidates as matched-branch metadata, not renderer behavior.
- Route metadata extension boundary without app-specific fields.
- Tests proving Phase 1B outputs are data-only and do not mark any UI active.

Phase 1B must go through the same multi-agent review loop before implementation. If Phase 1B review finds that Phase 1A public shapes are insufficient, revise Phase 1A through a reviewed compatibility patch instead of adding hidden assumptions in Phase 2.

---

## 3. File Structure

- Create `crates/bevy_ui_router_core/src/pattern.rs`
  - Owns route pattern grammar, path segment parsing, parameter-name validation, and pathless/index/path pattern distinction.
- Create `crates/bevy_ui_router_core/src/branch.rs`
  - Owns `RouteParams`, `MatchedRoute`, and `MatchedBranch`.
- Create `crates/bevy_ui_router_core/src/route_tree.rs`
  - Owns `RouteTree`, `RouteTreeBuilder`, `RouteNode`, child declaration, deterministic declaration order, and internally allocated `RouteId`.
- Create `crates/bevy_ui_router_core/src/matcher.rs`
  - Owns `RouteMatcher`, match traversal, specificity ranking, ambiguity detection, and conversion from `RouteLocation` to `MatchedBranch`.
- Modify `crates/bevy_ui_router_core/src/error.rs`
  - Add match-specific constructors for `NotFound`, `MatchConflict`, and invalid route declaration failures while keeping the enum router-neutral.
- Modify `crates/bevy_ui_router_core/src/diagnostics.rs`
  - Add core-internal match diagnostics events that expose requested location and redacted messages without exposing forgeable identities.
- Modify `crates/bevy_ui_router_core/src/lib.rs`
  - Export new modules and stable Phase 1A prelude types.
- Modify `crates/bevy_ui_router_core/tests/dependency_boundary.rs`
  - Keep zero-dependency enforcement and add public-constructor boundary coverage if new public types expose raw internals.
- Modify `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`
  - Track implementation progress, review notes, and final verification evidence during execution.

---

## 4. Public Contract

Phase 1A should leave these public shapes available as core contract surface. The prelude exports the stable user-facing subset listed in Task 8; `RouteNode` remains public only because `RouteTree::node` returns it.

```rust
pub enum RoutePatternKind {
    Path,
    Index,
    Pathless,
}

pub enum RoutePathSegment {
    Static(String),
    Param(String),
    Wildcard(String),
}

pub struct RoutePattern;

impl RoutePattern {
    pub fn path(pattern: impl Into<String>) -> Result<Self, RouteError>;
    pub fn index() -> Self;
    pub fn pathless() -> Self;
    pub fn kind(&self) -> RoutePatternKind;
    pub fn segments(&self) -> &[RoutePathSegment];
    pub fn original(&self) -> &str;
}

pub struct RouteTree;

impl RouteTree {
    pub fn builder() -> RouteTreeBuilder;
    pub fn root(&self) -> RouteId;
    pub fn node(&self, id: RouteId) -> Option<&RouteNode>;
}

pub struct RouteNode;

impl RouteNode {
    pub fn id(&self) -> RouteId;
    pub fn parent(&self) -> Option<RouteId>;
    pub fn pattern(&self) -> &RoutePattern;
}

pub struct RouteTreeBuilder;

impl RouteTreeBuilder {
    pub fn root(mut self, pattern: RoutePattern) -> Result<Self, RouteError>;
    pub fn root_id(&self) -> Option<RouteId>;
    pub fn route(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
    ) -> Result<RouteId, RouteError>;
    pub fn build(self) -> Result<RouteTree, RouteError>;
}

pub struct RouteParams;

impl RouteParams {
    pub fn get(&self, name: &str) -> Option<&str>;
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)>;
    pub fn is_empty(&self) -> bool;
}

pub struct MatchedRoute;

impl MatchedRoute {
    pub fn route(&self) -> RouteId;
    // Local captures produced by this matched route's own pattern only.
    pub fn params(&self) -> &RouteParams;
}

pub struct MatchedBranch;

impl MatchedBranch {
    pub fn routes(&self) -> &[MatchedRoute];
    pub fn leaf(&self) -> Option<&MatchedRoute>;
    pub fn is_empty(&self) -> bool;
}

pub struct RouteMatcher;

impl RouteMatcher {
    pub fn new(tree: RouteTree) -> Self;
    pub fn match_location(&self, location: &RouteLocation) -> Result<MatchedBranch, RouteError>;
}
```

The implementation may refine internal representation, but the plan must be updated before changing these public names or semantics.

`RouteNode` is a public read-only declaration inspection type because `RouteTree::node` returns it, but it is not part of the prelude in Phase 1A. Its declaration order and child list are internal matcher inputs, not adapter lifecycle authority.

### 4.1 Param Ownership Decision

Phase 1A freezes local params only:

- `MatchedRoute::params()` returns captures from that matched route's own `RoutePattern`.
- A leaf route must not implicitly contain params captured by ancestor routes.
- Duplicate capture names inside one pattern are invalid.
- Duplicate capture names in different route nodes are allowed in Phase 1A because params remain local to each `MatchedRoute`.
- Accumulated params, params/search fingerprints, and duplicate-name policy for branch-level fingerprints belong to Phase 1B.

---

## 5. Match Semantics

Patterns:

- `RoutePattern::path("/")` matches the root path with no remaining path segments.
- `RoutePattern::path("settings")` and `RoutePattern::path("/settings")` represent the same single static segment.
- `RoutePattern::path("projects/:project_id")` captures one non-empty segment into `project_id`.
- `RoutePattern::path("files/*rest")` captures the remaining path tail into `rest`; the wildcard must be the final segment.
- `RoutePattern::index()` matches only when the parent branch has consumed the full location path.
- `RoutePattern::pathless()` consumes no path segment and participates in the matched branch.

Invalid declarations:

- Empty parameter name such as `":"`.
- Empty wildcard name such as `"*"`.
- Duplicate parameter names inside one route pattern such as `":id/:id"`.
- Wildcard not in the final position such as `"files/*rest/edit"`.
- Empty interior path segment such as `"settings//profile"`.

Deterministic ranking:

- Static segment outranks dynamic parameter.
- Dynamic parameter outranks wildcard.
- Index route outranks pathless child when no path remains.
- Longer consumed path outranks shorter consumed path when both reach a leaf.
- If two candidates have identical specificity and both fully match, return `RouteErrorKind::MatchConflict`.
- Declaration order can only break ties when no public behavior changes; if two full matches would select different route ids with equal specificity, declaration order must not hide the ambiguity.

---

## 6. Implementation Tasks

### Task 1: Route Pattern Grammar

**Files:**
- Create: `crates/bevy_ui_router_core/src/pattern.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouteError`, `RouteErrorKind`, `RouteErrorSourceCategory`.
- Produces: `RoutePattern`, `RoutePatternKind`, `RoutePathSegment`.

- [ ] **Step 1: Wire the new module and write failing tests for valid patterns**

Add these tests to `crates/bevy_ui_router_core/src/pattern.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};

    #[test]
    fn path_pattern_normalizes_leading_slash_and_segments() {
        let pattern = RoutePattern::path("projects/:project_id/files/*rest").unwrap();
        let with_slash = RoutePattern::path("/projects/:project_id/files/*rest").unwrap();
        let root = RoutePattern::path("/").unwrap();

        assert_eq!(pattern.kind(), RoutePatternKind::Path);
        assert_eq!(pattern.original(), "projects/:project_id/files/*rest");
        assert_eq!(with_slash.segments(), pattern.segments());
        assert!(root.segments().is_empty());
        assert_eq!(
            pattern.segments(),
            &[
                RoutePathSegment::Static("projects".to_owned()),
                RoutePathSegment::Param("project_id".to_owned()),
                RoutePathSegment::Static("files".to_owned()),
                RoutePathSegment::Wildcard("rest".to_owned()),
            ]
        );
    }

    #[test]
    fn index_and_pathless_patterns_consume_no_segments() {
        assert_eq!(RoutePattern::index().kind(), RoutePatternKind::Index);
        assert_eq!(RoutePattern::pathless().kind(), RoutePatternKind::Pathless);
        assert!(RoutePattern::index().segments().is_empty());
        assert!(RoutePattern::pathless().segments().is_empty());
    }
}
```

Also add `pub mod pattern;` to `crates/bevy_ui_router_core/src/lib.rs` before running the RED command so Cargo compiles the new test module.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test -p bevy_ui_router_core pattern::tests -- --nocapture`

Expected: compile failure or test failure because `RoutePattern`, `RoutePatternKind`, or `RoutePathSegment` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid and must be fixed before continuing.

- [ ] **Step 3: Implement minimal pattern parser**

Implement `crates/bevy_ui_router_core/src/pattern.rs`:

```rust
use std::collections::BTreeSet;

use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutePatternKind {
    Path,
    Index,
    Pathless,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutePathSegment {
    Static(String),
    Param(String),
    Wildcard(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutePattern {
    kind: RoutePatternKind,
    original: String,
    segments: Vec<RoutePathSegment>,
}

impl RoutePattern {
    pub fn path(pattern: impl Into<String>) -> Result<Self, RouteError> {
        let original = pattern.into();
        let normalized = original.trim_start_matches('/');

        if normalized.is_empty() {
            return Ok(Self {
                kind: RoutePatternKind::Path,
                original,
                segments: Vec::new(),
            });
        }

        let mut seen_names = BTreeSet::new();
        let parts: Vec<&str> = normalized.split('/').collect();
        let mut segments = Vec::with_capacity(parts.len());

        for (index, part) in parts.iter().copied().enumerate() {
            if part.is_empty() {
                return Err(invalid_pattern("route path contains an empty segment"));
            }

            if let Some(name) = part.strip_prefix(':') {
                validate_capture_name(name, &mut seen_names)?;
                segments.push(RoutePathSegment::Param(name.to_owned()));
                continue;
            }

            if let Some(name) = part.strip_prefix('*') {
                validate_capture_name(name, &mut seen_names)?;
                if index + 1 != parts.len() {
                    return Err(invalid_pattern("wildcard segment must be last"));
                }
                segments.push(RoutePathSegment::Wildcard(name.to_owned()));
                continue;
            }

            segments.push(RoutePathSegment::Static(part.to_owned()));
        }

        Ok(Self {
            kind: RoutePatternKind::Path,
            original,
            segments,
        })
    }

    pub fn index() -> Self {
        Self {
            kind: RoutePatternKind::Index,
            original: String::new(),
            segments: Vec::new(),
        }
    }

    pub fn pathless() -> Self {
        Self {
            kind: RoutePatternKind::Pathless,
            original: String::new(),
            segments: Vec::new(),
        }
    }

    pub const fn kind(&self) -> RoutePatternKind {
        self.kind
    }

    pub fn segments(&self) -> &[RoutePathSegment] {
        &self.segments
    }

    pub fn original(&self) -> &str {
        &self.original
    }
}

fn validate_capture_name(name: &str, seen_names: &mut BTreeSet<String>) -> Result<(), RouteError> {
    if name.is_empty() {
        return Err(invalid_pattern("capture name must not be empty"));
    }

    if !name
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return Err(invalid_pattern("capture name must be ascii alphanumeric or underscore"));
    }

    if !seen_names.insert(name.to_owned()) {
        return Err(invalid_pattern("capture name must be unique inside one pattern"));
    }

    Ok(())
}

fn invalid_pattern(message: &'static str) -> RouteError {
    RouteError::new(
        RouteErrorKind::InvalidRoutePattern,
        RouteErrorSourceCategory::RouterCore,
        message,
    )
}
```

Modify the prelude in `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod prelude {
    pub use crate::pattern::{RoutePathSegment, RoutePattern, RoutePatternKind};
}
```

- [ ] **Step 4: Add the missing error kind**

Modify `crates/bevy_ui_router_core/src/error.rs`:

```rust
pub enum RouteErrorKind {
    InvalidRoutePattern,
    NotFound,
    MatchConflict,
    GuardRejected,
    ViewBuildFailed,
    ViewApplyFailed,
    StaleResult,
    AdapterFailed,
    HostOrSlotUnavailable,
    FinalizeFailed,
    CleanupFailed,
    Internal,
}
```

- [ ] **Step 5: Run tests and verify they pass**

Run: `cargo test -p bevy_ui_router_core pattern::tests -- --nocapture`

Expected: both pattern tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/error.rs crates/bevy_ui_router_core/src/pattern.rs
git commit -m "feat(core): add route pattern grammar"
```

### Task 2: Invalid Pattern Coverage

**Files:**
- Modify: `crates/bevy_ui_router_core/src/pattern.rs`

**Interfaces:**
- Consumes: `RoutePattern::path`.
- Produces: executable coverage for invalid declaration failures.

- [ ] **Step 1: Add invalid-pattern regression coverage**

Add this test to `crates/bevy_ui_router_core/src/pattern.rs`:

```rust
#[test]
fn invalid_patterns_return_router_core_errors() {
    for pattern in [":", "*", ":id/:id", "files/*rest/edit", "settings//profile"] {
        let error = RoutePattern::path(pattern).unwrap_err();

        assert_eq!(error.kind(), RouteErrorKind::InvalidRoutePattern);
        assert_eq!(error.source_category(), RouteErrorSourceCategory::RouterCore);
        assert!(!error.redacted_message().is_empty());
    }
}
```

- [ ] **Step 2: Run tests and interpret the result**

Run: `cargo test -p bevy_ui_router_core pattern::tests::invalid_patterns_return_router_core_errors -- --nocapture`

Expected: the test may pass immediately if Task 1 already implemented the invalid-pattern checks. If it fails because an invalid pattern is accepted or emits the wrong error kind/source, fix the parser in Step 3.

- [ ] **Step 3: Adjust parser defects found by the test**

Keep parser changes inside `crates/bevy_ui_router_core/src/pattern.rs`. Do not add dependencies and do not move validation into route tree construction.

- [ ] **Step 4: Run pattern tests**

Run: `cargo test -p bevy_ui_router_core pattern::tests -- --nocapture`

Expected: all pattern tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router_core/src/pattern.rs
git commit -m "test(core): cover invalid route patterns"
```

### Task 3: Matched Branch And Params

**Files:**
- Create: `crates/bevy_ui_router_core/src/branch.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouteId`.
- Produces: `RouteParams`, `MatchedRoute`, `MatchedBranch`.

- [ ] **Step 1: Wire the new module and write failing branch tests**

Add these tests to `crates/bevy_ui_router_core/src/branch.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::RouteId;

    #[test]
    fn params_preserve_deterministic_name_order() {
        let params = RouteParams::from_pairs([
            ("project".to_owned(), "alpha".to_owned()),
            ("document".to_owned(), "intro".to_owned()),
        ]);

        assert_eq!(params.get("project"), Some("alpha"));
        assert_eq!(
            params.iter().collect::<Vec<_>>(),
            vec![("document", "intro"), ("project", "alpha")]
        );
    }

    #[test]
    fn matched_branch_exposes_ordered_route_chain() {
        let root = MatchedRoute::new(RouteId::new(1), RouteParams::new());
        let child = MatchedRoute::new(
            RouteId::new(2),
            RouteParams::from_pairs([("id".to_owned(), "42".to_owned())]),
        );
        let branch = MatchedBranch::new(vec![root, child]);

        assert_eq!(branch.routes().len(), 2);
        assert_eq!(branch.leaf().map(MatchedRoute::route), Some(RouteId::new(2)));
        assert!(!branch.is_empty());
    }
}
```

Also add `pub mod branch;` to `crates/bevy_ui_router_core/src/lib.rs` before running the RED command so Cargo compiles the new test module.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test -p bevy_ui_router_core branch::tests -- --nocapture`

Expected: compile failure or test failure because `RouteParams`, `MatchedRoute`, or `MatchedBranch` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid and must be fixed before continuing.

- [ ] **Step 3: Implement branch data types**

Implement `crates/bevy_ui_router_core/src/branch.rs`:

```rust
use std::collections::BTreeMap;

use crate::ids::RouteId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteParams {
    values: BTreeMap<String, String>,
}

impl RouteParams {
    pub(crate) fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    pub(crate) fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            values: pairs.into_iter().collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchedRoute {
    route: RouteId,
    params: RouteParams,
}

impl MatchedRoute {
    pub(crate) fn new(route: RouteId, params: RouteParams) -> Self {
        Self { route, params }
    }

    pub const fn route(&self) -> RouteId {
        self.route
    }

    pub const fn params(&self) -> &RouteParams {
        &self.params
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchedBranch {
    routes: Vec<MatchedRoute>,
}

impl MatchedBranch {
    pub(crate) fn new(routes: Vec<MatchedRoute>) -> Self {
        Self { routes }
    }

    pub fn routes(&self) -> &[MatchedRoute] {
        &self.routes
    }

    pub fn leaf(&self) -> Option<&MatchedRoute> {
        self.routes.last()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}
```

`MatchedRoute::params()` is local to this matched route. Do not merge ancestor params into the leaf route.

Modify the prelude in `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod prelude {
    pub use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
}
```

- [ ] **Step 4: Run branch tests**

Run: `cargo test -p bevy_ui_router_core branch::tests -- --nocapture`

Expected: both branch tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/branch.rs
git commit -m "feat(core): add matched branch data"
```

### Task 4: Route Tree Builder

**Files:**
- Create: `crates/bevy_ui_router_core/src/route_tree.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouteId`, `RoutePattern`.
- Produces: `RouteTree`, `RouteTreeBuilder`, `RouteNode`.

- [ ] **Step 1: Wire the new module and write failing route tree tests**

Add these tests to `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};
    use crate::ids::RouteId;
    use crate::pattern::RoutePattern;

    #[test]
    fn builder_allocates_stable_route_ids_in_declaration_order() {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
        let root = builder.root_id().unwrap();
        let settings = builder.route(root, RoutePattern::path("settings")).unwrap();
        let profile = builder.route(settings, RoutePattern::path(":section")).unwrap();
        let tree = builder.build().unwrap();

        assert_eq!(tree.root(), root);
        assert_eq!(root.raw(), 1);
        assert_eq!(settings.raw(), 2);
        assert_eq!(profile.raw(), 3);
        assert_eq!(tree.node(settings).unwrap().parent(), Some(root));
    }

    #[test]
    fn builder_requires_exactly_one_root() {
        let error = RouteTree::builder().build().unwrap_err();

        assert_eq!(error.kind(), RouteErrorKind::InvalidRouteTree);
        assert_eq!(error.source_category(), RouteErrorSourceCategory::RouterCore);
    }

    #[test]
    fn builder_rejects_second_root_and_missing_parent() {
        let duplicate_root = RouteTree::builder()
            .root(RoutePattern::path("/"))
            .unwrap()
            .root(RoutePattern::path("other"))
            .unwrap_err();
        assert_eq!(duplicate_root.kind(), RouteErrorKind::InvalidRouteTree);

        let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
        let missing_parent = builder
            .route(RouteId::new(99), RoutePattern::path("orphan"))
            .unwrap_err();
        assert_eq!(missing_parent.kind(), RouteErrorKind::InvalidRouteTree);
    }

    #[test]
    fn children_remain_in_explicit_declaration_order() {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
        let root = builder.root_id().unwrap();
        let first = builder.route(root, RoutePattern::path("first")).unwrap();
        let second = builder.route(root, RoutePattern::path("second")).unwrap();
        let tree = builder.build().unwrap();

        assert_eq!(tree.node(root).unwrap().children(), &[first, second]);
    }
}
```

Also add `pub mod route_tree;` to `crates/bevy_ui_router_core/src/lib.rs` before running the RED command so Cargo compiles the new test module.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test -p bevy_ui_router_core route_tree::tests -- --nocapture`

Expected: compile failure or test failure because `RouteTree`, `RouteTreeBuilder`, or `RouteNode` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid and must be fixed before continuing.

- [ ] **Step 3: Add route tree implementation**

Implement `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
use std::collections::BTreeMap;

use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
use crate::ids::RouteId;
use crate::pattern::RoutePattern;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteNode {
    id: RouteId,
    parent: Option<RouteId>,
    pattern: RoutePattern,
    declaration_index: usize,
    children: Vec<RouteId>,
}

impl RouteNode {
    pub const fn id(&self) -> RouteId {
        self.id
    }

    pub const fn parent(&self) -> Option<RouteId> {
        self.parent
    }

    pub const fn pattern(&self) -> &RoutePattern {
        &self.pattern
    }

    pub(crate) const fn declaration_index(&self) -> usize {
        self.declaration_index
    }

    pub(crate) fn children(&self) -> &[RouteId] {
        &self.children
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTree {
    root: RouteId,
    nodes: BTreeMap<RouteId, RouteNode>,
}

impl RouteTree {
    pub fn builder() -> RouteTreeBuilder {
        RouteTreeBuilder::new()
    }

    pub const fn root(&self) -> RouteId {
        self.root
    }

    pub fn node(&self, id: RouteId) -> Option<&RouteNode> {
        self.nodes.get(&id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTreeBuilder {
    root: Option<RouteId>,
    nodes: BTreeMap<RouteId, RouteNode>,
    next_id: u64,
    next_declaration_index: usize,
}

impl RouteTreeBuilder {
    fn new() -> Self {
        Self {
            root: None,
            nodes: BTreeMap::new(),
            next_id: 1,
            next_declaration_index: 0,
        }
    }

    pub fn root(mut self, pattern: RoutePattern) -> Result<Self, RouteError> {
        if self.root.is_some() {
            return Err(invalid_tree("route tree root is already declared"));
        }

        let id = self.allocate_id();
        self.root = Some(id);
        self.nodes.insert(id, self.node(id, None, pattern));
        Ok(self)
    }

    pub const fn root_id(&self) -> Option<RouteId> {
        self.root
    }

    pub fn route(
        &mut self,
        parent: RouteId,
        pattern: RoutePattern,
    ) -> Result<RouteId, RouteError> {
        if !self.nodes.contains_key(&parent) {
            return Err(invalid_tree("route parent does not exist"));
        }

        let id = self.allocate_id();
        self.nodes.insert(id, self.node(id, Some(parent), pattern));
        self.nodes.get_mut(&parent).unwrap().children.push(id);
        Ok(id)
    }

    pub fn build(self) -> Result<RouteTree, RouteError> {
        let root = self.root.ok_or_else(|| invalid_tree("route tree root is missing"))?;
        Ok(RouteTree {
            root,
            nodes: self.nodes,
        })
    }

    fn allocate_id(&mut self) -> RouteId {
        let id = RouteId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn node(&mut self, id: RouteId, parent: Option<RouteId>, pattern: RoutePattern) -> RouteNode {
        let declaration_index = self.next_declaration_index;
        self.next_declaration_index += 1;
        RouteNode {
            id,
            parent,
            pattern,
            declaration_index,
            children: Vec::new(),
        }
    }
}

fn invalid_tree(message: &'static str) -> RouteError {
    RouteError::new(
        RouteErrorKind::InvalidRouteTree,
        RouteErrorSourceCategory::RouterCore,
        message,
    )
}
```

Modify `crates/bevy_ui_router_core/src/error.rs`:

```rust
pub enum RouteErrorKind {
    InvalidRoutePattern,
    InvalidRouteTree,
    NotFound,
    MatchConflict,
    GuardRejected,
    ViewBuildFailed,
    ViewApplyFailed,
    StaleResult,
    AdapterFailed,
    HostOrSlotUnavailable,
    FinalizeFailed,
    CleanupFailed,
    Internal,
}
```

`RouteTreeBuilder` must not derive `Default`. Its only construction path is `RouteTree::builder()`, which initializes `next_id` to `1`. `RouteNode::declaration_index()` and `RouteNode::children()` remain `pub(crate)` because declaration order is matcher evidence, not adapter lifecycle authority.

Modify the prelude in `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod prelude {
    pub use crate::route_tree::{RouteTree, RouteTreeBuilder};
}
```

- [ ] **Step 4: Run route tree tests**

Run: `cargo test -p bevy_ui_router_core route_tree::tests -- --nocapture`

Expected: all route tree tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/error.rs crates/bevy_ui_router_core/src/route_tree.rs
git commit -m "feat(core): add route tree builder"
```

### Task 5: Deterministic Matcher

**Files:**
- Create: `crates/bevy_ui_router_core/src/matcher.rs`
- Modify: `crates/bevy_ui_router_core/src/lib.rs`

**Interfaces:**
- Consumes: `RouteLocation`, `RouteTree`, `RoutePattern`, `MatchedBranch`, `RouteParams`, `RouteError`.
- Produces: `RouteMatcher::match_location`.

- [ ] **Step 1: Wire the new module and write failing matcher success tests**

Add these tests to `crates/bevy_ui_router_core/src/matcher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::branch::MatchedRoute;
    use crate::error::{RouteErrorKind, RouteErrorSourceCategory};
    use crate::ids::RouteId;
    use crate::location::RouteLocation;
    use crate::pattern::RoutePattern;
    use crate::route_tree::RouteTree;

    fn sample_tree() -> (RouteTree, RouteId, RouteId, RouteId, RouteId, RouteId) {
        let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
        let root = builder.root_id().unwrap();
        let layout = builder.route(root, RoutePattern::pathless()).unwrap();
        let projects = builder.route(layout, RoutePattern::path("projects")).unwrap();
        let project = builder.route(projects, RoutePattern::path(":project_id")).unwrap();
        let files = builder.route(project, RoutePattern::path("files/*rest")).unwrap();
        (builder.build().unwrap(), root, layout, projects, project, files)
    }

    #[test]
    fn matcher_returns_ordered_branch_with_params() {
        let (tree, root, layout, projects, project, files) = sample_tree();
        let branch = RouteMatcher::new(tree)
            .match_location(&RouteLocation::new("/projects/alpha/files/src/lib.rs"))
            .unwrap();

        assert_eq!(
            branch
                .routes()
                .iter()
                .map(MatchedRoute::route)
                .collect::<Vec<_>>(),
            vec![root, layout, projects, project, files]
        );
        let project_params = branch.routes()[3].params();
        let leaf_params = branch.leaf().unwrap().params();
        assert_eq!(project_params.get("project_id"), Some("alpha"));
        assert_eq!(leaf_params.get("project_id"), None);
        assert_eq!(leaf_params.get("rest"), Some("src/lib.rs"));
    }
}
```

Also add `pub mod matcher;` to `crates/bevy_ui_router_core/src/lib.rs` before running the RED command so Cargo compiles the new test module.

- [ ] **Step 2: Run tests and verify they fail**

Run: `cargo test -p bevy_ui_router_core matcher::tests::matcher_returns_ordered_branch_with_params -- --nocapture`

Expected: compile failure or test failure because `RouteMatcher` is not implemented. If the command exits 0 with `running 0 tests`, the RED step is invalid and must be fixed before continuing.

- [ ] **Step 3: Implement traversal and full-match selection**

Implement `crates/bevy_ui_router_core/src/matcher.rs` with these internal concepts:

```rust
use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
use crate::ids::RouteId;
use crate::location::RouteLocation;
use crate::pattern::{RoutePathSegment, RoutePatternKind};
use crate::route_tree::{RouteNode, RouteTree};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteMatcher {
    tree: RouteTree,
}

impl RouteMatcher {
    pub fn new(tree: RouteTree) -> Self {
        Self { tree }
    }

    pub fn match_location(&self, location: &RouteLocation) -> Result<MatchedBranch, RouteError> {
        let segments = split_location_path(location.path());
        let mut candidates = Vec::new();
        self.visit(self.tree.root(), &segments, 0, Vec::new(), RouteParams::new(), &mut candidates)?;
        select_candidate(location, candidates)
    }
}
```

The implementation must:

- Split `RouteLocation::path()` into non-empty path segments, with `/` producing an empty segment list.
- Traverse from `RouteTree::root()`.
- Add pathless routes to the branch without consuming segments.
- Add index routes only when all path segments are consumed.
- Add path routes only when their full pattern can consume the next location segments.
- Capture `:param` into that matched route's local `RouteParams`.
- Capture `*rest` as the slash-joined remaining path tail in that matched route's local `RouteParams`.
- Return `RouteErrorKind::NotFound` when no candidate fully consumes the location path.

- [ ] **Step 4: Export matcher**

Modify the prelude in `crates/bevy_ui_router_core/src/lib.rs`:

```rust
pub mod prelude {
    pub use crate::matcher::RouteMatcher;
}
```

- [ ] **Step 5: Run matcher success test**

Run: `cargo test -p bevy_ui_router_core matcher::tests::matcher_returns_ordered_branch_with_params -- --nocapture`

Expected: the matcher success test passes.

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/matcher.rs
git commit -m "feat(core): add deterministic route matcher"
```

### Task 6: Ranking, Index, And Conflict Semantics

**Files:**
- Modify: `crates/bevy_ui_router_core/src/matcher.rs`

**Interfaces:**
- Consumes: `RouteMatcher::match_location`.
- Produces: deterministic ranking and conflict behavior.

- [ ] **Step 1: Add failing ranking tests**

Add these tests to `crates/bevy_ui_router_core/src/matcher.rs`:

```rust
#[test]
fn static_routes_outrank_param_and_param_outranks_wildcard() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
    let root = builder.root_id().unwrap();
    let wildcard = builder.route(root, RoutePattern::path("settings/*rest")).unwrap();
    let param = builder.route(root, RoutePattern::path("settings/:section")).unwrap();
    let exact = builder.route(root, RoutePattern::path("settings/profile")).unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());

    assert_eq!(
        matcher
            .match_location(&RouteLocation::new("/settings/profile"))
            .unwrap()
            .leaf()
            .map(MatchedRoute::route),
        Some(exact)
    );
    assert_eq!(
        matcher
            .match_location(&RouteLocation::new("/settings/security"))
            .unwrap()
            .leaf()
            .map(MatchedRoute::route),
        Some(param)
    );
    assert_eq!(
        matcher
            .match_location(&RouteLocation::new("/settings/security/advanced"))
            .unwrap()
            .leaf()
            .map(MatchedRoute::route),
        Some(wildcard)
    );
}

#[test]
fn index_route_matches_only_when_parent_consumed_the_path() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
    let root = builder.root_id().unwrap();
    let settings = builder.route(root, RoutePattern::path("settings")).unwrap();
    let pathless = builder.route(settings, RoutePattern::pathless()).unwrap();
    let index = builder.route(settings, RoutePattern::index()).unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());

    assert_eq!(
        matcher
            .match_location(&RouteLocation::new("/settings"))
            .unwrap()
            .leaf()
            .map(MatchedRoute::route),
        Some(index)
    );
    assert_ne!(pathless, index);
}

#[test]
fn matcher_returns_not_found_when_no_candidate_consumes_location() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
    let root = builder.root_id().unwrap();
    builder.route(root, RoutePattern::path("settings")).unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());

    let error = matcher
        .match_location(&RouteLocation::new("/missing"))
        .unwrap_err();

    assert_eq!(error.kind(), RouteErrorKind::NotFound);
    assert_eq!(error.source_category(), RouteErrorSourceCategory::RouterCore);
}
```

- [ ] **Step 2: Add failing conflict test**

Add this test to `crates/bevy_ui_router_core/src/matcher.rs`:

```rust
#[test]
fn equally_specific_full_matches_return_conflict() {
    let mut builder = RouteTree::builder().root(RoutePattern::path("/")).unwrap();
    let root = builder.root_id().unwrap();
    let first = builder.route(root, RoutePattern::path("projects/:id")).unwrap();
    let second = builder.route(root, RoutePattern::path("projects/:slug")).unwrap();
    let matcher = RouteMatcher::new(builder.build().unwrap());

    let error = matcher
        .match_location(&RouteLocation::new("/projects/alpha"))
        .unwrap_err();

    assert_ne!(first, second);
    assert_eq!(error.kind(), RouteErrorKind::MatchConflict);
    assert_eq!(error.source_category(), RouteErrorSourceCategory::RouterCore);
}
```

- [ ] **Step 3: Run tests and verify failures identify ranking gaps**

Run: `cargo test -p bevy_ui_router_core matcher::tests -- --nocapture`

Expected: success test may pass, ranking or conflict tests fail until specificity selection is implemented.

- [ ] **Step 4: Implement explicit specificity key**

Represent each candidate with a key equivalent to:

```rust
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SpecificityKey {
    consumed_segments: usize,
    static_segments: usize,
    param_segments: usize,
    wildcard_segments: usize,
    index_bonus: usize,
    pathless_segments: usize,
}
```

Selection rules:

- Compare `consumed_segments`, `static_segments`, `param_segments`, and `index_bonus` in descending order.
- Compare `wildcard_segments` and `pathless_segments` in ascending order.
- If two full candidates have the same normalized specificity key and different leaf route ids, return `RouteErrorKind::MatchConflict`.
- Do not use child declaration order to select between equal full matches.

- [ ] **Step 5: Run matcher tests**

Run: `cargo test -p bevy_ui_router_core matcher::tests -- --nocapture`

Expected: all matcher tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_ui_router_core/src/matcher.rs
git commit -m "feat(core): rank route matches deterministically"
```

### Task 7: Error And Diagnostic Integration

**Files:**
- Modify: `crates/bevy_ui_router_core/src/error.rs`
- Modify: `crates/bevy_ui_router_core/src/diagnostics.rs`
- Modify: `crates/bevy_ui_router_core/src/matcher.rs`

**Interfaces:**
- Consumes: matcher errors and requested `RouteLocation`.
- Produces: match-specific error constructors and diagnostic event kinds.

- [ ] **Step 1: Add failing error tests**

Add these tests to `crates/bevy_ui_router_core/src/error.rs`:

```rust
#[test]
fn match_errors_are_router_core_errors_with_redacted_messages() {
    let not_found = RouteError::not_found("no route matched requested location");
    let conflict = RouteError::match_conflict("ambiguous route match");

    assert_eq!(not_found.kind(), RouteErrorKind::NotFound);
    assert_eq!(conflict.kind(), RouteErrorKind::MatchConflict);
    assert_eq!(not_found.source_category(), RouteErrorSourceCategory::RouterCore);
    assert_eq!(conflict.source_category(), RouteErrorSourceCategory::RouterCore);
    assert!(!not_found.redacted_message().contains("/"));
}
```

- [ ] **Step 2: Implement match error constructors**

Modify `crates/bevy_ui_router_core/src/error.rs`:

```rust
impl RouteError {
    pub fn invalid_route_pattern(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::InvalidRoutePattern,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub fn invalid_route_tree(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::InvalidRouteTree,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub fn not_found(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::NotFound,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }

    pub fn match_conflict(redacted_message: impl Into<String>) -> Self {
        Self::new(
            RouteErrorKind::MatchConflict,
            RouteErrorSourceCategory::RouterCore,
            redacted_message,
        )
    }
}
```

- [ ] **Step 3: Add failing diagnostic tests**

Add these tests to `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
#[test]
fn match_diagnostics_record_requested_location_without_active_identity() {
    let snapshot = DiagnosticSnapshot::new()
        .with_requested(RouteLocation::new("/missing"))
        .with_event(DiagnosticEvent::match_failed("no route matched requested location"));

    assert_eq!(snapshot.requested().map(RouteLocation::path), Some("/missing"));
    assert!(snapshot.active().is_none());
    assert_eq!(snapshot.events()[0].kind(), DiagnosticEventKind::MatchFailed);
    assert!(snapshot.events()[0].identity().is_none());
}
```

- [ ] **Step 4: Implement diagnostic event kind**

Modify `crates/bevy_ui_router_core/src/diagnostics.rs`:

```rust
pub enum DiagnosticEventKind {
    PendingOverwrite,
    HostOrSlotUnavailable,
    MatchFailed,
    MatchConflict,
}

impl DiagnosticEvent {
    pub(crate) fn match_failed(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::MatchFailed,
            identity: None,
            redacted_message: redacted_message.into(),
        }
    }

    pub(crate) fn match_conflict(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::MatchConflict,
            identity: None,
            redacted_message: redacted_message.into(),
        }
    }
}
```

- [ ] **Step 5: Use constructors from matcher and parser**

Replace local match/parser error helpers with:

```rust
RouteError::invalid_route_pattern("capture name must not be empty")
RouteError::invalid_route_tree("route tree root is missing")
RouteError::not_found("no route matched requested location")
RouteError::match_conflict("ambiguous route match")
```

Keep requested path values out of redacted messages.

- [ ] **Step 6: Run focused tests**

Run: `cargo test -p bevy_ui_router_core -- --nocapture`

Expected: all core tests pass. This broader command avoids relying on multiple Cargo test filters in one invocation.

- [ ] **Step 7: Commit**

```bash
git add crates/bevy_ui_router_core/src/error.rs crates/bevy_ui_router_core/src/diagnostics.rs crates/bevy_ui_router_core/src/matcher.rs crates/bevy_ui_router_core/src/pattern.rs crates/bevy_ui_router_core/src/route_tree.rs
git commit -m "feat(core): integrate match errors and diagnostics"
```

### Task 8: Public Boundary And Dependency Verification

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Modify: `crates/bevy_ui_router_core/tests/dependency_boundary.rs`
- Modify: `crates/bevy_ui_router_core/Cargo.toml`

**Interfaces:**
- Consumes: all Phase 1A public types.
- Produces: stable prelude exports and boundary tests.

- [ ] **Step 1: Add boundary tests**

Add this test to `crates/bevy_ui_router_core/tests/dependency_boundary.rs`:

```rust
#[test]
fn core_manifest_still_has_no_dependency_entries_after_matcher() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");
    let dependency_entries = dependency_entries(&manifest);

    assert!(
        dependency_entries.is_empty(),
        "matcher phase must not add dependency-like entries:\n{}",
        dependency_entries.join("\n")
    );
}
```

Add this doc test to `crates/bevy_ui_router_core/src/route_tree.rs`:

```rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteId;
//!
//! let _ = RouteId::new(99);
//! ```
```

- [ ] **Step 2: Run boundary tests and doctests**

Run: `cargo test -p bevy_ui_router_core --test dependency_boundary -- --nocapture`

Expected: dependency-boundary tests pass.

Run: `cargo test -p bevy_ui_router_core --doc`

Expected: doctests pass, including the `compile_fail` route id constructor proof.

- [ ] **Step 3: Normalize prelude exports**

Ensure `crates/bevy_ui_router_core/src/lib.rs` contains:

```rust
pub mod back;
pub mod branch;
pub mod diagnostics;
pub mod error;
pub mod history;
pub mod ids;
pub mod location;
pub mod matcher;
pub mod ownership;
pub mod pattern;
pub mod route_tree;

pub mod prelude {
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::branch::{MatchedBranch, MatchedRoute, RouteParams};
    pub use crate::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticIdentity, DiagnosticSnapshot,
    };
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
    pub use crate::matcher::RouteMatcher;
    pub use crate::ownership::{CleanupScope, OwnedRegion};
    pub use crate::pattern::{RoutePathSegment, RoutePattern, RoutePatternKind};
    pub use crate::route_tree::{RouteTree, RouteTreeBuilder};
}
```

Do not export `RouteNode` through the prelude in Phase 1A. It remains accessible through `RouteTree::node` for declaration inspection, but adapters should not treat declaration order or child lists as lifecycle authority.

- [ ] **Step 4: Run formatting and full core tests**

Run: `cargo fmt --all --check`

Expected: formatting check passes.

Run: `cargo test -p bevy_ui_router_core`

Expected: all core unit and integration tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/route_tree.rs crates/bevy_ui_router_core/tests/dependency_boundary.rs crates/bevy_ui_router_core/Cargo.toml
git commit -m "test(core): lock matcher public boundaries"
```

### Task 9: Facade Boundary Verification

**Files:**
- Modify: `crates/bevy_ui_router/src/lib.rs`
- Modify: `crates/bevy_ui_router/tests/facade_features.rs`

**Interfaces:**
- Consumes: `bevy_ui_router_core::prelude`.
- Produces: facade re-export coverage for Phase 1A without changing core matcher behavior.

- [ ] **Step 1: Add facade test for matcher exports**

Add this test to `crates/bevy_ui_router/tests/facade_features.rs`:

```rust
#[test]
fn facade_prelude_exposes_core_matcher_contracts() {
    use bevy_ui_router::prelude::{
        MatchedBranch, RouteMatcher, RoutePattern, RoutePatternKind, RouteTree,
    };

    let pattern = RoutePattern::path("/").unwrap();
    assert_eq!(pattern.kind(), RoutePatternKind::Path);

    let tree = RouteTree::builder().root(pattern).unwrap().build().unwrap();
    let _matcher = RouteMatcher::new(tree);

    fn assert_types<T>() {}
    assert_types::<MatchedBranch>();
}
```

- [ ] **Step 2: Run facade boundary test**

Run: `cargo test -p bevy_ui_router --no-default-features facade_prelude_exposes_core_matcher_contracts -- --nocapture`

Expected: the test may pass immediately because the facade already re-exports `bevy_ui_router_core::prelude::*`. If it fails because the facade does not expose the new core contracts, fix the facade in Step 3.

- [ ] **Step 3: Adjust facade only if needed**

If the test fails because the facade prelude is missing core exports, modify `crates/bevy_ui_router/src/lib.rs` so the existing `pub use bevy_ui_router_core::prelude::*;` path remains active under `--no-default-features`. Do not add ECS, BSN, platform, or adapter behavior in this task.

- [ ] **Step 4: Run facade checks**

Run: `cargo test -p bevy_ui_router --no-default-features`

Expected: facade tests pass without default features.

Run: `cargo check -p bevy_ui_router --no-default-features --features platform`

Expected: platform feature compiles without pulling matcher dependencies outside core or changing platform behavior.

Run: `cargo check -p bevy_ui_router --no-default-features --features bsn`

Expected: bsn feature stub compiles without adding BSN behavior to core or changing BSN behavior.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_ui_router/src/lib.rs crates/bevy_ui_router/tests/facade_features.rs
git commit -m "test: expose matcher through facade"
```

### Task 10: Final Phase 1A Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md`

**Interfaces:**
- Consumes: all Phase 1A changes.
- Produces: final verification evidence and review handoff.

- [ ] **Step 1: Run formatting check**

Run: `cargo fmt --all --check`

Expected: exit code 0.

- [ ] **Step 2: Run core tests**

Run: `cargo test -p bevy_ui_router_core`

Expected: exit code 0.

- [ ] **Step 3: Run core doctests**

Run: `cargo test -p bevy_ui_router_core --doc`

Expected: exit code 0.

- [ ] **Step 4: Run core boundary test**

Run: `cargo test -p bevy_ui_router_core --test dependency_boundary`

Expected: exit code 0.

- [ ] **Step 5: Run facade no-default tests**

Run: `cargo test -p bevy_ui_router --no-default-features`

Expected: exit code 0.

- [ ] **Step 6: Run facade feature checks**

Run: `cargo check -p bevy_ui_router --no-default-features --features platform`

Expected: exit code 0.

Run: `cargo check -p bevy_ui_router --no-default-features --features bsn`

Expected: exit code 0.

- [ ] **Step 7: Confirm no generated lockfile remains unless the project deliberately adopts one**

Run: `git status --short`

Expected: only intentional Phase 1A files are modified or staged. If `Cargo.lock` appears from Cargo commands and the repository still does not track it, remove it before final status.

- [ ] **Step 8: Commit verification notes**

```bash
git add docs/superpowers/plans/2026-07-07-phase-1a-route-model-matcher-contract-plan.md
git commit -m "docs: record phase 1a matcher verification"
```

#### Verification Evidence

Recorded on 2026-07-07 after implementation commits `dcd2837` through `2b89032` and review-fix commit `b6d5c96`.

- `cargo fmt --all --check`: exit 0.
- `cargo test -p bevy_ui_router_core`: exit 0; 35 unit tests, 3 dependency-boundary integration tests, and 7 doc-tests passed.
- `cargo test -p bevy_ui_router_core --doc`: exit 0; 7 doc-tests passed.
- `cargo test -p bevy_ui_router_core --test dependency_boundary`: exit 0; 3 dependency-boundary tests passed.
- `cargo test -p bevy_ui_router --no-default-features`: exit 0; 3 facade integration tests passed.
- `cargo check -p bevy_ui_router --no-default-features --features platform`: exit 0.
- `cargo check -p bevy_ui_router --no-default-features --features bsn`: exit 0.
- `git ls-files Cargo.lock`: no tracked lockfile.
- `git status --short`: clean after removing the generated untracked `Cargo.lock`, before writing this verification evidence note.

Independent review found two implementation issues after the first verification pass: matcher conflict detection could run before considering all higher-specificity candidates, and route tree derived `Debug`/equality exposed internal matcher inputs. Commit `b6d5c96` fixed both and added regression coverage. A minor missing diagnostic conflict test was also added.

Cargo invocations that needed `rustc` through local `sccache` were rerun with approved elevated execution after sandboxed execution returned Windows `os error 5` before compilation.

---

## 7. Review Gate Before Implementation

Before implementing Phase 1A, review this plan with independent agents using these responsibility directions:

- Architecture and module-boundary review: verify Phase 1A stays core-only except facade boundary verification and does not leak transaction, ECS, adapter, BSN, or app assumptions.
- API and lifecycle review: verify public names, constructor visibility, id allocation, and matched branch semantics remain compatible with Phase 0 authority boundaries.
- Fact and source review: verify claims against current repository files and existing docs, including Bevy/BSN boundary statements already recorded in the project.
- Test and risk review: verify every semantic requirement has an executable test, every task can be rejected independently, and the verification commands match touched areas.

The lead agent must evaluate findings before accepting them. If multiple findings share a root cause such as excessive scope, public API overexposure, or hidden lifecycle coupling, revise the architecture instead of applying isolated edits.

---

## 8. Acceptance Criteria

- The final reviewed plan keeps Phase 1A limited to route pattern, route tree, local matched-route params, matcher, match errors, match diagnostics, and facade boundary verification.
- The plan has no ECS, BSN, platform adapter, async resolve/apply, retained subtree, focus, or host mutation implementation tasks.
- The plan explicitly states that Phase 1A is not the full Phase 1 gate and that Phase 1B must provide branch diff, branch fingerprint, named slot placement, and error-boundary declaration inputs before Phase 2 begins.
- The plan defines deterministic match ranking and conflict behavior without relying on query iteration order or hash iteration order.
- The plan preserves Phase 0 public-constructor boundaries for ids, generation, diagnostic identity, and active/pending diagnostic state.
- The plan defines `MatchedRoute::params()` as local params only and does not require leaf routes to contain ancestor params.
- The plan includes focused tests for valid patterns, invalid patterns, route tree construction, root path normalization, successful matching, local params, not found, ranking, index-vs-pathless precedence, conflicts, errors, diagnostics, dependency boundaries, doctests, and facade exports.
- The plan distinguishes RED evidence from coverage and boundary-verification steps; only explicitly labeled RED steps require a failing command before implementation.
- The plan lists concrete verification commands and requires command output before success claims.
