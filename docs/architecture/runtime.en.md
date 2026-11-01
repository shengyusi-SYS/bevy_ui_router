# Runtime behavior

[中文](runtime.md) | [English](runtime.en.md)

This page follows a navigation through entity structure, execution, and Bevy scheduling. See the [architecture](README.en.md) for modules and the [lifecycle execution protocol](lifecycle.en.md) for internal ownership handoffs.

## Logical model and entity structure

Core produces an ordered matched branch. `BranchDiff` preserves its reusable common prefix, leaves the old suffix from leaf to root, then prepares the new suffix from root to leaf. ECS binds each logical frame to a host, slot, and content boundary.

```mermaid
flowchart TD
    Host["application host Node"] --> Top["Router top slot"]
    Top --> Shell["Router FrameShell"]
    Shell --> Root["Scene root: FrameContentTarget"]
    Root --> Layout["sidebar, toolbar, and other content"]
    Root --> Outlet["inner Node + DefaultOutlet"]
    Outlet --> Slot["Router child slot"]
    Slot --> ChildShell["active child FrameShell"]
    Slot --> Retained["retained shell: hidden, separate owner"]
    Retained --> CachedRoot["original content and local state"]
    CachedRoot --> CachedOutlet["instance-owned outlet / child slot"]
    CachedOutlet --> CachedChild["retained child tied to its original parent scope"]
```

The application creates and styles the host and adds `RouterHostRegistration`. Router manages the top slot, frame shells, child slots, and authorized content. The application keeps control of the host, its style, and its other children.

`DefaultOutlet` is an ordinary Scene declaration on the content root or an inner container. ECS validates it and creates the actual mount binding. Retained and active instances may share the original child slot, each with its own role, claim, and owner. A parent frame's content traversal stops at child slots.

Structural identity includes Router, host, slot, frame, and generation; execution results also match the task. A host registration incarnation distinguishes each registration on the same Entity, including removal and reinsertion in one tick.

Router structures and Scene creation allocations are registered for ownership before component or relationship writes, then validated again. After successful adoption, ordinary Scene descendants are managed by current `ChildOf/Children` containment. Applications can update components and dynamic children; entities added inside this content boundary are managed with the page, and ordinary content moved out returns to application ownership. Router structures remain their original owner's responsibility.

## Normal navigation

Immediate navigation returns a request ID. Commands navigation enqueues only when Bevy applies deferred commands, without a synchronous ID. Later `Update` passes advance requests in order, possibly across several updates. A configured initial location enters the same queue after the first Ready state; successful explicit navigation consumes the unsubmitted default.

```mermaid
sequenceDiagram
    participant App as Application
    participant Commands as Bevy Commands
    participant Nav as ECS orchestration
    participant Core as Core RouterEngine
    participant Scene as BSN factory / Scene
    participant World as ECS World
    alt Commands entry
        App->>Commands: router(handle).push / replace / back
        Commands->>Nav: ApplyDeferred: actually enqueue
    else Immediate entry
        App->>Nav: push / replace / back
    end
    Nav->>Core: enqueue
    Core-->>Nav: request ID
    opt Immediate call
        Nav-->>App: request ID
    end
    Nav->>Core: get action, freeze instance selection
    loop Target chain, parent to child
        alt Current / Retained
            Nav->>World: prepare using original entities and slots
        else Fresh
            Nav->>Scene: build and resolve fresh Scene
            Nav->>World: register, hidden staging, apply, revalidate
            Nav->>Core: return execution and mounting results
        end
    end
    opt Application configures a transition driver
        Nav->>App: prepare hidden entering and current leaving views
    end
    Nav->>World: prepare current/next, input, focus, and subsequent owners
    Nav->>World: apply prepared publication tail
    Nav->>Core: publish presentation / history
    opt Application transition
        Nav->>App: presented (new route is effective)
        opt Application holds leaving views
            App->>Nav: release / finish, or a new request cancels
            Nav->>World: hide old views, freeze actual cleanup scope
        end
    end
    Nav->>World: clean up leaving content / transfer retained instances
    Nav-->>App: update RouterSnapshots
```

Fresh content uses official `ScenePatch::load/resolve` and the Scene executor. A cache hit reuses the original frame, content handles, and entities, skipping factory, resolve, and apply. The new request still has its own execution identity.

Mixed branches prepare as `Current* → Retained* → Fresh*`. Core decides selection, capacity, and eviction; ECS holds each instance's original content and owned slots. A fresh child uses the parent mount selected for this request. New slots belong to preparation until publication, then transfer to the parent; failure cleans them up. Parent dependency eviction runs child-first.

Publication preparation combines the physical switch, claim rebinds, input/focus changes, and subsequent owners. The tail applies prepared state, and Core publishes presentation/history last. See the [publication contract](../contracts/lifecycle.en.md#ownership-and-publication). Cleanup of old content may follow: the request can remain in `PostCommitCleanup` while the new page and history are already effective.

Optional transitions separate the application presentation session from physical exit ownership. The application prepares initial styles before publication and starts its own animation afterward. Enter-only animation does not delay request completion. An exit hold keeps the old page visible but inactive; release then freezes actual containment for cleanup. See the [transition guide](../guides/transitions.en.md).

ECS [scroll.rs](../../crates/bevy_ui_router_ecs/src/scroll.rs) manages scroll records. Core provides entry and target frame/reuse facts; preparation freezes restoration intents, and the tail installs them. Existing and late containers receive their requested position before normal layout and settle once afterward. While waiting, restoration targets and displayed positions remain separate; leaving revalidates bindings and application edits. See [scroll restoration by visit](../contracts/navigation.en.md#scroll-restoration-by-visit).

Failure first settles its physical obligations, then follows the error boundary, fallback, or authority-settlement path. Cleanup validates target and protected content separately and checks again after writes. Content that cannot be safely deleted moves to an explicit residual owner. Router shells and the input gate provide isolation while preserving Scene-authored `Node`, `Visibility`, and `Pickable` values.

## Bevy scheduling and input

```mermaid
flowchart TB
    subgraph PreUpdate
        Refresh["RouterInputSet::Refresh"] --> Candidates["visible exit: project UiStack candidates"]
        Candidates --> Backend["official picking Backend"]
        Backend --> Restore["restore original UiStack"]
        Restore --> HoverMap["picking hovermap"]
        HoverMap --> Filter["filter inactive pointer state"]
        Filter --> Interaction["picking interactions"]
        Interaction --> Ready["RouterInputSet::Ready"]
    end
    subgraph Update
        Observe["ObserveAuthority"] --> Collect["CollectNavigation"]
        Collect --> Drive["DriveNavigation"]
        Drive --> Commit["CommitPresentation"]
        Commit --> Cleanup["SettleCleanup"]
        Cleanup --> Publish["RouterScheduleSet::PublishSnapshots"]
    end
    subgraph PostUpdate
        Tween["optional RouteTween: advance, interpolate, settle"] --> UiPrepare["UiSystems::Prepare"]
        UiPrepare --> Content
        Content["UiSystems::Content"] --> Prepare["RouterScrollSet::Prepare"]
        Prepare --> Layout["UiSystems::Layout"]
        Layout --> Scroll["revalidate, settle, and sample scroll"]
    end
    Ready --> Observe
    Publish --> Focus["observe BSN focus queue"]
    Focus --> Content
```

Applications use these public ordering points:

- Submit dynamic topology changes before `RouterInputSet::Refresh`; read `RouterInputGate` and handle target-addressed keyboard/gamepad input after `Ready`.
- Read this update's snapshots, `RouteInputs`, and `RouteActivity` after `RouterScheduleSet::PublishSnapshots`. Layout and scroll settlement still follow in `PostUpdate`.
- `RouterScrollSet::Prepare` runs after `UiSystems::Content` and before `UiSystems::Layout`, writing restoration requests for that update.

The other lifecycle sets in the diagram are crate-private and orchestrated by ECS. Plugins and App extensions share idempotent installation.

Optional `RouteTweenPlugin` advances `bevy_tween` controllers before UI Prepare using a separate time context, starting at presented. After official interpolation applies the final pose, the adapter removes the corresponding controllers and requests release or completion through the original transition session. Cancellation removes writers synchronously. Applications may also use their own Tween plugins. Navigation submitted after PublishSnapshots can publish in a later update; a Commands call is not a synchronous page switch.

Input eligibility refreshes before the picking Backend. While an exit is visible, its native UI candidates are temporarily excluded and the complete stack is restored after Backend, allowing the new page to receive hits. Inactive targets are then filtered between hover generation and interactions. Publication revokes leaving targets' historical hover, press, click, drag, and focus in the same call. Applications integrate target filtering for native TabNavigation/InputDispatch; see the [integration guide](../guides/integration.en.md).

Read-only phases can reuse frame content, membership, and child-slot boundaries within one World borrow. Writes and new scheduling phases require a new observation. Focus also validates shells, committed boundaries, and the closure outside slots; retained instances validate all descendants and their separation from input/focus.

Empty work phases return immediately, while idle hosts are still observed each update. Scroll Prepare returns when no intents exist; Settle keeps sampling actual layout. All `RouteScroll` markers participate in duplicate-key detection before layout eligibility is checked. Snapshots are marked changed only when their values change; the input gate is written when permissions change or revocation is needed. The BSN focus bridge checks the official drain on first use or after changes to the `PostUpdate` schedule graph, and keeps observing the focus queue during stable scheduling.
