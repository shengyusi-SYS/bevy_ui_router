//! Prepare application presentation while the original current owners are still
//! installed. Exit disposition and physical cleanup remain navigation duties.

use std::collections::HashSet;

use super::*;
use crate::transition::{
    self, RouteTransitionContext, RouteTransitionId, RouteTransitionTarget, TransitionCancelReason,
    TransitionPreparation, TransitionSessionError, TransitionSurfaceBinding, TransitionTargetRole,
};

/// Called before `prepare_commit_entry` moves current owners into its splice, so
/// application preparation can still read current RouteInputs/RouteActivity.
pub(super) fn prepare_application_transition(
    world: &mut World,
    handle: RouterHandle,
) -> Result<Option<RouteTransitionId>, TransitionSessionError> {
    if !transition::has_transition_driver(world, handle) {
        return Ok(None);
    }
    world.resource_scope(
        |_world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            if let Some(entry) = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
            {
                prepare_back_for_normal_publication(entry);
            }
        },
    );
    let Some((mut context, mut bindings)) = capture_targets(world, handle) else {
        return Ok(None);
    };
    if let Some(current) = transition::current_context(world, handle)
        && current.request() == context.request()
        && current.entering() == context.entering()
        && current.leaving() == context.leaving()
        && current.preserved() == context.preserved()
        && current.from() == context.from()
        && current.to() == context.to()
        && transition::transition_is_prepared(world, &current.id())
    {
        return Ok(Some(current.id()));
    }
    if transition::current_transition(world, handle).is_some() {
        transition::cancel_transition(world, handle, TransitionCancelReason::Superseded)?;
        // Cancellation can run application code. Reobserve the still-owned
        // preparation rather than using facts captured before that callback.
        let Some(recaptured) = capture_targets(world, handle) else {
            return Ok(None);
        };
        (context, bindings) = recaptured;
    }
    // Recovery keeps its existing immediate publication. Old writers have
    // already stopped above, even though this attempt starts no new effect.
    if context.entering().iter().any(|target| {
        !matches!(
            target.frame().origin(),
            bevy_ui_router_core::frame::FrameOrigin::MatchedRoute(_)
        )
    }) {
        return Ok(None);
    }
    let id = context.id();
    match transition::prepare_transition(world, context, bindings)? {
        TransitionPreparation::Skip => Ok(None),
        TransitionPreparation::Run { .. } => Ok(Some(id)),
    }
}

/// Transition-enabled Back uses the same physical publisher as retained Back.
/// The original driver/report/continuation pairing remains authoritative.
pub(super) fn uses_normal_back_publication(world: &World, entry: &RouterEntry) -> bool {
    transition::has_transition_driver(world, entry.handle)
        && matches!(
            entry.pending_back_target_execution,
            Some(PendingBackTargetExecution::Driving(_))
        )
}

fn prepare_back_for_normal_publication(entry: &mut RouterEntry) {
    // A legacy leaf can still own its final apply task here. Advance it through
    // the existing Back report protocol, never the unpaired generic report.
    if matches!(
        entry.pending_back_target_execution,
        Some(PendingBackTargetExecution::Driving(_))
    ) && entry
        .pending_commit
        .as_ref()
        .is_some_and(|pending| pending.ticket.is_none() && pending.final_task.is_some())
    {
        let mut pending = entry.pending_commit.take().unwrap();
        let task = pending.final_task.take().unwrap();
        if let RouteBackTargetAppliedOutcome::NonTarget {
            mut pending,
            task,
            report: _,
        } = route_back_target_applied_report(
            entry,
            pending,
            task,
            BackTargetAppliedReport::LegacyLeaf,
        ) {
            pending.final_task = Some(task);
            entry.pending_commit = Some(pending);
        }
    }

    let Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::AppliedCommitPending(owner),
    )) = entry.pending_back_target_execution.as_ref()
    else {
        return;
    };
    if entry.pending_commit.is_some()
        || !matches!(
            entry.engine.validate_back_target_execution_pairing(
                BackTargetExecutionOwnerRef::Driving {
                    history: &owner.driving.history,
                    continuation: &owner.driving.continuation,
                },
                BackTargetPhysicalOwnerRef::CommitTicket(&owner.ticket),
            ),
            BackTargetExecutionPairingOutcome::Matched
        )
    {
        // Keep the original owner for the existing mismatch/failure driver.
        return;
    }
    let Some(PendingBackTargetExecution::ResumeBlocked(
        PendingBackTargetResumeBlocked::AppliedCommitPending(owner),
    )) = entry.pending_back_target_execution.take()
    else {
        unreachable!("the checked original applied Back owner remains installed")
    };
    let PendingBackTargetAppliedCommit {
        driving,
        ticket,
        mut pending,
    } = owner;
    pending.ticket = Some(ticket);
    entry.pending_commit = Some(pending);
    entry.pending_back_target_execution = Some(PendingBackTargetExecution::Driving(driving));
}

/// A previous entering animation may outlive its request. Only this pending
/// request's unpublished session may be sealed by its publication plan.
pub(super) fn transition_for_pending(
    world: &World,
    handle: RouterHandle,
    pending: &PendingCommit,
) -> Option<RouteTransitionId> {
    let request = pending
        .ticket
        .as_ref()
        .map(r3_core::CommitTicket::request)
        .or_else(|| pending.final_task.as_ref().map(RootFrameTask::request))?;
    let context = transition::current_context(world, handle)?;
    let id = context.id();
    (context.request() == request && transition::transition_is_prepared(world, &id)).then_some(id)
}

/// Run after every dispatcher branch, including retries and early returns.
/// Presented sessions stay alive; every unpublished prepare gets cancellation.
pub(super) fn cancel_prepared_only(
    world: &mut World,
    handle: RouterHandle,
) -> Result<(), TransitionSessionError> {
    let Some(id) = transition::current_transition(world, handle) else {
        return Ok(());
    };
    if !transition::transition_is_prepared(world, &id) {
        return Ok(());
    }
    let result =
        transition::cancel_transition(world, handle, TransitionCancelReason::PreparationAborted);
    if transition::current_transition(world, handle).as_ref() != Some(&id) {
        transition::release_transition_presence(world, &id);
    }
    result
}

/// Process sessions which no longer own a physical exit, even when the router
/// has no cleanup ledger or current request. Held exits use their owner driver.
pub(super) fn settle_application_transitions(world: &mut World) {
    let handles = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .filter(|entry| entry.deferred_exit.is_none())
        .map(|entry| entry.handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let Some(requests) = transition::pending_transition_requests(world, handle) else {
            continue;
        };
        if transition::transition_is_prepared(world, &requests.id) {
            continue;
        }
        let result = if requests.finish {
            transition::finish_transition(world, &requests.id)
        } else if requests.release_exit {
            transition::release_transition_exit(world, &requests.id)
        } else {
            continue;
        };
        if result.is_err() {
            let _ =
                transition::cancel_transition(world, handle, TransitionCancelReason::AuthorityLost);
        }
    }
}

fn capture_targets(
    world: &World,
    handle: RouterHandle,
) -> Option<(RouteTransitionContext, Vec<TransitionSurfaceBinding>)> {
    let entry = world
        .get_resource::<RouterNavigation>()?
        .entries
        .iter()
        .find(|entry| entry.handle == handle)?;
    let pending = entry.pending_commit.as_ref()?;
    if entry.authority_protocol_residual.is_some()
        || entry.retained_miss_cleanup.is_some()
        || entry.projectionless_fail_close.is_some()
        || pending.rejected_publication.is_some()
        || !current_host_binding_is_exact(world, entry.host.as_ref()?)
    {
        return None;
    }
    let (request_id, projection) = match (&pending.ticket, &pending.final_task) {
        (Some(ticket), _) => (
            ticket.request(),
            entry
                .engine
                .reconciliation_projection_for_commit(ticket)
                .ok()?,
        ),
        (None, Some(task)) => (
            task.request(),
            entry.engine.reconciliation_projection_for_task(task).ok()?,
        ),
        _ => return None,
    };
    let request = entry.engine.request(request_id)?;
    let to = request.requested()?.clone();
    let from = entry
        .engine
        .active()
        .location()
        .or_else(|| entry.engine.active().requested())
        .cloned();
    let preserved = projection
        .preserved()
        .map(|frame| frame.version().clone())
        .collect();
    let leaving = projection
        .leaving()
        .map(|projected| {
            entry
                .active
                .as_ref()?
                .frames()
                .iter()
                .find(|frame| frame.frame() == projected.version())
        })
        .collect::<Option<Vec<_>>>()?;
    let selected = projection
        .retained()
        .map(|projection| {
            entry
                .retained
                .payload_for(projection)
                .map(|payload| payload.frame())
        })
        .collect::<Option<Vec<_>>>()?;
    let entering = selected
        .into_iter()
        .chain(pending.branch.frames())
        .collect::<Vec<_>>();
    if entering.is_empty() && leaving.is_empty() {
        return None;
    }
    let (entering, mut bindings) = capture_role(world, entering, TransitionTargetRole::Entering)?;
    let (leaving, leaving_bindings) = capture_role(world, leaving, TransitionTargetRole::Leaving)?;
    bindings.extend(leaving_bindings);
    let context = RouteTransitionContext::new(
        handle,
        request_id,
        request.kind(),
        from,
        to,
        entering,
        leaving,
        preserved,
    );
    Some((context, bindings))
}

fn capture_role(
    world: &World,
    frames: Vec<&RealizedFrame>,
    role: TransitionTargetRole,
) -> Option<(Vec<RouteTransitionTarget>, Vec<TransitionSurfaceBinding>)> {
    let shells = frames
        .iter()
        .map(|frame| frame.shell())
        .collect::<HashSet<_>>();
    let mut targets = Vec::with_capacity(frames.len());
    let mut bindings = Vec::with_capacity(frames.len());
    for frame in frames {
        let shell = frame.shell();
        let content = frame.native_root()?;
        let mut ancestor = world.get::<ChildOf>(shell).map(ChildOf::parent);
        let mut seen = HashSet::new();
        let mut outermost = true;
        while let Some(entity) = ancestor {
            if !seen.insert(entity) {
                return None;
            }
            if shells.contains(&entity) {
                outermost = false;
                break;
            }
            ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        let claim = frame
            .claims()
            .find(|claim| claim.entity() == shell)?
            .clone();
        let target = RouteTransitionTarget::new(frame.frame().clone(), shell, content, outermost);
        bindings.push(TransitionSurfaceBinding::new(target.clone(), role, claim));
        targets.push(target);
    }
    Some((targets, bindings))
}
