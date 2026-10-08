use super::*;

pub(super) fn publish_normal_work(
    world: &mut World,
    handle: RouterHandle,
    mut pending: PendingCommit,
    splice: SealedReconciliationSplice,
    had_active: bool,
    has_leaving: bool,
    mut sealed: Option<SealedNormalPreparation>,
) {
    let host_is_current = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .find(|entry| entry.handle == handle)
        .and_then(|entry| entry.host.as_ref())
        .is_some_and(|host| current_host_binding_is_exact(world, host));
    let missing = RequiredResourceSet::missing_from(world);
    let preflight = if missing.is_empty() {
        reconciliation_preflight(world, host_is_current, has_leaving, splice)
    } else {
        ReconciliationPreflightOutcome::AuthorityLost {
            splice,
            evidence: LifecyclePreflightEvidence::new(
                LifecyclePreflightSource::RequiredResources(missing),
                LifecyclePreflightExpectation::ResourcePresent,
                LifecyclePreflightObserved::Missing,
            ),
        }
    };
    let (mut splice, mut focus_leaving) = match preflight {
        ReconciliationPreflightOutcome::Ready {
            splice,
            focus_leaving,
        } => (splice, focus_leaving),
        rejected => {
            world.resource_scope(
                |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                    let entry = navigation
                        .entries
                        .iter_mut()
                        .find(|entry| entry.handle == handle)
                        .unwrap();
                    if let Some(sealed) = sealed.take() {
                        let splice = match rejected {
                            ReconciliationPreflightOutcome::PendingRejected { splice, .. }
                            | ReconciliationPreflightOutcome::AuthorityLost { splice, .. } => {
                                splice
                            }
                            ReconciliationPreflightOutcome::Ready { .. } => unreachable!(),
                        };
                        let (active, entering, _) = splice.reject().into_parts();
                        pending.branch = entering;
                        if entry
                            .engine
                            .cancel_prepared_commit_for_authority_settlement(sealed.prepared)
                            .is_err()
                        {
                            if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                                world,
                                entry,
                                active,
                                pending,
                                AuthorityProtocolResidualKind::Authority(
                                    BeginAuthoritySettlementErrorKind::Stale,
                                ),
                            ) {
                                entry.active = had_active.then_some(active);
                                entry.pending_commit = Some(pending);
                            }
                        } else {
                            entry.active = had_active.then_some(active);
                            entry.pending_commit = Some(pending);
                        }
                        return;
                    }
                    match rejected {
                        ReconciliationPreflightOutcome::Ready { .. } => unreachable!(),
                        ReconciliationPreflightOutcome::PendingRejected {
                            splice,
                            evidence: _,
                        } => {
                            let (active, entering, _roles) = splice.reject().into_parts();
                            entry.active = had_active.then_some(active);
                            pending.branch = entering;
                            let Some(task) = pending.final_task.take() else {
                                entry.pending_commit = Some(pending);
                                return;
                            };
                            let failed = PendingFailedBranch {
                                terminal: PendingFailedBranchTerminal::TaskWithPhysicalResidual {
                                    task,
                                    failure: ViewFailureKind::Apply,
                                    message: "frame final preflight rejected".into(),
                                },
                                branch: failed_prepared_branch(pending, None),
                            };
                            let _ = isolate_exact_failed_prepared_branch(
                                world,
                                &failed.branch,
                                entry.active.as_ref(),
                            );
                            entry.pending_failed_branch = Some(failed);
                            return;
                        }
                        ReconciliationPreflightOutcome::AuthorityLost { splice, evidence } => {
                            let (active, entering, _roles) = splice.reject().into_parts();
                            pending.branch = entering;
                            settle_prepublication_authority_loss(
                                world, entry, active, pending, had_active, evidence, false,
                            );
                            return;
                        }
                    }
                },
            );
            return;
        }
    };
    // The complete Core partition is frozen before any retained capability
    // moves. The ordinary, default-disabled path keeps its original protocol.
    let retention = world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .unwrap();
            if entry.engine.keep_alive_capacity() == 0 && entry.retained.is_empty() {
                return Ok(None);
            }
            if sealed.is_some() {
                return Err(Some(AuthorityProtocolResidualKind::Authority(
                    BeginAuthoritySettlementErrorKind::Stale,
                )));
            }
            if pending.ticket.is_none() {
                let task = pending
                    .final_task
                    .take()
                    .expect("preflighted task retains its owner");
                pending.ticket = Some(
                    entry
                        .engine
                        .report_applied(task)
                        .expect("preflighted task reports once"),
                );
            }
            let inventory = entry.retained.inventory().ok_or_else(|| {
                Some(AuthorityProtocolResidualKind::Authority(
                    BeginAuthoritySettlementErrorKind::Stale,
                ))
            })?;
            let plan = entry
                .engine
                .prepare_retained_commit_plan(pending.ticket.as_ref().unwrap(), &inventory)
                .map_err(|_| {
                    Some(AuthorityProtocolResidualKind::Authority(
                        BeginAuthoritySettlementErrorKind::Stale,
                    ))
                })?;
            if let Some(projection) = plan.retained() {
                if !splice.retain_leaf(projection) {
                    return Err(Some(AuthorityProtocolResidualKind::Authority(
                        BeginAuthoritySettlementErrorKind::InvalidManifest,
                    )));
                }
            }
            let change = entry
                .retained
                .prepare_change(plan.retiring(), plan.retained().is_some())
                .ok_or(None)?;
            #[cfg(feature = "bsn")]
            for (_, branch) in change.selected_mut(&mut entry.retained) {
                if let Err(error) = branch.prepare_native_cleanup(world) {
                    return Err(if error.is_retryable() {
                        None
                    } else {
                        Some(AuthorityProtocolResidualKind::NativeCleanup(error))
                    });
                }
            }
            Ok(Some((plan, change)))
        },
    );
    let (mut retention, mut retained_change) = match retention {
        Ok(Some((plan, change))) => (Some(plan), Some(change)),
        Ok(None) => (None, None),
        Err(contradiction) => {
            world.resource_scope(
                |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                    let entry = navigation
                        .entries
                        .iter_mut()
                        .find(|entry| entry.handle == handle)
                        .unwrap();
                    let (active, entering, _) = splice.reject().into_parts();
                    pending.branch = entering;
                    if let Some(kind) = contradiction {
                        if let Err((active, pending)) =
                            fail_closed_normal_owner_protocol(world, entry, active, pending, kind)
                        {
                            entry.active = had_active.then_some(active);
                            entry.pending_commit = Some(pending);
                        }
                    } else {
                        entry.active = had_active.then_some(active);
                        entry.pending_commit = Some(pending);
                    }
                },
            );
            return;
        }
    };
    let retained_projection = retention.as_ref().and_then(|plan| plan.retained().cloned());
    let retiring = retention
        .as_ref()
        .map(|plan| plan.retiring().to_vec())
        .unwrap_or_default();
    #[cfg(feature = "bsn")]
    if let Err(error) = splice.prepare_native_cleanup(world) {
        world.resource_scope(
            |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                let entry = navigation
                    .entries
                    .iter_mut()
                    .find(|entry| entry.handle == handle)
                    .unwrap();
                let (active, entering, _) = splice.reject().into_parts();
                pending.branch = entering;
                if error.is_retryable() {
                    entry.active = had_active.then_some(active);
                    entry.pending_commit = Some(pending);
                } else if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                    world,
                    entry,
                    active,
                    pending,
                    AuthorityProtocolResidualKind::NativeCleanup(error),
                ) {
                    entry.active = had_active.then_some(active);
                    entry.pending_commit = Some(pending);
                }
            },
        );
        return;
    }
    let retained_empty_focus = observed_descendant_closure(
        world,
        splice
            .slot_roles()
            .iter()
            .filter(|role| role.kind() == ReconciliationSlotRoleKind::RetainEmpty)
            .map(|role| role.entity()),
    );
    focus_leaving.extend(retained_empty_focus);
    focus_leaving.sort_by_key(|entity| entity.to_bits());
    focus_leaving.dedup();
    let logical_roots = retention
        .as_ref()
        .map(|plan| plan.disposable_roots().to_vec())
        .unwrap_or_else(|| logical_cleanup_roots(splice.leaving()));
    let scroll_eligible = crate::scroll::eligible_containers(world);
    let scroll_observation = crate::scroll::ScrollObservation::capture(
        world,
        splice
            .preserved()
            .frames()
            .iter()
            .chain(splice.leaving().frames()),
        splice
            .preserved()
            .frames()
            .iter()
            .chain(splice.entering().frames()),
        &scroll_eligible,
    );
    let scroll_targets = scroll_observation.target_entities();
    let gate_before = splice.current_live_interactive_entities(world);
    let (mut targets, mut isolate_count) = splice.presentation_mutation_targets();
    let mut transitions = splice.lifecycle_transitions();
    {
        let navigation = world.resource::<RouterNavigation>();
        let entry = navigation
            .entries
            .iter()
            .find(|entry| entry.handle == handle)
            .unwrap();
        for (_, branch) in retained_change
            .as_ref()
            .into_iter()
            .flat_map(|change| change.selected(&entry.retained))
        {
            let retired_targets = branch
                .retained_leaving_mutation_targets()
                .expect("retained payload was sealed with its owner");
            let count = retired_targets.len();
            targets.splice(isolate_count..isolate_count, retired_targets);
            isolate_count += count;
            transitions.extend(branch.lifecycle_transitions(EntityLifecycle::Leaving, true));
            focus_leaving.extend(observed_descendant_closure(
                world,
                branch.frames().iter().map(RealizedFrame::shell),
            ));
        }
    }
    focus_leaving.sort_by_key(|entity| entity.to_bits());
    focus_leaving.dedup();
    let gate_after = splice.resulting_live_interactive_entities(world, &targets);

    let registry_changes = transitions
        .iter()
        .any(|(claim, next)| claim.lifecycle() != *next);
    let root_rebase =
        if splice.disposable_frames().next().is_some() && !splice.entering().frames().is_empty() {
            match splice.prepare_root_rebase() {
                Some(prepared) => Some(prepared),
                None => {
                    let (active, entering, _roles) = splice.reject().into_parts();
                    let mut navigation = world.resource_mut::<RouterNavigation>();
                    let entry = navigation
                        .entries
                        .iter_mut()
                        .find(|entry| entry.handle == handle)
                        .unwrap();
                    entry.active = had_active.then_some(active);
                    pending.branch = entering;
                    entry.pending_commit = Some(pending);
                    return;
                }
            }
        } else {
            None
        };

    let cleanup_preparation = (|| {
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        let registry_plan = registry.prepare_lifecycle_mutation(&transitions)?;
        let resulting_claims = registry_plan.resulting_claims().to_vec();
        let frozen_manifest = if retention.is_some() {
            let navigation = world.get_resource::<RouterNavigation>()?;
            let entry = navigation
                .entries
                .iter()
                .find(|entry| entry.handle == handle)?;
            let retired = retained_change
                .as_ref()?
                .selected(&entry.retained)
                .collect::<Vec<_>>();
            FrozenCleanupManifest::from_retained_partition(
                world,
                registry,
                splice.leaving(),
                retained_projection.as_ref(),
                &retired,
                &resulting_claims,
                (!splice.entering().frames().is_empty()).then_some(splice.entering()),
            )?
        } else if splice.leaving().obligation_count() == 0 {
            FrozenCleanupManifest::empty()
        } else {
            FrozenCleanupManifest::from_leaving_branch(
                world,
                registry,
                splice.leaving(),
                &resulting_claims,
                (!splice.entering().frames().is_empty()).then_some(splice.entering()),
            )?
        };

        Some((frozen_manifest, registry_plan, resulting_claims))
    })();
    #[cfg(feature = "bsn")]
    let native_focus =
        crate::native_focus::prepare_publication(world, splice.entering().frames(), &gate_after);
    let mut splice_owner = Some(splice);
    let mut cleanup_reserve_contradiction = None;
    let mut commit_authority_contradiction = None;
    let mut rejected_retained_publication = None;
    let committed = (|| {
        let (frozen_manifest, registry_plan, resulting_claims) = cleanup_preparation?;
        if sealed
            .as_ref()
            .is_some_and(|sealed| sealed.manifest != frozen_manifest)
        {
            commit_authority_contradiction =
                Some(BeginAuthoritySettlementErrorKind::InvalidManifest);
            return None;
        }
        let pointer_targets = prepare_input_gate_mutation(
            world.get_resource_mut::<RouterInputGate>()?,
            &gate_before,
            gate_after.clone(),
        )?
        .revoked_targets()
        .to_vec();
        let (presentation, resources) =
            crate::presentation::prepare_publication_access_with_scroll(
                world,
                &targets,
                &pointer_targets,
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
        let retired_branches = retained_change
            .as_ref()
            .into_iter()
            .flat_map(|change| change.selected(&entry.retained).map(|(_, branch)| branch))
            .collect::<Vec<_>>();
        let leaving_assembly = if retention.is_some() {
            Some(
                crate::cleanup::PreparedLeavingAssembly::reserve_retained_partition(
                    &frozen_manifest,
                    splice_owner.as_ref()?,
                    &retired_branches,
                )?,
            )
        } else {
            None
        };
        let retained_boundaries = if retention.is_some() {
            let addition = if retained_projection.is_some() {
                Some(
                    splice_owner
                        .as_ref()?
                        .prepare_retained_boundary(&resulting_claims)?,
                )
            } else {
                None
            };
            Some(
                registry
                    .prepare_retained_boundaries(addition, &retiring)
                    .ok()?,
            )
        } else {
            None
        };
        let retiring_rebind = retired_branches
            .iter()
            .map(|branch| {
                branch.prepare_retained_claim_rebind(&resulting_claims, EntityLifecycle::Leaving)
            })
            .collect::<Option<Vec<_>>>()?;
        let manifest_digest = frozen_manifest.digest();
        let obligation_count = frozen_manifest.len();
        let reserved_ledger = match CleanupLedger::reserve(frozen_manifest) {
            Ok(reserved) => reserved,
            Err(error) => match error.kind() {
                CleanupLedgerReserveErrorKind::RetryableCapacity => {
                    return None;
                }
                kind @ (CleanupLedgerReserveErrorKind::DomainExhausted
                | CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts) => {
                    let _returned_manifest = error.into_manifest();
                    cleanup_reserve_contradiction = Some(kind);
                    return None;
                }
            },
        };

        let gate_plan = prepare_input_gate_mutation(gate, &gate_before, gate_after)?;
        let focus_plan = prepare_focus_mutation(input_focus, route_focus, &focus_leaving);
        #[cfg(feature = "bsn")]
        let focus_plan = focus_plan.with_native_publication(native_focus.as_ref()?)?;
        let splice = splice_owner.as_ref()?;
        let splice_transitions = splice.lifecycle_transitions();
        let splice_claims = resulting_claims
            .iter()
            .filter(|claim| {
                splice_transitions
                    .iter()
                    .any(|(source, _)| source.entity() == claim.entity())
            })
            .cloned()
            .collect::<Vec<_>>();
        let claim_rebind = splice.prepare_claim_rebind(&splice_claims)?;
        if pending.ticket.is_none() {
            let task = pending
                .final_task
                .take()
                .expect("fully preflighted legacy leaf task remains owned");
            pending.ticket = Some(
                entry
                    .engine
                    .report_applied(task)
                    .expect("sealed task projection must report exactly once"),
            );
        }
        let ticket = pending.ticket.as_ref()?;
        let presentation_plan = presentation.seal_commit(isolate_count)?;
        let prepared = if let Some(sealed) = sealed.take() {
            crate::retained::PreparedNormalPublication::Ordinary(sealed.prepared)
        } else if let Some(plan) = retention.take() {
            let owners = retained_change.as_mut()?.take_owners(&mut entry.retained);
            let manifest =
                CleanupManifest::new(logical_roots.clone(), manifest_digest, obligation_count);
            match entry
                .engine
                .prepare_commit_with_retention(ticket, plan, manifest, owners)
            {
                Ok(prepared) => crate::retained::PreparedNormalPublication::Retained(prepared),
                Err(error) => {
                    let (_plan, _manifest, owners, error) = error.into_parts();
                    assert!(
                        entry.retained.restore_owners(owners).is_ok(),
                        "original owners return to their untouched holders"
                    );
                    commit_authority_contradiction =
                        Some(if error == IntegrationError::InvalidManifest {
                            BeginAuthoritySettlementErrorKind::InvalidManifest
                        } else {
                            BeginAuthoritySettlementErrorKind::Stale
                        });
                    return None;
                }
            }
        } else {
            let manifest =
                CleanupManifest::new(logical_roots.clone(), manifest_digest, obligation_count);
            match entry.engine.prepare_commit(ticket, manifest) {
                Ok(prepared) => crate::retained::PreparedNormalPublication::Ordinary(prepared),
                Err(error) => {
                    commit_authority_contradiction = Some(match error {
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
                    });
                    return None;
                }
            }
        };
        let engine = match prepared.validate(&mut entry.engine) {
            Ok(validated) => validated,
            Err((returned, error)) => {
                rejected_retained_publication = returned;
                commit_authority_contradiction = Some(match error {
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
                });
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
        let mut splice = splice_owner
            .take()
            .expect("fully prepared commit retains its sealed splice");
        splice.apply_claim_rebind(claim_rebind);
        if let Some(rebase) = root_rebase {
            splice.apply_root_rebase(rebase);
        }
        let outcome = splice.apply();
        let (mut active, leaving, retained_branch) = outcome.into_active_leaving_and_retained();
        active.consume_native_focus_owners();
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
        let next_active = (!active.frames().is_empty()).then_some(active);
        let next_cleanup = Some(if let Some(assembly) = leaving_assembly {
            CleanupLedger::activate_leaving_forest(
                cleanup_ledger,
                assembly.assemble(leaving, retired_payloads),
            )
        } else {
            CleanupLedger::activate_leaving_branch(
                cleanup_ledger,
                (!leaving.frames().is_empty() || !leaving.child_slots().is_empty())
                    .then_some(leaving),
            )
        });
        let next_retained = retained_branch.map(|branch| {
            (
                retained_projection.expect("retained splice has the same Core projection"),
                branch,
            )
        });
        let retained_owner = &mut entry.retained;
        let active_owner = &mut entry.active;
        let cleanup_owner = &mut entry.cleanup_ledger;
        let scroll_owner = &mut entry.scroll;
        engine.publish(|owner| {
            presentation_plan.apply_staged(
                |_| focus_plan.apply(),
                |_| {
                    if registry_changes {
                        registry_plan.apply(&mut registry);
                    }
                    if let Some(boundaries) = retained_boundaries {
                        boundaries.apply(&mut registry);
                    }
                },
                |_| gate_plan.apply(),
            );
            *active_owner = next_active;
            *cleanup_owner = next_cleanup;
            *scroll_owner = scroll.state;
            if let Some((projection, branch)) = next_retained {
                retained_owner.install_published(projection, owner, branch);
            }
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
            let Some(()) = committed else {
                if let Some(sealed) = sealed.take()
                    && entry
                        .engine
                        .cancel_prepared_commit_for_authority_settlement(sealed.prepared)
                        .is_err()
                {
                    commit_authority_contradiction = Some(BeginAuthoritySettlementErrorKind::Stale);
                }
                if let Some(prepared) = rejected_retained_publication.take() {
                    if let Ok(owners) = entry
                        .engine
                        .cancel_prepared_retained_commit_for_authority_settlement(prepared)
                    {
                        assert!(
                            entry.retained.restore_owners(owners).is_ok(),
                            "returned original owners have exact untouched holders"
                        );
                    }
                }
                let splice = splice_owner
                    .take()
                    .expect("failed commit retains its sealed splice owner");
                let (active, entering, _roles) = splice.reject().into_parts();
                pending.branch = entering;
                if let Some(kind) = commit_authority_contradiction {
                    if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                        world,
                        entry,
                        active,
                        pending,
                        AuthorityProtocolResidualKind::Authority(kind),
                    ) {
                        entry.active = had_active.then_some(active);
                        entry.pending_commit = Some(pending);
                    }
                    return;
                }
                if let Some(kind) = cleanup_reserve_contradiction {
                    if let Err((active, pending)) = fail_closed_normal_owner_protocol(
                        world,
                        entry,
                        active,
                        pending,
                        AuthorityProtocolResidualKind::CleanupLedger(kind),
                    ) {
                        entry.active = had_active.then_some(active);
                        entry.pending_commit = Some(pending);
                    }
                    return;
                }
                entry.active = had_active.then_some(active);
                if pending.ticket.is_some() || pending.final_task.is_some() {
                    entry.pending_commit = Some(pending);
                }
                return;
            };
        },
    );
}
