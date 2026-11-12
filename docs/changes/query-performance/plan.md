# Public query performance plan

Status: active. Scope: share public query proofs within one immutable reader borrow, preserving the existing [activity/input](../../contracts/input-focus.md#page-activity-queries) and [scroll restoration](../../contracts/navigation.md#scroll-restoration-by-visit) contracts. This is the delivery plan; the existing contracts remain the behavior specification.

## Problem and evidence boundary

`RouteActivity`, `RouteInputs`, and pending-scroll lookup resolve active ownership through the same hierarchy and frame/mount facts. Repeated target queries currently repeat that proof. Internal World observation already shares some facts through `ReadObservation`, but public SystemParam queries use a separate resolver path. More application queries can therefore increase work without any navigation or topology change.

The optimization target is repeated proof within one reader, not a new routing-state source. Wide/deep content, history size, Router count, and queued requests are separate cost dimensions. Existing single-navigation and averaged-update measurements do not establish their combined cost; they do not justify bundling history, queue, or lifecycle redesign into this change.

## Minimal design

- Add the read-only `RouteQueries` SystemParam. `read()` creates a borrowed `RouteQueryReader` with `is_active`, `current`, and `pending` queries.
- Share exact frame/mount proofs and repeated hierarchy observations inside that reader. All three queries use one resolver. Keep `RouteActivity`, `RouteInputs`, and `RouteScrollRestorations` as thin entry points using the same resolver; do not add independent persistent sessions to them.
- Facts live only for the immutable reader borrow. Releasing it permits World mutation; a subsequent reader rebuilds its evidence even within the same tick. Do not use change ticks to authorize reuse or weaken nearest-boundary, registration-incarnation, exact claim, mount-chain, inactive-role, or quarantine checks.
- Preserve the ability to use Router queries alongside mutable application component queries and Commands navigation. The reader grants no input, mutation, publication, or cleanup authority.

## Delivery and verification

1. Correct current Bevy compatibility documentation and record a reproducible public-query baseline using the existing benchmark harness. Keep one-call cost separate from repeated calls through one reader.
2. Implement the shared resolver and borrowed API, expose it through the ECS facade/prelude, and retain existing entry points. Exercise repeated queries, current inputs, and pending-scroll lookup through public production paths.
3. Verify inactive/invalid nearest-boundary rejection, nested mounting, retained siblings, and non-interactive content. Drop the reader, change hierarchy/claims or remove and reinsert a host registration in the same tick, then require a new reader to observe the change. Verify the public borrow/API boundary and standalone consumer usage.
4. Compare equivalent flat and nested workloads with fixed content and varying query counts. Include old entry points, the shared reader, and an application-only control; report per-call and whole-update costs separately. Preserve source, configuration, and complete paired results according to [performance measurement](../../performance.md).
5. Run formatting/static checks, full affected ECS tests, the relevant facade activity/input/scroll journeys, and independent facade consumers. Run documentation link checks. The shared-target Cargo owner runs all Rust validation sequentially. Update English/Chinese query guides and runtime diagrams after the API is final.

## Completion and stopping conditions

Finish when the public reader demonstrates reduced repeated-proof work, all query results and exact-identity behavior remain equivalent, affected validation passes, and documentation describes the shipped API and measured limits. Record durable measurement evidence in the existing performance documentation/data locations, then retire this completed plan from the default tree through Git history.

Stop and reassess the implementation if sharing requires evidence to survive its reader borrow, weakens ownership/freshness checks, breaks application Query/Commands composition, or repeated measurements fail to show a benefit outside normal variation. Do not substitute a persistent cache or alter queue/history behavior to force a favorable result. New bottlenecks require their own measured scope before further work.
