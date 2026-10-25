# Lifecycle and cleanup contract

[中文](lifecycle.md) | [English](lifecycle.en.md)

This page covers host admission, execution identity, entity ownership, publication, and recovery. See [navigation](navigation.en.md) for outcomes, [Scenes](scene.en.md) for creation obligations, and [input and focus](input-focus.en.md) for revocation.

## Host admission and registration

The application creates and styles its host. Router manages the authorized edge from the host to its top slot; the application keeps control of the host, its styles, and other children.

The host and every Entity in its full `ChildOf` ancestry must exist, have `Node`, and form an acyclic chain. Admission keeps that exact ancestry. Reparenting the host or an ancestor, or removing an ancestor's `Node`, invalidates the binding. Ordinary style changes preserve its topology identity.

Every registration has a separate incarnation. RouterHandle, exact Entity, registration instance, and host epoch define admission identity. Removing and reinserting on the same Entity in one tick creates a new instance.

Initial admission requires one valid candidate. With a valid host already bound, additional candidates are rejected and the current binding continues. Once the old foreground responsibility settles, registration can establish a new epoch and accept new commands.

## Ownership and publication

Execution identity includes Router, host, slot, frame identity, and generation; delayed completion also matches the task. Registry writes validate exact Entity, owner, seal, and expected lifecycle. Consumed authority IDs and ownership seals are permanently retired.

Router-created entities are registered for ownership before component/relationship writes that may invoke hooks or observers, then revalidated. Drift keeps content inert and stops publication.

A lifecycle capability is a uniquely held execution credential, moved or consumed once. Rejection, retry, and completion errors return the original owner. Snapshots, fingerprints, and projections store comparison evidence.

Current and next are frozen and validated independently. `next=None` performs validation with an empty physical mutation plan. Delayed or one-shot actions freeze the complete source projection and compare it exactly with the current authority projection when consumed.

Pending shells use `Display::None` for isolation while preserving ordinary authored content values. Successful resolve/apply, identity checks, and adoption lead into publication preparation.

Preparation completes validation, allocation, sorting, owner composition and installation, claim rebinds, cleanup reservations, and input/focus work. The same exclusive call then executes the publication tail: infallible in-place updates of prepared, pre-borrowed state. Queries, fallible validation, structural writes, and hook/observer-triggering writes happen before the tail. Core publishes presentation/history last.

This point switches Router state and input eligibility together: only old or new content is Router-input-active. Ordinary systems may observe staging entities. Router rollback covers managed entities; application resource side effects remain application-owned.

Stale or mismatched results settle their own pending obligations while preserving the current request's identity and result. Asynchronous work carries movable data across await, with World mutation in apply; current BSN creation is synchronous.

## Optional transition presence

`RouteTransitionDriver::prepare` runs before the first visible presentation; `presented` means the new route has published. The application owns effects, timing, and redraw. Router owns exact presentation surfaces, completion/cancellation, and instance disposition. See the [transition guide](../guides/transitions.en.md).

A held exit may remain visible but has no activity, current input, or focus. Publication fixes the retain/dispose/retire partition; release freezes the actual dynamic-content cleanup manifest. An outgoing retained instance becomes a Live cache candidate only after its original exit owner transfers it. Entering animation may outlive request completion; an unreleased exit remains the request's responsibility.

Requests keep their original order. A new request first cancels a held exit session and finishes that handoff. With enter-only animation, an actual view change cancels the previous session; equivalent navigation does not restart it. Completion commands match opaque session identity, so stale commands cannot finish a newer session. Cancellation must stop application writers before returning.

## Inactive instances and long-lived owners

Eligible [retained instances](navigation.en.md#retained-instances) transfer individually to independent, non-Clone long-lived owners issued by Core. ECS keeps original content and instance-owned slots, with separate parent/child responsibility. The original request can terminate after these transfers and its other cleanup complete.

A retained shell stays in its original child slot alongside an active sibling, with its own retained role/claim. Once available as Live cache inventory, it is hidden and excluded from layout. Valid protected instances match their complete scope and original owner. Before a hit, inventory, current, and next are revalidated separately. Rejection, cancellation, or cleanup errors keep or return the original owner to the corresponding holder.

Capacity eviction, parent-scope invalidation, and host loss settle through cleanup owners. Retired inventory keeps its original normal cleanup token; live owners bind to the appropriate token during controlled transfer. Every responsibility transfers once. Readmission waits for original settlement. A new incarnation starts with new content, while history follows its existing rules.

Instance-local invalidation and shared authority loss are classified separately. If the shared parent/slot/host remains valid and the invalid instance and dependencies are safely cleaned up, the same request resumes fresh from that point with its original continuation. Valid active content remains. Failure of a fresh child first cleans up this attempt's new content, then separately settles invalid inventory.

If safe deletion cannot be established, deletion stops with explicit ownership retained. When a stopped dependency cleanup still has live targets and host authority is lost, its original token, receipts, owners, and content move to private residual responsibility. The foreground request terminates with `AuthorityLost`, and the Router remains unavailable. Physical cleanup remains recorded as incomplete. Deleted targets and original shells proven empty and inert use their corresponding controlled-settlement paths.

Ordinary fresh-content failure preserves active content and other inventory in still-valid parent scopes. Error boundaries preserve valid scopes above them; fallback or parent-scope replacement settles the corresponding leaving inventory.

## Failure, recovery, and host loss

Build, resolve, apply, dependency readiness, and mount failures retain their source and phase. Public failures provide classification; detailed diagnostics provide context.

| Situation | Presentation | History / request |
| --- | --- | --- |
| Normal success | New route | Commit history; Succeeded after settlement |
| Match failure | Error if fallback succeeds | Keep history; Failed(original) |
| Build/resolve/apply/entering mount failure | Nearest usable boundary error view, otherwise fallback | Keep history; Failed(original) |
| Recovery itself fails | Keep valid previous presentation, or None | Keep history; Failed(RecoveryFailed), retaining original and recovery failures |
| Host loss before publication | None after settlement | Keep old history; Failed(HostLost), Unavailable |
| Host loss after publication | Clear active afterward | Keep published history and original result, add diagnostic, Unavailable |

Boundary search starts at the failed route and walks ancestors. It preserves the valid prefix strictly above the boundary and replaces the boundary/suffix with its error view. Recovery receives a new identity and is attempted once per failure. A failed nested Push without an error binding enters router fallback after pending cleanup.

Committed authority loss isolates and clears invalid active content, then settles according to the actual host state. Existing history remains. Request classification depends on whether loss occurred before or after publication.

Host loss also covers idle Routers and Ready bindings that have no active content yet. The original host grant retires once and creates a separate settlement owner; completed requests remain terminal.

Even without active content, the top slot and authorized mounting edge must settle. Safe settlement allows readmission; a new host waits for new navigation, while old requests and history remain.

Cleanup freezes its target/obligation manifest and post-completion continuation independently. The continuation comes from the original failure/authority, attempt, and branch classification. Physical deletion counts describe cleanup progress.

## Cleanup and content boundaries

Router manages its structures and authorized frame content boundaries. Ordinary content placed inside that boundary is managed with the page; borrowed application entities belong outside it. Ordinary content moved out returns to application ownership. Original owners remain responsible for Router shells, roots, and slots.

Creation registers exact allocation obligations, including unattached output. Successful adoption switches to current-containment management. A cleanup ledger and unique owner manage two obligation types:

- `Registered`: structures or output not yet adopted, using their own exact claim.
- `ContainedContent`: ordinary content, with frozen exact Entity, source root/shell, and relationships, authorized by the original frame owner.

Ordinary content removal uses this relationship evidence while preserving the existing registry records, markers, and seals. Root claims and ordinary child registration identities are checked separately.

Cleanup first revokes target input/focus, then removes child frames before parents and content leaves before roots. Each operation defines its target, protected closures, and allowed write set. Application-owned, foreign, preserved, and other-authority non-target content stays protected. Targets must be disjoint from every protected closure.

Entities attached after freezing do not expand the original deletion authority. Missing content, relationship changes, historical creation obligations, and actual residuals are checked separately. An operation may freeze roles, deletion order, and receipts; each real write is followed by revalidation of its source, remaining relationships, and full target/protected partition.

Valid unrelated component or style edits remain acceptable. Relevant identity, relationships, isolation, input, and write sets must match. Validation primitives are shared only when their purpose, accepted roles, and failure semantics agree. Read-only preservation and zero-target checks allow valid protected extras.

When safe deletion cannot be proven, destructive work stops and records its source. Proven isolated residuals use private inert ownership; `Unisolated` keeps its explicit classification. Permanent contradictions terminate within bounded work. Capacity or satisfiable-prerequisite shortages can retry while keeping the original owner.

Before terminal state, the foreground scope consumes all its actions, tickets, and capabilities. Private residuals have separate responsibility and follow their original settlement paths. Unfinished apply/settlement retains dependencies. Terminal inert residuals release the original attempt's dependency retention; strong Handles on entities continue to retain assets under Bevy's rules.

Sources: [cleanup](../../crates/bevy_ui_router_ecs/src/cleanup.rs) and [registry](../../crates/bevy_ui_router_ecs/src/registry.rs).
