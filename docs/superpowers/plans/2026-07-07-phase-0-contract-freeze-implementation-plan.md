# Phase 0 Contract Freeze Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Phase 0 workspace and core contract baseline so later phases can add matching, transactions, ECS, and BSN without contaminating core.

**Architecture:** Migrate the current single package into a Cargo workspace with a core crate, a facade crate, and minimal adapter-boundary crates. Phase 0 implements only core-neutral contracts: raw location, controlled ids, memory history, router-neutral back types, error taxonomy, diagnostics snapshots, ownership cleanup scope, and dependency graph gates.

**Tech Stack:** Rust 2024, Cargo workspace resolver 3, Markdown plans, PowerShell verification commands, `cargo fmt`, `cargo check`, `cargo test`.

---

## 1. Scope Check

Phase 0 is a contract-freeze and workspace-baseline phase. It prepares the ground for later phases but does not implement route matching, branch diff, runtime navigation transaction, Bevy ECS hierarchy mutation, BSN rendering, platform packages, async loaders, typed routes, retained entity cache, transitions, or examples.

This plan is executable only after it passes review. It does not authorize code implementation by itself.

---

## 2. Required Reading

Before implementing any task, the worker must read:

- `AGENTS.md`
- `docs/superpowers/specs/2026-07-07-project-decisions-design.md`
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-plan.md`
- `docs/superpowers/specs/2026-07-07-project-architecture-roadmap-review-log.md`
- `docs/superpowers/plans/2026-07-07-phase-0-3-execution-flow.md`
- `docs/superpowers/plans/2026-07-07-implementation-readiness-brief.md`
- `docs/superpowers/plans/2026-07-07-implementation-readiness-brief-review-log.md`
- `docs/01-BSN-scene-authoring-boundaries.md`
- `docs/02-Web-router-patterns-for-Bevy.md`
- `docs/03-Core-architecture-and-MVP.md`
- `docs/04-Risks-tests-and-handoff.md`
- `docs/05-Non-MVP-capabilities-in-Bevy.md`

Record the reading list in the Phase 0 gate evidence before implementation starts.

---

## 3. File Structure

Phase 0 creates or modifies these files.

```text
Cargo.toml
src/main.rs
crates/bevy_ui_router_core/Cargo.toml
crates/bevy_ui_router_core/src/lib.rs
crates/bevy_ui_router_core/src/back.rs
crates/bevy_ui_router_core/src/diagnostics.rs
crates/bevy_ui_router_core/src/error.rs
crates/bevy_ui_router_core/src/history.rs
crates/bevy_ui_router_core/src/ids.rs
crates/bevy_ui_router_core/src/location.rs
crates/bevy_ui_router_core/src/ownership.rs
crates/bevy_ui_router_core/tests/dependency_boundary.rs
crates/bevy_ui_router_view/Cargo.toml
crates/bevy_ui_router_view/src/lib.rs
crates/bevy_ui_router_ecs/Cargo.toml
crates/bevy_ui_router_ecs/src/lib.rs
crates/bevy_ui_router_bsn/Cargo.toml
crates/bevy_ui_router_bsn/src/lib.rs
crates/bevy_ui_router_platform/Cargo.toml
crates/bevy_ui_router_platform/src/lib.rs
crates/bevy_ui_router/Cargo.toml
crates/bevy_ui_router/src/lib.rs
crates/bevy_ui_router/tests/facade_features.rs
```

`src/main.rs` is deleted during migration because the repository root becomes a workspace root rather than the package that contains the router facade.

Files reserved for later phases:

- `crates/bevy_ui_router_core/src/route_tree.rs`: Phase 1.
- `crates/bevy_ui_router_core/src/matcher.rs`: Phase 1.
- `crates/bevy_ui_router_core/src/branch.rs`: Phase 1.
- `crates/bevy_ui_router_core/src/frame.rs`: Phase 2.
- `examples/minimal_nested_routes.rs`: Phase 3D.

---

## 4. Phase 0 Frozen Decisions

Phase 0 freezes these decisions:

- `bevy_ui_router_core` has no Bevy, BSN, platform, view, or app-profile dependency.
- `bevy_ui_router` remains the facade crate.
- Facade features are `view`, `ecs`, `bsn`, `platform`, and `devtools`.
- Facade default is `["view"]`, so default does not pull Bevy.
- `bsn` implies `ecs` at the facade feature level.
- ECS, BSN, and platform crates exist as boundary packages but do not add Bevy or BSN behavior in Phase 0.
- Core owns `BackIntent`, `BackPolicy`, and `BackCapabilitySnapshot`.
- Core id constructors for router, host, slot, route, frame, and generation are crate-visible; public API exposes observation methods only.
- Core diagnostic identity and active/pending event constructors are crate-visible; public API exposes observation methods only.
- Core cleanup scope can describe only router-owned subtree cleanup in Phase 0 and must explicitly exclude app-owned host cleanup.
- `GuardRejected` is reserved in the error taxonomy but has no reachable behavior in Phase 0.
- `RouteLocation` payload, app extension, and typed search are outside Phase 0.
- `ViewBindingKey` in the view crate is a Phase 0 view-local placeholder. It does not authorize core to depend on view, and Phase 1 must separately freeze any renderer-neutral binding key if one is needed.

If implementation changes any frozen decision, stop and revise this plan through the same review flow before coding continues.

---

## 5. Evidence Ledger Template

Each task must append an evidence entry to the worker's task notes before commit:

```text
Task:
Required reading recorded:
Files changed:
RED command:
RED exit code:
RED output summary:
GREEN command:
GREEN exit code:
GREEN output summary:
Verification commands:
Verification exit codes:
Verification output summaries:
Commit SHA or reason before commit boundary:
Skipped or unavailable commands:
Blocked or unverified commands:
User-accepted exceptions:
Risk notes:
```

If a command cannot run because a crate does not exist yet, the entry must name the task that creates the crate and the later step that reruns the command.

For each verification command, record evidence in this shape:

```text
Command:
Exit code:
Output summary:
Evidence status: verified | blocked | skipped | user-accepted-risk
```

---

## 6. Pre-Implementation Acceptance Checklist

Implementation may start only after these items are recorded in the worker's notes:

- This plan has passed the required multi-agent review loop.
- Review findings have been evaluated by the lead for shared root causes.
- All accepted findings have been incorporated into this plan.
- `docs/superpowers/plans/2026-07-07-implementation-readiness-brief.md` has been read.
- `docs/superpowers/plans/2026-07-07-implementation-readiness-brief-review-log.md` has been read.
- Required reading from Section 2 has been recorded.
- The user has confirmed the execution mode for code implementation.

No code implementation task may begin before this checklist is complete.

---

## 7. Implementation Tasks

### Task 1: Workspace And Crate Skeleton

**Files:**
- Modify: `Cargo.toml`
- Delete: `src/main.rs`
- Create: `crates/bevy_ui_router_core/Cargo.toml`
- Create: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_view/Cargo.toml`
- Create: `crates/bevy_ui_router_view/src/lib.rs`
- Create: `crates/bevy_ui_router_ecs/Cargo.toml`
- Create: `crates/bevy_ui_router_ecs/src/lib.rs`
- Create: `crates/bevy_ui_router_bsn/Cargo.toml`
- Create: `crates/bevy_ui_router_bsn/src/lib.rs`
- Create: `crates/bevy_ui_router_platform/Cargo.toml`
- Create: `crates/bevy_ui_router_platform/src/lib.rs`
- Create: `crates/bevy_ui_router/Cargo.toml`
- Create: `crates/bevy_ui_router/src/lib.rs`

- [ ] **Step 1: Record the current failing boundary**

Run:

```powershell
powershell -NoProfile -Command "if (Select-String -LiteralPath 'Cargo.toml' -Pattern '^\s*bevy\s*=' -Quiet) { exit 1 } else { exit 0 }"
```

Expected: exit code 1. The current root package has an unconditional `bevy` dependency, so the Phase 0 facade boundary is not yet satisfied.

- [ ] **Step 2: Replace the root manifest with a workspace manifest**

Replace `Cargo.toml` with:

```toml
[workspace]
members = [
    "crates/bevy_ui_router_core",
    "crates/bevy_ui_router_view",
    "crates/bevy_ui_router_ecs",
    "crates/bevy_ui_router_bsn",
    "crates/bevy_ui_router_platform",
    "crates/bevy_ui_router",
]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
```

Delete `src/main.rs`.

- [ ] **Step 3: Create the core crate skeleton**

Create `crates/bevy_ui_router_core/Cargo.toml`:

```toml
[package]
name = "bevy_ui_router_core"
version.workspace = true
edition.workspace = true

[dependencies]
```

Create `crates/bevy_ui_router_core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod prelude {}
```

- [ ] **Step 4: Create the view crate skeleton**

Create `crates/bevy_ui_router_view/Cargo.toml`:

```toml
[package]
name = "bevy_ui_router_view"
version.workspace = true
edition.workspace = true

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
```

Create `crates/bevy_ui_router_view/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ViewBindingKey(String);

impl ViewBindingKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub mod prelude {
    pub use crate::ViewBindingKey;
}
```

This key is intentionally view-local in Phase 0. It must not be referenced from `bevy_ui_router_core`, and it does not settle the Phase 1 renderer-neutral binding model.

- [ ] **Step 5: Create adapter-boundary crate skeletons**

Create `crates/bevy_ui_router_ecs/Cargo.toml`:

```toml
[package]
name = "bevy_ui_router_ecs"
version.workspace = true
edition.workspace = true

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
bevy_ui_router_view = { path = "../bevy_ui_router_view" }
```

Create `crates/bevy_ui_router_ecs/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod prelude {}
```

Create `crates/bevy_ui_router_bsn/Cargo.toml`:

```toml
[package]
name = "bevy_ui_router_bsn"
version.workspace = true
edition.workspace = true

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
bevy_ui_router_ecs = { path = "../bevy_ui_router_ecs" }
bevy_ui_router_view = { path = "../bevy_ui_router_view" }
```

Create `crates/bevy_ui_router_bsn/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod prelude {}
```

Create `crates/bevy_ui_router_platform/Cargo.toml`:

```toml
[package]
name = "bevy_ui_router_platform"
version.workspace = true
edition.workspace = true

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
```

Create `crates/bevy_ui_router_platform/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod prelude {}
```

- [ ] **Step 6: Create the facade crate skeleton**

Create `crates/bevy_ui_router/Cargo.toml`:

```toml
[package]
name = "bevy_ui_router"
version.workspace = true
edition.workspace = true

[features]
default = ["view"]
view = ["dep:bevy_ui_router_view"]
ecs = ["dep:bevy_ui_router_ecs"]
bsn = ["ecs", "dep:bevy_ui_router_bsn"]
platform = ["dep:bevy_ui_router_platform"]
devtools = []

[dependencies]
bevy_ui_router_core = { path = "../bevy_ui_router_core" }
bevy_ui_router_view = { path = "../bevy_ui_router_view", optional = true }
bevy_ui_router_ecs = { path = "../bevy_ui_router_ecs", optional = true }
bevy_ui_router_bsn = { path = "../bevy_ui_router_bsn", optional = true }
bevy_ui_router_platform = { path = "../bevy_ui_router_platform", optional = true }
```

Create `crates/bevy_ui_router/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub use bevy_ui_router_core as core;

#[cfg(feature = "view")]
pub use bevy_ui_router_view as view;

#[cfg(feature = "ecs")]
pub use bevy_ui_router_ecs as ecs;

#[cfg(feature = "bsn")]
pub use bevy_ui_router_bsn as bsn;

#[cfg(feature = "platform")]
pub use bevy_ui_router_platform as platform;

pub mod prelude {
    pub use bevy_ui_router_core::prelude::*;

    #[cfg(feature = "view")]
    pub use bevy_ui_router_view::prelude::*;
}
```

- [ ] **Step 7: Verify the workspace skeleton**

Run:

```powershell
cargo metadata --format-version 1 --no-deps
cargo check -p bevy_ui_router --no-default-features
```

Expected: both commands exit successfully. The second command must not fetch or compile Bevy because the facade no-default configuration depends only on core.

- [ ] **Step 8: Commit the skeleton**

Run:

```powershell
git add Cargo.toml src/main.rs crates
git commit -m "chore: scaffold router workspace"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 2: Core Ids And Generation

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_core/src/ids.rs`

- [ ] **Step 1: Write failing id tests**

Replace `crates/bevy_ui_router_core/src/lib.rs` with:

```rust
#![forbid(unsafe_code)]

pub mod ids;

pub mod prelude {
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
}
```

Create `crates/bevy_ui_router_core/src/ids.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_expose_raw_values_without_public_construction() {
        let router = RouterInstanceId::new(7);
        let host = RouteHostId::new(8);
        let slot = RouteSlotId::new(9);
        let route = RouteId::new(10);
        let frame = RouteFrameId::new(11);

        assert_eq!(router.raw(), 7);
        assert_eq!(host.raw(), 8);
        assert_eq!(slot.raw(), 9);
        assert_eq!(route.raw(), 10);
        assert_eq!(frame.raw(), 11);
    }

    #[test]
    fn generation_advances_inside_core() {
        let first = RouteGeneration::initial();
        let second = first.next();

        assert_eq!(first.raw(), 0);
        assert_eq!(second.raw(), 1);
        assert_ne!(first, second);
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```powershell
cargo test -p bevy_ui_router_core ids::
```

Expected: fails because `RouterInstanceId`, `RouteHostId`, `RouteSlotId`, `RouteId`, `RouteFrameId`, and `RouteGeneration` are referenced by tests but not defined.

- [ ] **Step 3: Implement controlled id types**

Replace `crates/bevy_ui_router_core/src/ids.rs` with:

````rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteFrameId;
//!
//! let _ = RouteFrameId(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteFrameId;
//!
//! let _ = RouteFrameId::new(1);
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::RouteGeneration;
//!
//! let generation = RouteGeneration::initial();
//! let _ = generation.next();
//! ```

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            pub(crate) const fn new(raw: u64) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(RouterInstanceId);
id_type!(RouteHostId);
id_type!(RouteSlotId);
id_type!(RouteId);
id_type!(RouteFrameId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RouteGeneration(u64);

impl RouteGeneration {
    pub(crate) const fn initial() -> Self {
        Self(0)
    }

    pub(crate) const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_expose_raw_values_without_public_construction() {
        let router = RouterInstanceId::new(7);
        let host = RouteHostId::new(8);
        let slot = RouteSlotId::new(9);
        let route = RouteId::new(10);
        let frame = RouteFrameId::new(11);

        assert_eq!(router.raw(), 7);
        assert_eq!(host.raw(), 8);
        assert_eq!(slot.raw(), 9);
        assert_eq!(route.raw(), 10);
        assert_eq!(frame.raw(), 11);
    }

    #[test]
    fn generation_advances_inside_core() {
        let first = RouteGeneration::initial();
        let second = first.next();

        assert_eq!(first.raw(), 0);
        assert_eq!(second.raw(), 1);
        assert_ne!(first, second);
    }
}
````

- [ ] **Step 4: Run the id tests**

Run:

```powershell
cargo test -p bevy_ui_router_core ids::
```

Expected: tests pass.

- [ ] **Step 5: Commit the id contract**

Run:

```powershell
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/ids.rs
git commit -m "feat(core): add controlled route identity types"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 3: Raw RouteLocation

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_core/src/location.rs`

- [ ] **Step 1: Write failing location tests**

Update `crates/bevy_ui_router_core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod ids;
pub mod location;

pub mod prelude {
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
}
```

Create `crates/bevy_ui_router_core/src/location.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_empty_and_relative_path_to_raw_path_boundary() {
        assert_eq!(RouteLocation::new("").path(), "/");
        assert_eq!(RouteLocation::new("settings").path(), "/settings");
    }

    #[test]
    fn keeps_raw_search_and_fragment_separate() {
        let location = RouteLocation::new("/items")
            .with_search("a=1&a=2")
            .with_fragment("details");

        assert_eq!(location.path(), "/items");
        assert_eq!(location.search(), Some("a=1&a=2"));
        assert_eq!(location.fragment(), Some("details"));
    }
}
```

- [ ] **Step 2: Run the failing location tests**

Run:

```powershell
cargo test -p bevy_ui_router_core location::
```

Expected: fails because `RouteLocation` and its methods are referenced by tests but not defined.

- [ ] **Step 3: Implement raw RouteLocation**

Replace `crates/bevy_ui_router_core/src/location.rs` with:

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RouteLocation {
    path: String,
    search: Option<String>,
    fragment: Option<String>,
}

impl RouteLocation {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: normalize_path(path.into()),
            search: None,
            fragment: None,
        }
    }

    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }
}

fn normalize_path(mut path: String) -> String {
    if path.is_empty() {
        return "/".to_owned();
    }

    if !path.starts_with('/') {
        path.insert(0, '/');
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_empty_and_relative_path_to_raw_path_boundary() {
        assert_eq!(RouteLocation::new("").path(), "/");
        assert_eq!(RouteLocation::new("settings").path(), "/settings");
    }

    #[test]
    fn keeps_raw_search_and_fragment_separate() {
        let location = RouteLocation::new("/items")
            .with_search("a=1&a=2")
            .with_fragment("details");

        assert_eq!(location.path(), "/items");
        assert_eq!(location.search(), Some("a=1&a=2"));
        assert_eq!(location.fragment(), Some("details"));
    }
}
```

- [ ] **Step 4: Run the location tests**

Run:

```powershell
cargo test -p bevy_ui_router_core location::
```

Expected: tests pass.

- [ ] **Step 5: Commit the location contract**

Run:

```powershell
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/location.rs
git commit -m "feat(core): add raw route location"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 4: Memory History

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_core/src/history.rs`

- [ ] **Step 1: Write failing history tests**

Update `crates/bevy_ui_router_core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod history;
pub mod ids;
pub mod location;

pub mod prelude {
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
}
```

Create `crates/bevy_ui_router_core/src/history.rs` with tests first:

```rust
use crate::location::RouteLocation;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_replace_and_back_track_memory_locations() {
        let mut history = MemoryHistory::new();

        history.push(RouteLocation::new("/"));
        history.push(RouteLocation::new("/settings"));
        history.replace(RouteLocation::new("/settings/profile"));

        assert_eq!(history.current().map(RouteLocation::path), Some("/settings/profile"));
        assert_eq!(history.back().map(|location| location.path().to_owned()), Ok("/".to_owned()));
        assert_eq!(history.current().map(RouteLocation::path), Some("/"));
    }

    #[test]
    fn back_reports_empty_and_start_boundaries() {
        let mut history = MemoryHistory::new();

        assert_eq!(history.back(), Err(HistoryError::Empty));

        history.push(RouteLocation::new("/"));

        assert_eq!(history.back(), Err(HistoryError::AtStart));
    }
}
```

- [ ] **Step 2: Run the failing history tests**

Run:

```powershell
cargo test -p bevy_ui_router_core history::
```

Expected: fails because `MemoryHistory` and `HistoryError` are referenced by tests but not defined.

- [ ] **Step 3: Implement memory history**

Replace `crates/bevy_ui_router_core/src/history.rs` with:

```rust
use crate::location::RouteLocation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryError {
    Empty,
    AtStart,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemoryHistory {
    entries: Vec<RouteLocation>,
    cursor: Option<usize>,
}

impl MemoryHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, location: RouteLocation) {
        match self.cursor {
            Some(cursor) => {
                self.entries.truncate(cursor + 1);
                self.entries.push(location);
                self.cursor = Some(self.entries.len() - 1);
            }
            None => {
                self.entries.push(location);
                self.cursor = Some(0);
            }
        }
    }

    pub fn replace(&mut self, location: RouteLocation) {
        match self.cursor {
            Some(cursor) => self.entries[cursor] = location,
            None => self.push(location),
        }
    }

    pub fn back(&mut self) -> Result<RouteLocation, HistoryError> {
        let cursor = self.cursor.ok_or(HistoryError::Empty)?;
        if cursor == 0 {
            return Err(HistoryError::AtStart);
        }

        let next_cursor = cursor - 1;
        self.cursor = Some(next_cursor);
        Ok(self.entries[next_cursor].clone())
    }

    pub fn current(&self) -> Option<&RouteLocation> {
        self.cursor.and_then(|cursor| self.entries.get(cursor))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_replace_and_back_track_memory_locations() {
        let mut history = MemoryHistory::new();

        history.push(RouteLocation::new("/"));
        history.push(RouteLocation::new("/settings"));
        history.replace(RouteLocation::new("/settings/profile"));

        assert_eq!(history.current().map(RouteLocation::path), Some("/settings/profile"));
        assert_eq!(history.back().map(|location| location.path().to_owned()), Ok("/".to_owned()));
        assert_eq!(history.current().map(RouteLocation::path), Some("/"));
    }

    #[test]
    fn back_reports_empty_and_start_boundaries() {
        let mut history = MemoryHistory::new();

        assert_eq!(history.back(), Err(HistoryError::Empty));

        history.push(RouteLocation::new("/"));

        assert_eq!(history.back(), Err(HistoryError::AtStart));
    }
}
```

- [ ] **Step 4: Run the history tests**

Run:

```powershell
cargo test -p bevy_ui_router_core history::
```

Expected: tests pass.

- [ ] **Step 5: Commit the history contract**

Run:

```powershell
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/history.rs
git commit -m "feat(core): add memory history"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 5: Back Intent And Error Taxonomy

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_core/src/back.rs`
- Create: `crates/bevy_ui_router_core/src/error.rs`
- Modify: `crates/bevy_ui_router_platform/src/lib.rs`

- [ ] **Step 1: Write failing back and error tests**

Update `crates/bevy_ui_router_core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod back;
pub mod error;
pub mod history;
pub mod ids;
pub mod location;

pub mod prelude {
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
}
```

Create `crates/bevy_ui_router_core/src/back.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_capability_snapshot_is_router_neutral() {
        let snapshot = BackCapabilitySnapshot::new(true, false, true);

        assert!(snapshot.can_pop_history());
        assert!(!snapshot.has_local_layer());
        assert!(snapshot.host_may_handle());
    }
}
```

Create `crates/bevy_ui_router_core/src/error.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_keeps_adapter_failures_router_neutral() {
        let error = RouteError::new(
            RouteErrorKind::AdapterFailed,
            RouteErrorSourceCategory::RendererContract,
            "renderer reported a redacted failure",
        );

        assert_eq!(error.kind(), RouteErrorKind::AdapterFailed);
        assert_eq!(error.source_category(), RouteErrorSourceCategory::RendererContract);
    }

    #[test]
    fn finalization_error_does_not_name_specific_integration_crate() {
        assert_eq!(format!("{:?}", RouteErrorKind::FinalizeFailed), "FinalizeFailed");
    }
}
```

- [ ] **Step 2: Run the failing back/error tests**

Run:

```powershell
cargo test -p bevy_ui_router_core back::
cargo test -p bevy_ui_router_core error::
```

Expected: fails because `BackCapabilitySnapshot`, `BackIntent`, `BackPolicy`, `RouteError`, `RouteErrorKind`, and `RouteErrorSourceCategory` are referenced by tests but not defined.

- [ ] **Step 3: Implement back intent types**

Replace `crates/bevy_ui_router_core/src/back.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackIntent {
    NavigateBack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackPolicy {
    PopHistory,
    Block,
    DelegateToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackCapabilitySnapshot {
    can_pop_history: bool,
    has_local_layer: bool,
    host_may_handle: bool,
}

impl BackCapabilitySnapshot {
    pub const fn new(
        can_pop_history: bool,
        has_local_layer: bool,
        host_may_handle: bool,
    ) -> Self {
        Self {
            can_pop_history,
            has_local_layer,
            host_may_handle,
        }
    }

    pub const fn can_pop_history(self) -> bool {
        self.can_pop_history
    }

    pub const fn has_local_layer(self) -> bool {
        self.has_local_layer
    }

    pub const fn host_may_handle(self) -> bool {
        self.host_may_handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_capability_snapshot_is_router_neutral() {
        let snapshot = BackCapabilitySnapshot::new(true, false, true);

        assert!(snapshot.can_pop_history());
        assert!(!snapshot.has_local_layer());
        assert!(snapshot.host_may_handle());
    }
}
```

- [ ] **Step 4: Implement router-neutral error taxonomy**

Replace `crates/bevy_ui_router_core/src/error.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteErrorKind {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteErrorSourceCategory {
    RouterCore,
    RendererContract,
    HostSlotIntegration,
    InputLocationIntegration,
    AppExtension,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteError {
    kind: RouteErrorKind,
    source_category: RouteErrorSourceCategory,
    redacted_message: String,
}

impl RouteError {
    pub fn new(
        kind: RouteErrorKind,
        source_category: RouteErrorSourceCategory,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source_category,
            redacted_message: redacted_message.into(),
        }
    }

    pub const fn kind(&self) -> RouteErrorKind {
        self.kind
    }

    pub const fn source_category(&self) -> RouteErrorSourceCategory {
        self.source_category
    }

    pub fn redacted_message(&self) -> &str {
        &self.redacted_message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_keeps_adapter_failures_router_neutral() {
        let error = RouteError::new(
            RouteErrorKind::AdapterFailed,
            RouteErrorSourceCategory::RendererContract,
            "renderer reported a redacted failure",
        );

        assert_eq!(error.kind(), RouteErrorKind::AdapterFailed);
        assert_eq!(error.source_category(), RouteErrorSourceCategory::RendererContract);
    }

    #[test]
    fn finalization_error_does_not_name_specific_integration_crate() {
        assert_eq!(format!("{:?}", RouteErrorKind::FinalizeFailed), "FinalizeFailed");
    }
}
```

- [ ] **Step 5: Run the back/error tests**

Run:

```powershell
cargo test -p bevy_ui_router_core back::
cargo test -p bevy_ui_router_core error::
```

Expected: tests pass.

- [ ] **Step 6: Wire platform re-export glue after core back types exist**

Replace `crates/bevy_ui_router_platform/src/lib.rs` with:

```rust
#![forbid(unsafe_code)]

pub mod prelude {
    pub use bevy_ui_router_core::prelude::{BackCapabilitySnapshot, BackIntent, BackPolicy};
}
```

Run:

```powershell
cargo check -p bevy_ui_router --no-default-features --features platform
```

Expected: exits successfully. The platform feature compiles the platform glue without adding any platform package dependency.

- [ ] **Step 7: Commit back and error contracts**

Run:

```powershell
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/back.rs crates/bevy_ui_router_core/src/error.rs crates/bevy_ui_router_platform/src/lib.rs
git commit -m "feat(core): add back intent and error taxonomy"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 6: Diagnostics Baseline

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_core/src/diagnostics.rs`

- [ ] **Step 1: Write failing diagnostics tests**

Update `crates/bevy_ui_router_core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod back;
pub mod diagnostics;
pub mod error;
pub mod history;
pub mod ids;
pub mod location;

pub mod prelude {
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticIdentity, DiagnosticSnapshot,
    };
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
}
```

Create `crates/bevy_ui_router_core/src/diagnostics.rs` with tests first:

```rust
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::location::RouteLocation;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_chain_records_router_host_slot_frame_generation_and_route() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
            Some(RouteId::new(5)),
        );

        assert_eq!(identity.router().raw(), 1);
        assert_eq!(identity.host().raw(), 2);
        assert_eq!(identity.slot().raw(), 3);
        assert_eq!(identity.frame().raw(), 4);
        assert_eq!(identity.generation().raw(), 0);
        assert_eq!(identity.route().map(RouteId::raw), Some(5));
    }

    #[test]
    fn pending_overwrite_and_missing_slot_are_explicit_events() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
            None,
        );

        let pending = DiagnosticEvent::pending_overwrite(identity, "newer navigation won");
        let missing = DiagnosticEvent::host_or_slot_unavailable("slot was not present");

        assert_eq!(pending.kind(), DiagnosticEventKind::PendingOverwrite);
        assert_eq!(pending.identity().map(DiagnosticIdentity::frame), Some(RouteFrameId::new(4)));
        assert_eq!(missing.kind(), DiagnosticEventKind::HostOrSlotUnavailable);
        assert!(missing.identity().is_none());
    }

    #[test]
    fn snapshot_keeps_requested_location_separate_from_active_identity() {
        let snapshot = DiagnosticSnapshot::new()
            .with_requested(RouteLocation::new("/settings"))
            .with_event(DiagnosticEvent::host_or_slot_unavailable("slot missing"));

        assert_eq!(snapshot.requested().map(RouteLocation::path), Some("/settings"));
        assert!(snapshot.active().is_none());
        assert_eq!(snapshot.events().len(), 1);
    }
}
```

- [ ] **Step 2: Run the failing diagnostics tests**

Run:

```powershell
cargo test -p bevy_ui_router_core diagnostics::
```

Expected: fails because diagnostics types are referenced by tests but not defined.

- [ ] **Step 3: Implement diagnostics snapshot types**

Replace `crates/bevy_ui_router_core/src/diagnostics.rs` with:

````rust
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{
//!     DiagnosticIdentity, RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId,
//!     RouterInstanceId,
//! };
//!
//! fn build(
//!     router: RouterInstanceId,
//!     host: RouteHostId,
//!     slot: RouteSlotId,
//!     frame: RouteFrameId,
//!     generation: RouteGeneration,
//!     route: Option<RouteId>,
//! ) {
//!     let _ = DiagnosticIdentity::new(router, host, slot, frame, generation, route);
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{DiagnosticEvent, DiagnosticIdentity};
//!
//! fn emit(identity: DiagnosticIdentity) {
//!     let _ = DiagnosticEvent::pending_overwrite(identity, "hidden");
//! }
//! ```
//!
//! ```compile_fail
//! use bevy_ui_router_core::prelude::{DiagnosticIdentity, DiagnosticSnapshot};
//!
//! fn activate(identity: DiagnosticIdentity) {
//!     let _ = DiagnosticSnapshot::new().with_active(identity);
//! }
//! ```

use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
};
use crate::location::RouteLocation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticIdentity {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    route: Option<RouteId>,
}

impl DiagnosticIdentity {
    pub(crate) const fn new(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        frame: RouteFrameId,
        generation: RouteGeneration,
        route: Option<RouteId>,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            frame,
            generation,
            route,
        }
    }

    pub const fn router(self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(self) -> RouteHostId {
        self.host
    }

    pub const fn slot(self) -> RouteSlotId {
        self.slot
    }

    pub const fn frame(self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(self) -> RouteGeneration {
        self.generation
    }

    pub const fn route(self) -> Option<RouteId> {
        self.route
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticEventKind {
    PendingOverwrite,
    HostOrSlotUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    kind: DiagnosticEventKind,
    identity: Option<DiagnosticIdentity>,
    redacted_message: String,
}

impl DiagnosticEvent {
    pub(crate) fn pending_overwrite(
        identity: DiagnosticIdentity,
        redacted_message: impl Into<String>,
    ) -> Self {
        Self {
            kind: DiagnosticEventKind::PendingOverwrite,
            identity: Some(identity),
            redacted_message: redacted_message.into(),
        }
    }

    pub(crate) fn host_or_slot_unavailable(redacted_message: impl Into<String>) -> Self {
        Self {
            kind: DiagnosticEventKind::HostOrSlotUnavailable,
            identity: None,
            redacted_message: redacted_message.into(),
        }
    }

    pub const fn kind(&self) -> DiagnosticEventKind {
        self.kind
    }

    pub const fn identity(&self) -> Option<DiagnosticIdentity> {
        self.identity
    }

    pub fn redacted_message(&self) -> &str {
        &self.redacted_message
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticSnapshot {
    requested: Option<RouteLocation>,
    active: Option<DiagnosticIdentity>,
    events: Vec<DiagnosticEvent>,
}

impl DiagnosticSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_requested(mut self, requested: RouteLocation) -> Self {
        self.requested = Some(requested);
        self
    }

    pub(crate) fn with_active(mut self, active: DiagnosticIdentity) -> Self {
        self.active = Some(active);
        self
    }

    pub(crate) fn with_event(mut self, event: DiagnosticEvent) -> Self {
        self.events.push(event);
        self
    }

    pub fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    pub const fn active(&self) -> Option<DiagnosticIdentity> {
        self.active
    }

    pub fn events(&self) -> &[DiagnosticEvent] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_chain_records_router_host_slot_frame_generation_and_route() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
            Some(RouteId::new(5)),
        );

        assert_eq!(identity.router().raw(), 1);
        assert_eq!(identity.host().raw(), 2);
        assert_eq!(identity.slot().raw(), 3);
        assert_eq!(identity.frame().raw(), 4);
        assert_eq!(identity.generation().raw(), 0);
        assert_eq!(identity.route().map(RouteId::raw), Some(5));
    }

    #[test]
    fn pending_overwrite_and_missing_slot_are_explicit_events() {
        let identity = DiagnosticIdentity::new(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
            None,
        );

        let pending = DiagnosticEvent::pending_overwrite(identity, "newer navigation won");
        let missing = DiagnosticEvent::host_or_slot_unavailable("slot was not present");

        assert_eq!(pending.kind(), DiagnosticEventKind::PendingOverwrite);
        assert_eq!(pending.identity().map(DiagnosticIdentity::frame), Some(RouteFrameId::new(4)));
        assert_eq!(missing.kind(), DiagnosticEventKind::HostOrSlotUnavailable);
        assert!(missing.identity().is_none());
    }

    #[test]
    fn snapshot_keeps_requested_location_separate_from_active_identity() {
        let snapshot = DiagnosticSnapshot::new()
            .with_requested(RouteLocation::new("/settings"))
            .with_event(DiagnosticEvent::host_or_slot_unavailable("slot missing"));

        assert_eq!(snapshot.requested().map(RouteLocation::path), Some("/settings"));
        assert!(snapshot.active().is_none());
        assert_eq!(snapshot.events().len(), 1);
    }
}
````

- [ ] **Step 4: Run the diagnostics tests**

Run:

```powershell
cargo test -p bevy_ui_router_core diagnostics::
```

Expected: tests pass.

- [ ] **Step 5: Commit the diagnostics baseline**

Run:

```powershell
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/diagnostics.rs
git commit -m "feat(core): add diagnostics baseline"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 7: Ownership Cleanup Scope Contract

**Files:**
- Modify: `crates/bevy_ui_router_core/src/lib.rs`
- Create: `crates/bevy_ui_router_core/src/ownership.rs`

- [ ] **Step 1: Write failing ownership scope tests**

Update `crates/bevy_ui_router_core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod back;
pub mod diagnostics;
pub mod error;
pub mod history;
pub mod ids;
pub mod location;
pub mod ownership;

pub mod prelude {
    pub use crate::back::{BackCapabilitySnapshot, BackIntent, BackPolicy};
    pub use crate::diagnostics::{
        DiagnosticEvent, DiagnosticEventKind, DiagnosticIdentity, DiagnosticSnapshot,
    };
    pub use crate::error::{RouteError, RouteErrorKind, RouteErrorSourceCategory};
    pub use crate::history::{HistoryError, MemoryHistory};
    pub use crate::ids::{
        RouteFrameId, RouteGeneration, RouteHostId, RouteId, RouteSlotId, RouterInstanceId,
    };
    pub use crate::location::RouteLocation;
    pub use crate::ownership::{CleanupScope, OwnedRegion};
}
```

Create `crates/bevy_ui_router_core/src/ownership.rs` with tests first:

```rust
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId, RouterInstanceId,
};

#[cfg(test)]
mod tests {
    use super::*;

    const PHASE_3B_HOST_INTEGRITY_CARRY_FORWARD: &str =
        "Phase 3B must prove ECS cleanup does not despawn or replace app-owned host entities, and any required relationship updates are explicit.";

    #[test]
    fn cleanup_scope_targets_only_router_owned_subtree() {
        let scope = CleanupScope::router_owned_subtree(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
        );

        assert_eq!(scope.router().raw(), 1);
        assert_eq!(scope.host().raw(), 2);
        assert_eq!(scope.slot().raw(), 3);
        assert_eq!(scope.frame().raw(), 4);
        assert_eq!(scope.generation().raw(), 0);
        assert_eq!(scope.region(), OwnedRegion::RouterOwnedSubtree);
        assert!(scope.excludes_app_owned_host());
    }

    #[test]
    fn phase_3b_host_integrity_verification_is_carried_forward() {
        let note = PHASE_3B_HOST_INTEGRITY_CARRY_FORWARD;

        assert!(note.contains("Phase 3B"));
        assert!(note.contains("app-owned host"));
        assert!(note.contains("does not despawn"));
    }
}
```

The Phase 3B carry-forward note is test evidence in this plan. It must not become public core API.

- [ ] **Step 2: Run the failing ownership tests**

Run:

```powershell
cargo test -p bevy_ui_router_core ownership::
```

Expected: fails because `CleanupScope` and `OwnedRegion` are referenced by tests but not defined.

- [ ] **Step 3: Implement the core-neutral cleanup scope**

Replace `crates/bevy_ui_router_core/src/ownership.rs` with:

```rust
use crate::ids::{
    RouteFrameId, RouteGeneration, RouteHostId, RouteSlotId, RouterInstanceId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedRegion {
    RouterOwnedSubtree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupScope {
    router: RouterInstanceId,
    host: RouteHostId,
    slot: RouteSlotId,
    frame: RouteFrameId,
    generation: RouteGeneration,
    region: OwnedRegion,
}

impl CleanupScope {
    pub(crate) const fn router_owned_subtree(
        router: RouterInstanceId,
        host: RouteHostId,
        slot: RouteSlotId,
        frame: RouteFrameId,
        generation: RouteGeneration,
    ) -> Self {
        Self {
            router,
            host,
            slot,
            frame,
            generation,
            region: OwnedRegion::RouterOwnedSubtree,
        }
    }

    pub const fn router(self) -> RouterInstanceId {
        self.router
    }

    pub const fn host(self) -> RouteHostId {
        self.host
    }

    pub const fn slot(self) -> RouteSlotId {
        self.slot
    }

    pub const fn frame(self) -> RouteFrameId {
        self.frame
    }

    pub const fn generation(self) -> RouteGeneration {
        self.generation
    }

    pub const fn region(self) -> OwnedRegion {
        self.region
    }

    pub const fn excludes_app_owned_host(self) -> bool {
        matches!(self.region, OwnedRegion::RouterOwnedSubtree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHASE_3B_HOST_INTEGRITY_CARRY_FORWARD: &str =
        "Phase 3B must prove ECS cleanup does not despawn or replace app-owned host entities, and any required relationship updates are explicit.";

    #[test]
    fn cleanup_scope_targets_only_router_owned_subtree() {
        let scope = CleanupScope::router_owned_subtree(
            RouterInstanceId::new(1),
            RouteHostId::new(2),
            RouteSlotId::new(3),
            RouteFrameId::new(4),
            RouteGeneration::initial(),
        );

        assert_eq!(scope.router().raw(), 1);
        assert_eq!(scope.host().raw(), 2);
        assert_eq!(scope.slot().raw(), 3);
        assert_eq!(scope.frame().raw(), 4);
        assert_eq!(scope.generation().raw(), 0);
        assert_eq!(scope.region(), OwnedRegion::RouterOwnedSubtree);
        assert!(scope.excludes_app_owned_host());
    }

    #[test]
    fn phase_3b_host_integrity_verification_is_carried_forward() {
        let note = PHASE_3B_HOST_INTEGRITY_CARRY_FORWARD;

        assert!(note.contains("Phase 3B"));
        assert!(note.contains("app-owned host"));
        assert!(note.contains("does not despawn"));
    }
}
```

- [ ] **Step 4: Run the ownership tests**

Run:

```powershell
cargo test -p bevy_ui_router_core ownership::
```

Expected: tests pass. This freezes only the abstract cleanup scope; it does not claim ECS host-integrity behavior is implemented.

- [ ] **Step 5: Commit the ownership contract**

Run:

```powershell
git add crates/bevy_ui_router_core/src/lib.rs crates/bevy_ui_router_core/src/ownership.rs
git commit -m "feat(core): add ownership cleanup scope"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 8: Boundary Regression And Dependency Graph Verification

This task is a regression and gate task, not a TDD construction task. It records whether Phase 0 boundaries hold after the earlier implementation tasks; failures identify drift that must be corrected before Phase 0 can be accepted.

**Files:**
- Create: `crates/bevy_ui_router_core/tests/dependency_boundary.rs`
- Create: `crates/bevy_ui_router/tests/facade_features.rs`

- [ ] **Step 1: Add boundary regression tests**

Create `crates/bevy_ui_router_core/tests/dependency_boundary.rs`:

```rust
use std::fs;
use std::path::Path;

#[test]
fn core_manifest_does_not_depend_on_forbidden_layers() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("core manifest should be readable");

    for line in manifest.lines().map(str::trim) {
        assert!(!line.starts_with("bevy"));
        assert!(!line.starts_with("bevy_ui_router_view"));
        assert!(!line.starts_with("bevy_ui_router_ecs"));
        assert!(!line.starts_with("bevy_ui_router_bsn"));
        assert!(!line.starts_with("bevy_ui_router_platform"));
    }
}
```

Create `crates/bevy_ui_router/tests/facade_features.rs`:

```rust
use std::fs;
use std::path::Path;

#[test]
fn facade_default_keeps_bevy_out_of_the_default_path() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("facade manifest should be readable");

    assert!(manifest.contains("default = [\"view\"]"));
    assert!(manifest.contains("bsn = [\"ecs\", \"dep:bevy_ui_router_bsn\"]"));

    for line in manifest.lines().map(str::trim) {
        assert!(!line.starts_with("bevy ="));
    }
}
```

- [ ] **Step 2: Run boundary regression tests and constructor visibility doctests**

Run:

```powershell
cargo test -p bevy_ui_router_core --test dependency_boundary
cargo test -p bevy_ui_router --test facade_features
cargo test -p bevy_ui_router_core --doc
```

Expected: commands pass. The doc tests must include the `compile_fail` examples proving external crates cannot construct route frame ids, route generations, diagnostic identities, or active/pending diagnostic events through public API.

- [ ] **Step 3: Verify no-default facade dependency graph**

Run:

```powershell
$tree = cargo tree -p bevy_ui_router --no-default-features
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$tree
$forbidden = '(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v'
if ($tree -match $forbidden) { exit 1 }
```

Expected: exit code 0. The tree may include workspace crates named `bevy_ui_router_*`, but it must not include external Bevy package-family crates or `bsn` packages. The negative lookahead intentionally excludes workspace crates while still matching package names such as `bevy_ecs`, `bevy_core_pipeline`, and `bevy_dev_tools`.

- [ ] **Step 4: Verify default facade dependency graph**

Run:

```powershell
$tree = cargo tree -p bevy_ui_router
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$tree
$forbidden = '(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v'
if ($tree -match $forbidden) { exit 1 }
```

Expected: exit code 0. The default `view` feature may include `bevy_ui_router_view`; it must not include external Bevy package-family crates or BSN packages.

- [ ] **Step 5: Verify BSN feature stub dependency graph**

Run:

```powershell
$tree = cargo tree -p bevy_ui_router --no-default-features --features bsn
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$tree
$forbidden = '(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v'
if ($tree -match $forbidden) { exit 1 }
```

Expected: exit code 0. The Phase 0 `bsn` feature may include `bevy_ui_router_bsn` and `bevy_ui_router_ecs`; it must not pull external Bevy package-family crates or BSN packages until a later reviewed adapter phase.

- [ ] **Step 6: Correct boundary drift if a gate failed**

If any boundary gate fails, align `crates/bevy_ui_router/Cargo.toml` with the feature block from Task 1 Step 6 and remove forbidden dependencies from the drifted crate:

```toml
[features]
default = ["view"]
view = ["dep:bevy_ui_router_view"]
ecs = ["dep:bevy_ui_router_ecs"]
bsn = ["ecs", "dep:bevy_ui_router_bsn"]
platform = ["dep:bevy_ui_router_platform"]
devtools = []
```

After correcting drift, rerun every command from Steps 2 through 5 and record both the failing and corrected evidence.

- [ ] **Step 7: Commit boundary regression tests**

Run:

```powershell
git add crates/bevy_ui_router_core/tests/dependency_boundary.rs crates/bevy_ui_router/tests/facade_features.rs crates/bevy_ui_router/Cargo.toml
git commit -m "test: lock phase 0 dependency boundaries"
```

Expected: commit succeeds. Record the commit SHA in the evidence ledger.

### Task 9: Phase 0 Full Verification

**Files:**
- Read: all Phase 0 files
- Modify: no production files unless verification exposes a defect

- [ ] **Step 1: Run formatting**

Run:

```powershell
cargo fmt --all --check
```

Expected: exits successfully. If it fails, run `cargo fmt --all`, inspect the diff, and rerun `cargo fmt --all --check`.

- [ ] **Step 2: Run core tests**

Run:

```powershell
cargo test -p bevy_ui_router_core
```

Expected: all core tests pass, including ids, location, history, back, error, diagnostics, ownership, and dependency boundary tests.

- [ ] **Step 3: Run core doctests**

Run:

```powershell
cargo test -p bevy_ui_router_core --doc
```

Expected: all doctests pass, including `compile_fail` examples for public constructor boundaries.

- [ ] **Step 4: Run facade no-default check**

Run:

```powershell
cargo check -p bevy_ui_router --no-default-features
```

Expected: exits successfully without compiling Bevy.

- [ ] **Step 5: Re-run dependency graph proofs**

Run:

```powershell
$tree = cargo tree -p bevy_ui_router --no-default-features
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$tree
$forbidden = '(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v'
if ($tree -match $forbidden) { exit 1 }

$tree = cargo tree -p bevy_ui_router
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$tree
$forbidden = '(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v'
if ($tree -match $forbidden) { exit 1 }

$tree = cargo tree -p bevy_ui_router --no-default-features --features bsn
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$tree
$forbidden = '(^|\s)bevy(?!_ui_router)(_[a-z0-9]+)* v|(^|\s)bsn v'
if ($tree -match $forbidden) { exit 1 }
```

Expected: all three dependency graph checks exit successfully and show no external Bevy package-family crates or BSN packages.

- [ ] **Step 6: Run facade feature checks and tests**

Run:

```powershell
cargo check -p bevy_ui_router --no-default-features --features platform
cargo check -p bevy_ui_router --no-default-features --features bsn
cargo test -p bevy_ui_router --no-default-features
cargo test -p bevy_ui_router
```

Expected: all commands pass. The default facade test may compile `bevy_ui_router_view`; it must not compile Bevy. The `bsn` feature command compiles only the Phase 0 boundary stub and must not add Bevy or BSN behavior.

- [ ] **Step 7: Record Phase 0 gate evidence**

Create or append the Phase 0 evidence entry in the worker notes:

```text
Phase 0 gate evidence
Required reading recorded:
Final commit SHA:
Command evidence:
- Command:
- Exit code:
- Output summary:
- Evidence status:
cargo fmt --all --check:
cargo test -p bevy_ui_router_core:
cargo test -p bevy_ui_router_core --doc:
cargo check -p bevy_ui_router --no-default-features:
cargo tree -p bevy_ui_router --no-default-features:
cargo tree -p bevy_ui_router:
cargo tree -p bevy_ui_router --no-default-features --features bsn:
cargo check -p bevy_ui_router --no-default-features --features platform:
cargo check -p bevy_ui_router --no-default-features --features bsn:
cargo test -p bevy_ui_router --no-default-features:
cargo test -p bevy_ui_router:
Skipped or unavailable commands:
Blocked or unverified commands:
User-accepted exceptions:
Review checklist:
- core has no Bevy dependency:
- facade no-default does not pull Bevy:
- facade default does not pull Bevy:
- facade bsn feature stub does not pull external Bevy or BSN packages:
- public API cannot construct frame ids, generations, diagnostic identities, or active/pending diagnostic events:
- diagnostics express router, host, slot, frame, generation:
- cleanup scope excludes app-owned host:
- Phase 3B host-integrity verification remains explicitly carried forward:
- no BSN or app-specific concept entered core:
Unresolved risks:
Authorized next step:
```

- [ ] **Step 8: Commit Phase 0 completion docs if created**

If a separate evidence file is created by the implementation worker, commit it:

```powershell
git add docs/superpowers/plans
git commit -m "docs: record phase 0 completion"
```

Expected: commit succeeds. If evidence is kept in the worker's response rather than a file, state that explicitly and do not create a docs-only commit.

---

## 8. Phase 0 Acceptance Gate

Phase 0 implementation can be accepted only when all items are true:

- Workspace root is no longer a Rust package.
- `src/main.rs` is removed.
- `bevy_ui_router_core` compiles without Bevy, BSN, platform, view, or app-profile dependencies.
- `bevy_ui_router` facade default is `["view"]`.
- `bevy_ui_router --no-default-features` compiles with core only.
- Core has raw `RouteLocation`.
- Core has controlled id and generation types.
- Core has memory history `push`, `replace`, and `back`.
- Core has router-neutral back intent/capability types.
- Core has router-neutral error taxonomy.
- Core diagnostics can express router, host, slot, frame, generation, route, requested location, pending overwrite, and missing host/slot.
- Core cleanup scope explicitly targets only router-owned subtree and excludes app-owned host cleanup.
- Phase 3B ECS host-integrity verification remains explicitly carried forward.
- Boundary tests, doctests, and dependency graph checks prove core dependency isolation, facade feature defaults, no external Bevy/BSN packages in Phase 0 feature paths, and public constructor boundaries.
- No Phase 1 matcher, Phase 2 transaction runtime, Phase 3 ECS behavior, or BSN adapter behavior is implemented.
- Required verification commands have fresh exit codes and output summaries.
- Failed required commands block acceptance.
- Environment-limited commands are marked unverified or blocked unless the user explicitly accepts the risk in writing.

---

## 9. Review Gate Before Implementation

This plan must be reviewed before implementation starts. The review must cover at least these responsibility directions:

- Process and gate alignment.
- Phase 0 scope and task granularity.
- Core/API boundary and dependency ownership.
- TDD, verification, and evidence adequacy.
- Fact consistency with the current repository.

The lead must evaluate review results for shared root causes before editing this plan. If a finding implies the workspace split, feature baseline, or public type ownership decision is wrong, pause and revise the higher-level planning documents first.

---

## 10. Execution Handoff

After this plan passes review and document checks, offer execution mode:

```text
Phase 0 plan is ready. Execution options:
1. Subagent-Driven: one worker per bounded task, with review between tasks.
2. Inline Execution: execute in this session with checkpoints.
```

Code implementation may begin only after the user confirms one execution mode.
