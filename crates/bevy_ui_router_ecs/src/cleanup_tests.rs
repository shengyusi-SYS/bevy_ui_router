use super::*;
use crate::registry::{
    ComponentManifest, EntityLifecycle, ProvisionalOwner, QuarantineLeaseTable, RegistryOwner,
    RouteEntityRegistry,
};
use bevy_ecs::{component::Component, hierarchy::ChildOf};
use bevy_ui_router_core::{
    frame::RouteFrameVersion,
    integration::{CleanupManifest, EngineAction, RouterEngine},
    prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
    slot::RouteSlotKey,
};

#[derive(Component)]
struct ExternalParentDriftMarker;

#[derive(Component)]
struct ForeignBoundaryMarker;

#[test]
fn back_capacity_reclaimed_cell_advances_generation_and_rejects_aba() {
    let mut domains = BackCapacityDomainIssuer::new();
    let mut issuer = domains.issue().unwrap();
    let mut foreign_issuer = domains.issue().unwrap();

    let first = issuer.reserve().unwrap();
    let first_identity = first.ordinal_identity_for_test();
    let first_backing = issuer.backing_capacities_for_test();
    let first_evidence = match issuer.retire_unbound(first, EnqueueRejected::CoreTokenDrift) {
        BackCapacityRetireOutcome::Retired(evidence) => evidence,
        BackCapacityRetireOutcome::Rejected { .. } => {
            panic!("the exact outstanding owner must retire")
        }
    };
    assert_eq!(
        first_evidence.enqueue_rejection_for_test(),
        Some(&EnqueueRejected::CoreTokenDrift),
        "retirement evidence retains the original rejection"
    );
    assert_eq!(issuer.backing_capacities_for_test(), first_backing);
    assert_eq!(issuer.semantic_snapshot_for_test().cell_count(), 1);
    assert_eq!(
        issuer.semantic_snapshot_for_test().outstanding_count(),
        0,
        "exact retirement removes the outstanding entry"
    );

    let replay_before = issuer.semantic_snapshot_for_test();
    let replay_backing_before = issuer.backing_capacities_for_test();
    assert_eq!(
        issuer.classify_retirement_evidence(&first_evidence),
        Err(BackCapacityRetireError::ConsumedReplay),
        "retirement evidence is observational and cannot authorize a second retirement"
    );
    assert_eq!(issuer.semantic_snapshot_for_test(), replay_before);
    assert_eq!(issuer.backing_capacities_for_test(), replay_backing_before);

    let second = issuer.reserve().unwrap();
    let second_identity = second.ordinal_identity_for_test();
    assert_eq!(second_identity.0, first_identity.0);
    assert_eq!(second_identity.1, first_identity.1);
    assert_ne!(second_identity.2, first_identity.2, "owners are burned");
    assert_eq!(
        second_identity.3,
        first_identity.3.checked_add(1).unwrap(),
        "the reclaimed cell advances by the exact successor generation"
    );
    assert_eq!(issuer.backing_capacities_for_test(), first_backing);

    let stale_before = issuer.semantic_snapshot_for_test();
    let stale_backing_before = issuer.backing_capacities_for_test();
    assert_eq!(
        issuer.classify_retirement_evidence(&first_evidence),
        Err(BackCapacityRetireError::StaleGeneration),
        "the old generation must not authorize the reused ordinal"
    );
    assert_eq!(issuer.semantic_snapshot_for_test(), stale_before);
    assert_eq!(issuer.backing_capacities_for_test(), stale_backing_before);
    assert_eq!(second.ordinal_identity_for_test(), second_identity);

    let foreign = foreign_issuer.reserve().unwrap();
    let foreign_identity = foreign.ordinal_identity_for_test();
    let local_before_foreign = issuer.semantic_snapshot_for_test();
    let local_backing_before_foreign = issuer.backing_capacities_for_test();
    let foreign_before_rejection = foreign_issuer.semantic_snapshot_for_test();
    let foreign_backing_before_rejection = foreign_issuer.backing_capacities_for_test();
    let foreign = match issuer.retire_unbound(foreign, EnqueueRejected::CoreTokenDrift) {
        BackCapacityRetireOutcome::Rejected {
            owner,
            rejection,
            error,
        } => {
            assert_eq!(error, BackCapacityRetireError::ForeignDomain);
            assert_eq!(rejection, EnqueueRejected::CoreTokenDrift);
            owner
        }
        BackCapacityRetireOutcome::Retired(_) => {
            panic!("a foreign issuer must not retire the move-only owner")
        }
    };
    assert_eq!(foreign.ordinal_identity_for_test(), foreign_identity);
    assert_eq!(issuer.semantic_snapshot_for_test(), local_before_foreign);
    assert_eq!(
        issuer.backing_capacities_for_test(),
        local_backing_before_foreign
    );
    assert_eq!(
        foreign_issuer.semantic_snapshot_for_test(),
        foreign_before_rejection
    );
    assert_eq!(
        foreign_issuer.backing_capacities_for_test(),
        foreign_backing_before_rejection
    );
    assert!(matches!(
        foreign_issuer.retire_unbound(foreign, EnqueueRejected::CoreTokenDrift),
        BackCapacityRetireOutcome::Retired(_)
    ));

    issuer.set_next_new_cell_initial_generation_for_test(u64::MAX);
    let terminal = issuer.reserve().unwrap();
    let terminal_identity = terminal.ordinal_identity_for_test();
    assert_ne!(terminal_identity.1, second_identity.1);
    assert_eq!(terminal_identity.3, u64::MAX);
    let terminal_backing = issuer.backing_capacities_for_test();
    let terminal_evidence = match issuer.retire_unbound(terminal, EnqueueRejected::CoreTokenDrift) {
        BackCapacityRetireOutcome::Retired(evidence) => evidence,
        BackCapacityRetireOutcome::Rejected { .. } => {
            panic!("the exact terminal-generation owner must retire and seal its cell")
        }
    };
    assert_eq!(issuer.backing_capacities_for_test(), terminal_backing);
    assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 1);

    let terminal_replay_before = issuer.semantic_snapshot_for_test();
    let terminal_replay_backing_before = issuer.backing_capacities_for_test();
    assert_eq!(
        issuer.classify_retirement_evidence(&terminal_evidence),
        Err(BackCapacityRetireError::ConsumedReplay)
    );
    assert_eq!(issuer.semantic_snapshot_for_test(), terminal_replay_before);
    assert_eq!(
        issuer.backing_capacities_for_test(),
        terminal_replay_backing_before
    );

    let cells_before_successor = issuer.semantic_snapshot_for_test().cell_count();
    let successor = issuer.reserve().unwrap();
    let successor_identity = successor.ordinal_identity_for_test();
    assert_ne!(
        successor_identity.1, terminal_identity.1,
        "a no-successor cell is sealed and cannot be reused"
    );
    assert_eq!(
        issuer.semantic_snapshot_for_test().cell_count(),
        cells_before_successor + 1,
        "reserve must grow a different real cell"
    );
    assert!(matches!(
        issuer.retire_unbound(successor, EnqueueRejected::CoreTokenDrift),
        BackCapacityRetireOutcome::Retired(_)
    ));

    assert_eq!(
        second.ordinal_identity_for_test(),
        second_identity,
        "foreign, stale and replay probes must not alter the live owner"
    );
    assert!(matches!(
        issuer.retire_unbound(second, EnqueueRejected::CoreTokenDrift),
        BackCapacityRetireOutcome::Retired(_)
    ));
    assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 0);
}

#[test]
fn back_no_target_zero_completion_reclaim_rejection_is_zero_write() {
    let (issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
    let mut domains = BackCapacityDomainIssuer::new();
    let mut foreign = domains.issue().unwrap();
    foreign.set_domain_for_test(
        identity
            .0
            .checked_add(1)
            .expect("the test issuer domain has a foreign successor"),
    );
    let before = foreign.semantic_snapshot_for_test();
    let backing_before = foreign.backing_capacities_for_test();

    assert!(matches!(
        foreign.preflight_installed_back_capacity_reclaim(&prepared),
        Err(BackCapacityRetireError::ForeignDomain)
    ));
    assert_eq!(foreign.semantic_snapshot_for_test(), before);
    assert_eq!(foreign.backing_capacities_for_test(), backing_before);
    assert_eq!(prepared.capacity_identity_for_test(), identity);
    assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 0);
}

#[test]
fn back_no_target_zero_completion_preparation_rejection_returns_whole_ledger() {
    let (_issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
    let ledger = prepared.into_ledger();
    let (returned, evidence) = match ledger.prepare_back_no_target_zero_completion() {
        Err(failure) => failure.into_parts(),
        Ok(_) => panic!("a ledger without the dedicated batch must fail closed"),
    };
    assert_eq!(
        evidence.source_identity().capacity(),
        BackNoTargetInstalledCapacitySource::Present {
            domain: identity.0,
            cell: identity.1,
            owner: identity.2,
            generation: identity.3,
        }
    );
    assert_eq!(
        evidence.source_identity().ledger_domain(),
        returned.ledger_domain
    );
    assert_eq!(
        evidence.expected(),
        BackNoTargetZeroCompletionFacts {
            manifest_digest: canonical_zero_manifest_digest(),
            obligation_count: 0,
            completion_digest: canonical_zero_completion_digest(),
            settled_count: 0,
        }
    );
    assert_eq!(
        evidence.observed().dedicated_batch(),
        BackNoTargetDedicatedBatchObservation::Absent
    );
    assert_eq!(
        evidence.observed().completion_digest(),
        Some(canonical_zero_completion_digest())
    );
    let capacity = returned
        .back_no_target_capacity
        .as_ref()
        .expect("rejection returns the installed capacity lease");
    assert_eq!(
        (
            capacity.domain.0,
            capacity.cell.0,
            capacity.owner.0,
            capacity.generation.0,
        ),
        identity
    );
    assert!(returned.batch.is_none());
    assert!(returned.back_no_target_batch.is_none());
    assert!(returned.owner.is_none());
    assert!(returned.obligations.is_empty());
    assert!(returned.mount_parents.is_empty());
    assert!(returned.receipts.is_empty());
    assert_eq!(
        returned.completion_digest,
        Some(canonical_empty_completion_digest())
    );
}

#[test]
fn back_no_target_zero_completion_evidence_distinguishes_digest_and_batch_drift() {
    let (_issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
    let missing_batch = match prepared
        .into_ledger()
        .prepare_back_no_target_zero_completion()
    {
        Err(failure) => failure.evidence(),
        Ok(_) => panic!("missing dedicated batch commitment must reject"),
    };

    let (_issuer, prepared, _) = prepared_installed_back_zero_completion(1);
    let mut ledger = prepared.into_ledger();
    ledger.drift_back_no_target_completion_digest_for_test();
    let completion_drift = match ledger.prepare_back_no_target_zero_completion() {
        Err(failure) => failure.evidence(),
        Ok(_) => panic!("noncanonical stored completion digest must reject"),
    };
    let expected = missing_batch.expected();
    let exact_observed = BackNoTargetZeroCompletionObservedFacts {
        dedicated_batch: BackNoTargetDedicatedBatchObservation::Present {
            manifest_digest: expected.manifest_digest(),
            obligation_count: expected.obligation_count(),
            expected_completion_digest: expected.completion_digest(),
        },
        ..missing_batch.observed()
    };
    let exact = BackNoTargetZeroCompletionPreparationEvidence {
        source: missing_batch.source_identity(),
        expected,
        observed: exact_observed,
    };
    assert!(exact.is_exact());
    let batch_commitment_drift = BackNoTargetZeroCompletionPreparationEvidence {
        observed: BackNoTargetZeroCompletionObservedFacts {
            dedicated_batch: BackNoTargetDedicatedBatchObservation::Present {
                manifest_digest: expected.manifest_digest(),
                obligation_count: expected.obligation_count(),
                expected_completion_digest: expected.completion_digest() ^ 1,
            },
            ..exact_observed
        },
        ..exact
    };
    assert!(!batch_commitment_drift.is_exact());

    assert_eq!(
        missing_batch.source_identity().capacity(),
        BackNoTargetInstalledCapacitySource::Present {
            domain: identity.0,
            cell: identity.1,
            owner: identity.2,
            generation: identity.3,
        }
    );
    assert_eq!(
        missing_batch.observed().dedicated_batch(),
        BackNoTargetDedicatedBatchObservation::Absent
    );
    assert_eq!(
        missing_batch.observed().completion_digest(),
        Some(canonical_zero_completion_digest())
    );
    assert_eq!(
        completion_drift.observed().dedicated_batch(),
        BackNoTargetDedicatedBatchObservation::Absent
    );
    assert_eq!(
        completion_drift.observed().completion_digest(),
        Some(canonical_zero_completion_digest() ^ 1)
    );
    assert_eq!(
        batch_commitment_drift.observed().dedicated_batch(),
        BackNoTargetDedicatedBatchObservation::Present {
            manifest_digest: canonical_zero_manifest_digest(),
            obligation_count: 0,
            expected_completion_digest: canonical_zero_completion_digest() ^ 1,
        }
    );
    assert_eq!(
        batch_commitment_drift.observed().completion_digest(),
        Some(canonical_zero_completion_digest())
    );
    assert_ne!(batch_commitment_drift, completion_drift);
    assert_ne!(missing_batch, completion_drift);
}

#[test]
fn back_no_target_zero_completion_exact_reclaim_reuses_the_advanced_cell() {
    let (mut issuer, prepared, first_identity) = prepared_installed_back_zero_completion(1);
    let facts = prepared.facts();
    assert_eq!(facts.manifest_digest(), canonical_zero_manifest_digest());
    assert_eq!(facts.obligation_count(), 0);
    assert_eq!(
        facts.completion_digest(),
        canonical_empty_completion_digest()
    );
    assert_eq!(facts.settled_count(), 0);
    assert_eq!(
        prepared.source_identity().capacity(),
        BackNoTargetInstalledCapacitySource::Present {
            domain: first_identity.0,
            cell: first_identity.1,
            owner: first_identity.2,
            generation: first_identity.3,
        }
    );
    let cells_before = issuer.semantic_snapshot_for_test().cell_count();
    let guard = issuer
        .preflight_installed_back_capacity_reclaim(&prepared)
        .expect("the exact installed lease must preflight");
    guard.commit();

    assert_eq!(issuer.semantic_snapshot_for_test().outstanding_count(), 0);
    let reclaimed = issuer
        .cell_projection_for_test(first_identity)
        .expect("the reclaimed cell remains observable");
    assert_eq!(reclaimed.current_generation, first_identity.3 + 1);
    assert!(!reclaimed.original_owner_live);
    assert_eq!(
        reclaimed.disposition,
        BackCapacityCellDispositionForTest::ReusableFree
    );
    let successor = issuer.reserve().unwrap();
    let successor_identity = successor.ordinal_identity_for_test();
    assert_eq!(successor_identity.1, first_identity.1);
    assert_ne!(successor_identity.2, first_identity.2);
    assert_eq!(successor_identity.3, first_identity.3 + 1);
    assert_eq!(
        issuer.semantic_snapshot_for_test().cell_count(),
        cells_before
    );
}

#[test]
fn back_no_target_zero_completion_reclaim_replay_is_zero_write() {
    let (mut issuer, prepared, identity) = prepared_installed_back_zero_completion(1);
    issuer
        .preflight_installed_back_capacity_reclaim(&prepared)
        .expect("the exact installed lease must preflight")
        .commit();
    let before = issuer.semantic_snapshot_for_test();
    let backing_before = issuer.backing_capacities_for_test();

    assert!(matches!(
        issuer.preflight_installed_back_capacity_reclaim(&prepared),
        Err(BackCapacityRetireError::ConsumedReplay)
    ));
    assert_eq!(issuer.semantic_snapshot_for_test(), before);
    assert_eq!(issuer.backing_capacities_for_test(), backing_before);
    assert_eq!(prepared.capacity_identity_for_test(), identity);
}

#[test]
fn back_no_target_zero_completion_max_generation_permanently_retires_cell() {
    let (mut issuer, prepared, terminal_identity) =
        prepared_installed_back_zero_completion(u64::MAX);
    issuer
        .preflight_installed_back_capacity_reclaim(&prepared)
        .expect("the exact terminal-generation lease must preflight")
        .commit();
    let cells_before = issuer.semantic_snapshot_for_test().cell_count();
    let retired = issuer
        .cell_projection_for_test(terminal_identity)
        .expect("the permanently retired cell remains observable");
    assert_eq!(retired.current_generation, u64::MAX);
    assert!(!retired.original_owner_live);
    assert_eq!(
        retired.disposition,
        BackCapacityCellDispositionForTest::PermanentlyRetired
    );
    assert!(matches!(
        issuer.preflight_installed_back_capacity_reclaim(&prepared),
        Err(BackCapacityRetireError::ConsumedReplay)
    ));

    let successor = issuer.reserve().unwrap();
    let successor_identity = successor.ordinal_identity_for_test();
    assert_ne!(
        successor_identity.1, terminal_identity.1,
        "a no-successor installed lease cell must remain permanently retired"
    );
    assert_eq!(
        issuer.semantic_snapshot_for_test().cell_count(),
        cells_before + 1
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CleanupOwnerSnapshot {
    IdleHostAuthority {
        active_claims: Option<Vec<ExactRegistryClaim>>,
        host: Entity,
        top_slot: Entity,
        top_slot_claim: ExactRegistryClaim,
    },
    None,
    Leaving {
        claims: Vec<ExactRegistryClaim>,
    },
    FailedPrepared {
        claims: Vec<ExactRegistryClaim>,
    },
    AuthoritySettlement {
        active_claims: Vec<ExactRegistryClaim>,
        pending_claims: Vec<ExactRegistryClaim>,
        host: Entity,
        top_slot: Entity,
        top_slot_claim: ExactRegistryClaim,
    },
    BackNoTargetAuthority {
        active_claims: Vec<ExactRegistryClaim>,
        host: Entity,
        top_slot: Entity,
        top_slot_claim: ExactRegistryClaim,
    },
}

fn cleanup_owner_snapshot(owner: Option<&CleanupOwner>) -> CleanupOwnerSnapshot {
    let claims = |branch: &RealizedBranch| branch.claims().cloned().collect::<Vec<_>>();
    match owner {
        Some(CleanupOwner::IdleHostAuthority(owner)) => CleanupOwnerSnapshot::IdleHostAuthority {
            active_claims: owner.active.as_ref().map(&claims),
            host: owner.host.host,
            top_slot: owner.host.top_slot,
            top_slot_claim: owner.host.top_slot_claim.clone(),
        },
        None => CleanupOwnerSnapshot::None,
        Some(CleanupOwner::Leaving(branch)) => CleanupOwnerSnapshot::Leaving {
            claims: claims(branch),
        },
        Some(CleanupOwner::FailedPrepared(owner)) => CleanupOwnerSnapshot::FailedPrepared {
            claims: claims(owner.branch.realized()),
        },
        Some(CleanupOwner::AuthoritySettlement(owner)) => {
            CleanupOwnerSnapshot::AuthoritySettlement {
                active_claims: owner.active.as_ref().map(&claims).unwrap_or_default(),
                pending_claims: claims(owner.pending.branch().realized()),
                host: owner.host.host,
                top_slot: owner.host.top_slot,
                top_slot_claim: owner.host.top_slot_claim.clone(),
            }
        }
        Some(CleanupOwner::BackNoTargetAuthority(owner)) => {
            CleanupOwnerSnapshot::BackNoTargetAuthority {
                active_claims: claims(&owner.active),
                host: owner.host.host,
                top_slot: owner.host.top_slot,
                top_slot_claim: owner.host.top_slot_claim.clone(),
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupRejectionSnapshot {
    owner: CleanupOwnerSnapshot,
    receipts: Vec<Option<CleanupReceipt>>,
    next_attempt: u64,
    issued_attempt: Option<CleanupAttempt>,
    completion_digest: Option<u64>,
    quarantine_event_observed: bool,
    post_settlement_focus: Vec<PostSettlementFocusRoot>,
    authority_host_children: Option<Vec<Entity>>,
    back_no_target_authority_partition: Option<FrozenBackNoTargetAuthorityPartition>,
    quarantine_sequences: Option<(u64, u64, usize)>,
    entity_claim: Option<ExactRegistryClaim>,
    entity_seal: Option<RouteOwnershipSeal>,
    entity_quarantine_claim: Option<ExactRegistryClaim>,
    entity_unresolved_claim: Option<ExactRegistryClaim>,
    entity_manifest: Option<ComponentManifest>,
    entity_parent: Option<Entity>,
    entity_parent_children: Option<Vec<Entity>>,
    parent_claim: Option<ExactRegistryClaim>,
    parent_seal: Option<RouteOwnershipSeal>,
    parent_quarantine_claim: Option<ExactRegistryClaim>,
    parent_unresolved_claim: Option<ExactRegistryClaim>,
    parent_manifest: Option<ComponentManifest>,
    parent_parent: Option<Entity>,
    parent_parent_children: Option<Vec<Entity>>,
}

fn cleanup_rejection_snapshot(
    world: &World,
    ledger: &CleanupLedger,
    entity: Entity,
    parent: Entity,
) -> CleanupRejectionSnapshot {
    let physical = |entity| {
        let parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        (
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .map(|record| record.claim()),
            ComponentManifest::from_world(world, entity),
            parent,
            parent.map(|parent| {
                world
                    .get::<bevy_ecs::hierarchy::Children>(parent)
                    .map(|children| children.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default()
            }),
        )
    };
    let entity_physical = physical(entity);
    let parent_physical = physical(parent);
    let quarantine = world.get_resource::<QuarantineLeaseTable>();
    CleanupRejectionSnapshot {
        owner: cleanup_owner_snapshot(ledger.owner.as_ref()),
        receipts: ledger.receipts.clone(),
        next_attempt: ledger.next_attempt,
        issued_attempt: ledger.issued_attempt.clone(),
        completion_digest: ledger.completion_digest,
        quarantine_event_observed: ledger.quarantine_event_observed,
        post_settlement_focus: ledger.post_settlement_focus.clone(),
        authority_host_children: ledger.authority_host_children.clone(),
        back_no_target_authority_partition: ledger.back_no_target_authority_partition.clone(),
        quarantine_sequences: quarantine.map(|table| {
            (
                table.next_sequence_for_test(),
                table.next_residual_sequence_for_test(),
                table.leases().len(),
            )
        }),
        entity_claim: entity_physical.0,
        entity_seal: world.get::<RouteOwnershipSeal>(entity).copied(),
        entity_quarantine_claim: quarantine
            .and_then(|table| table.record(entity))
            .map(|record| record.claim()),
        entity_unresolved_claim: quarantine
            .and_then(|table| table.unresolved_record(entity))
            .map(|record| record.claim()),
        entity_manifest: entity_physical.1,
        entity_parent: entity_physical.2,
        entity_parent_children: entity_physical.3,
        parent_claim: parent_physical.0,
        parent_seal: world.get::<RouteOwnershipSeal>(parent).copied(),
        parent_quarantine_claim: quarantine
            .and_then(|table| table.record(parent))
            .map(|record| record.claim()),
        parent_unresolved_claim: quarantine
            .and_then(|table| table.unresolved_record(parent))
            .map(|record| record.claim()),
        parent_manifest: parent_physical.1,
        parent_parent: parent_physical.2,
        parent_parent_children: parent_physical.3,
    }
}

fn assert_cleanup_rejection_is_zero(
    world: &World,
    ledger: &CleanupLedger,
    entity: Entity,
    parent: Entity,
    before: &CleanupRejectionSnapshot,
) {
    assert_eq!(
        cleanup_rejection_snapshot(world, ledger, entity, parent),
        *before
    );
}

fn published_empty_batch() -> (RouterEngine, CleanupBatch) {
    let view = RouteViewKey::new("root").unwrap();
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut engine = RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
    engine.confirm_host(grant);
    engine.push(RouteLocation::new("/")).unwrap();
    engine.collect();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("root task expected")
    };
    let ticket = engine.report_applied(task).unwrap();
    let prepared = engine
        .prepare_commit(&ticket, CleanupManifest::empty())
        .unwrap();
    let batch = prepared.cleanup().clone();
    let validated = engine.validate_publication(prepared).unwrap();
    validated.publish(|| {});
    (engine, batch)
}

#[test]
fn replacement_mount_children_come_from_contiguous_owner_facts() {
    let mut world = World::new();
    let sibling = world.spawn_empty().id();
    let leaving = world.spawn_empty().id();
    let entering = world.spawn_empty().id();

    assert_eq!(
        replacement_mount_children(&[sibling], leaving, &[sibling, leaving], entering),
        Some((vec![sibling, leaving, entering], vec![sibling, entering]))
    );
    assert_eq!(
        replacement_mount_children(&[sibling], leaving, &[leaving, sibling], entering),
        None
    );
}

#[test]
fn failed_prepared_branch_manifest_orders_child_frame_before_slot_and_parent_frame() {
    let (app, branch, child_entities, child_slot, parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("an exact prepared branch must freeze one cleanup manifest");
    let entities = manifest.entities();
    let child_end = child_entities
        .iter()
        .map(|entity| {
            entities
                .iter()
                .position(|candidate| candidate == entity)
                .unwrap()
        })
        .max()
        .unwrap();
    let slot_index = entities
        .iter()
        .position(|entity| *entity == child_slot)
        .unwrap();
    let parent_start = parent_entities
        .iter()
        .map(|entity| {
            entities
                .iter()
                .position(|candidate| candidate == entity)
                .unwrap()
        })
        .min()
        .unwrap();

    assert!(child_end < slot_index);
    assert!(slot_index < parent_start);
    assert_eq!(
        manifest.len(),
        child_entities.len() + 1 + parent_entities.len()
    );
}

#[test]
fn failed_prepared_manifest_accepts_multiple_exact_slot_only_roots() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    while realized.pop_frame_for_test().is_some() {}
    let slot_entities = realized
        .child_slots()
        .iter()
        .map(MountedChildSlot::slot)
        .collect::<Vec<_>>();
    assert_eq!(slot_entities.len(), 2);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("multiple exact slot-only roots need no synthetic frames");

    assert_eq!(manifest.entities(), slot_entities);
}

#[test]
fn failed_prepared_sparse_external_frame_root_uses_exact_mount_slot_boundary() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let child = realized.pop_frame_for_test().unwrap();
    let child_entities = child
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let child = crate::branch_transaction::RealizedBranch::from_single_frame(child);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(child),
    );

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("an exact external mount-slot boundary proves one sparse frame root");

    assert_eq!(
        manifest.entities().into_iter().collect::<HashSet<_>>(),
        child_entities.into_iter().collect::<HashSet<_>>()
    );
}

#[test]
fn failed_prepared_sparse_retained_empty_slot_is_a_leaf() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let removed_child = realized.pop_frame_for_test().unwrap();
    let retained_slot = realized.child_slots().last().unwrap().slot();
    let parent_entities = realized
        .frames()
        .last()
        .unwrap()
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    drop(removed_child);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("a retained-empty slot needs no synthetic child frame");
    let entities = manifest.entities();
    let slot_index = entities
        .iter()
        .position(|entity| *entity == retained_slot)
        .unwrap();
    let parent_start = parent_entities
        .iter()
        .map(|entity| {
            entities
                .iter()
                .position(|candidate| candidate == entity)
                .unwrap()
        })
        .min()
        .unwrap();

    assert!(slot_index < parent_start);
}

#[test]
fn failed_prepared_sparse_multiple_frame_roots_are_permutation_stable() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    realized.clear_child_slots_for_test();
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );
    let first = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("each frame has an exact external sparse boundary");

    let mut realized = branch.into_realized_for_test();
    realized.reverse_frames_for_test();
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );
    let permuted = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("input permutation does not change exact sparse roots");

    assert_eq!(first.entities(), permuted.entities());
    assert_eq!(first.digest(), permuted.digest());
}

#[test]
fn failed_prepared_sparse_slot_permutation_preserves_order_and_digest() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    while realized.pop_frame_for_test().is_some() {}
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );
    let first = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("multiple slot-only roots are exact");

    let mut realized = branch.into_realized_for_test();
    realized.reverse_child_slots_for_test();
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );
    let permuted = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("slot input permutation preserves the frozen topology");

    assert_eq!(first.entities(), permuted.entities());
    assert_eq!(first.digest(), permuted.digest());
}

#[test]
fn failed_prepared_sparse_missing_external_boundary_is_rejected_without_world_write() {
    let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let child = realized.pop_frame_for_test().unwrap();
    let child_frame = child.frame().clone();
    let child_shell = child.shell();
    let boundary = child.mount_binding().slot_claim().clone();
    let child = crate::branch_transaction::RealizedBranch::from_single_frame(child);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(child),
    );
    assert!(
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(&boundary)
    );
    let entities_before = app.world().iter_entities().count();

    let prepared = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("the builder freezes missing boundary evidence for caller classification");
    assert_eq!(
        prepared.boundary_drifts,
        vec![FailedBoundaryDrift {
            source: FailedBoundarySource::FrameMountSlot {
                frame: child_frame,
                removed_child: child_shell,
            },
            expected: FailedBoundaryExpectation::ExactClaim(boundary),
            state: FailedManifestClaimState::Missing,
            predicate: FailedMountParentFreezePredicate::ClaimAuthority,
        }]
    );
    assert_eq!(app.world().iter_entities().count(), entities_before);
}

#[test]
fn failed_prepared_sparse_freezes_every_missing_boundary_in_stable_source_order() {
    let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    realized.clear_child_slots_for_test();
    let expected = realized.frames()[1..]
        .iter()
        .map(|frame| {
            (
                frame.frame().clone(),
                frame.shell(),
                frame.mount_binding().slot_claim().clone(),
            )
        })
        .collect::<Vec<_>>();
    for (_, _, claim) in &expected {
        assert!(
            app.world_mut()
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(claim)
        );
    }
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();

    assert_eq!(manifest.boundary_drifts.len(), expected.len());
    for (drift, (frame, removed_child, claim)) in manifest.boundary_drifts.iter().zip(expected) {
        assert_eq!(
            drift,
            &FailedBoundaryDrift {
                source: FailedBoundarySource::FrameMountSlot {
                    frame,
                    removed_child,
                },
                expected: FailedBoundaryExpectation::ExactClaim(claim),
                state: FailedManifestClaimState::Missing,
                predicate: FailedMountParentFreezePredicate::ClaimAuthority,
            }
        );
    }
}

#[test]
fn failed_prepared_sparse_detailed_mount_drift_retains_exact_source_and_expectation() {
    let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    realized.clear_child_slots_for_test();
    let frame = &realized.frames()[1];
    let frame_identity = frame.frame().clone();
    let removed_child = frame.shell();
    let boundary = frame.mount_binding().slot_claim().clone();
    app.world_mut()
        .entity_mut(boundary.entity())
        .remove::<bevy_ui::Node>();
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();

    let expected_drift = FailedBoundaryDrift {
        source: FailedBoundarySource::FrameMountSlot {
            frame: frame_identity,
            removed_child,
        },
        expected: FailedBoundaryExpectation::ExactClaim(boundary),
        state: FailedManifestClaimState::Exact,
        predicate: FailedMountParentFreezePredicate::RegistryManifestMismatch,
    };
    assert!(
        manifest.boundary_drifts.contains(&expected_drift),
        "expected={expected_drift:#?} actual={:#?}",
        manifest.boundary_drifts
    );
}

#[test]
fn failed_prepared_sparse_local_claim_loss_does_not_flatten_exact_siblings() {
    let (mut app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let target = branch.frames()[1]
        .claims()
        .find(|claim| claim.entity() != branch.frames()[1].shell())
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_lifecycle(&target, EntityLifecycle::Quarantined)
        .unwrap();

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("one locally lost claim must not erase exact sibling obligations");

    assert_eq!(
        manifest
            .obligations
            .iter()
            .filter(|obligation| obligation.terminal_mode == CleanupTerminalMode::AuthorityLost)
            .map(|obligation| obligation.entity())
            .collect::<Vec<_>>(),
        vec![target.entity()]
    );
    assert!(manifest.obligations.iter().any(|obligation| {
        obligation.entity() != target.entity()
            && obligation.terminal_mode == CleanupTerminalMode::Normal
    }));
}

#[test]
fn failed_prepared_sparse_duplicate_graph_entity_is_rejected() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let duplicate = realized.child_slots()[0].clone_for_test();
    realized.push_child_slot_unchecked_for_test(duplicate);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    assert_eq!(
        FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::DuplicateEntity,
        ))
    );
}

#[test]
fn failed_prepared_sparse_materializes_residual_and_rejected_topology_nodes() {
    let (residual_app, residual_branch) = crate::navigation::tests::failed_mount_residual_fixture();
    let residual_topology = FailedPreparedTopology::build(
        residual_app.world(),
        residual_app.world().resource::<RouteEntityRegistry>(),
        &residual_branch,
    )
    .unwrap();
    assert_eq!(residual_topology.auxiliary_slots.len(), 1);
    assert!(matches!(
        residual_topology.auxiliary_slots[0].source,
        FailedPreparedAuxiliarySlotSource::Residual(_)
    ));

    let (rejected_app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let slot = branch.child_slots()[0].clone_for_test();
    let mut empty = RealizedBranch::empty();
    let rejected = empty
        .push_child_slot(slot)
        .expect_err("a slot without its stored parent becomes a rejected owner");
    let rejected_branch = FailedPreparedBranch::with_rejected_child_slot(
        crate::branch_transaction::PreparedBranch::from_realized(empty),
        rejected,
    );
    let rejected_topology = FailedPreparedTopology::build(
        rejected_app.world(),
        rejected_app.world().resource::<RouteEntityRegistry>(),
        &rejected_branch,
    )
    .unwrap();
    assert_eq!(rejected_topology.auxiliary_slots.len(), 1);
    assert!(matches!(
        rejected_topology.auxiliary_slots[0].source,
        FailedPreparedAuxiliarySlotSource::Rejected(_)
    ));
}

#[test]
fn failed_prepared_sparse_residual_and_rejected_duplicates_cross_primary_once() {
    let (residual_app, mut residual_branch) =
        crate::navigation::tests::failed_mount_residual_fixture();
    let residual_claim = residual_branch.mount_residuals()[0].claim().clone();
    let target = residual_branch.frames()[0]
        .claims()
        .find(|claim| claim.entity() != residual_branch.frames()[0].shell())
        .unwrap()
        .clone();
    residual_branch.replace_frame_claim_for_test(0, target.entity(), residual_claim.clone());
    assert!(matches!(
        FailedPreparedTopology::build(
            residual_app.world(),
            residual_app.world().resource::<RouteEntityRegistry>(),
            &residual_branch,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::DuplicateEntity
        ))
    ));

    let (rejected_app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let duplicate = realized.child_slots()[0].clone_for_test();
    let rejected = realized
        .push_child_slot(duplicate)
        .expect_err("stored primary slot rejects the duplicate owner");
    let rejected_branch = FailedPreparedBranch::with_rejected_child_slot(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
        rejected,
    );
    assert!(matches!(
        FailedPreparedTopology::build(
            rejected_app.world(),
            rejected_app.world().resource::<RouteEntityRegistry>(),
            &rejected_branch,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::DuplicateEntity
        ))
    ));
}

#[test]
fn failed_prepared_sparse_residual_and_rejected_duplicate_each_other_once() {
    let (app, mut branch) = crate::navigation::tests::failed_mount_residual_fixture();
    let residual = &branch.mount_residuals()[0];
    let (parent, outlet, slot_id) = match residual.claim().owner() {
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => (parent.clone(), outlet.clone(), *slot),
        owner => panic!("fixture must retain a provisional child-slot owner: {owner:?}"),
    };
    let committed = residual
        .claim()
        .with_owner_for_test(RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent: parent.clone(),
            outlet: outlet.clone(),
            slot: slot_id,
        }));
    let (_, source) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut rejected_slot = source.child_slots()[0].clone_for_test();
    rejected_slot.replace_slot_binding_for_test(
        residual.entity(),
        committed,
        parent,
        outlet,
        slot_id,
    );
    let mut empty = RealizedBranch::empty();
    let rejected = empty
        .push_child_slot(rejected_slot)
        .expect_err("slot without its stored parent becomes a rejected owner");
    branch.push_rejected_child_slot_for_test(rejected);

    match FailedPreparedTopology::build(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    ) {
        Err(error) => assert_eq!(
            error,
            FailedManifestBuildError::InvalidTopology(FailedPreparedTopologyError::DuplicateEntity,)
        ),
        Ok(_) => panic!("residual/rejected duplicate must fail during topology build"),
    }
}

#[test]
fn failed_prepared_sparse_duplicate_external_incoming_is_typed_during_build() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    realized.clear_child_slots_for_test();
    realized.replace_frame_mount_from_for_test(1, 0);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    assert_eq!(
        FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::DuplicateIncoming,
        ))
    );
}

#[test]
fn failed_prepared_sparse_duplicate_owner_and_seal_are_typed_from_stored_input() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let source = realized.frames()[0]
        .claims()
        .find(|claim| claim.entity() != realized.frames()[0].shell())
        .unwrap()
        .clone();
    let target = realized.frames()[1]
        .claims()
        .find(|claim| claim.entity() != realized.frames()[1].shell())
        .unwrap()
        .clone();
    realized.replace_frame_claim_for_test(
        1,
        target.entity(),
        target.with_owner_for_test(source.owner().clone()),
    );
    let duplicate_owner = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );
    assert_eq!(
        FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &duplicate_owner,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::ConflictingOwner,
        ))
    );

    let mut realized = duplicate_owner.into_realized_for_test();
    realized.replace_frame_claim_for_test(
        1,
        target.entity(),
        target.with_seal_for_test(source.seal()),
    );
    let duplicate_seal = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );
    assert_eq!(
        FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &duplicate_seal,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::ConflictingOwner,
        ))
    );
}

#[test]
fn failed_prepared_sparse_stored_cycle_is_rootless_through_complete_builder() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let slot = &realized.child_slots()[0];
    let frame = realized
        .frames()
        .iter()
        .position(|frame| frame.frame() == slot.parent())
        .unwrap();
    let mount_source = realized
        .frames()
        .iter()
        .position(|frame| frame.mount_binding().matches_mounted_child_slot(slot))
        .unwrap();
    realized.retain_cycle_for_test(frame, 0, mount_source);
    let branch = FailedPreparedBranch::new(
        crate::branch_transaction::PreparedBranch::from_realized(realized),
    );

    assert_eq!(
        FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        ),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::Rootless,
        ))
    );
}

#[test]
fn failed_prepared_sparse_shared_revisit_is_typed_invalid_topology() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut topology = FailedPreparedTopology::build(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();
    let root = topology
        .frames
        .iter()
        .position(|frame| frame.incoming.is_none() && !frame.outgoing.is_empty())
        .unwrap();
    let shared = topology.frames[root].outgoing[0];
    topology.frames[root].outgoing.push(shared);

    assert!(matches!(
        topology.freeze_obligations(app.world(), app.world().resource::<RouteEntityRegistry>(),),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::CycleOrRevisit
        ))
    ));
}

#[test]
fn failed_prepared_sparse_rootless_stored_component_is_typed_invalid_topology() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let mut topology = FailedPreparedTopology::build(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();
    for frame in &mut topology.frames {
        frame.incoming = Some(0);
    }
    for slot in &mut topology.slots {
        slot.parent = Some(0);
    }

    assert!(matches!(
        topology.freeze_obligations(app.world(), app.world().resource::<RouteEntityRegistry>(),),
        Err(FailedManifestBuildError::InvalidTopology(
            FailedPreparedTopologyError::Rootless
        ))
    ));
}

fn projected_leaving_claims(
    registry: &RouteEntityRegistry,
    frames: &[RealizedFrame],
    child_slots: &[MountedChildSlot],
) -> Vec<ExactRegistryClaim> {
    let transitions = frames
        .iter()
        .flat_map(|frame| frame.lifecycle_transitions(EntityLifecycle::Leaving))
        .chain(
            child_slots
                .iter()
                .map(|slot| (slot.slot_claim().clone(), EntityLifecycle::Leaving)),
        )
        .collect::<Vec<_>>();
    registry
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec()
}

#[test]
fn slot_only_leaving_manifest_freezes_exact_obligation_and_mount_parent() {
    let (app, branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let slot = &branch.child_slots()[0];
    let registry = app.world().resource::<RouteEntityRegistry>();
    let projected = projected_leaving_claims(registry, &[], std::slice::from_ref(slot));

    let manifest = FrozenCleanupManifest::from_leaving_branch_parts(
        app.world(),
        registry,
        &[],
        std::slice::from_ref(slot),
        &projected,
        None,
    )
    .expect("a slot-only exact leaving residual must freeze independently");

    assert_eq!(manifest.entities(), vec![child_slot]);
    assert_eq!(manifest.mount_parents.len(), 1);
    assert_eq!(manifest.mount_parents[0].claim, *slot.anchor_claim());
    let reserved = CleanupLedger::reserve(manifest).unwrap();
    assert_eq!(reserved.obligations.len(), 1);
    assert_eq!(reserved.receipts, vec![None]);
}

#[test]
fn exact_external_slot_only_authority_branch_preserves_exact_terminal_mode() {
    let (app, branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let slot = &branch.child_slots()[0];
    let mut obligations = Vec::new();

    assert!(
        append_exact_authority_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &[],
            std::slice::from_ref(slot),
            &mut obligations,
        )
        .is_ok()
    );
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].entity(), child_slot);
    assert_eq!(
        obligations[0].terminal_mode,
        CleanupTerminalMode::AuthorityExact
    );
}

#[test]
fn authority_obligation_normalization_deduplicates_only_identical_bindings() {
    let (app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let slot = &branch.child_slots()[0];
    let mut obligations = Vec::new();
    append_exact_authority_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &[],
        std::slice::from_ref(slot),
        &mut obligations,
    )
    .unwrap();
    let obligation = obligations.pop().unwrap();
    let mut identical = vec![obligation.clone(), obligation.clone()];
    normalize_authority_obligations(&mut identical).unwrap();
    assert_eq!(identical, vec![obligation.clone()]);

    let mut conflicting = obligation.clone();
    conflicting.terminal_mode = CleanupTerminalMode::AuthorityLost;
    let mut conflicting = vec![obligation, conflicting];
    assert_eq!(
        normalize_authority_obligations(&mut conflicting),
        Err(AuthoritySettlementManifestError::InvalidTopology)
    );
}

#[test]
fn authority_structural_duplicate_is_invalid_topology_without_flat_lost_fallback() {
    let (app, branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let slot = &branch.child_slots()[0];
    let duplicate = slot.clone_for_test();
    let registry = app.world().resource::<RouteEntityRegistry>();
    let claim_before = registry.record(child_slot).unwrap().claim().clone();
    let mut obligations = Vec::new();

    assert_eq!(
        append_exact_authority_branch(
            app.world(),
            registry,
            branch.frames(),
            &[slot.clone_for_test(), duplicate],
            &mut obligations,
        ),
        Err(AuthoritySettlementManifestError::InvalidTopology)
    );
    assert_eq!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(child_slot)
            .unwrap()
            .claim(),
        claim_before
    );
    assert!(obligations.iter().all(|obligation| {
        obligation.terminal_mode != CleanupTerminalMode::AuthorityLost
            || !cleanup_claim_has_exact_authority(
                app.world(),
                obligation
                    .identity
                    .registered()
                    .expect("registered test fixture"),
            )
    }));
}

#[test]
fn nested_leaving_manifest_orders_child_frame_before_slot_and_parent_frame() {
    let (app, branch, child_entities, child_slot, parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let registry = app.world().resource::<RouteEntityRegistry>();
    let projected = projected_leaving_claims(registry, branch.frames(), branch.child_slots());

    let manifest = FrozenCleanupManifest::from_leaving_branch_parts(
        app.world(),
        registry,
        branch.frames(),
        branch.child_slots(),
        &projected,
        None,
    )
    .expect("an exact leaving branch must freeze one cleanup manifest");
    let entities = manifest.entities();
    let child_end = child_entities
        .iter()
        .map(|entity| {
            entities
                .iter()
                .position(|candidate| candidate == entity)
                .unwrap()
        })
        .max()
        .unwrap();
    let slot_index = entities
        .iter()
        .position(|entity| *entity == child_slot)
        .unwrap();
    let parent_start = parent_entities
        .iter()
        .map(|entity| {
            entities
                .iter()
                .position(|candidate| candidate == entity)
                .unwrap()
        })
        .min()
        .unwrap();

    assert!(child_end < slot_index);
    assert!(slot_index < parent_start);
    assert_eq!(
        manifest.len(),
        branch
            .frames()
            .iter()
            .map(RealizedFrame::obligation_count)
            .sum::<usize>()
            + branch.child_slots().len()
    );
}

#[test]
fn nested_leaving_manifest_rejects_non_leaving_or_incomplete_projection() {
    let (app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let registry = app.world().resource::<RouteEntityRegistry>();
    let mut projected = projected_leaving_claims(registry, branch.frames(), branch.child_slots());
    projected.pop();

    assert!(
        FrozenCleanupManifest::from_leaving_branch_parts(
            app.world(),
            registry,
            branch.frames(),
            branch.child_slots(),
            &projected,
            None,
        )
        .is_none()
    );

    let active = registry
        .prepare_lifecycle_mutation(&[(
            branch.child_slots()[0].slot_claim().clone(),
            EntityLifecycle::Active,
        )])
        .unwrap()
        .resulting_claims()[0]
        .clone();
    let mut projected = projected_leaving_claims(registry, branch.frames(), branch.child_slots());
    let child_slot = branch.child_slots()[0].slot();
    *projected
        .iter_mut()
        .find(|claim| claim.entity() == child_slot)
        .unwrap() = active;

    assert!(
        FrozenCleanupManifest::from_leaving_branch_parts(
            app.world(),
            registry,
            branch.frames(),
            branch.child_slots(),
            &projected,
            None,
        )
        .is_none()
    );
}

#[test]
fn nested_leaving_cleanup_settles_exact_slot_and_preserves_external_host() {
    let (mut app, branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let top_slot = branch.frames()[0].top_slot();
    let app_host = app.world().get::<ChildOf>(top_slot).unwrap().parent();
    let projected = projected_leaving_claims(
        app.world().resource::<RouteEntityRegistry>(),
        branch.frames(),
        branch.child_slots(),
    );
    let manifest = FrozenCleanupManifest::from_leaving_branch_parts(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        branch.frames(),
        branch.child_slots(),
        &projected,
        None,
    )
    .unwrap();
    let obligation_count = manifest.len();
    let transitions = branch
        .frames()
        .iter()
        .flat_map(|frame| frame.lifecycle_transitions(EntityLifecycle::Leaving))
        .chain(
            branch
                .child_slots()
                .iter()
                .map(|slot| (slot.slot_claim().clone(), EntityLifecycle::Leaving)),
        )
        .collect::<Vec<_>>();
    let registry_plan = app
        .world()
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap();
    for frame in branch.frames() {
        for claim in frame.claims() {
            app.world_mut()
                .entity_mut(claim.entity())
                .insert(FrameLifecycle(EntityLifecycle::Leaving));
        }
    }
    app.world_mut()
        .entity_mut(child_slot)
        .insert(ChildSlotLifecycle::Leaving);
    registry_plan.apply(
        app.world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .into_inner(),
    );
    app.world_mut().flush();
    let prepared = CleanupLedger::reserve(manifest).unwrap();
    let mut ledger = CleanupLedger::activate(prepared, None);
    let attempt = ledger.attempt(app.world()).unwrap();

    assert!(ledger.settle(app.world_mut(), attempt));
    assert_eq!(ledger.receipt_count(), obligation_count);
    assert!(app.world().get_entity(top_slot).is_ok());
    assert!(app.world().get_entity(app_host).is_ok());
}

#[test]
fn nested_failed_cleanup_only_crosses_the_frozen_exact_owner_boundary() {
    let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let top_slot = branch.frames()[0].top_slot();
    let app_host = app.world().get::<ChildOf>(top_slot).unwrap().parent();
    let foreign = app
        .world_mut()
        .spawn((ForeignBoundaryMarker, ChildOf(app_host)))
        .id();
    let sibling_router = registered_entity(app.world_mut(), app_host);
    app.world_mut().flush();
    let sibling_claim = app
        .world()
        .resource::<RouteEntityRegistry>()
        .record(sibling_router)
        .unwrap()
        .claim();
    let owned = branch
        .frames()
        .iter()
        .flat_map(|frame| frame.claims().map(ExactRegistryClaim::entity))
        .chain(branch.child_slots().iter().map(MountedChildSlot::slot))
        .collect::<Vec<_>>();
    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();
    let prepared = CleanupLedger::reserve(manifest).unwrap();
    let mut ledger = CleanupLedger::activate_failed(prepared, branch);
    let attempt = ledger.attempt(app.world()).unwrap();

    assert!(ledger.settle(app.world_mut(), attempt));
    assert!(
        owned
            .iter()
            .all(|entity| app.world().get_entity(*entity).is_err())
    );
    assert!(app.world().get_entity(app_host).is_ok());
    assert!(app.world().get_entity(top_slot).is_ok());
    assert!(app.world().get::<ForeignBoundaryMarker>(foreign).is_some());
    assert_eq!(
        app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(app_host)
    );
    assert_eq!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(sibling_router)
            .map(|record| record.claim()),
        Some(sibling_claim)
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(sibling_router)
            .map(ChildOf::parent),
        Some(app_host)
    );
}

#[test]
fn nested_cleanup_exact_claim_drift_in_each_unit_is_world_zero() {
    for unit in 0..3 {
        let (mut app, branch, _child_entities, child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let target = match unit {
            0 => branch.frames()[1].shell(),
            1 => child_slot,
            2 => branch.frames()[0].shell(),
            _ => unreachable!(),
        };
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate_failed(prepared, branch);
        let frozen_claim = ledger
            .obligations
            .iter()
            .find(|obligation| obligation.entity() == target)
            .unwrap()
            .identity
            .registered()
            .expect("registered test fixture")
            .clone();
        let drifted_claim = app
            .world_mut()
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&frozen_claim, EntityLifecycle::Quarantined)
            .unwrap();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(target);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(target));
        let registry_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .to_vec();
        let world_before = app
            .world()
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        let attempt = ledger.attempt(app.world()).unwrap();
        let obligation = ledger
            .obligations
            .iter()
            .position(|obligation| obligation.entity() == target)
            .unwrap();

        assert!(
            attempt
                .evidence
                .contains(&(obligation, CleanupEvidence::StaleClaim))
        );
        assert!(!ledger.settle(app.world_mut(), attempt));
        assert_eq!(ledger.receipt_count(), 0);
        assert_eq!(ledger.failed_prepared_frames().len(), 2);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(target));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(target)
        );
        assert_eq!(
            app.world().resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(target)
                .map(|record| record.claim()),
            Some(drifted_claim)
        );
        assert_eq!(
            app.world()
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>(),
            world_before
        );
    }
}

#[test]
fn failed_prepared_branch_manifest_survives_child_slot_relationship_drift() {
    let (mut app, branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    app.world_mut().entity_mut(child_slot).remove::<ChildOf>();
    app.world_mut().flush();

    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .expect("stored exact ownership must remain cleanup-authoritative after relationship drift");

    assert!(manifest.entities().contains(&child_slot));
}

#[test]
fn failed_prepared_branch_child_slot_lifecycle_drift_is_locally_lost_and_bounded() {
    let (mut app, branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let exact_claim = app
        .world()
        .resource::<RouteEntityRegistry>()
        .record(child_slot)
        .unwrap()
        .claim();
    app.world_mut()
        .entity_mut(child_slot)
        .insert(ChildSlotLifecycle::ProvisionalInert);
    app.world_mut().flush();
    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();
    let prepared = CleanupLedger::reserve(manifest).unwrap();
    let mut ledger = CleanupLedger::activate_failed(prepared, branch);
    let attempt = ledger.attempt(app.world()).unwrap();

    assert!(ledger.settle(app.world_mut(), attempt));
    assert_eq!(ledger.receipt_count(), ledger.obligations.len());
    assert!(ledger.is_settled());
    assert!(ledger.attempt(app.world()).is_none());
    assert_eq!(
        app.world().get::<ChildSlotLifecycle>(child_slot),
        Some(&ChildSlotLifecycle::ProvisionalInert)
    );
    assert!(
        app.world()
            .resource::<RouteEntityRegistry>()
            .record(child_slot)
            .is_none()
    );
    assert!(
        app.world()
            .resource::<QuarantineLeaseTable>()
            .authority_lost_tombstone(child_slot)
            .is_some_and(|tombstone| tombstone.source_identity().registered() == Some(&exact_claim))
    );
}

#[test]
fn nested_cleanup_accepts_exact_active_and_leaving_child_slot_claims() {
    let (app, _branch, _child_entities, child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let registry = app.world().resource::<RouteEntityRegistry>();
    let current = registry.record(child_slot).unwrap().claim();
    let active = registry
        .prepare_lifecycle_mutation(&[(current.clone(), EntityLifecycle::Active)])
        .unwrap()
        .resulting_claims()[0]
        .clone();
    let leaving = registry
        .prepare_lifecycle_mutation(&[(current, EntityLifecycle::Leaving)])
        .unwrap()
        .resulting_claims()[0]
        .clone();
    let authority = app.world().get::<ChildSlotAuthority>(child_slot);

    assert!(child_slot_components_match_claim(
        Some(&ChildSlotLifecycle::Active),
        authority,
        &active,
    ));
    assert!(child_slot_components_match_claim(
        Some(&ChildSlotLifecycle::Leaving),
        authority,
        &leaving,
    ));
    assert!(child_slot_components_are_compatible(
        Some(&ChildSlotLifecycle::Active),
        authority,
        &active,
        false,
    ));
    assert!(child_slot_components_are_compatible(
        Some(&ChildSlotLifecycle::Leaving),
        authority,
        &leaving,
        false,
    ));
}

fn single_frame_cleanup_ledger(world: &World, frame: &RealizedFrame) -> CleanupLedger {
    let manifest = FrozenCleanupManifest::from_current_frame(
        world,
        world.resource::<RouteEntityRegistry>(),
        frame,
    )
    .unwrap();
    CleanupLedger::activate(CleanupLedger::reserve(manifest).unwrap(), None)
}

fn assert_zero_write_frame_cleanup(
    world: &mut World,
    ledger: &mut CleanupLedger,
    frame_entities: &[Entity],
) {
    let attempt = ledger.attempt(world).unwrap();
    assert!(!ledger.settle(world, attempt));
    assert_eq!(ledger.receipt_count(), 0);
    assert!(
        frame_entities
            .iter()
            .all(|entity| world.get_entity(*entity).is_ok())
    );
}

#[test]
fn external_host_top_seal_drift_is_zero_write() {
    let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let frame = &branch.frames()[0];
    let top_slot = frame.top_slot();
    let frame_entities = frame
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
    let drifted = app
        .world_mut()
        .resource_mut::<RouteEntityRegistry>()
        .issue_unregistered_seal_for_test();
    app.world_mut()
        .entity_mut(top_slot)
        .insert(RouteOwnershipSeal::new(drifted));

    assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
}

#[test]
fn external_host_top_value_drift_is_zero_write_and_retains_owner() {
    for drift_node in [false, true] {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame = &branch.frames()[0];
        let top_slot = frame.top_slot();
        let focus = frame.shell();
        let claims = branch
            .frames()
            .iter()
            .flat_map(|frame| frame.claims().cloned())
            .collect::<Vec<_>>();
        let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &branch,
        )
        .unwrap();
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate_failed(prepared, branch);
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus));
        if drift_node {
            app.world_mut().entity_mut(top_slot).insert(Node {
                width: Val::Percent(99.0),
                height: Val::Percent(100.0),
                ..Node::default()
            });
        } else {
            app.world_mut()
                .entity_mut(top_slot)
                .insert(RouterInputBarrier::allow_all());
        }
        app.world_mut().flush();
        let attempt = ledger.attempt(app.world()).unwrap();

        assert!(!ledger.settle(app.world_mut(), attempt));
        assert_eq!(ledger.receipt_count(), 0);
        assert_eq!(ledger.failed_prepared_frames().len(), 2);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(focus)
        );
        assert!(claims.iter().all(|claim| {
            app.world().get_entity(claim.entity()).is_ok()
                && app
                    .world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_some_and(|record| record.claim() == *claim)
        }));
        if drift_node {
            assert_eq!(
                app.world().get::<Node>(top_slot).map(|node| node.width),
                Some(Val::Percent(99.0))
            );
        } else {
            assert!(
                !app.world()
                    .get::<RouterInputBarrier>(top_slot)
                    .unwrap()
                    .denies_all()
            );
        }
    }
}

#[test]
fn attached_shell_relationship_drift_is_zero_write() {
    let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let frame = &branch.frames()[0];
    let shell = frame.shell();
    let frame_entities = frame
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
    assert!(frame.shell_is_attached());
    app.world_mut().entity_mut(shell).remove::<ChildOf>();
    app.world_mut().flush();

    assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
}

#[test]
fn external_child_slot_lifecycle_and_authority_drift_are_zero_write() {
    for authority_drift in [false, true] {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame = &branch.frames()[1];
        let mounted = &branch.child_slots()[0];
        let slot = mounted.slot();
        let frame_entities = frame
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
        if authority_drift {
            app.world_mut()
                .entity_mut(slot)
                .insert(ChildSlotAuthority::new(
                    mounted.parent().clone(),
                    RouteSlotKey::named("drifted").unwrap(),
                    mounted.slot_id(),
                ));
        } else {
            app.world_mut()
                .entity_mut(slot)
                .insert(ChildSlotLifecycle::ProvisionalInert);
        }

        assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
    }
}

#[test]
fn external_child_slot_value_drift_is_zero_write_and_retains_owner() {
    for drift_node in [false, true] {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame = &branch.frames()[1];
        let slot = branch.child_slots()[0].slot();
        let focus = frame.shell();
        let frame_entities = frame
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(focus);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(focus));
        if drift_node {
            app.world_mut().entity_mut(slot).insert(Node {
                width: Val::Percent(99.0),
                height: Val::Percent(100.0),
                ..Node::default()
            });
        } else {
            app.world_mut()
                .entity_mut(slot)
                .insert(RouterInputBarrier::allow_all());
        }
        app.world_mut().flush();

        assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
        assert_eq!(branch.frames().len(), 2);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(focus));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(focus)
        );
        if drift_node {
            assert_eq!(
                app.world().get::<Node>(slot).map(|node| node.width),
                Some(Val::Percent(99.0))
            );
        } else {
            assert!(
                !app.world()
                    .get::<RouterInputBarrier>(slot)
                    .unwrap()
                    .denies_all()
            );
        }
    }
}

#[test]
fn detailed_mount_parent_error_preserves_unsupported_owner_fact() {
    let (app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let mounted = &branch.child_slots()[0];
    let obligations = vec![
        mounted_slot_obligation(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            mounted,
        )
        .unwrap(),
    ];
    let shell_claim = branch.frames()[0]
        .claims()
        .find(|claim| {
            matches!(
                claim.owner(),
                RegistryOwner::Committed(EntityOwner::FrameShell(_))
            )
        })
        .unwrap()
        .clone();
    let mut candidate = mounted_anchor_parent_candidate(app.world(), mounted).unwrap();
    candidate.claim = shell_claim.clone();

    let error = freeze_external_mount_parents_detailed(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &obligations,
        [candidate],
    )
    .unwrap_err();

    assert_eq!(error.source, None);
    assert_eq!(
        error.expected,
        FailedBoundaryExpectation::ExactClaim(shell_claim)
    );
    assert_eq!(error.state, FailedManifestClaimState::Exact);
    assert_eq!(
        error.predicate,
        FailedMountParentFreezePredicate::UnsupportedOwner
    );
}

#[test]
fn detailed_mount_parent_errors_preserve_each_physical_predicate() {
    fn freeze_error(case: usize) -> FailedMountParentFreezeError {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let mounted = &branch.child_slots()[0];
        let anchor = mounted.anchor();
        let anchor_claim = mounted.anchor_claim().clone();
        let obligations = vec![
            mounted_slot_obligation(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                mounted,
            )
            .unwrap(),
        ];
        let mut first = mounted_anchor_parent_candidate(app.world(), mounted).unwrap();
        let mut second = None;
        match case {
            0 => {
                let foreign = app.world_mut().spawn_empty().id();
                first.children_before = vec![foreign];
            }
            1 => {
                let mut conflict = mounted_anchor_parent_candidate(app.world(), mounted).unwrap();
                conflict.removed_child = app.world_mut().spawn_empty().id();
                conflict.removed_child_was_attached = false;
                second = Some(conflict);
            }
            2 => {
                assert!(
                    app.world_mut()
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(&anchor_claim)
                );
            }
            3 => {
                first.children_before.clear();
                first.children_after.clear();
                first.removed_child_was_attached = false;
                assert!(app.world_mut().despawn(anchor));
            }
            4 => {
                app.world_mut()
                    .resource_mut::<RouteEntityRegistry>()
                    .compare_exchange_lifecycle(&anchor_claim, EntityLifecycle::Quarantined)
                    .unwrap();
            }
            5 => {
                first.supplied_parent = Some((app.world_mut().spawn_empty().id(), None));
            }
            6 => {
                app.world_mut().entity_mut(anchor).insert(Node {
                    width: Val::Percent(99.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                });
            }
            _ => unreachable!(),
        }
        let candidates = match second {
            Some(second) => vec![first, second],
            None => vec![first],
        };
        freeze_external_mount_parents_detailed(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &obligations,
            candidates,
        )
        .unwrap_err()
    }

    let expected = [
        (
            FailedMountParentFreezePredicate::ChildrenProjection,
            FailedManifestClaimState::Exact,
        ),
        (
            FailedMountParentFreezePredicate::ConflictingCandidate,
            FailedManifestClaimState::Exact,
        ),
        (
            FailedMountParentFreezePredicate::MissingRegistryRecord,
            FailedManifestClaimState::Missing,
        ),
        (
            FailedMountParentFreezePredicate::MissingPhysicalManifest,
            FailedManifestClaimState::Absent,
        ),
        (
            FailedMountParentFreezePredicate::RegistryManifestMismatch,
            FailedManifestClaimState::Replaced,
        ),
        (
            FailedMountParentFreezePredicate::ParentAuthority,
            FailedManifestClaimState::Exact,
        ),
        (
            FailedMountParentFreezePredicate::PhysicalAuthority,
            FailedManifestClaimState::Exact,
        ),
    ];
    for (case, (predicate, state)) in expected.into_iter().enumerate() {
        let error = freeze_error(case);
        assert_eq!(error.source, None, "case={case}");
        assert!(matches!(
            error.expected,
            FailedBoundaryExpectation::ExactClaim(_)
        ));
        assert_eq!(error.state, state, "case={case}");
        assert_eq!(error.predicate, predicate, "case={case}");
    }
}

#[test]
pub(crate) fn external_frame_output_value_and_manifest_drift_is_zero_write_and_retains_focus() {
    for drift in 0..6 {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let mounted = &branch.child_slots()[0];
        let anchor = mounted.anchor();
        let slot = mounted.slot();
        let obligations = vec![
            mounted_slot_obligation(
                app.world(),
                app.world().resource::<RouteEntityRegistry>(),
                mounted,
            )
            .unwrap(),
        ];
        let mount_parents = freeze_external_mount_parents(
            app.world(),
            app.world().resource::<RouteEntityRegistry>(),
            &obligations,
            [mounted_anchor_parent_candidate(app.world(), mounted).unwrap()],
        )
        .unwrap();
        assert_eq!(mount_parents[0].claim.entity(), anchor);
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        let manifest = FrozenCleanupManifest {
            obligations,
            mount_parents,
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest,
        };
        let prepared = CleanupLedger::reserve(manifest).unwrap();
        let mut ledger = CleanupLedger::activate(prepared, None);
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(slot);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(slot));
        match drift {
            0 => {
                app.world_mut().entity_mut(anchor).insert(Node {
                    width: Val::Percent(99.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                });
            }
            1 => {
                app.world_mut()
                    .entity_mut(anchor)
                    .insert(Visibility::Visible);
            }
            2 => {
                app.world_mut().entity_mut(anchor).insert(Pickable {
                    should_block_lower: false,
                    is_hoverable: true,
                });
            }
            3 => {
                app.world_mut()
                    .entity_mut(anchor)
                    .insert(RouterInputBarrier::allow_all());
            }
            4 => {
                app.world_mut()
                    .entity_mut(anchor)
                    .insert(ExternalParentDriftMarker);
            }
            5 => {
                let foreign = app.world_mut().spawn_empty().id();
                app.world_mut().entity_mut(foreign).insert(ChildOf(anchor));
            }
            _ => unreachable!(),
        }
        app.world_mut().flush();
        let attempt = ledger.attempt(app.world()).unwrap();

        assert!(
            !ledger.settle(app.world_mut(), attempt),
            "external mount-parent drift case {drift} must reject cleanup",
        );
        assert_eq!(ledger.receipt_count(), 0);
        assert!(app.world().get_entity(slot).is_ok());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(slot));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(slot)
        );
    }
}

#[test]
fn external_child_slot_relationship_and_children_order_drift_are_zero_write() {
    for relationship_drift in [false, true] {
        let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
            crate::navigation::tests::failed_nested_branch_fixture();
        let frame = &branch.frames()[1];
        let slot = branch.child_slots()[0].slot();
        let frame_entities = frame
            .claims()
            .map(ExactRegistryClaim::entity)
            .collect::<Vec<_>>();
        let mut ledger = single_frame_cleanup_ledger(app.world(), frame);
        let foreign = app.world_mut().spawn_empty().id();
        if relationship_drift {
            app.world_mut().entity_mut(slot).insert(ChildOf(foreign));
        } else {
            app.world_mut().entity_mut(foreign).insert(ChildOf(slot));
        }
        app.world_mut().flush();

        assert_zero_write_frame_cleanup(app.world_mut(), &mut ledger, &frame_entities);
    }
}

#[test]
fn frame_quarantine_preserves_foreign_child_slot_lifecycle() {
    let (mut app, branch, _child_entities, _child_slot, _parent_entities) =
        crate::navigation::tests::failed_nested_branch_fixture();
    let frame_shell = branch.frames()[0].shell();
    let manifest = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();
    app.world_mut()
        .entity_mut(frame_shell)
        .insert(ChildSlotLifecycle::ProvisionalInert);
    app.world_mut().flush();
    let prepared = CleanupLedger::reserve(manifest).unwrap();
    let mut ledger = CleanupLedger::activate_failed(prepared, branch);
    let attempt = ledger.attempt(app.world()).unwrap();

    assert!(ledger.settle(app.world_mut(), attempt));
    assert_eq!(
        app.world().get::<FrameLifecycle>(frame_shell),
        Some(&FrameLifecycle(EntityLifecycle::Quarantined))
    );
    assert_eq!(
        app.world().get::<ChildSlotLifecycle>(frame_shell),
        Some(&ChildSlotLifecycle::ProvisionalInert)
    );
}

#[test]
fn quarantine_detachment_projection_accumulates_later_edges_without_rebaselining_targets() {
    let (app, branch) = crate::navigation::tests::failed_deep_nested_branch_fixture();
    let claims = branch
        .frames()
        .iter()
        .flat_map(RealizedFrame::claims)
        .take(4)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 4);
    let registry = app.world().resource::<RouteEntityRegistry>();
    let observed = |claim: &ExactRegistryClaim| {
        FrozenFailureCleanupObservedEntity::observe(app.world(), registry, claim.entity())
    };
    let empty_manifest =
        || ComponentManifest::new(std::iter::empty::<bevy_ecs::component::ComponentId>());
    let edge = |parent: ExactRegistryClaim, child: ExactRegistryClaim| {
        let parent_manifest = empty_manifest();
        let child_manifest = empty_manifest();
        PreparedCleanupQuarantineMountParentDetachment {
            parent: CleanupMountParentSettlement {
                physical: parent.entity(),
                binding: CleanupParentBinding::Structural { upstream: None },
                claim: parent.clone(),
                manifest: parent_manifest.clone(),
                physical_manifest: parent_manifest.clone(),
                expected_parent: parent.entity(),
                expected_parent_claim: None,
                removed_child: child.entity(),
                removed_child_was_attached: true,
                children_before: vec![child.entity()],
                children_after: Vec::new(),
            },
            parent_next_manifest: parent_manifest.clone(),
            parent_physical_manifest_after: parent_manifest,
            child_claim: child,
            child_manifest_before: child_manifest.clone(),
            child_manifest_after: child_manifest,
        }
    };

    let first = PreparedCleanupQuarantineDetachment {
        targets_after_inert: vec![observed(&claims[1]), observed(&claims[3])],
        protected_before: vec![observed(&claims[0]), observed(&claims[2])],
        edges: vec![edge(claims[0].clone(), claims[1].clone())],
    };
    let mut accumulated = FrozenCleanupQuarantineDetachment::from_prepared(&first, 1);
    let original_first_target = accumulated.targets_after[0].clone();
    let mut second_targets = accumulated.targets_after.clone();
    second_targets[0].parent = Some(claims[2].entity());
    let second = PreparedCleanupQuarantineDetachment {
        targets_after_inert: second_targets,
        protected_before: accumulated.protected_after.clone(),
        edges: vec![edge(claims[2].clone(), claims[3].clone())],
    };

    assert!(accumulated.reserve_merge_from_prepared(&second));
    let second = FrozenCleanupQuarantineDetachment::from_prepared(&second, 1);
    assert!(accumulated.merge_conflict_kind(&second).is_none());
    accumulated.merge_prechecked(second);
    assert_eq!(accumulated.edges.len(), 2);
    assert!(accumulated.contains_parent(&edge(claims[0].clone(), claims[1].clone()).parent));
    assert!(accumulated.contains_parent(&edge(claims[2].clone(), claims[3].clone()).parent));
    assert_eq!(accumulated.targets_after.len(), 2);
    assert_eq!(accumulated.targets_after[0], original_first_target);
}

#[test]
fn exhausted_ledger_domain_never_advances_or_reuses_authority() {
    let counter = AtomicU64::new(u64::MAX - 1);

    assert_eq!(allocate_ledger_domain(&counter), Some(u64::MAX - 1));
    assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    assert_eq!(allocate_ledger_domain(&counter), None);
    assert_eq!(allocate_ledger_domain(&counter), None);
    assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn cleanup_ledger_reserve_returns_manifest_and_classifies_every_failure_domain() {
    let (mut app, branch, _, _, _) = crate::navigation::tests::failed_nested_branch_fixture();
    let top_slot = app
        .world()
        .resource::<crate::navigation::RouterNavigation>()
        .entries[0]
        .host
        .as_ref()
        .unwrap()
        .top_slot;
    let stale_seal = app
        .world_mut()
        .resource_mut::<RouteEntityRegistry>()
        .issue_unregistered_seal_for_test();
    app.world_mut()
        .entity_mut(top_slot)
        .insert(RouteOwnershipSeal::new(stale_seal));
    let unfinalized = FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world().resource::<RouteEntityRegistry>(),
        &branch,
    )
    .unwrap();
    assert!(!unfinalized.boundary_drifts().is_empty());
    let boundary_counter = AtomicU64::new(41);
    let boundary = match CleanupLedger::reserve_with_domain_counter(unfinalized, &boundary_counter)
    {
        Err(error) => error,
        Ok(_) => panic!("unfinalized boundary facts are a permanent reserve contradiction"),
    };
    assert_eq!(
        boundary.kind(),
        CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts
    );
    assert!(!boundary.into_manifest().boundary_drifts().is_empty());
    assert_eq!(boundary_counter.load(Ordering::Relaxed), 41);

    let capacity_counter = AtomicU64::new(51);
    fail_next_reservation_for_test();
    let capacity = match CleanupLedger::reserve_with_domain_counter(
        FrozenCleanupManifest::empty(),
        &capacity_counter,
    ) {
        Err(error) => error,
        Ok(_) => panic!("receipt reservation fault is retryable capacity"),
    };
    assert_eq!(
        capacity.kind(),
        CleanupLedgerReserveErrorKind::RetryableCapacity
    );
    assert_eq!(capacity.into_manifest().len(), 0);
    assert_eq!(capacity_counter.load(Ordering::Relaxed), 51);

    let exhausted_counter = AtomicU64::new(u64::MAX - 1);
    let prepared = CleanupLedger::reserve_with_domain_counter(
        FrozenCleanupManifest::empty(),
        &exhausted_counter,
    )
    .unwrap();
    assert_eq!(prepared.ledger_domain, u64::MAX - 1);
    assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
    let exhausted = match CleanupLedger::reserve_with_domain_counter(
        FrozenCleanupManifest::empty(),
        &exhausted_counter,
    ) {
        Err(error) => error,
        Ok(_) => panic!("MAX-1 is the last successful domain allocation"),
    };
    assert_eq!(
        exhausted.kind(),
        CleanupLedgerReserveErrorKind::DomainExhausted
    );
    assert_eq!(exhausted.into_manifest().len(), 0);
    assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
    let repeated = match CleanupLedger::reserve_with_domain_counter(
        FrozenCleanupManifest::empty(),
        &exhausted_counter,
    ) {
        Err(error) => error,
        Ok(_) => panic!("exhausted domain remains terminal and cannot wrap"),
    };
    assert_eq!(
        repeated.kind(),
        CleanupLedgerReserveErrorKind::DomainExhausted
    );
    assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn reconciliation_zero_obligation_completion_is_exact_once() {
    let (mut engine, batch) = published_empty_batch();
    let prepared = CleanupLedger::prepare(batch, FrozenCleanupManifest::empty()).unwrap();
    let ledger = CleanupLedger::activate(prepared, None);
    let first = ledger.pending_completion().unwrap();
    let replay = ledger.pending_completion().unwrap();
    engine.complete_cleanup(first).unwrap();
    assert!(engine.complete_cleanup(replay).is_err());
}

#[test]
fn reconciliation_sealed_batch_must_match_the_frozen_ecs_manifest() {
    let (_engine, batch) = published_empty_batch();
    assert!(cleanup_batch_matches(&batch, 0, 0));
    assert!(!cleanup_batch_matches(&batch, 1, 0));
    assert!(!cleanup_batch_matches(&batch, 0, 1));
}

fn one_entity_failure_ledger(
    world: &mut World,
    entity: Entity,
    expected_parent: Entity,
) -> CleanupLedger {
    let claim = world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .unwrap()
        .claim();
    let manifest = world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .unwrap()
        .manifest()
        .clone();
    let obligations = vec![CleanupObligation {
        identity: CleanupTargetIdentity::Registered(claim),
        manifest,
        expected_parent,
        unit_root: entity,
        authorized_parent: expected_parent,
        allow_missing_components: false,
        terminal_mode: CleanupTerminalMode::Normal,
    }];
    let manifest = FrozenCleanupManifest {
        digest: manifest_digest(&obligations),
        obligations,
        mount_parents: Vec::new(),
        boundary_drifts: Vec::new(),
        pending_boundary_loss_claims: Vec::new(),
    };
    let prepared = CleanupLedger::prepare_physical(None, manifest, &NEXT_LEDGER_DOMAIN).unwrap();
    CleanupLedger::activate(prepared, None)
}

fn frame_fixture() -> RouteFrameVersion {
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut engine = RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
    engine.confirm_host(grant);
    engine.push(RouteLocation::new("/")).unwrap();
    engine.collect();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("frame task expected")
    };
    task.frame().clone()
}

fn registered_entity(world: &mut World, parent: Entity) -> Entity {
    let frame = frame_fixture();
    let outlet = RouteSlotKey::default();
    let slot = frame.slot();
    let entity = world.spawn_empty().id();
    let initial_manifest = ComponentManifest::from_world(world, entity).unwrap();
    let claim = world
        .resource_mut::<RouteEntityRegistry>()
        .register(
            entity,
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                parent: frame.clone(),
                outlet: outlet.clone(),
                slot,
            }),
            initial_manifest.clone(),
            EntityLifecycle::PreparedInert,
        )
        .unwrap();
    world.entity_mut(entity).insert((
        ChildOf(parent),
        RouteOwnershipSeal::new(claim.seal()),
        ChildSlotLifecycle::ProvisionalInert,
        ChildSlotAuthority::new(frame, outlet, slot),
    ));
    world.flush();
    let manifest = ComponentManifest::from_world(world, entity).unwrap();
    assert!(
        world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_manifest_exact(&claim, &initial_manifest, manifest)
    );
    entity
}

#[test]
fn exact_clean_removal_despawns_and_removes_the_registry_claim() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);

    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));

    assert!(world.get_entity(entity).is_err());
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .is_none()
    );
    assert_eq!(ledger.disposition(0), Some(CleanupDisposition::Removed));
}

#[test]
fn absent_exact_generation_never_touches_a_reused_index() {
    #[derive(Component)]
    struct Replacement;

    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
    let replacement = world.despawn_no_free(entity).unwrap();
    world.spawn_at(replacement, Replacement).unwrap();
    assert_eq!(replacement.index(), entity.index());
    assert_ne!(replacement, entity);

    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));

    assert!(world.get::<Replacement>(replacement).is_some());
    assert_eq!(
        ledger.disposition(0),
        Some(CleanupDisposition::AlreadyAbsent)
    );
}

#[test]
fn absent_entity_with_an_alternative_registry_claim_is_stale_not_complete() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
    let original = ledger.obligations[0]
        .identity
        .registered()
        .expect("registered test fixture")
        .clone();
    assert!(world.despawn(entity));
    let alternative = world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_lifecycle(&original, EntityLifecycle::Quarantined)
        .unwrap();

    let attempt = ledger.attempt(&world).unwrap();
    assert!(!ledger.settle(&mut world, attempt));

    assert_eq!(ledger.receipt_count(), 0);
    assert!(!ledger.is_settled());
    assert_eq!(
        world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .unwrap()
            .claim(),
        alternative
    );
}

#[test]
fn stale_living_claim_never_clears_focus_before_settlement_revalidation() {
    for stale_registry_claim in [false, true] {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        world.insert_resource(RouteFocusState::default());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let claim = ledger.obligations[0]
            .identity
            .registered()
            .expect("registered test fixture")
            .clone();
        world.insert_resource(InputFocus::from_entity(entity));
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(entity));
        let attempt = ledger.attempt(&world).unwrap();

        if stale_registry_claim {
            world
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_lifecycle(&claim, EntityLifecycle::Quarantined)
                .unwrap();
        } else {
            let stale_seal = world
                .resource_mut::<RouteEntityRegistry>()
                .issue_unregistered_seal_for_test();
            world
                .entity_mut(entity)
                .insert(RouteOwnershipSeal::new(stale_seal));
        }

        assert!(!ledger.settle(&mut world, attempt));
        assert_eq!(world.resource::<InputFocus>().get(), Some(entity));
        assert_eq!(world.resource::<RouteFocusState>().focused(), Some(entity));
        assert_eq!(ledger.receipt_count(), 0);
    }
}

#[test]
fn detached_exact_entity_uses_removed_detached() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let expected_parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, expected_parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, expected_parent);
    world.entity_mut(entity).remove::<ChildOf>();

    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));

    assert!(world.get_entity(entity).is_err());
    assert_eq!(
        ledger.disposition(0),
        Some(CleanupDisposition::RemovedDetached)
    );
}

#[test]
fn manifest_receipt_and_completion_digests_are_domain_separated() {
    let manifest = StructuredDigest::new(MANIFEST_DOMAIN).finish();
    let attempt = StructuredDigest::new(ATTEMPT_DOMAIN).finish();
    let receipt = StructuredDigest::new(RECEIPT_DOMAIN).finish();
    let completion = StructuredDigest::new(COMPLETION_DOMAIN).finish();
    let values = [manifest, attempt, receipt, completion];
    for (index, value) in values.iter().enumerate() {
        assert!(!values[index + 1..].contains(value));
    }
}

#[test]
fn nested_cleanup_obligation_digest_binds_exact_claim_lifecycle() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let ledger = one_entity_failure_ledger(&mut world, entity, parent);
    let prepared = ledger.obligations[0].clone();
    let quarantined_claim = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&[(
            prepared
                .identity
                .registered()
                .expect("registered test fixture")
                .clone(),
            EntityLifecycle::Quarantined,
        )])
        .unwrap()
        .resulting_claims()[0]
        .clone();
    let mut quarantined = prepared.clone();
    quarantined.identity = CleanupTargetIdentity::Registered(quarantined_claim);

    assert_ne!(
        manifest_digest(&[prepared]),
        manifest_digest(&[quarantined])
    );
}

#[test]
fn reconciliation_superseded_and_foreign_attempts_are_world_zero() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
    let old = ledger.attempt(&world).unwrap();
    let current = ledger.attempt(&world).unwrap();
    let mut foreign = current.clone();
    foreign.ledger_domain = foreign.ledger_domain.wrapping_add(1);
    let before_rejections = cleanup_rejection_snapshot(&world, &ledger, entity, parent);

    assert!(!ledger.settle(&mut world, old));
    assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before_rejections);
    assert!(!ledger.settle(&mut world, foreign));
    assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before_rejections);
    assert!(world.get_entity(entity).is_ok());
    assert_eq!(ledger.receipt_count(), 0);

    assert!(ledger.settle(&mut world, current));
    assert!(world.get_entity(entity).is_err());
}

pub(crate) fn foreign_and_replayed_receipts_are_ledger_zero() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
    let attempt = ledger.attempt(&world).unwrap();
    let foreign_domain = ledger.ledger_domain.wrapping_add(1);
    let foreign = CleanupReceipt::new(
        foreign_domain,
        attempt.revision,
        0,
        CleanupDisposition::Removed,
        &ledger.obligations[0],
    );
    let unknown_obligation = CleanupReceipt::new(
        ledger.ledger_domain,
        attempt.revision,
        1,
        CleanupDisposition::Removed,
        &ledger.obligations[0],
    );
    let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
    let world_before = world
        .iter_entities()
        .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
        .collect::<Vec<_>>();
    let before = cleanup_rejection_snapshot(&world, &ledger, entity, parent);

    assert!(!ledger.accept_receipt(&attempt, foreign));
    assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before);
    assert!(!ledger.accept_receipt(&attempt, unknown_obligation));
    assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before);
    assert_eq!(ledger.receipt_count(), 0);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records(),
        registry_before
    );
    assert_eq!(
        world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>(),
        world_before
    );

    assert!(ledger.settle(&mut world, attempt.clone()));
    let replay = ledger.receipts[0].clone().unwrap();
    let receipts_before = ledger.receipts.clone();
    let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
    let world_before = world
        .iter_entities()
        .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
        .collect::<Vec<_>>();
    let before = cleanup_rejection_snapshot(&world, &ledger, entity, parent);
    assert!(!ledger.accept_receipt(&attempt, replay));
    assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before);
    assert_eq!(ledger.receipts, receipts_before);
    assert_eq!(ledger.receipt_count(), 1);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records(),
        registry_before
    );
    assert_eq!(
        world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>(),
        world_before
    );
}

#[test]
fn reconciliation_foreign_unknown_and_replayed_receipts_are_world_zero() {
    foreign_and_replayed_receipts_are_ledger_zero();
}

#[test]
fn reconciliation_cleanup_receipt_observation_is_exact_once() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);

    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));
    let (obligation, disposition) = ledger
        .next_unobserved_receipt()
        .expect("settlement must expose one unobserved receipt");

    assert!(ledger.mark_receipt_observed(obligation, disposition));
    assert!(ledger.next_unobserved_receipt().is_none());
    let before_duplicate = cleanup_rejection_snapshot(&world, &ledger, entity, parent);
    assert!(!ledger.mark_receipt_observed(obligation, disposition));
    assert_cleanup_rejection_is_zero(&world, &ledger, entity, parent, &before_duplicate);
}

#[test]
fn reconciliation_partial_receipt_precedes_aggregate_completion() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let parent = world.spawn_empty().id();
    let entity = registered_entity(&mut world, parent);
    let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
    ledger.obligations.push(ledger.obligations[0].clone());
    ledger.receipts.push(None);
    ledger.manifest_digest = manifest_digest(&ledger.obligations);
    let attempt = ledger.attempt(&world).unwrap();
    let receipt = CleanupReceipt::new(
        ledger.ledger_domain,
        attempt.revision,
        0,
        CleanupDisposition::Removed,
        &ledger.obligations[0],
    );

    assert!(ledger.accept_receipt(&attempt, receipt));
    ledger.finish_if_settled();
    assert!(!ledger.is_settled());
    assert_eq!(
        ledger.next_unobserved_receipt(),
        Some((0, CleanupDisposition::Removed))
    );
    assert!(ledger.mark_receipt_observed(0, CleanupDisposition::Removed));
    assert!(ledger.next_unobserved_receipt().is_none());
}

#[test]
fn router_cleanup_observation_counts_terminal_quarantine_events_not_receipts() {
    let mut observation = RouterCleanupObservation::default();
    observation.record(CleanupDisposition::Removed, false);
    observation.record(CleanupDisposition::QuarantinedProtectedParent, true);
    observation.record(CleanupDisposition::QuarantinedProtectedParent, false);

    let pending = observation.summary(true);
    assert!(pending.foreground_pending());
    assert_eq!(pending.quarantine_count(), 1);
    assert_eq!(
        pending.last_disposition(),
        Some(bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedProtectedParent)
    );
}

#[test]
fn registered_same_batch_reparent_uses_actual_leaf_order_and_individual_removal_receipts() {
    use bevy_ecs::prelude::{Despawn, On, ResMut, Resource};

    #[derive(Component)]
    struct RecordDespawn;
    #[derive(Resource, Default)]
    struct DespawnOrder(Vec<Entity>);

    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    world.insert_resource(DespawnOrder::default());
    world.add_observer(
        |event: On<Despawn, RecordDespawn>, mut order: ResMut<DespawnOrder>| {
            order.0.push(event.entity);
        },
    );
    let host = world.spawn_empty().id();
    let frame = frame_fixture();
    let mut obligations = Vec::new();
    for local_index in 0..2 {
        let entity = world.spawn_empty().id();
        let initial = ComponentManifest::from_world(&world, entity).unwrap();
        let claim = world
            .resource_mut::<RouteEntityRegistry>()
            .register(
                entity,
                RegistryOwner::Committed(EntityOwner::FrameOutput {
                    frame: frame.clone(),
                    output: 1,
                    local_index,
                }),
                initial.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        world.entity_mut(entity).insert((
            ChildOf(host),
            RouteOwnershipSeal::new(claim.seal()),
            FrameLifecycle(claim.lifecycle()),
            RecordDespawn,
        ));
        world.flush();
        let manifest = ComponentManifest::from_world(&world, entity).unwrap();
        assert!(
            world
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_manifest_exact(&claim, &initial, manifest.clone())
        );
        obligations.push(CleanupObligation {
            identity: CleanupTargetIdentity::Registered(claim),
            manifest,
            expected_parent: host,
            unit_root: entity,
            authorized_parent: host,
            allow_missing_components: false,
            terminal_mode: CleanupTerminalMode::Normal,
        });
    }
    let parent = obligations[0].entity();
    let child = obligations[1].entity();
    let manifest = FrozenCleanupManifest {
        digest: manifest_digest(&obligations),
        obligations,
        mount_parents: Vec::new(),
        boundary_drifts: Vec::new(),
        pending_boundary_loss_claims: Vec::new(),
    };
    let prepared = CleanupLedger::prepare_physical(None, manifest, &NEXT_LEDGER_DOMAIN).unwrap();
    let mut ledger = CleanupLedger::activate(prepared, None);

    // Actual relationships change after freezing the manifest. Both exact claims
    // still belong to this batch; the ordinary production classifier decides
    // whether that remains removable instead of receiving a synthetic verdict.
    world.entity_mut(child).insert(ChildOf(parent));
    world.flush();
    let attempt = ledger.attempt(&world).unwrap();
    for (index, evidence) in &attempt.evidence {
        assert!(
            matches!(evidence, CleanupEvidence::Exact {
            foreign_descendants,
            foreign_components,
            missing_components,
            protected_parent: None,
            relationship_cycle: false,
            ..
        } if foreign_descendants.is_empty() && foreign_components.is_empty() && missing_components.is_empty()),
            "registered obligation {index} must be accepted by the real classifier: {evidence:?}"
        );
    }
    assert!(
        matches!(&attempt.evidence.iter().find(|(index, _)| *index == 1).unwrap().1,
        CleanupEvidence::Exact { actual_parent: Some(actual), .. } if *actual == parent)
    );
    assert!(ledger.settle(&mut world, attempt));
    assert_eq!(world.resource::<DespawnOrder>().0, vec![child, parent]);
    assert_eq!(ledger.receipt_count(), 2);
    assert_eq!(ledger.disposition(0), Some(CleanupDisposition::Removed));
    assert_eq!(
        ledger.disposition(1),
        Some(CleanupDisposition::RemovedDetached)
    );
    assert!(ledger.is_settled());
    for entity in [parent, child] {
        assert!(world.get_entity(entity).is_err());
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .is_none()
        );
    }
    assert!(world.get_entity(host).is_ok());
}
