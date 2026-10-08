use super::*;
use crate::prelude::*;

fn engine(capacity: usize, opt_in: bool) -> RouterEngine {
    engine_with_lease(capacity, opt_in).0
}

fn engine_with_lease(capacity: usize, opt_in: bool) -> (RouterEngine, HostLease) {
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/:section").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let parent = builder.root_id().unwrap();
    let leaf = builder
        .route_with_options(
            parent,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("leaf").unwrap())
                .with_keep_alive(opt_in),
        )
        .unwrap();
    builder
        .route_with_options(
            leaf,
            RoutePattern::path("detail").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("detail").unwrap()),
        )
        .unwrap();
    let mut engine = RouterEngine::install(
        builder.build().unwrap(),
        RouteViewKey::new("fallback").unwrap(),
    )
    .unwrap()
    .with_keep_alive_capacity(capacity);
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
    engine.record_top_slot_registered(&grant).unwrap();
    let lease = engine.confirm_host(grant);
    (engine, lease)
}

fn ticket(engine: &mut RouterEngine, path: &str) -> CommitTicket {
    engine.push(RouteLocation::new(path)).unwrap();
    engine.collect();
    loop {
        match engine.action().expect("real navigation action") {
            EngineAction::FrameTask(task) => {
                let outlets = engine
                    .expected_child_outlet(&task)
                    .unwrap()
                    .into_iter()
                    .collect();
                engine.report_applied_with_outlets(task, outlets).unwrap();
            }
            EngineAction::MountRequest(mount) => {
                let receipt = engine.resolve_mount(mount).unwrap().attest_receipt(0x61);
                let completion = engine.accept_mount_receipt(receipt).unwrap().complete(0x63);
                engine.complete_mount_registration(completion).unwrap();
            }
            EngineAction::Commit(ticket) => return ticket,
            _ => panic!("normal route must reach commit"),
        }
    }
}

fn prepare(
    engine: &mut RouterEngine,
    ticket: &CommitTicket,
    owners: &mut Vec<RetainedLeafOwner>,
) -> PreparedRetainedCommit {
    let inventory = owners.iter().collect::<Vec<_>>();
    let plan = engine
        .prepare_retained_commit_plan(ticket, &inventory)
        .unwrap();
    let mut retiring = Vec::new();
    let mut preserved = Vec::new();
    for owner in std::mem::take(owners) {
        if plan
            .retiring()
            .iter()
            .any(|projection| projection.frame() == owner.projection().frame())
        {
            retiring.push(owner);
        } else {
            preserved.push(owner);
        }
    }
    *owners = preserved;
    let roots = plan.disposable_roots().to_vec();
    let count = roots.len();
    let manifest = CleanupManifest::new(roots, 0x6163, count);
    match engine.prepare_commit_with_retention(ticket, plan, manifest, retiring, None) {
        Ok(prepared) => prepared,
        Err(_) => panic!("exact retention partition must prepare"),
    }
}

fn publish(
    engine: &mut RouterEngine,
    prepared: PreparedRetainedCommit,
    owners: &mut Vec<RetainedLeafOwner>,
) {
    let cleanup = prepared.cleanup().clone();
    let count = cleanup.obligation_count();
    let validated = match engine.validate_retained_publication(prepared) {
        Ok(validated) => validated,
        Err(_) => panic!("exact retained publication must validate"),
    };
    validated.publish(|owner| owners.extend(owner));
    let completion = if count == 0 {
        cleanup.complete_empty()
    } else {
        cleanup.complete(0x6567, count)
    };
    engine.complete_cleanup(completion).unwrap();
    assert!(
        engine.action().is_none(),
        "the prior request must not own the cache"
    );
}

fn navigate(engine: &mut RouterEngine, path: &str, owners: &mut Vec<RetainedLeafOwner>) {
    let ticket = ticket(engine, path);
    let prepared = prepare(engine, &ticket, owners);
    publish(engine, prepared, owners);
}

fn retained_hit_task(engine: &mut RouterEngine, path: &str) -> FrameTask {
    engine.push(RouteLocation::new(path)).unwrap();
    engine.collect();
    let EngineAction::FrameTask(task) = engine.action().expect("preserved parent dispatches leaf")
    else {
        panic!("the retained leaf must be selected before its factory runs")
    };
    task
}

fn back_task(
    engine: &mut RouterEngine,
    world: &HistoryAdmissionWorldSeal,
) -> (
    HistoryExecutionBindingView,
    LifecycleContinuationCapacity,
    FrameTask,
) {
    let stamp = engine.history_admission_stamp(world).unwrap();
    let admission = engine.prepare_back_admission(world, &stamp).unwrap();
    let (_, accepted) = engine.enqueue_back(admission).unwrap().into_parts();
    engine.collect();
    let Some(EngineAction::BackResolution(handoff)) = engine.action() else {
        panic!("a real Back must resolve its canonical predecessor")
    };
    let verified = match engine.verify_back_resolution_handoff(accepted, handoff) {
        BackResolutionVerifyOutcome::Matched(verified) => verified,
        BackResolutionVerifyOutcome::Mismatch { .. } => panic!("exact Back resolution"),
    };
    let (_, owner) = verified.into_parts();
    let VerifiedBackResolutionOwnerHandoff::Target(owner) = owner else {
        panic!("two entries have a Back target")
    };
    let resume = match engine.resume_back_target_with_retirement(owner, || {}) {
        BackTargetCoreResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
        _ => panic!("the exact Back target must resume"),
    };
    let (action, history, capacity) = resume.into_parts();
    let BackTargetResumeAction::FrameTask(task) = action else {
        panic!("a changed leaf dispatches its original fresh task")
    };
    (history, capacity, task)
}

#[test]
fn retained_back_publication_preserves_original_continuation_for_hits_misses_and_cancel() {
    for capacity in [0, 1] {
        let mut engine = engine(capacity, true);
        let world = engine.claim_history_admission_world().unwrap();
        let mut owners = Vec::new();
        navigate(&mut engine, "/items/1", &mut owners);
        let first = engine.active().frames().last().unwrap().clone();
        let first_entry = engine.canonical_history().current_entry().unwrap();
        navigate(&mut engine, "/items/2", &mut owners);
        let before = engine.history().clone();
        let (history, continuation, task) = back_task(&mut engine, &world);
        let plan = engine
            .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
            .unwrap();
        let (prepared, continuation) = if let Some(plan) = plan {
            // A foreign original continuation rejects before task/cache transfer.
            let mut foreign = self::engine(0, true);
            let foreign_world = foreign.claim_history_admission_world().unwrap();
            let mut foreign_owners = Vec::new();
            navigate(&mut foreign, "/items/1", &mut foreign_owners);
            navigate(&mut foreign, "/items/2", &mut foreign_owners);
            let (foreign_history, foreign_continuation, _) =
                back_task(&mut foreign, &foreign_world);
            let (task, plan, manifest, returned, error) = engine
                .prepare_retained_hit_commit(
                    task,
                    plan,
                    CleanupManifest::empty(),
                    owners,
                    Some((&foreign_history, &foreign_continuation)),
                )
                .unwrap_err()
                .into_parts();
            assert!(matches!(
                error,
                RetainedPublicationError::BackTargetMismatch(_)
            ));
            assert_eq!(engine.history(), &before);
            let prepared = engine
                .prepare_retained_hit_commit(
                    task,
                    plan,
                    manifest,
                    returned,
                    Some((&history, &continuation)),
                )
                .unwrap();
            // The sealed owner cannot use ordinary publication to skip the cap.
            let (prepared, _) = match engine.validate_retained_publication(prepared) {
                Err(returned) => returned,
                Ok(_) => panic!("Back publication requires its original continuation"),
            };
            let (prepared, foreign_continuation, error) = match engine
                .validate_retained_publication_for_back(prepared, foreign_continuation)
            {
                Err(returned) => returned,
                Ok(_) => panic!("foreign capacity must not consume the real prepared owner"),
            };
            assert!(matches!(
                error,
                RetainedPublicationError::BackTargetMismatch(_)
            ));
            drop(foreign_continuation);
            (prepared, continuation)
        } else {
            let outlets = engine
                .expected_child_outlet(&task)
                .unwrap()
                .into_iter()
                .collect();
            let (ticket, continuation) = match engine.report_back_target_applied(
                &history,
                continuation,
                task,
                BackTargetAppliedReport::WithOutlets(outlets),
            ) {
                BackTargetAppliedReportOutcome::Commit { ticket, capacity } => (ticket, capacity),
                _ => panic!("the fresh Back leaf must reach its original commit ticket"),
            };
            let plan = engine.prepare_retained_commit_plan(&ticket, &[]).unwrap();
            let roots = plan.disposable_roots().to_vec();
            let count = roots.len();
            let prepared = engine
                .prepare_commit_with_retention(
                    &ticket,
                    plan,
                    CleanupManifest::new(roots, 0x6261, count),
                    Vec::new(),
                    Some((&history, &continuation)),
                )
                .unwrap();
            (prepared, continuation)
        };
        assert_eq!(engine.history(), &before);
        assert!(prepared.history_publication().restores_back());
        let cleanup = prepared.cleanup().clone();
        let count = cleanup.obligation_count();
        owners = Vec::new();
        engine
            .validate_retained_publication_for_back(prepared, continuation)
            .unwrap_or_else(|_| panic!("the original Back continuation must validate"))
            .publish(|owner| owners.extend(owner));
        engine
            .complete_cleanup(if count == 0 {
                cleanup.complete_empty()
            } else {
                cleanup.complete(0x6263, count)
            })
            .unwrap();
        assert_eq!(
            engine.canonical_history().current_entry(),
            Some(first_entry)
        );
        if capacity == 0 {
            assert_ne!(engine.active().frames().last(), Some(&first));
        } else {
            assert_eq!(engine.active().frames().last(), Some(&first));
            // Cancellation returns the original cache owner; the borrowed Back
            // capacity remains available in the physical authority holder.
            navigate(&mut engine, "/items/3", &mut owners);
            let before = engine.history().clone();
            let (history, continuation, task) = back_task(&mut engine, &world);
            let plan = engine
                .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
                .unwrap()
                .unwrap();
            let cached = plan.hit().clone();
            let prepared = engine
                .prepare_retained_hit_commit(
                    task,
                    plan,
                    CleanupManifest::empty(),
                    owners,
                    Some((&history, &continuation)),
                )
                .unwrap();
            let returned = engine
                .cancel_prepared_retained_commit_for_authority_settlement(prepared)
                .unwrap_or_else(|_| panic!("exact cancellation must return original cache owners"));
            assert_eq!(returned.len(), 1);
            assert_eq!(returned[0].projection(), &cached);
            assert_eq!(engine.history(), &before);
            assert!(continuation.inner.scope.matches_execution(&history.inner));
        }
    }
}

#[test]
fn retained_hit_reuses_original_frames_and_empty_slots_with_fresh_history_entries() {
    let mut engine = engine(1, true);
    let mut owners = Vec::new();
    // Leaving a real child produces a Core-owned empty slot on the cached leaf.
    navigate(&mut engine, "/items/1/detail", &mut owners);
    navigate(&mut engine, "/items/1", &mut owners);
    let first = engine.active().frames().last().unwrap().clone();
    let first_entry = engine.canonical_history().current_entry().unwrap();
    navigate(&mut engine, "/items/2", &mut owners);
    let second = engine.active().frames().last().unwrap().clone();
    assert_eq!(owners[0].projection().empty_child_slots().count(), 1);

    for (path, expected, departing) in
        [("/items/1", &first, &second), ("/items/2", &second, &first)]
    {
        let task = retained_hit_task(&mut engine, path);
        assert_ne!(
            task.frame(),
            expected,
            "the dispatched task is a fresh attempt"
        );
        let plan = engine
            .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
            .unwrap()
            .expect("the same exact parent has the requested retained leaf");
        assert_eq!(plan.hit().frame(), expected);
        assert_eq!(plan.target_frame(), expected);
        assert!(plan.commit_plan().disposable_roots().is_empty());
        assert!(plan.commit_plan().retiring().is_empty());
        assert_eq!(plan.commit_plan().retained().unwrap().frame(), departing);
        let prepared = engine
            .prepare_retained_hit_commit(
                task,
                plan,
                CleanupManifest::empty(),
                std::mem::take(&mut owners),
                None,
            )
            .unwrap();
        assert_eq!(
            prepared
                .history_publication()
                .target_frames()
                .last()
                .unwrap()
                .frame(),
            expected
        );
        publish(&mut engine, prepared, &mut owners);
        assert_eq!(engine.active().frames().last(), Some(expected));
        assert_eq!(owners.len(), 1, "the active hit releases one capacity slot");
        assert_eq!(owners[0].projection().frame(), departing);
        assert_ne!(
            engine.canonical_history().current_entry(),
            Some(first_entry)
        );
    }
    assert_eq!(owners[0].projection().empty_child_slots().count(), 1);
}

#[test]
fn retained_redeparture_preserves_original_identity_without_reordering_lru() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    for path in ["/items/1", "/items/2", "/items/3"] {
        navigate(&mut engine, path, &mut owners);
    }
    let original = owners[0].projection().clone();
    let oldest_remaining = owners[1].projection().clone();
    let third = engine.active().frames().last().unwrap().clone();

    // Restore the oldest physical instance without creating a new identity.
    owners.reverse();
    let task = retained_hit_task(&mut engine, "/items/1");
    let plan = engine
        .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
        .unwrap()
        .unwrap();
    assert_eq!(plan.hit(), &original);
    let hit = owners.remove(1);
    let prepared = engine
        .prepare_retained_hit_commit(task, plan, CleanupManifest::empty(), vec![hit], None)
        .unwrap();
    publish(&mut engine, prepared, &mut owners);
    assert_eq!(engine.active().frames().last(), Some(original.frame()));

    // Its next departure is newest in LRU, even though its frame is oldest.
    let next = ticket(&mut engine, "/items/4");
    let plan = engine
        .prepare_retained_commit_plan(&next, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    assert_eq!(plan.retiring(), &[oldest_remaining]);
    assert_eq!(plan.retained().unwrap().frame(), original.frame());
    assert_eq!(plan.retained().unwrap().host(), original.host());
    let prepared = prepare(&mut engine, &next, &mut owners);
    publish(&mut engine, prepared, &mut owners);

    // Inventory order is now [third, original], unlike physical creation order.
    // Arbitrary owner order must still validate and evict the third, not original.
    owners.reverse();
    let next = ticket(&mut engine, "/items/5");
    let plan = engine
        .prepare_retained_commit_plan(&next, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    assert_eq!(plan.retiring().len(), 1);
    assert_eq!(plan.retiring()[0].frame(), &third);
    let prepared = prepare(&mut engine, &next, &mut owners);
    publish(&mut engine, prepared, &mut owners);
    assert!(
        owners
            .iter()
            .any(|owner| owner.projection().frame() == original.frame()
                && owner.projection().host() == original.host())
    );
}

#[test]
fn retained_hit_source_rejection_returns_original_task_and_owners_without_history_change() {
    let mut engine = engine(1, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    navigate(&mut engine, "/items/2", &mut owners);
    let original = owners[0].projection().clone();
    let history = engine.history().clone();
    let task = retained_hit_task(&mut engine, "/items/1");
    let fresh_frame = task.frame().clone();
    let request = task.request();
    let plan = engine
        .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
        .unwrap()
        .unwrap();
    engine = engine.with_keep_alive_capacity(0);
    let (task, plan, manifest, returned, error) = engine
        .prepare_retained_hit_commit(task, plan, CleanupManifest::empty(), owners, None)
        .unwrap_err()
        .into_parts();
    assert_eq!(
        error,
        RetainedPublicationError::Integration(IntegrationError::Stale)
    );
    assert_eq!(task.request(), request);
    assert_eq!(task.frame(), &fresh_frame);
    assert_eq!(returned[0].projection(), &original);
    assert_eq!(engine.history(), &history);
    engine.inspect_report_identity(&task).unwrap();
    assert_eq!(engine.active().location().unwrap().path(), "/items/2");

    engine = engine.with_keep_alive_capacity(1);
    let prepared = engine
        .prepare_retained_hit_commit(task, plan, manifest, returned, None)
        .unwrap();
    owners = Vec::new();
    publish(&mut engine, prepared, &mut owners);
    assert_eq!(engine.active().frames().last(), Some(original.frame()));
}

#[test]
fn retained_miss_cleanup_returns_the_original_fresh_task_after_exact_completion() {
    let (mut engine, lease) = engine_with_lease(1, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    let cached = engine.active().frames().last().unwrap().clone();
    navigate(&mut engine, "/items/2", &mut owners);
    let active = engine.active().clone();
    let history = engine.history().clone();
    let task = retained_hit_task(&mut engine, "/items/1");
    let original_identity = task.report_identity();
    let original_frame = task.frame().clone();
    let request = task.request();
    let plan = engine
        .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
        .unwrap()
        .unwrap();
    assert_eq!(plan.hit().frame(), &cached);
    let mut installed = None;
    engine
        .begin_retained_miss_cleanup(
            task,
            plan,
            CleanupManifest::new(vec![cached], 0x7173, 1),
            std::mem::take(&mut owners),
            |owner| installed = Some(owner),
        )
        .unwrap();
    let mut owner = installed.unwrap();
    let batch = owner.cleanup().clone();
    assert_eq!(engine.active(), &active);
    assert_eq!(engine.history(), &history);
    assert!(engine.action().is_none());
    assert_eq!(
        engine.complete_cleanup(batch.clone().complete(0x7577, 1)),
        Err(IntegrationError::Stale),
        "ordinary completion must still reject Applying"
    );
    let lease = engine
        .begin_authority_settlement_for_cause_retaining(
            lease,
            PreCommitAuthorityCause::HostLost,
            CleanupManifest::empty(),
            |_| panic!("the live cache cleanup owner must not be bypassed"),
        )
        .err()
        .expect("authority settlement must retain the cache-only responsibility")
        .into_owner();
    let mut protocol_callback_ran = false;
    let lease = engine
        .fail_closed_authority_protocol_retaining(Some(lease), || protocol_callback_ran = true)
        .err()
        .expect("protocol termination must retain the cache-only responsibility")
        .expect("the original host lease must be returned");
    assert!(!protocol_callback_ran);
    engine
        .observe_retained_miss_cleanup_receipt(&mut owner, CleanupDisposition::AlreadyAbsent)
        .unwrap();
    engine = engine.with_keep_alive_capacity(0);
    let (owner, completion, physical, error) = engine
        .complete_retained_miss_cleanup(owner, batch.complete(0x7577, 1), 73, |_| {
            panic!("source drift must return the original owner and physical payload")
        })
        .err()
        .unwrap()
        .into_parts();
    assert_eq!(error, IntegrationError::Stale);
    assert_eq!(physical, 73);
    assert_eq!(engine.active(), &active);
    assert_eq!(engine.history(), &history);
    engine = engine.with_keep_alive_capacity(1);
    let mut reclaimed = None;
    let task = engine
        .complete_retained_miss_cleanup(owner, completion, physical, |value| {
            reclaimed = Some(value)
        })
        .unwrap();
    assert_eq!(reclaimed, Some(73));
    assert!(task.report_identity() == original_identity);
    assert_eq!(task.frame(), &original_frame);
    assert_eq!(engine.active(), &active);
    assert_eq!(engine.history(), &history);
    assert!(
        engine
            .prepare_retained_hit_plan(&task, &[])
            .unwrap()
            .is_none()
    );
    let outlets = engine
        .expected_child_outlet(&task)
        .unwrap()
        .into_iter()
        .collect();
    engine.report_applied_with_outlets(task, outlets).unwrap();
    let EngineAction::Commit(ticket) = engine.action().unwrap() else {
        panic!("the original fresh task must continue to normal publication");
    };
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    publish(&mut engine, prepared, &mut owners);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(engine.active().frames().last(), Some(&original_frame));
    assert_eq!(owners.len(), 1);
    assert_eq!(
        owners[0].projection().frame(),
        active.frames().last().unwrap()
    );
    drop(lease);
}

#[test]
fn retained_leaf_defaults_and_capacity_are_real_policy_inputs() {
    assert!(!RouteNodeOptions::new().keep_alive());
    for (capacity, opt_in) in [(0, true), (2, false)] {
        let mut engine = engine(capacity, opt_in);
        let mut owners = Vec::new();
        navigate(&mut engine, "/items/1", &mut owners);
        navigate(&mut engine, "/items/2", &mut owners);
        assert!(owners.is_empty());
    }
}

#[test]
fn retained_leaf_two_transfers_terminate_old_requests_and_parent_loss_disposes_exact_owners() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    let first = engine.active().frames().last().unwrap().clone();
    navigate(&mut engine, "/items/2", &mut owners);
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].projection().frame(), &first);
    navigate(&mut engine, "/items/3", &mut owners);
    assert_eq!(owners.len(), 2);
    assert_ne!(
        owners[0].projection().frame(),
        owners[1].projection().frame()
    );
    assert_eq!(
        owners[0].projection().parent(),
        owners[1].projection().parent()
    );
    assert_eq!(owners[0].projection().slot(), owners[1].projection().slot());
    assert_eq!(owners[0].projection().host(), owners[1].projection().host());
    let ticket = ticket(&mut engine, "/outside");
    let plan = engine
        .prepare_retained_commit_plan(&ticket, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    assert!(plan.retained().is_none());
    assert_eq!(plan.retiring().len(), 2);
    for owner in &owners {
        assert!(plan.disposable_roots().contains(owner.projection().frame()));
    }
    drop(plan);
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    assert!(owners.is_empty());
    publish(&mut engine, prepared, &mut owners);
    assert!(owners.is_empty());
    navigate(&mut engine, "/other/1", &mut owners);
    navigate(&mut engine, "/other/2", &mut owners);
    assert_eq!(
        owners.len(),
        1,
        "completed disposal releases the real capacity"
    );
}

#[test]
fn retained_leaf_lru_eviction_transfers_original_owner_to_its_cleanup_token() {
    let mut engine = engine(1, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    let first = engine.active().frames().last().unwrap().clone();
    navigate(&mut engine, "/items/2", &mut owners);
    let second = engine.active().frames().last().unwrap().clone();
    let evicted = owners[0].projection().clone();
    assert_eq!(evicted.frame(), &first);

    let third_ticket = ticket(&mut engine, "/items/3");
    let plan = engine
        .prepare_retained_commit_plan(&third_ticket, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    assert_eq!(plan.retiring(), std::slice::from_ref(&evicted));
    assert!(plan.preserved().is_empty());
    assert_eq!(plan.retained().unwrap().frame(), &second);
    assert_eq!(plan.disposable_roots(), std::slice::from_ref(&first));
    let manifest = CleanupManifest::new(plan.disposable_roots().to_vec(), 0x6163, 1);
    let prepared = engine
        .prepare_commit_with_retention(
            &third_ticket,
            plan,
            manifest,
            std::mem::take(&mut owners),
            None,
        )
        .unwrap();
    assert_eq!(
        engine.inner.testing_retained_transfer_owners(),
        vec![evicted]
    );
    let cleanup = prepared.cleanup().clone();
    assert_eq!(cleanup.obligation_count(), 1);
    let validated = match engine.validate_retained_publication(prepared) {
        Ok(validated) => validated,
        Err(_) => panic!("the exact LRU owner transfer must validate"),
    };
    validated.publish(|owner| owners.extend(owner));
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].projection().frame(), &second);
    engine
        .complete_cleanup(cleanup.complete(0x6567, 1))
        .unwrap();
    assert!(engine.action().is_none());

    // Completing that original token must remove its retirement metadata, so
    // the sole surviving owner can form the next complete canonical inventory.
    let third = engine.active().frames().last().unwrap().clone();
    let fourth_ticket = ticket(&mut engine, "/items/4");
    let plan = engine
        .prepare_retained_commit_plan(&fourth_ticket, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    assert_eq!(
        plan.retiring(),
        std::slice::from_ref(owners[0].projection())
    );
    assert_eq!(plan.retained().unwrap().frame(), &third);
    drop(plan);
    let prepared = prepare(&mut engine, &fourth_ticket, &mut owners);
    publish(&mut engine, prepared, &mut owners);
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].projection().frame(), &third);
}

#[test]
fn retained_leaf_inventory_and_disposable_roots_are_exact() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    navigate(&mut engine, "/items/2", &mut owners);
    let ticket = ticket(&mut engine, "/outside");
    assert!(engine.prepare_retained_commit_plan(&ticket, &[]).is_err());
    assert!(
        engine
            .prepare_retained_commit_plan(&ticket, &[&owners[0], &owners[0]])
            .is_err()
    );
    assert!(
        engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .is_err()
    );
    let plan = engine
        .prepare_retained_commit_plan(&ticket, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let expected = plan.disposable_roots().to_vec();
    let returned = match engine.prepare_commit_with_retention(
        &ticket,
        plan,
        CleanupManifest::empty(),
        std::mem::take(&mut owners),
        None,
    ) {
        Err(error) => error.into_parts(),
        Ok(_) => panic!("retiring roots cannot be omitted"),
    };
    let (plan, _, returned_owners, error) = returned;
    assert_eq!(
        error,
        RetainedPublicationError::Integration(IntegrationError::InvalidManifest)
    );
    assert_eq!(returned_owners.len(), 1);
    assert_eq!(plan.disposable_roots(), expected);
    owners = returned_owners;
    drop(plan);
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    publish(&mut engine, prepared, &mut owners);
}

#[test]
fn retained_leaf_foreign_router_owner_cannot_form_inventory() {
    let mut first = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut first, "/items/1", &mut owners);
    navigate(&mut first, "/items/2", &mut owners);
    let mut second = engine(2, true);
    let ticket = ticket(&mut second, "/items/1");
    assert!(
        second
            .prepare_retained_commit_plan(&ticket, &owners.iter().collect::<Vec<_>>())
            .is_err()
    );
}

#[test]
fn retained_leaf_cancel_returns_original_retiring_owner_and_never_publishes_new_history() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    navigate(&mut engine, "/items/2", &mut owners);
    let original = owners[0].projection().clone();
    let history = engine.history().clone();
    let ticket = ticket(&mut engine, "/outside");
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    let returned = match engine.cancel_prepared_retained_commit_for_authority_settlement(prepared) {
        Ok(returned) => returned,
        Err(_) => panic!("exact unpublished transfer must cancel"),
    };
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].projection(), &original);
    assert_eq!(engine.history(), &history);
}

#[test]
fn retained_leaf_abandoned_prepared_handle_preserves_the_unique_core_holder() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    navigate(&mut engine, "/items/2", &mut owners);
    let original = owners[0].projection().clone();
    let history = engine.history().clone();
    let ticket = ticket(&mut engine, "/outside");
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    drop(prepared);
    assert_eq!(engine.history(), &history);
    assert_eq!(
        engine.inner.testing_retained_transfer_owners(),
        vec![original]
    );
}

#[test]
fn retained_leaf_validation_rejection_returns_handle_and_preserves_original_owner() {
    let mut source = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut source, "/items/1", &mut owners);
    navigate(&mut source, "/items/2", &mut owners);
    let original = owners[0].projection().clone();
    let ticket = ticket(&mut source, "/outside");
    let prepared = prepare(&mut source, &ticket, &mut owners);
    let mut foreign = engine(2, true);
    let (prepared, _) = match foreign.validate_retained_publication(prepared) {
        Err(returned) => returned,
        Ok(_) => panic!("foreign receiver cannot validate this transfer"),
    };
    let returned = source
        .cancel_prepared_retained_commit_for_authority_settlement(prepared)
        .unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].projection(), &original);
}

#[test]
fn retained_leaf_abandoned_validated_handle_preserves_owners_without_history_publication() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    navigate(&mut engine, "/items/2", &mut owners);
    let original = owners[0].projection().clone();
    let history = engine.history().clone();
    let ticket = ticket(&mut engine, "/outside");
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    let validated = engine.validate_retained_publication(prepared).unwrap();
    drop(validated);
    assert_eq!(engine.history(), &history);
    assert_eq!(
        engine.inner.testing_retained_transfer_owners(),
        vec![original]
    );
}

#[test]
fn retained_leaf_another_publication_invalidates_the_full_old_plan_in_the_same_tick() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    let ticket = ticket(&mut engine, "/items/2");
    let old = engine.prepare_retained_commit_plan(&ticket, &[]).unwrap();
    let original = old.retained().unwrap().clone();
    let roots = old.disposable_roots().to_vec();
    let manifest = CleanupManifest::new(roots.clone(), 0x6163, roots.len());
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    publish(&mut engine, prepared, &mut owners);
    assert_eq!(owners[0].projection(), &original);
    let error = engine
        .prepare_commit_with_retention(&ticket, old, manifest, Vec::new(), None)
        .unwrap_err();
    let (_, _, returned, error) = error.into_parts();
    assert_eq!(
        error,
        RetainedPublicationError::Integration(IntegrationError::Stale)
    );
    assert!(returned.is_empty());
    assert_eq!(owners.len(), 1);
}

#[test]
fn retained_leaf_capacity_change_invalidates_an_unconsumed_plan() {
    let mut engine = engine(2, true);
    let mut owners = Vec::new();
    navigate(&mut engine, "/items/1", &mut owners);
    let ticket = ticket(&mut engine, "/items/2");
    let plan = engine.prepare_retained_commit_plan(&ticket, &[]).unwrap();
    let roots = plan.disposable_roots().to_vec();
    let manifest = CleanupManifest::new(roots.clone(), 0x6163, roots.len());
    engine = engine.with_keep_alive_capacity(0);
    let error = engine
        .prepare_commit_with_retention(&ticket, plan, manifest, Vec::new(), None)
        .unwrap_err();
    assert_eq!(
        error.into_parts().3,
        RetainedPublicationError::Integration(IntegrationError::Stale)
    );
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    assert!(prepared.retained_leaf().is_none());
    publish(&mut engine, prepared, &mut owners);
    assert!(owners.is_empty());
}

#[test]
fn retained_leaf_rejected_publication_cannot_bypass_inventory_and_keeps_original_owners() {
    let (mut engine, lease) = engine_with_lease(2, true);
    let mut owners = Vec::new();
    for path in ["/items/1", "/items/2", "/items/3"] {
        navigate(&mut engine, path, &mut owners);
    }
    let original = owners
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    assert_eq!(original.len(), 2);
    let history = engine.history().clone();
    let ticket = ticket(&mut engine, "/outside");
    let prepared = prepare(&mut engine, &ticket, &mut owners);
    assert!(owners.is_empty());

    // This real configuration input changes the frozen Core source. Validation
    // must retain a Rejected publication rather than publish its prepared history.
    engine = engine.with_keep_alive_capacity(1);
    let (prepared, error) = match engine.validate_retained_publication(prepared) {
        Err(returned) => returned,
        Ok(_) => panic!("same-source drift must reject the prepared publication"),
    };
    assert_eq!(error, IntegrationError::Stale);
    assert_eq!(engine.history(), &history);
    assert_eq!(engine.inner.testing_retained_transfer_owners(), original);
    assert!(matches!(
        engine.take_unpublished_retained_owners_for_settlement(),
        Err(IntegrationError::Stale)
    ));
    let (prepared, error) = engine
        .cancel_prepared_retained_commit_for_authority_settlement(prepared)
        .unwrap_err();
    assert_eq!(error, IntegrationError::Stale);
    assert_eq!(engine.inner.testing_retained_transfer_owners(), original);

    // The legacy authority entry cannot claim to settle a batch that omitted
    // these original owners. It must return the unchanged host lease.
    let roots = engine.authority_settlement_logical_roots();
    let count = roots.len();
    let error = match engine.begin_authority_settlement_retaining(
        lease,
        CleanupManifest::new(roots, 0x7163, count),
        |_| panic!("the legacy entry cannot bypass the transit owners"),
    ) {
        Err(error) => error,
        Ok(_) => panic!("transit owners require an explicit inventory transfer"),
    };
    assert_eq!(error.kind(), BeginAuthoritySettlementErrorKind::WrongPhase);
    assert_eq!(engine.inner.testing_retained_transfer_owners(), original);

    // This test explicitly chooses the existing permanent protocol sink. It
    // preserves unique ownership; it does not model a recoverable registration
    // loss or prove that the returned physical payloads have been disposed.
    engine
        .fail_closed_authority_protocol_retaining(Some(error.into_owner()), || {})
        .unwrap_or_else(|_| panic!("the original lease retires at the protocol sink"));
    assert_eq!(
        engine.snapshot().availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::RegistryInvariantViolation)
    );
    let returned = engine
        .take_unpublished_retained_owners_for_settlement()
        .unwrap();
    assert_eq!(
        returned
            .iter()
            .map(|owner| owner.projection().clone())
            .collect::<Vec<_>>(),
        original
    );
    assert!(engine.inner.testing_retained_transfer_owners().is_empty());
    assert_eq!(engine.history(), &history);
    assert!(matches!(
        engine.take_unpublished_retained_owners_for_settlement(),
        Err(IntegrationError::Stale)
    ));

    // Returning a unique owner does not settle its physical cleanup.
    drop((prepared, returned));
}

fn retained_idle_fixture() -> (RouterEngine, HostLease, Vec<RetainedLeafOwner>) {
    let (mut engine, lease) = engine_with_lease(2, true);
    let mut owners = Vec::new();
    for path in ["/items/1", "/items/2", "/items/3"] {
        navigate(&mut engine, path, &mut owners);
    }
    assert_eq!(owners.len(), 2);
    (engine, lease, owners)
}

#[test]
fn retained_failed_task_authority_preserves_owners_on_trigger_drift_and_settles_one_batch() {
    let (mut engine, lease, mut owners) = retained_idle_fixture();
    owners.reverse();
    let history = engine.history().clone();
    let keys = engine.canonical_history();
    let originals = owners
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    let task = retained_hit_task(&mut engine, "/items/4");
    let base_roots = vec![task.frame().clone()];
    let trigger = engine.failed_authority_trigger_for_task(&task).unwrap();
    let plan = engine
        .prepare_retained_failed_authority_settlement_plan(
            &lease,
            &trigger,
            &base_roots,
            &owners.iter().collect::<Vec<_>>(),
        )
        .unwrap();
    let mut roots = base_roots.clone();
    let mut canonical = originals.iter().collect::<Vec<_>>();
    canonical.sort_by(|left, right| left.compare_instance(right));
    roots.extend(
        canonical
            .iter()
            .map(|projection| projection.frame().clone()),
    );
    assert_eq!(plan.logical_roots(), roots);
    assert_eq!(plan.retiring().iter().collect::<Vec<_>>(), canonical);

    // A new real trigger changes the frozen owner inventory without changing
    // the failed task, active presentation, or cache payloads.
    let newer_trigger = engine.failed_authority_trigger_for_task(&task).unwrap();
    let manifest = CleanupManifest::new(roots.clone(), 0x8470, 5);
    let error = match engine.begin_failed_authority_settlement_with_retention(
        lease,
        newer_trigger,
        PreCommitAuthorityCause::HostLost,
        plan,
        manifest,
        owners,
        |_| panic!("a different trigger revision must not install cleanup"),
    ) {
        Err(error) => error,
        Ok(_) => panic!("the full frozen inventory must reject trigger drift"),
    };
    let (lease, plan, manifest, owners, kind) = error.into_parts();
    assert_eq!(kind, BeginAuthoritySettlementErrorKind::Stale);
    assert_eq!(plan.logical_roots(), roots);
    assert_eq!(
        owners
            .iter()
            .map(|owner| owner.projection())
            .collect::<Vec<_>>(),
        originals.iter().collect::<Vec<_>>()
    );
    assert_eq!(engine.history(), &history);
    engine.inspect_report_identity(&task).unwrap();
    drop((trigger, plan));

    let trigger = engine.failed_authority_trigger_for_task(&task).unwrap();
    let plan = engine
        .prepare_retained_failed_authority_settlement_plan(
            &lease,
            &trigger,
            &base_roots,
            &owners.iter().collect::<Vec<_>>(),
        )
        .unwrap();
    let mut installed = None;
    let cleanup = engine
        .begin_failed_authority_settlement_with_retention(
            lease,
            trigger,
            PreCommitAuthorityCause::HostLost,
            plan,
            manifest,
            owners,
            |batch| installed = Some(batch),
        )
        .unwrap();
    assert_eq!(installed.unwrap().inner, cleanup.inner);
    assert_eq!(engine.history(), &history);
    engine
        .complete_authority_cleanup(cleanup.complete(0x8471, 5), || {})
        .unwrap();
    assert_eq!(engine.history(), &history);
    assert_eq!(engine.canonical_history(), keys);
    assert!(engine.snapshot().running_request().is_none());
    drop(task);

    let lease = readmit_after_host_loss(&mut engine);
    let mut owners = Vec::new();
    navigate(&mut engine, "/other/1", &mut owners);
    navigate(&mut engine, "/other/2", &mut owners);
    assert_eq!(
        owners.len(),
        1,
        "the exact authority batch cleared the cache"
    );
    drop(lease);
}

#[test]
fn retained_foreground_host_loss_binds_original_owners_to_the_authority_cleanup() {
    let (mut engine, lease, owners) = retained_idle_fixture();
    let history = engine.history().clone();
    let keys = engine.canonical_history();
    let originals = owners
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    let task = retained_hit_task(&mut engine, "/items/1");
    let fresh_frame = task.frame().clone();
    let hit = engine
        .prepare_retained_hit_plan(&task, &owners.iter().collect::<Vec<_>>())
        .unwrap()
        .unwrap();
    assert!(
        engine
            .authority_settlement_logical_roots()
            .contains(&fresh_frame)
    );
    let error = match engine.begin_authority_settlement_for_cause_retaining(
        lease,
        PreCommitAuthorityCause::HostLost,
        CleanupManifest::new(engine.authority_settlement_logical_roots(), 0x8370, 3),
        |_| panic!("the old entry cannot bypass retained owner transfer"),
    ) {
        Err(error) => error,
        Ok(_) => panic!("live owners require an explicit inventory transfer"),
    };
    let lease = error.into_owner();
    let plan = engine
        .prepare_retained_authority_settlement_plan(&lease, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let mut roots = engine.authority_settlement_logical_roots();
    roots.extend(
        originals
            .iter()
            .map(|projection| projection.frame().clone()),
    );
    assert_eq!(plan.logical_roots(), roots);
    assert_eq!(plan.retiring(), originals);
    let manifest = CleanupManifest::new(roots.clone(), 0x8371, 5);

    // Changing a real source input must return every original capability before
    // the callback installs any physical authority-cleanup owner.
    engine = engine.with_keep_alive_capacity(1);
    let error = match engine.begin_authority_settlement_with_retention(
        lease,
        PreCommitAuthorityCause::HostLost,
        plan,
        manifest,
        owners,
        |_| panic!("source drift must not publish authority cleanup"),
    ) {
        Err(error) => error,
        Ok(_) => panic!("source drift must return all original inputs"),
    };
    let (lease, plan, manifest, owners, kind) = error.into_parts();
    assert_eq!(kind, BeginAuthoritySettlementErrorKind::Stale);
    assert_eq!(plan.logical_roots(), roots);
    assert_eq!(
        owners
            .iter()
            .map(|owner| owner.projection())
            .collect::<Vec<_>>(),
        originals.iter().collect::<Vec<_>>()
    );
    engine.inspect_report_identity(&task).unwrap();
    assert_eq!(engine.history(), &history);
    engine = engine.with_keep_alive_capacity(2);
    let mut installed = None;
    let cleanup = engine
        .begin_authority_settlement_with_retention(
            lease,
            PreCommitAuthorityCause::HostLost,
            plan,
            manifest,
            owners,
            |batch| installed = Some(batch),
        )
        .unwrap();
    assert_eq!(installed.unwrap().inner, cleanup.inner);
    assert_eq!(
        engine.snapshot().availability(),
        &RouterAvailability::RecoveringAuthority
    );
    assert_eq!(engine.history(), &history);
    engine
        .complete_authority_cleanup(cleanup.complete(0x8372, 5), || {})
        .unwrap();
    assert_eq!(engine.history(), &history);
    assert_eq!(engine.canonical_history(), keys);
    assert!(engine.snapshot().running_request().is_none());
    drop((task, hit));

    let lease = readmit_after_host_loss(&mut engine);
    let mut owners = Vec::new();
    navigate(&mut engine, "/other/1", &mut owners);
    navigate(&mut engine, "/other/2", &mut owners);
    assert_eq!(
        owners.len(),
        1,
        "the original authority token cleared the old inventory"
    );
    drop(lease);
}

#[test]
fn retained_idle_host_loss_transfers_original_owners_and_readmits_without_changing_history() {
    let (mut engine, lease, mut owners) = retained_idle_fixture();
    let history = engine.history().clone();
    let entry_keys = engine.canonical_history();
    let original = owners
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    assert!(engine.idle_host_loss_eligible());
    let current_roots = engine
        .active()
        .frames()
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    let (_, lease) = engine
        .begin_idle_host_loss(
            lease,
            &CleanupManifest::new(current_roots.clone(), 0x8170, 3),
            |_| panic!("the old entry must not bypass live retained owners"),
        )
        .unwrap_err();
    let plan = engine
        .prepare_retained_idle_host_loss_plan(&lease, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let mut roots = current_roots;
    roots.extend(original.iter().map(|projection| projection.frame().clone()));
    assert_eq!(plan.logical_roots(), roots);
    assert_eq!(plan.retiring(), original);
    let manifest = CleanupManifest::new(roots, 0x8170, 5);
    let mut owner = None;
    engine
        .begin_idle_host_loss_with_retention(
            lease,
            plan,
            &manifest,
            std::mem::take(&mut owners),
            |value| owner = Some(value),
        )
        .unwrap();
    assert!(!engine.idle_host_loss_eligible());
    let facts = IdleHostLossPhysicalFacts {
        manifest_digest: 0x8170,
        obligation_count: 5,
        disposition: IdleHostLossPhysicalDisposition::Settled {
            completion_digest: 0x8171,
            settled_count: 5,
        },
    };
    let terminal = engine
        .complete_idle_host_loss(owner.unwrap(), facts, |terminal| terminal)
        .unwrap_or_else(|_| panic!("the exact retained idle owner must complete"));
    assert_eq!(terminal.physical_facts(), facts);
    assert_eq!(engine.history(), &history);
    assert_eq!(engine.canonical_history(), entry_keys);
    assert!(engine.snapshot().running_request().is_none());

    let lease = readmit_after_host_loss(&mut engine);
    navigate(&mut engine, "/other/1", &mut owners);
    navigate(&mut engine, "/other/2", &mut owners);
    assert_eq!(
        owners.len(),
        1,
        "idle completion releases the retired host's capacity"
    );
    drop(lease);
}

fn readmit_after_host_loss(engine: &mut RouterEngine) -> HostLease {
    let HostAdmissionSourcePermitOutcome::Ineligible { source, .. } =
        engine.issue_host_admission_source_permit()
    else {
        panic!("settled loss waits for a changed candidate observation")
    };
    let observation = HostAdmissionUnavailableEvidenceProjection::host_lost(
        &source,
        Some(0x8172),
        0x8173,
        0x8174,
    );
    let HostAdmissionUnavailableCauseBindOutcome::Bound(cause) =
        engine.bind_host_admission_unavailable_cause(source, &observation, ())
    else {
        panic!("the actual HostLost source binds a changed candidate")
    };
    assert!(matches!(
        engine.complete_observed_host_admission_unavailable(cause, || {}),
        HostAdmissionUnavailableObservedOutcome::Published(_)
    ));
    let HostAdmissionSourcePermitOutcome::Issued(permit) =
        engine.issue_host_admission_source_permit()
    else {
        panic!("the changed observation permits admission")
    };
    let token = engine
        .request_source_bound_host_admission(permit)
        .ok()
        .unwrap();
    let grant = engine
        .grant_source_bound_host_admission(token)
        .ok()
        .unwrap();
    engine
        .record_source_bound_top_slot_registered(&grant)
        .ok()
        .unwrap();
    let HostConfirmationPreparationOutcome::Prepared(prepared) =
        engine.prepare_host_confirmation(bind_granted_host_admission_source(grant, ()))
    else {
        panic!("the new exact grant prepares confirmation")
    };
    let HostAdmissionConfirmationCoreOutcome::Ready {
        installed: lease, ..
    } = engine.confirm_host_admission_with_binding(prepared, |(), lease| lease)
    else {
        panic!("the new exact grant confirms one real host lease")
    };
    lease
}

#[test]
fn retained_idle_source_drift_returns_the_complete_original_responsibility() {
    let (mut engine, lease, owners) = retained_idle_fixture();
    let original = owners
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    let history = engine.history().clone();
    let plan = engine
        .prepare_retained_idle_host_loss_plan(&lease, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let roots = plan.logical_roots().to_vec();
    let manifest = CleanupManifest::new(roots.clone(), 0x8270, 5);
    engine = engine.with_keep_alive_capacity(1);
    let (lease, plan, owners, kind) = engine
        .begin_idle_host_loss_with_retention(lease, plan, &manifest, owners, |_| {
            panic!("source drift must not install a physical idle owner")
        })
        .unwrap_err()
        .into_parts();
    assert_eq!(kind, IdleHostLossErrorKind::ReceiverMismatch);
    assert_eq!(plan.logical_roots(), roots);
    assert_eq!(
        owners
            .iter()
            .map(|owner| owner.projection().clone())
            .collect::<Vec<_>>(),
        original
    );
    assert_eq!(engine.history(), &history);
    assert!(engine.idle_host_loss_eligible());
    drop(plan);
    let plan = engine
        .prepare_retained_idle_host_loss_plan(&lease, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let mut installed = None;
    engine
        .begin_idle_host_loss_with_retention(lease, plan, &manifest, owners, |owner| {
            installed = Some(owner)
        })
        .unwrap();
    let facts = IdleHostLossPhysicalFacts {
        manifest_digest: 0x8270,
        obligation_count: 5,
        disposition: IdleHostLossPhysicalDisposition::SafeResidual {
            observation_digest: 0x8271,
        },
    };
    engine = engine.with_keep_alive_capacity(2);
    let owner = engine
        .complete_idle_host_loss(installed.unwrap(), facts, |_| -> () {
            panic!("completion source drift must return the original idle owner")
        })
        .unwrap_err();
    assert_eq!(engine.history(), &history);
    engine = engine.with_keep_alive_capacity(1);
    let terminal = engine
        .complete_idle_host_loss(owner, facts, |terminal| terminal)
        .unwrap_or_else(|_| panic!("restored exact source must accept the original owner"));
    assert_eq!(terminal.physical_facts(), facts);
    assert_eq!(engine.history(), &history);
}
