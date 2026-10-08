use super::*;

enum ProjectionlessFallbackPreflight {
    Ready {
        admission: ProjectionlessFallbackAdmission,
        focus_leaving: Vec<bevy_ecs::entity::Entity>,
    },
    PendingRejected(LifecyclePreflightEvidence),
    AuthorityLost(LifecyclePreflightEvidence),
}

enum ProjectionlessCommitFailure {
    PostReportAuthorityLost {
        admission: ProjectionlessFallbackAdmission,
        reason: BeginAuthoritySettlementErrorKind,
    },
    InitialPostReportAuthorityLost {
        reason: BeginAuthoritySettlementErrorKind,
    },
}

impl ProjectionlessCommitFailure {
    fn post_report(
        admission: Option<ProjectionlessFallbackAdmission>,
        reason: BeginAuthoritySettlementErrorKind,
    ) -> Self {
        match admission {
            Some(admission) => Self::PostReportAuthorityLost { admission, reason },
            None => Self::InitialPostReportAuthorityLost { reason },
        }
    }
}

pub(super) fn branch_owner_projection_is_exact(world: &World, branch: &RealizedBranch) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let claims_are_exact = branch.claims().all(|claim| {
        registry.record(claim.entity()).is_some_and(|record| {
            record.claim() == *claim
                && crate::registry::ComponentManifest::from_world(world, claim.entity())
                    .is_some_and(|actual| {
                        record
                            .manifest()
                            .component_ids()
                            .iter()
                            .all(|component| actual.component_ids().contains(component))
                    })
        }) && cleanup_claim_has_exact_authority(world, claim)
    });
    let frame_relationships_are_exact = branch.frames().iter().all(|frame| {
        frame.cleanup_members().all(|(claim, parent)| {
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(claim.entity())
                .map(bevy_ecs::hierarchy::ChildOf::parent)
                == Some(parent)
        })
    });
    let slot_relationships_are_exact = branch.child_slots().iter().all(|slot| {
        world
            .get::<bevy_ecs::hierarchy::ChildOf>(slot.slot())
            .map(bevy_ecs::hierarchy::ChildOf::parent)
            == Some(slot.anchor())
    });
    claims_are_exact && frame_relationships_are_exact && slot_relationships_are_exact
}

fn projectionless_fallback_preflight(
    world: &World,
    entry: &RouterEntry,
    pending: &PendingCommit,
) -> ProjectionlessFallbackPreflight {
    let missing_resources = RequiredResourceSet::missing_from(world);
    let active = entry.active.as_ref();
    let task = pending.final_task.as_ref();
    let host = entry.host.as_ref();
    let current_loss = if !missing_resources.is_empty() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(missing_resources),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        ))
    } else if active.is_none() || host.is_none() {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Missing,
        ))
    } else if active.is_some_and(|active| active.frames().is_empty())
        || host.is_some_and(|host| !current_host_binding_is_exact(world, host))
    {
        Some(LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::HostBinding,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Drifted,
        ))
    } else {
        None
    };
    if !missing_resources.is_empty() {
        return ProjectionlessFallbackPreflight::AuthorityLost(
            current_loss.expect("missing Resource must retain typed authority evidence"),
        );
    }
    let (Some(active), Some(task), Some(host)) = (active, task, host) else {
        return ProjectionlessFallbackPreflight::AuthorityLost(current_loss.unwrap_or(
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::PendingAdmission,
                LifecyclePreflightExpectation::ExactTaskShape,
                LifecyclePreflightObserved::Missing,
            ),
        ));
    };
    let mount_target = task.mount_target();
    let admission_rejection = (pending.ticket.is_some()
        || pending.branch.frames().len() != 1
        || !pending.branch.child_slots().is_empty()
        || task.frame().origin() != &bevy_ui_router_core::prelude::FrameOrigin::RouterFallback
        || pending.branch.frames()[0].frame() != task.frame()
        || !matches!(
            &mount_target,
            FrameMountTarget::TopSlot { host: task_host, slot }
                if task_host == &host.version && slot == &host.slot_id
        ))
    .then(|| {
        LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::PendingAdmission,
            LifecyclePreflightExpectation::ExactTaskShape,
            LifecyclePreflightObserved::Drifted,
        )
    });
    let entering = pending.branch.as_realized();
    let entering_boundaries = entering
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let entering_is_valid = validate_prepared_branch(world, entering).is_ok();
    let entering_closure = validate_branch_descendants(world, entering);
    let active_local = validate_active_branch_local(world, active, &entering_boundaries);
    let active_closure = (active_local.is_ok() && entering_is_valid)
        .then(|| validate_active_branch_with_entering(world, active, Some(entering), None));
    let current_loss = current_loss
        .or_else(|| {
            (!branch_owner_projection_is_exact(world, active)).then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PreservedCurrent,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        })
        .or_else(|| {
            active_local.as_ref().err().map(|_| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PreservedCurrent,
                    LifecyclePreflightExpectation::ExactRelationshipClosure,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        });
    let cross_role_is_valid = active_closure
        .as_ref()
        .is_some_and(|closure| closure.is_ok());
    let pending_rejection = admission_rejection
        .or_else(|| {
            (!entering_is_valid).then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PendingEntering,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        })
        .or_else(|| {
            entering_closure.as_ref().err().map(|_| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::PendingEntering,
                    LifecyclePreflightExpectation::ExactRelationshipClosure,
                    LifecyclePreflightObserved::Drifted,
                )
            })
        })
        .or_else(|| {
            entering.prepare_root_rebase(active).is_none().then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::CrossRoleRebase,
                    LifecyclePreflightExpectation::RebaseCompatible,
                    LifecyclePreflightObserved::Contradictory,
                )
            })
        })
        .or_else(|| {
            (!cross_role_is_valid).then(|| {
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::CrossRoleRebase,
                    LifecyclePreflightExpectation::RebaseCompatible,
                    LifecyclePreflightObserved::Contradictory,
                )
            })
        })
        .or_else(|| {
            active_closure
                .as_ref()
                .and_then(|closure| closure.as_ref().ok())
                .zip(entering_closure.as_ref().ok())
                .is_some_and(|(active, entering)| {
                    active.iter().any(|entity| entering.contains(entity))
                })
                .then(|| {
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::TargetPartition,
                        LifecyclePreflightExpectation::DisjointClosure,
                        LifecyclePreflightObserved::Contradictory,
                    )
                })
        });
    if let Some(evidence) = current_loss {
        return ProjectionlessFallbackPreflight::AuthorityLost(evidence);
    }
    if let Some(evidence) = pending_rejection {
        return ProjectionlessFallbackPreflight::PendingRejected(evidence);
    }
    ProjectionlessFallbackPreflight::Ready {
        admission: ProjectionlessFallbackAdmission {
            frame: task.frame().clone(),
            mount_target,
            top_slot: host.top_slot,
            top_slot_claim: host.top_slot_claim.clone(),
        },
        focus_leaving: active_closure
            .expect("both local sides must produce one coupled closure")
            .expect("cross-role closure was independently validated"),
    }
}

fn reject_projectionless_fallback_pending(
    entry: &mut RouterEntry,
    mut pending: PendingCommit,
    message: impl Into<String>,
) {
    let message = message.into();
    let task = pending
        .final_task
        .take()
        .expect("projectionless fallback rejection retains its exact task");
    entry.pending_failed_branch = Some(PendingFailedBranch {
        terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
            task,
            failure: ViewFailureKind::Apply,
            message,
        },
        branch: failed_prepared_branch(pending, None),
    });
}

fn fail_closed_projectionless_replacement_contradiction_with_evidence(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
    evidence: LifecyclePreflightEvidence,
) -> Result<(), PendingCommit> {
    let active = entry
        .active
        .take()
        .expect("projectionless fallback contradiction retains existing active");
    match fail_closed_normal_owner_protocol(
        world,
        entry,
        active,
        pending,
        AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence),
    ) {
        Ok(()) => Ok(()),
        Err((active, pending)) => {
            entry.active = Some(active);
            Err(pending)
        }
    }
}

fn fail_closed_projectionless_replacement_contradiction(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
) -> Result<(), PendingCommit> {
    fail_closed_projectionless_replacement_contradiction_with_evidence(
        world,
        entry,
        pending,
        LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::LeavingCurrent,
            LifecyclePreflightExpectation::ExactAuthority,
            LifecyclePreflightObserved::Contradictory,
        ),
    )
}

pub(super) struct ProjectionlessPublicationWork {
    pending: PendingCommit,
    admission: Option<ProjectionlessFallbackAdmission>,
    host: Option<ProjectionlessHostProjection>,
    focus_leaving: Vec<Entity>,
    gate_before: Vec<Entity>,
    gate_after: Vec<Entity>,
    targets: Vec<crate::presentation::FrameMutationTarget>,
    isolate_count: usize,
    transitions: Vec<(ExactRegistryClaim, EntityLifecycle)>,
    resulting_claims: Vec<ExactRegistryClaim>,
    entering_rebind: crate::branch_transaction::PreparedBranchClaimRebind,
    leaving_rebind: Option<crate::branch_transaction::PreparedBranchClaimRebind>,
    retiring: Vec<r3_core::RetainedLeafProjection>,
    retained_change: Option<crate::retained::PreparedRetainedChange>,
    retiring_rebind: Vec<crate::branch_transaction::PreparedBranchClaimRebind>,
    leaving_assembly: Option<crate::cleanup::PreparedLeavingAssembly>,
    retained_boundaries: Option<crate::registry::PreparedRetainedBoundaries>,
    expected_manifest: FrozenCleanupManifest,
    reserved_ledger: PreparedCleanupLedger,
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: u64,
    obligation_count: usize,
    reject_prepare_commit: bool,
}

fn freeze_projectionless_leaving(
    world: &World,
    registry: &RouteEntityRegistry,
    entry: &RouterEntry,
    pending: &PendingCommit,
    resulting_claims: &[ExactRegistryClaim],
    retained_change: Option<&crate::retained::PreparedRetainedChange>,
) -> Option<FrozenCleanupManifest> {
    match (entry.active.as_ref(), retained_change) {
        (Some(leaving), Some(change)) => FrozenCleanupManifest::from_retained_partition(
            world,
            registry,
            leaving,
            None,
            &change.selected(&entry.retained).collect::<Vec<_>>(),
            resulting_claims,
        ),
        (Some(leaving), None) => FrozenCleanupManifest::from_leaving_branch(
            world,
            registry,
            leaving,
            resulting_claims,
            Some(pending.branch.as_realized()),
        ),
        (None, None) => Some(FrozenCleanupManifest::empty()),
        (None, Some(_)) => None,
    }
}

fn projectionless_leaving_focus(
    world: &World,
    entry: &RouterEntry,
    retained_change: Option<&crate::retained::PreparedRetainedChange>,
    mut focus: Vec<Entity>,
) -> Vec<Entity> {
    for (_, branch) in retained_change
        .into_iter()
        .flat_map(|change| change.selected(&entry.retained))
    {
        focus.extend(observed_descendant_closure(
            world,
            branch.frames().iter().map(RealizedFrame::shell),
        ));
    }
    focus.sort_by_key(|entity| entity.to_bits());
    focus.dedup();
    focus
}

#[cfg(test)]
pub(super) fn commit_projectionless_branch(
    world: &mut World,
    entry: &mut RouterEntry,
    pending: PendingCommit,
) -> Result<(), PendingCommit> {
    let mut work = None;
    let result = prepare_projectionless_branch(world, entry, pending, &mut work);
    assert!(
        work.is_none(),
        "this rejection-only helper cannot publish through a resource scope"
    );
    result
}

pub(super) fn prepare_projectionless_branch(
    world: &mut World,
    entry: &mut RouterEntry,
    mut pending: PendingCommit,
    work: &mut Option<ProjectionlessPublicationWork>,
) -> Result<(), PendingCommit> {
    if pending.ticket.is_some()
        || pending.final_task.is_none()
        || pending.branch.frames().is_empty()
    {
        return Err(pending);
    }
    let task_is_current = pending
        .final_task
        .as_ref()
        .is_some_and(|task| entry.engine.inspect_report_identity(task).is_ok());
    if !task_is_current {
        return Err(pending);
    }
    let replaces_active = entry.active.is_some();
    if replaces_active
        && pending.final_task.as_ref().is_none_or(|task| {
            task.frame().origin() != &bevy_ui_router_core::prelude::FrameOrigin::RouterFallback
        })
    {
        return Err(pending);
    }
    let missing = RequiredResourceSet::missing_from(world);
    if !replaces_active && !missing.is_empty() {
        let evidence = LifecyclePreflightEvidence::new(
            LifecyclePreflightSource::RequiredResources(missing),
            LifecyclePreflightExpectation::ResourcePresent,
            LifecyclePreflightObserved::Missing,
        );
        return match fail_closed_normal_owner_protocol(
            world,
            entry,
            RealizedBranch::empty(),
            pending,
            AuthorityProtocolResidualKind::PreflightAuthorityLost(evidence),
        ) {
            Ok(()) => Ok(()),
            Err((active, pending)) => {
                entry.active = (!active.frames().is_empty()).then_some(active);
                Err(pending)
            }
        };
    }
    let (admission, focus_leaving) = if replaces_active {
        match projectionless_fallback_preflight(world, entry, &pending) {
            ProjectionlessFallbackPreflight::Ready {
                admission,
                focus_leaving,
            } => (Some(admission), focus_leaving),
            ProjectionlessFallbackPreflight::PendingRejected(evidence) => {
                reject_projectionless_fallback_pending(entry, pending, evidence.pending_message());
                return Ok(());
            }
            ProjectionlessFallbackPreflight::AuthorityLost(evidence) => {
                return fail_closed_projectionless_replacement_contradiction_with_evidence(
                    world, entry, pending, evidence,
                );
            }
        }
    } else {
        (None, Vec::new())
    };
    // These are physical preparation candidates, held by their original owners.
    // Core's late commit plan must authorize this complete retirement before
    // any capability or payload moves. Fallback never retains a new leaf.
    let retiring = if entry.retained.is_empty() {
        Vec::new()
    } else {
        let Some(inventory) = entry.retained.inventory() else {
            return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
        };
        inventory
            .iter()
            .map(|owner| owner.projection().clone())
            .collect::<Vec<_>>()
    };
    let retained_change = if retiring.is_empty() {
        None
    } else {
        let Some(change) = entry.retained.prepare_all(&retiring) else {
            return Err(pending);
        };
        Some(change)
    };
    #[cfg(feature = "bsn")]
    let native_preparation = (|| {
        if let Some(active) = entry.active.as_mut() {
            active.prepare_native_cleanup(world)?;
        }
        if let Some(change) = &retained_change {
            for (_, branch) in change.selected_mut(&mut entry.retained) {
                branch.prepare_native_cleanup(world)?;
            }
        }
        Ok::<(), crate::presentation::native_frame::NativeCleanupPreparationError>(())
    })();
    #[cfg(feature = "bsn")]
    if let Err(error) = native_preparation {
        if error.is_retryable() {
            return Err(pending);
        }
        let active = entry
            .active
            .take()
            .expect("the leaving native owner is retained");
        return match fail_closed_normal_owner_protocol(
            world,
            entry,
            active,
            pending,
            AuthorityProtocolResidualKind::NativeCleanup(error),
        ) {
            Ok(()) => Ok(()),
            Err((active, pending)) => {
                entry.active = Some(active);
                Err(pending)
            }
        };
    }
    let gate_before = entry
        .active
        .as_ref()
        .map(|branch| branch.live_interactive_entities(world))
        .unwrap_or_default();
    let mut targets = match entry
        .active
        .as_ref()
        .map(|leaving| leaving.active_sparse_leaving_mutation_targets(EntityLifecycle::Leaving))
    {
        Some(Some(targets)) => targets,
        Some(None) => {
            return fail_closed_projectionless_replacement_contradiction_with_evidence(
                world,
                entry,
                pending,
                LifecyclePreflightEvidence::new(
                    LifecyclePreflightSource::LeavingCurrent,
                    LifecyclePreflightExpectation::ExactAuthority,
                    LifecyclePreflightObserved::Contradictory,
                ),
            );
        }
        None => Vec::new(),
    };
    let retirement_targets = (|| {
        for (_, branch) in retained_change
            .as_ref()
            .into_iter()
            .flat_map(|change| change.selected(&entry.retained))
        {
            targets.extend(branch.retained_leaving_mutation_targets()?);
        }
        Some(())
    })();
    if retirement_targets.is_none() {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    }
    let focus_leaving =
        projectionless_leaving_focus(world, entry, retained_change.as_ref(), focus_leaving);
    let isolate_count = targets.len();
    targets.extend(
        pending
            .branch
            .as_realized()
            .entering_mutation_targets(EntityLifecycle::PreparedInert, EntityLifecycle::Active),
    );
    let gate_after: Vec<_> = pending
        .branch
        .frames()
        .iter()
        .flat_map(|frame| frame.content_input_facts(world, &targets).1)
        .collect();
    let mut transitions = entry
        .active
        .as_ref()
        .map(|leaving| leaving.lifecycle_transitions(EntityLifecycle::Leaving, true))
        .unwrap_or_default();
    for (_, branch) in retained_change
        .as_ref()
        .into_iter()
        .flat_map(|change| change.selected(&entry.retained))
    {
        transitions.extend(branch.lifecycle_transitions(EntityLifecycle::Leaving, true));
    }
    let leaving_transition_count = transitions.len();
    transitions.extend(
        pending
            .branch
            .as_realized()
            .lifecycle_transitions(EntityLifecycle::Active, false),
    );
    let logical_roots = entry
        .active
        .as_ref()
        .map(logical_cleanup_roots)
        .unwrap_or_default();
    let prevalidated = {
        let registry = world.resource::<RouteEntityRegistry>();
        let leaving_exact = registry
            .prepare_lifecycle_mutation(&transitions[..leaving_transition_count])
            .is_some();
        let entering_exact = registry
            .prepare_lifecycle_mutation(&transitions[leaving_transition_count..])
            .is_some();
        let combined = registry.prepare_lifecycle_mutation(&transitions);
        (leaving_exact, entering_exact, combined)
    };
    if replaces_active && !prevalidated.0 {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    }
    if !prevalidated.1 {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering registry drifted",
            );
            return Ok(());
        }
        return Err(pending);
    }
    let Some(registry_plan) = prevalidated.2 else {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    };
    let resulting_claims = registry_plan.resulting_claims().to_vec();
    drop(registry_plan);

    let entering_claims = pending
        .branch
        .as_realized()
        .claims()
        .map(|current| {
            resulting_claims
                .iter()
                .find(|next| next.entity() == current.entity())
                .cloned()
        })
        .collect::<Option<Vec<_>>>();
    let entering_rebind = entering_claims.as_ref().and_then(|claims| {
        pending.branch.as_realized().prepare_claim_rebind(
            claims,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        )
    });
    let Some(entering_rebind) = entering_rebind else {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering claim drifted",
            );
            return Ok(());
        }
        return Err(pending);
    };
    let leaving_rebind = entry.active.as_ref().map(|leaving| {
        let claims = leaving
            .claims()
            .map(|current| {
                resulting_claims
                    .iter()
                    .find(|next| next.entity() == current.entity())
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        leaving.prepare_active_sparse_claim_rebind(&claims, EntityLifecycle::Leaving)
    });
    if matches!(leaving_rebind, Some(None)) {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    }
    let leaving_rebind = leaving_rebind.flatten();

    let frozen_manifest = freeze_projectionless_leaving(
        world,
        world.resource::<RouteEntityRegistry>(),
        entry,
        &pending,
        &resulting_claims,
        retained_change.as_ref(),
    );
    let Some(frozen_manifest) = frozen_manifest else {
        return fail_closed_projectionless_replacement_contradiction(world, entry, pending);
    };
    let manifest_digest = frozen_manifest.digest();
    let obligation_count = frozen_manifest.len();
    let retirement_preparation = (|| {
        let Some(change) = &retained_change else {
            return Some((None, Vec::new(), None));
        };
        let leaving = entry.active.as_ref()?;
        let retired = change
            .selected(&entry.retained)
            .map(|(_, branch)| branch)
            .collect::<Vec<_>>();
        let frames = leaving
            .frames()
            .iter()
            .chain(retired.iter().flat_map(|branch| branch.frames()))
            .collect::<Vec<_>>();
        let slots = leaving
            .child_slots()
            .iter()
            .chain(retired.iter().flat_map(|branch| branch.child_slots()))
            .collect::<Vec<_>>();
        let assembly =
            crate::cleanup::PreparedLeavingAssembly::reserve(&frozen_manifest, &frames, &slots)?;
        let rebinds = retired
            .iter()
            .map(|branch| {
                branch.prepare_retained_claim_rebind(&resulting_claims, EntityLifecycle::Leaving)
            })
            .collect::<Option<Vec<_>>>()?;
        let boundaries = world
            .resource::<RouteEntityRegistry>()
            .prepare_retained_boundaries(None, &retiring)
            .ok()?;
        Some((Some(assembly), rebinds, Some(boundaries)))
    })();
    let Some((leaving_assembly, retiring_rebind, retained_boundaries)) = retirement_preparation
    else {
        return Err(pending);
    };

    if prepare_input_gate_mutation(
        world.resource_mut::<RouterInputGate>(),
        &gate_before,
        Vec::new(),
    )
    .is_none()
    {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    }
    if prepare_input_gate_mutation(
        world.resource_mut::<RouterInputGate>(),
        &gate_before,
        gate_after.clone(),
    )
    .is_none()
    {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering input authority drifted",
            );
            return Ok(());
        }
        return Err(pending);
    }
    if prepare_presentation_mutation(world, &targets[..isolate_count]).is_none() {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    }
    if prepare_presentation_mutation(world, &targets[isolate_count..]).is_none() {
        if replaces_active {
            reject_projectionless_fallback_pending(
                entry,
                pending,
                "projectionless fallback entering presentation drifted",
            );
            return Ok(());
        }
        return Err(pending);
    }
    if prepare_presentation_mutation(world, &targets)
        .and_then(|prepared| prepared.seal_commit(isolate_count))
        .is_none()
    {
        return if replaces_active {
            fail_closed_projectionless_replacement_contradiction(world, entry, pending)
        } else {
            Err(pending)
        };
    }

    let expected_manifest = frozen_manifest.clone();
    let reserved_ledger = match CleanupLedger::reserve(frozen_manifest) {
        Ok(reserved) => reserved,
        Err(error) => match error.kind() {
            CleanupLedgerReserveErrorKind::RetryableCapacity => return Err(pending),
            kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
            | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                let _returned_manifest = error.into_manifest();
                return if replaces_active {
                    let active = entry
                        .active
                        .take()
                        .expect("projectionless replacement retains active owner");
                    match fail_closed_normal_owner_protocol(
                        world,
                        entry,
                        active,
                        pending,
                        AuthorityProtocolResidualKind::CleanupLedger(kind),
                    ) {
                        Ok(()) => Ok(()),
                        Err((active, pending)) => {
                            entry.active = Some(active);
                            Err(pending)
                        }
                    }
                } else {
                    fail_closed_projectionless_owner_protocol(world, entry, pending, kind)
                };
            }
        },
    };
    let reject_prepare_commit = inject_projectionless_prepare_commit_rejection(world);
    *work = Some(ProjectionlessPublicationWork {
        pending,
        admission,
        host: entry.host.as_ref().map(ProjectionlessHostProjection::from),
        focus_leaving,
        gate_before,
        gate_after,
        targets,
        isolate_count,
        transitions,
        resulting_claims,
        entering_rebind,
        leaving_rebind,
        retiring,
        retained_change,
        retiring_rebind,
        leaving_assembly,
        retained_boundaries,
        expected_manifest,
        reserved_ledger,
        logical_roots,
        manifest_digest,
        obligation_count,
        reject_prepare_commit,
    });
    Ok(())
}

pub(super) fn publish_projectionless_work(
    world: &mut World,
    handle: RouterHandle,
    work: ProjectionlessPublicationWork,
) {
    let ProjectionlessPublicationWork {
        mut pending,
        admission,
        host,
        focus_leaving,
        gate_before,
        gate_after,
        targets,
        isolate_count,
        transitions,
        resulting_claims,
        entering_rebind,
        leaving_rebind,
        retiring,
        mut retained_change,
        retiring_rebind,
        leaving_assembly,
        retained_boundaries,
        expected_manifest,
        reserved_ledger,
        logical_roots,
        manifest_digest,
        obligation_count,
        reject_prepare_commit,
    } = work;
    let preflight = {
        let navigation = world.resource::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)
            .unwrap();
        if admission.is_some() {
            let current = match projectionless_fallback_preflight(world, entry, &pending) {
                ProjectionlessFallbackPreflight::Ready {
                    admission,
                    focus_leaving,
                } => ProjectionlessFallbackPreflight::Ready {
                    admission,
                    focus_leaving: projectionless_leaving_focus(
                        world,
                        entry,
                        retained_change.as_ref(),
                        focus_leaving,
                    ),
                },
                other => other,
            };
            Some(current)
        } else {
            let missing = RequiredResourceSet::missing_from(world);
            if !missing.is_empty() {
                Some(ProjectionlessFallbackPreflight::AuthorityLost(
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::RequiredResources(missing),
                        LifecyclePreflightExpectation::ResourcePresent,
                        LifecyclePreflightObserved::Missing,
                    ),
                ))
            } else if entry.host.as_ref().map(ProjectionlessHostProjection::from) != host
                || entry
                    .host
                    .as_ref()
                    .is_none_or(|host| !current_host_binding_is_exact(world, host))
                || entry.active.is_some()
            {
                Some(ProjectionlessFallbackPreflight::AuthorityLost(
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::HostBinding,
                        LifecyclePreflightExpectation::ExactAuthority,
                        LifecyclePreflightObserved::Drifted,
                    ),
                ))
            } else if validate_prepared_branch(world, pending.branch.as_realized()).is_err()
                || validate_branch_descendants(world, pending.branch.as_realized()).is_err()
            {
                Some(ProjectionlessFallbackPreflight::PendingRejected(
                    LifecyclePreflightEvidence::new(
                        LifecyclePreflightSource::PendingEntering,
                        LifecyclePreflightExpectation::ExactRelationshipClosure,
                        LifecyclePreflightObserved::Drifted,
                    ),
                ))
            } else {
                None
            }
        }
    };
    let evidence = match preflight {
        Some(ProjectionlessFallbackPreflight::Ready {
            admission: current,
            focus_leaving: current_focus,
        }) if Some(&current) == admission.as_ref() && current_focus == focus_leaving => None,
        Some(ProjectionlessFallbackPreflight::PendingRejected(evidence)) => Some((false, evidence)),
        Some(ProjectionlessFallbackPreflight::AuthorityLost(evidence)) => Some((true, evidence)),
        Some(ProjectionlessFallbackPreflight::Ready { .. }) => Some((
            true,
            LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::TargetPartition,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Drifted,
            ),
        )),
        None => None,
    };
    if let Some((authority, evidence)) = evidence {
        world.resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == handle)
                    .unwrap();
                if authority {
                    if entry.active.is_none() {
                        settle_prepublication_authority_loss(
                            world,
                            entry,
                            RealizedBranch::empty(),
                            pending,
                            false,
                            evidence,
                            true,
                        );
                    } else if let Err(pending) =
                        fail_closed_projectionless_replacement_contradiction_with_evidence(
                            world, entry, pending, evidence,
                        )
                    {
                        entry.pending_commit = Some(pending);
                    }
                } else {
                    reject_projectionless_fallback_pending(
                        entry,
                        pending,
                        evidence.pending_message(),
                    );
                }
            },
        );
        return;
    }
    let frozen_is_exact = (|| {
        let navigation = world.get_resource::<RouterNavigation>()?;
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)?;
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        entry
            .engine
            .inspect_report_identity(pending.final_task.as_ref()?)
            .ok()?;
        let current = registry.prepare_lifecycle_mutation(&transitions)?;
        if current.resulting_claims() != resulting_claims {
            return None;
        }
        let frozen = freeze_projectionless_leaving(
            world,
            registry,
            entry,
            &pending,
            &resulting_claims,
            retained_change.as_ref(),
        )?;
        (frozen == expected_manifest).then_some(())
    })()
    .is_some();
    #[cfg(feature = "bsn")]
    let native_focus =
        crate::native_focus::prepare_publication(world, pending.branch.frames(), &gate_after);
    let pointer_targets = world
        .get_resource_mut::<RouterInputGate>()
        .and_then(|gate| prepare_input_gate_mutation(gate, &gate_before, gate_after.clone()))
        .map(|plan| plan.revoked_targets().to_vec());
    let scroll_eligible = crate::scroll::eligible_containers(world);
    let scroll_observation = {
        let navigation = world.resource::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)
            .unwrap();
        crate::scroll::ScrollObservation::capture(
            world,
            entry.active.iter().flat_map(|branch| branch.frames()),
            pending.branch.frames().iter(),
            &scroll_eligible,
        )
    };
    let scroll_targets = scroll_observation.target_entities();
    let mut branch_owners = None;
    let mut failure = None;
    let mut rejected_retained_publication = None;
    let committed = (|| {
        if !frozen_is_exact {
            return None;
        }
        let (presentation, resources) =
            crate::presentation::prepare_publication_access_with_scroll(
                world,
                &targets,
                pointer_targets.as_ref()?,
                &scroll_targets,
            )?;
        let crate::presentation::PublicationResources {
            mut navigation,
            mut registry,
            gate,
            input_focus,
            route_focus,
        } = resources;
        let entry = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)?;
        let registry_plan = registry.prepare_lifecycle_mutation(&transitions)?;
        let gate_plan = prepare_input_gate_mutation(gate, &gate_before, gate_after)?;
        let focus_plan = prepare_focus_mutation(input_focus, route_focus, &focus_leaving);
        #[cfg(feature = "bsn")]
        let focus_plan = focus_plan.with_native_publication(native_focus.as_ref()?)?;
        let presentation_plan = presentation.seal_commit(isolate_count)?;
        let root_rebase = match entry.active.as_ref() {
            Some(leaving) => Some(pending.branch.as_realized().prepare_root_rebase(leaving)?),
            None => None,
        };
        let leaving = entry.active.take();
        let entering =
            std::mem::replace(&mut pending.branch, PreparedBranch::empty()).into_realized();
        branch_owners = Some((leaving, entering));
        let task = pending
            .final_task
            .take()
            .expect("projectionless fallback task remains owned");
        let ticket = entry
            .engine
            .report_applied(task)
            .expect("preflighted fallback task remains reportable");
        pending.ticket = Some(ticket);
        let retention = if retained_change.is_some() {
            let plan = entry
                .retained
                .inventory()
                .ok_or(IntegrationError::Stale)
                .and_then(|inventory| {
                    entry
                        .engine
                        .prepare_retained_commit_plan(pending.ticket.as_ref().unwrap(), &inventory)
                });
            match plan {
                Ok(plan)
                    if plan.retained().is_none()
                        && plan.preserved().is_empty()
                        && plan.retiring().len() == retiring.len()
                        && plan
                            .retiring()
                            .iter()
                            .all(|projection| retiring.contains(projection))
                        && plan
                            .disposable_roots()
                            .iter()
                            .eq(logical_roots.iter().chain(
                                plan.retiring().iter().map(|projection| projection.frame()),
                            )) =>
                {
                    Some(plan)
                }
                _ => {
                    failure = Some(ProjectionlessCommitFailure::post_report(
                        admission.clone(),
                        BeginAuthoritySettlementErrorKind::InvalidManifest,
                    ));
                    return None;
                }
            }
        } else {
            None
        };
        let mut manifest_logical_roots = retention
            .as_ref()
            .map(|plan| plan.disposable_roots().to_vec())
            .unwrap_or(logical_roots);
        if reject_prepare_commit {
            let duplicate = manifest_logical_roots
                .first()
                .expect("replacement rejection fixture has one leaving root")
                .clone();
            manifest_logical_roots.push(duplicate);
        }
        let manifest =
            CleanupManifest::new(manifest_logical_roots, manifest_digest, obligation_count);
        let prepared = if let Some(plan) = retention {
            let owners = retained_change
                .as_mut()
                .unwrap()
                .take_owners(&mut entry.retained);
            entry
                .engine
                .prepare_commit_with_retention(
                    pending.ticket.as_ref().unwrap(),
                    plan,
                    manifest,
                    owners,
                    None,
                )
                .map(crate::retained::PreparedPublication::Retained)
                .map_err(|error| {
                    let (_plan, _manifest, owners, kind) = error.into_parts();
                    assert!(
                        entry.retained.restore_owners(owners).is_ok(),
                        "fallback preparation returns the original cache owners"
                    );
                    kind
                })
        } else {
            entry
                .engine
                .prepare_commit(pending.ticket.as_ref().unwrap(), manifest)
                .map(crate::retained::PreparedPublication::Ordinary)
                .map_err(r3_core::RetainedPublicationError::Integration)
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(_) => {
                failure = Some(ProjectionlessCommitFailure::post_report(
                    admission.clone(),
                    BeginAuthoritySettlementErrorKind::InvalidManifest,
                ));
                return None;
            }
        };
        let engine = match prepared.validate(&mut entry.engine) {
            Ok(validated) => validated,
            Err((returned, error)) => {
                rejected_retained_publication = returned;
                failure = Some(ProjectionlessCommitFailure::post_report(
                    admission.clone(),
                    match error {
                        IntegrationError::InvalidManifest => {
                            BeginAuthoritySettlementErrorKind::InvalidManifest
                        }
                        IntegrationError::Exhausted
                        | IntegrationError::AwaitingHost
                        | IntegrationError::OutstandingAdmission
                        | IntegrationError::AuthorityExists
                        | IntegrationError::ForeignRouter
                        | IntegrationError::Duplicate
                        | IntegrationError::Stale
                        | IntegrationError::Invalid => BeginAuthoritySettlementErrorKind::Stale,
                    },
                ));
                return None;
            }
        };
        let scroll = entry
            .scroll
            .prepare(engine.history_publication(), scroll_observation);
        let presentation_plan = presentation_plan.with_scroll(&scroll.writes);
        let cleanup = engine.cleanup().clone();
        let cleanup_ledger = reserved_ledger.bind_batch(cleanup);
        pending.ticket.take();
        let (mut leaving, mut entering) = branch_owners
            .take()
            .expect("fully prepared projectionless commit retains its branch owners");
        let retired_payloads = if let Some(change) = retained_change.take() {
            for ((_, branch), rebind) in change
                .selected_mut(&mut entry.retained)
                .zip(retiring_rebind)
            {
                branch.apply_claim_rebind(rebind);
            }
            change.into_payloads(&mut entry.retained)
        } else {
            Vec::new()
        };
        if let (Some(leaving), Some(rebind)) = (leaving.as_mut(), leaving_rebind) {
            leaving.apply_claim_rebind(rebind);
        }
        entering.apply_claim_rebind(entering_rebind);
        if let Some(rebase) = root_rebase {
            entering.apply_root_rebase(rebase);
        }
        entering.consume_native_focus_owners();
        let next_active = Some(entering);
        let next_cleanup = Some(if let Some(assembly) = leaving_assembly {
            CleanupLedger::activate_leaving_forest(
                cleanup_ledger,
                assembly.assemble(
                    leaving.expect("cache retirement retains its parent branch"),
                    retired_payloads,
                ),
            )
        } else {
            CleanupLedger::activate_leaving_branch(cleanup_ledger, leaving)
        });
        let active_owner = &mut entry.active;
        let cleanup_owner = &mut entry.cleanup_ledger;
        let scroll_owner = &mut entry.scroll;
        engine.publish(|_| {
            presentation_plan.apply_staged(
                |_| focus_plan.apply(),
                |_| {
                    registry_plan.apply(&mut registry);
                    if let Some(boundaries) = retained_boundaries {
                        boundaries.apply(&mut registry);
                    }
                },
                |_| gate_plan.apply(),
            );
            *active_owner = next_active;
            *cleanup_owner = next_cleanup;
            *scroll_owner = scroll.state;
        });
        Some(())
    })();
    if committed.is_some() {
        return;
    }
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .unwrap();
            if let Some(prepared) = rejected_retained_publication.take() {
                match entry
                    .engine
                    .cancel_prepared_retained_commit_for_authority_settlement(prepared)
                {
                    Ok(owners) => assert!(
                        entry.retained.restore_owners(owners).is_ok(),
                        "rejected fallback validation returns every original cache owner"
                    ),
                    Err((prepared, _)) => {
                        pending.rejected_retained_publication = Some(Box::new(prepared))
                    }
                }
            }
            if branch_owners.is_none() {
                let result = if entry.active.is_some() {
                    fail_closed_projectionless_replacement_contradiction(world, entry, pending)
                } else {
                    reject_projectionless_fallback_pending(
                        entry,
                        pending,
                        "initial fallback final preparation rejected",
                    );
                    Ok(())
                };
                if let Err(pending) = result {
                    entry.pending_commit = Some(pending);
                }
                return;
            }
            let result = (|| {
                let Some(()) = committed else {
                    let (leaving, entering) = branch_owners
                        .take()
                        .expect("failed projectionless commit retains its branch owners");
                    pending.branch = PreparedBranch::from_realized(entering);
                    return match failure
                        .expect("projectionless commit returns None only with typed failure")
                    {
                        ProjectionlessCommitFailure::InitialPostReportAuthorityLost { reason } => {
                            match fail_closed_normal_owner_protocol(
                                world,
                                entry,
                                leaving.unwrap_or_else(RealizedBranch::empty),
                                pending,
                                AuthorityProtocolResidualKind::Authority(reason),
                            ) {
                                Ok(()) => Ok(()),
                                Err((active, pending)) => {
                                    entry.active = (!active.frames().is_empty()).then_some(active);
                                    Err(pending)
                                }
                            }
                        }
                        ProjectionlessCommitFailure::PostReportAuthorityLost {
                            admission,
                            reason,
                        } => {
                            let active = leaving.expect(
                                "projectionless replacement continuation retains active owner",
                            );
                            let source = ProjectionlessFailCloseSource::freeze(
                                world, entry, admission, reason, &active, &pending,
                            );
                            match settle_projectionless_fail_close(
                                world, entry, active, pending, source,
                            ) {
                                Ok(()) => Ok(()),
                                Err((active, pending, source)) => {
                                    entry.active = Some(active);
                                    entry.pending_commit = Some(pending);
                                    entry.projectionless_fail_close = Some(
                                        ProjectionlessFailCloseContinuation::Retryable(source),
                                    );
                                    Ok(())
                                }
                            }
                        }
                    };
                };
                Ok(())
            })();
            if let Err(pending) = result {
                entry.pending_commit = Some(pending);
            }
        },
    );
}
