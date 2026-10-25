# Transition lifecycle implementation plan

[中文](plan.md) | [English](plan.en.md)

Status: **In progress.** The [design](design.en.md) defines scope and responsibilities. Completion requires public integration seams, a runnable application-defined animation example, and the necessary combined-behavior verification. Document or internal-type counts do not measure progress.

## Order and completion criteria

| Stage | Deliverables and checks | Risk / return |
| --- | --- | --- |
| 1. Key seams | A small official UI/picking probe demonstrates that a visible old page does not block hits on the new page. Separate stable obligation identity from the deletion seal while preserving immediate-path behavior. | Verify input feasibility first; lifecycle pairing is high risk and necessary groundwork. |
| 2. Exit obligations | Request-scoped deferred owner, release/seal, parent/child retain/dispose, and host loss before sealing. Do not freeze a deletion manifest for content that can still change. | Highest risk; required for complete exit animations, with independent cross-review. |
| 3. Application integration | Preparation before publication, published notification, controlled presentation surfaces, completion/cancellation, successive navigation, and stale notifications. Default pass-through without a driver. | Medium to high risk; makes the feature usable by real applications beyond internal passing tests. |
| 4. Example and completion | At least two application-defined effects use the same interface, covering fresh/retained instances, Back, and rapid successive navigation. Update contracts, architecture, and guides. | High return; confirms interface generality and real integration. |

The initial estimate is about 6–10 developer-days. The main uncertainty is late sealing and authority settlement before sealing. Revise the remaining estimate using stage 1 evidence. Upstream patches, modern Android gestures, shared elements, and a group-opacity renderer are outside this task.

## Required verification

- Disabled-by-default and equivalent navigation preserve existing behavior. Coexisting old/new visuals have one logical active/history publication point.
- Ordinary `App::update` proves that initial presentation preparation before publication, the published notification, and application completion/cancellation are connected. Enter animations are independent of request termination.
- Ordinary content can be added or removed dynamically during exit, then cleaned up correctly by the original owner. Parent retention with child cleanup, full retention, and Back reactivation remain correct.
- Real official picking rejects events for the visible old page while positively hitting the new page and other valid targets. Old gestures/focus are revoked, and authored properties remain intact.
- Successive requests advance in their original order. Old animations stop writing first, stale completion cannot settle a new session, and host loss preserves the published result while settling original obligations.
- Navigating again while an enter-only animation is still running, or failing publication revalidation after initial presentation preparation, synchronously cancels the application session. No old writes remain before target reuse.
- Reuse per-visit scroll journeys to verify that animation integration preserves entries, ready/cancel, and actual restored positions.
- Run independent consumers, the windowed example, relevant crates, and complete public journeys. Assess the effects in an actual window; report message/geometry checks separately from rendered appearance.

Use the commands in the [verification guide](../../development/verification.en.md). In the shared workspace, only the root agent runs Cargo, manages the index, and commits. Sub-agents collaborate according to actual file ownership for Core, native integration, and the application example.

## Commits and documentation

Create reviewable batches in this order: stable obligations and behavior-preserving refactoring → deferred exit protocol → application integration and input → example and contracts. Complete each batch's checks before proceeding, and describe new APIs as delivered once their integration is working.

Update current contracts after the behavior lands. Keep only new decisions in the design instead of repeating existing ownership protocols. After delivery, move the required semantics into the relevant contracts and guides. Remove the completed plan from the default documentation tree and retain it in Git history.
