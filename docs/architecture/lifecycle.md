# Lifecycle execution protocol

[中文](lifecycle.zh-CN.md) | [English](lifecycle.md)

This page helps contributors follow an action from Core through ECS to completion or failure. User-visible behavior belongs to the [lifecycle contract](../contracts/lifecycle.md); entity structure and scheduling are covered by [runtime behavior](runtime.md). A holder is an internal container that keeps the original owner and execution progress.

Structural manifests record component presence relevant to routing structure. Bevy-derived `CalculatedClip`, `Inherited<ComputedUiTargetCamera>`, and `Inherited<ComputedUiRenderTargetInfo>` may change during normal UI updates and are excluded from structural equality; preregistration and observation use the same projection. Staging and detachment separately freeze and revalidate the UI inheritance facts they depend on. Registry claims, owners, seals, lifecycles, and relationships remain exact.

## Actions and physical ownership handoffs

Core [engine](../../crates/bevy_ui_router_core/src/engine.rs) chooses the next action, and [integration](../../crates/bevy_ui_router_core/src/integration.rs) provides cross-crate handoffs. ECS [navigation](../../crates/bevy_ui_router_ecs/src/navigation.rs) keeps owners and physical progress across updates in `RouterEntry`. Core's logical projection and ECS entity structure are validated independently.

| Core handoff | ECS preparation and holder | Subsequent responsibility |
| --- | --- | --- |
| `FrameTask` | `PendingCommit` accumulates the prepared branch; `pending_frame_drive` holds unfinished task, effect, and pending content | Return apply/adoption results to Core; failed content moves to the failed branch with its original task/owner |
| `MountRequest` | Validate parent/outlet, create a provisional slot, keep receipt and confirmation | Confirmation adds it to the prepared branch; rejection settles the slot's original owner through the failed branch |
| `CommitTicket` or final `final_task` | Seal reconciliation of current/next; prepare switching, claim rebinds, input/focus, and cleanup owners | Consume publication work; Core publishes; cleanup receives leaving responsibility |
| `FailureCleanup` | Freeze the failed-branch manifest, reserve the ledger, bind the Core cleanup batch | Physical receipts form typed completion; Core consumes the original continuation to choose recovery or termination |
| `BackResolution` | Pair history authority with capacity reservation, preserve full source evidence, prepare target and no-target paths separately | A target enters execution/publication; no target follows its failure or authority-settlement path |
| `RetainedMissCleanupOwner` | Hold the original continuation, separate cleanup token, dependency-forest ledger, and invalidation evidence | Return the original continuation after completion; stopped and authority-terminal paths consume their corresponding original responsibility |

Rejection, capacity shortages, and completion errors return the original owner. Active content, pending content, cleanup ledgers, and residuals have separate responsibilities; each protocol and its receipts determine completion.

When cleanup of an invalid retained instance stops, completed deletions keep their receipts. If remaining original shells are proven empty and isolated, the controlled stop path transfers them to private residual ownership, completes the original token, and returns the fresh task to authority observation and settlement.

If a stopped dependency cleanup still has live targets and its host binding is lost, a dedicated authority terminal consumes the installed owner and lease, transferring the token, receipts, stop evidence, payload, and retained owners to the authority residual. This ends the foreground continuation while preserving physical incompletion. Rejection returns the full owner/lease; the residual continues to block readmission.

History issuance and pairing live in [history_authority.rs](../../crates/bevy_ui_router_core/src/history_authority.rs). After publication, Core [post_commit.rs](../../crates/bevy_ui_router_core/src/engine/post_commit.rs) determines logical completion, ECS [navigation/post_commit.rs](../../crates/bevy_ui_router_ecs/src/navigation/post_commit.rs) advances the original owner, and [cleanup/post_commit.rs](../../crates/bevy_ui_router_ecs/src/cleanup/post_commit.rs) settles physical evidence. Already retired instances keep their normal token; live retained inventory enters a second authority batch. Joint completion consumes the two tokens separately. The original receiver freezes the full inventory. Removing delayed preservation proofs removes only originally selected items and preserves valid changes belonging to other Routers.

Idle host loss uses a separate settlement owner, managed by Core [idle_host_loss.rs](../../crates/bevy_ui_router_core/src/engine/idle_host_loss.rs) and ECS [idle_host_loss.rs](../../crates/bevy_ui_router_ecs/src/navigation/idle_host_loss.rs). Completed requests remain terminal.

## Deferred exit and application presentation

[Core exit](../../crates/bevy_ui_router_core/src/engine/exit.rs) separates stable cleanup batch identity from the actual manifest seal. Ordinary navigation seals immediately. A visible exit transfers its original `DeferredExitOwner` at publication and creates the manifest from actual containment only on release. History commits at the original publication, never again at release.

ECS [transition](../../crates/bevy_ui_router_ecs/src/transition.rs) holds application sessions and authorized presentation values. [deferred_exit](../../crates/bevy_ui_router_ecs/src/navigation/deferred_exit.rs) holds the original logical owner, entity partitions, and frozen hiding targets. A session may cancel first; its exact presence projection remains until the old content is physically hidden. Visibility never grants lifecycle/input activity.

Preparation calls application code while original active owners remain queryable and freezes current/next claims independently. Immediate, retained, and deferred publication share the [publication](../../crates/bevy_ui_router_ecs/src/navigation/publication.rs) receiver; Back keeps its original continuation. Release joins the real seal to the existing ledger rather than introducing a parallel animation cleanup state machine.

## Preparation outcomes and failure routing

`reconciliation_preflight` checks host/current and pending entering content. [lifecycle_preparation.rs](../../crates/bevy_ui_router_ecs/src/lifecycle_preparation.rs) keeps the source, expectation, and observed state. Each preparation path has its own concrete types; this table compares their meanings.

| Outcome | Facts and next step |
| --- | --- |
| `Ready` | Receiver and prepared facts agree; continue publication preparation |
| `PendingRejected` | Entering/rebase rejection; projectionless fallback also covers admission and target overlap. Keep the original pending owner, clean up this attempt, and recover from the original failure source |
| `AuthorityLost` | Host, preserved current, or leaving current authority is lost; isolate affected content and enter authority settlement |
| Retryable preparation shortage | Capacity or a satisfiable prerequisite is missing; keep the original owner and unfinished obligations, then retry that preparation |
| `Permanent` | A permanent contradiction; stop progress and settle responsibility |
| `PairingMismatch` / `EntryMismatch` | Return the complete owner and evidence to the matching mismatch/inert holder, then settle through that protocol |

When current authority loss and pending rejection occur together, authority loss takes priority. Contradictions preparing a Back target's registry mutation plan or leaving manifest are `Permanent`.

Cleanup freezes physical obligations and the continuation separately. The continuation comes from the original failure/authority, attempt, and branch classification and is checked again at completion. Deletion counts, remaining targets, and old active state describe physical progress only.

## Module map

| Module | Responsibility |
| --- | --- |
| [branch_transaction.rs](../../crates/bevy_ui_router_ecs/src/branch_transaction.rs) | Core projection handoff, sparse slot roles, sealed splice, prepared/failed branches, claim rebinds |
| [normal_publication.rs](../../crates/bevy_ui_router_ecs/src/navigation/normal_publication.rs) / [projectionless_publication.rs](../../crates/bevy_ui_router_ecs/src/navigation/projectionless_publication.rs) | Reconciliation/cache-hit and fallback preparation paths; share retained commit, leaving forest, and publication holders while keeping their own failure classifications |
| [retained.rs](../../crates/bevy_ui_router_ecs/src/retained.rs) / [instance.rs](../../crates/bevy_ui_router_ecs/src/instance.rs) | Selection and transfer of original long-lived owners; single-instance payloads and preallocated receivers. Core [retained.rs](../../crates/bevy_ui_router_core/src/engine/retained.rs) decides retention/eviction |
| [frame_local.rs](../../crates/bevy_ui_router_ecs/src/frame_local.rs) | Validate local closure using a frame's own role/binding; stop at child slots |
| [bsn.rs](../../crates/bevy_ui_router_ecs/src/bsn.rs) / [native_scene.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_scene.rs) | Declaration preparation, preallocation/registration, official executor apply, adoption, immediate failure isolation |
| [native_frame.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_frame.rs) | Post-adoption containment, native cleanup preparation, dependency retention |
| [native_failure.rs](../../crates/bevy_ui_router_ecs/src/presentation/native_failure.rs) | Scene creation failure sources, dependencies, and residual ownership |
| [authority_isolation.rs](../../crates/bevy_ui_router_ecs/src/navigation/authority_isolation.rs) | Isolation write sets and settlement for current authority loss |
| [cleanup.rs](../../crates/bevy_ui_router_ecs/src/cleanup.rs) | Removal plans, execution, receipt/ledger completion; [failure_partition.rs](../../crates/bevy_ui_router_ecs/src/cleanup/failure_partition.rs) manages target/protected partitions and per-write checks, and [capacity.rs](../../crates/bevy_ui_router_ecs/src/cleanup/capacity.rs) manages reservations |

Before changing a path, locate its action receiver, current holder, and failure return value. Validation includes [ECS native regressions](../../crates/bevy_ui_router_ecs/src/navigation/native_tests.rs), module tests, [public navigation journeys](../../crates/bevy_ui_router/tests/public_navigation_journeys.rs), and the [standalone BSN consumer](../../crates/bevy_ui_router/tests/consumers/bsn.rs). Choose the combination using the [verification guide](../development/verification.md).
