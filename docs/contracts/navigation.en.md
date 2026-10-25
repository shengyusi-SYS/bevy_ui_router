# Navigation and observation contract

[中文](navigation.md) | [English](navigation.en.md)

This page explains how pages match and reuse content, and how navigation, history, and scroll restoration behave. See [lifecycle](lifecycle.en.md) for publication and failure settlement, and [Scenes](scene.en.md) for view execution.

## Matching and location updates

The matcher selects a complete branch by comparing cumulative specificity in this order: more consumed path segments, more static segments, more parameter segments, more index routes, fewer wildcards, then fewer pathless routes. It compares the whole branch, with stable traversal order.

Equal top-ranked candidates with different leaves return `MatchConflict`. For example, sibling patterns `a/:x` and `:x/b` tie for `/a/b`. Declaration order determines traversal and diagnostic order. Invalid definitions fail during construction or installation; no complete match produces a route failure.

Static segments match exactly. `:name` matches one segment; a final `*name` captures the remaining segments, including an empty remainder. Index routes match only when no segments remain; pathless routes consume none. Patterns strip leading `/`; interior empty segments and non-root trailing `/` are invalid. Capture names must be nonempty ASCII letters, digits, or underscores and unique within the pattern. Params are local to each route; ancestor params remain in their own route scope.

`RouteLocation` normalizes the path to one leading `/`, removes empty segments, and removes non-root trailing `/`. Pass search and fragment separately with `with_search` / `with_fragment`. It treats the path as a path string: full URLs, `?`/`#` separation, URL decoding, and `.`/`..` resolution belong to the caller.

Search compares optional raw strings exactly. Missing, empty, `?` prefixes, key order, repeated-value order, and percent encoding all matter. Fragment uses the same raw optional-string comparison and preserves a supplied `#` prefix.

| Navigation change | View/lifecycle behavior |
| --- | --- |
| Switch child under the same ancestors | Keep the unchanged prefix; create a fresh suffix or reuse eligible retained instances |
| Params or reuse identity changes at a level | Replace from that level onward; keep the earlier prefix |
| Raw search changes | Rebuild from the root by default; retain a consecutive prefix whose nodes all opt in and keep the same identity |
| Only fragment changes | Keep ancestors and rebuild the deepest normal frame; keep frame/slot identities and advance generation |
| Canonical path, search, and fragment are equivalent | Keep the normal presentation structure; history still follows the command, and explicit scroll restoration may write positions |

A fresh or rebuilt view receives an immutable `RouteViewInput` factory snapshot. Preserved layouts and retained hits keep their local state and skip the factory. Applications read this publication's inputs with `RouteInputs::current(Entity)`.

`RouteNodeOptions::with_preserve_on_search(true)` opts a node into reuse across raw search changes; the default is false. It applies to both current-prefix reuse and retained hits while keeping params, route/view/slot/key, and parent-scope identity checks. Each level opts in separately. When search and fragment both change and the whole chain would otherwise be reusable, ancestors remain and the deepest frame rebuilds with a new generation. Preserved content reads new search values through `RouteInputs::current`.

`RouteInputRef` has five read-only getters: `route()`, `params()`, `location()`, `origin()`, and `failure()`. Search/fragment are available through `location()`. Params belong to the local route; the full location belongs to the current presentation. Normal frames have no failure. A route-error frame uses the boundary route, its local params, and the original failure; normal ancestors keep no failure. Router fallback has no route, empty params, and the original failure. Error/fallback location is the failed request's location.

Publication switches all target inputs together, including preserved and retained content. Pending, stale, or rejected work leaves active inputs unchanged. Inactive content returns no current inputs. New inputs are available immediately after publication, before request termination. Structural no-ops add no physical writes for input updates. See [activity queries](input-focus.en.md#page-activity-queries) for entity scope and borrowing.

Active error presentations use the normal execution path instead of the equivalent-normal-navigation shortcut. Reuse checks include route-tree provenance and route/slot/explicit-key identity.

Sources: [matcher](../../crates/bevy_ui_router_core/src/matcher.rs), [branch diff](../../crates/bevy_ui_router_core/src/branch_diff.rs), and [location](../../crates/bevy_ui_router_core/src/location.rs).

## Retained instances

`RouteNodeOptions::with_keep_alive(true)` enables retention for a route. `RouterConfig::with_keep_alive_capacity(n)` limits inactive instances per Router; its default is 0. Active instances consume no capacity. Routes without the option, or with zero capacity, use fresh creation and cleanup.

Normal pages and layouts can opt in separately. Their exact parent frame/generation, outlet, slot binding, and host incarnation must remain valid; the parent may be active or retained. Pages directly under the host without a normal parent frame, error/fallback content, failed content, and quarantined content stay outside the inventory. Retention uses one target chain, default outlets, and the same Router/host scope.

For `R/P/C → R/Q → R/P/D`, retaining P but not C preserves P and its outlet while cleaning up C. Returning reuses P and matches D. If both P and C opt in, C can remain independently retained inside inactive P; returning to P/D still selects D. If P is not retained or is evicted, its dependent inventory is cleaned up child-first.

Selection prefers reusable current content, then inventory in the exact parent scope, then fresh content. Among valid retained candidates, the most recently successfully deactivated instance wins. Parent identity is independent of its former child target and former full location.

A hit compares route/view reuse identity, params, and raw search within that parent scope. `preserve_on_search` allows a different search string for the opted-in node. The deepest target still requires an exact raw fragment match. A hit preserves original entities, local component state, and content handles and skips factory/resolve/apply. The request uses a new execution identity and completes publication. A miss creates a fresh Scene.

Inventory maintains a simple LRU by successful deactivation; failures leave the order unchanged. Hits leave inactive inventory, and each level evaluates retention separately. Capacity eviction selects the oldest candidate and cleans up its dependency closure, so the remaining count can be below capacity. Invalid parent/outlet/slot or host scope is settled through the [lifecycle protocol](lifecycle.en.md#inactive-instances-and-long-lived-owners).

A single deactivation adds children before parents. With capacity 1 and both opted in, the parent can remain while the child is cleaned up. Selected instances and their parent dependencies are protected from capacity eviction.

Keep-alive and search reuse are separate options. Raw search changes still rebuild by default; stable layouts also need `preserve_on_search` to preserve their parent scope. Fragment-only changes still rebuild the deepest normal frame.

A history entry identifies a visit; retention identifies a page instance. Multiple entries may reuse one Entity. Push/Replace create new entry identities; Back targets an existing entry. Explicit scrolling restores the target entry's saved position.

## Requests, history, and presentation

Commands within one Router run in internal sequence order. Push/Replace fix their requested location at enqueue time. Back freezes its previous successful entry when execution begins.

- Successful Push appends an entry and truncates the forward branch.
- Successful Replace substitutes a new entry identity, or creates the first entry when history is empty.
- Successful Back moves the cursor. Failure keeps the cursor and history unchanged.

If the published `can_back` is false at enqueue time, Back returns `NoBackEntry` immediately and creates no request. If an enqueued Back has no target when it executes, it fails with `BackTargetUnavailable`, skips the view, and leaves UI/history unchanged.

Normal success publishes the route presentation and history together. An error view or fallback can become visible while the original navigation remains failed and history stays unchanged. Equivalent locations without scroll restoration use zero physical writes while preserving Push/Replace/Back history semantics.

Publication and request termination are separate moments. A published request may remain in `PostCommitCleanup`; the active presentation tells you which page is current. Cleanup or host loss after publication preserves committed history and the request's original success/failure classification. Retry uses a new explicit command.

The standalone Core `MemoryHistory` type is a data container: its `back()` moves the cursor immediately. Production navigation uses Core's [history authority](../../crates/bevy_ui_router_core/src/history_authority.rs) protocol for view execution, publication, and failure handling.

## Scroll restoration by visit

Add `RouteScroll::new(key)` to a container with `Node` and `ScrollPosition`. Keys must be stable and unique within one route frame; different frames may reuse them. Unmarked containers keep application behavior. Duplicate keys within a frame are excluded and produce a bounded diagnostic.

When a normal page leaves, Router saves the last valid displayed two-dimensional position in logical pixels, associated with the canonical history entry and route reuse identity. If the same exact frame, Entity, and key remain valid but layout is temporarily unavailable, the latest displayed witness remains. Removing the marker, losing ownership, or replacing the Entity clears that live witness. Repeated URLs have separate visit records; Replace creates a new entry. An error/fallback replacing a normal page saves the original position without overwriting successful history records. Failure that preserves the current page leaves its scrolling alone.

Successful Back freezes restoration intents for saved frame/key pairs in the target entry, including retained hits and preserved prefixes. Containers may already exist or appear later. Push/Replace keep current values for live/reused instances, while fresh Scenes use initial values. A target with no record keeps its current value. Navigation can terminate while content is still becoming ready.

Each publication creates fresh opaque intent IDs, even for repeated Back to the same entry, frame, or Entity. `RouteScrollRestorations::pending(scope_entity, key)` queries the frozen position and ID from any content in the exact active frame, including before the container exists. An intent binds once to one container, recording its original Entity, frame, key, policy, and position. Losing that binding or changing the key/policy consumes the intent rather than transferring it to a replacement Entity.

The default `Automatic` policy attempts restoration on the first normal layout after binding. `with_restore_policy(RouteScrollRestorePolicy::Explicit)` waits for `ready_route_scroll(id)` through `Commands`; ready may arrive before a late container. A new publication cancels old intents. Enqueueing, unpublished preparation, and failure that preserves current content leave the current intent intact. Old or consumed IDs have no effect.

While waiting, the frozen target and original entry record remain, and the actual displayed witness keeps updating. Sampling and leaving preserve the record of a still-valid intent. An application edit to the bound `ScrollPosition` cancels restoration and resumes ordinary displayed-position sampling; immediate departure in the same Update also revalidates this. Application edits after execution take priority over settlement. Use `cancel_route_scroll(id)` to express cancellation when assigning the same value or before binding. Cancellation keeps already written and application-owned values.

`RouterScrollSet::Prepare` writes requested positions in `PostUpdate`, after `UiSystems::Content` and before `UiSystems::Layout`. Reads between publication and Prepare may still see the previous component value. After normal layout, Router revalidates complete active-branch ownership, records the clamped result once, and consumes the intent. Later content growth leaves that result alone. Missing real layout or a valid camera abandons the attempt, preserves the value from binding, and emits a bounded diagnostic. Restoration uses normal layout rather than extra layout passes or fixed-frame retries.

Records follow canonical entries: Replace, forward truncation, and Router removal release the corresponding records. Host loss cancels intents while retaining records for visits still in history. See the [integration guide](../guides/integration.en.md#restore-scroll-positions).

## Snapshots and diagnostics

A request snapshot contains its requested/matched location, status, trace, and failure. Router snapshots separately expose the running request, active presentation, availability, history/can-back, and cleanup/diagnostics.

Queued and last-terminal requests remain distinct from the running request. An error presentation's requested location stays distinct from successful history. A queued Back may not yet have a resolved target.

Read this update's snapshots after `RouterScheduleSet::PublishSnapshots` in `Update`. ECS owns the internal observe/drive/commit/cleanup ordering.

`RouterSnapshots` is marked changed only when observable values change, so Bevy change detection can skip unchanged projections. This signal covers all Routers and requests. Query current content ownership and activity through `RouteInputs` / `RouteActivity`; those facts and scrolling may change independently of snapshots.

Diagnostics identify Router/request, route when matched, failure source, and phase. They distinguish recovery failures from post-publication anomalies and record semantic transitions rather than polling iterations.

Each Router retains its latest 64 terminal requests; queued/running requests are exempt from eviction. A previously issued request in the exact domain/Router that was evicted returns `RequestLookup::NotRetained`. Cross-App/Router, future, or incorrectly paired identities return `Unknown`. `earliest_retained_sequence` exposes the retained range. Eviction leaves authority unchanged.

See [snapshots](../../crates/bevy_ui_router_ecs/src/snapshots.rs) for implementation and the [verification guide](../development/verification.en.md) for checks.
