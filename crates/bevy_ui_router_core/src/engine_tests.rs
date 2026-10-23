use super::CoreBackTargetPermanentBeginOwner;
use super::*;
use crate::admission::HostLease;
use std::collections::VecDeque;

use super::{
    ActionSnapshot, BackNoTargetSourceDriftSettlementOutcome, BackNoTargetSourceRefreezeOutcome,
    BackTargetPermanentSettlementOutcome, BackTargetResumeAction, BackTargetResumeOutcome,
    BackTargetStart, CleanupBatchProjection, CleanupError, CommitError,
    CoreAuthorityCleanupCompletionFacts, CoreBackNoTargetPublicationPreparationFailureProjection,
    CoreBackNoTargetReadyMismatchProjection, CoreNormalCleanupCompletionFacts,
    CorePostCommitAuthorityBeginPermanentEvidence, CorePostCommitAuthorityContradiction,
    CorePostCommitAuthorityPreparationSource, CorePostCommitCleanupStageProjection,
    CorePostCommitCompoundBatchOwner, CorePostCommitCompoundMismatchEvidence,
    CorePostCommitPlannedCleanupProjection, CorePostCommitPlannedHostEdgeProjection,
    CorePostCommitSettlementAttestation, CorePostCommitSettlementKeyProjection,
    CorePostCommitSettlementPermitOutcome, CorePostCommitSettlementPlanBuilder,
    CorePostCommitSettlementPlanDescriptor, CorePostCommitSettlementRecordOutcome,
    CorePostCommitSettlementRole, CorePostCommitSettlementTraceMismatchKind,
    CorePostCommitSettlementTraceProgress, CorePostCommitSettlementTraceProjection,
    CorePreparationReceiverMismatchProjection, CorePreparedPostCommitAuthorityTransfer,
    CoreRetiredHostEdgeCompletionFacts, EngineAction, FailureCleanupCompletionDisposition,
    FrameMountTarget, MountError, NavigationCommand, PostCommitAuthorityAcceptanceOutcome,
    PostCommitAuthorityBeginCoreOutcome, PostCommitAuthorityCause,
    PostCommitAuthorityPreparationOutcome, PostCommitCompoundCompletionCoreOutcome,
    PostCommitCompoundCompletionInputs, PostCommitCompoundCompletionPreparationOutcome,
    PrivateRequestLookup, PushError, QueuedNavigation, QueuedReservation,
    ROUTER_DIAGNOSTIC_RETENTION, ReportError, RootTaskIdentity, RouterEngine, TERMINAL_RETENTION,
    WorkStage, navigation_trace_digest, occupied_child_slots,
};
use crate::admission::{AdmissionError, HostAdmissionIssuer};
use crate::cleanup::{
    CleanupBatchToken, CleanupCompletion, CleanupCompletionDigest, CleanupManifestDigest,
    CleanupManifestSeal,
};
use crate::diagnostics::ReportDisposition;
use crate::frame::{
    FrameOrigin, LogicalChildSlotBinding, ProjectedChildSlotRoleKind, ProjectedEnteringSlot,
    RouteFrameVersion,
};
use crate::history_authority::{
    BackResolutionEvidenceClassification, BackResolutionVerifyOutcome,
    BackTargetPermanentCauseSnapshot, VerifiedBackResolutionOwnerHandoff,
};
use crate::ids::{
    CleanupBatchId, NavigationRequestId, ReservationCounter, RouteFrameId, RouteGeneration,
    RouteHostId, RouteSlotId,
};
use crate::location::RouteLocation;
use crate::meta::ErrorBoundaryKey;
use crate::pattern::RoutePattern;
use crate::presentation::{
    ActivePresentation, ActivePresentationKind, RouterAvailability, RouterUnavailableReason,
};
use crate::request::{
    NavigationFailure, NavigationFailureKind, NavigationPhase, RequestRecordError, RequestStatus,
};
use crate::route_tree::{RouteNodeOptions, RouteTree};
use crate::slot::RouteSlotKey;
use crate::trace::{NavigationTraceEvent, NavigationTraceEventKind};
use crate::view_key::RouteViewKey;

const NONZERO_OBLIGATION_COUNT: usize = 3;

#[test]
fn projectionless_fallback_active_source_refreezes_current_active_branch_and_bindings() {
    for drift in [
        "active",
        "branch",
        "bindings",
        "old-epoch",
        "current-location",
    ] {
        let (mut engine, mut issuer, lease) = ready_engine_with_tree_and_authority(tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let lease_identity = lease.retirement_projection();
        let roots = engine
            .active()
            .frames()
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        engine.push(RouteLocation::new("/unmatched")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("real unbound fallback task");
        };
        let trigger = engine
            .host_loss_trigger_for_unstarted_fallback_with_active(task.identity())
            .unwrap();
        match drift {
            "active" => engine.testing_set_active_generation(RouteGeneration::testing_max()),
            "branch" => engine.active_branch = None,
            "bindings" => {
                assert!(!engine.active_child_slots.is_empty());
                engine.active_child_slots.clear();
            }
            "old-epoch" => engine.current.as_mut().unwrap().identity.host_epoch += 1,
            _ => engine.current.as_mut().unwrap().location = RouteLocation::new("/changed-source"),
        }
        let state = engine.state.clone();
        let error = engine
            .begin_failed_authority_settlement_for_cause_retaining(
                &mut issuer,
                lease,
                trigger,
                PreCommitAuthorityCause::HostLost,
                CleanupManifestSeal::new(roots, manifest_digest(0xFAB1), 2),
                |_, _| panic!("drifted source must not install physical ownership"),
            )
            .unwrap_err();
        assert_eq!(error.kind(), BeginAuthoritySettlementErrorKind::Stale);
        assert_eq!(error.into_owner().retirement_projection(), lease_identity);
        assert_eq!(engine.state, state);
        assert!(issuer.has_current());
        assert!(engine.live_failed_authority_trigger_revision.is_none());
    }
}

#[test]
fn idle_host_loss_core_rejects_foreign_queued_and_invalid_manifest_without_consuming_lease() {
    use super::idle_host_loss::IdleHostLossErrorKind;
    let (mut engine, mut issuer, lease) = ready_engine_with_tree_and_authority(tree());
    let identity = lease.retirement_projection();
    let before = engine.state.clone();
    let (_, mut foreign_issuer, _) = ready_engine_with_tree_and_authority(tree());
    let (_, lease) = engine
        .begin_idle_host_loss(&mut foreign_issuer, lease, &[], 31, 1, |_| {
            panic!("foreign issuer cannot install")
        })
        .unwrap_err();
    assert_eq!(lease.retirement_projection(), identity);
    assert_eq!(engine.state, before);
    let (kind, lease) = engine
        .begin_idle_host_loss(&mut issuer, lease, &[], 31, 0, |_| {
            panic!("top-slot obligation is mandatory")
        })
        .unwrap_err();
    assert_eq!(kind, IdleHostLossErrorKind::InvalidManifest);
    assert_eq!(lease.retirement_projection(), identity);
    let queued = engine.push(RouteLocation::new("/items/1")).unwrap();
    assert!(!engine.idle_host_loss_eligible());
    let before = engine.request(queued).unwrap().clone();
    let (kind, lease) = engine
        .begin_idle_host_loss(&mut issuer, lease, &[], 31, 1, |_| {
            panic!("queued request owns foreground")
        })
        .unwrap_err();
    assert_eq!(kind, IdleHostLossErrorKind::ReceiverMismatch);
    assert_eq!(lease.retirement_projection(), identity);
    assert_eq!(engine.request(queued), Some(&before));
    assert!(issuer.has_current());
}

#[test]
fn idle_host_loss_core_completion_returns_exact_owner_and_preserves_terminal_success() {
    use super::idle_host_loss::{IdleHostLossPhysicalDisposition, IdleHostLossPhysicalFacts};
    for with_active in [false, true] {
        let (mut engine, mut issuer, lease) = ready_engine_with_tree_and_authority(tree());
        if with_active {
            engine.push(RouteLocation::new("/items/1")).unwrap();
            settle_current(&mut engine);
        }
        assert!(engine.idle_host_loss_eligible());
        let records = engine.records.clone();
        let history = engine.history.snapshot().clone();
        let roots = engine
            .state
            .active()
            .frames()
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        let mut owner = None;
        assert!(
            engine
                .begin_idle_host_loss(&mut issuer, lease, &roots, 71, 5, |value| owner =
                    Some(value))
                .is_ok()
        );
        assert!(!issuer.has_current());
        assert!(!engine.idle_host_loss_eligible());
        assert_eq!(
            engine.state.availability(),
            &RouterAvailability::RecoveringAuthority
        );
        assert_eq!(engine.state.active().kind(), ActivePresentationKind::None);
        assert!(engine.current.is_none());
        assert!(engine.action.is_none());
        assert_eq!(engine.records, records);
        assert_eq!(engine.router_diagnostics.last().unwrap().request(), None);
        let facts = IdleHostLossPhysicalFacts {
            manifest_digest: 71,
            obligation_count: 5,
            disposition: IdleHostLossPhysicalDisposition::Settled {
                completion_digest: 99,
                settled_count: 5,
            },
        };
        let bad = IdleHostLossPhysicalFacts {
            obligation_count: 4,
            ..facts
        };
        let owner = match engine.complete_idle_host_loss(owner.unwrap(), bad, |_| {
            panic!("wrong physical manifest must not install")
        }) {
            Err(owner) => owner,
            Ok(_) => panic!("wrong manifest must return owner"),
        };
        assert!(engine.complete_idle_host_loss(owner, facts, |_| ()).is_ok());
        assert!(engine.pending_idle_host_loss.is_none());
        assert!(engine.current.is_none());
        assert!(engine.action.is_none());
        assert_eq!(engine.records, records);
        assert_eq!(engine.history.snapshot(), history);
        assert_eq!(
            engine.state.availability(),
            &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
        );
    }
}

fn tree() -> RouteTree {
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    builder
        .route_with_options(
            root,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("items").unwrap()),
        )
        .unwrap();
    builder
        .route_with_options(
            root,
            RoutePattern::path("about").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("about").unwrap()),
        )
        .unwrap();
    builder.build().unwrap()
}

fn reconciliation_tree() -> RouteTree {
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    let items = builder
        .route_with_options(
            root,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("items").unwrap()),
        )
        .unwrap();
    builder
        .route_with_options(
            items,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    builder.build().unwrap()
}

fn ready_engine() -> RouterEngine {
    ready_engine_with_tree(tree())
}

fn ready_engine_with_tree(tree: RouteTree) -> RouterEngine {
    ready_engine_with_tree_and_authority(tree).0
}

fn ready_engine_with_tree_and_authority(
    tree: RouteTree,
) -> (RouterEngine, HostAdmissionIssuer, HostLease) {
    let mut engine = RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();
    let lease = confirmation.into_lease();
    (engine, issuer, lease)
}

fn publish_current(engine: &mut RouterEngine) -> crate::cleanup::CleanupBatchToken {
    let action = engine.action().unwrap();
    publish_action(engine, action)
}

fn publish_action(
    engine: &mut RouterEngine,
    action: EngineAction,
) -> crate::cleanup::CleanupBatchToken {
    let ticket = drive_to_commit(engine, action);
    let sealed = seal_current(engine, ticket);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(engine, sealed, || {}).unwrap();
    cleanup
}

#[test]
fn back_no_target_publication_preparation_retains_typed_failure_evidence() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();

    let missing = match RouterEngine::prepare_back_no_target_publication(
        VecDeque::new(),
        engine.state.clone(),
        request,
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("a missing request record must not prepare publication"),
    };
    assert_eq!(
        RouterEngine::back_no_target_publication_preparation_mismatch(request, missing),
        CorePreparationReceiverMismatchProjection::BackNoTargetReady(
            CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                request,
                failure: CoreBackNoTargetPublicationPreparationFailureProjection::MissingRecord,
            }
        )
    );

    let trace = RouterEngine::back_no_target_publication_preparation_mismatch(
        request,
        CoreBackNoTargetPublicationPreparationFailureProjection::HistoryIntentRejectedTrace(
            RequestRecordError::TraceClosed,
        ),
    );
    assert_eq!(
            trace,
            CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                    request,
                    failure:
                        CoreBackNoTargetPublicationPreparationFailureProjection::HistoryIntentRejectedTrace(
                            RequestRecordError::TraceClosed
                        ),
                }
            )
        );

    let cleanup_pending_trace = RouterEngine::back_no_target_publication_preparation_mismatch(
        request,
        CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTrace(
            RequestRecordError::TraceClosed,
        ),
    );
    assert_eq!(
        cleanup_pending_trace,
        CorePreparationReceiverMismatchProjection::BackNoTargetReady(
            CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                request,
                failure:
                    CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTrace(
                        RequestRecordError::TraceClosed
                    ),
            }
        )
    );

    let mut terminal_records = engine.records.clone();
    let record = terminal_records
        .iter_mut()
        .find(|record| record.id() == request)
        .expect("the real request record remains retained");
    record
        .transition(RequestStatus::Failed(NavigationFailure::new(
            NavigationFailureKind::AuthorityLost,
            "typed preparation failure fixture",
        )))
        .unwrap();
    let transition = match RouterEngine::prepare_back_no_target_publication(
        terminal_records,
        engine.state.clone(),
        request,
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("a terminal request record must not re-enter CleanupPending"),
    };
    assert_eq!(
            RouterEngine::back_no_target_publication_preparation_mismatch(request, transition),
            CorePreparationReceiverMismatchProjection::BackNoTargetReady(
                CoreBackNoTargetReadyMismatchProjection::PublicationPreparation {
                    request,
                    failure:
                        CoreBackNoTargetPublicationPreparationFailureProjection::CleanupPendingTransition(
                            RequestRecordError::AlreadyTerminal
                        ),
                }
            )
        );
}

fn drive_to_commit(engine: &mut RouterEngine, mut action: EngineAction) -> super::CommitTicket {
    loop {
        match action {
            EngineAction::FrameTask(task) => {
                let next_outlet = engine.current.as_ref().and_then(|current| {
                    current
                        .branch
                        .as_ref()
                        .and_then(|branch| branch.routes().get(task.identity().branch_index() + 1))
                        .map(|matched| matched.slot().clone())
                });
                engine
                    .accept_applied_report(
                        task.into_applied_report(next_outlet.into_iter().collect()),
                    )
                    .unwrap();
                action = engine.action().expect("accepted report produces an action");
            }
            EngineAction::MountRequest(request) => {
                let grant = engine.resolve_mount(request).unwrap();
                let receipt =
                    grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(1));
                let permit = engine.accept_mount_receipt(receipt).unwrap();
                let completion =
                    permit.complete(crate::mount::MountRegistrationDigest::from_opaque(2));
                engine.complete_mount_registration(completion).unwrap();
                action = engine
                    .action()
                    .expect("mount confirmation produces child task");
            }
            EngineAction::FailureCleanup(_) => {
                panic!("successful drive cannot produce failure cleanup")
            }
            EngineAction::Commit(ticket) => break ticket,
            EngineAction::BackResolution(_) => {
                panic!("commit helper does not drive unresolved back")
            }
        }
    }
}

fn back_target_permanent_begin_fixture() -> (
    RouterEngine,
    crate::history_authority::BackResolutionScopeProjection,
    super::CommitTicket,
    Vec<RouteFrameVersion>,
    HostAdmissionIssuer,
    HostLease,
) {
    let (mut engine, issuer, lease) = ready_engine_with_tree_and_authority(tree());
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();
    settle_current(&mut engine);

    let world = engine
        .claim_history_admission_world()
        .expect("the confirmed fixture issues one history world seal");
    let stamp = engine
        .history_admission_stamp(&world)
        .expect("the two-entry history admits Back");
    let admission = engine
        .prepare_back_admission(&world, &stamp)
        .expect("the target Back exact-refreezes the live history");
    let accepted = engine.enqueue_back(admission).unwrap();
    engine.collect();
    let handoff = match engine.action().unwrap() {
        EngineAction::BackResolution(handoff) => handoff,
        _ => panic!("the accepted Back must produce a real resolution handoff"),
    };
    let (_request, view) = accepted.into_parts();
    let verified = match engine.verify_back_resolution_handoff(view, handoff) {
        BackResolutionVerifyOutcome::Matched(verified) => verified,
        BackResolutionVerifyOutcome::Mismatch { .. } => {
            panic!("the exact Back lineage must verify")
        }
    };
    let (_view, owner) = verified.into_parts();
    let target = match owner {
        VerifiedBackResolutionOwnerHandoff::Target(target) => target,
        VerifiedBackResolutionOwnerHandoff::NoTarget(_)
        | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
            panic!("the two-entry history must resolve its predecessor")
        }
    };
    let resume = match engine.resume_back_target_with_retirement(target, || {}) {
        BackTargetResumeOutcome::Resumed(BackTargetStart::Ready(resume)) => resume,
        BackTargetResumeOutcome::Resumed(BackTargetStart::FailureCleanup(_))
        | BackTargetResumeOutcome::Permanent(_)
        | BackTargetResumeOutcome::Mismatch(_) => {
            panic!("the exact target Back must reach a normal action")
        }
    };
    let super::BackTargetResume {
        action,
        execution: _,
        continuation,
    } = resume;
    let action = match action {
        BackTargetResumeAction::FrameTask(task) => EngineAction::FrameTask(task),
        BackTargetResumeAction::MountRequest(request) => EngineAction::MountRequest(request),
        BackTargetResumeAction::Commit(ticket) => EngineAction::Commit(ticket),
    };
    let ticket = drive_to_commit(&mut engine, action);
    let logical_roots = engine
        .current
        .as_ref()
        .expect("the ReadyToCommit fixture retains current work")
        .cleanup_roots
        .clone();
    (engine, continuation, ticket, logical_roots, issuer, lease)
}

fn real_retired_host_authority() -> (
    crate::ids::RouterIdentity,
    crate::admission::HostVersion,
    RouteSlotId,
    crate::admission::RetiredHostAuthority,
) {
    let mut donor = RouterEngine::install(tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = donor.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = donor
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    donor.confirm_host(&issuer, &confirmation).unwrap();
    let router = donor.router;
    let version = confirmation.version().clone();
    let top_slot = confirmation.top_slot();
    let authority = issuer.retire_current(confirmation.into_lease()).unwrap();
    assert!(authority.matches(router, &version, top_slot));
    (router, version, top_slot, authority)
}

fn real_pending_mount() -> super::PendingMount {
    let mut donor = ready_engine();
    donor.push(RouteLocation::new("/items/1")).unwrap();
    let task = match donor.action().unwrap() {
        EngineAction::FrameTask(task) => task,
        _ => panic!("the nested route must begin with its real root frame task"),
    };
    let next_outlet = donor
        .current
        .as_ref()
        .and_then(|current| current.branch.as_ref())
        .and_then(|branch| branch.routes().get(task.identity().branch_index() + 1))
        .map(|matched| matched.slot().clone())
        .expect("the nested route retains its real child outlet");
    donor
        .accept_applied_report(task.into_applied_report(vec![next_outlet]))
        .unwrap();
    assert!(matches!(
        donor.action.as_ref(),
        Some(EngineAction::MountRequest(_))
    ));
    donor
        .current
        .as_mut()
        .and_then(|current| current.pending_mount.take())
        .expect("the real mount request installs its pending mount fact")
}

#[test]
fn back_target_permanent_begin_rejects_noncanonical_state_frozen_before_acceptance() {
    #[derive(Clone, Copy, Debug)]
    enum NoncanonicalFact {
        CurrentLocation,
        LogicalRoots,
        Reconciliation,
        Availability,
        PendingRelation,
    }

    for fact in [
        NoncanonicalFact::CurrentLocation,
        NoncanonicalFact::LogicalRoots,
        NoncanonicalFact::Reconciliation,
        NoncanonicalFact::Availability,
        NoncanonicalFact::PendingRelation,
    ] {
        let (mut engine, scope, ticket, actual_logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let mut submitted_logical_roots = actual_logical_roots.clone();
        match fact {
            NoncanonicalFact::CurrentLocation => {
                let requested = engine
                    .records
                    .iter()
                    .find(|record| record.id() == scope.request())
                    .and_then(|record| record.requested())
                    .cloned()
                    .expect("the Back request retains its requested target");
                let current = engine
                    .current
                    .as_mut()
                    .expect("the fixture retains ReadyToCommit work");
                assert_eq!(current.location, requested);
                current.location = RouteLocation::new("/items/noncanonical-current");
            }
            NoncanonicalFact::LogicalRoots => {
                assert!(
                    !actual_logical_roots.is_empty(),
                    "the real Back splice fixture must retain cleanup roots"
                );
                submitted_logical_roots.clear();
            }
            NoncanonicalFact::Reconciliation => {
                assert!(
                    engine
                        .current
                        .as_ref()
                        .and_then(|current| current.reconciliation.as_ref())
                        .is_some(),
                    "the real Back fixture retains its reconciliation proof"
                );
                assert!(
                    engine.active_branch.take().is_some(),
                    "the seam changes the real active-side reconciliation input"
                );
            }
            NoncanonicalFact::Availability => {
                let host = engine
                    .state
                    .host()
                    .cloned()
                    .expect("the host remains exact");
                let top_slot = engine.state.top_slot().expect("the top slot remains exact");
                assert!(engine.state.begin_authority_recovery(&host, top_slot));
            }
            NoncanonicalFact::PendingRelation => {
                let mut donor = ready_engine();
                donor.push(RouteLocation::new("/")).unwrap();
                settle_current(&mut donor);
                engine.testing_move_back_target_busy_receiver_slot_from(
                    &mut donor,
                    super::BackTargetBusyReceiverSlot::PendingNoTargetBack,
                );
                assert!(engine.pending_no_target_back.is_some());
            }
        }

        let settlement = engine
            .prepare_back_target_permanent_settlement_owner(scope, submitted_logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);
        let installation = format!("{fact:?}");

        let evidence = match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            submitted_logical_roots,
            0x4341_4e4f_4e49_4341,
            NONZERO_OBLIGATION_COUNT,
            installation.clone(),
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation: returned_installation,
                evidence,
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(returned_installation, installation);
                evidence
            }
            super::BackTargetPermanentBeginCoreOutcome::Begun
            | super::BackTargetPermanentBeginCoreOutcome::ReceiverMismatch { .. }
            | super::BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch { .. } => {
                panic!("{fact:?} must not pass or be downgraded to receiver/argument mismatch")
            }
        };
        assert_eq!(callback_calls.get(), 0, "fact: {fact:?}");
        assert_eq!(engine.snapshot(), before, "fact: {fact:?}");

        let evidence = format!("{evidence:?}");
        assert!(
            evidence.contains("CanonicalRelationContradiction"),
            "{fact:?} must report purpose-specific typed canonical relation evidence, got {evidence}"
        );
        assert!(
            evidence.contains("expected:") && evidence.contains("observed:"),
            "{fact:?} canonical evidence must retain named expected/observed relations"
        );
        assert!(
            !evidence.contains("CurrentStateContradiction"),
            "{fact:?} was already noncanonical when frozen and is not ordinary snapshot drift"
        );
    }
}

#[test]
fn back_target_permanent_begin_rejects_owned_noncanonical_relations_losslessly() {
    {
        let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let (router, version, top_slot, retired_host) = real_retired_host_authority();
        engine
            .current
            .as_mut()
            .expect("the fixture retains ReadyToCommit work")
            .retired_host = Some(retired_host);

        let settlement =
            engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);
        let installation = "owned-retired-host";

        let evidence = match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            logical_roots,
            0x4f57_4e45_445f_524c,
            NONZERO_OBLIGATION_COUNT,
            installation,
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation: returned_installation,
                evidence,
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(returned_installation, installation);
                evidence
            }
            super::BackTargetPermanentBeginCoreOutcome::Begun
            | super::BackTargetPermanentBeginCoreOutcome::ReceiverMismatch { .. }
            | super::BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch { .. } => {
                panic!("owned retired-host contradiction must fail closed")
            }
        };
        match evidence {
                super::BackTargetPermanentBeginCorePermanentEvidence::CanonicalRelationContradiction {
                    expected,
                    observed,
                } => {
                    assert!(expected.retired_host.is_none());
                    assert!(
                        observed
                            .retired_host
                            .as_ref()
                            .is_some_and(|projection| projection.matches(
                                router,
                                &version,
                                top_slot
                            )),
                        "canonical evidence must retain the exact retired-host identity"
                    );
                }
                other => panic!(
                    "owned retired-host contradiction needs canonical relation evidence, got {other:?}"
                ),
            }
        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), before);
        assert!(
            engine
                .current
                .as_ref()
                .and_then(|current| current.retired_host.as_ref())
                .is_some_and(|authority| authority.matches(router, &version, top_slot)),
            "rejection must leave the sole authority in current"
        );
    }

    {
        let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
            back_target_permanent_begin_fixture();
        let pending_mount = real_pending_mount();
        let expected_child_index = pending_mount.child_index;
        let expected_parent = pending_mount.parent.clone();
        let expected_outlet = pending_mount.outlet.clone();
        let expected_parent_source = pending_mount.parent_source;
        engine
            .current
            .as_mut()
            .expect("the fixture retains ReadyToCommit work")
            .pending_mount = Some(pending_mount);

        let settlement =
            engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
        let expected_scope = settlement.scope.clone();
        let expected_current = settlement.expected_current.clone();
        let expected_request = settlement.expected_request.clone();
        let expected_roots = settlement.logical_roots.clone();
        let before = engine.snapshot();
        let callback_calls = std::cell::Cell::new(0usize);
        let installation = "owned-pending-mount";

        let evidence = match engine.begin_back_target_permanent_cleanup(
            &mut issuer,
            CoreBackTargetPermanentBeginOwner::new(settlement, lease),
            &ticket,
            logical_roots,
            0x4f57_4e45_445f_524c,
            NONZERO_OBLIGATION_COUNT,
            installation,
            |_, _| callback_calls.set(callback_calls.get() + 1),
        ) {
            super::BackTargetPermanentBeginCoreOutcome::Permanent {
                owner,
                installation: returned_installation,
                evidence,
            } => {
                let (owner, _lease) = owner.into_parts();
                assert_eq!(owner.scope, expected_scope);
                assert_eq!(owner.expected_current, expected_current);
                assert_eq!(owner.expected_request, expected_request);
                assert_eq!(owner.logical_roots, expected_roots);
                assert_eq!(returned_installation, installation);
                evidence
            }
            super::BackTargetPermanentBeginCoreOutcome::Begun
            | super::BackTargetPermanentBeginCoreOutcome::ReceiverMismatch { .. }
            | super::BackTargetPermanentBeginCoreOutcome::LogicalRootsMismatch { .. } => {
                panic!("owned pending-mount contradiction must fail closed")
            }
        };
        let observed = match evidence {
                super::BackTargetPermanentBeginCorePermanentEvidence::CanonicalRelationContradiction {
                    expected,
                    observed,
                } => {
                    assert!(expected.pending_mount.is_none());
                    observed
                        .pending_mount
                        .expect("canonical evidence must retain the pending-mount projection")
                }
                other => panic!(
                    "owned pending-mount contradiction needs canonical relation evidence, got {other:?}"
                ),
            };
        assert_eq!(observed.child_index, expected_child_index);
        assert_eq!(observed.parent, expected_parent);
        assert_eq!(observed.outlet, expected_outlet);
        assert_eq!(observed.parent_source, expected_parent_source);
        assert_eq!(callback_calls.get(), 0);
        assert_eq!(engine.snapshot(), before);
        let current = engine
            .current
            .as_ref()
            .and_then(|current| current.pending_mount.as_ref())
            .expect("rejection must leave the pending mount in current");
        assert_eq!(current.child_index, expected_child_index);
        assert_eq!(current.parent, expected_parent);
        assert_eq!(current.outlet, expected_outlet);
        assert_eq!(current.parent_source, expected_parent_source);
    }

    fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
        let tail = &source[source
            .find(marker)
            .expect("typed contract item must remain named")..];
        let open = tail
            .find('{')
            .expect("typed contract item must have a body");
        let mut depth = 0usize;
        for (offset, character) in tail[open..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &tail[..open + offset + character.len_utf8()];
                    }
                }
                _ => {}
            }
        }
        panic!("typed contract item body must close")
    }

    fn compact(source: &str) -> String {
        source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    }

    let admission_and_tests = include_str!("../../bevy_ui_router_core/src/admission.rs");
    let admission = &admission_and_tests[..admission_and_tests
        .find("mod tests {")
        .expect("the admission production/test boundary must remain explicit")];
    let retired_projection = braced_item(admission, "struct RetiredHostAuthorityProjection");
    let compact_retired_projection = compact(retired_projection);
    let exact_projection = compact_retired_projection.contains("key:AdmissionKey")
        && compact_retired_projection.contains("version:HostVersion")
        && compact_retired_projection.contains("top_slot:RouteSlotId");
    assert!(
        exact_projection
            && !compact_retired_projection.contains(":RetiredHostAuthority,")
            && !compact_retired_projection.contains(":Option<RetiredHostAuthority>")
            && !compact_retired_projection.contains("bool")
            && !compact_retired_projection.contains(":()"),
        "admission must expose an exact, typed, non-authorizing retired-host identity projection"
    );
    let admission_key = compact(braced_item(admission, "struct AdmissionKey"));
    for exact_field in [
        "issuer:AdmissionIssuerId",
        "router:RouterIdentity",
        "sequence:u64",
    ] {
        assert!(
            admission_key.contains(exact_field),
            "AdmissionKey must retain exact {exact_field} identity"
        );
    }
    let retired_authority_marker = admission
        .find("struct RetiredHostAuthority")
        .expect("retired-host authority must remain explicit");
    let retired_authority_derive = &admission[..retired_authority_marker][admission
        [..retired_authority_marker]
        .rfind("#[derive(")
        .expect("retired-host authority must retain an explicit derive list")..];
    assert!(
        !retired_authority_derive.contains("Clone"),
        "linear retired-host authority must not implement Clone"
    );
    let compact_admission = compact(admission);
    assert_eq!(
        compact_admission.matches("RetiredHostAuthority{").count(),
        3,
        "retired-host authority may appear only in its declaration, sole retirement construction, and impl"
    );
    assert_eq!(
        compact_admission
            .matches("Ok(RetiredHostAuthority{")
            .count(),
        1,
        "retire_current must remain the sole retired-host authority constructor"
    );
    let retired_authority_impl = compact(braced_item(admission, "impl RetiredHostAuthority"));
    let projection_method = compact(braced_item(&retired_authority_impl, "fnprojection"));
    for exact_mapping in [
        "key:self.identity.key",
        "version:self.identity.version.clone()",
        "top_slot:self.identity.top_slot",
    ] {
        assert!(
            projection_method.contains(exact_mapping),
            "borrowed projection must map exact authority identity: {exact_mapping}"
        );
    }

    let engine_and_tests = include_str!("../../bevy_ui_router_core/src/engine.rs");
    let engine = &engine_and_tests[..engine_and_tests
        .find("#[cfg(test)]\n#[path = \"engine_tests.rs\"]\nmod tests;")
        .expect("the engine production/test boundary must remain explicit")];
    let current_projection = braced_item(
        engine,
        "struct CoreBackTargetPermanentCurrentStateProjection",
    );
    let canonical_projection = braced_item(
        engine,
        "struct CoreBackTargetPermanentCanonicalRelationProjection",
    );
    let evidence = braced_item(engine, "enum BackTargetPermanentBeginCorePermanentEvidence");
    for projection_or_evidence in [current_projection, canonical_projection, evidence] {
        let projection_or_evidence = compact(projection_or_evidence);
        assert!(
            !projection_or_evidence.contains("Option<RetiredHostAuthority>")
                && !projection_or_evidence.contains(":RetiredHostAuthority,"),
            "permanent validation evidence must not hold retired-host authority"
        );
    }
    assert!(
        compact(current_projection)
            .contains("current_retired_host:Option<RetiredHostAuthorityProjection>")
            && compact(canonical_projection)
                .contains("retired_host:Option<RetiredHostAuthorityProjection>"),
        "permanent begin must retain the admission projection, not a parallel Core identity type"
    );
    let current_builder = compact(braced_item(
        engine,
        "fn back_target_permanent_current_state_projection",
    ));
    let canonical_builder = compact(braced_item(
        engine,
        "fn back_target_permanent_canonical_relation_projections",
    ));
    let named_borrow_count = [&current_builder, &canonical_builder]
        .into_iter()
        .map(|builder| {
            builder
                .matches("current.retired_host.as_ref().map(RetiredHostAuthority::projection)")
                .count()
        })
        .sum::<usize>();
    let closure_borrow_count = [&current_builder, &canonical_builder]
        .into_iter()
        .map(|builder| {
            builder
                .matches("current.retired_host.as_ref().map(|authority|authority.projection())")
                .count()
        })
        .sum::<usize>();
    assert!(
        named_borrow_count + closure_borrow_count >= 2
            && [&current_builder, &canonical_builder]
                .into_iter()
                .all(|builder| !builder.contains("retired_host.clone()")),
        "both permanent projection builders must borrow retired-host authority without cloning"
    );

    let integration_and_tests = include_str!("../../bevy_ui_router_core/src/integration.rs");
    let integration = &integration_and_tests[..integration_and_tests
        .find("mod tests;")
        .unwrap_or(integration_and_tests.len())];
    for guarded_item in [
        braced_item(
            integration,
            "enum BackTargetPermanentCleanupMismatchEvidence",
        ),
        braced_item(
            integration,
            "struct BackTargetPermanentCleanupTerminalEvidence",
        ),
        braced_item(integration, "enum BackTargetPermanentBeginOutcome"),
        braced_item(
            integration,
            "enum BackTargetPermanentBeginPermanentEvidence",
        ),
        braced_item(integration, "fn begin_back_target_permanent_cleanup"),
        braced_item(integration, "fn complete_back_target_permanent_cleanup"),
    ] {
        assert!(
            !compact(guarded_item).contains("RetiredHostAuthority"),
            "back-target permanent integration evidence must not acquire or clone retired-host authority"
        );
    }
}

#[test]
fn back_target_permanent_canonical_evidence_is_typed_and_snapshot_drift_stays_distinct() {
    let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
        back_target_permanent_begin_fixture();
    let settlement =
        engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
    let expected_scope = settlement.scope.clone();
    let expected_current = settlement.expected_current.clone();
    let expected_request = settlement.expected_request.clone();
    let expected_roots = settlement.logical_roots.clone();
    engine
        .current
        .as_mut()
        .expect("the fixture retains ReadyToCommit work")
        .reconciliation = None;
    let before = engine.snapshot();
    let callback_calls = std::cell::Cell::new(0usize);

    match engine.begin_back_target_permanent_cleanup(
        &mut issuer,
        CoreBackTargetPermanentBeginOwner::new(settlement, lease),
        &ticket,
        logical_roots,
        0x534e_4150_5f44_5246,
        NONZERO_OBLIGATION_COUNT,
        "snapshot-drift-installation",
        |_, _| callback_calls.set(callback_calls.get() + 1),
    ) {
        super::BackTargetPermanentBeginCoreOutcome::Permanent {
            owner,
            installation,
            evidence:
                super::BackTargetPermanentBeginCorePermanentEvidence::CurrentStateContradiction {
                    expected,
                    observed,
                },
        } => {
            let (owner, _lease) = owner.into_parts();
            assert_eq!(owner.scope, expected_scope);
            assert_eq!(owner.expected_current, expected_current);
            assert_eq!(owner.expected_request, expected_request);
            assert_eq!(owner.logical_roots, expected_roots);
            assert_eq!(installation, "snapshot-drift-installation");
            assert_ne!(expected, observed);
        }
        _ => panic!("post-freeze projection drift must remain CurrentStateContradiction"),
    }
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(engine.snapshot(), before);

    fn item_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let tail = &source[source.find(start).expect("contract item must remain named")..];
        &tail[..tail.find(end).expect("contract boundary must remain named")]
    }

    fn braced_item<'a>(source: &'a str, marker: &str) -> &'a str {
        let tail = &source[source
            .find(marker)
            .expect("typed contract item must remain named")..];
        let open = tail
            .find('{')
            .expect("typed contract item must have a body");
        let mut depth = 0usize;
        for (offset, character) in tail[open..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &tail[..open + offset + character.len_utf8()];
                    }
                }
                _ => {}
            }
        }
        panic!("typed contract item body must close")
    }

    let engine_source = include_str!("../../bevy_ui_router_core/src/engine.rs");
    let production_source = &engine_source[..engine_source
        .find("#[cfg(test)]\n#[path = \"engine_tests.rs\"]\nmod tests;")
        .expect("the production/test boundary must remain explicit")];
    let evidence = item_between(
        production_source,
        "pub(crate) enum BackTargetPermanentBeginCorePermanentEvidence",
        "pub(crate) struct CoreBackTargetPermanentCurrentStateProjection",
    );
    assert!(
        evidence.contains("CanonicalRelationContradiction"),
        "pre-freeze canonical rejection needs a purpose-specific evidence variant"
    );
    let canonical = braced_item(evidence, "CanonicalRelationContradiction");
    let compact_canonical = canonical
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(
        compact_canonical.contains("expected:CoreBackTargetPermanentCanonicalRelationProjection")
            && compact_canonical
                .contains("observed:CoreBackTargetPermanentCanonicalRelationProjection"),
        "canonical rejection must carry the complete typed expected/observed relation pair"
    );
    assert!(
        !compact_canonical.contains("canonical:bool"),
        "canonical relation evidence must not collapse to a hidden boolean"
    );

    let projection = braced_item(
        production_source,
        "struct CoreBackTargetPermanentCanonicalRelationProjection",
    );
    for relation in [
        "request",
        "location",
        "logical_roots",
        "cleanup_roots",
        "reconciliation",
        "active",
        "stage",
        "availability",
        "retired_host",
        "pending_mount",
        "forbidden_pending",
    ] {
        assert!(
            projection.contains(relation),
            "the compact canonical relation projection must name {relation}"
        );
    }
}

#[test]
fn back_target_permanent_request_contradiction_preserves_presence_and_trace_contract() {
    let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
        back_target_permanent_begin_fixture();
    let settlement =
        engine.prepare_back_target_permanent_settlement_owner(scope.clone(), logical_roots.clone());
    let expected_scope = settlement.scope.clone();
    let expected_current = settlement.expected_current.clone();
    let expected_request = settlement.expected_request.clone();
    let expected_roots = settlement.logical_roots.clone();
    engine
        .records
        .iter_mut()
        .find(|record| record.id() == scope.request())
        .expect("the trace fixture retains its current request")
        .testing_close_trace();
    let before = engine.snapshot();
    let callback_calls = std::cell::Cell::new(0usize);

    let source = match engine.begin_back_target_permanent_cleanup(
        &mut issuer,
        CoreBackTargetPermanentBeginOwner::new(settlement, lease),
        &ticket,
        logical_roots,
        0x5452_4143_455f_4f4e,
        NONZERO_OBLIGATION_COUNT,
        "trace-only-installation",
        |_, _| callback_calls.set(callback_calls.get() + 1),
    ) {
        super::BackTargetPermanentBeginCoreOutcome::Permanent {
            owner,
            installation,
            evidence:
                super::BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                    source,
                    expected,
                    observed,
                },
        } => {
            let (owner, _lease) = owner.into_parts();
            assert_eq!(owner.scope, expected_scope);
            assert_eq!(owner.expected_current, expected_current);
            assert_eq!(owner.expected_request, expected_request);
            assert_eq!(owner.logical_roots, expected_roots);
            assert_eq!(installation, "trace-only-installation");
            assert!(source.current_request.trace_closed);
            assert!(matches!(
                expected,
                super::CoreBackTargetPermanentTerminalExpectedProjection::Request {
                    operation: super::CoreBackTargetPermanentTerminalOperation::RecoveringTrace,
                    request,
                } if request.request == scope.request() && !request.trace_closed
            ));
            assert!(matches!(
                observed,
                super::CoreBackTargetPermanentTerminalObservedProjection::Request {
                    operation: super::CoreBackTargetPermanentTerminalOperation::RecoveringTrace,
                    request: Some(request),
                    failure: super::CoreBackTargetPermanentTerminalFailureProjection::RequestRejected(
                        RequestRecordError::TraceClosed,
                    ),
                } if request.request == scope.request() && request.trace_closed
            ));
            source
        }
        super::BackTargetPermanentBeginCoreOutcome::Permanent { .. } => {
            panic!("trace-only drift must retain purpose-specific terminal evidence")
        }
        _ => panic!("the closed real trace must fail before physical installation"),
    };
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(engine.snapshot(), before);
    assert!(source.current_request.trace_closed);
}

#[test]
fn back_target_permanent_missing_current_record_returns_lossless_typed_evidence() {
    let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
        back_target_permanent_begin_fixture();
    let settlement =
        engine.prepare_back_target_permanent_settlement_owner(scope.clone(), logical_roots.clone());
    let expected_scope = settlement.scope.clone();
    let expected_current = settlement.expected_current.clone();
    let expected_request = settlement.expected_request.clone();
    let expected_roots = settlement.logical_roots.clone();
    engine
        .records
        .retain(|record| record.id() != scope.request());
    let before = engine.snapshot();
    let callback_calls = std::cell::Cell::new(0usize);

    let evidence = match engine.begin_back_target_permanent_cleanup(
        &mut issuer,
        CoreBackTargetPermanentBeginOwner::new(settlement, lease),
        &ticket,
        logical_roots,
        0x4d49_5353_494e_475f,
        NONZERO_OBLIGATION_COUNT,
        "missing-current-installation",
        |_, _| callback_calls.set(callback_calls.get() + 1),
    ) {
        super::BackTargetPermanentBeginCoreOutcome::Permanent {
            owner,
            installation,
            evidence,
        } => {
            let (owner, _lease) = owner.into_parts();
            assert_eq!(owner.scope, expected_scope);
            assert_eq!(owner.expected_current, expected_current);
            assert_eq!(owner.expected_request, expected_request);
            assert_eq!(owner.logical_roots, expected_roots);
            assert_eq!(installation, "missing-current-installation");
            assert!(matches!(
                evidence,
                super::BackTargetPermanentBeginCorePermanentEvidence::RequestRecordContradiction { .. }
            ));
            evidence
        }
        _ => panic!("a missing frozen current record must return typed contradiction evidence"),
    };
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(engine.snapshot(), before);
    let evidence = format!("{evidence:?}");
    assert!(
        evidence.contains("expected: Some(")
            && evidence.contains("trace_closed: false")
            && evidence.contains("observed: None"),
        "missing record evidence must not synthesize an observed status: {evidence}"
    );
}

#[test]
fn back_target_permanent_terminal_source_retains_missing_queued_record_expectation() {
    let (mut engine, scope, ticket, logical_roots, mut issuer, lease) =
        back_target_permanent_begin_fixture();
    let queued_location = RouteLocation::new("/items/queued-after-back");
    let queued_request = engine
        .push(queued_location.clone())
        .expect("the ReadyToCommit fixture queues a later request");
    assert_eq!(engine.queue.len(), 1);
    let settlement =
        engine.prepare_back_target_permanent_settlement_owner(scope, logical_roots.clone());
    let expected_scope = settlement.scope.clone();
    let expected_current = settlement.expected_current.clone();
    let expected_request = settlement.expected_request.clone();
    let expected_roots = settlement.logical_roots.clone();
    engine
        .records
        .retain(|record| record.id() != queued_request);
    let before = engine.snapshot();
    let callback_calls = std::cell::Cell::new(0usize);

    let source = match engine.begin_back_target_permanent_cleanup(
        &mut issuer,
        CoreBackTargetPermanentBeginOwner::new(settlement, lease),
        &ticket,
        logical_roots,
        0x5155_4555_455f_4f4e,
        NONZERO_OBLIGATION_COUNT,
        "missing-queued-installation",
        |_, _| callback_calls.set(callback_calls.get() + 1),
    ) {
        super::BackTargetPermanentBeginCoreOutcome::Permanent {
            owner,
            installation,
            evidence:
                super::BackTargetPermanentBeginCorePermanentEvidence::TerminalPlanContradiction {
                    source,
                    expected,
                    observed,
                },
        } => {
            let (owner, _lease) = owner.into_parts();
            assert_eq!(owner.scope, expected_scope);
            assert_eq!(owner.expected_current, expected_current);
            assert_eq!(owner.expected_request, expected_request);
            assert_eq!(owner.logical_roots, expected_roots);
            assert_eq!(installation, "missing-queued-installation");
            assert!(matches!(
                expected,
                super::CoreBackTargetPermanentTerminalExpectedProjection::Request {
                    operation: super::CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                    request,
                } if request.request == queued_request
                    && request.status == RequestStatus::Queued
                    && request.requested.as_ref() == Some(&queued_location)
                    && !request.trace_closed
            ));
            assert!(matches!(
                observed,
                super::CoreBackTargetPermanentTerminalObservedProjection::Request {
                    operation:
                        super::CoreBackTargetPermanentTerminalOperation::QueuedTerminalTransition,
                    request: None,
                    failure:
                        super::CoreBackTargetPermanentTerminalFailureProjection::MissingRequest,
                }
            ));
            source
        }
        _ => panic!("the missing queued record must return its typed terminal contradiction"),
    };
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(engine.snapshot(), before);
    assert_eq!(
        source.queue.len(),
        1,
        "terminal source must retain every queued command even when its record is absent"
    );
    let source = format!("{source:?}");
    assert!(
        source.contains("expected:")
            && source.contains(&format!("request: {queued_request:?}"))
            && source.contains("observed: None"),
        "the queued source item must pair command expectation with observed record presence: {source}"
    );
}

fn publish_sealed<F>(
    engine: &mut RouterEngine,
    sealed: super::SealedCommit,
    mutation: F,
) -> Result<(), CommitError>
where
    F: FnOnce(),
{
    let prepared = engine.prepare_publication(sealed)?;
    engine.publish_prepared(prepared, mutation)
}

#[test]
fn back_resolution_action_blocks_next_collect() {
    let mut engine = ready_engine();
    engine.enqueue_push(RouteLocation::new("/")).unwrap();
    engine.collect();
    settle_current(&mut engine);
    engine.enqueue_push(RouteLocation::new("/about")).unwrap();
    engine.collect();
    settle_current(&mut engine);

    let world = engine
        .claim_history_admission_world()
        .expect("the confirmed engine issues one real history world seal");
    let stamp = engine
        .history_admission_stamp(&world)
        .expect("the published history has a real back entry");
    let first_admission = engine
        .prepare_back_admission(&world, &stamp)
        .expect("the first back admission exact-refreezes the live history");
    let second_admission = engine
        .prepare_back_admission(&world, &stamp)
        .expect("the second back admission exact-refreezes the same live history");
    let history_authority_before = engine.testing_history_authority_snapshot();
    let history_world_before = engine.history_admission_world;
    let active_before = engine.active().clone();
    let state_before = engine.state.snapshot();
    let first_accepted = engine
        .enqueue_back(first_admission)
        .expect("the first back reserves a real branch owner");
    let second_accepted = engine
        .enqueue_back(second_admission)
        .expect("the second back reserves a distinct real branch owner");
    let first_request = first_accepted.request();
    let second_request = second_accepted.request();
    assert_ne!(first_request, second_request);

    let queued_back_projection = |queued: &QueuedNavigation| {
        let QueuedReservation::Branch(reservation) = &queued.reservation else {
            panic!("accepted Back must retain its complete branch reservation")
        };
        (
            queued.request,
            matches!(&queued.command, NavigationCommand::Back),
            queued.outcome.is_none(),
            reservation.request(),
            reservation.transaction(),
            reservation.cleanup_batch(),
            reservation
                .frames()
                .iter()
                .map(|frame| (frame.frame_task(), frame.frame()))
                .collect::<Vec<_>>(),
        )
    };
    let second_queued_before_collect = queued_back_projection(
        engine
            .queue
            .get(1)
            .expect("the second accepted Back remains queued behind the first"),
    );
    assert_eq!(second_queued_before_collect.0, second_request);
    assert!(second_queued_before_collect.1);
    assert!(second_queued_before_collect.2);
    assert_eq!(engine.queue.len(), 2);
    for (queued, request) in engine.queue.iter().zip([first_request, second_request]) {
        assert_eq!(queued.request, request);
        assert!(matches!(&queued.command, NavigationCommand::Back));
        assert!(queued.outcome.is_none());
        let QueuedReservation::Branch(reservation) = &queued.reservation else {
            panic!("each accepted Back must retain its complete branch reservation")
        };
        assert_eq!(reservation.request(), request);
        assert_eq!(
            reservation.frames().len(),
            engine.matcher.max_branch_depth(),
            "each accepted Back reserves the installed maximum depth before resolution"
        );
    }
    for request in [first_request, second_request] {
        let record = engine
            .request(request)
            .expect("each accepted Back retains its queued request record");
        assert_eq!(record.status(), &RequestStatus::Queued);
        assert_eq!(record.requested(), None);
        assert_eq!(record.matched(), None);
        assert!(record.trace().is_empty());
        assert!(record.diagnostics().is_empty());
    }
    assert!(engine.current.is_none());
    assert!(engine.pending_no_target_back.is_none());
    assert!(engine.action.is_none());
    assert!(engine.current_history_capability.is_none());
    assert!(engine.pending_history_preparation_error.is_none());
    assert!(engine.prepared_publication_authority.is_none());
    assert_eq!(
        engine.testing_history_authority_snapshot(),
        history_authority_before
    );
    assert_eq!(engine.history_admission_world, history_world_before);
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.state.snapshot(), state_before);

    engine.collect();
    let queue_len_with_outstanding_action = engine.queue.len();
    let first_action_evidence = match engine.action.as_ref() {
        Some(EngineAction::BackResolution(handoff)) => handoff.evidence(),
        _ => panic!("the first accepted back must leave one raw resolution action outstanding"),
    };
    assert_eq!(queue_len_with_outstanding_action, 1);

    engine.collect();

    assert_eq!(first_accepted.request(), first_request);
    assert_eq!(second_accepted.request(), second_request);
    let observed_action_evidence = match engine.action.as_ref() {
        Some(EngineAction::BackResolution(handoff)) => handoff.evidence(),
        _ => panic!("the original raw resolution action must remain outstanding"),
    };
    assert_eq!(
        observed_action_evidence, first_action_evidence,
        "an outstanding BackResolution action must not be overwritten by collect"
    );
    assert_eq!(
        engine.queue.len(),
        queue_len_with_outstanding_action,
        "an outstanding BackResolution action must block collect from consuming the queued back"
    );
    assert_eq!(
        queued_back_projection(
            engine
                .queue
                .front()
                .expect("the second accepted Back must remain at the front of the queue"),
        ),
        second_queued_before_collect,
        "the blocked second Back must retain its exact non-authorizing queue projection"
    );
}

#[test]
fn failed_normal_view_report_issues_fresh_router_fallback_without_publishing() {
    let mut engine = ready_engine();
    let request = engine.enqueue_push(RouteLocation::new("/")).unwrap();
    engine.collect();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("root frame task expected")
    };
    let normal_identity = task.identity().clone();
    let normal_cleanup_batch = engine.current.as_ref().unwrap().cleanup_batch;

    let failure = engine
        .begin_view_report(task.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "view build rejected",
        ))
        .expect("zero-residual normal failure must enter cleanup");

    assert!(failure.logical_roots().is_empty());
    assert!(engine.action().is_none());
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xb11, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    assert_eq!(engine.history().len(), 0);
    assert_eq!(engine.state.active().kind(), ActivePresentationKind::None);
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("normal failure must issue a router fallback task")
    };
    assert_eq!(fallback.identity().request(), request);
    assert_eq!(fallback.identity().host(), normal_identity.host());
    assert_eq!(fallback.identity().slot(), normal_identity.slot());
    assert_ne!(
        fallback.identity().transaction(),
        normal_identity.transaction()
    );
    assert_ne!(
        fallback.identity().frame_task(),
        normal_identity.frame_task()
    );
    assert_ne!(fallback.identity().frame(), normal_identity.frame());
    assert_ne!(
        engine.current.as_ref().unwrap().cleanup_batch,
        normal_cleanup_batch
    );
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    assert_eq!(fallback.view(), &RouteViewKey::new("fallback").unwrap());
    assert_eq!(
        fallback.input().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Build)
    );

    let before_stale = engine.snapshot();
    assert_eq!(
        engine.begin_view_report(&normal_identity).unwrap_err(),
        ReportError::Stale
    );
    assert_eq!(engine.state.snapshot(), before_stale.state);
    assert_eq!(engine.ids.snapshot(), before_stale.ids);
    assert_eq!(
        engine.testing_history_authority_snapshot(),
        before_stale.history
    );
    assert_eq!(
        engine
            .current
            .as_ref()
            .map(|current| format!("{current:?}")),
        before_stale.current
    );
    assert_eq!(
        engine.action.as_ref().map(ActionSnapshot::from),
        before_stale.action
    );
    assert_eq!(
        report_disposition_count(&engine, request, ReportDisposition::StaleDiscarded),
        1
    );
}

#[test]
fn match_failure_recovery_keeps_matched_none_and_commits_error_without_history() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/missing")).unwrap();
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("match failure must issue a router fallback task")
    };

    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    assert!(fallback.input().route().is_none());
    assert!(engine.request(request).unwrap().matched().is_none());
    assert_eq!(
        engine.request(request).unwrap().trace()[0].kind(),
        NavigationTraceEventKind::HistoryIntentRejected
    );
    assert_eq!(
        engine.request(request).unwrap().trace()[1].kind(),
        NavigationTraceEventKind::FrameTaskIssued
    );
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
            .count(),
        1
    );
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);

    let cleanup = publish_action(&mut engine, EngineAction::FrameTask(fallback));
    assert_eq!(engine.active().kind(), ActivePresentationKind::Error);
    assert!(engine.active().matched().is_none());
    assert_eq!(
        engine.active().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Match)
    );
    assert!(engine.history().is_empty());
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
    );

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Match
    ));
}

#[test]
fn replace_match_failure_and_fallback_failure_preserve_published_history() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active_before = engine.active().clone();
    let history_before = engine.testing_history_authority_snapshot();

    let request = engine.replace(RouteLocation::new("/missing")).unwrap();
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("replace match failure starts a fresh fallback attempt")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "replace match fallback failed",
        ))
        .expect("fallback failure must retain cleanup authority before terminal");
    let recovery_cleanup = prepare_failure_cleanup(&mut engine, recovery_failure, 0xA0E1, 0);
    engine
        .complete_cleanup(valid_completion(&recovery_cleanup))
        .unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert!(!engine.testing_history_capability_snapshot().has_owner());
}

#[test]
fn replace_mount_failure_after_capability_issue_preserves_history_through_fallback_failure() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    let active_before = engine.active().clone();
    let history_before = engine.testing_history_authority_snapshot();

    let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
    assert!(
        engine
            .testing_history_capability_snapshot()
            .has_exactly_one_owner()
    );
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("the preserved root must request the entering child mount")
    };
    assert!(
        engine
            .testing_history_capability_snapshot()
            .has_exactly_one_owner()
    );
    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    let roots = failure.logical_roots().to_vec();
    let count = usize::from(!roots.is_empty());
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(roots, manifest_digest(0xA0F1), count),
        )
        .unwrap();
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("replace mount failure continues through a fresh fallback")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "replace mount fallback failed",
        ))
        .expect("fallback failure must retain cleanup authority before terminal");
    let recovery_cleanup = prepare_failure_cleanup(&mut engine, recovery_failure, 0xA0F2, 0);
    engine
        .complete_cleanup(valid_completion(&recovery_cleanup))
        .unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert!(!engine.testing_history_capability_snapshot().has_owner());
}

#[test]
fn fallback_failure_preserves_old_presentation_and_history_then_reports_recovery_failed() {
    let mut engine = ready_engine();
    let old_active = engine.active().clone();
    let old_history = engine.history().clone();

    let request = engine.push(RouteLocation::new("/about")).unwrap();
    let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let original_matched = engine.request(request).unwrap().matched().cloned();
    let normal_failure = engine
        .begin_view_report(normal.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "normal apply rejected",
        ))
        .expect("normal failure must enter canonical cleanup before fallback");
    assert!(normal_failure.logical_roots().is_empty());
    assert_eq!(engine.active(), &old_active);
    assert_eq!(engine.history(), &old_history);
    assert!(engine.action().is_none());
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    let normal_cleanup = prepare_failure_cleanup(&mut engine, normal_failure, 0xb21, 0);
    engine
        .complete_cleanup(valid_completion(&normal_cleanup))
        .unwrap();
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("fallback task expected")
    };
    assert_eq!(
        engine.request(request).unwrap().matched(),
        original_matched.as_ref()
    );

    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "fallback build rejected",
        ))
        .expect("fallback failure must retain cleanup authority before terminal");

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Running(NavigationPhase::CleanupPending)
    ));
    let recovery_cleanup = prepare_failure_cleanup(&mut engine, recovery_failure, 0xB22, 0);
    engine
        .complete_cleanup(valid_completion(&recovery_cleanup))
        .unwrap();

    assert_eq!(engine.active(), &old_active);
    assert_eq!(engine.history(), &old_history);
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert!(engine.current.is_none());
    assert!(engine.action.is_none());
    let accepted_reports = engine
        .request(request)
        .unwrap()
        .diagnostics()
        .iter()
        .filter(|event| event.report_disposition() == Some(ReportDisposition::Accepted))
        .map(|event| event.redacted_message())
        .collect::<Vec<_>>();
    assert_eq!(accepted_reports.len(), 2);
    assert!(accepted_reports[0].contains("normal Apply"));
    assert!(accepted_reports[1].contains("fallback Build"));
}

#[test]
fn post_match_fallback_commit_keeps_original_fingerprint_and_failure() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/items/7")).unwrap();
    let original_matched = engine.request(request).unwrap().matched().cloned().unwrap();
    let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let normal_failure = engine
        .begin_view_report(normal.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "normal apply failed",
        ))
        .expect("post-match failure must enter canonical cleanup");
    assert!(normal_failure.logical_roots().is_empty());
    assert!(engine.action().is_none());
    let normal_cleanup = prepare_failure_cleanup(&mut engine, normal_failure, 0xb22, 0);
    engine
        .complete_cleanup(valid_completion(&normal_cleanup))
        .unwrap();
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("post-match failure must issue fallback")
    };
    let original_leaf = engine
        .matcher
        .match_location(&RouteLocation::new("/items/7"))
        .unwrap()
        .leaf()
        .unwrap()
        .clone();
    assert_eq!(fallback.input().route(), Some(original_leaf.route()));
    assert_eq!(fallback.input().params(), original_leaf.params());
    let trace = engine.request(request).unwrap().trace();
    let history_rejected = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
        .expect("normal failure must reject history intent");
    let fallback_issued = trace
        .iter()
        .rposition(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
        .expect("fallback task must be traced");
    assert!(history_rejected < fallback_issued);
    assert_eq!(
        trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::HistoryIntentRejected)
            .count(),
        1
    );

    let cleanup = publish_action(&mut engine, EngineAction::FrameTask(fallback));

    assert_eq!(engine.active().matched(), Some(&original_matched));
    assert_eq!(
        engine.active().failure().map(NavigationFailure::kind),
        Some(NavigationFailureKind::Apply)
    );
    assert!(engine.history().is_empty());

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Apply
    ));
}

#[test]
fn an_error_presentation_never_takes_the_equivalent_zero_effect_path() {
    let mut engine = ready_engine();
    let failed = engine.push(RouteLocation::new("/items/7")).unwrap();
    let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let normal_failure = engine
        .begin_view_report(normal.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "normal build failed",
        ))
        .expect("normal build failure must enter canonical cleanup");
    assert!(normal_failure.logical_roots().is_empty());
    assert!(engine.action().is_none());
    let normal_cleanup = prepare_failure_cleanup(&mut engine, normal_failure, 0xb23, 0);
    engine
        .complete_cleanup(valid_completion(&normal_cleanup))
        .unwrap();
    let fallback_cleanup = publish_current(&mut engine);
    engine
        .complete_cleanup(valid_completion(&fallback_cleanup))
        .unwrap();
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Failed(_)
    ));
    let error_frame = engine.active().frames()[0].clone();

    engine.push(RouteLocation::new("/items/7")).unwrap();
    let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
        panic!("same-location push after error must build a fresh normal frame")
    };
    assert!(matches!(
        normal.identity().origin(),
        FrameOrigin::MatchedRoute(_)
    ));
    assert_ne!(normal.identity().frame(), error_frame.frame());
    assert_eq!(
        engine.current.as_ref().unwrap().cleanup_roots,
        vec![error_frame]
    );
}

fn report_disposition_count(
    engine: &RouterEngine,
    request: crate::ids::NavigationRequestId,
    disposition: ReportDisposition,
) -> usize {
    engine
        .request(request)
        .unwrap()
        .diagnostics()
        .iter()
        .filter(|event| event.report_disposition() == Some(disposition))
        .count()
}

#[test]
fn rejected_view_reports_are_classified_deduplicated_and_lifecycle_zero_write() {
    let mut duplicate_engine = ready_engine();
    let duplicate_request = duplicate_engine
        .push(RouteLocation::new("/items/7"))
        .unwrap();
    let EngineAction::FrameTask(duplicate_task) = duplicate_engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let ticket = drive_to_commit(
        &mut duplicate_engine,
        EngineAction::FrameTask(duplicate_task),
    );
    let duplicate_identity = ticket.identity().clone();
    let duplicate_frame = duplicate_engine
        .current
        .as_ref()
        .unwrap()
        .target_frames
        .last()
        .unwrap()
        .clone();
    let duplicate_lifecycle = (
        duplicate_engine.state.snapshot(),
        duplicate_engine.ids.snapshot(),
        duplicate_engine.testing_history_authority_snapshot(),
        format!("{:?}", duplicate_engine.current),
        duplicate_engine.action.as_ref().map(ActionSnapshot::from),
    );
    assert!(
        duplicate_engine
            .begin_view_report(&duplicate_identity)
            .is_err()
    );
    assert!(
        duplicate_engine
            .begin_view_report(&duplicate_identity)
            .is_err()
    );
    assert_eq!(
        report_disposition_count(
            &duplicate_engine,
            duplicate_request,
            ReportDisposition::DuplicateRejected,
        ),
        1
    );
    let duplicate_diagnostic = duplicate_engine
        .request(duplicate_request)
        .unwrap()
        .diagnostics()
        .iter()
        .find(|event| event.report_disposition() == Some(ReportDisposition::DuplicateRejected))
        .unwrap();
    assert_eq!(duplicate_diagnostic.request(), Some(duplicate_request));
    assert_eq!(duplicate_diagnostic.frame(), Some(&duplicate_frame));
    assert_eq!(
        (
            duplicate_engine.state.snapshot(),
            duplicate_engine.ids.snapshot(),
            duplicate_engine.testing_history_authority_snapshot(),
            format!("{:?}", duplicate_engine.current),
            duplicate_engine.action.as_ref().map(ActionSnapshot::from),
        ),
        duplicate_lifecycle
    );

    let mut stale_engine = ready_engine();
    let stale_request = stale_engine.push(RouteLocation::new("/items/7")).unwrap();
    let EngineAction::FrameTask(stale_task) = stale_engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let stale_identity = stale_task.identity().clone();
    let stale_failure = stale_engine
        .begin_view_report(&stale_identity)
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "normal build failed",
        ))
        .expect("normal build failure must enter canonical cleanup");
    assert!(stale_failure.logical_roots().is_empty());
    assert!(stale_engine.action().is_none());
    let stale_cleanup = prepare_failure_cleanup(&mut stale_engine, stale_failure, 0xb24, 0);
    stale_engine
        .complete_cleanup(valid_completion(&stale_cleanup))
        .unwrap();
    let stale_lifecycle = (
        stale_engine.state.snapshot(),
        stale_engine.ids.snapshot(),
        stale_engine.testing_history_authority_snapshot(),
        format!("{:?}", stale_engine.current),
        stale_engine.action.as_ref().map(ActionSnapshot::from),
    );
    assert!(stale_engine.begin_view_report(&stale_identity).is_err());
    assert!(stale_engine.begin_view_report(&stale_identity).is_err());
    assert_eq!(
        report_disposition_count(
            &stale_engine,
            stale_request,
            ReportDisposition::StaleDiscarded,
        ),
        1
    );
    assert_eq!(
        (
            stale_engine.state.snapshot(),
            stale_engine.ids.snapshot(),
            stale_engine.testing_history_authority_snapshot(),
            format!("{:?}", stale_engine.current),
            stale_engine.action.as_ref().map(ActionSnapshot::from),
        ),
        stale_lifecycle
    );

    let mut invalid_engine = ready_engine();
    let invalid_request = invalid_engine.push(RouteLocation::new("/items/7")).unwrap();
    let EngineAction::FrameTask(invalid_task) = invalid_engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let mut invalid_identity = invalid_task.identity().clone();
    invalid_identity.testing_set_host_epoch(invalid_identity.host_epoch() + 1);
    let invalid_lifecycle = (
        invalid_engine.state.snapshot(),
        invalid_engine.ids.snapshot(),
        invalid_engine.testing_history_authority_snapshot(),
        format!("{:?}", invalid_engine.current),
        invalid_engine.action.as_ref().map(ActionSnapshot::from),
    );
    assert!(invalid_engine.begin_view_report(&invalid_identity).is_err());
    assert!(invalid_engine.begin_view_report(&invalid_identity).is_err());
    let foreign_identity = distinct_identity();
    assert!(invalid_engine.begin_view_report(&foreign_identity).is_err());
    assert!(invalid_engine.begin_view_report(&foreign_identity).is_err());
    assert_eq!(
        report_disposition_count(
            &invalid_engine,
            invalid_request,
            ReportDisposition::InvalidRejected,
        ),
        1
    );
    assert_eq!(
        invalid_engine
            .router_diagnostics()
            .iter()
            .filter(|event| {
                event.report_disposition() == Some(ReportDisposition::InvalidRejected)
            })
            .count(),
        1
    );
    assert_eq!(
        (
            invalid_engine.state.snapshot(),
            invalid_engine.ids.snapshot(),
            invalid_engine.testing_history_authority_snapshot(),
            format!("{:?}", invalid_engine.current),
            invalid_engine.action.as_ref().map(ActionSnapshot::from),
        ),
        invalid_lifecycle
    );

    let mut terminal_engine = ready_engine();
    let terminal_request = terminal_engine
        .push(RouteLocation::new("/items/7"))
        .unwrap();
    let EngineAction::FrameTask(terminal_task) = terminal_engine.action().unwrap() else {
        panic!("normal task expected")
    };
    let terminal_cleanup =
        publish_action(&mut terminal_engine, EngineAction::FrameTask(terminal_task));
    let terminal_identity = terminal_cleanup.testing_identity().clone();
    terminal_engine
        .complete_cleanup(valid_completion(&terminal_cleanup))
        .unwrap();
    assert!(
        terminal_engine
            .begin_view_report(&terminal_identity)
            .is_err()
    );
    assert_eq!(
        report_disposition_count(
            &terminal_engine,
            terminal_request,
            ReportDisposition::DuplicateRejected,
        ),
        1
    );
}

#[test]
fn evicted_report_replays_are_router_scoped_and_private_indexes_stay_bounded() {
    let mut engine = ready_engine();
    let total = TERMINAL_RETENTION + ROUTER_DIAGNOSTIC_RETENTION + 2;
    let mut requests = Vec::new();
    let mut identities = Vec::new();
    for index in 0..total {
        let request = engine
            .push(RouteLocation::new(format!("/items/{index}")))
            .unwrap();
        requests.push(request);
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("changed root must issue one task")
        };
        identities.push(task.identity().clone());
        let cleanup = publish_action(&mut engine, EngineAction::FrameTask(task));
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    }
    let newest = *requests.last().unwrap();
    let newest_diagnostics = engine.request(newest).unwrap().diagnostics().to_vec();

    for identity in identities.iter().take(total - TERMINAL_RETENTION) {
        assert_eq!(
            engine.begin_view_report(identity).unwrap_err(),
            ReportError::Invalid
        );
        assert_eq!(
            engine.begin_view_report(identity).unwrap_err(),
            ReportError::Invalid
        );
    }

    assert_eq!(
        engine.request(newest).unwrap().diagnostics(),
        newest_diagnostics
    );
    assert_eq!(
        engine.router_diagnostics().len(),
        ROUTER_DIAGNOSTIC_RETENTION
    );
    assert_eq!(engine.issued_view_tasks.len(), TERMINAL_RETENTION);
    assert_eq!(engine.completed_cleanup.len(), TERMINAL_RETENTION);
    assert_eq!(
        engine.report_dispositions.len(),
        TERMINAL_RETENTION + ROUTER_DIAGNOSTIC_RETENTION
    );
}

#[test]
fn fallback_reservation_exhaustion_is_atomic_and_settles_recovery_failed() {
    for counter in [
        ReservationCounter::Transaction,
        ReservationCounter::FrameTask,
        ReservationCounter::CandidateFrame,
        ReservationCounter::CleanupBatch,
    ] {
        let mut engine = ready_engine();
        let request = engine.push(RouteLocation::new("/items/7")).unwrap();
        let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
            panic!("normal task expected")
        };
        engine.testing_exhaust_reservation_counter(counter);
        let ids_before = engine.ids.snapshot();

        let failure = engine
            .begin_view_report(normal.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Build,
                "normal build failed",
            ))
            .expect("reservation exhaustion is observed after canonical cleanup");

        assert_eq!(engine.ids.snapshot(), ids_before, "counter: {counter:?}");
        assert!(failure.logical_roots().is_empty(), "counter: {counter:?}");
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending),
            "counter: {counter:?}"
        );
        assert!(engine.action().is_none());
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
        );
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xb25, 0);
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert_eq!(engine.ids.snapshot(), ids_before, "counter: {counter:?}");
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert!(engine.action().is_none());
    }
}

fn seal_current(engine: &mut RouterEngine, ticket: super::CommitTicket) -> super::SealedCommit {
    let roots = engine
        .current
        .as_ref()
        .expect("test requires current work")
        .cleanup_roots
        .clone();
    let count = if roots.is_empty() {
        0
    } else {
        NONZERO_OBLIGATION_COUNT
    };
    engine
        .seal_commit(
            &ticket,
            CleanupManifestSeal::new(roots, manifest_digest(41), count),
        )
        .unwrap()
}

fn manifest_digest(tag: u64) -> CleanupManifestDigest {
    CleanupManifestDigest::from_opaque(0x4d41_4e49_4645_5354 ^ tag)
}

fn settle_current(engine: &mut RouterEngine) {
    let cleanup = publish_current(engine);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

fn valid_completion(cleanup: &CleanupBatchToken) -> CleanupCompletion {
    let digest = if cleanup.obligation_count() == 0 {
        CleanupCompletionDigest::canonical_empty()
    } else {
        CleanupCompletionDigest::from_opaque(0x5445_524d_494e_414c)
    };
    CleanupCompletion::attest(cleanup.clone(), digest, cleanup.obligation_count())
}

fn pending_reconciliation_mount_completion(
    engine: &mut RouterEngine,
) -> crate::mount::MountRegistrationCompletion {
    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::MountRequest(request) = engine.action().unwrap() else {
        panic!("reconciliation mount request expected")
    };
    let grant = engine.resolve_mount(request).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(81));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    permit.complete(crate::mount::MountRegistrationDigest::from_opaque(82))
}

fn settle_reconciliation_authority_loss_and_restart_mount(
    engine: &mut RouterEngine,
    active_child_slots: Vec<LogicalChildSlotBinding>,
) {
    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!("reconciliation drift must yield failure cleanup")
    };
    let request = failure.request();
    let count = usize::from(!failure.logical_roots().is_empty());
    let cleanup = prepare_failure_cleanup(engine, failure, 83, count);
    assert_failure_waits_then_settles(
        engine,
        request,
        &cleanup,
        NavigationFailureKind::AuthorityLost,
    );

    engine.active_child_slots = active_child_slots;
    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::MountRequest(request) = engine.action().unwrap() else {
        panic!("replacement reconciliation mount request expected")
    };
    assert!(engine.resolve_mount(request).is_ok());
}

fn distinct_identity() -> RootTaskIdentity {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected second frame task");
    };
    task.identity().clone()
}

fn identity_mismatches(
    exact: &RootTaskIdentity,
    distinct: &RootTaskIdentity,
) -> Vec<RootTaskIdentity> {
    let mut mismatches = Vec::new();
    let mut value = exact.clone();
    value.testing_set_router(distinct.router_identity());
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_host(RouteHostId::new(exact.host().raw() + 1));
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_host_epoch(exact.host_epoch() + 1);
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_top_slot(RouteSlotId::new(exact.slot().raw() + 1));
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_request(distinct.request());
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_transaction(distinct.transaction());
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_frame_task(distinct.frame_task());
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_frame(distinct.frame());
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_generation(RouteGeneration::initial().next());
    mismatches.push(value);
    let mut value = exact.clone();
    value.testing_set_origin(FrameOrigin::RouterFallback);
    mismatches.push(value);
    mismatches
}

#[test]
fn no_host_push_rejects_without_allocating_or_changing_state() {
    let mut engine = RouterEngine::install(tree(), RouteViewKey::new("fallback").unwrap()).unwrap();
    let before = engine.snapshot();

    assert!(engine.push(RouteLocation::new("/items/1")).is_err());
    assert_eq!(engine.snapshot(), before);
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
}

#[test]
fn confirmed_host_push_matches_internally_and_queues_one_running_request() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/items//1/")).unwrap();
    let queued = engine.push(RouteLocation::new("/about")).unwrap();

    assert_ne!(request, queued);
    assert_eq!(
        engine.request(request).unwrap().requested().unwrap().path(),
        "/items/1"
    );
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
    );
    assert_eq!(
        engine.request(queued).unwrap().status(),
        &RequestStatus::Queued
    );
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("first root push must issue a frame task");
    };
    assert_eq!(task.location().path(), "/items/1");
    assert!(matches!(
        task.identity().origin(),
        FrameOrigin::MatchedRoute(_)
    ));
}

#[test]
fn every_root_task_identity_field_is_checked_before_report_mutation() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected frame task");
    };
    let exact = task.identity().clone();
    let foreign = distinct_identity();

    for mismatch in identity_mismatches(&exact, &foreign) {
        let before = engine.snapshot();
        assert_eq!(
            engine.begin_view_report(&mismatch).unwrap_err(),
            ReportError::Invalid
        );
        assert_eq!(engine.state.snapshot(), before.state);
        assert_eq!(engine.ids.snapshot(), before.ids);
        assert_eq!(engine.testing_history_authority_snapshot(), before.history);
        assert_eq!(
            engine
                .current
                .as_ref()
                .map(|current| format!("{current:?}")),
            before.current
        );
        assert_eq!(
            engine.action.as_ref().map(ActionSnapshot::from),
            before.action
        );
    }
}

#[test]
fn successful_report_does_not_publish_until_preflighted_commit_returns_normally() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected frame task");
    };
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));

    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert!(engine.history().is_empty());

    let mut stale = ticket.testing_duplicate();
    stale.testing_set_host_epoch(stale.identity().host_epoch() + 1);
    let before = engine.snapshot();
    assert_eq!(
        engine
            .seal_commit(
                &stale,
                CleanupManifestSeal::new(vec![], manifest_digest(1), 0),
            )
            .unwrap_err(),
        CommitError::Stale
    );
    assert_eq!(engine.snapshot(), before);

    let sealed = seal_current(&mut engine, ticket);
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert!(engine.history().is_empty());
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();
    assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
    assert_eq!(engine.history().len(), 1);
    assert_eq!(
        engine.current_status(),
        Some(&RequestStatus::Running(NavigationPhase::PostCommitCleanup))
    );
    assert_eq!(cleanup.obligation_count(), 0);
}

#[test]
fn every_root_task_identity_field_is_checked_before_commit_mutation() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected frame task");
    };
    let exact = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
    let foreign = distinct_identity();

    for mismatch in identity_mismatches(exact.identity(), &foreign) {
        let before = engine.snapshot();
        let stale = exact.testing_with_identity(mismatch);
        assert_eq!(
            engine
                .seal_commit(
                    &stale,
                    CleanupManifestSeal::new(vec![], manifest_digest(2), 0),
                )
                .unwrap_err(),
            CommitError::Stale
        );
        assert_eq!(engine.snapshot(), before);
    }

    let sealed = seal_current(&mut engine, exact);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn equivalent_fragment_and_changed_root_pushes_have_exact_final_semantics() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let first_root = engine.active().frames()[0].clone();
    let first_leaf = engine.active().frames()[1].clone();

    engine.push(RouteLocation::new("items//1/")).unwrap();
    let equivalent = engine.action().unwrap();
    assert!(matches!(equivalent, EngineAction::Commit(_)));
    let equivalent_cleanup = publish_action(&mut engine, equivalent);
    assert_eq!(equivalent_cleanup.obligation_count(), 0);
    engine
        .complete_cleanup(valid_completion(&equivalent_cleanup))
        .unwrap();
    assert_eq!(engine.active().frames()[0], first_root);

    engine
        .push(RouteLocation::new("/items/1").with_fragment("details"))
        .unwrap();
    let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
        panic!("fragment change must issue one task");
    };
    assert_eq!(fragment.identity().frame(), first_leaf.frame());
    assert_eq!(
        fragment.identity().generation(),
        first_leaf.generation().next()
    );
    let fragment_cleanup = publish_action(&mut engine, EngineAction::FrameTask(fragment));
    assert_eq!(
        fragment_cleanup.obligation_count(),
        NONZERO_OBLIGATION_COUNT
    );
    engine
        .complete_cleanup(valid_completion(&fragment_cleanup))
        .unwrap();

    engine.push(RouteLocation::new("/about")).unwrap();
    let EngineAction::FrameTask(changed) = engine.action().unwrap() else {
        panic!("changed root must issue one entering task");
    };
    assert_ne!(changed.identity().frame(), first_leaf.frame());
    assert_eq!(changed.identity().generation(), RouteGeneration::initial());
    let changed_cleanup = publish_action(&mut engine, EngineAction::FrameTask(changed));
    assert_eq!(changed_cleanup.obligation_count(), NONZERO_OBLIGATION_COUNT);
    engine
        .complete_cleanup(valid_completion(&changed_cleanup))
        .unwrap();
    assert_eq!(engine.history().len(), 4);
}

#[test]
fn reconciliation_projection_is_sealed_when_the_request_becomes_running() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active_frames = engine.active().frames().to_vec();

    engine.enqueue_push(RouteLocation::new("/items/2")).unwrap();
    engine.collect();

    let current = engine.current.as_ref().expect("request must be running");
    let projection = current
        .reconciliation
        .as_ref()
        .unwrap_or_else(|| panic!("running reconciliation missing: {current:?}"));
    assert_eq!(projection.preserved().len(), active_frames.len() - 1);
    assert_eq!(projection.leaving().len(), 1);
    assert_eq!(projection.entering().len(), 1);
    assert_eq!(
        projection.preserved()[0].version().frame(),
        active_frames[0].frame()
    );
}

#[test]
fn reconciliation_projection_classifies_equivalent_and_nested_fragment() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();

    engine.push(RouteLocation::new("/items/1")).unwrap();
    let equivalent = engine
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert_eq!(equivalent.preserved().len(), active.len());
    assert!(equivalent.leaving().is_empty());
    assert!(equivalent.entering().is_empty());
    assert!(
        equivalent
            .child_slots()
            .iter()
            .all(|slot| slot.kind() == ProjectedChildSlotRoleKind::PreservedActive)
    );
    settle_current(&mut engine);

    engine
        .push(RouteLocation::new("/items/1").with_fragment("details"))
        .unwrap();
    let fragment = engine
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert_eq!(fragment.preserved().len(), active.len() - 1);
    assert_eq!(fragment.leaving().len(), 1);
    assert_eq!(fragment.entering().len(), 1);
    assert_eq!(fragment.leaving()[0].version(), active.last().unwrap());
    assert_eq!(
        fragment.entering()[0].frame(),
        active.last().unwrap().frame()
    );
    assert_eq!(
        fragment.entering()[0].generation(),
        active.last().unwrap().generation().next()
    );
    assert_eq!(
        fragment.entering()[0].slot(),
        ProjectedEnteringSlot::Existing(active.last().unwrap().slot())
    );
}

#[test]
fn reconciliation_projection_classifies_append_truncate_and_empty_slot_reuse() {
    let mut append = ready_engine_with_tree(reconciliation_tree());
    append.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut append);
    append.push(RouteLocation::new("/items/1")).unwrap();
    let projection = append
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert_eq!(projection.preserved().len(), 2);
    assert!(projection.leaving().is_empty());
    assert_eq!(projection.entering().len(), 1);
    assert_eq!(
        projection.entering()[0].slot(),
        ProjectedEnteringSlot::MountNew
    );
    assert!(projection.child_slots().iter().any(|slot| {
        slot.parent_branch_index() == 1 && slot.kind() == ProjectedChildSlotRoleKind::MountNew
    }));

    let mut reuse = ready_engine_with_tree(reconciliation_tree());
    reuse.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut reuse);
    let active = reuse.active().frames().to_vec();
    let target = reuse
        .matcher
        .match_location(&RouteLocation::new("/items/1"))
        .unwrap();
    let retained_slot = RouteSlotId::new(900);
    reuse
        .active_child_slots
        .push(LogicalChildSlotBinding::retained_empty(
            1,
            active[1].clone(),
            target.routes()[2].slot().clone(),
            retained_slot,
        ));
    reuse.push(RouteLocation::new("/items/1")).unwrap();
    let projection = reuse
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert_eq!(
        projection.entering()[0].slot(),
        ProjectedEnteringSlot::Existing(retained_slot)
    );
    assert!(projection.child_slots().iter().any(|slot| {
        slot.parent_branch_index() == 1
            && slot.kind() == ProjectedChildSlotRoleKind::ActivateExisting
    }));

    let mut truncate = ready_engine_with_tree(reconciliation_tree());
    truncate.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut truncate);
    truncate.push(RouteLocation::new("/items")).unwrap();
    let projection = truncate
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert_eq!(projection.preserved().len(), 2);
    assert_eq!(projection.leaving().len(), 1);
    assert!(projection.entering().is_empty());
    assert!(projection.child_slots().iter().any(|slot| {
        slot.parent_branch_index() == 1 && slot.kind() == ProjectedChildSlotRoleKind::RetainEmpty
    }));
}

#[test]
fn reconciliation_projection_search_change_fails_closed() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine
        .push(RouteLocation::new("/items/1").with_search("tab=a"))
        .unwrap();
    settle_current(&mut engine);

    engine
        .push(RouteLocation::new("/items/1").with_search("tab=b"))
        .unwrap();
    let projection = engine
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert!(projection.preserved().is_empty());
    assert_eq!(projection.leaving().len(), 3);
    assert_eq!(projection.entering().len(), 3);
    assert!(
        projection
            .child_slots()
            .iter()
            .filter(|slot| slot.kind() == ProjectedChildSlotRoleKind::Leaving)
            .count()
            >= 2
    );
}

#[test]
fn reconciliation_projection_queued_request_uses_the_newly_published_active() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let previous = engine.active().frames().to_vec();

    engine.push(RouteLocation::new("/items/2")).unwrap();
    engine.enqueue_push(RouteLocation::new("/items/3")).unwrap();
    let cleanup = publish_current(&mut engine);
    let newly_published = engine.active().frames().to_vec();
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let projection = engine
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert_eq!(projection.preserved()[0].version(), &newly_published[0]);
    assert_eq!(projection.preserved()[1].version(), &newly_published[1]);
    assert_eq!(projection.preserved()[0].version(), &previous[0]);
    assert_eq!(projection.preserved()[1].version(), &previous[1]);
    assert_ne!(newly_published[2], previous[2]);
}

#[test]
fn reconciliation_projection_drift_blocks_later_mount_or_commit_action() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);

    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("whole-branch scheduling still starts with one frame task in Task 1")
    };
    engine.active_child_slots.clear();
    let next_outlet = engine
        .current
        .as_ref()
        .and_then(|current| current.branch.as_ref())
        .and_then(|branch| branch.routes().get(1))
        .map(|route| route.slot().clone())
        .unwrap();

    assert_eq!(
        engine.accept_applied_report(task.into_applied_report(vec![next_outlet])),
        Err(ReportError::Stale)
    );
    assert!(matches!(
        engine.action(),
        Some(EngineAction::FailureCleanup(_))
    ));
}

#[test]
fn reconciliation_projection_unused_equivalent_reservations_remain_burned() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let before = engine.ids.snapshot();

    engine.push(RouteLocation::new("/items/1")).unwrap();
    let after = engine.ids.snapshot();
    assert_eq!(after.next_frame(), before.next_frame() + 3);
    assert_eq!(after.next_frame_task(), before.next_frame_task() + 3);
    let projection = engine
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.as_ref())
        .unwrap();
    assert!(projection.entering().is_empty());
}

#[test]
fn reconciliation_projection_rejects_an_empty_binding_for_an_active_child() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let occupied = engine.active_child_slots[1].clone();
    engine.active_child_slots[1] = LogicalChildSlotBinding::retained_empty(
        occupied.parent_branch_index(),
        occupied.parent().clone(),
        occupied.outlet().clone(),
        occupied.slot(),
    );

    let request = engine.push(RouteLocation::new("/items/2")).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert!(engine.current.is_none());
    assert!(engine.action().is_none());
}

#[test]
fn reconciliation_projection_rejects_duplicate_slot_and_parent_outlet_mappings() {
    for duplicate_slot in [true, false] {
        let mut engine = ready_engine_with_tree(reconciliation_tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let occupied = engine.active_child_slots[1].clone();
        let slot = if duplicate_slot {
            engine.active_child_slots[0].slot()
        } else {
            RouteSlotId::new(901)
        };
        let outlet = if duplicate_slot {
            RouteSlotKey::named("alternate").unwrap()
        } else {
            occupied.outlet().clone()
        };
        engine
            .active_child_slots
            .push(LogicalChildSlotBinding::retained_empty(
                occupied.parent_branch_index(),
                occupied.parent().clone(),
                outlet,
                slot,
            ));

        let request = engine.push(RouteLocation::new("/items/2")).unwrap();
        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert!(engine.current.is_none());
        assert!(engine.action().is_none());
    }
}

#[test]
fn reconciliation_projection_nested_fragment_overflow_is_pre_mutation() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let history_before = engine.history().clone();
    let ids_before = engine.ids.snapshot();
    let location = engine.active().location().unwrap().clone();
    let mut frames = engine.active().frames().to_vec();
    let leaf = frames.last().unwrap().clone();
    *frames.last_mut().unwrap() = RouteFrameVersion::new(
        leaf.router_identity(),
        leaf.host(),
        leaf.slot(),
        leaf.origin().clone(),
        leaf.frame(),
        RouteGeneration::testing_max(),
    );
    engine
        .state
        .set_active(ActivePresentation::route(location.clone(), frames.clone()));
    engine.active_child_slots = occupied_child_slots(engine.active_branch.as_ref(), &frames);

    let request = engine.push(location.with_fragment("overflow")).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert_eq!(engine.active().frames(), frames);
    assert_eq!(engine.history(), &history_before);
    assert_eq!(
        engine.ids.snapshot().next_frame(),
        ids_before.next_frame() + 3
    );
    assert!(engine.action().is_none());
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| {
                !matches!(
                    event.kind(),
                    NavigationTraceEventKind::CleanupSettled
                        | NavigationTraceEventKind::CommitPublished
                )
            })
    );
}

#[test]
fn reconciliation_projection_stale_commit_cannot_trigger_drift_settlement() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/items/2")).unwrap();
    let action = engine.action().unwrap();
    let ticket = drive_to_commit(&mut engine, action);
    engine.active_child_slots.clear();
    let before = engine.snapshot();
    let stale = ticket.testing_with_identity(distinct_identity());
    let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
    let count = if roots.is_empty() {
        0
    } else {
        NONZERO_OBLIGATION_COUNT
    };

    assert!(matches!(
        engine.seal_commit(
            &stale,
            CleanupManifestSeal::new(roots.clone(), manifest_digest(70), count),
        ),
        Err(CommitError::Stale)
    ));
    assert_eq!(engine.snapshot(), before);
    assert!(matches!(
        engine.seal_commit(
            &ticket,
            CleanupManifestSeal::new(roots, manifest_digest(71), count),
        ),
        Err(CommitError::Stale)
    ));
    assert!(matches!(
        engine.action(),
        Some(EngineAction::FailureCleanup(_))
    ));
}

#[test]
fn reconciliation_projection_foreign_report_cannot_trigger_drift_settlement() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(exact) = engine.action().unwrap() else {
        panic!("exact task expected")
    };
    engine.active_child_slots.clear();

    let mut foreign = ready_engine_with_tree(reconciliation_tree());
    foreign.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(foreign_task) = foreign.action().unwrap() else {
        panic!("foreign task expected")
    };
    let next_outlet = engine
        .current
        .as_ref()
        .and_then(|current| current.branch.as_ref())
        .and_then(|branch| branch.routes().get(1))
        .map(|route| route.slot().clone())
        .unwrap();

    assert_eq!(
        engine.accept_applied_report(foreign_task.into_applied_report(vec![next_outlet.clone()])),
        Err(ReportError::Invalid)
    );
    assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
    assert_eq!(
        engine.accept_applied_report(exact.into_applied_report(vec![next_outlet])),
        Err(ReportError::Stale)
    );
    assert!(matches!(
        engine.action(),
        Some(EngineAction::FailureCleanup(_))
    ));
}

#[test]
fn reconciliation_projection_mount_completion_is_capability_first_then_drift_checked() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let exact = pending_reconciliation_mount_completion(&mut engine);
    let active_child_slots = engine.active_child_slots.clone();
    engine.active_child_slots.clear();

    let mut foreign = ready_engine_with_tree(reconciliation_tree());
    foreign.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut foreign);
    let foreign_completion = pending_reconciliation_mount_completion(&mut foreign);
    let before = engine.snapshot();
    assert_eq!(
        engine.complete_mount_registration(foreign_completion),
        Err(MountError::Foreign)
    );
    assert_eq!(engine.snapshot(), before);

    assert_eq!(
        engine.complete_mount_registration(exact),
        Err(MountError::WrongPhase)
    );
    settle_reconciliation_authority_loss_and_restart_mount(&mut engine, active_child_slots);
}

#[test]
fn reconciliation_projection_mount_confirmation_drift_releases_mount_issuer() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let completion = pending_reconciliation_mount_completion(&mut engine);
    let confirmation = engine.mount.complete_registration(&completion).unwrap();
    let active_child_slots = engine.active_child_slots.clone();
    engine.active_child_slots.clear();

    assert_eq!(
        engine.consume_mount_confirmation(confirmation),
        Err(MountError::WrongPhase)
    );
    settle_reconciliation_authority_loss_and_restart_mount(&mut engine, active_child_slots);
}

#[test]
fn cleanup_completion_is_exact_single_use_and_terminal_event_is_last() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let cleanup = publish_current(&mut engine);
    let replay = valid_completion(&cleanup);

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::PostCommitCleanup)
    );
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
            .count(),
        0
    );
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    let terminal = engine.snapshot();
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .map(|event| event.kind())
            .rev()
            .take(3)
            .collect::<Vec<_>>(),
        vec![
            NavigationTraceEventKind::RequestTerminal,
            NavigationTraceEventKind::CleanupSettled,
            NavigationTraceEventKind::RequestPhaseChanged,
        ]
    );
    let settled = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .collect::<Vec<_>>();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].cleanup_disposition(), None);
    assert_eq!(
        engine.complete_cleanup(replay),
        Err(CleanupError::Duplicate)
    );
    assert_eq!(engine.snapshot(), terminal);
}

#[test]
fn cleanup_rejects_each_foreign_identity_field_and_digest_without_change() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let cleanup = publish_current(&mut engine);
    let foreign = distinct_identity();

    for mismatch in identity_mismatches(cleanup.testing_identity(), &foreign) {
        let before = engine.snapshot();
        let stale = cleanup.testing_with_identity(mismatch);
        assert!(engine.complete_cleanup(valid_completion(&stale)).is_err());
        assert_eq!(engine.snapshot(), before);
    }

    let mut wrong_digest = valid_completion(&cleanup);
    wrong_digest.testing_set_binding(u64::MAX);
    let before = engine.snapshot();
    assert!(engine.complete_cleanup(wrong_digest).is_err());
    assert_eq!(engine.snapshot(), before);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn private_retention_keeps_64_terminal_records_and_distinguishes_lookup_facts() {
    let mut engine = ready_engine();
    let mut requests = Vec::new();
    for index in 0..66 {
        let request = engine
            .push(RouteLocation::new(format!("/items/{index}")))
            .unwrap();
        requests.push(request);
        settle_current(&mut engine);
    }

    assert!(matches!(
        engine.private_lookup(requests[0]),
        PrivateRequestLookup::Evicted
    ));
    assert!(matches!(
        engine.private_lookup(requests[1]),
        PrivateRequestLookup::Evicted
    ));
    assert!(matches!(
        engine.private_lookup(requests[2]),
        PrivateRequestLookup::Retained(_)
    ));
    assert!(matches!(
        engine.private_lookup(*requests.last().unwrap()),
        PrivateRequestLookup::Retained(_)
    ));

    let mut foreign = ready_engine();
    let unknown = foreign.push(RouteLocation::new("/items/0")).unwrap();
    assert!(matches!(
        engine.private_lookup(unknown),
        PrivateRequestLookup::Unknown
    ));
    assert_eq!(
        engine.testing_evicted_watermark(),
        Some(requests[1].sequence())
    );
}

#[test]
fn back_no_target_source_drift_prepared_terminal_evicts_exact_oldest_retention_state() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();
    settle_current(&mut engine);
    for index in 0..(TERMINAL_RETENTION - 2) {
        engine
            .replace(RouteLocation::new(format!("/items/{index}")))
            .unwrap();
        settle_current(&mut engine);
    }
    assert_eq!(
        engine
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                )
            })
            .count(),
        TERMINAL_RETENTION,
        "real push/replace cleanup must fill retention before Back resolution"
    );
    assert_eq!(engine.history().len(), 2);

    let world = engine
        .claim_history_admission_world()
        .expect("the confirmed engine issues one real history world seal");
    let stamp = engine
        .history_admission_stamp(&world)
        .expect("the two-entry history admits Back");
    let first_admission = engine
        .prepare_back_admission(&world, &stamp)
        .expect("the first Back exact-refreezes the live history");
    let second_admission = engine
        .prepare_back_admission(&world, &stamp)
        .expect("the second Back shares the same pre-publication source");
    let first_accepted = engine.enqueue_back(first_admission).unwrap();
    let second_accepted = engine.enqueue_back(second_admission).unwrap();
    let source_drift_request = second_accepted.request();

    engine.collect();
    let first_handoff = match engine.action().unwrap() {
        EngineAction::BackResolution(handoff) => handoff,
        _ => panic!("the first accepted Back must produce a real resolution handoff"),
    };
    let (_first_request, first_view) = first_accepted.into_parts();
    let first_verified = match engine.verify_back_resolution_handoff(first_view, first_handoff) {
        BackResolutionVerifyOutcome::Matched(verified) => verified,
        BackResolutionVerifyOutcome::Mismatch { .. } => {
            panic!("the exact first Back lineage must verify")
        }
    };
    let (_first_view, first_owner) = first_verified.into_parts();
    let target = match first_owner {
        VerifiedBackResolutionOwnerHandoff::Target(owner) => owner,
        VerifiedBackResolutionOwnerHandoff::NoTarget(_)
        | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
            panic!("the first Back from a two-entry history must resolve its predecessor")
        }
    };
    let start = match engine.resume_back_target_with_retirement(target, || {}) {
        BackTargetResumeOutcome::Resumed(start) => start,
        BackTargetResumeOutcome::Permanent(_) | BackTargetResumeOutcome::Mismatch(_) => {
            panic!("the exact first Back target must resume")
        }
    };
    let resume = match start {
        BackTargetStart::Ready(resume) => resume,
        BackTargetStart::FailureCleanup(_) => {
            panic!("the real predecessor must reach a normal target action")
        }
    };
    let target_action = match resume.action {
        BackTargetResumeAction::FrameTask(task) => EngineAction::FrameTask(task),
        BackTargetResumeAction::MountRequest(request) => EngineAction::MountRequest(request),
        BackTargetResumeAction::Commit(ticket) => EngineAction::Commit(ticket),
    };
    let first_cleanup = publish_action(&mut engine, target_action);
    engine
        .complete_cleanup(valid_completion(&first_cleanup))
        .unwrap();
    assert_eq!(engine.history_snapshot().cursor(), Some(0));

    let second_handoff = match engine.action().unwrap() {
        EngineAction::BackResolution(handoff) => handoff,
        _ => panic!("the queued second Back must resolve after the first terminal"),
    };
    let (_second_request, second_view) = second_accepted.into_parts();
    let second_verified = match engine.verify_back_resolution_handoff(second_view, second_handoff) {
        BackResolutionVerifyOutcome::Matched(verified) => verified,
        BackResolutionVerifyOutcome::Mismatch { .. } => {
            panic!("the exact second Back lineage must verify")
        }
    };
    let (_second_view, second_owner) = second_verified.into_parts();
    let candidate = match second_owner {
        VerifiedBackResolutionOwnerHandoff::NoTarget(candidate) => candidate,
        VerifiedBackResolutionOwnerHandoff::Target(_)
        | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
            panic!("the second live Back at cursor zero must resolve NoTarget")
        }
    };
    let source_resolution = candidate.evidence();

    engine
        .replace(RouteLocation::new("/items/drifted"))
        .unwrap();
    settle_current(&mut engine);
    let transfer_calls = std::cell::Cell::new(0usize);
    let drift = match engine.refreeze_back_no_target_source_with_transfer(candidate, || {
        transfer_calls.set(transfer_calls.get() + 1);
    }) {
        BackNoTargetSourceRefreezeOutcome::Drift(owner) => owner,
        BackNoTargetSourceRefreezeOutcome::Unchanged(_)
        | BackNoTargetSourceRefreezeOutcome::Mismatch { .. } => {
            panic!("the real replacement must produce an exact source-drift owner")
        }
    };
    assert_eq!(transfer_calls.get(), 1);

    let mut retained_requests = engine
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.status(),
                RequestStatus::Succeeded | RequestStatus::Failed(_)
            )
        })
        .map(|record| record.id())
        .collect::<Vec<_>>();
    retained_requests.sort_by_key(|request| request.sequence());
    assert_eq!(
        retained_requests.len(),
        TERMINAL_RETENTION,
        "the drift settlement must begin with a full terminal retention window"
    );
    let evicted_request = retained_requests[0];
    let expected_earliest_after = retained_requests[1].sequence();
    let watermark_before = engine.testing_evicted_watermark();
    assert!(
        engine
            .issued_view_tasks
            .iter()
            .any(|identity| identity.request == evicted_request),
        "the oldest real terminal must retain issued task inventory before eviction"
    );
    assert!(
        engine
            .completed_cleanup
            .iter()
            .any(|tombstone| tombstone.request == evicted_request),
        "the oldest real successful terminal must retain cleanup inventory before eviction"
    );

    let retirement_calls = std::cell::Cell::new(0usize);
    let _terminal = match engine.settle_back_no_target_source_drift_with_retirement(drift, || {
        retirement_calls.set(retirement_calls.get() + 1);
    }) {
        BackNoTargetSourceDriftSettlementOutcome::Settled(terminal) => terminal,
        BackNoTargetSourceDriftSettlementOutcome::Mismatch { .. } => {
            panic!("the exact observed drift must terminal under retention pressure")
        }
    };
    assert_eq!(retirement_calls.get(), 1);
    assert!(matches!(
        engine.private_lookup(evicted_request),
        PrivateRequestLookup::Evicted
    ));
    assert_eq!(
        engine.testing_evicted_watermark(),
        Some(evicted_request.sequence()),
        "prepared retention must advance through the exact oldest retained request"
    );
    assert!(
        watermark_before.is_none_or(|watermark| watermark < evicted_request.sequence()),
        "the prepared eviction must strictly advance the existing watermark"
    );
    assert_eq!(
        engine.earliest_retained_sequence(),
        Some(expected_earliest_after)
    );
    assert!(
        engine
            .issued_view_tasks
            .iter()
            .all(|identity| identity.request != evicted_request)
    );
    assert!(
        engine
            .report_dispositions
            .iter()
            .all(|record| record.target != super::DiagnosticTarget::Request(evicted_request))
    );
    assert!(
        engine
            .completed_cleanup
            .iter()
            .all(|tombstone| tombstone.request != evicted_request)
    );
    assert!(engine.completed_cleanup.len() <= TERMINAL_RETENTION);
    assert!(engine.report_dispositions.len() <= TERMINAL_RETENTION + ROUTER_DIAGNOSTIC_RETENTION);
    let drift_record = engine
        .request(source_drift_request)
        .expect("the newest source-drift request remains retained");
    assert!(matches!(
        drift_record.status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert_eq!(
        drift_record
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .count(),
        1
    );
    assert_eq!(
        engine.classify_back_resolution_evidence(&source_resolution),
        BackResolutionEvidenceClassification::ConsumedReplay
    );
}

#[test]
fn back_target_permanent_prepared_terminal_evicts_exact_oldest_retention_state() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();
    settle_current(&mut engine);
    for index in 0..(TERMINAL_RETENTION - 2) {
        engine
            .replace(RouteLocation::new(format!("/items/{index}")))
            .unwrap();
        settle_current(&mut engine);
    }

    let mut retained_terminals = engine
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.status(),
                RequestStatus::Succeeded | RequestStatus::Failed(_)
            )
        })
        .map(|record| record.id())
        .collect::<Vec<_>>();
    retained_terminals.sort_by_key(|request| request.sequence());
    assert_eq!(retained_terminals.len(), TERMINAL_RETENTION);
    assert_eq!(engine.history().len(), 2);
    let evicted_request = retained_terminals[0];
    let next_earliest_request = retained_terminals[1];
    assert!(
        engine
            .issued_view_tasks
            .iter()
            .any(|identity| identity.request == evicted_request),
        "the oldest real terminal must own issued task inventory before eviction"
    );
    assert!(
        engine
            .report_dispositions
            .iter()
            .any(|record| { record.target == super::DiagnosticTarget::Request(evicted_request) }),
        "the oldest real terminal must own report inventory before eviction"
    );
    assert!(
        engine
            .completed_cleanup
            .iter()
            .any(|tombstone| tombstone.request == evicted_request),
        "the oldest real terminal must own cleanup inventory before eviction"
    );

    let world = engine
        .claim_history_admission_world()
        .expect("the confirmed engine issues one real history world seal");
    let stamp = engine
        .history_admission_stamp(&world)
        .expect("the two-entry history admits a target Back");
    let admission = engine
        .prepare_back_admission(&world, &stamp)
        .expect("the target Back exact-refreezes the live history");
    let accepted = engine.enqueue_back(admission).unwrap();
    let permanent_request = accepted.request();
    engine.collect();
    let handoff = match engine.action().unwrap() {
        EngineAction::BackResolution(handoff) => handoff,
        _ => panic!("the accepted Back must produce a real resolution handoff"),
    };
    let (_request, view) = accepted.into_parts();
    let verified = match engine.verify_back_resolution_handoff(view, handoff) {
        BackResolutionVerifyOutcome::Matched(verified) => verified,
        BackResolutionVerifyOutcome::Mismatch { .. } => {
            panic!("the exact Back lineage must verify")
        }
    };
    let (_view, owner) = verified.into_parts();
    let target = match owner {
        VerifiedBackResolutionOwnerHandoff::Target(owner) => owner,
        VerifiedBackResolutionOwnerHandoff::NoTarget(_)
        | VerifiedBackResolutionOwnerHandoff::Contradiction(_) => {
            panic!("the two-entry history must resolve a real target")
        }
    };
    engine.testing_request_back_target_source_drift_before_freeze_once();
    let retirement_calls = std::cell::Cell::new(0usize);
    let permanent = match engine.resume_back_target_with_retirement(target, || {
        retirement_calls.set(retirement_calls.get() + 1);
    }) {
        BackTargetResumeOutcome::Permanent(owner) => owner,
        BackTargetResumeOutcome::Resumed(_) => {
            panic!("the real post-refreeze source drift must form a permanent owner")
        }
        BackTargetResumeOutcome::Mismatch(_) => {
            panic!("the exact target receiver must not reject its real owner")
        }
    };
    assert_eq!(retirement_calls.get(), 1);
    let owner_snapshot = permanent.testing_evidence_snapshot();
    assert_eq!(
        owner_snapshot.cause(),
        BackTargetPermanentCauseSnapshot::SourceCapability
    );
    assert!(matches!(
        engine.request(permanent_request).unwrap().status(),
        RequestStatus::Queued
    ));
    assert_eq!(
        engine
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                )
            })
            .count(),
        TERMINAL_RETENTION,
        "the permanent target must remain queued beside a full terminal window"
    );
    let watermark_before = engine.testing_evicted_watermark();

    let terminal = match engine.settle_back_target_permanent_exact(permanent) {
        BackTargetPermanentSettlementOutcome::Settled(terminal) => terminal,
        BackTargetPermanentSettlementOutcome::Mismatch { .. } => {
            panic!("the exact permanent target owner must settle under retention pressure")
        }
    };

    assert_eq!(terminal.testing_evidence_snapshot(), owner_snapshot);
    assert!(matches!(
        engine.private_lookup(evicted_request),
        PrivateRequestLookup::Evicted
    ));
    assert!(matches!(
        engine.private_lookup(next_earliest_request),
        PrivateRequestLookup::Retained(_)
    ));
    assert_eq!(
        engine.testing_evicted_watermark(),
        Some(evicted_request.sequence())
    );
    assert!(watermark_before.is_none_or(|watermark| watermark < evicted_request.sequence()));
    assert_eq!(
        engine.earliest_retained_sequence(),
        Some(next_earliest_request.sequence())
    );
    assert!(
        engine
            .issued_view_tasks
            .iter()
            .all(|identity| identity.request != evicted_request)
    );
    assert!(
        engine
            .report_dispositions
            .iter()
            .all(|record| { record.target != super::DiagnosticTarget::Request(evicted_request) })
    );
    assert!(
        engine
            .completed_cleanup
            .iter()
            .all(|tombstone| tombstone.request != evicted_request)
    );
    let permanent_record = engine
        .request(permanent_request)
        .expect("the new permanent target request remains retained");
    assert!(matches!(
        permanent_record.status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert_eq!(
        permanent_record
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .count(),
        1
    );
    assert_eq!(
        engine
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.status(),
                    RequestStatus::Succeeded | RequestStatus::Failed(_)
                )
            })
            .count(),
        TERMINAL_RETENTION
    );
}

#[test]
fn confirmed_engine_rejects_new_host_admission_before_touching_fresh_issuer() {
    let mut engine = ready_engine();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let before = engine.snapshot();

    assert_eq!(
        engine.request_legacy_host_admission(&mut issuer),
        Err(crate::admission::AdmissionError::AuthorityExists)
    );
    assert!(!issuer.has_outstanding());
    assert_eq!(engine.snapshot(), before);
}

fn r3_initial_host_admission_permit(engine: &mut RouterEngine) -> HostAdmissionSourcePermit {
    match engine.issue_host_admission_source_permit() {
        HostAdmissionSourcePermitOutcome::Issued(permit) => {
            assert_eq!(permit.source.kind, HostAdmissionSourceKind::InitialAwaiting);
            assert_eq!(permit.source.public_reason, None);
            permit
        }
        HostAdmissionSourcePermitOutcome::Ineligible { .. } => {
            panic!("an initial AwaitingHost source must be eligible")
        }
        HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            panic!("a fresh engine must not have an outstanding admission owner")
        }
    }
}

fn r3_bind_candidate_unavailable<Evidence>(
    engine: &RouterEngine,
    issuer: &HostAdmissionIssuer,
    source: &HostAdmissionSourceProjection,
    kind: HostAdmissionUnavailableEvidenceKind,
    candidate_revision: u64,
    expected_digest: u64,
    observed_digest: u64,
    evidence: Evidence,
) -> PreparedHostAdmissionUnavailableCause<Evidence> {
    let projection = HostAdmissionUnavailableEvidenceProjection {
        source: source.clone(),
        kind,
        candidate_revision: Some(candidate_revision),
        expected_digest,
        observed_digest,
        exhaustion: None,
        operation_failure: None,
    };
    match engine.bind_host_admission_unavailable_cause(
        issuer,
        source.clone(),
        &projection,
        evidence,
    ) {
        HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => prepared,
        HostAdmissionUnavailableCauseBindOutcome::Mismatch { .. } => {
            panic!("the exact candidate evidence must bind to its source")
        }
    }
}

fn r3_source_bound_token(
    engine: &mut RouterEngine,
    issuer: &mut HostAdmissionIssuer,
) -> SourceBoundHostAdmissionToken {
    let permit = r3_initial_host_admission_permit(engine);
    match engine.request_host_admission(issuer, permit) {
        Ok(token) => token,
        Err(_) => panic!("an exact initial permit must issue one source-bound token"),
    }
}

fn r3_source_bound_grant(
    engine: &mut RouterEngine,
    issuer: &mut HostAdmissionIssuer,
) -> SourceBoundHostGrant {
    let token = r3_source_bound_token(engine, issuer);
    match engine.grant_host_admission(issuer, token) {
        Ok(grant) => grant,
        Err(_) => panic!("an exact source-bound token must issue one source-bound grant"),
    }
}

fn r3_conflict_pre_staging_source<Installation, Evidence>(
    engine: &mut RouterEngine,
    issuer: &mut HostAdmissionIssuer,
    installation: Installation,
    evidence: Evidence,
) -> HostAdmissionUnavailablePreStagingSource<Installation, Evidence> {
    let token = r3_source_bound_token(engine, issuer);
    let source = token.permit.source.clone();
    let prepared = r3_bind_candidate_unavailable(
        engine,
        issuer,
        &source,
        HostAdmissionUnavailableEvidenceKind::Conflict,
        7,
        0x434f_4e46_4c49_4354,
        0x434f_4e46_4c49_4354,
        evidence,
    );
    HostAdmissionUnavailablePreStagingSource {
        prepared,
        owner: HostAdmissionUnavailablePreStagingOwner::Token(token),
        installation,
    }
}

#[test]
fn host_admission_source_permit_reopens_only_changed_retryable_unavailable() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("source-permit-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let permit = r3_initial_host_admission_permit(&mut engine);
    let initial_source = permit.source.clone();
    let prepared = r3_bind_candidate_unavailable(
        &engine,
        &issuer,
        &initial_source,
        HostAdmissionUnavailableEvidenceKind::Conflict,
        1,
        0x1111,
        0x2222,
        Box::new(1_u64),
    );
    let source = HostAdmissionUnavailablePreStagingSource {
        prepared,
        owner: HostAdmissionUnavailablePreStagingOwner::Permit(permit),
        installation: Box::new(0xA11C_u64),
    };
    let published = engine.complete_pre_staging_host_admission_unavailable(
        &mut issuer,
        source,
        |installation| installation,
    );
    let first_terminal = match published {
        HostAdmissionUnavailablePreStagingOutcome::Published { terminal, physical } => {
            assert_eq!(*physical, 0xA11C_u64);
            terminal
        }
        HostAdmissionUnavailablePreStagingOutcome::Mismatch { .. } => {
            panic!("an exact permit owner must publish its derived conflict")
        }
    };
    assert_eq!(first_terminal.reason, RouterUnavailableReason::HostConflict);
    assert_eq!(
        first_terminal.permanence,
        HostAdmissionPermanence::RetryableAfterCandidateChange
    );

    let unavailable_source = match engine.issue_host_admission_source_permit() {
        HostAdmissionSourcePermitOutcome::Ineligible { source, .. } => source,
        HostAdmissionSourcePermitOutcome::Issued(_) => {
            panic!("an unchanged retryable source must not reopen")
        }
        HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            panic!("the conflict terminal consumed the permit owner")
        }
    };
    let changed = r3_bind_candidate_unavailable(
        &engine,
        &issuer,
        &unavailable_source,
        HostAdmissionUnavailableEvidenceKind::Conflict,
        2,
        0x2222,
        0x3333,
        Box::new(2_u64),
    );
    let clear_calls = std::cell::Cell::new(0usize);
    match engine.complete_observed_host_admission_unavailable(&issuer, changed, || {
        clear_calls.set(clear_calls.get() + 1);
    }) {
        HostAdmissionUnavailableObservedOutcome::Published(terminal) => {
            assert_eq!(terminal.reason, RouterUnavailableReason::HostConflict);
        }
        HostAdmissionUnavailableObservedOutcome::Awaiting { .. }
        | HostAdmissionUnavailableObservedOutcome::Stable { .. }
        | HostAdmissionUnavailableObservedOutcome::Mismatch { .. } => {
            panic!("a changed retryable observation must publish a fresh source")
        }
    }
    assert_eq!(clear_calls.get(), 1);
    let reopened = match engine.issue_host_admission_source_permit() {
        HostAdmissionSourcePermitOutcome::Issued(permit) => permit,
        HostAdmissionSourcePermitOutcome::Ineligible { .. }
        | HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            panic!("only the changed retryable source must reopen")
        }
    };
    assert!(
        reopened.source.availability_incarnation.0 > unavailable_source.availability_incarnation.0
    );

    let mut permanent_engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("source-permit-permanent-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut permanent_issuer = HostAdmissionIssuer::new().unwrap();
    let permanent_permit = r3_initial_host_admission_permit(&mut permanent_engine);
    let permanent_source = permanent_permit.source.clone();
    let permanent_projection = HostAdmissionUnavailableEvidenceProjection {
        source: permanent_source.clone(),
        kind: HostAdmissionUnavailableEvidenceKind::AuthorityEstablishmentFailed,
        candidate_revision: Some(u64::MAX),
        expected_digest: 0xE7A0,
        observed_digest: 0xE7A0,
        exhaustion: Some(HostAdmissionAllocatorExhaustionEvidence {
            counter: HostAdmissionAllocatorCounter::CandidateRevision,
            current: u64::MAX,
            attempted: u64::MAX,
        }),
        operation_failure: None,
    };
    let permanent_prepared = match permanent_engine.bind_host_admission_unavailable_cause(
        &permanent_issuer,
        permanent_source,
        &permanent_projection,
        Box::new(3_u64),
    ) {
        HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => prepared,
        HostAdmissionUnavailableCauseBindOutcome::Mismatch { .. } => {
            panic!("candidate revision exhaustion must bind as a permanent source")
        }
    };
    let permanent_owner = HostAdmissionUnavailablePreStagingSource {
        prepared: permanent_prepared,
        owner: HostAdmissionUnavailablePreStagingOwner::Permit(permanent_permit),
        installation: Box::new(0xE7A0_u64),
    };
    let permanent_terminal = match permanent_engine.complete_pre_staging_host_admission_unavailable(
        &mut permanent_issuer,
        permanent_owner,
        |installation| installation,
    ) {
        HostAdmissionUnavailablePreStagingOutcome::Published { terminal, .. } => terminal,
        HostAdmissionUnavailablePreStagingOutcome::Mismatch { .. } => {
            panic!("the exact permanent permit owner must publish")
        }
    };
    assert_eq!(
        permanent_terminal.permanence,
        HostAdmissionPermanence::PermanentForRouter
    );
    match permanent_engine.issue_host_admission_source_permit() {
        HostAdmissionSourcePermitOutcome::Ineligible { source, .. } => {
            assert_eq!(
                source.kind,
                HostAdmissionSourceKind::PermanentIdentityExhausted
            );
        }
        HostAdmissionSourcePermitOutcome::Issued(_)
        | HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            panic!("a permanent source must never reopen")
        }
    }
}

#[test]
fn pre_staging_permit_unavailable_refreeze_rejects_idle_issuer_counter_drift() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("permit-unavailable-issuer-drift-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let permit = r3_initial_host_admission_permit(&mut engine);
    let source = permit.source.clone();
    let prepared = r3_bind_candidate_unavailable(
        &engine,
        &issuer,
        &source,
        HostAdmissionUnavailableEvidenceKind::Conflict,
        3,
        0x3333,
        0x4444,
        Box::new(0x51A1_u64),
    );
    let expected_issuer = issuer.projection();
    let token = issuer.issue(engine.router).unwrap();
    issuer.cancel(&token).unwrap();
    let observed_issuer = issuer.projection();
    assert_ne!(observed_issuer, expected_issuer);
    assert!(!issuer.has_outstanding());
    let source = HostAdmissionUnavailablePreStagingSource {
        prepared,
        owner: HostAdmissionUnavailablePreStagingOwner::Permit(permit),
        installation: Box::new(0x51A2_u64),
    };
    let callback_calls = std::cell::Cell::new(0usize);

    let outcome = engine.complete_pre_staging_host_admission_unavailable(
        &mut issuer,
        source,
        |installation| {
            callback_calls.set(callback_calls.get() + 1);
            installation
        },
    );
    let HostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence } = outcome else {
        panic!("permit completion must reject cause-bound issuer drift")
    };
    let HostAdmissionUnavailableFailureEvidence::Issuer(mismatch) = evidence else {
        panic!("permit issuer drift must retain typed issuer evidence")
    };
    assert_eq!(
        mismatch.operation,
        HostAdmissionUnavailableIssuerOperation::RefreezeCause
    );
    assert_eq!(mismatch.expected_issuer, expected_issuer);
    assert_eq!(mismatch.observed_issuer, observed_issuer);
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(*source.installation, 0x51A2_u64);
    assert_eq!(*source.prepared.evidence, 0x51A1_u64);
    let HostAdmissionUnavailablePreStagingOwner::Permit(permit) = source.owner else {
        panic!("issuer drift must return the original permit owner")
    };
    assert_eq!(permit.source, source.prepared.core.source);
    assert_eq!(engine.availability(), &RouterAvailability::AwaitingHost);
}

#[test]
fn observed_host_unavailable_refreeze_rejects_idle_issuer_counter_drift() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("observed-unavailable-issuer-drift-fallback").unwrap(),
    )
    .unwrap();
    let source = engine.testing_host_admission_source_projection();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let prepared = r3_bind_candidate_unavailable(
        &engine,
        &issuer,
        &source,
        HostAdmissionUnavailableEvidenceKind::Conflict,
        2,
        0x1111,
        0x2222,
        Box::new(0x1D1E_u64),
    );
    let expected_issuer = issuer.projection();
    let token = issuer.issue(engine.router).unwrap();
    issuer.cancel(&token).unwrap();
    let observed_issuer = issuer.projection();
    assert!(observed_issuer.outstanding.is_none());
    assert!(observed_issuer.current.is_none());
    assert_ne!(observed_issuer, expected_issuer);

    let clear_calls = std::cell::Cell::new(0usize);
    let outcome = engine.complete_observed_host_admission_unavailable(&issuer, prepared, || {
        clear_calls.set(clear_calls.get() + 1)
    });
    let HostAdmissionUnavailableObservedOutcome::Mismatch { prepared, evidence } = outcome else {
        panic!("idle issuer counter drift must reject final unavailable publication")
    };
    let HostAdmissionUnavailableFailureEvidence::ObservedReceiver(mismatch) = evidence else {
        panic!("issuer drift must retain the full observed mismatch")
    };
    assert_eq!(mismatch.expected_issuer, expected_issuer);
    assert_eq!(mismatch.observed_issuer, observed_issuer);
    assert_eq!(
        mismatch.expected_owner_inventory,
        mismatch.observed_owner_inventory
    );
    assert_eq!(*prepared.evidence, 0x1D1E_u64);
    assert_eq!(clear_calls.get(), 0);
}

#[test]
fn observed_host_unavailable_final_refreeze_rejects_ready_active_running_authority() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("observed-unavailable-refreeze-fallback").unwrap(),
    )
    .unwrap();
    let initial_source = engine.testing_host_admission_source_projection();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let stale_prepared = r3_bind_candidate_unavailable(
        &engine,
        &issuer,
        &initial_source,
        HostAdmissionUnavailableEvidenceKind::Conflict,
        2,
        0x1111,
        0x2222,
        Box::new(0xC0DE_u64),
    );

    let grant = r3_source_bound_grant(&mut engine, &mut issuer);
    let owner = bind_granted_host_admission_source(grant, ());
    let prepared = match engine.prepare_host_confirmation(owner) {
        HostConfirmationPreparationOutcome::Prepared(prepared) => prepared,
        HostConfirmationPreparationOutcome::ReceiverMismatch { .. } => {
            panic!("the exact admission owner must prepare confirmation")
        }
    };
    let lease = match engine.confirm_host_admission_with_binding(
        &mut issuer,
        prepared,
        |(), lease| lease,
    ) {
        HostAdmissionConfirmationCoreOutcome::Ready { installed, .. } => installed,
        HostAdmissionConfirmationCoreOutcome::ReceiverMismatch { .. }
        | HostAdmissionConfirmationCoreOutcome::IssuerRejected { .. } => {
            panic!("the exact source-bound admission must publish Ready")
        }
    };
    engine.push(RouteLocation::new("/home")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();

    let engine_before = engine.snapshot();
    let issuer_before = issuer.projection();
    let clear_calls = std::cell::Cell::new(0usize);
    let outcome =
        engine.complete_observed_host_admission_unavailable(&issuer, stale_prepared, || {
            clear_calls.set(clear_calls.get() + 1);
        });
    let HostAdmissionUnavailableObservedOutcome::Mismatch { prepared, evidence } = outcome else {
        panic!("a stale unavailable observation must not overwrite live Ready authority")
    };

    assert_eq!(*prepared.evidence, 0xC0DE_u64);
    let HostAdmissionUnavailableFailureEvidence::ObservedReceiver(mismatch) = evidence else {
        panic!("the stale observation must retain its full observed receiver projection")
    };
    assert_eq!(mismatch.expected_source, initial_source);
    assert_eq!(mismatch.observed_availability, RouterAvailability::Ready);
    assert!(mismatch.observed_host.is_some());
    assert!(mismatch.observed_top_slot.is_some());
    assert_ne!(mismatch.observed_active, ActivePresentation::none());
    assert!(mismatch.observed_request.is_some());
    assert!(mismatch.observed_current_identity.is_some());
    assert!(mismatch.observed_current_stage.is_some());
    assert!(mismatch.observed_action_present);
    assert_ne!(
        mismatch.observed_owner_inventory,
        mismatch.expected_owner_inventory
    );
    assert_eq!(
        &mismatch.expected_issuer,
        prepared.core.expected_issuer.as_ref()
    );
    assert!(mismatch.observed_issuer.current.is_some());
    assert_ne!(mismatch.observed_issuer, mismatch.expected_issuer);
    assert_eq!(clear_calls.get(), 0);
    assert_eq!(engine.snapshot(), engine_before);
    assert_eq!(issuer.projection(), issuer_before);
    drop(lease);
}

#[test]
fn host_admission_unavailable_cause_is_derived_from_exact_observation() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("derived-unavailable-r3-fallback").unwrap(),
    )
    .unwrap();
    let permit = r3_initial_host_admission_permit(&mut engine);
    let source = permit.source.clone();
    let projection = HostAdmissionUnavailableEvidenceProjection {
        source: source.clone(),
        kind: HostAdmissionUnavailableEvidenceKind::Conflict,
        candidate_revision: Some(11),
        expected_digest: 0xABCD,
        observed_digest: 0xDCBA,
        exhaustion: None,
        operation_failure: None,
    };
    let evidence = Box::new(0xCAFE_u64);
    let issuer = HostAdmissionIssuer::new().unwrap();

    let prepared = match engine.bind_host_admission_unavailable_cause(
        &issuer,
        source.clone(),
        &projection,
        evidence,
    ) {
        HostAdmissionUnavailableCauseBindOutcome::Bound(prepared) => prepared,
        HostAdmissionUnavailableCauseBindOutcome::Mismatch { .. } => {
            panic!("an exact observation must bind")
        }
    };
    assert_eq!(prepared.core.source, source);
    assert_eq!(prepared.core.evidence, projection);
    assert_eq!(
        prepared.core.disposition,
        HostAdmissionUnavailableDisposition::Unavailable {
            reason: RouterUnavailableReason::HostConflict,
            permanence: HostAdmissionPermanence::RetryableAfterCandidateChange,
        }
    );
    assert_ne!(prepared.core.cause_digest, 0);
    assert_eq!(prepared.core.expected_issuer.as_ref(), &issuer.projection());
    assert_eq!(*prepared.evidence, 0xCAFE_u64);
}

#[test]
fn host_unavailable_rejection_clears_issuer_before_publication() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("unavailable-rejection-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let source = r3_conflict_pre_staging_source(
        &mut engine,
        &mut issuer,
        Box::new(0x5155_u64),
        Box::new(0xE11D_u64),
    );
    let callback_calls = std::cell::Cell::new(0usize);

    let outcome = engine.complete_pre_staging_host_admission_unavailable(
        &mut issuer,
        source,
        |installation| {
            callback_calls.set(callback_calls.get() + 1);
            installation
        },
    );

    let HostAdmissionUnavailablePreStagingOutcome::Published { terminal, physical } = outcome
    else {
        panic!("the exact token cancellation must publish")
    };
    assert_eq!(callback_calls.get(), 1);
    assert_eq!(*physical, 0x5155_u64);
    assert!(!issuer.has_outstanding());
    assert_eq!(terminal.reason, RouterUnavailableReason::HostConflict);
    assert_eq!(
        engine.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostConflict)
    );
}

#[test]
fn host_unavailable_issuer_failure_returns_typed_observed_evidence() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("unavailable-issuer-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut owning_issuer = HostAdmissionIssuer::new().unwrap();
    let source = r3_conflict_pre_staging_source(
        &mut engine,
        &mut owning_issuer,
        Box::new(0x5156_u64),
        Box::new(0xE12D_u64),
    );
    let expected_issuer = owning_issuer.projection();
    let mut observed_issuer = HostAdmissionIssuer::new().unwrap();
    let observed = observed_issuer.projection();
    let callback_calls = std::cell::Cell::new(0usize);

    let outcome = engine.complete_pre_staging_host_admission_unavailable(
        &mut observed_issuer,
        source,
        |installation| {
            callback_calls.set(callback_calls.get() + 1);
            installation
        },
    );
    let HostAdmissionUnavailablePreStagingOutcome::Mismatch { source, evidence } = outcome else {
        panic!("a foreign issuer must return the prepared source whole")
    };
    let HostAdmissionUnavailableFailureEvidence::Issuer(mismatch) = evidence else {
        panic!("the exact Core receiver must classify only issuer drift")
    };
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(
        mismatch.operation,
        HostAdmissionUnavailableIssuerOperation::RefreezeCause
    );
    assert_eq!(mismatch.expected_issuer, expected_issuer);
    assert_eq!(mismatch.observed_issuer, observed);
    assert!(owning_issuer.has_outstanding());
    assert_eq!(*source.installation, 0x5156_u64);
    assert_eq!(*source.prepared.evidence, 0xE12D_u64);
    let HostAdmissionUnavailablePreStagingOwner::Token(token) = source.owner else {
        panic!("issuer rejection must return the source-bound token")
    };
    assert_eq!(token.permit.source, source.prepared.core.source);
    assert_eq!(engine.availability(), &RouterAvailability::AwaitingHost);
}

#[test]
fn host_admission_request_sequence_exhaustion_is_permanent_and_exact() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("request-exhaustion-r3-fallback").unwrap(),
    )
    .unwrap();
    let permit = r3_initial_host_admission_permit(&mut engine);
    let expected_source = permit.source.clone();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    issuer.testing_exhaust_request_sequence_counter();
    let expected_issuer = issuer.projection();
    let issuer_before = issuer.snapshot();

    let error = match engine.request_host_admission(&mut issuer, permit) {
        Err(error) => error,
        Ok(_) => panic!("request-sequence exhaustion must reject the permit"),
    };
    assert_eq!(issuer.snapshot(), issuer_before);
    assert_eq!(error.owner.source, expected_source);
    let HostAdmissionOperationFailureFreezeOutcome::Frozen(evidence) = error.evidence else {
        panic!("an exact request exhaustion pair must freeze")
    };
    assert_eq!(
        evidence.projection.operation,
        HostAdmissionOperation::Request
    );
    assert_eq!(evidence.projection.error, AdmissionError::Exhausted);
    assert_eq!(
        evidence.projection.disposition,
        HostAdmissionOperationFailureDisposition::PermanentIdentityExhausted
    );
    assert_eq!(evidence.projection.expected_issuer, expected_issuer);
    assert_eq!(evidence.projection.observed_issuer, expected_issuer);
    assert_eq!(
        evidence.projection.exhaustion,
        Some(HostAdmissionAllocatorExhaustionEvidence {
            counter: HostAdmissionAllocatorCounter::RequestSequence,
            current: u64::MAX,
            attempted: u64::MAX,
        })
    );
}

#[test]
fn host_admission_grant_identity_exhaustion_is_permanent_and_exact() {
    for counter in [
        HostAdmissionAllocatorCounter::GrantEpoch,
        HostAdmissionAllocatorCounter::HostVersion,
        HostAdmissionAllocatorCounter::TopSlot,
    ] {
        let mut engine = RouterEngine::install(
            tree(),
            RouteViewKey::new("grant-exhaustion-r3-fallback").unwrap(),
        )
        .unwrap();
        let mut issuer = HostAdmissionIssuer::new().unwrap();
        let token = r3_source_bound_token(&mut engine, &mut issuer);
        let expected_source = token.permit.source.clone();
        match counter {
            HostAdmissionAllocatorCounter::GrantEpoch => {
                issuer.testing_exhaust_grant_epoch_counter();
            }
            HostAdmissionAllocatorCounter::HostVersion => {
                engine.ids.testing_exhaust_host_counter();
            }
            HostAdmissionAllocatorCounter::TopSlot => {
                engine.ids.testing_exhaust_slot_counter();
            }
            HostAdmissionAllocatorCounter::RegistrationIncarnation
            | HostAdmissionAllocatorCounter::CandidateRevision
            | HostAdmissionAllocatorCounter::RequestSequence => unreachable!(),
        }
        let expected_issuer = issuer.projection();
        let issuer_before = issuer.snapshot();
        let ids_before = engine.ids.snapshot();

        let error = match engine.grant_host_admission(&mut issuer, token) {
            Err(error) => error,
            Ok(_) => panic!("grant identity exhaustion must return the token owner"),
        };
        assert_eq!(issuer.snapshot(), issuer_before, "counter: {counter:?}");
        assert_eq!(engine.ids.snapshot(), ids_before, "counter: {counter:?}");
        assert_eq!(error.owner.permit.source, expected_source);
        let HostAdmissionOperationFailureFreezeOutcome::Frozen(evidence) = error.evidence else {
            panic!("an exact grant exhaustion pair must freeze: {counter:?}")
        };
        assert_eq!(evidence.projection.operation, HostAdmissionOperation::Grant);
        assert_eq!(evidence.projection.error, AdmissionError::Exhausted);
        assert_eq!(
            evidence.projection.disposition,
            HostAdmissionOperationFailureDisposition::PermanentIdentityExhausted
        );
        assert_eq!(evidence.projection.expected_issuer, expected_issuer);
        assert_eq!(evidence.projection.observed_issuer, expected_issuer);
        assert_eq!(
            evidence.projection.exhaustion,
            Some(HostAdmissionAllocatorExhaustionEvidence {
                counter,
                current: u64::MAX,
                attempted: u64::MAX,
            })
        );
    }
}

#[test]
fn host_confirmation_receiver_mismatch_returns_whole_owner_and_calls_zero() {
    let mut source_engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("confirmation-source-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let grant = r3_source_bound_grant(&mut source_engine, &mut issuer);
    let expected_source = grant.source.permit.source.clone();
    let expected_version = grant.grant.version().clone();
    let expected_top_slot = grant.grant.top_slot();
    let owner = bind_granted_host_admission_source(grant, Box::new(0xC0DE_u64));
    let mut foreign_engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("confirmation-foreign-r3-fallback").unwrap(),
    )
    .unwrap();
    let callback_calls = std::cell::Cell::new(0usize);

    let outcome = foreign_engine.prepare_host_confirmation(owner);
    let HostConfirmationPreparationOutcome::ReceiverMismatch { owner, evidence } = outcome else {
        panic!("a foreign Core receiver must reject before Confirming")
    };
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(evidence.expected.source, expected_source);
    assert!(evidence.observed.is_none());
    assert_eq!(owner.grant.grant.version(), &expected_version);
    assert_eq!(owner.grant.grant.top_slot(), expected_top_slot);
    assert_eq!(*owner.installation, 0xC0DE_u64);
    assert!(issuer.is_outstanding_grant(&owner.grant.grant));
}

#[test]
fn host_confirmation_issuer_rejection_is_last_fallible_and_returns_whole_owner() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("confirmation-issuer-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let grant = r3_source_bound_grant(&mut engine, &mut issuer);
    let expected_version = grant.grant.version().clone();
    let expected_top_slot = grant.grant.top_slot();
    let owner = bind_granted_host_admission_source(grant, Box::new(0x1A57_u64));
    let prepared = match engine.prepare_host_confirmation(owner) {
        HostConfirmationPreparationOutcome::Prepared(prepared) => prepared,
        HostConfirmationPreparationOutcome::ReceiverMismatch { .. } => {
            panic!("the exact receiver must prepare confirmation")
        }
    };
    let mut foreign_issuer = HostAdmissionIssuer::new().unwrap();
    let observed_issuer = foreign_issuer.projection();
    let callback_calls = std::cell::Cell::new(0usize);

    let outcome = engine.confirm_host_admission_with_binding(
        &mut foreign_issuer,
        prepared,
        |installation, _lease| {
            callback_calls.set(callback_calls.get() + 1);
            installation
        },
    );
    let HostAdmissionConfirmationCoreOutcome::IssuerRejected { owner, evidence } = outcome else {
        panic!("foreign issuer drift must return the confirming owner")
    };
    assert_eq!(callback_calls.get(), 0);
    assert_eq!(evidence.observed_issuer, observed_issuer);
    assert_eq!(owner.grant.grant.version(), &expected_version);
    assert_eq!(owner.grant.grant.top_slot(), expected_top_slot);
    assert_eq!(*owner.installation, 0x1A57_u64);
    assert_eq!(engine.availability(), &RouterAvailability::AwaitingHost);
}

#[test]
fn host_confirmation_unwind_returns_grant_and_installation() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("confirmation-unwind-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let grant = r3_source_bound_grant(&mut engine, &mut issuer);
    let expected_source = grant.source.permit.source.clone();
    let expected_version = grant.grant.version().clone();
    let expected_top_slot = grant.grant.top_slot();
    let owner = bind_granted_host_admission_source(grant, Box::new(0x5154_u64));
    let prepared = match engine.prepare_host_confirmation(owner) {
        HostConfirmationPreparationOutcome::Prepared(prepared) => prepared,
        HostConfirmationPreparationOutcome::ReceiverMismatch { .. } => {
            panic!("the exact receiver must prepare confirmation")
        }
    };

    let outcome = engine.unwind_host_confirmation(prepared);
    let HostConfirmationUnwindOutcome::Grant(owner) = outcome else {
        panic!("an unchanged Confirming owner must unwind to Grant")
    };
    assert_eq!(owner.grant.source.permit.source, expected_source);
    assert_eq!(owner.grant.grant.version(), &expected_version);
    assert_eq!(owner.grant.grant.top_slot(), expected_top_slot);
    assert_eq!(*owner.installation, 0x5154_u64);
    assert!(issuer.is_outstanding_grant(&owner.grant.grant));
}

#[test]
fn host_confirmation_callback_installs_binding_before_ready_publication() {
    let mut engine = RouterEngine::install(
        tree(),
        RouteViewKey::new("confirmation-ready-r3-fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let grant = r3_source_bound_grant(&mut engine, &mut issuer);
    let expected_router = grant.grant.version().router_identity();
    let expected_version = grant.grant.version().clone();
    let expected_top_slot = grant.grant.top_slot();
    let owner = bind_granted_host_admission_source(grant, Box::new(0xB1AD_u64));
    let prepared = match engine.prepare_host_confirmation(owner) {
        HostConfirmationPreparationOutcome::Prepared(prepared) => prepared,
        HostConfirmationPreparationOutcome::ReceiverMismatch { .. } => {
            panic!("the exact receiver must prepare confirmation")
        }
    };
    let callback_calls = std::cell::Cell::new(0usize);

    let outcome =
        engine.confirm_host_admission_with_binding(&mut issuer, prepared, |installation, lease| {
            callback_calls.set(callback_calls.get() + 1);
            (installation, lease)
        });
    let HostAdmissionConfirmationCoreOutcome::Ready {
        projection,
        installed,
    } = outcome
    else {
        panic!("the exact final issuer operation must publish Ready")
    };
    assert_eq!(callback_calls.get(), 1);
    assert_eq!(*installed.0, 0xB1AD_u64);
    assert!(
        installed
            .1
            .matches(expected_router, &expected_version, expected_top_slot)
    );
    assert_eq!(projection.host, expected_version);
    assert_eq!(projection.top_slot, expected_top_slot);
    assert_eq!(projection.availability, RouterAvailability::Ready);
    assert_eq!(engine.availability(), &RouterAvailability::Ready);
    assert_eq!(engine.host(), Some(&expected_version));
    assert_eq!(engine.top_slot(), Some(expected_top_slot));
}

#[test]
fn push_reservation_exhaustion_is_zero_change_for_every_counter() {
    for counter in [
        ReservationCounter::Request,
        ReservationCounter::Transaction,
        ReservationCounter::FrameTask,
        ReservationCounter::CandidateFrame,
        ReservationCounter::CleanupBatch,
    ] {
        let mut engine = ready_engine();
        engine.testing_exhaust_reservation_counter(counter);
        let before = engine.snapshot();

        assert_eq!(
            engine.push(RouteLocation::new("/items/1")),
            Err(super::PushError::IdExhausted),
            "counter: {counter:?}"
        );
        assert_eq!(engine.snapshot(), before, "counter: {counter:?}");
    }
}

#[test]
fn commit_ticket_is_logical_only_and_sealed_token_is_tracked_before_publication() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected frame task");
    };
    let task_identity = task.identity().clone();
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
    let super::CommitTicket { identity } = &ticket;
    assert_eq!(identity.request(), task_identity.request());
    let ids_after_action = engine.ids.snapshot();
    let sealed = engine
        .seal_commit(
            &ticket,
            CleanupManifestSeal::new(vec![], manifest_digest(3), 0),
        )
        .unwrap();
    let expected_cleanup = sealed.cleanup().clone();
    assert!(matches!(
        &engine.current.as_ref().unwrap().stage,
        WorkStage::CommitSealed { token } if token == &expected_cleanup
    ));
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert!(engine.history().is_empty());

    let mut mutated = false;
    publish_sealed(&mut engine, sealed, || {
        mutated = true;
    })
    .unwrap();

    assert!(mutated);
    assert_eq!(engine.ids.snapshot(), ids_after_action);
    assert_eq!(engine.active().kind(), ActivePresentationKind::Route);
}

#[test]
fn sealed_publication_preserves_later_queue_and_starts_next_request() {
    let mut engine = ready_engine();
    let first = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected first frame task");
    };
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
    let sealed = seal_current(&mut engine, ticket);
    let cleanup = sealed.cleanup().clone();

    let second = engine.push(RouteLocation::new("/about")).unwrap();
    let ids_after_second = engine.ids.snapshot();
    assert_eq!(
        engine.request(second).unwrap().status(),
        &RequestStatus::Queued
    );
    assert_eq!(engine.queue.front().unwrap().request, second);

    publish_sealed(&mut engine, sealed, || {}).unwrap();

    assert_eq!(engine.ids.snapshot(), ids_after_second);
    assert_eq!(
        engine.request(second).unwrap().status(),
        &RequestStatus::Queued
    );
    assert_eq!(engine.queue.front().unwrap().request, second);

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(engine.ids.snapshot(), ids_after_second);
    assert_eq!(
        engine.request(first).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(
        engine.request(second).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::ApplyingFrame(1))
    );
    let EngineAction::FrameTask(second_task) = engine.action().unwrap() else {
        panic!("second request must start after first cleanup");
    };
    assert_eq!(second_task.identity().request(), second);
}

#[test]
fn logical_cleanup_roots_are_retained_and_verified_in_exact_order() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();
    let first = active[0].clone();
    let child = active[1].clone();

    engine
        .push(RouteLocation::new("/items/1").with_fragment("details"))
        .unwrap();
    let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
        panic!("expected fragment task");
    };
    let fragment_ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(fragment));
    assert_eq!(engine.current.as_ref().unwrap().cleanup_roots, vec![child]);

    let second = RouteFrameVersion::new(
        first.router_identity(),
        first.host(),
        first.slot(),
        first.origin().clone(),
        RouteFrameId::new(first.frame().raw() + 10_000),
        first.generation(),
    );
    let foreign = RouteFrameVersion::new(
        first.router_identity(),
        first.host(),
        first.slot(),
        first.origin().clone(),
        RouteFrameId::new(first.frame().raw() + 20_000),
        first.generation(),
    );
    engine.current.as_mut().unwrap().cleanup_roots = vec![first.clone(), second.clone()];

    for invalid in [
        vec![],
        vec![first.clone()],
        vec![first.clone(), second.clone(), first.clone()],
        vec![first.clone(), first.clone()],
        vec![first.clone(), foreign],
        vec![second.clone(), first.clone()],
    ] {
        let before = engine.snapshot();
        assert_eq!(
            engine
                .seal_commit(
                    &fragment_ticket.testing_duplicate(),
                    CleanupManifestSeal::new(invalid, manifest_digest(4), 7),
                )
                .unwrap_err(),
            CommitError::InvalidManifest
        );
        assert_eq!(engine.snapshot(), before);
    }

    let sealed = engine
        .seal_commit(
            &fragment_ticket,
            CleanupManifestSeal::new(vec![first, second], manifest_digest(4), 7),
        )
        .unwrap();
    assert_eq!(sealed.cleanup().obligation_count(), 7);
}

#[test]
fn logical_roots_and_ecs_obligation_count_seal_independently() {
    fn ready_to_seal() -> (RouterEngine, super::CommitTicket) {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("expected frame task");
        };
        let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
        (engine, ticket)
    }

    let (mut empty_roots, empty_ticket) = ready_to_seal();
    let empty_digest = manifest_digest(5);
    let empty_expected = CleanupBatchToken::sealed(
        empty_ticket.identity().clone(),
        empty_roots.current.as_ref().unwrap().cleanup_batch,
        &[],
        empty_digest,
        1,
    );
    let empty_sealed = empty_roots
        .seal_commit(
            &empty_ticket,
            CleanupManifestSeal::new(Vec::new(), empty_digest, 1),
        )
        .expect("logical roots do not infer the ECS entity obligation count");
    assert_eq!(empty_sealed.cleanup(), &empty_expected);

    let (mut nonempty_roots, nonempty_ticket) = ready_to_seal();
    let root = nonempty_roots
        .current
        .as_ref()
        .unwrap()
        .target_frames
        .last()
        .unwrap()
        .clone();
    nonempty_roots.current.as_mut().unwrap().cleanup_roots = vec![root.clone()];
    let nonempty_digest = manifest_digest(6);
    let nonempty_expected = CleanupBatchToken::sealed(
        nonempty_ticket.identity().clone(),
        nonempty_roots.current.as_ref().unwrap().cleanup_batch,
        std::slice::from_ref(&root),
        nonempty_digest,
        0,
    );
    let nonempty_sealed = nonempty_roots
        .seal_commit(
            &nonempty_ticket,
            CleanupManifestSeal::new(vec![root], nonempty_digest, 0),
        )
        .expect("ECS can prove zero entity obligations for nonempty logical roots");
    assert_eq!(nonempty_sealed.cleanup(), &nonempty_expected);
}

#[test]
fn coherent_foreign_evidence_is_rejected_by_real_prepare_and_consumed_replay_is_zero_write() {
    let mut foreign_probe = ready_engine();
    foreign_probe.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = foreign_probe.action().unwrap() else {
        panic!("expected frame task");
    };
    let ticket = drive_to_commit(&mut foreign_probe, EngineAction::FrameTask(task));
    let sealed = seal_current(&mut foreign_probe, ticket);
    let foreign_identity = distinct_identity();
    let foreign = sealed.evidence().testing_with_observed_router_and_request(
        foreign_identity.router_identity(),
        foreign_identity.request(),
    );
    let before = foreign_probe.snapshot();
    let capability_before = foreign_probe.testing_history_capability_snapshot();
    assert_eq!(
        foreign_probe.probe_sealed_commit_evidence(&foreign),
        Err(CommitError::Stale)
    );
    assert_eq!(foreign_probe.snapshot(), before);
    assert_eq!(
        foreign_probe.testing_history_capability_snapshot(),
        capability_before
    );
    let prepared = foreign_probe
        .prepare_publication(sealed)
        .expect("foreign projection probing cannot consume the real sealed owner");
    foreign_probe.publish_prepared(prepared, || {}).unwrap();

    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected frame task");
    };
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
    let sealed = seal_current(&mut engine, ticket);
    let prepared = engine.prepare_publication(sealed).unwrap();
    let consumed = engine
        .publish_prepared_with_evidence(prepared, || {})
        .unwrap();
    let before = engine.snapshot();
    assert_eq!(
        engine.probe_consumed_publication_evidence(&consumed),
        Err(CommitError::Stale),
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn cross_incarnation_history_replay_is_zero_write() {
    fn stage_equal_history_owner(
        engine: &mut RouterEngine,
    ) -> (super::NavigationRequestId, super::SealedCommit) {
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(engine);
        let request = engine
            .replace(RouteLocation::new("/items/2"))
            .expect("the equal-shape fixture reserves its replacement");
        let action = engine.action().expect("the replacement has real work");
        let ticket = drive_to_commit(engine, action);
        (request, seal_current(engine, ticket))
    }

    let mut source = ready_engine();
    let mut receiver = ready_engine();
    let (source_request, source_owner) = stage_equal_history_owner(&mut source);
    let (receiver_request, receiver_owner) = stage_equal_history_owner(&mut receiver);

    assert_ne!(source.router, receiver.router);
    assert_eq!(source.history_snapshot(), receiver.history_snapshot());
    let source_engine_before = source.snapshot();
    let receiver_engine_before = receiver.snapshot();
    let source_history_before = source.testing_history_authority_snapshot();
    let receiver_history_before = receiver.testing_history_authority_snapshot();
    let source_request_before = source.request(source_request).unwrap().clone();
    let receiver_request_before = receiver.request(receiver_request).unwrap().clone();
    let source_owner_before = super::PreparedPublicationOwnerSnapshot::from(&source_owner);
    let receiver_owner_before = super::PreparedPublicationOwnerSnapshot::from(&receiver_owner);

    assert_eq!(
        receiver.probe_sealed_commit_evidence(source_owner.evidence()),
        Err(CommitError::Stale),
        "equal raw history shape cannot cross the router/history incarnation boundary"
    );

    assert_eq!(source.snapshot(), source_engine_before);
    assert_eq!(receiver.snapshot(), receiver_engine_before);
    assert_eq!(
        source.testing_history_authority_snapshot(),
        source_history_before
    );
    assert_eq!(
        receiver.testing_history_authority_snapshot(),
        receiver_history_before
    );
    assert_eq!(source.request(source_request), Some(&source_request_before));
    assert_eq!(
        receiver.request(receiver_request),
        Some(&receiver_request_before)
    );
    assert_eq!(
        super::PreparedPublicationOwnerSnapshot::from(&source_owner),
        source_owner_before
    );
    assert_eq!(
        super::PreparedPublicationOwnerSnapshot::from(&receiver_owner),
        receiver_owner_before
    );
}

#[test]
fn prepared_replace_keeps_exact_owner_in_core() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.replace(RouteLocation::new("/items/2")).unwrap();
    let action = engine
        .action()
        .expect("replace has one real execution action");
    let ticket = drive_to_commit(&mut engine, action);
    let exact_owner = engine.testing_history_capability_snapshot();
    assert!(exact_owner.has_exactly_one_owner());
    assert!(exact_owner.execution().is_some());
    assert!(exact_owner.source().is_some());

    let sealed = seal_current(&mut engine, ticket);
    let _ticket = engine
        .prepare_publication(sealed)
        .expect("the exact replace publication prepares");

    assert_eq!(
        engine.testing_history_capability_snapshot(),
        exact_owner,
        "preparation must leave the one exact authorizing owner in Core; the returned value is only a ticket"
    );
}

#[test]
fn prepare_rejection_retains_exact_owner_without_stage_reconstruction() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.replace(RouteLocation::new("/items/2")).unwrap();
    let action = engine
        .action()
        .expect("replace has one real execution action");
    let ticket = drive_to_commit(&mut engine, action);
    let exact_owner = engine.testing_history_capability_snapshot();
    let sealed = seal_current(&mut engine, ticket);
    engine.current.as_mut().unwrap().stage = WorkStage::Applying;
    let history_before = engine.testing_history_authority_snapshot();
    let state_before = engine.state.snapshot();
    let ids_before = engine.ids.snapshot();

    assert!(matches!(
        engine.prepare_publication(sealed),
        Err(CommitError::Stale)
    ));
    assert_eq!(engine.state.snapshot(), state_before);
    assert_eq!(engine.ids.snapshot(), ids_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert_eq!(engine.current.as_ref().unwrap().stage, WorkStage::Applying);
    assert_eq!(
        engine.testing_history_capability_snapshot(),
        exact_owner,
        "a real preparation rejection must retain the exact owner independently of WorkStage"
    );
}

#[test]
fn publication_refreeze_rejection_retains_owner_until_bounded_fail_closed() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.replace(RouteLocation::new("/items/2")).unwrap();
    let action = engine
        .action()
        .expect("replace has one real execution action");
    let ticket = drive_to_commit(&mut engine, action);
    let sealed = seal_current(&mut engine, ticket);
    let prepared = engine
        .prepare_publication(sealed)
        .expect("replace publication prepares before refreeze drift");
    let exact_owner = engine.testing_history_capability_snapshot();
    engine.current.as_mut().unwrap().stage = WorkStage::Applying;
    let state_before = engine.state.snapshot();
    let history_before = engine.testing_history_authority_snapshot();
    let mutation_ran = std::cell::Cell::new(false);

    assert_eq!(
        engine.publish_prepared(prepared, || mutation_ran.set(true)),
        Err(CommitError::Stale)
    );
    assert!(!mutation_ran.get());
    assert_eq!(engine.state.snapshot(), state_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert_eq!(engine.testing_history_capability_snapshot(), exact_owner);

    assert!(engine.fail_closed_authority_protocol(|| true));
    assert!(!engine.testing_history_capability_snapshot().has_owner());
}

#[test]
fn prepared_replace_ticket_after_fail_closed_is_exact_zero_write_and_ownerless() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let request = engine.replace(RouteLocation::new("/items/2")).unwrap();
    let action = engine
        .action()
        .expect("replace has one real execution action");
    let ticket = drive_to_commit(&mut engine, action);
    let sealed = seal_current(&mut engine, ticket);
    let prepared = engine
        .prepare_publication(sealed)
        .expect("replace publication prepares before authority loss");

    assert!(engine.fail_closed_authority_protocol(|| true));
    let terminal_state = engine.state.snapshot();
    let terminal_history = engine.testing_history_authority_snapshot();
    let terminal_status = engine.request(request).unwrap().status().clone();
    let terminal_trace = engine.request(request).unwrap().trace().to_vec();
    let terminal_owner = engine.testing_history_capability_snapshot();
    assert!(!terminal_owner.has_owner());
    assert!(engine.current.is_none());
    assert!(engine.action.is_none());

    let mutation_ran = std::cell::Cell::new(false);
    assert_eq!(
        engine.publish_prepared(prepared, || mutation_ran.set(true)),
        Err(CommitError::Stale)
    );

    assert_eq!(
        (
            mutation_ran.get(),
            engine.state.snapshot(),
            engine.testing_history_authority_snapshot(),
            engine.request(request).unwrap().status().clone(),
            engine.request(request).unwrap().trace().to_vec(),
            format!("{:?}", engine.current),
            engine.action.is_none(),
            engine.testing_history_capability_snapshot(),
        ),
        (
            false,
            terminal_state,
            terminal_history,
            terminal_status,
            terminal_trace,
            "None".to_owned(),
            true,
            terminal_owner,
        ),
        "a stale prepared ticket cannot run mutation, revive fail-closed state/current, or publish history"
    );
}

#[test]
fn running_replace_stale_report_is_exact_zero_write_for_history_and_owner() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
        panic!("empty-history replace starts with the real parent frame task")
    };
    let stale_parent = parent.identity().clone();
    let next_outlet = engine
        .current
        .as_ref()
        .unwrap()
        .branch
        .as_ref()
        .unwrap()
        .routes()[1]
        .slot()
        .clone();
    engine
        .accept_applied_report(parent.into_applied_report(vec![next_outlet]))
        .unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("the accepted parent report yields the real child mount")
    };
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0x51));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0x52));
    engine.complete_mount_registration(completion).unwrap();
    assert!(matches!(&engine.action, Some(EngineAction::FrameTask(_))));

    let status_before = engine.request(request).unwrap().status().clone();
    let trace_before = engine.request(request).unwrap().trace().to_vec();
    let state_before = engine.state.snapshot();
    let ids_before = engine.ids.snapshot();
    let history_before = engine.testing_history_authority_snapshot();
    let owner_before = engine.testing_history_capability_snapshot();
    let current_before = format!("{:?}", engine.current);
    let action_before = engine.action.as_ref().map(ActionSnapshot::from);

    assert_eq!(
        engine.begin_view_report(&stale_parent).map(|_| ()),
        Err(ReportError::Stale)
    );
    assert_eq!(engine.request(request).unwrap().status(), &status_before);
    assert_eq!(engine.request(request).unwrap().trace(), trace_before);
    assert_eq!(engine.state.snapshot(), state_before);
    assert_eq!(engine.ids.snapshot(), ids_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert_eq!(engine.testing_history_capability_snapshot(), owner_before);
    assert_eq!(format!("{:?}", engine.current), current_before);
    assert_eq!(
        engine.action.as_ref().map(ActionSnapshot::from),
        action_before
    );
}

#[test]
fn running_replace_old_cleanup_replay_is_exact_zero_write_for_history_and_owner() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let old_cleanup = publish_current(&mut engine);
    let old_replay = valid_completion(&old_cleanup);
    engine
        .complete_cleanup(valid_completion(&old_cleanup))
        .unwrap();

    let request = engine.replace(RouteLocation::new("/items/2")).unwrap();
    let status_before = engine.request(request).unwrap().status().clone();
    let trace_before = engine.request(request).unwrap().trace().to_vec();
    let state_before = engine.state.snapshot();
    let ids_before = engine.ids.snapshot();
    let history_before = engine.testing_history_authority_snapshot();
    let owner_before = engine.testing_history_capability_snapshot();
    let current_before = format!("{:?}", engine.current);
    let action_before = engine.action.as_ref().map(ActionSnapshot::from);

    assert_eq!(
        engine.complete_cleanup(old_replay),
        Err(CleanupError::Duplicate)
    );
    assert_eq!(engine.request(request).unwrap().status(), &status_before);
    assert_eq!(engine.request(request).unwrap().trace(), trace_before);
    assert_eq!(engine.state.snapshot(), state_before);
    assert_eq!(engine.ids.snapshot(), ids_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert_eq!(engine.testing_history_capability_snapshot(), owner_before);
    assert_eq!(format!("{:?}", engine.current), current_before);
    assert_eq!(
        engine.action.as_ref().map(ActionSnapshot::from),
        action_before
    );
}

#[test]
fn normal_root_binding_is_independently_refrozen_before_history_prepare() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.replace(RouteLocation::new("/items/2")).unwrap();
    let action = engine
        .action()
        .expect("replace has one real execution action");
    let ticket = drive_to_commit(&mut engine, action);
    let capability = engine.testing_history_capability_snapshot();
    let active_before = engine.active().clone();
    let history_before = engine.testing_history_authority_snapshot();
    let alternate = engine.current.as_ref().unwrap().reservation[1].clone();
    assert_ne!(
        engine.current.as_ref().unwrap().reservation[0].frame_task(),
        alternate.frame_task()
    );
    engine.current.as_mut().unwrap().reservation[0] = alternate;
    let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
    let count = usize::from(!roots.is_empty());

    assert_eq!(
        engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(roots, manifest_digest(0xB1AD), count),
            )
            .map(|_| ()),
        Err(CommitError::Stale),
        "the ticket identity cannot substitute for an exact normal-root refreeze"
    );
    let evidence = engine
        .testing_history_drift_evidence_snapshot()
        .expect("binding drift keeps typed evidence through cleanup");
    assert_eq!(
        evidence.kind(),
        crate::history_authority::HistoryDriftEvidenceKind::Binding
    );
    assert_eq!(evidence.capability(), &capability);
    assert_eq!(evidence.expected_binding(), capability.execution());
    assert_ne!(evidence.observed_binding(), capability.execution());
    assert!(evidence.capability().has_exactly_one_owner());
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);

    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!("binding drift enters bounded authority-loss cleanup")
    };
    let request = failure.request();
    let roots = failure.logical_roots().to_vec();
    let count = usize::from(!roots.is_empty());
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(roots, manifest_digest(0xB1AE), count),
        )
        .unwrap();
    assert!(engine.testing_history_drift_evidence_snapshot().is_some());
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    assert!(engine.testing_history_drift_evidence_snapshot().is_none());
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
}

#[test]
fn source_drift_keeps_exact_owner_and_observed_evidence_until_cleanup_terminal() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();
    settle_current(&mut engine);
    engine.replace(RouteLocation::new("/items/2")).unwrap();
    let action = engine
        .action()
        .expect("replace has one real execution action");
    let ticket = drive_to_commit(&mut engine, action);
    let capability = engine.testing_history_capability_snapshot();
    engine.history.testing_move_cursor_back();
    let active_before = engine.active().clone();
    let history_after_drift = engine.testing_history_authority_snapshot();
    let roots = engine.current.as_ref().unwrap().cleanup_roots.clone();
    let count = usize::from(!roots.is_empty());

    assert_eq!(
        engine
            .seal_commit(
                &ticket,
                CleanupManifestSeal::new(roots, manifest_digest(0x50A2), count),
            )
            .map(|_| ()),
        Err(CommitError::Stale)
    );
    let evidence = engine
        .testing_history_drift_evidence_snapshot()
        .expect("source drift keeps typed evidence through cleanup");
    assert_eq!(
        evidence.kind(),
        crate::history_authority::HistoryDriftEvidenceKind::Source
    );
    assert_eq!(evidence.capability(), &capability);
    assert_eq!(evidence.expected_source(), capability.source());
    assert_ne!(evidence.observed_source(), capability.source());
    assert!(evidence.capability().has_exactly_one_owner());
    assert_eq!(engine.active(), &active_before);
    assert_eq!(
        engine.testing_history_authority_snapshot(),
        history_after_drift
    );

    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!("source drift enters bounded authority-loss cleanup")
    };
    let request = failure.request();
    let roots = failure.logical_roots().to_vec();
    let count = usize::from(!roots.is_empty());
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(roots, manifest_digest(0x50A3), count),
        )
        .unwrap();
    assert!(engine.testing_history_drift_evidence_snapshot().is_some());
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    assert!(engine.testing_history_drift_evidence_snapshot().is_none());
    assert_eq!(engine.active(), &active_before);
    assert_eq!(
        engine.testing_history_authority_snapshot(),
        history_after_drift
    );
}

#[test]
fn manifest_digest_count_and_batch_are_bound_into_the_token() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/about")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("expected replacement task");
    };
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(task));
    let current = engine.current.as_ref().unwrap();
    let exact = CleanupBatchToken::sealed(
        ticket.identity().clone(),
        current.cleanup_batch,
        &current.cleanup_roots,
        manifest_digest(10),
        NONZERO_OBLIGATION_COUNT,
    );
    let different_manifest = CleanupBatchToken::sealed(
        ticket.identity().clone(),
        current.cleanup_batch,
        &current.cleanup_roots,
        manifest_digest(11),
        NONZERO_OBLIGATION_COUNT,
    );
    let different_count = CleanupBatchToken::sealed(
        ticket.identity().clone(),
        current.cleanup_batch,
        &current.cleanup_roots,
        manifest_digest(10),
        NONZERO_OBLIGATION_COUNT + 1,
    );
    let different_batch = CleanupBatchToken::sealed(
        ticket.identity().clone(),
        CleanupBatchId::new(current.cleanup_batch.raw() + 1),
        &current.cleanup_roots,
        manifest_digest(10),
        NONZERO_OBLIGATION_COUNT,
    );

    for different in [different_manifest, different_count, different_batch] {
        assert_ne!(exact, different);
        assert_ne!(exact.testing_binding(), different.testing_binding());
    }

    let sealed = engine
        .seal_commit(
            &ticket,
            CleanupManifestSeal::new(
                current.cleanup_roots.clone(),
                manifest_digest(10),
                NONZERO_OBLIGATION_COUNT,
            ),
        )
        .unwrap();
    assert_eq!(sealed.cleanup(), &exact);
}

#[test]
fn nonzero_completion_is_aggregate_attestation_without_core_receipts() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let request = engine.push(RouteLocation::new("/about")).unwrap();
    let cleanup = publish_current(&mut engine);
    assert_eq!(cleanup.obligation_count(), NONZERO_OBLIGATION_COUNT);

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    let settled = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .collect::<Vec<_>>();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].cleanup_disposition(), None);
}

#[test]
fn completion_rejects_wrong_token_count_zero_representation_and_binding() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let cleanup = publish_current(&mut engine);

    let mut foreign_engine = ready_engine();
    foreign_engine.push(RouteLocation::new("/items/1")).unwrap();
    let foreign = publish_current(&mut foreign_engine);

    let mut wrong_binding = valid_completion(&cleanup);
    wrong_binding.testing_set_binding(u64::MAX);
    let wrong_batch =
        cleanup.testing_with_batch(CleanupBatchId::new(cleanup.testing_batch().raw() + 1));
    for (invalid, expected) in [
        (
            CleanupCompletion::attest(foreign, CleanupCompletionDigest::canonical_empty(), 0),
            CleanupError::Stale,
        ),
        (
            CleanupCompletion::attest(cleanup.clone(), CleanupCompletionDigest::from_opaque(76), 1),
            CleanupError::Invalid,
        ),
        (
            CleanupCompletion::attest(cleanup.clone(), CleanupCompletionDigest::from_opaque(77), 0),
            CleanupError::Invalid,
        ),
        (
            CleanupCompletion::attest(wrong_batch, CleanupCompletionDigest::canonical_empty(), 0),
            CleanupError::Stale,
        ),
        (wrong_binding, CleanupError::Invalid),
    ] {
        let before = engine.snapshot();
        assert_eq!(engine.complete_cleanup(invalid), Err(expected));
        assert_eq!(engine.snapshot(), before);
    }

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn generation_overflow_settles_accepted_request_and_does_not_poison_prior_cleanup() {
    let mut engine = ready_engine();
    let first = engine.push(RouteLocation::new("/")).unwrap();
    let cleanup = publish_current(&mut engine);
    engine.testing_set_active_generation(RouteGeneration::testing_max());
    let overflow = engine
        .push(RouteLocation::new("/").with_fragment("details"))
        .unwrap();

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(
        engine.request(first).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert!(matches!(
        engine.request(overflow).unwrap().status(),
        RequestStatus::Failed(_)
    ));
    assert_eq!(engine.current_status(), None);
    assert!(engine.action().is_none());
}

#[test]
fn match_failure_stays_recovering_until_fallback_settles() {
    let mut engine = ready_engine();
    let failed = engine.push(RouteLocation::new("/missing")).unwrap();

    assert_eq!(
        engine.request(failed).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let fallback = engine.action().unwrap();
    let cleanup = publish_action(&mut engine, fallback);
    assert_eq!(
        engine.current_status(),
        Some(&RequestStatus::Running(NavigationPhase::PostCommitCleanup))
    );
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Match
    ));
    assert_eq!(engine.current_status(), None);

    let next = engine.push(RouteLocation::new("/items/1")).unwrap();
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
    );
}

#[test]
fn queued_match_failure_fallback_settles_before_next_reserved_request_starts() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let cleanup = publish_current(&mut engine);
    let failed = engine.push(RouteLocation::new("/missing")).unwrap();
    let next = engine.push(RouteLocation::new("/about")).unwrap();

    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(
        engine.request(failed).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("failed request must issue fallback before advancing the queue")
    };
    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "fallback build failed",
        ))
        .expect("fallback failure must retain cleanup authority before terminal");
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    let recovery_cleanup = prepare_failure_cleanup(&mut engine, recovery_failure, 0xB24, 0);
    engine
        .complete_cleanup(valid_completion(&recovery_cleanup))
        .unwrap();
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::ApplyingFrame(1))
    );
    assert!(matches!(engine.action(), Some(EngineAction::FrameTask(_))));
}

#[test]
fn nested_happy_path_schedules_root_to_leaf_branch() {
    let mut engine = ready_engine();
    let requested = RouteLocation::new("/items/1");
    let request = engine.push(requested.clone()).unwrap();

    let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
        panic!("nested navigation must start with its parent frame task")
    };
    assert_eq!(parent.view(), &RouteViewKey::new("root").unwrap());
    assert!(matches!(
        parent.mount_target(),
        super::FrameMountTarget::TopSlot { .. }
    ));
    let parent_version = parent.frame_version().clone();
    engine
        .accept_applied_report(
            parent.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
        )
        .unwrap();

    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("parent report must request its default child outlet")
    };
    assert_eq!(mount.request(), request);
    assert_eq!(mount.parent(), &parent_version);
    assert!(mount.outlet().is_default());
    let grant = engine.resolve_mount(mount).unwrap();
    let child_slot = grant.slot();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(11));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    assert_eq!(permit.slot(), child_slot);
    assert!(engine.action().is_none());
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(13));
    assert!(engine.action().is_none());
    engine.complete_mount_registration(completion).unwrap();

    let EngineAction::FrameTask(child) = engine.action().unwrap() else {
        panic!("exact final mount confirmation must issue the child task")
    };
    assert_eq!(child.view(), &RouteViewKey::new("items").unwrap());
    assert_eq!(child.frame_version().slot(), child_slot);
    assert!(matches!(
        child.mount_target(),
        super::FrameMountTarget::ChildOutlet { parent, outlet }
            if parent == &parent_version && outlet.is_default()
    ));
    let child_version = child.frame_version().clone();
    engine
        .accept_applied_report(child.into_applied_report(Vec::new()))
        .unwrap();
    let EngineAction::Commit(ticket) = engine.action().unwrap() else {
        panic!("leaf report must issue one commit")
    };
    let sealed = seal_current(&mut engine, ticket);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();

    assert_eq!(
        engine.active().frames(),
        &[parent_version.clone(), child_version.clone()]
    );
    assert_eq!(engine.history().len(), 1);
    assert_eq!(engine.history().current(), Some(&requested));
    let exact = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter_map(|event| match event.kind() {
            NavigationTraceEventKind::FrameTaskIssued
            | NavigationTraceEventKind::ShellPrepared
            | NavigationTraceEventKind::ViewApplied
            | NavigationTraceEventKind::OutletRegistered
            | NavigationTraceEventKind::MountRequested
            | NavigationTraceEventKind::MountResolved
            | NavigationTraceEventKind::MountGranted
            | NavigationTraceEventKind::FrameActivated
            | NavigationTraceEventKind::CommitPublished => Some(event.kind()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        exact,
        vec![
            NavigationTraceEventKind::FrameTaskIssued,
            NavigationTraceEventKind::ShellPrepared,
            NavigationTraceEventKind::ViewApplied,
            NavigationTraceEventKind::OutletRegistered,
            NavigationTraceEventKind::MountRequested,
            NavigationTraceEventKind::MountResolved,
            NavigationTraceEventKind::MountGranted,
            NavigationTraceEventKind::FrameTaskIssued,
            NavigationTraceEventKind::ShellPrepared,
            NavigationTraceEventKind::ViewApplied,
            NavigationTraceEventKind::FrameActivated,
            NavigationTraceEventKind::FrameActivated,
            NavigationTraceEventKind::CommitPublished,
        ]
    );
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn mount_confirmation_consumer_rejects_foreign_and_consumed_replays_without_write() {
    fn completed_confirmation(
        engine: &mut RouterEngine,
        digest: u64,
    ) -> crate::mount::MountConfirmation {
        let (_, _, mount) = begin_nested_mount(engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(digest));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(
            digest + 1,
        ));
        engine
            .complete_mount_registration_with_replay(completion)
            .unwrap()
    }

    let mut source = ready_engine();
    let replay = completed_confirmation(&mut source, 0x431);
    let completed = source.snapshot();
    assert_eq!(
        source.consume_mount_confirmation(replay),
        Err(crate::mount::MountError::Stale)
    );
    assert_eq!(source.snapshot(), completed);

    let mut foreign_source = ready_engine();
    let foreign = completed_confirmation(&mut foreign_source, 0x439);
    let mut receiver = ready_engine();
    let (_, _, mount) =
        begin_nested_mount(&mut receiver, vec![crate::slot::RouteSlotKey::default()]);
    let grant = receiver.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0x443));
    receiver.accept_mount_receipt(receipt).unwrap();
    let before = receiver.snapshot();
    assert_eq!(
        receiver.consume_mount_confirmation(foreign),
        Err(crate::mount::MountError::Foreign)
    );
    assert_eq!(receiver.snapshot(), before);
    assert!(receiver.action().is_none());
}

#[test]
fn nested_equivalent_nested_push_is_zero_entering() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();

    let request = engine.push(RouteLocation::new("items//1/")).unwrap();
    let action = engine.action().unwrap();
    assert!(matches!(action, EngineAction::Commit(_)));
    let cleanup = publish_action(&mut engine, action);
    assert_eq!(cleanup.obligation_count(), 0);
    assert_eq!(engine.active().frames(), active);
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
            .count(),
        0
    );
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn reconciliation_schedule_first_entering_leaf_targets_the_preserved_boundary_slot() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();
    let boundary_slot = active[2].slot();

    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("preserved boundary slot must issue the entering leaf directly")
    };

    assert_eq!(task.identity().branch_index(), 2);
    assert_eq!(task.frame_version().slot(), boundary_slot);
    assert_eq!(task.view(), &RouteViewKey::new("item").unwrap());
    assert_eq!(
        &engine.current.as_ref().unwrap().target_frames[..2],
        &active[..2]
    );
    assert!(engine.current.as_ref().unwrap().applied_frames.is_empty());
    assert_eq!(
        engine.current.as_ref().unwrap().cleanup_roots,
        vec![active[2].clone()]
    );
}

#[test]
fn reconciliation_schedule_append_without_retained_slot_uses_sealed_mount_protocol() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();

    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::MountRequest(request) = engine.action().unwrap() else {
        panic!("append without a registered boundary slot must request one mount")
    };
    assert_eq!(request.parent(), &active[1]);
    let grant = engine.resolve_mount(request).unwrap();
    let slot = grant.slot();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0x901));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0x903));
    engine.complete_mount_registration(completion).unwrap();

    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("sealed mount completion must issue the entering leaf")
    };
    assert_eq!(task.identity().branch_index(), 2);
    assert_eq!(task.frame_version().slot(), slot);
    assert_eq!(
        &engine.current.as_ref().unwrap().target_frames[..2],
        &active
    );
}

#[test]
fn reconciliation_schedule_preserved_mount_rejects_foreign_projection_context() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::MountRequest(request) = engine.action().unwrap() else {
        panic!("preserved parent must request the missing child slot")
    };

    let mut foreign = ready_engine_with_tree(reconciliation_tree());
    foreign.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut foreign);
    foreign.push(RouteLocation::new("/items/2")).unwrap();
    let foreign_projection = foreign
        .current
        .as_ref()
        .and_then(|current| current.reconciliation.clone())
        .unwrap();
    engine.current.as_mut().unwrap().reconciliation = Some(foreign_projection);
    let before = engine.snapshot();

    assert_eq!(
        engine.resolve_mount(request).map_err(|error| error.kind()),
        Err(MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn reconciliation_schedule_truncate_reaches_ready_to_switch_without_entering_task() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();
    let history_before = engine.history().clone();

    let request = engine.push(RouteLocation::new("/items")).unwrap();
    let EngineAction::Commit(ticket) = engine.action().unwrap() else {
        panic!("truncate must consume a sealed commit ticket without entering work")
    };
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::ReadyToSwitch)
    );
    assert_eq!(engine.history(), &history_before);
    let current = engine.current.as_ref().unwrap();
    assert_eq!(current.target_frames, active[..2]);
    assert!(current.applied_frames.is_empty());
    assert_eq!(current.cleanup_roots, vec![active[2].clone()]);

    let sealed = seal_current(&mut engine, ticket);
    assert_eq!(engine.history(), &history_before);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();
    assert_eq!(engine.history().len(), history_before.len() + 1);
    assert_eq!(engine.active().frames(), &active[..2]);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn reconciliation_schedule_nested_fragment_issues_one_new_generation_effect() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active = engine.active().frames().to_vec();

    let request = engine
        .push(RouteLocation::new("/items/1").with_fragment("details"))
        .unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("nested fragment must issue the leaf effect directly")
    };
    assert_eq!(task.identity().branch_index(), 2);
    assert_eq!(task.identity().frame(), active[2].frame());
    assert_eq!(task.identity().generation(), active[2].generation().next());
    assert_eq!(task.input().fragment(), Some("details"));
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::FrameTaskIssued)
            .count(),
        1
    );
}

#[test]
fn reconciliation_schedule_failure_cleanup_contains_only_applied_entering_suffix() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let preserved = engine.active().frames().to_vec();

    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("entering leaf task expected")
    };
    let entering = task.frame_version().clone();
    engine
        .accept_applied_report(task.into_applied_report(Vec::new()))
        .unwrap();
    let EngineAction::Commit(_) = engine.action().unwrap() else {
        panic!("applied entering leaf must reach commit")
    };
    let failure = engine.begin_failure_cleanup(
        super::PendingFailureKind::Apply,
        NavigationFailure::new(NavigationFailureKind::Apply, "pre-commit apply rejected"),
    );

    assert_eq!(failure.logical_roots(), &[entering]);
    assert!(
        failure
            .logical_roots()
            .iter()
            .all(|root| !preserved.contains(root))
    );
}

#[test]
fn reconciliation_schedule_cleanup_settles_before_router_fallback() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let request = engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("entering leaf task expected")
    };
    let failure = engine
        .begin_view_report(task.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "entering effect rejected",
        ))
        .unwrap();
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(Vec::new(), manifest_digest(0x905), 0),
        )
        .unwrap();
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("reconciliation failure cleanup must issue RouterFallback")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let trace = engine.request(request).unwrap().trace();
    let settled = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .unwrap();
    let fallback_issued = trace
        .iter()
        .rposition(|event| {
            event.kind() == NavigationTraceEventKind::FrameTaskIssued
                && event
                    .frame()
                    .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
        })
        .unwrap();
    assert!(settled < fallback_issued);
    assert!(
        trace
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
}

#[test]
fn reconciliation_schedule_fragment_residual_cleans_only_new_generation_before_fallback() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active_before = engine.active().clone();
    let history_before = engine.history().clone();

    let request = engine
        .push(RouteLocation::new("/items/1").with_fragment("details"))
        .unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("fragment leaf effect expected")
    };
    let new_generation = task.frame_version().clone();
    let failure = engine
        .begin_view_report(task.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "fragment effect left a physical residual",
        ));

    assert_eq!(failure.logical_roots(), &[new_generation.clone()]);
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.history(), &history_before);
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(
                vec![new_generation],
                manifest_digest(0x907),
                NONZERO_OBLIGATION_COUNT,
            ),
        )
        .unwrap();
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.history(), &history_before);
    let _fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.history(), &history_before);
}

#[test]
fn reconciliation_schedule_equivalent_consumes_commit_and_publishes_push_once() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let history_before = engine.history().clone();

    let request = engine.push(RouteLocation::new("items//1/")).unwrap();
    let EngineAction::Commit(ticket) = engine.action().unwrap() else {
        panic!("equivalent push must still consume one sealed commit ticket")
    };
    assert_eq!(engine.history(), &history_before);
    let sealed = seal_current(&mut engine, ticket);
    assert_eq!(engine.history(), &history_before);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();
    assert_eq!(engine.history().len(), history_before.len() + 1);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert_eq!(engine.history().len(), history_before.len() + 1);
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
            .count(),
        1
    );
}

#[test]
fn reconciliation_schedule_truncate_retains_slot_for_direct_append_reuse() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let retained_slot = engine.active().frames()[2].slot();

    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    assert!(engine.active_child_slots.iter().any(|binding| {
        binding.parent_branch_index() == 1
            && binding.slot() == retained_slot
            && binding.child().is_none()
    }));
    let next_slot_before = engine.ids.snapshot().next_slot();

    engine.push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("retained empty slot must issue the appended leaf directly")
    };
    assert_eq!(task.identity().branch_index(), 2);
    assert_eq!(task.frame_version().slot(), retained_slot);
    assert_eq!(engine.ids.snapshot().next_slot(), next_slot_before);
}

#[test]
fn reconciliation_schedule_prefix_zero_failure_starts_fallback_with_existing_active() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine
        .push(RouteLocation::new("/items/1").with_search("?tab=one"))
        .unwrap();
    settle_current(&mut engine);
    let active_before = engine.active().clone();
    let history_before = engine.history().clone();

    engine
        .push(RouteLocation::new("/items/2").with_search("?tab=two"))
        .unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("prefix-zero replacement root effect expected")
    };
    assert_eq!(task.identity().branch_index(), 0);
    let failure_cleanup = engine
        .begin_view_report(task.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "replacement root effect rejected",
        ))
        .expect("prefix-zero root failure must enter cleanup");

    assert!(failure_cleanup.logical_roots().is_empty());
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.history(), &history_before);
    assert!(engine.action().is_none());
    let cleanup = prepare_failure_cleanup(&mut engine, failure_cleanup, 0xb12, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("prefix-zero root failure must retain mandatory fallback")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
}

#[test]
fn reconciliation_schedule_prefix_zero_residual_cleans_new_root_before_fallback() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine
        .push(RouteLocation::new("/items/1").with_search("?tab=one"))
        .unwrap();
    settle_current(&mut engine);
    let old = engine.active().frames().to_vec();
    let active_before = engine.active().clone();
    let history_before = engine.history().clone();

    engine
        .push(RouteLocation::new("/items/2").with_search("?tab=two"))
        .unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("prefix-zero replacement root effect expected")
    };
    let new_root = task.frame_version().clone();
    let failure = engine
        .begin_view_report(task.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "replacement root left a physical residual",
        ));

    assert_eq!(failure.logical_roots(), &[new_root.clone()]);
    assert!(
        failure
            .logical_roots()
            .iter()
            .all(|root| !old.contains(root))
    );
    assert!(matches!(
        &failure.continuation,
        super::FailureCleanupContinuationOwner::BeginFallback
    ));
    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.history(), &history_before);

    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(
                vec![new_root],
                manifest_digest(0x915),
                NONZERO_OBLIGATION_COUNT,
            ),
        )
        .unwrap();
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.history(), &history_before);
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("prefix-zero residual cleanup must continue into fallback")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
}

#[test]
fn nested_branch_changing_nested_push_replaces_only_the_suffix() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let old = engine.active().frames().to_vec();
    engine.push(RouteLocation::new("/about")).unwrap();
    let EngineAction::FrameTask(leaf) = engine.action().unwrap() else {
        panic!("changed nested branch starts with its entering suffix")
    };
    assert_eq!(leaf.view(), &RouteViewKey::new("about").unwrap());
    assert_eq!(leaf.identity().branch_index(), 1);
    assert_ne!(leaf.frame_version().frame(), old[1].frame());
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(leaf));
    let current = engine.current.as_ref().unwrap();
    assert_eq!(current.cleanup_roots, vec![old[1].clone()]);
    assert_eq!(current.target_frames[0], old[0]);
    assert_ne!(current.target_frames[1].frame(), old[1].frame());
    let sealed = seal_current(&mut engine, ticket);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();
    assert_eq!(engine.active().frames().len(), 2);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn nested_fragment_push_reconciles_only_the_leaf_generation() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let old = engine.active().frames().to_vec();
    engine
        .push(RouteLocation::new("/items/1").with_fragment("details"))
        .unwrap();
    let EngineAction::FrameTask(leaf) = engine.action().unwrap() else {
        panic!("nested fragment starts with the leaf effect")
    };
    assert_eq!(leaf.view(), &RouteViewKey::new("items").unwrap());
    assert_eq!(leaf.identity().branch_index(), 1);
    assert_eq!(leaf.frame_version().frame(), old[1].frame());
    assert_eq!(
        leaf.frame_version().generation(),
        old[1].generation().next()
    );
    let ticket = drive_to_commit(&mut engine, EngineAction::FrameTask(leaf));
    let current = engine.current.as_ref().unwrap();
    assert_eq!(current.cleanup_roots, vec![old[1].clone()]);
    assert_eq!(current.target_frames[0], old[0]);
    assert_eq!(current.target_frames[1].frame(), old[1].frame());
    let sealed = seal_current(&mut engine, ticket);
    let cleanup = sealed.cleanup().clone();
    publish_sealed(&mut engine, sealed, || {}).unwrap();
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn nested_depth_one_fragment_push_preserves_frame_and_advances_generation() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    let first = engine.active().frames()[0].clone();

    engine
        .push(RouteLocation::new("/").with_fragment("details"))
        .unwrap();
    let EngineAction::FrameTask(fragment) = engine.action().unwrap() else {
        panic!("depth-one fragment push must issue a frame task")
    };

    assert_eq!(fragment.identity().frame(), first.frame());
    assert_eq!(fragment.identity().generation(), first.generation().next());
}

#[test]
fn nested_queued_push_matches_once_and_reuses_the_stored_branch() {
    let mut engine = ready_engine();
    let request = engine.enqueue_push(RouteLocation::new("/items/1")).unwrap();
    engine.matcher = crate::matcher::RouteMatcher::new(
        RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap(),
    );

    engine.collect();

    let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
        panic!("stored match must still issue the original branch")
    };
    assert_eq!(parent.identity().request(), request);
    assert_eq!(parent.view(), &RouteViewKey::new("root").unwrap());
}

#[test]
fn nested_match_failure_queues_without_branch_reservation_then_reserves_fallback_attempt() {
    let mut engine = ready_engine();
    let before = engine.ids.snapshot();
    let request = engine.enqueue_push(RouteLocation::new("/missing")).unwrap();
    let queued = engine.ids.snapshot();

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Queued
    );
    assert!(engine.action().is_none());
    assert_eq!(
        queued.next_request(engine.router),
        Some(request.sequence() + 1)
    );
    assert_eq!(queued.next_frame(), before.next_frame());
    assert_eq!(queued.next_frame_task(), before.next_frame_task());
    assert_eq!(queued.next_transaction(), before.next_transaction());
    assert_eq!(queued.next_cleanup_batch(), before.next_cleanup_batch());
    engine.collect();
    let fallback_reserved = engine.ids.snapshot();
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("match failure must reserve and issue one fallback attempt")
    };
    assert_eq!(fallback.identity().request(), request);
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    assert_eq!(fallback_reserved.next_frame(), queued.next_frame() + 1);
    assert_eq!(
        fallback_reserved.next_frame_task(),
        queued.next_frame_task() + 1
    );
    assert_eq!(
        fallback_reserved.next_transaction(),
        queued.next_transaction() + 1
    );
    assert_eq!(
        fallback_reserved.next_cleanup_batch(),
        queued.next_cleanup_batch() + 1
    );
}

#[test]
fn nested_applied_reports_reject_unsorted_foreign_and_duplicate_capabilities() {
    let mut unsorted_engine = ready_engine();
    unsorted_engine
        .push(RouteLocation::new("/items/1"))
        .unwrap();
    let EngineAction::FrameTask(task) = unsorted_engine.action().unwrap() else {
        panic!("parent task expected")
    };
    let exact_identity = task.identity().clone();
    let lifecycle = (
        unsorted_engine.state.snapshot(),
        unsorted_engine.ids.snapshot(),
        unsorted_engine.testing_history_authority_snapshot(),
        unsorted_engine.records.clone(),
        format!("{:?}", unsorted_engine.current),
    );
    let report = task.into_applied_report(vec![
        crate::slot::RouteSlotKey::named("z").unwrap(),
        crate::slot::RouteSlotKey::default(),
    ]);
    assert_eq!(
        unsorted_engine.accept_applied_report(report),
        Err(ReportError::Invalid)
    );
    assert_eq!(
        (
            unsorted_engine.state.snapshot(),
            unsorted_engine.ids.snapshot(),
            unsorted_engine.testing_history_authority_snapshot(),
            unsorted_engine.records.clone(),
            format!("{:?}", unsorted_engine.current),
        ),
        lifecycle
    );
    let EngineAction::FrameTask(reissued) = unsorted_engine.action().unwrap() else {
        panic!("invalid consumed report must restore its exact task")
    };
    assert_eq!(reissued.identity(), &exact_identity);

    let mut foreign_engine = ready_engine();
    foreign_engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = foreign_engine.action().unwrap() else {
        panic!("parent task expected")
    };
    let exact_identity = task.identity().clone();
    let mut foreign = task.into_applied_report(vec![crate::slot::RouteSlotKey::default()]);
    foreign
        .identity
        .testing_set_router(distinct_identity().router_identity());
    let before = (
        foreign_engine.state.snapshot(),
        foreign_engine.ids.snapshot(),
        foreign_engine.testing_history_authority_snapshot(),
        format!("{:?}", foreign_engine.current),
    );
    assert_eq!(
        foreign_engine.accept_applied_report(foreign),
        Err(ReportError::Invalid)
    );
    assert_eq!(
        (
            foreign_engine.state.snapshot(),
            foreign_engine.ids.snapshot(),
            foreign_engine.testing_history_authority_snapshot(),
            format!("{:?}", foreign_engine.current),
        ),
        before
    );
    let EngineAction::FrameTask(reissued) = foreign_engine.action().unwrap() else {
        panic!("foreign consumed report must restore the current exact task")
    };
    assert_eq!(reissued.identity(), &exact_identity);

    let mut duplicate_engine = ready_engine();
    duplicate_engine
        .push(RouteLocation::new("/items/1"))
        .unwrap();
    let EngineAction::FrameTask(task) = duplicate_engine.action().unwrap() else {
        panic!("parent task expected")
    };
    let report = task.into_applied_report(vec![crate::slot::RouteSlotKey::default()]);
    let duplicate = report.testing_duplicate();
    duplicate_engine.accept_applied_report(report).unwrap();
    let before = (
        duplicate_engine.state.snapshot(),
        duplicate_engine.ids.snapshot(),
        duplicate_engine.testing_history_authority_snapshot(),
        format!("{:?}", duplicate_engine.current),
        duplicate_engine.action.as_ref().map(ActionSnapshot::from),
    );
    assert_eq!(
        duplicate_engine.accept_applied_report(duplicate),
        Err(ReportError::Duplicate)
    );
    assert_eq!(
        (
            duplicate_engine.state.snapshot(),
            duplicate_engine.ids.snapshot(),
            duplicate_engine.testing_history_authority_snapshot(),
            format!("{:?}", duplicate_engine.current),
            duplicate_engine.action.as_ref().map(ActionSnapshot::from),
        ),
        before
    );
}

#[test]
fn nested_branch_reservation_exhaustion_is_complete_zero_change() {
    let mut engine = ready_engine();
    engine.testing_exhaust_reservation_counter(ReservationCounter::FrameTask);
    let before = engine.snapshot();

    assert_eq!(
        engine.push(RouteLocation::new("/items/1")),
        Err(super::PushError::IdExhausted)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn nested_mount_slot_exhaustion_settles_and_starts_queued_root_navigation() {
    let mut engine = ready_engine();
    let before = engine.ids.snapshot();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let next = engine.enqueue_push(RouteLocation::new("/")).unwrap();
    let reserved = engine.ids.snapshot();
    assert_eq!(reserved.next_frame(), before.next_frame() + 3);
    assert_eq!(reserved.next_frame_task(), before.next_frame_task() + 3);
    let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
        panic!("parent task expected")
    };
    engine
        .accept_applied_report(
            parent.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
        )
        .unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("mount request expected")
    };
    engine.ids.testing_exhaust_slot_counter();
    let exhausted = engine.ids.snapshot();

    let error = engine.resolve_mount(mount).unwrap_err();
    assert_eq!(error.kind(), crate::mount::MountError::Exhausted);
    assert!(matches!(
        error.into_owner(),
        super::ResolveMountOwner::Consumed
    ));
    assert_eq!(engine.ids.snapshot(), exhausted);
    assert!(!engine.mount.projection().has_outstanding());
    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!("permanent slot exhaustion must settle the attempt")
    };
    assert_eq!(failure.request(), request);
    assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
    assert_eq!(failure.logical_roots().len(), 1);
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0x51e, 1);
    let fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
    let cleanup = publish_action(&mut engine, EngineAction::FrameTask(fallback));
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(
        matches!(engine.request(request).unwrap().status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::Mount)
    );
    assert!(
        engine.history().is_empty(),
        "the failed navigation must not enter history"
    );
    let cleanup = publish_current(&mut engine);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(engine.history().current(), Some(&RouteLocation::new("/")));
    assert!(engine.action().is_none());
}

#[test]
fn frame_task_issued_is_the_first_trace_event_for_normal_push() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();

    assert_eq!(
        engine.request(request).unwrap().trace()[0].kind(),
        NavigationTraceEventKind::FrameTaskIssued
    );
}

fn begin_nested_mount(
    engine: &mut RouterEngine,
    outlets: Vec<crate::slot::RouteSlotKey>,
) -> (
    crate::ids::NavigationRequestId,
    RouteFrameVersion,
    super::FrameMountRequest,
) {
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
        panic!("nested request starts with a parent task")
    };
    let parent_frame = parent.frame_version().clone();
    engine
        .accept_applied_report(parent.into_applied_report(outlets))
        .unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("accepted parent report yields the required mount request")
    };
    (request, parent_frame, mount)
}

#[test]
fn exact_mount_request_stage_drift_does_not_grant_or_allocate() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
    let before = engine.snapshot();

    assert_eq!(
        engine.resolve_mount(mount).map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn exact_mount_request_stage_drift_does_not_reject_or_clear_issuer() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
    engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
    let before = engine.snapshot();

    assert_eq!(
        engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn exact_mount_request_pending_identity_drift_is_zero_write() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
    engine
        .current
        .as_mut()
        .unwrap()
        .pending_mount
        .as_mut()
        .unwrap()
        .outlet = crate::slot::RouteSlotKey::named("drift").unwrap();
    let before = engine.snapshot();

    assert_eq!(
        engine.resolve_mount(mount).map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn exact_mount_grant_stage_drift_does_not_reject_or_clear_issuer() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    engine.current.as_mut().unwrap().stage = WorkStage::ConfirmingMount;
    let before = engine.snapshot();

    assert_eq!(
        engine
            .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant))
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn exact_mount_permit_stage_drift_does_not_reject_or_clear_issuer() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(231));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
    let before = engine.snapshot();

    assert_eq!(
        engine
            .reject_mount_after_mutation(super::MountMutationArtifact::RegistrationPermit(permit),)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn exact_mount_receipt_stage_drift_is_zero_write() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(233));
    engine.current.as_mut().unwrap().stage = WorkStage::ConfirmingMount;
    let before = engine.snapshot();

    assert_eq!(
        engine
            .accept_mount_receipt(receipt)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn exact_mount_completion_stage_drift_is_zero_write() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(239));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(241));
    engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
    let before = engine.snapshot();

    assert_eq!(
        engine.complete_mount_registration(completion),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn mount_registration_begin_context_drift_is_zero_write() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let (receipt, proof) =
        grant.attest_receipt_with_proof(crate::mount::MountReceiptDigest::from_opaque(257));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    engine.current.as_mut().unwrap().stage = WorkStage::AwaitingReceipt;
    let before = engine.snapshot();

    assert_eq!(
        engine.validate_mount_registration(&permit, &proof),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn current_parent_slot_drift_blocks_mount_resolution_without_writes() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let drifted_slot = engine.ids.allocate_slot().unwrap();
    engine
        .current
        .as_mut()
        .unwrap()
        .identity
        .testing_set_top_slot(drifted_slot);
    let before = engine.snapshot();

    assert_eq!(
        engine.resolve_mount(mount).map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn current_parent_origin_drift_blocks_mount_rejection_without_writes() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
    engine
        .current
        .as_mut()
        .unwrap()
        .identity
        .testing_set_origin(FrameOrigin::RouterFallback);
    let before = engine.snapshot();

    assert_eq!(
        engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn current_frame_task_drift_blocks_mount_rejection_without_writes() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, Vec::new());
    let drifted_frame_task = engine.ids.allocate_frame_task().unwrap();
    engine
        .current
        .as_mut()
        .unwrap()
        .identity
        .testing_set_frame_task(drifted_frame_task);
    let before = engine.snapshot();

    assert_eq!(
        engine
            .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn current_parent_frame_drift_blocks_mount_receipt_without_writes() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(243));
    let drifted_frame = engine.ids.allocate_frame().unwrap();
    engine
        .current
        .as_mut()
        .unwrap()
        .identity
        .testing_set_frame(drifted_frame);
    let before = engine.snapshot();

    assert_eq!(
        engine
            .accept_mount_receipt(receipt)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn current_parent_generation_drift_blocks_mount_completion_without_writes() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(247));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(249));
    let generation = engine.current.as_ref().unwrap().identity.generation;
    engine
        .current
        .as_mut()
        .unwrap()
        .identity
        .testing_set_generation(generation.checked_next().unwrap());
    let before = engine.snapshot();

    assert_eq!(
        engine.complete_mount_registration(completion),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn pending_child_index_drift_blocks_grant_rejection_without_writes() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    engine
        .current
        .as_mut()
        .unwrap()
        .pending_mount
        .as_mut()
        .unwrap()
        .child_index = 0;
    let before = engine.snapshot();

    assert_eq!(
        engine
            .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant))
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn pending_child_index_drift_cannot_issue_the_wrong_child() {
    let mut engine = ready_engine();
    let (_, _, mount) = begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(253));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(255));
    engine
        .current
        .as_mut()
        .unwrap()
        .pending_mount
        .as_mut()
        .unwrap()
        .child_index = 0;
    let before = engine.snapshot();

    assert_eq!(
        engine.complete_mount_registration(completion),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);
}

fn prepare_failure_cleanup(
    engine: &mut RouterEngine,
    request: super::FailureCleanupRequest,
    digest: u64,
    count: usize,
) -> CleanupBatchToken {
    let roots = request.logical_roots().to_vec();
    engine
        .prepare_failure_cleanup(
            request,
            CleanupManifestSeal::new(roots, manifest_digest(digest), count),
        )
        .unwrap()
}

fn assert_failure_waits_then_settles(
    engine: &mut RouterEngine,
    request: crate::ids::NavigationRequestId,
    cleanup: &CleanupBatchToken,
    expected: NavigationFailureKind,
) {
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );

    engine.complete_cleanup(valid_completion(cleanup)).unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure) if failure.kind() == expected
    ));
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .map(NavigationTraceEvent::kind)
            .rev()
            .take(2)
            .collect::<Vec<_>>(),
        vec![
            NavigationTraceEventKind::RequestTerminal,
            NavigationTraceEventKind::CleanupSettled,
        ]
    );
}

fn complete_failure_cleanup_into_router_fallback(
    engine: &mut RouterEngine,
    request: crate::ids::NavigationRequestId,
    cleanup: &CleanupBatchToken,
) -> super::FrameTask {
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );

    engine.complete_cleanup(valid_completion(cleanup)).unwrap();

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("cleanup completion must issue RouterFallback")
    };
    assert_eq!(fallback.identity().request(), request);
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let trace = engine.request(request).unwrap().trace();
    let settled = trace
        .iter()
        .rposition(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .unwrap();
    let fallback_issued = trace
        .iter()
        .rposition(|event| {
            event.kind() == NavigationTraceEventKind::FrameTaskIssued
                && event
                    .frame()
                    .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
        })
        .unwrap();
    assert!(settled < fallback_issued);
    fallback
}

struct ZeroFailureCleanupPhysical<'a> {
    owner_id: u64,
    receiver: super::FailureCleanupReceiverProjection,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
    drops: &'a std::cell::Cell<usize>,
    panic_on_drop: &'a std::cell::Cell<bool>,
}

impl ZeroFailureCleanupPhysical<'_> {
    fn observed_projection(&self) -> super::FailureCleanupZeroPhysicalProjection {
        super::FailureCleanupZeroPhysicalProjection {
            receiver: self.receiver.clone(),
            manifest_digest: self.manifest_digest,
            obligation_count: self.obligation_count,
        }
    }
}

impl Drop for ZeroFailureCleanupPhysical<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(
            !self.panic_on_drop.get(),
            "zero failure-cleanup physical consumption panicked"
        );
    }
}

fn permanent_zero_recovery_failure_fixture() -> (
    RouterEngine,
    crate::ids::NavigationRequestId,
    crate::ids::NavigationRequestId,
    super::FailureCleanupRequest,
    super::FailureCleanupReceiverProjection,
    RouteFrameVersion,
) {
    let mut engine = ready_engine();
    let failed = engine.push(RouteLocation::new("/about")).unwrap();
    let next = engine.enqueue_push(RouteLocation::new("/items/1")).unwrap();
    let Some(EngineAction::FrameTask(normal)) = engine.action() else {
        panic!("permanent-zero fixture requires one normal frame task")
    };
    let normal_failure = engine
        .begin_view_report(normal.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "permanent-zero fixture normal apply failed",
        ))
        .expect("normal failure must clean up before RouterFallback");
    let normal_cleanup = prepare_failure_cleanup(&mut engine, normal_failure, 0xA001, 0);
    engine
        .complete_cleanup(valid_completion(&normal_cleanup))
        .unwrap();
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("normal cleanup must issue the RouterFallback recovery attempt")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);

    let submitted_logical_root = fallback.frame_version().clone();
    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "permanent-zero fixture RouterFallback apply failed",
        ));
    assert!(recovery_failure.logical_roots().is_empty());
    assert_eq!(
        recovery_failure.failure().kind(),
        NavigationFailureKind::RecoveryFailed
    );
    assert!(matches!(
        &recovery_failure.continuation,
        super::FailureCleanupContinuationOwner::Terminal
    ));
    let receiver = recovery_failure.receiver.clone();
    assert_eq!(
        engine.request(failed).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    assert!(engine.action().is_none());

    (
        engine,
        failed,
        next,
        recovery_failure,
        receiver,
        submitted_logical_root,
    )
}

#[test]
fn failure_cleanup_zero_terminal_consumes_typed_physical_before_publication() {
    let physical_drops = std::cell::Cell::new(0);
    let panic_on_drop = std::cell::Cell::new(false);
    let panic_callback_count = std::cell::Cell::new(0);
    let (mut engine, failed, next, recovery_failure, receiver, submitted_logical_root) =
        permanent_zero_recovery_failure_fixture();
    let submitted_digest = manifest_digest(0xA002);
    let permanent = engine.prepare_failure_cleanup_retaining(
        recovery_failure,
        CleanupManifestSeal::new(vec![submitted_logical_root.clone()], submitted_digest, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent { request, evidence } = permanent else {
        panic!("a real logical-root contradiction must be permanently classified")
    };
    assert_eq!(request.request(), failed);
    assert_eq!(
        evidence,
        super::FailureCleanupManifestContradictionProjection {
            source: receiver.clone(),
            expected_logical_roots: Vec::new(),
            observed_logical_roots: vec![submitted_logical_root],
            submitted_manifest_digest: submitted_digest,
            submitted_obligation_count: 0,
        }
    );
    let expected_zero = super::FailureCleanupZeroPhysicalProjection {
        receiver: receiver.clone(),
        manifest_digest: submitted_digest,
        obligation_count: 0,
    };
    let holder = super::FailureCleanupPermanentHolder {
        request,
        physical: ZeroFailureCleanupPhysical {
            owner_id: 0xA003,
            receiver: receiver.clone(),
            manifest_digest: submitted_digest,
            obligation_count: 0,
            drops: &physical_drops,
            panic_on_drop: &panic_on_drop,
        },
        evidence,
    };
    let classified = engine.classify_failure_cleanup_permanent(holder, |physical, expected| {
        assert_eq!(&expected.physical, &expected_zero);
        let observed = physical.observed_projection();
        super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, expected, observed)
    });
    let super::FailureCleanupPermanentCoreOutcome::ProvenZero { zero, owner } = classified else {
        panic!("exact independently observed zero physical facts must bind once")
    };
    assert_eq!(zero.proof, expected_zero);
    assert_eq!(zero.physical.owner_id, 0xA003);
    assert_eq!(owner.receiver, receiver);
    assert_eq!(physical_drops.get(), 0);

    panic_on_drop.set(true);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = engine.complete_failure_cleanup_zero_terminal(owner, zero, |zero| {
            panic_callback_count.set(panic_callback_count.get() + 1);
            drop(zero);
        });
    }));
    assert!(panic.is_err());
    assert_eq!(panic_callback_count.get(), 1);
    assert_eq!(physical_drops.get(), 1);
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert!(
        engine
            .request(failed)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    assert_eq!(
        engine
            .current
            .as_ref()
            .map(|current| current.identity.request),
        Some(failed)
    );
    assert!(engine.action().is_none());

    let successful_drops = std::cell::Cell::new(0);
    let successful_panic_on_drop = std::cell::Cell::new(false);
    let callback_count = std::cell::Cell::new(0);
    let (mut engine, failed, next, recovery_failure, receiver, submitted_logical_root) =
        permanent_zero_recovery_failure_fixture();
    let submitted_digest = manifest_digest(0xA004);
    let permanent = engine.prepare_failure_cleanup_retaining(
        recovery_failure,
        CleanupManifestSeal::new(vec![submitted_logical_root], submitted_digest, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent { request, evidence } = permanent else {
        panic!("the fresh fixture must preserve the permanent contradiction")
    };
    let expected_zero = super::FailureCleanupZeroPhysicalProjection {
        receiver: receiver.clone(),
        manifest_digest: submitted_digest,
        obligation_count: 0,
    };
    let holder = super::FailureCleanupPermanentHolder {
        request,
        physical: ZeroFailureCleanupPhysical {
            owner_id: 0xA005,
            receiver,
            manifest_digest: submitted_digest,
            obligation_count: 0,
            drops: &successful_drops,
            panic_on_drop: &successful_panic_on_drop,
        },
        evidence,
    };
    let classified = engine.classify_failure_cleanup_permanent(holder, |physical, expected| {
        assert_eq!(&expected.physical, &expected_zero);
        let observed = physical.observed_projection();
        super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, expected, observed)
    });
    let super::FailureCleanupPermanentCoreOutcome::ProvenZero { zero, owner } = classified else {
        panic!("the fresh exact fixture must bind one zero owner")
    };
    let completed = engine.complete_failure_cleanup_zero_terminal(owner, zero, |zero| {
        drop(zero);
        assert_eq!(successful_drops.get(), 1);
        callback_count.set(callback_count.get() + 1);
    });
    let super::FailureCleanupZeroTerminalOutcome::Terminal(terminal) = completed else {
        panic!("the exact zero owner must publish one prepared terminal")
    };
    assert_eq!(callback_count.get(), 1);
    assert_eq!(successful_drops.get(), 1);
    assert_eq!(terminal.identity.request(), failed);
    assert!(matches!(
        terminal.status,
        RequestStatus::Failed(ref failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(
        terminal.failure.kind(),
        NavigationFailureKind::RecoveryFailed
    );
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(
        engine
            .request(failed)
            .unwrap()
            .trace()
            .last()
            .map(NavigationTraceEvent::kind),
        Some(NavigationTraceEventKind::RequestTerminal)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    assert!(engine.current.is_none());
    assert!(engine.action().is_none());

    engine.collect();
    assert!(matches!(
        engine.request(next).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert_eq!(
        engine
            .current
            .as_ref()
            .map(|current| current.identity.request),
        Some(next)
    );
    let Some(EngineAction::FrameTask(next_action)) = engine.action() else {
        panic!("an explicit collect starts the queued request after terminal publication")
    };
    assert_eq!(next_action.identity().request(), next);
}

#[test]
fn failure_cleanup_zero_terminal_refreezes_records_and_queue_before_consumption() {
    let physical_drops = std::cell::Cell::new(0);
    let panic_on_drop = std::cell::Cell::new(false);
    let callback_count = std::cell::Cell::new(0);
    let (mut engine, failed, next, recovery_failure, receiver, submitted_logical_root) =
        permanent_zero_recovery_failure_fixture();
    let submitted_digest = manifest_digest(0xA009);
    let permanent = engine.prepare_failure_cleanup_retaining(
        recovery_failure,
        CleanupManifestSeal::new(vec![submitted_logical_root], submitted_digest, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent { request, evidence } = permanent else {
        panic!("the delayed-source fixture must preserve the permanent contradiction")
    };
    let holder = super::FailureCleanupPermanentHolder {
        request,
        physical: ZeroFailureCleanupPhysical {
            owner_id: 0xA00A,
            receiver: receiver.clone(),
            manifest_digest: submitted_digest,
            obligation_count: 0,
            drops: &physical_drops,
            panic_on_drop: &panic_on_drop,
        },
        evidence,
    };
    let classified = engine.classify_failure_cleanup_permanent(holder, |physical, expected| {
        let observed = physical.observed_projection();
        super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, expected, observed)
    });
    let super::FailureCleanupPermanentCoreOutcome::ProvenZero { zero, owner } = classified else {
        panic!("the exact physical facts must produce a delayed zero-terminal owner")
    };

    let drift = engine
        .enqueue_push(RouteLocation::new("/items/2"))
        .expect("a queued push is a real delayed-source mutation");
    let after_drift = engine.snapshot();
    let completed = engine.complete_failure_cleanup_zero_terminal(owner, zero, |zero| {
        callback_count.set(callback_count.get() + 1);
        drop(zero);
    });
    let super::FailureCleanupZeroTerminalOutcome::Mismatch {
        zero,
        owner,
        evidence,
    } = completed
    else {
        panic!("record and queue drift must reject delayed terminal publication")
    };

    assert_eq!(callback_count.get(), 0);
    assert_eq!(physical_drops.get(), 0);
    assert_eq!(owner.receiver, receiver);
    assert_eq!(zero.physical.owner_id, 0xA00A);
    assert_eq!(evidence.expected.queue, vec![next]);
    assert_eq!(evidence.observed.queue, vec![next, drift]);
    assert_eq!(
        evidence.observed.records.len(),
        evidence.expected.records.len() + 1
    );
    assert_eq!(
        evidence.observed.records.back().map(|record| record.id()),
        Some(drift)
    );
    assert_eq!(engine.snapshot(), after_drift);
    assert_eq!(
        engine.request(failed).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    assert_eq!(
        engine.request(drift).unwrap().status(),
        &RequestStatus::Queued
    );
    assert!(engine.action().is_none());

    drop(owner);
    drop(zero);
    assert_eq!(physical_drops.get(), 1);
}

#[test]
fn failure_cleanup_zero_terminal_refreezes_non_stage_current_facts() {
    let physical_drops = std::cell::Cell::new(0);
    let panic_on_drop = std::cell::Cell::new(false);
    let callback_count = std::cell::Cell::new(0);
    let (mut engine, failed, _next, recovery_failure, receiver, submitted_logical_root) =
        permanent_zero_recovery_failure_fixture();
    let drift_root = submitted_logical_root.clone();
    let submitted_digest = manifest_digest(0xA00F);
    let permanent = engine.prepare_failure_cleanup_retaining(
        recovery_failure,
        CleanupManifestSeal::new(vec![submitted_logical_root], submitted_digest, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent { request, evidence } = permanent else {
        panic!("the current-source fixture must preserve the permanent contradiction")
    };
    let classified = engine.classify_failure_cleanup_permanent(
        super::FailureCleanupPermanentHolder {
            request,
            physical: ZeroFailureCleanupPhysical {
                owner_id: 0xA010,
                receiver: receiver.clone(),
                manifest_digest: submitted_digest,
                obligation_count: 0,
                drops: &physical_drops,
                panic_on_drop: &panic_on_drop,
            },
            evidence,
        },
        |physical, binding| {
            let observed = physical.observed_projection();
            super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, binding, observed)
        },
    );
    let super::FailureCleanupPermanentCoreOutcome::ProvenZero { zero, owner } = classified else {
        panic!("exact physical facts must produce one delayed terminal pair")
    };

    engine
        .current
        .as_mut()
        .expect("classification retains the dedicated current stage")
        .cleanup_roots
        .push(drift_root.clone());
    let after_drift = engine.snapshot();
    let completed = engine.complete_failure_cleanup_zero_terminal(owner, zero, |zero| {
        callback_count.set(callback_count.get() + 1);
        drop(zero);
    });
    let super::FailureCleanupZeroTerminalOutcome::Mismatch {
        zero,
        owner,
        evidence,
    } = completed
    else {
        panic!("same-stage cleanup-root drift must reject terminal publication")
    };

    let expected_current = evidence
        .expected
        .current
        .as_ref()
        .expect("the terminal owner freezes one expected current projection");
    let observed_current = evidence
        .observed
        .current
        .as_ref()
        .expect("the mismatch observes one current projection");
    assert!(expected_current.cleanup_roots.is_empty());
    assert_eq!(observed_current.cleanup_roots, vec![drift_root]);
    assert_eq!(expected_current.identity, observed_current.identity);
    assert_eq!(expected_current.stage, observed_current.stage);
    assert_eq!(callback_count.get(), 0);
    assert_eq!(physical_drops.get(), 0);
    assert_eq!(owner.receiver, receiver);
    assert_eq!(zero.physical.owner_id, 0xA010);
    assert_eq!(engine.snapshot(), after_drift);
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Running(NavigationPhase::CleanupPending)
    ));
    assert!(
        engine
            .request(failed)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );

    drop(owner);
    drop(zero);
    assert_eq!(physical_drops.get(), 1);
}

#[test]
fn failure_cleanup_zero_terminal_rejects_crossed_exact_pairs() {
    let physical_drops = std::cell::Cell::new(0);
    let panic_on_drop = std::cell::Cell::new(false);
    let callback_count = std::cell::Cell::new(0);

    let (mut engine_a, failed_a, _next_a, recovery_failure_a, receiver_a, submitted_root_a) =
        permanent_zero_recovery_failure_fixture();
    let digest_a = manifest_digest(0xA00B);
    let permanent_a = engine_a.prepare_failure_cleanup_retaining(
        recovery_failure_a,
        CleanupManifestSeal::new(vec![submitted_root_a], digest_a, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent {
        request: request_a,
        evidence: evidence_a,
    } = permanent_a
    else {
        panic!("pair A must preserve one real permanent contradiction")
    };
    let classified_a = engine_a.classify_failure_cleanup_permanent(
        super::FailureCleanupPermanentHolder {
            request: request_a,
            physical: ZeroFailureCleanupPhysical {
                owner_id: 0xA00C,
                receiver: receiver_a.clone(),
                manifest_digest: digest_a,
                obligation_count: 0,
                drops: &physical_drops,
                panic_on_drop: &panic_on_drop,
            },
            evidence: evidence_a,
        },
        |physical, binding| {
            let observed = physical.observed_projection();
            super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, binding, observed)
        },
    );
    let super::FailureCleanupPermanentCoreOutcome::ProvenZero {
        zero: zero_a,
        owner: owner_a,
    } = classified_a
    else {
        panic!("pair A must bind exactly")
    };

    let (mut engine_b, failed_b, _next_b, recovery_failure_b, receiver_b, submitted_root_b) =
        permanent_zero_recovery_failure_fixture();
    let digest_b = manifest_digest(0xA00D);
    let permanent_b = engine_b.prepare_failure_cleanup_retaining(
        recovery_failure_b,
        CleanupManifestSeal::new(vec![submitted_root_b], digest_b, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent {
        request: request_b,
        evidence: evidence_b,
    } = permanent_b
    else {
        panic!("pair B must preserve one real permanent contradiction")
    };
    let classified_b = engine_b.classify_failure_cleanup_permanent(
        super::FailureCleanupPermanentHolder {
            request: request_b,
            physical: ZeroFailureCleanupPhysical {
                owner_id: 0xA00E,
                receiver: receiver_b.clone(),
                manifest_digest: digest_b,
                obligation_count: 0,
                drops: &physical_drops,
                panic_on_drop: &panic_on_drop,
            },
            evidence: evidence_b,
        },
        |physical, binding| {
            let observed = physical.observed_projection();
            super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, binding, observed)
        },
    );
    let super::FailureCleanupPermanentCoreOutcome::ProvenZero {
        zero: zero_b,
        owner: owner_b,
    } = classified_b
    else {
        panic!("pair B must bind exactly")
    };

    let before_a = engine_a.snapshot();
    let crossed_a = engine_a.complete_failure_cleanup_zero_terminal(owner_a, zero_b, |zero| {
        callback_count.set(callback_count.get() + 1);
        drop(zero);
    });
    let super::FailureCleanupZeroTerminalOutcome::Mismatch {
        zero: zero_b,
        owner: owner_a,
        evidence: mismatch_a,
    } = crossed_a
    else {
        panic!("owner A must reject prepared-zero B")
    };
    assert_eq!(callback_count.get(), 0);
    assert_eq!(zero_b.physical.owner_id, 0xA00E);
    assert_eq!(owner_a.receiver, receiver_a);
    assert_ne!(mismatch_a.expected_binding, mismatch_a.observed_binding);
    assert_eq!(engine_a.snapshot(), before_a);
    assert_eq!(
        engine_a.request(failed_a).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );

    let before_b = engine_b.snapshot();
    let crossed_b = engine_b.complete_failure_cleanup_zero_terminal(owner_b, zero_a, |zero| {
        callback_count.set(callback_count.get() + 1);
        drop(zero);
    });
    let super::FailureCleanupZeroTerminalOutcome::Mismatch {
        zero: zero_a,
        owner: owner_b,
        evidence: mismatch_b,
    } = crossed_b
    else {
        panic!("owner B must reject prepared-zero A")
    };
    assert_eq!(callback_count.get(), 0);
    assert_eq!(zero_a.physical.owner_id, 0xA00C);
    assert_eq!(owner_b.receiver, receiver_b);
    assert_ne!(mismatch_b.expected_binding, mismatch_b.observed_binding);
    assert_eq!(engine_b.snapshot(), before_b);
    assert_eq!(
        engine_b.request(failed_b).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );

    drop(owner_a);
    drop(owner_b);
    drop(zero_a);
    drop(zero_b);
    assert_eq!(physical_drops.get(), 2);
}

#[test]
fn failure_cleanup_zero_bind_mismatch_retains_exact_evidence_and_redrives_zero() {
    let physical_drops = std::cell::Cell::new(0);
    let panic_on_drop = std::cell::Cell::new(false);
    let bind_calls = std::cell::Cell::new(0);
    let (mut engine, failed, next, recovery_failure, receiver, submitted_logical_root) =
        permanent_zero_recovery_failure_fixture();
    let submitted_digest = manifest_digest(0xA006);
    let observed_digest = manifest_digest(0xA007);
    let permanent = engine.prepare_failure_cleanup_retaining(
        recovery_failure,
        CleanupManifestSeal::new(vec![submitted_logical_root.clone()], submitted_digest, 0),
    );
    let super::FailureCleanupPrepareOutcome::Permanent { request, evidence } = permanent else {
        panic!("a real logical-root contradiction must be permanently classified")
    };
    let contradiction = super::FailureCleanupManifestContradictionProjection {
        source: receiver.clone(),
        expected_logical_roots: Vec::new(),
        observed_logical_roots: vec![submitted_logical_root],
        submitted_manifest_digest: submitted_digest,
        submitted_obligation_count: 0,
    };
    assert_eq!(evidence, contradiction);
    let holder = super::FailureCleanupPermanentHolder {
        request,
        physical: ZeroFailureCleanupPhysical {
            owner_id: 0xA008,
            receiver: receiver.clone(),
            manifest_digest: observed_digest,
            obligation_count: 0,
            drops: &physical_drops,
            panic_on_drop: &panic_on_drop,
        },
        evidence,
    };
    let expected = super::FailureCleanupZeroPhysicalProjection {
        receiver: receiver.clone(),
        manifest_digest: submitted_digest,
        obligation_count: 0,
    };
    let observed = super::FailureCleanupZeroPhysicalProjection {
        receiver: receiver.clone(),
        manifest_digest: observed_digest,
        obligation_count: 0,
    };
    let expected_mismatch = super::FailureCleanupZeroPhysicalMismatchProjection {
        source: receiver,
        expected,
        observed,
    };
    let before_classification = engine.snapshot();

    let classified = engine.classify_failure_cleanup_permanent(holder, |physical, expected| {
        bind_calls.set(bind_calls.get() + 1);
        let observed = physical.observed_projection();
        super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, expected, observed)
    });
    let super::FailureCleanupPermanentCoreOutcome::ZeroMismatch { holder, evidence } = classified
    else {
        panic!("digest drift must return the whole permanent holder")
    };
    assert_eq!(bind_calls.get(), 1);
    assert_eq!(physical_drops.get(), 0);
    assert_eq!(holder.request.request(), failed);
    assert_eq!(holder.physical.owner_id, 0xA008);
    assert_eq!(holder.evidence, contradiction);
    assert_eq!(evidence, expected_mismatch);
    assert_eq!(engine.snapshot(), before_classification);
    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert!(
        engine
            .request(failed)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    assert!(engine.action().is_none());

    let classified = engine.classify_failure_cleanup_permanent(holder, |physical, expected| {
        bind_calls.set(bind_calls.get() + 1);
        let observed = physical.observed_projection();
        super::PreparedFailureCleanupZeroPhysical::bind_zero(physical, expected, observed)
    });
    let super::FailureCleanupPermanentCoreOutcome::ZeroMismatch { holder, evidence } = classified
    else {
        panic!("an explicit retry must reproduce the same typed zero mismatch")
    };
    assert_eq!(bind_calls.get(), 2);
    assert_eq!(physical_drops.get(), 0);
    assert_eq!(holder.request.request(), failed);
    assert_eq!(holder.physical.owner_id, 0xA008);
    assert_eq!(holder.evidence, contradiction);
    assert_eq!(evidence, expected_mismatch);
    assert_eq!(engine.snapshot(), before_classification);
    assert!(
        engine
            .request(failed)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    assert!(engine.action().is_none());
    drop(holder);
    assert_eq!(physical_drops.get(), 1);
}

#[test]
fn missing_outlet_waits_for_cleanup_before_router_fallback() {
    let mut engine = ready_engine();
    let (request, parent, mount) = begin_nested_mount(&mut engine, Vec::new());

    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();

    assert_eq!(failure.logical_roots(), &[parent]);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    assert!(engine.action().is_none());
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 101, 1);
    let rejected = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .find(|event| event.kind() == NavigationTraceEventKind::MountRejected)
        .unwrap();
    assert_eq!(
        rejected.mount_rejection(),
        Some(crate::trace::MountRejectionKind::MissingOutlet)
    );
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("missing outlet cleanup must issue RouterFallback")
    };
    assert_eq!(fallback.identity().request(), request);
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let trace = engine.request(request).unwrap().trace();
    let rejected = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::MountRejected)
        .unwrap();
    let settled = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .unwrap();
    let fallback_issued = trace
        .iter()
        .rposition(|event| {
            event.kind() == NavigationTraceEventKind::FrameTaskIssued
                && event
                    .frame()
                    .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
        })
        .unwrap();
    assert!(rejected < settled && settled < fallback_issued);
}

#[test]
fn nested_failure_cleanup_preserves_old_active_branch_and_history() {
    let mut engine = ready_engine_with_tree(reconciliation_tree());
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let old_active = engine.active().clone();
    let old_history = engine.history().clone();
    let old_frames = old_active.frames().to_vec();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("append without a retained slot must issue one mount request")
    };

    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();

    assert!(failure.logical_roots().is_empty());
    assert!(
        failure
            .logical_roots()
            .iter()
            .all(|root| !old_frames.contains(root))
    );
    assert_eq!(engine.active(), &old_active);
    assert_eq!(engine.history(), &old_history);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    assert!(engine.action().is_none());
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 102, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert_eq!(engine.active(), &old_active);
    assert_eq!(engine.history(), &old_history);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("missing nested binding must recover through RouterFallback")
    };
    assert_eq!(fallback.identity().request(), request);
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
}

#[test]
fn failure_cleanup_orders_only_accepted_applied_prefix_child_to_parent() {
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    let layout = builder
        .route_with_options(
            root,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    builder
        .route_with_options(
            layout,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(root_task) = engine.action().unwrap() else {
        panic!("root task expected")
    };
    let root_frame = root_task.frame_version().clone();
    engine
        .accept_applied_report(
            root_task.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
        )
        .unwrap();
    let EngineAction::MountRequest(layout_mount) = engine.action().unwrap() else {
        panic!("layout mount expected")
    };
    let grant = engine.resolve_mount(layout_mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(211));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(223));
    engine.complete_mount_registration(completion).unwrap();
    let EngineAction::FrameTask(layout_task) = engine.action().unwrap() else {
        panic!("layout task expected")
    };
    let layout_frame = layout_task.frame_version().clone();
    engine
        .accept_applied_report(layout_task.into_applied_report(Vec::new()))
        .unwrap();
    let EngineAction::MountRequest(leaf_mount) = engine.action().unwrap() else {
        panic!("missing leaf outlet must still yield a mount request")
    };

    let failure = engine
        .reject_mount(leaf_mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();

    assert_eq!(
        failure.logical_roots(),
        &[layout_frame.clone(), root_frame.clone()]
    );
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(vec![layout_frame, root_frame], manifest_digest(227), 2),
        )
        .unwrap();
    let _fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
}

#[test]
fn invalid_outlet_waits_for_cleanup_before_router_fallback() {
    let mut engine = ready_engine();
    let (request, parent, mount) =
        begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);

    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::InvalidOutlet)
        .unwrap();

    assert_eq!(failure.logical_roots(), &[parent]);
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 103, 1);
    let rejected = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .find(|event| event.kind() == NavigationTraceEventKind::MountRejected)
        .unwrap();
    assert_eq!(
        rejected.mount_rejection(),
        Some(crate::trace::MountRejectionKind::InvalidOutlet)
    );
    let _fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
}

#[test]
fn rejected_after_mutation_burns_slot_and_waits_for_cleanup() {
    let mut engine = ready_engine();
    let (request, parent, mount) =
        begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let burned_slot = grant.slot();
    let next_slot = engine.ids.snapshot().next_slot();

    let failure = engine
        .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant))
        .unwrap();

    assert_eq!(failure.logical_roots(), &[parent]);
    assert_eq!(engine.ids.snapshot().next_slot(), next_slot);
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 107, 2);
    assert_eq!(cleanup.obligation_count(), 2);
    let rejected = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .find(|event| event.kind() == NavigationTraceEventKind::MountRejectedAfterMutation)
        .unwrap();
    assert_eq!(rejected.slot(), Some(burned_slot));
    assert_eq!(
        rejected.mount_rejection(),
        Some(crate::trace::MountRejectionKind::RejectedAfterMutation)
    );
    let _fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
}

#[test]
fn child_build_and_apply_failure_wait_for_cleanup() {
    for kind in [NavigationFailureKind::Build, NavigationFailureKind::Apply] {
        let mut engine = ready_engine();
        let (request, parent, mount) =
            begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
        let grant = engine.resolve_mount(mount).unwrap();
        let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(109));
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(113));
        engine.complete_mount_registration(completion).unwrap();
        let EngineAction::FrameTask(child) = engine.action().unwrap() else {
            panic!("confirmed child task expected")
        };
        let child_frame = child.frame_version().clone();

        let failure = engine
            .begin_view_report(child.identity())
            .unwrap()
            .failed(NavigationFailure::new(kind, "child frame failed"))
            .expect("child failure requires cleanup");

        assert_eq!(failure.logical_roots(), &[parent.clone()]);
        assert_ne!(failure.logical_roots(), &[child_frame]);
        let cleanup = prepare_failure_cleanup(&mut engine, failure, 127, 2);
        let _fallback =
            complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
    }
}

#[test]
fn parent_authority_loss_recovers_before_authority_lost_terminal() {
    let mut engine = ready_engine();
    let (request, parent, mount) =
        begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);

    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::ParentAuthorityLost)
        .unwrap();

    assert_eq!(failure.logical_roots(), &[parent]);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    assert!(engine.action().is_none());
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| {
                !(event.kind() == NavigationTraceEventKind::FrameTaskIssued
                    && event
                        .frame()
                        .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback))
            })
    );
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 131, 1);
    assert_failure_waits_then_settles(
        &mut engine,
        request,
        &cleanup,
        NavigationFailureKind::AuthorityLost,
    );
}

#[test]
fn reconciliation_authority_loss_rejects_queue_settles_then_readmits_without_replay() {
    let mut engine = RouterEngine::install(
        reconciliation_tree(),
        RouteViewKey::new("fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();
    let lease = confirmation.into_lease();
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let history_before = engine.history().clone();

    let current = engine.push(RouteLocation::new("/items/1")).unwrap();
    let action = engine.action().unwrap();
    let _ticket = drive_to_commit(&mut engine, action);
    assert!(
        engine
            .testing_history_capability_snapshot()
            .has_exactly_one_owner()
    );
    let queued_first = engine.enqueue_push(RouteLocation::new("/items/2")).unwrap();
    let queued_second = engine.enqueue_push(RouteLocation::new("/items/3")).unwrap();

    let roots = engine.authority_settlement_logical_roots();
    let obligation_count = usize::from(!roots.is_empty());
    let cleanup = engine
        .begin_authority_settlement_retaining(
            &mut issuer,
            lease,
            CleanupManifestSeal::new(roots, manifest_digest(0x515), obligation_count),
            |engine, _| {
                assert_eq!(
                    engine.availability(),
                    &crate::presentation::RouterAvailability::Ready
                );
                assert!(engine.host().is_some());
                assert!(matches!(
                    engine.request(current).unwrap().status(),
                    RequestStatus::Running(_)
                ));
            },
        )
        .unwrap();

    assert!(!engine.testing_history_capability_snapshot().has_owner());
    assert!(engine.testing_history_drift_evidence_snapshot().is_none());

    assert_eq!(
        engine.availability(),
        &crate::presentation::RouterAvailability::RecoveringAuthority
    );
    assert_eq!(
        engine.request(current).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    for queued in [queued_first, queued_second] {
        assert!(matches!(
            engine.request(queued).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::AuthorityLost
        ));
        assert_eq!(
            engine
                .request(queued)
                .unwrap()
                .trace()
                .last()
                .map(NavigationTraceEvent::kind),
            Some(NavigationTraceEventKind::RequestTerminal)
        );
    }

    let ids_before_rejected_push = engine.ids.snapshot();
    assert_eq!(
        engine.enqueue_push(RouteLocation::new("/items/4")),
        Err(PushError::AuthorityRecovering)
    );
    assert_eq!(engine.ids.snapshot(), ids_before_rejected_push);
    if obligation_count != 0 {
        engine
            .observe_cleanup_receipt(&cleanup, crate::trace::CleanupDisposition::Removed)
            .unwrap();
    }
    let receipt_events = engine
        .request(current)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.cleanup_disposition().is_some())
        .count();
    let mut invalid_completion = valid_completion(&cleanup);
    invalid_completion.testing_invalidate_binding();
    let mutation_ran = std::cell::Cell::new(false);
    assert_eq!(
        engine.complete_cleanup_with_authority_mutation(invalid_completion, || {
            mutation_ran.set(true)
        }),
        Err(CleanupError::Invalid)
    );
    assert!(!mutation_ran.get());
    assert_eq!(
        engine.availability(),
        &crate::presentation::RouterAvailability::RecoveringAuthority
    );
    assert_eq!(
        engine
            .request(current)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.cleanup_disposition().is_some())
            .count(),
        receipt_events
    );
    engine
        .complete_cleanup_with_authority_mutation(valid_completion(&cleanup), || {})
        .unwrap();

    let trace = engine.request(current).unwrap().trace();
    let receipt = trace.iter().position(|event| {
        event.kind() == NavigationTraceEventKind::CleanupSettled
            && event.cleanup_disposition().is_some()
    });
    let settled = trace
        .iter()
        .rposition(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .unwrap();
    let terminal = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .unwrap();
    if let Some(receipt) = receipt {
        assert!(receipt < settled);
    }
    assert!(settled < terminal);
    assert!(matches!(
        engine.request(current).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert!(engine.host().is_none());
    assert_eq!(engine.history(), &history_before);
    assert!(engine.action().is_none());
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    assert!(engine.testing_history_drift_evidence_snapshot().is_none());

    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();

    assert_eq!(
        engine.availability(),
        &crate::presentation::RouterAvailability::Ready
    );
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert_eq!(engine.history(), &history_before);
    assert!(engine.running_request().is_none());
    assert!(engine.action().is_none());
}

#[test]
fn authority_protocol_failure_retires_lease_and_rejects_readmission_before_issue() {
    let mut engine = RouterEngine::install(
        reconciliation_tree(),
        RouteViewKey::new("fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();
    let lease = confirmation.into_lease();
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let action = engine.action().unwrap();
    let _ticket = drive_to_commit(&mut engine, action);
    assert!(
        engine
            .testing_history_capability_snapshot()
            .has_exactly_one_owner()
    );
    let isolated = std::cell::Cell::new(false);

    engine
        .fail_closed_authority_protocol_retaining(&mut issuer, Some(lease), || isolated.set(true))
        .unwrap();

    assert!(isolated.get());
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    assert!(engine.testing_history_drift_evidence_snapshot().is_none());
    assert!(!issuer.has_current());
    assert_eq!(
        engine.availability(),
        &crate::presentation::RouterAvailability::Unavailable(
            crate::presentation::RouterUnavailableReason::RegistryInvariantViolation,
        )
    );
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost
    ));
    let ids_before = engine.ids.snapshot();
    assert_eq!(
        engine.request_legacy_host_admission(&mut issuer),
        Err(AdmissionError::AuthorityExists)
    );
    assert_eq!(engine.ids.snapshot(), ids_before);
}

#[test]
fn authority_protocol_terminal_prepares_queue_retention_before_consuming_owner() {
    let (mut engine, mut issuer, lease) = ready_engine_with_tree_and_authority(tree());
    let running = engine.push(RouteLocation::new("/items/1")).unwrap();
    let queued = (0..70)
        .map(|_| engine.enqueue_push(RouteLocation::new("/items/2")).unwrap())
        .collect::<Vec<_>>();
    let before = engine.snapshot();
    assert!(!engine.fail_closed_authority_protocol(|| false));
    assert_eq!(engine.snapshot(), before);
    assert!(issuer.has_current());

    let consumed = std::cell::Cell::new(0);
    assert!(
        engine
            .fail_closed_authority_protocol_retaining(&mut issuer, Some(lease), || consumed
                .set(consumed.get() + 1),)
            .is_ok()
    );
    assert_eq!(consumed.get(), 1);
    assert!(!issuer.has_current());
    assert!(engine.action().is_none());
    assert!(engine.running_request().is_none());
    assert!(engine.queue.is_empty());
    assert_eq!(engine.records.len(), 64);
    assert!(matches!(
        engine.private_lookup(running),
        PrivateRequestLookup::Evicted
    ));
    for request in &queued[..6] {
        assert!(matches!(
            engine.private_lookup(*request),
            PrivateRequestLookup::Evicted
        ));
    }
    for request in &queued[6..] {
        let record = engine.request(*request).unwrap();
        assert!(matches!(record.status(), RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost));
        assert_eq!(
            record.trace().last(),
            Some(&NavigationTraceEvent::request_terminal())
        );
    }
}

#[test]
fn permanent_cleanup_failure_preserves_published_request_outcome_and_history() {
    for (location, succeeded) in [("/items/2", true), ("/missing", false)] {
        let (mut engine, mut issuer, lease) = ready_engine_with_tree_and_authority(tree());
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        let request = engine.push(RouteLocation::new(location)).unwrap();
        let _normal_cleanup = publish_current(&mut engine);
        let committed_history = engine.history().clone();
        let queued = engine.enqueue_push(RouteLocation::new("/items/3")).unwrap();
        let isolated = std::cell::Cell::new(0);
        assert!(
            engine
                .fail_closed_authority_protocol_retaining(&mut issuer, Some(lease), || isolated
                    .set(isolated.get() + 1),)
                .is_ok()
        );
        assert_eq!(isolated.get(), 1);
        assert_eq!(engine.history(), &committed_history);
        let status = engine.request(request).unwrap().status();
        if succeeded {
            assert_eq!(status, &RequestStatus::Succeeded);
        } else {
            assert!(matches!(status, RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::Match));
        }
        assert!(
            matches!(engine.request(queued).unwrap().status(), RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::AuthorityLost)
        );
        assert!(engine.running_request().is_none());
        assert!(engine.action().is_none());
        assert!(engine.pending_post_commit_terminal.is_none());
        assert!(engine.pending_post_commit_settlement_trace.is_none());
    }
}

#[test]
fn failed_authority_trigger_requires_an_exact_refrozen_terminal_snapshot() {
    let mut engine = RouterEngine::install(
        reconciliation_tree(),
        RouteViewKey::new("fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();
    let lease = confirmation.into_lease();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("failed terminal fixture requires one exact frame task")
    };
    let trigger = engine
        .failed_authority_trigger_for_task(task.identity())
        .unwrap();

    engine
        .current
        .as_mut()
        .expect("task retains current work")
        .applied_frames
        .push(task.frame_version().clone());
    let mut before = engine.snapshot();
    before.live_failed_authority_trigger_revision = None;
    let mutation_ran = std::cell::Cell::new(false);
    let error = engine
        .begin_failed_authority_settlement_retaining(
            &mut issuer,
            lease,
            trigger,
            CleanupManifestSeal::new(
                vec![task.frame_version().clone()],
                manifest_digest(0xFA16),
                1,
            ),
            |_, _| mutation_ran.set(true),
        )
        .expect_err("a merely valid but different refrozen snapshot is stale");

    assert_eq!(
        error.kind(),
        super::BeginAuthoritySettlementErrorKind::Stale
    );
    let _lease = error.into_owner();
    assert!(!mutation_ran.get());
    assert!(issuer.has_current());
    assert_eq!(
        engine.live_failed_authority_trigger_revision, None,
        "an exact-receiver rejection burns the one-shot trigger while returning the lease"
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn failed_authority_source_selection_uses_exact_roots_not_entity_count() {
    let mut engine = RouterEngine::install(
        reconciliation_tree(),
        RouteViewKey::new("fallback").unwrap(),
    )
    .unwrap();
    let mut issuer = HostAdmissionIssuer::new().unwrap();
    let token = engine.request_legacy_host_admission(&mut issuer).unwrap();
    let grant = engine
        .grant_legacy_host_admission(&mut issuer, &token)
        .unwrap();
    let confirmation = issuer.confirm(&grant).unwrap();
    engine.confirm_host(&issuer, &confirmation).unwrap();
    let lease = confirmation.into_lease();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("failed source fixture requires one exact frame task")
    };
    let trigger = engine
        .failed_authority_trigger_for_task(task.identity())
        .unwrap();
    let roots = vec![task.frame_version().clone()];
    let digest = manifest_digest(0xA570);
    let expected = CleanupBatchToken::sealed(
        task.identity().clone(),
        engine.current.as_ref().unwrap().cleanup_batch,
        &roots,
        digest,
        0,
    );

    let cleanup = engine
        .begin_failed_authority_settlement_retaining(
            &mut issuer,
            lease,
            trigger,
            CleanupManifestSeal::new(roots, digest, 0),
            |_, _| {},
        )
        .expect("exact logical target selection is independent of ECS entity count");

    assert_eq!(cleanup, expected);
    assert!(matches!(
        engine.current.as_ref().map(|current| &current.stage),
        Some(WorkStage::FailureCleanupPrepared {
            continuation: super::FailureCleanupContinuationOwner::AuthoritySettlement,
            ..
        })
    ));
}

#[test]
fn failure_cleanup_roots_count_and_continuation_are_independent() {
    fn pending_failure(
        roots: Vec<RouteFrameVersion>,
        continuation: super::FailureCleanupContinuationOwner,
    ) -> (RouterEngine, super::FailureCleanupRequest) {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        let request = engine.begin_failure_cleanup_with_roots(
            super::PendingFailureKind::Apply,
            NavigationFailure::new(
                NavigationFailureKind::Apply,
                "independent cleanup manifest fixture",
            ),
            continuation,
            roots,
        );
        (engine, request)
    }

    let (mut empty, empty_request) =
        pending_failure(Vec::new(), super::FailureCleanupContinuationOwner::Terminal);
    let empty_digest = manifest_digest(0xC1EA);
    let empty_expected = CleanupBatchToken::sealed(
        empty_request.receiver.identity.clone(),
        empty.current.as_ref().unwrap().cleanup_batch,
        &[],
        empty_digest,
        2,
    );
    let empty_cleanup = empty
        .prepare_failure_cleanup(
            empty_request,
            CleanupManifestSeal::new(Vec::new(), empty_digest, 2),
        )
        .unwrap();
    assert_eq!(empty_cleanup, empty_expected);
    assert!(matches!(
        empty.current.as_ref().map(|current| &current.stage),
        Some(WorkStage::FailureCleanupPrepared {
            continuation: super::FailureCleanupContinuationOwner::Terminal,
            ..
        })
    ));

    let mut roots_engine = ready_engine();
    roots_engine.push(RouteLocation::new("/items/1")).unwrap();
    let root = roots_engine.current.as_ref().unwrap().target_frames[0].clone();
    let roots_request = roots_engine.begin_failure_cleanup_with_roots(
        super::PendingFailureKind::Apply,
        NavigationFailure::new(
            NavigationFailureKind::Apply,
            "zero entity obligation fixture",
        ),
        super::FailureCleanupContinuationOwner::BeginFallback,
        vec![root.clone()],
    );
    let roots_digest = manifest_digest(0xC1EB);
    let roots_expected = CleanupBatchToken::sealed(
        roots_request.receiver.identity.clone(),
        roots_engine.current.as_ref().unwrap().cleanup_batch,
        std::slice::from_ref(&root),
        roots_digest,
        0,
    );
    let roots_cleanup = roots_engine
        .prepare_failure_cleanup(
            roots_request,
            CleanupManifestSeal::new(vec![root], roots_digest, 0),
        )
        .expect("logical cleanup roots do not imply an ECS entity obligation");
    assert_eq!(roots_cleanup, roots_expected);
    assert!(matches!(
        roots_engine.current.as_ref().map(|current| &current.stage),
        Some(WorkStage::FailureCleanupPrepared {
            continuation: super::FailureCleanupContinuationOwner::BeginFallback,
            ..
        })
    ));
}

#[test]
fn failure_cleanup_request_stage_manifest_is_exact_zero_write() {
    let mut engine = ready_engine();
    let (_request, parent, mount) = begin_nested_mount(&mut engine, Vec::new());
    let mut failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();

    let router = failure.receiver.identity.router_identity();
    failure
        .receiver
        .identity
        .testing_set_router(distinct_identity().router_identity());
    let roots = failure.logical_roots().to_vec();
    let before = engine.snapshot();
    let outcome = engine.prepare_failure_cleanup_retaining(
        failure,
        CleanupManifestSeal::new(roots.clone(), manifest_digest(135), 1),
    );
    let super::FailureCleanupPrepareOutcome::Mismatch { request, evidence } = outcome else {
        panic!("foreign prepare must return the request and typed receiver evidence")
    };
    assert_eq!(evidence.kind, super::FailureCleanupError::Foreign);
    failure = request;
    assert_eq!(engine.snapshot(), before);

    failure.receiver.identity.testing_set_router(router);
    let host_epoch = failure.receiver.identity.host_epoch();
    failure
        .receiver
        .identity
        .testing_set_host_epoch(host_epoch + 1);
    let before = engine.snapshot();
    let outcome = engine.prepare_failure_cleanup_retaining(
        failure,
        CleanupManifestSeal::new(roots.clone(), manifest_digest(137), 1),
    );
    let super::FailureCleanupPrepareOutcome::Mismatch { request, evidence } = outcome else {
        panic!("stale identity must return the request and typed receiver evidence")
    };
    assert_eq!(evidence.kind, super::FailureCleanupError::Stale);
    failure = request;
    assert_eq!(engine.snapshot(), before);

    failure.receiver.identity.testing_set_host_epoch(host_epoch);
    let expected = failure.receiver.expected.clone();
    failure.receiver.expected = super::PendingFailureKind::Build;
    let before = engine.snapshot();
    let outcome = engine.prepare_failure_cleanup_retaining(
        failure,
        CleanupManifestSeal::new(roots.clone(), manifest_digest(139), 1),
    );
    let super::FailureCleanupPrepareOutcome::Mismatch { request, evidence } = outcome else {
        panic!("stale failure kind must return the request and typed receiver evidence")
    };
    assert_eq!(evidence.kind, super::FailureCleanupError::Stale);
    failure = request;
    assert_eq!(engine.snapshot(), before);

    failure.receiver.expected = expected;
    let before = engine.snapshot();
    let outcome = engine.prepare_failure_cleanup_retaining(
        failure,
        CleanupManifestSeal::new(Vec::new(), manifest_digest(149), 1),
    );
    let super::FailureCleanupPrepareOutcome::Permanent { request, evidence } = outcome else {
        panic!("logical-root contradiction must be permanently classified")
    };
    assert_eq!(evidence.source, request.receiver);
    assert_eq!(evidence.expected_logical_roots, roots);
    assert!(evidence.observed_logical_roots.is_empty());
    assert_eq!(evidence.submitted_manifest_digest, manifest_digest(149));
    assert_eq!(evidence.submitted_obligation_count, 1);
    failure = request;
    assert_eq!(engine.snapshot(), before);

    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(vec![parent], manifest_digest(151), 3),
        )
        .unwrap();
    let prepared = engine.snapshot();

    let malformed = CleanupCompletion::attest(
        cleanup.clone(),
        CleanupCompletionDigest::from_opaque(157),
        cleanup.obligation_count() - 1,
    );
    assert_eq!(
        engine.complete_cleanup(malformed),
        Err(CleanupError::Invalid)
    );
    assert_eq!(engine.snapshot(), prepared);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
}

#[test]
fn failure_cleanup_completion_receiver_mismatch_returns_whole_owner_and_calls_zero() {
    struct PhysicalOwner<'a>(&'a std::cell::Cell<usize>);

    impl Drop for PhysicalOwner<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let mut exact = ready_engine();
    let request = exact.push(RouteLocation::new("/items/1")).unwrap();
    let failure = exact.begin_failure_cleanup_with_roots(
        super::PendingFailureKind::Apply,
        NavigationFailure::new(
            NavigationFailureKind::Apply,
            "completion owner receiver mismatch fixture",
        ),
        super::FailureCleanupContinuationOwner::BeginFallback,
        Vec::new(),
    );
    assert!(matches!(
        exact.observe_prepared_publication_source(0).stage,
        Some(super::WorkStageProjection::FailureCleanupManifestPending(_))
    ));
    let cleanup = exact
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(Vec::new(), manifest_digest(0xC1EC), 0),
        )
        .unwrap();
    assert!(matches!(
        exact.observe_prepared_publication_source(0).stage,
        Some(super::WorkStageProjection::FailureCleanupPrepared { .. })
    ));
    let owner = exact
        .prepare_failure_cleanup_completion(valid_completion(&cleanup))
        .expect("the exact prepared cleanup mints one completion owner");
    assert!(matches!(
        exact.observe_prepared_publication_source(0).stage,
        Some(super::WorkStageProjection::FailureCleanupCompleting(_))
    ));

    let mut foreign = ready_engine();
    let callback_count = std::cell::Cell::new(0);
    let physical_drops = std::cell::Cell::new(0);
    let error = foreign
        .complete_prepared_failure_cleanup_with_ledger(
            owner,
            PhysicalOwner(&physical_drops),
            |physical| {
                drop(physical);
                callback_count.set(callback_count.get() + 1);
            },
        )
        .expect_err("a foreign Core receiver must return the whole completion owner");
    assert_eq!(error.kind(), CleanupError::Stale);
    assert_eq!(callback_count.get(), 0);
    assert_eq!(
        physical_drops.get(),
        0,
        "receiver mismatch must return rather than drop the typed physical owner"
    );
    let (owner, physical, mismatch) = error.into_parts();
    assert_ne!(mismatch.expected, mismatch.observed);

    exact
        .complete_prepared_failure_cleanup_with_ledger(owner, physical, |physical| {
            drop(physical);
            callback_count.set(callback_count.get() + 1);
        })
        .expect("the returned owner remains consumable by its exact Core receiver");
    assert_eq!(callback_count.get(), 1);
    assert_eq!(physical_drops.get(), 1);
    assert_eq!(
        exact.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let Some(EngineAction::FrameTask(fallback)) = exact.action() else {
        panic!("the exact completion must continue through RouterFallback")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
}

#[test]
fn foreign_and_replayed_mount_rejections_are_classified_without_lifecycle_write() {
    let mut engine = ready_engine();
    let (_request, _parent, exact) = begin_nested_mount(&mut engine, Vec::new());
    let replay = exact.testing_duplicate();
    let mut foreign_engine = ready_engine();
    let (_, _, foreign) = begin_nested_mount(&mut foreign_engine, Vec::new());

    let before = engine.snapshot();
    assert_eq!(
        engine
            .reject_mount(foreign, crate::trace::MountRejectionKind::MissingOutlet)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Foreign)
    );
    assert_eq!(engine.snapshot(), before);

    let before = engine.snapshot();
    assert_eq!(
        engine
            .reject_mount(
                exact.testing_duplicate(),
                crate::trace::MountRejectionKind::RejectedAfterMutation,
            )
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::WrongPhase)
    );
    assert_eq!(engine.snapshot(), before);

    engine
        .reject_mount(exact, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();
    let failed = engine.snapshot();
    assert_eq!(
        engine
            .reject_mount(replay, crate::trace::MountRejectionKind::MissingOutlet)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Stale)
    );
    assert_eq!(engine.snapshot(), failed);
}

#[test]
fn registration_stage_rejection_uses_only_the_live_permit_and_burned_slot() {
    let mut engine = ready_engine();
    let (request, parent, mount) =
        begin_nested_mount(&mut engine, vec![crate::slot::RouteSlotKey::default()]);
    let grant = engine.resolve_mount(mount).unwrap();
    let slot = grant.slot();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(163));
    let permit = engine.accept_mount_receipt(receipt).unwrap();

    let failure = engine
        .reject_mount_after_mutation(super::MountMutationArtifact::RegistrationPermit(permit))
        .unwrap();

    assert_eq!(failure.logical_roots(), &[parent]);
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 167, 2);
    let rejected = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .find(|event| event.kind() == NavigationTraceEventKind::MountRejectedAfterMutation)
        .unwrap();
    assert_eq!(rejected.slot(), Some(slot));
    let _fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
}

#[test]
fn recovery_cleanup_completion_starts_next_request_and_failure_replay_is_duplicate() {
    let mut engine = ready_engine();
    let (failed, parent, mount) = begin_nested_mount(&mut engine, Vec::new());
    let next = engine.enqueue_push(RouteLocation::new("/about")).unwrap();
    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(vec![parent], manifest_digest(173), 1),
        )
        .unwrap();
    let completion = valid_completion(&cleanup);
    let replay = completion.clone();

    assert!(
        engine
            .request(failed)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::RequestTerminal)
    );
    engine.complete_cleanup(completion).unwrap();

    assert_eq!(
        engine.request(failed).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("failure cleanup must issue RouterFallback before starting the queue")
    };
    assert_eq!(fallback.identity().request(), failed);
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let recovering = engine.snapshot();
    assert_eq!(
        engine.complete_cleanup(replay),
        Err(CleanupError::Duplicate)
    );
    assert_eq!(engine.snapshot(), recovering);

    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "router fallback apply failed",
        ));
    assert!(recovery_failure.logical_roots().is_empty());
    let recovery_cleanup = prepare_failure_cleanup(&mut engine, recovery_failure, 175, 0);
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    engine
        .complete_cleanup(valid_completion(&recovery_cleanup))
        .unwrap();

    assert!(matches!(
        engine.request(failed).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::ApplyingFrame(0))
    );
    let Some(EngineAction::FrameTask(task)) = engine.action() else {
        panic!("next queued request starts only after recovery cleanup completion")
    };
    assert_eq!(task.identity().request(), next);
}

#[test]
fn mount_sequence_exhaustion_waits_for_cleanup_before_fallback() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(parent) = engine.action().unwrap() else {
        panic!("nested parent task expected")
    };
    let parent_frame = parent.frame_version().clone();
    engine.testing_exhaust_mount_sequence_counter();
    let ids_before = engine.ids.snapshot();

    engine
        .accept_applied_report(
            parent.into_applied_report(vec![crate::slot::RouteSlotKey::default()]),
        )
        .unwrap();

    assert_eq!(engine.ids.snapshot(), ids_before);
    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!("accepted parent mount-sequence exhaustion must yield failure cleanup")
    };
    assert_eq!(failure.request(), request);
    assert_eq!(failure.logical_roots(), &[parent_frame.clone()]);
    assert_eq!(failure.failure().kind(), NavigationFailureKind::Mount);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    let trace = engine.request(request).unwrap().trace();
    assert!(trace.iter().all(|event| !matches!(
        event.kind(),
        NavigationTraceEventKind::MountRejected
            | NavigationTraceEventKind::MountRejectedAfterMutation
    )));
    assert!(trace.iter().all(|event| {
        event
            .frame()
            .is_none_or(|frame| frame.origin() != &FrameOrigin::RouterFallback)
    }));

    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(vec![parent_frame], manifest_digest(179), 1),
        )
        .unwrap();
    let _fallback = complete_failure_cleanup_into_router_fallback(&mut engine, request, &cleanup);
}

#[test]
fn resolve_and_mutation_artifacts_preserve_foreign_and_stale_taxonomy() {
    let mut resolve_engine = ready_engine();
    let (_, _, exact_request) = begin_nested_mount(
        &mut resolve_engine,
        vec![crate::slot::RouteSlotKey::default()],
    );
    let replay_request = exact_request.testing_duplicate();
    let mut foreign_engine = ready_engine();
    let (_, _, foreign_request) = begin_nested_mount(
        &mut foreign_engine,
        vec![crate::slot::RouteSlotKey::default()],
    );

    let before = resolve_engine.snapshot();
    assert!(matches!(
        resolve_engine
            .resolve_mount(foreign_request)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Foreign)
    ));
    assert_eq!(resolve_engine.snapshot(), before);
    let exact_grant = resolve_engine.resolve_mount(exact_request).unwrap();
    let grant_replay = exact_grant.clone();
    let resolved = resolve_engine.snapshot();
    assert!(matches!(
        resolve_engine
            .resolve_mount(replay_request)
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Stale)
    ));
    assert_eq!(resolve_engine.snapshot(), resolved);

    let mut foreign_grant_engine = ready_engine();
    let (_, _, foreign_mount) = begin_nested_mount(
        &mut foreign_grant_engine,
        vec![crate::slot::RouteSlotKey::default()],
    );
    let foreign_grant = foreign_grant_engine.resolve_mount(foreign_mount).unwrap();
    let before = resolve_engine.snapshot();
    assert!(matches!(
        resolve_engine
            .reject_mount_after_mutation(super::MountMutationArtifact::Grant(foreign_grant))
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Foreign)
    ));
    assert_eq!(resolve_engine.snapshot(), before);

    resolve_engine
        .reject_mount_after_mutation(super::MountMutationArtifact::Grant(exact_grant))
        .unwrap();
    let failed = resolve_engine.snapshot();
    assert!(matches!(
        resolve_engine
            .reject_mount_after_mutation(super::MountMutationArtifact::Grant(grant_replay))
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Stale)
    ));
    assert_eq!(resolve_engine.snapshot(), failed);
}

#[test]
fn root_apply_failure_waits_for_cleanup_before_router_fallback() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/")).unwrap();
    let EngineAction::FrameTask(root) = engine.action().unwrap() else {
        panic!("root task expected")
    };

    let failure_cleanup =
        engine
            .begin_view_report(root.identity())
            .unwrap()
            .failed(NavigationFailure::new(
                NavigationFailureKind::Apply,
                "root apply failed",
            ));

    let failure_cleanup = failure_cleanup.expect("root apply failure must enter canonical cleanup");
    assert!(failure_cleanup.logical_roots().is_empty());
    assert!(engine.action().is_none());
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    let cleanup = prepare_failure_cleanup(&mut engine, failure_cleanup, 0xb26, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("Root apply failure must retain mandatory fallback")
    };
    assert_eq!(fallback.identity().request(), request);
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
}

#[test]
fn nested_apply_failure_selects_bound_error_view_before_router_fallback() {
    let far_boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
    let far_error_view = RouteViewKey::new("root-error").unwrap();
    let near_boundary = ErrorBoundaryKey::named("items-boundary").unwrap();
    let near_error_view = RouteViewKey::new("items-error").unwrap();
    let router_fallback = RouteViewKey::new("fallback").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("layout").unwrap())
                .with_error_view(far_boundary.clone(), far_error_view.clone()),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    let boundary_route = builder
        .route_with_options(
            root_route,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("items").unwrap())
                .with_error_view(near_boundary.clone(), near_error_view.clone()),
        )
        .unwrap();
    builder
        .route_with_options(
            boundary_route,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    assert_ne!(near_boundary, far_boundary);
    assert_ne!(near_error_view, far_error_view);
    assert_ne!(near_error_view, router_fallback);
    let tree = builder.build().unwrap();
    assert_eq!(
        tree.node(boundary_route)
            .unwrap()
            .options()
            .error_view_binding(),
        Some((&near_boundary, &near_error_view)),
    );

    let mut engine = ready_engine_with_tree(tree);
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let committed = engine.active().frames().to_vec();
    assert_eq!(committed.len(), 3);

    let request = engine.replace(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(failing_leaf) = engine.action().unwrap() else {
        panic!("nested replacement must issue the entering leaf task")
    };
    assert_eq!(failing_leaf.identity().branch_index(), 2);
    let failure = engine
        .begin_view_report(failing_leaf.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "nested leaf apply failed",
        ))
        .expect("nested replace apply failure must enter cleanup");
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 229, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("nested apply failure must issue a recovery view task")
    };
    assert_eq!(recovery.identity().request(), request);
    let expected_origin = FrameOrigin::RouteError(boundary_route);
    let expected_target = FrameMountTarget::RegisteredChildSlot {
        parent: committed[0].clone(),
        outlet: RouteSlotKey::default(),
    };
    assert_eq!(
        (
            recovery.identity().origin(),
            recovery.identity().branch_index(),
            recovery.view(),
            recovery.mount_target(),
        ),
        (&expected_origin, 1, &near_error_view, &expected_target,),
        "the nearest exact boundary binding must replace the boundary-and-below suffix while the strict ancestor prefix remains mounted",
    );
}

#[test]
fn root_apply_failure_selects_bound_root_error_view() {
    let root_boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
    let root_error_view = RouteViewKey::new("root-error").unwrap();
    let builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("root").unwrap())
                .with_error_view(root_boundary.clone(), root_error_view.clone()),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    let tree = builder.build().unwrap();
    assert_eq!(
        tree.node(root_route)
            .unwrap()
            .options()
            .error_view_binding(),
        Some((&root_boundary, &root_error_view)),
    );
    let mut engine = ready_engine_with_tree(tree);
    let request = engine.push(RouteLocation::new("/")).unwrap();
    let EngineAction::FrameTask(root) = engine.action().unwrap() else {
        panic!("root task expected")
    };
    assert_eq!(root.identity().branch_index(), 0);
    let expected_target = root.mount_target().clone();
    let failure = engine
        .begin_view_report(root.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "bound root apply failed",
        ));
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 231, 1);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("root apply failure must issue the bound root error view")
    };
    let expected_origin = FrameOrigin::RouteError(root_route);
    assert_eq!(recovery.identity().request(), request);
    assert_eq!(
        (
            recovery.identity().origin(),
            recovery.identity().branch_index(),
            recovery.view(),
            recovery.mount_target(),
        ),
        (&expected_origin, 0, &root_error_view, &expected_target),
        "an exact root binding must win over RouterFallback after cleanup",
    );
}

#[test]
fn root_zero_residual_failure_uses_canonical_zero_failure_ledger() {
    for (kind, message, digest) in [
        (
            NavigationFailureKind::Build,
            "bound root zero-residual build failed",
            0xb01,
        ),
        (
            NavigationFailureKind::Apply,
            "bound root zero-residual apply failed",
            0xb02,
        ),
    ] {
        let root_boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
        let root_error_view = RouteViewKey::new("root-error").unwrap();
        let builder = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new()
                    .with_view(RouteViewKey::new("root").unwrap())
                    .with_error_view(root_boundary, root_error_view.clone()),
            )
            .unwrap();
        let root_route = builder.root_id().unwrap();
        let mut engine = ready_engine_with_tree(builder.build().unwrap());
        let request = engine.push(RouteLocation::new("/")).unwrap();
        let EngineAction::FrameTask(root) = engine.action().unwrap() else {
            panic!("root task expected")
        };
        let expected_target = root.mount_target().clone();
        let history_before = engine.history().clone();
        let active_before = engine.active().clone();

        let failure = engine
            .begin_view_report(root.identity())
            .unwrap()
            .failed(NavigationFailure::new(kind, message))
            .expect("every ordinary zero-residual root failure must enter cleanup first");
        assert!(failure.logical_roots().is_empty());
        assert!(engine.action().is_none());
        assert_eq!(engine.history(), &history_before);
        assert_eq!(engine.active(), &active_before);
        assert_eq!(
            engine.request(request).unwrap().status(),
            &RequestStatus::Running(NavigationPhase::CleanupPending)
        );
        assert!(
            engine
                .request(request)
                .unwrap()
                .trace()
                .iter()
                .all(|event| {
                    event.kind() != NavigationTraceEventKind::CommitPublished
                        && event.frame().is_none_or(|frame| {
                            frame.origin() != &FrameOrigin::RouteError(root_route)
                        })
                })
        );

        let cleanup = prepare_failure_cleanup(&mut engine, failure, digest, 0);
        assert_eq!(cleanup.obligation_count(), 0);
        assert!(engine.action().is_none());
        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
            panic!("canonical zero cleanup completion must issue the bound root error view")
        };
        assert_eq!(recovery.identity().request(), request);
        assert_eq!(
            recovery.identity().origin(),
            &FrameOrigin::RouteError(root_route)
        );
        assert_eq!(recovery.identity().branch_index(), 0);
        assert_eq!(recovery.view(), &root_error_view);
        assert_eq!(recovery.mount_target(), &expected_target);
        assert_eq!(engine.history(), &history_before);
        assert_eq!(engine.active(), &active_before);
    }
}

#[test]
fn nested_push_apply_failure_selects_nearest_bound_error_view() {
    let root_boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
    let root_error_view = RouteViewKey::new("root-error").unwrap();
    let items_boundary = ErrorBoundaryKey::named("items-boundary").unwrap();
    let items_error_view = RouteViewKey::new("items-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("layout").unwrap())
                .with_error_view(root_boundary, root_error_view),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    let items_route = builder
        .route_with_options(
            root_route,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("items").unwrap())
                .with_error_view(items_boundary, items_error_view.clone()),
        )
        .unwrap();
    builder
        .route_with_options(
            items_route,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/items")).unwrap();
    settle_current(&mut engine);
    let committed = engine.active().frames().to_vec();
    assert_eq!(committed.len(), 2);

    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("nested push must request the missing child mount")
    };
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(233));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(235));
    engine.complete_mount_registration(completion).unwrap();
    let EngineAction::FrameTask(failing_leaf) = engine.action().unwrap() else {
        panic!("confirmed nested push mount must issue the leaf task")
    };
    assert_eq!(failing_leaf.identity().branch_index(), 2);

    let failure = engine
        .begin_view_report(failing_leaf.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "nested push leaf apply failed",
        ))
        .expect("nested push apply failure must enter cleanup");
    assert!(failure.logical_roots().is_empty());
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 237, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("nested push apply failure must issue the nearest bound error view")
    };
    let expected_origin = FrameOrigin::RouteError(items_route);
    let expected_target = FrameMountTarget::RegisteredChildSlot {
        parent: committed[0].clone(),
        outlet: RouteSlotKey::default(),
    };
    assert_eq!(recovery.identity().request(), request);
    assert_eq!(
        (
            recovery.identity().origin(),
            recovery.identity().branch_index(),
            recovery.view(),
            recovery.mount_target(),
        ),
        (&expected_origin, 1, &items_error_view, &expected_target),
        "a Push failure must preserve only the strict root prefix and must not settle Terminal",
    );
}

#[test]
fn entering_boundary_recovery_preserves_only_strict_active_prefix() {
    let items_boundary = ErrorBoundaryKey::named("items-boundary").unwrap();
    let items_error_view = RouteViewKey::new("items-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    builder
        .route_with_options(
            root_route,
            RoutePattern::path("legacy").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("legacy").unwrap()),
        )
        .unwrap();
    let items_route = builder
        .route_with_options(
            root_route,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("items").unwrap())
                .with_error_view(items_boundary, items_error_view.clone()),
        )
        .unwrap();
    builder
        .route_with_options(
            items_route,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/legacy")).unwrap();
    settle_current(&mut engine);
    let committed = engine.active().frames().to_vec();
    assert_eq!(committed.len(), 2);

    let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(entering_boundary) = engine.action().unwrap() else {
        panic!("replace must issue the entering boundary task in the registered child slot")
    };
    assert_eq!(entering_boundary.identity().branch_index(), 1);
    let boundary_slot = entering_boundary.frame_version().slot();
    engine
        .accept_applied_report(entering_boundary.into_applied_report(vec![RouteSlotKey::default()]))
        .unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("entering boundary must request its child mount")
    };
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(239));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(241));
    engine.complete_mount_registration(completion).unwrap();
    let EngineAction::FrameTask(failing_leaf) = engine.action().unwrap() else {
        panic!("confirmed entering-boundary child mount must issue the leaf task")
    };
    assert_eq!(failing_leaf.identity().branch_index(), 2);

    let failure = engine
        .begin_view_report(failing_leaf.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "entering boundary child apply failed",
        ))
        .expect("entering boundary failure must enter cleanup");
    let obligation_count = usize::from(!failure.logical_roots().is_empty());
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 243, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("an entering boundary need not be preserved to issue its bound error view")
    };
    let expected_origin = FrameOrigin::RouteError(items_route);
    let expected_target = FrameMountTarget::RegisteredChildSlot {
        parent: committed[0].clone(),
        outlet: RouteSlotKey::default(),
    };
    assert_eq!(recovery.identity().request(), request);
    assert_eq!(recovery.frame_version().slot(), boundary_slot);
    assert_eq!(
        (
            recovery.identity().origin(),
            recovery.identity().branch_index(),
            recovery.view(),
            recovery.mount_target(),
        ),
        (&expected_origin, 1, &items_error_view, &expected_target),
        "only the strict active root prefix is preserved; the replaced boundary itself is not required to be active",
    );
}

#[test]
fn child_mount_rejection_selects_failed_child_boundary() {
    let root_boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
    let root_error_view = RouteViewKey::new("root-error").unwrap();
    let child_boundary = ErrorBoundaryKey::named("child-boundary").unwrap();
    let child_error_view = RouteViewKey::new("child-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("root").unwrap())
                .with_error_view(root_boundary, root_error_view),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    let child_route = builder
        .route_with_options(
            root_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("item").unwrap())
                .with_error_view(child_boundary, child_error_view.clone()),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    let committed = engine.active().frames().to_vec();
    assert_eq!(committed.len(), 1);

    let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("entering child without a registered slot must request a mount")
    };
    assert_eq!(
        engine
            .current
            .as_ref()
            .and_then(|current| current.pending_mount.as_ref())
            .map(|pending| pending.child_index),
        Some(1),
        "the real rejected mount belongs to the failed child route index",
    );
    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();
    assert!(failure.logical_roots().is_empty());
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 245, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::MountRequest(recovery_mount)) = engine.action() else {
        panic!("child mount rejection must issue a fresh recovery mount request")
    };
    assert_eq!(recovery_mount.request(), request);
    assert_eq!(recovery_mount.parent(), &committed[0]);
    assert_eq!(recovery_mount.outlet(), &RouteSlotKey::default());
    let grant = engine.resolve_mount(recovery_mount).unwrap();
    let recovery_slot = grant.slot();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(247));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(249));
    engine.complete_mount_registration(completion).unwrap();

    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("confirmed recovery mount must issue the failed child's bound error view")
    };
    let expected_origin = FrameOrigin::RouteError(child_route);
    let expected_target = FrameMountTarget::ChildOutlet {
        parent: committed[0].clone(),
        outlet: RouteSlotKey::default(),
    };
    assert_eq!(recovery.identity().request(), request);
    assert_eq!(recovery.frame_version().slot(), recovery_slot);
    assert_eq!(
        (
            recovery.identity().origin(),
            recovery.identity().branch_index(),
            recovery.view(),
            recovery.mount_target(),
        ),
        (&expected_origin, 1, &child_error_view, &expected_target),
        "mount rejection must select from pending.child_index rather than the parent frame identity",
    );
}

fn entering_boundary_direct_recovery_fixture() -> (
    RouterEngine,
    crate::ids::NavigationRequestId,
    super::FrameTask,
) {
    let items_boundary = ErrorBoundaryKey::named("items-boundary").unwrap();
    let items_error_view = RouteViewKey::new("items-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    builder
        .route_with_options(
            root_route,
            RoutePattern::path("legacy").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("legacy").unwrap()),
        )
        .unwrap();
    let items_route = builder
        .route_with_options(
            root_route,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("items").unwrap())
                .with_error_view(items_boundary, items_error_view.clone()),
        )
        .unwrap();
    builder
        .route_with_options(
            items_route,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/legacy")).unwrap();
    settle_current(&mut engine);

    let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(entering_boundary) = engine.action().unwrap() else {
        panic!("replace must issue the entering boundary task")
    };
    engine
        .accept_applied_report(entering_boundary.into_applied_report(vec![RouteSlotKey::default()]))
        .unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("entering boundary must request its child mount")
    };
    let grant = engine.resolve_mount(mount).unwrap();
    let receipt = grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0xa11));
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    let completion = permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0xa12));
    engine.complete_mount_registration(completion).unwrap();
    let EngineAction::FrameTask(failing_leaf) = engine.action().unwrap() else {
        panic!("confirmed child mount must issue the leaf task")
    };
    let failure = engine
        .begin_view_report(failing_leaf.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "entering boundary child apply failed before direct recovery",
        ))
        .expect("leaf failure must enter cleanup");
    let obligation_count = failure.logical_roots().len();
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xa13, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("settled failure must issue the entering boundary's direct error task")
    };
    assert_eq!(
        recovery.identity().origin(),
        &FrameOrigin::RouteError(items_route)
    );
    assert_eq!(recovery.identity().branch_index(), 1);
    assert_eq!(recovery.view(), &items_error_view);
    (engine, request, recovery)
}

fn retained_bound_error_cleanup_fixture(
    exhaust_before_failure: Option<ReservationCounter>,
) -> (
    RouterEngine,
    crate::ids::NavigationRequestId,
    crate::ids::NavigationRequestId,
    RouteFrameVersion,
    Vec<RouteFrameVersion>,
    Vec<LogicalChildSlotBinding>,
    FailureCleanupReceiverProjection,
    CleanupBatchToken,
) {
    let boundary = ErrorBoundaryKey::named("items-boundary").unwrap();
    let error_view = RouteViewKey::new("items-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    builder
        .route_with_options(
            root,
            RoutePattern::path("legacy").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("legacy").unwrap()),
        )
        .unwrap();
    let section = builder
        .route_with_options(
            root,
            RoutePattern::path("section").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("section").unwrap()),
        )
        .unwrap();
    let items = builder
        .route_with_options(
            section,
            RoutePattern::path("items").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("items").unwrap())
                .with_error_view(boundary, error_view),
        )
        .unwrap();
    builder
        .route_with_options(
            items,
            RoutePattern::path(":id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("leaf").unwrap()),
        )
        .unwrap();

    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/legacy")).unwrap();
    settle_current(&mut engine);
    let old_frames = engine.active().frames().to_vec();
    let old_slots = engine.active_child_slots.clone();

    let request = engine
        .replace(RouteLocation::new("/section/items/1"))
        .unwrap();
    let next = engine
        .enqueue_push(RouteLocation::new("/section/items/2"))
        .unwrap();
    let EngineAction::FrameTask(section_task) = engine.action().unwrap() else {
        panic!("the replacement starts with the entering section")
    };
    let retained_section = section_task.frame_version().clone();
    engine
        .accept_applied_report(section_task.into_applied_report(vec![RouteSlotKey::default()]))
        .unwrap();
    let EngineAction::MountRequest(items_mount) = engine.action().unwrap() else {
        panic!("the entering items boundary requires a child mount")
    };
    let items_grant = engine.resolve_mount(items_mount).unwrap();
    let items_receipt =
        items_grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0xa31));
    let items_permit = engine.accept_mount_receipt(items_receipt).unwrap();
    engine
        .complete_mount_registration(
            items_permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0xa32)),
        )
        .unwrap();
    let EngineAction::FrameTask(items_task) = engine.action().unwrap() else {
        panic!("the confirmed child mount issues the items boundary task")
    };
    engine
        .accept_applied_report(items_task.into_applied_report(vec![RouteSlotKey::default()]))
        .unwrap();
    let EngineAction::MountRequest(leaf_mount) = engine.action().unwrap() else {
        panic!("the entering leaf requires the boundary outlet")
    };
    let leaf_grant = engine.resolve_mount(leaf_mount).unwrap();
    let leaf_receipt =
        leaf_grant.attest_receipt(crate::mount::MountReceiptDigest::from_opaque(0xa33));
    let leaf_permit = engine.accept_mount_receipt(leaf_receipt).unwrap();
    engine
        .complete_mount_registration(
            leaf_permit.complete(crate::mount::MountRegistrationDigest::from_opaque(0xa34)),
        )
        .unwrap();
    let EngineAction::FrameTask(leaf_task) = engine.action().unwrap() else {
        panic!("the confirmed boundary outlet issues the leaf task")
    };
    if let Some(counter) = exhaust_before_failure {
        engine.testing_exhaust_reservation_counter(counter);
    }
    let first_failure = engine
        .begin_view_report(leaf_task.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "leaf apply failed after retaining the section prefix",
        ))
        .expect("the leaf failure enters canonical cleanup");
    if exhaust_before_failure.is_none() {
        assert_eq!(
            first_failure.retained_prepared_prefix(),
            &[retained_section.clone()],
        );
        assert!(!first_failure.logical_roots().contains(&retained_section));
    }
    let first_receiver = first_failure.receiver.clone();
    let first_count = first_failure.logical_roots().len();
    let first_cleanup = prepare_failure_cleanup(&mut engine, first_failure, 0xa35, first_count);

    (
        engine,
        request,
        next,
        retained_section,
        old_frames,
        old_slots,
        first_receiver,
        first_cleanup,
    )
}

#[test]
fn retained_bound_error_prefix_is_cleaned_before_source_drift_terminalizes() {
    let (
        mut engine,
        request,
        next,
        retained_section,
        _old_frames,
        _old_slots,
        _first_receiver,
        first_cleanup,
    ) = retained_bound_error_cleanup_fixture(None);
    let active_before_drift = engine.state.active().clone();

    engine.testing_set_active_generation(RouteGeneration::testing_max());
    let owner = engine
        .prepare_failure_cleanup_completion(valid_completion(&first_cleanup))
        .expect("the exact first cleanup completion must retain its Core owner");
    let disposition = engine
        .complete_prepared_failure_cleanup_with_ledger(owner, (), |()| {})
        .expect("source drift must hand the retained prefix to another cleanup");
    assert_eq!(
        disposition,
        FailureCleanupCompletionDisposition::RetainedPreparedPrefixCleanup,
    );

    let Some(EngineAction::FailureCleanup(second_failure)) = engine.action() else {
        panic!("source drift must clean the retained prefix before terminalizing")
    };
    assert_eq!(second_failure.request(), request);
    assert_eq!(second_failure.logical_roots(), &[retained_section]);
    assert!(second_failure.retained_prepared_prefix().is_empty());
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued,
    );
    engine.state.set_active(active_before_drift);
    let second_count = second_failure.logical_roots().len();
    let second_cleanup = prepare_failure_cleanup(&mut engine, second_failure, 0xa36, second_count);
    engine
        .complete_cleanup(valid_completion(&second_cleanup))
        .unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert!(
        matches!(
            engine.request(next).unwrap().status(),
            RequestStatus::Running(_)
        ),
        "queued status after retained-prefix cleanup: {:?}",
        engine.request(next).unwrap().status()
    );
}

#[test]
fn bound_error_publication_rebuilds_child_slots_without_stale_baseline_entries() {
    let (
        mut engine,
        request,
        _next,
        retained_section,
        old_frames,
        old_slots,
        _first_receiver,
        first_cleanup,
    ) = retained_bound_error_cleanup_fixture(None);
    assert_eq!(old_frames.len(), 2);
    assert_eq!(old_slots.len(), 1);
    engine
        .complete_cleanup(valid_completion(&first_cleanup))
        .unwrap();
    let Some(EngineAction::FrameTask(recovery)) = engine.action() else {
        panic!("settled cleanup must issue the deep bound-error task")
    };

    let cleanup = publish_action(&mut engine, EngineAction::FrameTask(recovery));
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let active = engine.active().frames();
    assert_eq!(active.len(), 3);
    assert_eq!(active[0], old_frames[0]);
    assert_eq!(active[1], retained_section);
    assert!(matches!(active[2].origin(), FrameOrigin::RouteError(_)));
    assert_ne!(active[1], old_frames[1]);
    assert_ne!(active[2], old_frames[1]);
    assert_eq!(engine.active_child_slots.len(), 2);
    let root_slot = engine
        .active_child_slots
        .iter()
        .find(|slot| slot.parent_branch_index() == 0)
        .expect("the exact root-to-section binding remains");
    assert_eq!(root_slot.parent(), &active[0]);
    assert_eq!(root_slot.child(), Some(&active[1]));
    assert_eq!(root_slot.slot(), active[1].slot());
    let error_slot = engine
        .active_child_slots
        .iter()
        .find(|slot| slot.parent_branch_index() == 1)
        .expect("the exact section-to-error binding is installed");
    assert_eq!(error_slot.parent(), &active[1]);
    assert_eq!(error_slot.child(), Some(&active[2]));
    assert_eq!(error_slot.slot(), active[2].slot());
    assert!(
        engine
            .active_child_slots
            .iter()
            .all(|slot| slot.child() != Some(&old_frames[1]))
    );
    assert_ne!(root_slot.slot(), error_slot.slot());
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::Apply
    ));
}

#[test]
fn empty_prefix_bound_error_attempt_identity_exhaustion_retains_terminal_evidence() {
    let counter = ReservationCounter::Transaction;
    let boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("root-view").unwrap())
                .with_error_view(boundary, RouteViewKey::new("root-error-view").unwrap()),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(routes.build().unwrap());
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let next = engine.enqueue_push(RouteLocation::new("/items/2")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("the matched root route must issue one frame task")
    };
    engine.testing_exhaust_reservation_counter(counter);
    let failure = engine
        .begin_view_report(task.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "root output failed before bound-error recovery",
        ))
        .expect("the failed root output enters canonical cleanup");

    let FailureCleanupRecoveryTargetProjection::BoundErrorIdentityExhausted(projection) =
        &failure.receiver.recovery
    else {
        panic!("empty-prefix recovery must retain typed identity exhaustion evidence")
    };
    assert!(projection.prepared_prefix.is_empty());
    assert_eq!(
        projection.original_failure.kind(),
        NavigationFailureKind::Apply
    );
    assert_eq!(
        projection.terminal_failure.kind(),
        NavigationFailureKind::RecoveryFailed
    );
    assert_eq!(projection.exhaustion.counter, counter);
    assert_eq!(projection.exhaustion.current, u64::MAX);
    assert_eq!(projection.exhaustion.attempted, u64::MAX);
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );

    let obligation_count = failure.logical_roots().len();
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xa29, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert!(matches!(
        engine.request(next).unwrap().status(),
        RequestStatus::Running(_)
    ));
}

#[test]
fn retained_bound_error_attempt_identity_exhaustion_terminalizes_recovery_failed() {
    for counter in [
        ReservationCounter::Transaction,
        ReservationCounter::FrameTask,
        ReservationCounter::CandidateFrame,
        ReservationCounter::CleanupBatch,
    ] {
        let (mut engine, request, next, retained_section, _, _, receiver, cleanup) =
            retained_bound_error_cleanup_fixture(Some(counter));

        assert!(receiver.logical_roots.contains(&retained_section));
        let FailureCleanupRecoveryTargetProjection::BoundErrorIdentityExhausted(projection) =
            &receiver.recovery
        else {
            panic!("the receiver must retain exact bound-error exhaustion evidence")
        };
        assert_eq!(projection.prepared_prefix, &[retained_section]);
        assert_eq!(
            projection.original_failure.kind(),
            NavigationFailureKind::Apply
        );
        assert_eq!(
            projection.terminal_failure.kind(),
            NavigationFailureKind::RecoveryFailed
        );
        assert_eq!(projection.exhaustion.counter, counter);
        assert_eq!(projection.exhaustion.current, u64::MAX);
        assert_eq!(projection.exhaustion.attempted, u64::MAX);
        let Some(WorkStage::FailureCleanupPrepared { failure, .. }) =
            engine.current.as_ref().map(|current| &current.stage)
        else {
            panic!("identity exhaustion must retain one prepared cleanup owner")
        };
        assert_eq!(failure.kind(), NavigationFailureKind::RecoveryFailed);
        assert_eq!(
            engine.request(next).unwrap().status(),
            &RequestStatus::Queued
        );

        engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

        assert!(matches!(
            engine.request(request).unwrap().status(),
            RequestStatus::Failed(failure)
                if failure.kind() == NavigationFailureKind::RecoveryFailed
        ));
        assert!(matches!(
            engine.request(next).unwrap().status(),
            RequestStatus::Running(_)
        ));
    }
}

#[test]
fn bound_error_direct_task_refreezes_active_source_before_applied_report() {
    let (mut engine, request, recovery) = entering_boundary_direct_recovery_fixture();
    let history_before = engine.history().clone();
    let published_before = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        .count();

    engine.testing_set_active_generation(RouteGeneration::testing_max());
    assert_eq!(
        engine.accept_applied_report(recovery.into_applied_report(Vec::new())),
        Err(ReportError::Stale),
        "the direct bound-error task must refreeze its active source before accepting applied output",
    );
    assert_eq!(engine.history(), &history_before);
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
            .count(),
        published_before,
    );

    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!("stale bound-error output must clean the recovery attempt before terminalizing")
    };
    assert_eq!(failure.request(), request);
    let obligation_count = failure.logical_roots().len();
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xa14, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(engine.history(), &history_before);
    assert!(engine.action().is_none());
}

fn child_mount_required_recovery_fixture() -> (
    RouterEngine,
    crate::ids::NavigationRequestId,
    super::FrameMountRequest,
) {
    let root_boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
    let root_error_view = RouteViewKey::new("root-error").unwrap();
    let child_boundary = ErrorBoundaryKey::named("child-boundary").unwrap();
    let child_error_view = RouteViewKey::new("child-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("root").unwrap())
                .with_error_view(root_boundary, root_error_view),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    builder
        .route_with_options(
            root_route,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("item").unwrap())
                .with_error_view(child_boundary, child_error_view),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/")).unwrap();
    settle_current(&mut engine);
    let committed_root = engine.active().frames()[0].clone();

    let request = engine.replace(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::MountRequest(mount) = engine.action().unwrap() else {
        panic!("entering child must request its missing mount")
    };
    let failure = engine
        .reject_mount(mount, crate::trace::MountRejectionKind::MissingOutlet)
        .unwrap();
    let obligation_count = failure.logical_roots().len();
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xa15, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::MountRequest(recovery_mount)) = engine.action() else {
        panic!("settled child mount rejection must issue its recovery mount request")
    };
    assert_eq!(recovery_mount.request(), request);
    assert_eq!(recovery_mount.parent(), &committed_root);
    assert_eq!(recovery_mount.outlet(), &RouteSlotKey::default());
    (engine, request, recovery_mount)
}

#[test]
fn bound_error_mount_required_refreezes_active_source_before_grant() {
    let (mut engine, request, recovery_mount) = child_mount_required_recovery_fixture();
    let source_request = recovery_mount.request();
    let source_parent = recovery_mount.parent().clone();
    let source_outlet = recovery_mount.outlet().clone();
    let ids_before = engine.testing_id_allocator_snapshot();
    let mount_issuer_before = engine.testing_mount_issuer_snapshot();
    let status_before = engine.request(request).unwrap().status().clone();
    let granted_before = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::MountGranted)
        .count();
    let published_before = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        .count();

    engine.testing_set_active_generation(RouteGeneration::testing_max());
    assert_eq!(
        engine.validate_mount_request(&recovery_mount),
        Err(crate::mount::MountError::WrongPhase),
        "a recovery mount must refreeze its active source before granting a slot",
    );
    let error = match engine.resolve_mount(recovery_mount) {
        Ok(_) => panic!("source-drifted recovery mount must not be granted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), crate::mount::MountError::WrongPhase);
    let super::ResolveMountOwner::Returned(returned) = error.into_owner() else {
        panic!("source drift must return the whole recovery mount request")
    };
    assert_eq!(returned.request(), source_request);
    assert_eq!(returned.parent(), &source_parent);
    assert_eq!(returned.outlet(), &source_outlet);
    assert_eq!(engine.testing_id_allocator_snapshot(), ids_before);
    assert_eq!(engine.testing_mount_issuer_snapshot(), mount_issuer_before);
    assert_eq!(engine.request(request).unwrap().status(), &status_before);
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::MountGranted)
            .count(),
        granted_before,
    );
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
            .count(),
        published_before,
    );
    assert!(engine.action().is_none());
}

#[test]
fn deep_bound_error_mount_required_retains_exact_parent_mount_source() {
    let layout_outlet = RouteSlotKey::named("layout-outlet").unwrap();
    let leaf_outlet = RouteSlotKey::named("leaf-outlet").unwrap();
    assert_ne!(layout_outlet, leaf_outlet);
    let leaf_boundary = ErrorBoundaryKey::named("leaf-boundary").unwrap();
    let leaf_error_view = RouteViewKey::new("leaf-error").unwrap();
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap();
    let root_route = builder.root_id().unwrap();
    let layout_route = builder
        .route_with_options(
            root_route,
            RoutePattern::path("layout").unwrap(),
            RouteNodeOptions::new()
                .with_slot(layout_outlet.clone())
                .with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    builder
        .route_with_options(
            layout_route,
            RoutePattern::path("leaf").unwrap(),
            RouteNodeOptions::new()
                .with_slot(leaf_outlet.clone())
                .with_view(RouteViewKey::new("leaf").unwrap())
                .with_error_view(leaf_boundary, leaf_error_view),
        )
        .unwrap();
    let mut engine = ready_engine_with_tree(builder.build().unwrap());
    engine.push(RouteLocation::new("/layout")).unwrap();
    settle_current(&mut engine);
    let committed = engine.active().frames().to_vec();
    assert_eq!(committed.len(), 2);
    assert!(engine.active_child_slots.iter().any(|slot| {
        slot.parent_branch_index() == 0
            && slot.parent() == &committed[0]
            && slot.outlet() == &layout_outlet
            && slot.child() == Some(&committed[1])
    }));

    let request = engine.push(RouteLocation::new("/layout/leaf")).unwrap();
    let EngineAction::MountRequest(failed_mount) = engine.action().unwrap() else {
        panic!("the entering leaf must request its distinct child outlet")
    };
    assert_eq!(failed_mount.request(), request);
    assert_eq!(failed_mount.parent(), &committed[1]);
    assert_eq!(failed_mount.outlet(), &leaf_outlet);
    let failure = engine
        .reject_mount(
            failed_mount,
            crate::trace::MountRejectionKind::MissingOutlet,
        )
        .unwrap();
    let obligation_count = failure.logical_roots().len();
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xa21, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let Some(EngineAction::MountRequest(recovery_mount)) = engine.action() else {
        panic!("the failed leaf boundary must issue a recovery mount request")
    };
    assert_eq!(recovery_mount.request(), request);
    assert_eq!(recovery_mount.parent(), &committed[1]);
    assert_eq!(recovery_mount.outlet(), &leaf_outlet);
    let current = engine
        .current
        .as_ref()
        .expect("the recovery mount retains its parent task source");
    assert_eq!(current.identity.branch_index(), 1);
    assert_eq!(
        current.current_target,
        FrameMountTarget::RegisteredChildSlot {
            parent: committed[0].clone(),
            outlet: layout_outlet,
        },
        "the layout parent task must retain the layout's original mount source, not borrow the leaf boundary outlet",
    );
    let pending = current
        .pending_mount
        .as_ref()
        .expect("the recovery mount remains pending");
    assert_eq!(pending.child_index, 2);
    assert_eq!(pending.parent, committed[1]);
    assert_eq!(pending.outlet, leaf_outlet);
}

#[test]
fn bound_error_mount_slot_exhaustion_is_bounded_and_starts_queue() {
    let (mut engine, request, recovery_mount) = child_mount_required_recovery_fixture();
    let next = engine.enqueue_push(RouteLocation::new("/items/2")).unwrap();
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    engine.testing_exhaust_slot_counter();
    let ids_before_resolution = engine.testing_id_allocator_snapshot();
    let granted_before = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::MountGranted)
        .count();
    let published_before = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
        .count();

    assert_eq!(
        engine
            .resolve_mount(recovery_mount)
            .map(|_| ())
            .map_err(|error| error.kind()),
        Err(crate::mount::MountError::Exhausted),
    );
    assert_eq!(
        engine.testing_id_allocator_snapshot(),
        ids_before_resolution,
        "slot exhaustion must not fabricate or advance a slot identity",
    );
    let Some(EngineAction::FailureCleanup(failure)) = engine.action() else {
        panic!(
            "bound-error slot exhaustion must fail the recovery attempt, not reissue its mount request"
        )
    };
    assert_eq!(failure.request(), request);
    assert_eq!(
        failure.failure().kind(),
        NavigationFailureKind::RecoveryFailed
    );
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    assert_eq!(
        engine.request(next).unwrap().status(),
        &RequestStatus::Queued
    );
    let obligation_count = failure.logical_roots().len();
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 0xa22, obligation_count);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert!(matches!(
        engine.request(next).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert_eq!(
        engine
            .current
            .as_ref()
            .expect("the queued request must start after recovery cleanup")
            .identity
            .request,
        next,
    );
    assert!(engine.action().is_some());
    let trace = engine.request(request).unwrap().trace();
    assert_eq!(
        trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::MountGranted)
            .count(),
        granted_before,
    );
    assert_eq!(
        trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
            .count(),
        published_before,
    );
}

#[test]
fn root_partial_failure_waits_for_cleanup_completion_before_fallback() {
    let mut engine = ready_engine();
    let request = engine.push(RouteLocation::new("/")).unwrap();
    let EngineAction::FrameTask(root) = engine.action().unwrap() else {
        panic!("root task expected")
    };
    let failure = engine
        .begin_view_report(root.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "partial root apply failed",
        ));

    assert!(failure.logical_roots().is_empty());
    assert!(engine.action().is_none());
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::CleanupPending)
    );
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(Vec::new(), manifest_digest(211), 3),
        )
        .unwrap();
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Running(NavigationPhase::Recovering)
    );
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("cleanup completion must issue the fallback task")
    };
    assert_eq!(fallback.identity().origin(), &FrameOrigin::RouterFallback);
    let trace = engine.request(request).unwrap().trace();
    let settled = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::CleanupSettled)
        .unwrap();
    let fallback_issued = trace
        .iter()
        .rposition(|event| {
            event.kind() == NavigationTraceEventKind::FrameTaskIssued
                && event
                    .frame()
                    .is_some_and(|frame| frame.origin() == &FrameOrigin::RouterFallback)
        })
        .unwrap();
    assert!(settled < fallback_issued);
}

#[test]
fn recovery_attempt_failure_cleans_residual_before_recovery_failed_terminal() {
    let mut engine = ready_engine();
    let history_before = engine.testing_history_authority_snapshot();
    let request = engine.push(RouteLocation::new("/")).unwrap();
    let EngineAction::FrameTask(root) = engine.action().unwrap() else {
        panic!("root task expected")
    };
    let normal_failure = engine
        .begin_view_report(root.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "normal root failed",
        ))
        .expect("normal root failure must enter cleanup before fallback");
    assert!(normal_failure.logical_roots().is_empty());
    assert!(engine.action().is_none());
    let normal_cleanup = prepare_failure_cleanup(&mut engine, normal_failure, 0xb13, 0);
    engine
        .complete_cleanup(valid_completion(&normal_cleanup))
        .unwrap();
    let Some(EngineAction::FrameTask(fallback)) = engine.action() else {
        panic!("normal root failure must issue fallback")
    };
    let failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed_with_physical_residual(NavigationFailure::new(
            NavigationFailureKind::Apply,
            "partial fallback apply failed",
        ));

    assert!(failure.logical_roots().is_empty());
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Running(_)
    ));
    let cleanup = engine
        .prepare_failure_cleanup(
            failure,
            CleanupManifestSeal::new(Vec::new(), manifest_digest(223), 2),
        )
        .unwrap();
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Running(_)
    ));
    assert!(engine.action().is_none());
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert!(engine.action().is_none());
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    let trace = engine.request(request).unwrap().trace();
    let aggregate_cleanup = trace
        .iter()
        .rposition(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_none()
        })
        .expect("recovery residual cleanup must emit its aggregate settlement");
    let terminal = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .expect("recovery failure must publish one terminal event");
    assert!(aggregate_cleanup < terminal);
    assert_eq!(terminal + 1, trace.len());
    assert_eq!(
        trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .count(),
        1
    );
}

#[test]
fn replace_failure_and_fallback_never_publish_history() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let history_before = engine.testing_history_authority_snapshot();

    let request = engine
        .replace(RouteLocation::new("/items/2"))
        .expect("replace must reserve one exact normal attempt");
    let EngineAction::FrameTask(normal) = engine.action().unwrap() else {
        panic!("replace must issue its changed frame task")
    };
    let failure = engine
        .begin_view_report(normal.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "replace build failed",
        ))
        .expect("replace failure must retain exact cleanup authority");
    let cleanup = prepare_failure_cleanup(&mut engine, failure, 227, 0);
    engine.complete_cleanup(valid_completion(&cleanup)).unwrap();

    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("settled replace failure must issue the fallback branch")
    };
    let recovery_failure = engine
        .begin_view_report(fallback.identity())
        .unwrap()
        .failed(NavigationFailure::new(
            NavigationFailureKind::Build,
            "replace fallback failed",
        ))
        .expect("fallback failure must retain cleanup authority before terminal");
    let recovery_cleanup = prepare_failure_cleanup(&mut engine, recovery_failure, 0xE4, 0);
    engine
        .complete_cleanup(valid_completion(&recovery_cleanup))
        .unwrap();

    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::RecoveryFailed
    ));
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);
    assert!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .all(|event| event.kind() != NavigationTraceEventKind::CommitPublished)
    );
}

#[test]
fn replace_publication_updates_active_and_history_once() {
    let mut engine = ready_engine();
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let active_before = engine.active().clone();
    let history_before = engine.testing_history_authority_snapshot();

    let request = engine
        .replace(RouteLocation::new("/items/2"))
        .expect("replace must enqueue");
    let action = engine.action().expect("replace must produce work");
    let ticket = drive_to_commit(&mut engine, action);
    let sealed = seal_current(&mut engine, ticket);
    let prepared = engine.prepare_publication(sealed).unwrap();

    assert_eq!(engine.active(), &active_before);
    assert_eq!(engine.testing_history_authority_snapshot(), history_before);

    engine.publish_prepared(prepared, || {}).unwrap();

    assert_eq!(
        engine.active().location(),
        Some(&RouteLocation::new("/items/2"))
    );
    assert_eq!(
        engine.history().current(),
        Some(&RouteLocation::new("/items/2"))
    );
    let history_after = engine.testing_history_authority_snapshot();
    assert_ne!(history_after.revision(), history_before.revision());
    assert_eq!(history_after.committed_by(), Some(request));
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::CommitPublished)
            .count(),
        1
    );
}

#[test]
fn replace_identity_exhaustion_is_complete_zero_change() {
    for counter in [
        ReservationCounter::Request,
        ReservationCounter::Transaction,
        ReservationCounter::FrameTask,
        ReservationCounter::CandidateFrame,
        ReservationCounter::CleanupBatch,
    ] {
        let mut engine = ready_engine();
        engine.push(RouteLocation::new("/items/1")).unwrap();
        settle_current(&mut engine);
        engine.testing_exhaust_reservation_counter(counter);
        let before = engine.snapshot();

        assert_eq!(
            engine.replace(RouteLocation::new("/items/2")),
            Err(PushError::IdExhausted),
            "counter: {counter:?}"
        );
        assert_eq!(engine.snapshot(), before, "counter: {counter:?}");
    }
}

#[test]
fn back_target_atomic_applied_receiver_never_derives_outcome_from_ambient_action() {
    let source = include_str!("engine.rs");
    let signature = "pub(crate) fn report_back_target_applied(";
    let function_start = source
        .find(signature)
        .expect("the target atomic applied receiver must remain present");
    let body_start = function_start
        + source[function_start..]
            .find('{')
            .expect("the target atomic applied receiver must have one body");
    let mut depth = 0usize;
    let mut body_end = None;
    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = Some(body_start + offset + character.len_utf8());
                    break;
                }
            }
            _ => {}
        }
    }
    let receiver = &source
        [function_start..body_end.expect("the target atomic applied receiver body must close")];

    assert!(
        !receiver.contains("self.action"),
        "the target atomic applied receiver must consume a direct typed producer result, not read ambient action"
    );
    assert!(
        !receiver.contains("EngineAction::"),
        "the target atomic applied receiver must not infer its typed outcome by matching EngineAction"
    );
}

fn publish_retained_post_commit_test_navigation(
    engine: &mut RouterEngine,
    path: &str,
    owners: &mut Vec<super::retained::RetainedLeafOwner>,
) -> CleanupBatchToken {
    engine.push(RouteLocation::new(path)).unwrap();
    let action = engine.action().unwrap();
    let ticket = drive_to_commit(engine, action);
    let plan = engine
        .prepare_retained_commit_plan(&ticket, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let (retiring, preserved) = std::mem::take(owners)
        .into_iter()
        .partition::<Vec<_>, _>(|owner| plan.retiring().contains(owner.projection()));
    *owners = preserved;
    let roots = plan.disposable_roots().to_vec();
    let count = roots.len();
    let (prepared, normal) = engine
        .prepare_retained_publication(
            &ticket,
            &plan,
            CleanupManifestSeal::new(roots, manifest_digest(0x7171), count),
            retiring,
        )
        .unwrap();
    let validated = match engine.validate_retained_publication(prepared) {
        Ok(validated) => validated,
        Err(_) => panic!("the exact retained publication must validate"),
    };
    validated.publish(|retained| owners.extend(retained));
    normal
}

fn prepared_retained_activation() -> (
    RouterEngine,
    PreparedPublication,
    CleanupBatchToken,
    RouteFrameVersion,
    NavigationRequestId,
) {
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap();
    routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("leaf").unwrap())
                .with_keep_alive(true),
        )
        .unwrap();
    let (engine, _, _) = ready_engine_with_tree_and_authority(routes.build().unwrap());
    let mut engine = engine.with_keep_alive_capacity(1);
    let mut owners = Vec::new();
    for path in ["/items/1", "/items/2"] {
        let normal = publish_retained_post_commit_test_navigation(&mut engine, path, &mut owners);
        let current = engine.current.as_ref().unwrap();
        let preserved = current.reconciliation.as_ref().unwrap().preserved().len();
        assert_eq!(current.applied_frames, current.target_frames[preserved..]);
        engine.complete_cleanup(valid_completion(&normal)).unwrap();
    }
    let original = owners[0].projection().frame().clone();
    let request = engine.push(RouteLocation::new("/items/1")).unwrap();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("the retained leaf is selected before its fresh factory runs")
    };
    let selection = engine
        .prepare_instance_selection(&task, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let EngineAction::Commit(ticket) = engine
        .accept_instance_selection(task, &selection)
        .unwrap_or_else(|_| panic!("original selected instance accepts"))
    else {
        panic!("all selected instances commit")
    };
    let plan = engine
        .prepare_retained_commit_plan(&ticket, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    let (prepared, normal) = engine
        .prepare_retained_publication(
            &ticket,
            &plan,
            CleanupManifestSeal::new(Vec::new(), manifest_digest(0x7171), 0),
            std::mem::take(&mut owners),
        )
        .unwrap_or_else(|_| panic!("the original cache owner must prepare"));
    (engine, prepared, normal, original, request)
}

#[test]
fn retained_activation_is_not_a_fresh_creation_but_keeps_activation_trace() {
    let (mut engine, prepared, normal, original, request) = prepared_retained_activation();
    let mut owners = Vec::new();
    let Some(super::PreparedPublicationAuthority::Ready(sealed)) =
        engine.prepared_publication_authority.as_ref()
    else {
        panic!("the exact retained handoff must prepare its independent next")
    };
    assert_eq!(
        sealed.publication.current.target_frames.last(),
        Some(&original)
    );
    assert!(
        sealed.publication.current.applied_frames.is_empty(),
        "retained activation cannot claim a fresh creation or fresh failure root"
    );
    assert_eq!(
        sealed.publication.current.selected_retained_frames,
        vec![original.clone()]
    );
    assert!(sealed.publication.current.cleanup_roots.is_empty());
    engine
        .validate_retained_publication(prepared)
        .unwrap_or_else(|_| panic!("the exact retained publication must validate"))
        .publish(|retained| owners.extend(retained));
    assert!(engine.current.as_ref().unwrap().applied_frames.is_empty());
    assert_eq!(engine.active().frames().last(), Some(&original));
    let trace = engine.request(request).unwrap().trace();
    let activated = trace
        .iter()
        .filter(|event| event.kind() == NavigationTraceEventKind::FrameActivated)
        .map(|event| event.frame().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(activated, vec![&original]);
    assert!(
        !trace
            .iter()
            .any(|event| event.kind() == NavigationTraceEventKind::ViewApplied)
    );
    engine.complete_cleanup(valid_completion(&normal)).unwrap();
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Succeeded
    );
}

#[test]
fn retained_publication_rejects_creation_source_drift_and_retains_original_owner() {
    for drift in ["retained-selection", "fresh-creation"] {
        let (mut engine, prepared, _, original, _) = prepared_retained_activation();
        let active = engine.active().clone();
        let history = engine.history_snapshot();
        let evidence = prepared.evidence.clone();
        let original_owners = engine.testing_retained_transfer_owners();
        let current = engine.current.as_mut().unwrap();
        assert_eq!(current.selected_retained_frames, vec![original.clone()]);
        match drift {
            "retained-selection" => current.selected_retained_frames.clear(),
            "fresh-creation" => current.applied_frames.push(original.clone()),
            _ => unreachable!(),
        }
        let (returned, error) = match engine.validate_retained_publication(prepared) {
            Err(error) => error,
            Ok(_) => panic!("creation-source drift must invalidate the frozen publication"),
        };
        assert_eq!(error, CommitError::Stale);
        assert_eq!(engine.active(), &active);
        assert_eq!(engine.history_snapshot(), history);
        assert_eq!(returned.evidence, evidence);
        assert!(matches!(
            engine.prepared_publication_authority,
            Some(PreparedPublicationAuthority::Rejected(_))
        ));
        // A rejected sealed publication retains its original transfer for
        // authority settlement; it must not recreate or silently drop owners.
        assert_eq!(engine.testing_retained_transfer_owners(), original_owners);
        assert_eq!(original_owners.len(), 1);
        assert_eq!(original_owners[0].frame(), &original);
    }
}

#[test]
fn post_commit_retained_transfer_preserves_both_batch_bindings_and_returns_source_owners() {
    let mut routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("retained-root").unwrap()),
        )
        .unwrap();
    let leaf = routes
        .route_with_options(
            routes.root_id().unwrap(),
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("retained-leaf").unwrap())
                .with_keep_alive(true),
        )
        .unwrap();
    routes
        .route_with_options(
            leaf,
            RoutePattern::path("detail").unwrap(),
            RouteNodeOptions::new()
                .with_view(RouteViewKey::new("retained-detail").unwrap())
                .with_keep_alive(true),
        )
        .unwrap();
    let (engine, mut issuer, lease) = ready_engine_with_tree_and_authority(routes.build().unwrap());
    let mut engine = engine.with_keep_alive_capacity(2);
    let mut owners = Vec::new();
    for path in ["/items/1/detail", "/items/1"] {
        let normal = publish_retained_post_commit_test_navigation(&mut engine, path, &mut owners);
        engine.complete_cleanup(valid_completion(&normal)).unwrap();
    }
    assert_eq!(owners.len(), 1);
    let original_child = owners[0].projection().clone();
    assert_eq!(
        original_child.parent(),
        engine.active().frames().last().unwrap()
    );
    // Independent parent retention no longer retires this child by itself.
    // A real capacity reduction exercises the normal-token retirement half.
    engine = engine.with_keep_alive_capacity(1);
    let normal = publish_retained_post_commit_test_navigation(&mut engine, "/items/2", &mut owners);
    assert_eq!(
        owners.len(),
        1,
        "capacity retires the older child while the departing parent keeps its owner"
    );
    assert_eq!(owners[0].projection().frame(), original_child.parent());
    assert_eq!(
        engine.retained_leaves.len(),
        2,
        "the original normal token also owns a retiring child"
    );
    let original = owners[0].projection().clone();
    let original_retiring = engine
        .retained_leaves
        .iter()
        .find(|entry| !entry.is_live())
        .unwrap()
        .projection()
        .frame()
        .clone();
    assert_eq!(&original_retiring, original_child.frame());
    assert_eq!(
        engine
            .retained_leaves
            .iter()
            .find(|entry| entry.projection().frame() == &original_retiring)
            .unwrap()
            .retirement(),
        Some(&normal)
    );
    let history = engine.history_snapshot();
    let request = normal.identity().request();
    let accepted = match engine.accept_post_commit_authority_transfer(
        &normal,
        PostCommitAuthorityCause::HostRegistrationLost,
    ) {
        PostCommitAuthorityAcceptanceOutcome::Accepted(owner) => owner,
        _ => panic!("the published source must remain accepted with mixed inventory"),
    };
    let source = CorePostCommitAuthorityPreparationSource::new(accepted, lease);
    let plan = engine
        .prepare_retained_post_commit_authority_plan(&source, &owners.iter().collect::<Vec<_>>())
        .unwrap();
    assert_eq!(plan.retiring(), &[original.clone()]);
    let roots = plan.logical_roots().to_vec();
    let count = roots.len() + 1;
    let manifest = CleanupManifestSeal::new(roots.clone(), manifest_digest(0x7272), count);
    let location = engine.current.as_ref().unwrap().location.clone();
    engine.current.as_mut().unwrap().location = RouteLocation::new("/changed-source");
    let source = match engine
        .prepare_post_commit_authority_transfer(source.with_retention(plan, owners), manifest)
    {
        PostCommitAuthorityPreparationOutcome::Permanent { source, .. } => source,
        _ => panic!("real source drift must return the original source and owners"),
    };
    assert_eq!(source.retained_logical_roots(), Some(roots.as_slice()));
    assert_eq!(
        source.transfer.retained.as_ref().unwrap().owners[0].projection(),
        &original,
    );
    assert_eq!(engine.history_snapshot(), history);
    engine.current.as_mut().unwrap().location = location;
    let (prepared, authority) = match engine.prepare_post_commit_authority_transfer(
        source,
        CleanupManifestSeal::new(roots, manifest_digest(0x7272), count),
    ) {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, batch } => (owner, batch),
        _ => panic!("retry borrows the original source attachment without reminting a plan"),
    };
    let mut installed = None;
    assert!(matches!(
        engine.begin_post_commit_authority_transfer(
            &mut issuer,
            prepared,
            (),
            |(), _, builder| {
                let prior = builder.current_trace.len();
                let (plan, physical) =
                    bind_test_settlement_plan_with_authority(builder, prior, Vec::new());
                Ok::<_, ((), ())>((physical, plan))
            },
            |physical, batch| installed = Some((batch, physical)),
        ),
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    assert_eq!(engine.retained_leaves.len(), 2);
    let (batch, physical) = installed.unwrap();
    assert_eq!(
        engine
            .retained_leaves
            .iter()
            .find(|entry| entry.projection().frame() == &original_retiring)
            .unwrap()
            .retirement(),
        Some(&normal)
    );
    assert_eq!(
        engine
            .retained_leaves
            .iter()
            .find(|entry| entry.projection() == &original)
            .unwrap()
            .retirement(),
        Some(&batch.authority_batch)
    );
    let (batch, _) = record_exact_post_commit_settlement_trace(&mut engine, batch, &physical);
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) =
        engine.prepare_post_commit_compound_completion(batch, inputs)
    else {
        panic!("both original receipts must prepare their exact joint completion")
    };
    assert!(matches!(
        engine.complete_post_commit_authority_compound(owner, |_| ()),
        PostCommitCompoundCompletionCoreOutcome::Terminal { .. }
    ));
    assert!(
        engine.retained_leaves.is_empty(),
        "each receipt clears only its own retirement metadata"
    );
    assert_eq!(engine.history_snapshot(), history);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert_eq!(
        engine.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert!(engine.current.is_none() && engine.action.is_none());
}

fn prepared_post_commit_authority_fixture_for_cause(
    cause: PostCommitAuthorityCause,
) -> (
    RouterEngine,
    HostAdmissionIssuer,
    NavigationRequestId,
    CleanupBatchToken,
    CorePreparedPostCommitAuthorityTransfer,
    CleanupBatchProjection,
) {
    prepared_post_commit_authority_fixture_for_location(cause, RouteLocation::new("/"))
}

fn prepared_post_commit_authority_fixture_for_location(
    cause: PostCommitAuthorityCause,
    location: RouteLocation,
) -> (
    RouterEngine,
    HostAdmissionIssuer,
    NavigationRequestId,
    CleanupBatchToken,
    CorePreparedPostCommitAuthorityTransfer,
    CleanupBatchProjection,
) {
    let (mut engine, issuer, lease) = ready_engine_with_tree_and_authority(tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let request = engine.push(location).unwrap();
    let normal = publish_current(&mut engine);
    assert!(normal.obligation_count() > 0);
    let accepted = match engine.accept_post_commit_authority_transfer(&normal, cause) {
        PostCommitAuthorityAcceptanceOutcome::Accepted(owner) => owner,
        PostCommitAuthorityAcceptanceOutcome::Mismatch { .. } => {
            panic!("the real post-commit receiver must be accepted")
        }
    };
    let source = CorePostCommitAuthorityPreparationSource::new(accepted, lease);
    let prepared = match engine.prepare_post_commit_authority_transfer(
        source,
        CleanupManifestSeal::new(
            engine.active().frames().iter().rev().cloned().collect(),
            manifest_digest(0xa0),
            engine.active().frames().len(),
        ),
    ) {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, batch } => (owner, batch),
        PostCommitAuthorityPreparationOutcome::Mismatch { .. }
        | PostCommitAuthorityPreparationOutcome::Retryable { .. }
        | PostCommitAuthorityPreparationOutcome::Permanent { .. } => {
            panic!("the exact accepted source must prepare")
        }
    };
    (engine, issuer, request, normal, prepared.0, prepared.1)
}

fn prepared_post_commit_authority_fixture() -> (
    RouterEngine,
    HostAdmissionIssuer,
    NavigationRequestId,
    CleanupBatchToken,
    CorePreparedPostCommitAuthorityTransfer,
    CleanupBatchProjection,
) {
    prepared_post_commit_authority_fixture_for_cause(PostCommitAuthorityCause::HostEntityMissing)
}

#[test]
fn published_fallback_post_commit_host_loss_preserves_original_failure() {
    let (mut engine, mut issuer, request, normal, prepared, authority) =
        prepared_post_commit_authority_fixture_for_location(
            PostCommitAuthorityCause::HostEntityMissing,
            RouteLocation::new("/unmatched"),
        );
    let AttemptKind::Fallback {
        original_failure, ..
    } = &engine.current.as_ref().unwrap().attempt
    else {
        panic!("real fallback publication")
    };
    let expected = RequestStatus::Failed(original_failure.clone());
    let history = engine.history_snapshot();
    assert_eq!(engine.active().kind(), ActivePresentationKind::Error);
    let queued = engine.push(RouteLocation::new("/items/2")).unwrap();
    let mut installed = None;
    assert!(matches!(
        engine.begin_post_commit_authority_transfer(
            &mut issuer,
            prepared,
            (),
            |(), _, plan| {
                let prior_trace_start = plan.current_trace.len();
                let (plan, physical) =
                    bind_test_settlement_plan_with_authority(plan, prior_trace_start, Vec::new());
                Ok::<_, ((), ())>((physical, plan))
            },
            |physical, batch| installed = Some((batch, physical)),
        ),
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    let (batch, physical) = installed.unwrap();
    let (batch, _) = record_exact_post_commit_settlement_trace(&mut engine, batch, &physical);
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) =
        engine.prepare_post_commit_compound_completion(batch, inputs)
    else {
        panic!("exact completion prepares")
    };
    assert!(matches!(
        engine.complete_post_commit_authority_compound(owner, |_| ()),
        PostCommitCompoundCompletionCoreOutcome::Terminal { .. }
    ));
    assert_eq!(engine.history_snapshot(), history);
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert_eq!(
        engine.availability(),
        &RouterAvailability::Unavailable(RouterUnavailableReason::HostLost)
    );
    assert!(engine.current.is_none() && engine.action.is_none());
    assert!(
        engine.pending_post_commit_terminal.is_none()
            && engine.pending_post_commit_settlement_trace.is_none()
    );
    assert!(!engine.testing_history_capability_snapshot().has_owner());
    assert!(
        matches!(engine.request(queued).unwrap().status(), RequestStatus::Failed(failure) if failure.kind() == NavigationFailureKind::HostLost)
    );
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .count(),
        1
    );
    assert_eq!(engine.request(request).unwrap().status(), &expected);
}

#[test]
fn post_commit_diagnostic_prepare_and_refresh_retire_only_exact_report_events() {
    let (mut engine, mut issuer, _request, normal, prepared, authority) =
        prepared_post_commit_authority_fixture();
    let mut sources = Vec::new();
    for _ in 0..=ROUTER_DIAGNOSTIC_RETENTION {
        let mut source = ready_engine();
        source.push(RouteLocation::new("/items/1")).unwrap();
        let EngineAction::FrameTask(task) = source.action().unwrap() else {
            panic!("real foreign task")
        };
        sources.push((source, task));
    }
    for (_, task) in sources.iter().take(ROUTER_DIAGNOSTIC_RETENTION) {
        assert_eq!(
            engine.validate_view_report_identity(task.identity()),
            Err(ReportError::Invalid)
        );
    }
    let mut installed = None;
    assert!(matches!(
        engine.begin_post_commit_authority_transfer(
            &mut issuer,
            prepared,
            (),
            |(), _, plan| {
                let prior_trace_start = plan.current_trace.len();
                let (plan, physical) =
                    bind_test_settlement_plan_with_authority(plan, prior_trace_start, Vec::new());
                Ok::<_, ((), ())>((physical, plan))
            },
            |physical, batch| installed = Some((batch, physical)),
        ),
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    let terminal = engine.pending_post_commit_terminal.as_ref().unwrap();
    assert_eq!(
        terminal.final_router_diagnostics.len(),
        ROUTER_DIAGNOSTIC_RETENTION
    );
    assert!(
        !terminal
            .final_report_dispositions
            .iter()
            .any(|record| record.identity == *sources[0].1.identity())
    );
    assert!(
        terminal
            .final_report_dispositions
            .iter()
            .any(|record| record.identity == *sources[1].1.identity())
    );
    // A genuine rejected report arriving during compound settlement changes the
    // live retained log; completion must refresh both events and their dedup.
    assert_eq!(
        engine.validate_view_report_identity(sources.last().unwrap().1.identity()),
        Err(ReportError::Invalid)
    );
    let (batch, physical) = installed.unwrap();
    let (batch, _) = record_exact_post_commit_settlement_trace(&mut engine, batch, &physical);
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) =
        engine.prepare_post_commit_compound_completion(batch, inputs)
    else {
        panic!("exact completion prepares")
    };
    assert!(matches!(
        engine.complete_post_commit_authority_compound(owner, |_| ()),
        PostCommitCompoundCompletionCoreOutcome::Terminal { .. }
    ));
    assert_eq!(
        engine.router_diagnostics().len(),
        ROUTER_DIAGNOSTIC_RETENTION
    );
    for (_, task) in sources.iter().take(2) {
        assert!(
            !engine
                .report_dispositions
                .iter()
                .any(|record| record.identity == *task.identity())
        );
    }
    let before = engine.router_diagnostics().to_vec();
    assert_eq!(
        engine.validate_view_report_identity(sources.last().unwrap().1.identity()),
        Err(ReportError::Invalid)
    );
    assert_eq!(engine.router_diagnostics(), before);
    assert_eq!(
        engine.validate_view_report_identity(sources[0].1.identity()),
        Err(ReportError::Invalid)
    );
    assert_eq!(
        engine.router_diagnostics.last().unwrap().request(),
        Some(sources[0].1.identity().request())
    );
    assert!(engine.report_dispositions.iter().all(|record| {
        match record.target {
            DiagnosticTarget::Router(index) => {
                engine.router_diagnostics.get(index).is_some_and(|event| {
                    event.request() == Some(record.identity.request())
                        && event.report_disposition() == Some(record.disposition)
                })
            }
            DiagnosticTarget::Request(_) => true,
        }
    }));
}

fn settlement_key(namespace: u64, identity: u64) -> CorePostCommitSettlementKeyProjection {
    CorePostCommitSettlementKeyProjection::new(namespace, vec![identity])
}

fn bind_exact_test_settlement_plan(
    builder: CorePostCommitSettlementPlanBuilder,
) -> super::CoreBoundPostCommitSettlementPlan {
    let prior_trace_start = builder.current_trace.len();
    bind_test_settlement_plan(builder, prior_trace_start, Vec::new())
}

fn bind_test_settlement_plan(
    builder: CorePostCommitSettlementPlanBuilder,
    prior_trace_start: usize,
    settled_original: Vec<super::CorePostCommitSettledOriginalProjection>,
) -> super::CoreBoundPostCommitSettlementPlan {
    let descriptor = test_settlement_plan_descriptor(&builder, prior_trace_start, settled_original);
    builder
        .bind(descriptor)
        .expect("the explicit test settlement plan must bind its exact source")
}

fn bind_test_settlement_plan_with_authority(
    builder: CorePostCommitSettlementPlanBuilder,
    prior_trace_start: usize,
    settled_original: Vec<super::CorePostCommitSettledOriginalProjection>,
) -> (
    super::CoreBoundPostCommitSettlementPlan,
    TestPostCommitPhysicalAuthority,
) {
    let descriptor = test_settlement_plan_descriptor(&builder, prior_trace_start, settled_original);
    let physical_authority = TestPostCommitPhysicalAuthority::from_descriptor(&descriptor);
    let plan = builder
        .bind(descriptor)
        .expect("the independently frozen physical source must bind the exact Core plan");
    (plan, physical_authority)
}

fn test_settlement_plan_descriptor(
    builder: &CorePostCommitSettlementPlanBuilder,
    prior_trace_start: usize,
    settled_original: Vec<super::CorePostCommitSettledOriginalProjection>,
) -> CorePostCommitSettlementPlanDescriptor {
    let residual_original = (settled_original.len()..builder.normal_obligation_count)
        .map(|ordinal| {
            CorePostCommitPlannedCleanupProjection::new(
                settlement_key(1, 100 + ordinal as u64),
                settlement_key(11, 1_000 + ordinal as u64),
                crate::trace::CleanupDisposition::Removed,
            )
        })
        .collect();
    let authority_non_top = (0..builder.authority_obligation_count.saturating_sub(1))
        .map(|ordinal| {
            CorePostCommitPlannedCleanupProjection::new(
                settlement_key(2, 900 - ordinal as u64),
                settlement_key(12, 2_000 + ordinal as u64),
                crate::trace::CleanupDisposition::AlreadyAbsent,
            )
        })
        .collect();
    CorePostCommitSettlementPlanDescriptor::new(
        prior_trace_start,
        settled_original,
        residual_original,
        authority_non_top,
        CorePostCommitPlannedCleanupProjection::new(
            settlement_key(2, 7),
            settlement_key(12, 77),
            crate::trace::CleanupDisposition::RemovedDetached,
        ),
        settlement_key(3, 41),
        CorePostCommitPlannedHostEdgeProjection::new(settlement_key(3, 41), settlement_key(13, 42)),
    )
}

fn active_post_commit_compound_fixture() -> (
    RouterEngine,
    NavigationRequestId,
    CleanupBatchToken,
    CleanupBatchProjection,
    CorePostCommitCompoundBatchOwner,
    TestPostCommitPhysicalAuthority,
) {
    let (mut engine, mut issuer, request, normal, prepared, authority_batch) =
        prepared_post_commit_authority_fixture();
    let installed = std::cell::RefCell::new(None);
    let outcome = engine.begin_post_commit_authority_transfer(
        &mut issuer,
        prepared,
        0xfeed_u64,
        |physical, _, plan| {
            let prior_trace_start = plan.current_trace.len();
            let (plan, physical_authority) =
                bind_test_settlement_plan_with_authority(plan, prior_trace_start, Vec::new());
            Ok::<_, (u64, &'static str)>(((physical, physical_authority), plan))
        },
        |(physical, physical_authority), batch| {
            assert_eq!(physical, 0xfeed);
            *installed.borrow_mut() = Some((batch, physical_authority));
        },
    );
    assert!(matches!(
        outcome,
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    let (batch, physical_authority) = installed
        .into_inner()
        .expect("the sole install callback receives the compound owner");
    (
        engine,
        request,
        normal,
        authority_batch,
        batch,
        physical_authority,
    )
}

fn exact_post_commit_completion_inputs(
    normal: &CleanupBatchToken,
    authority: &CleanupBatchProjection,
    batch: &CorePostCommitCompoundBatchOwner,
) -> PostCommitCompoundCompletionInputs {
    let normal_digest = if normal.obligation_count() == 0 {
        CleanupCompletionDigest::canonical_empty()
    } else {
        CleanupCompletionDigest::from_opaque(0x4c)
    };
    PostCommitCompoundCompletionInputs::new(
        CleanupCompletion::attest(normal.clone(), normal_digest, normal.obligation_count()),
        CoreNormalCleanupCompletionFacts::new(
            CleanupBatchProjection::from_token(normal),
            normal.manifest_digest(),
            normal_digest,
            normal.obligation_count(),
        ),
        CoreAuthorityCleanupCompletionFacts::new(
            authority.clone(),
            CleanupCompletionDigest::from_opaque(0xa11),
            authority.obligation_count,
        ),
        CoreAuthorityCleanupCompletionFacts::new(
            authority.clone(),
            CleanupCompletionDigest::from_opaque(0xa11),
            authority.obligation_count,
        ),
        CoreRetiredHostEdgeCompletionFacts::new(
            batch.host_edge.expected.clone(),
            batch.host_edge.expected_physical_digest,
        ),
        batch.settlement_projection(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TestPostCommitPhysicalFact {
    role: CorePostCommitSettlementRole,
    obligation: CorePostCommitSettlementKeyProjection,
    physical_source: CorePostCommitSettlementKeyProjection,
    disposition: Option<crate::trace::CleanupDisposition>,
}

impl TestPostCommitPhysicalFact {
    fn cleanup(
        role: CorePostCommitSettlementRole,
        cleanup: &CorePostCommitPlannedCleanupProjection,
    ) -> Self {
        Self {
            role,
            obligation: cleanup.obligation.clone(),
            physical_source: cleanup.physical_source.clone(),
            disposition: Some(cleanup.disposition),
        }
    }
}

struct TestPostCommitPhysicalAuthority {
    source: std::rc::Rc<()>,
    planned: Vec<TestPostCommitPhysicalFact>,
}

impl TestPostCommitPhysicalAuthority {
    fn from_descriptor(descriptor: &CorePostCommitSettlementPlanDescriptor) -> Self {
        let mut planned = descriptor
            .residual_original
            .iter()
            .map(|cleanup| {
                TestPostCommitPhysicalFact::cleanup(
                    CorePostCommitSettlementRole::OriginalResidual,
                    cleanup,
                )
            })
            .chain(descriptor.authority_non_top.iter().map(|cleanup| {
                TestPostCommitPhysicalFact::cleanup(
                    CorePostCommitSettlementRole::AuthorityNonTop,
                    cleanup,
                )
            }))
            .collect::<Vec<_>>();
        planned.push(TestPostCommitPhysicalFact {
            role: CorePostCommitSettlementRole::RetiredHostEdge,
            obligation: descriptor.host_edge.obligation.clone(),
            physical_source: descriptor.host_edge.physical_source.clone(),
            disposition: None,
        });
        planned.push(TestPostCommitPhysicalFact::cleanup(
            CorePostCommitSettlementRole::AuthorityTop,
            &descriptor.authority_top,
        ));
        Self {
            source: std::rc::Rc::new(()),
            planned,
        }
    }

    fn issue(
        &self,
        ordinal: usize,
        dropped: std::rc::Rc<std::cell::Cell<usize>>,
    ) -> FakePostCommitPhysicalProof {
        FakePostCommitPhysicalProof {
            source: self.source.clone(),
            fact: self.planned[ordinal].clone(),
            dropped,
        }
    }

    fn issue_fact(
        &self,
        fact: TestPostCommitPhysicalFact,
        dropped: std::rc::Rc<std::cell::Cell<usize>>,
    ) -> FakePostCommitPhysicalProof {
        FakePostCommitPhysicalProof {
            source: self.source.clone(),
            fact,
            dropped,
        }
    }

    fn foreign_equivalent(&self) -> Self {
        Self {
            source: std::rc::Rc::new(()),
            planned: self.planned.clone(),
        }
    }

    fn attest(
        &self,
        physical: FakePostCommitPhysicalProof,
        permit: &super::CorePostCommitSettlementPermit,
    ) -> Result<
        CorePostCommitSettlementAttestation<FakePostCommitPhysicalProof>,
        (FakePostCommitPhysicalProof, TestPostCommitPhysicalBindError),
    > {
        if !std::rc::Rc::ptr_eq(&self.source, &physical.source) {
            return Err((physical, TestPostCommitPhysicalBindError::WrongSource));
        }
        let expected = permit.expected();
        let exact = physical.fact.role == expected.role
            && physical.fact.obligation == expected.obligation
            && physical.fact.physical_source == expected.physical_source
            && physical.fact.disposition == expected.disposition;
        if !exact {
            return Err((physical, TestPostCommitPhysicalBindError::WrongFact));
        }
        Ok(CorePostCommitSettlementAttestation::from_trusted_physical(
            physical, permit,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestPostCommitPhysicalBindError {
    WrongSource,
    WrongFact,
}

struct FakePostCommitPhysicalProof {
    source: std::rc::Rc<()>,
    fact: TestPostCommitPhysicalFact,
    dropped: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Drop for FakePostCommitPhysicalProof {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

fn record_exact_post_commit_settlement_trace(
    engine: &mut RouterEngine,
    mut batch: CorePostCommitCompoundBatchOwner,
    physical_authority: &TestPostCommitPhysicalAuthority,
) -> (
    CorePostCommitCompoundBatchOwner,
    CorePostCommitSettlementTraceProjection,
) {
    let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
    let initial_cursor = batch.settlement_projection().cursor;
    let mut recorded = 0usize;
    loop {
        let trace = batch.settlement_projection();
        let Some(expected) = trace.expected.get(trace.cursor).cloned() else {
            assert_eq!(
                trace.progress(),
                CorePostCommitSettlementTraceProgress::Complete
            );
            assert_eq!(dropped.get(), recorded);
            return (batch, trace);
        };
        let attempt = match engine.prepare_next_post_commit_settlement(batch) {
            CorePostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
            CorePostCommitSettlementPermitOutcome::Mismatch { .. } => {
                panic!("the exact linear owner must issue its next private permit")
            }
        };
        let proof = physical_authority.issue(expected.ordinal, dropped.clone());
        batch = match engine.record_next_post_commit_settlement(attempt, proof, |proof, permit| {
            physical_authority.attest(proof, permit)
        }) {
            CorePostCommitSettlementRecordOutcome::Recorded { owner, trace } => {
                recorded += 1;
                assert_eq!(trace.cursor, initial_cursor + recorded);
                owner
            }
            CorePostCommitSettlementRecordOutcome::Mismatch { .. }
            | CorePostCommitSettlementRecordOutcome::BindingMismatch { .. }
            | CorePostCommitSettlementRecordOutcome::AttestationMismatch { .. } => {
                panic!(
                    "each independently source-bound non-Clone proof must attest the Core permit"
                )
            }
        };
    }
}

#[test]
fn post_commit_authority_transfer_receiver_mismatch_returns_whole_owner_and_calls_zero() {
    let (mut engine, mut issuer, _request, _normal, owner, batch) =
        prepared_post_commit_authority_fixture();
    let lease = owner.host.retirement_projection();
    let owner_batch = owner.authority_batch.projection();
    engine
        .current
        .as_mut()
        .expect("the accepted receiver remains current")
        .identity
        .branch_index += 1;
    let bind_calls = std::cell::Cell::new(0usize);
    let install_calls = std::cell::Cell::new(0usize);

    let outcome = engine.begin_post_commit_authority_transfer(
        &mut issuer,
        owner,
        7_u64,
        |physical, observed_batch, plan| {
            bind_calls.set(bind_calls.get() + 1);
            assert_eq!(observed_batch, &batch);
            Ok::<_, (u64, ())>((physical + 1, bind_exact_test_settlement_plan(plan)))
        },
        |_, _| install_calls.set(install_calls.get() + 1),
    );

    let PostCommitAuthorityBeginCoreOutcome::ReceiverMismatch {
        owner,
        bound,
        evidence: _,
    } = outcome
    else {
        panic!("post-bind receiver drift must return the whole prepared owner and Bound")
    };
    assert_eq!(bound, 8);
    assert_eq!(owner.host.retirement_projection(), lease);
    assert_eq!(owner.authority_batch.projection(), owner_batch);
    assert_eq!(bind_calls.get(), 1);
    assert_eq!(install_calls.get(), 0);
}

#[test]
fn post_commit_cleanup_batch_identity_exhaustion_is_permanent_and_zero_change() {
    let (mut engine, _issuer, lease) = ready_engine_with_tree_and_authority(tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    engine.push(RouteLocation::new("/")).unwrap();
    let normal = publish_current(&mut engine);
    let transfer = match engine
        .accept_post_commit_authority_transfer(&normal, PostCommitAuthorityCause::HostEntityMissing)
    {
        PostCommitAuthorityAcceptanceOutcome::Accepted(owner) => owner,
        PostCommitAuthorityAcceptanceOutcome::Mismatch { .. } => {
            panic!("the real post-commit receiver must be accepted")
        }
    };
    let source = CorePostCommitAuthorityPreparationSource::new(transfer, lease);
    let manifest = CleanupManifestSeal::new(
        engine.active().frames().iter().rev().cloned().collect(),
        manifest_digest(0xa1),
        engine.active().frames().len(),
    );
    engine.testing_exhaust_reservation_counter(ReservationCounter::CleanupBatch);
    let before = engine.snapshot();

    let PostCommitAuthorityPreparationOutcome::Permanent { source, evidence } =
        engine.prepare_post_commit_authority_transfer(source, manifest)
    else {
        panic!("monotonic cleanup-batch identity exhaustion must fail permanently")
    };
    assert_eq!(source.cause(), PostCommitAuthorityCause::HostEntityMissing);
    assert_eq!(
        evidence.contradiction(),
        CorePostCommitAuthorityContradiction::IdentityExhausted
    );
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn post_commit_closed_trace_is_permanent_and_calls_zero() {
    let (mut engine, mut issuer, request, _normal, owner, _batch) =
        prepared_post_commit_authority_fixture();
    engine
        .records
        .iter_mut()
        .find(|record| record.id() == request)
        .expect("the current request remains retained")
        .testing_close_trace();
    let before = engine.snapshot();
    let install_calls = std::cell::Cell::new(0usize);

    let PostCommitAuthorityBeginCoreOutcome::Permanent {
        owner,
        bound,
        evidence,
    } = engine.begin_post_commit_authority_transfer(
        &mut issuer,
        owner,
        7_u64,
        |physical, _, plan| {
            Ok::<_, (u64, ())>((physical + 1, bind_exact_test_settlement_plan(plan)))
        },
        |_, _| install_calls.set(install_calls.get() + 1),
    )
    else {
        panic!("a closed trace is a permanent post-commit terminal contradiction")
    };
    assert_eq!(bound, 8);
    assert_eq!(owner.cause(), PostCommitAuthorityCause::HostEntityMissing);
    assert_eq!(
        evidence.contradiction(),
        CorePostCommitAuthorityContradiction::TraceMismatch
    );
    assert_eq!(install_calls.get(), 0);
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn post_commit_host_retirement_is_last_fallible_before_callback() {
    let (mut engine, _issuer, _request, _normal, owner, batch) =
        prepared_post_commit_authority_fixture();
    let mut foreign_issuer = HostAdmissionIssuer::new().unwrap();
    let order = std::cell::RefCell::new(Vec::new());

    let outcome = engine.begin_post_commit_authority_transfer(
        &mut foreign_issuer,
        owner,
        "unbound",
        |physical, observed_batch, plan| {
            order.borrow_mut().push("bind");
            assert_eq!(observed_batch, &batch);
            Ok::<_, (&'static str, ())>((physical, bind_exact_test_settlement_plan(plan)))
        },
        |_, _| order.borrow_mut().push("install"),
    );

    assert!(matches!(
        outcome,
        PostCommitAuthorityBeginCoreOutcome::Permanent {
            bound: "unbound",
            evidence: CorePostCommitAuthorityBeginPermanentEvidence {
                contradiction: CorePostCommitAuthorityContradiction::RetiredHostMismatch,
                ..
            },
            ..
        }
    ));
    assert_eq!(&*order.borrow(), &["bind"]);

    let (mut engine, mut issuer, _request, _normal, owner, _) =
        prepared_post_commit_authority_fixture();
    let order = std::cell::RefCell::new(Vec::new());
    let outcome = engine.begin_post_commit_authority_transfer(
        &mut issuer,
        owner,
        (),
        |(), _, plan| {
            order.borrow_mut().push("bind");
            Ok::<_, ((), ())>(((), bind_exact_test_settlement_plan(plan)))
        },
        |(), _| order.borrow_mut().push("install"),
    );
    assert!(matches!(
        outcome,
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    assert_eq!(&*order.borrow(), &["bind", "install"]);
}

#[test]
fn post_commit_plan_rejects_cross_role_alias_and_wrong_h_top_dependency() {
    #[derive(Clone, Copy)]
    enum PlanDrift {
        CrossRoleAlias,
        WrongHostTopDependency,
    }

    for drift in [PlanDrift::CrossRoleAlias, PlanDrift::WrongHostTopDependency] {
        let (mut engine, mut issuer, _request, _normal, owner, _) =
            prepared_post_commit_authority_fixture();
        let outcome = engine.begin_post_commit_authority_transfer(
            &mut issuer,
            owner,
            (),
            |unbound, _, plan| {
                let mut descriptor =
                    test_settlement_plan_descriptor(&plan, plan.current_trace.len(), Vec::new());
                match drift {
                    PlanDrift::CrossRoleAlias => {
                        descriptor.authority_top.obligation =
                            descriptor.residual_original[0].obligation.clone();
                    }
                    PlanDrift::WrongHostTopDependency => {
                        descriptor.authority_top_dependency = settlement_key(30, 999);
                    }
                }
                let evidence = match plan.bind(descriptor) {
                    Err(evidence) => evidence,
                    Ok(_) => panic!("the drifted explicit partition must fail closed"),
                };
                Err::<((), super::CoreBoundPostCommitSettlementPlan), _>((unbound, evidence.kind()))
            },
            |(), _| unreachable!("a rejected plan cannot install a compound owner"),
        );
        let PostCommitAuthorityBeginCoreOutcome::BindingMismatch { evidence, .. } = outcome else {
            panic!("a plan-partition contradiction must remain a bind mismatch")
        };
        assert_eq!(
            evidence,
            match drift {
                PlanDrift::CrossRoleAlias => {
                    super::CorePostCommitSettlementPlanMismatchKind::DuplicateKey
                }
                PlanDrift::WrongHostTopDependency => {
                    super::CorePostCommitSettlementPlanMismatchKind::WrongSource
                }
            }
        );
    }
}

#[test]
fn post_commit_plan_binds_only_exact_prior_l_tail_and_receipt_identities() {
    let (mut engine, mut issuer, lease) = ready_engine_with_tree_and_authority(tree());
    engine.push(RouteLocation::new("/items/1")).unwrap();
    settle_current(&mut engine);
    let request = engine.push(RouteLocation::new("/")).unwrap();
    let normal = publish_current(&mut engine);
    assert!(normal.obligation_count() > 0);
    RouterEngine::append_cleanup_settlement_trace_record(
        &mut engine.records,
        request,
        Some(crate::trace::CleanupDisposition::AlreadyAbsent),
    )
    .expect("the retained request accepts an unrelated earlier cleanup event");
    let prior_trace_start = engine.request(request).unwrap().trace().len();
    engine
        .observe_cleanup_receipt(&normal, crate::trace::CleanupDisposition::Removed)
        .expect("the pre-handoff real L receiver accepts its exact receipt");

    let accepted = match engine
        .accept_post_commit_authority_transfer(&normal, PostCommitAuthorityCause::HostEntityMissing)
    {
        PostCommitAuthorityAcceptanceOutcome::Accepted(owner) => owner,
        PostCommitAuthorityAcceptanceOutcome::Mismatch { .. } => {
            panic!("the exact post-commit receiver must be accepted")
        }
    };
    let accepted_before = engine.snapshot();
    assert_eq!(
        engine.observe_cleanup_receipt(&normal, crate::trace::CleanupDisposition::RemovedDetached,),
        Err(CleanupError::Stale)
    );
    assert_eq!(engine.snapshot(), accepted_before);
    let prepared = match engine.prepare_post_commit_authority_transfer(
        CorePostCommitAuthorityPreparationSource::new(accepted, lease),
        CleanupManifestSeal::new(
            engine.active().frames().iter().rev().cloned().collect(),
            manifest_digest(0xa1),
            engine.active().frames().len(),
        ),
    ) {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, .. } => owner,
        _ => panic!("the exact accepted source must prepare"),
    };
    let installed = std::cell::RefCell::new(None);
    assert!(matches!(
        engine.begin_post_commit_authority_transfer(
            &mut issuer,
            prepared,
            (),
            |(), _, plan| {
                let settled_obligation = settlement_key(1, 100);
                let settled_receipt = settlement_key(10, 0xabc);
                let settled = vec![super::CorePostCommitSettledOriginalProjection::new(
                    settled_obligation,
                    settled_receipt,
                    crate::trace::CleanupDisposition::Removed,
                )];
                let (plan, physical_authority) =
                    bind_test_settlement_plan_with_authority(plan, prior_trace_start, settled);
                Ok::<_, ((), ())>((((), physical_authority), plan))
            },
            |((), physical_authority), batch| {
                *installed.borrow_mut() = Some((batch, physical_authority));
            },
        ),
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    let (batch, physical_authority) = installed.into_inner().unwrap();
    let trace = batch.settlement_projection();
    assert_eq!(trace.binding.prior_trace_start, prior_trace_start);
    assert_eq!(trace.binding.settled_original.len(), 1);
    assert_eq!(
        trace.binding.settled_original[0].receipt.identity(),
        &[0xabc]
    );
    assert_eq!(
        trace
            .expected
            .iter()
            .filter(|entry| entry.role == CorePostCommitSettlementRole::OriginalResidual)
            .count(),
        normal.obligation_count() - 1
    );
    assert!(trace.expected.iter().all(|entry| {
        entry.obligation() != &settlement_key(1, 100)
            && entry.physical_source() != &settlement_key(10, 0xabc)
    }));
    let trace_len_before = engine.request(request).unwrap().trace().len();
    let disposition_count_before = engine
        .request(request)
        .unwrap()
        .trace()
        .iter()
        .filter(|event| event.cleanup_disposition().is_some())
        .count();
    assert_eq!(
        disposition_count_before, 2,
        "the unrelated event and exact settled L receipt remain distinct from the new trace"
    );
    let expected_new_events = trace.expected.len();
    let expected_new_receipts = trace
        .expected
        .iter()
        .filter(|entry| entry.disposition().is_some())
        .count();
    let (_batch, complete) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    assert_eq!(
        complete
            .expected
            .iter()
            .filter(|entry| entry.role == CorePostCommitSettlementRole::OriginalResidual)
            .count(),
        normal.obligation_count() - 1
    );
    assert_eq!(complete.recorded.len(), expected_new_events);
    assert_eq!(
        engine.request(request).unwrap().trace().len(),
        trace_len_before + expected_new_events,
        "every planned L/A/H settlement appends exactly one public trace event"
    );
    assert_eq!(
        engine
            .request(request)
            .unwrap()
            .trace()
            .iter()
            .filter(|event| event.cleanup_disposition().is_some())
            .count(),
        disposition_count_before + expected_new_receipts,
        "the already-settled L receipt is not recorded again by residual settlement"
    );
}

#[test]
fn post_commit_settlement_trace_is_exact_ordered_and_zero_write_on_mismatch() {
    let (mut engine, request, normal, authority, batch, physical_authority) =
        active_post_commit_compound_fixture();
    assert!(normal.obligation_count() > 0);
    assert!(authority.obligation_count() > 0);

    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let before_incomplete = engine.snapshot();
    let PostCommitCompoundCompletionPreparationOutcome::Mismatch {
        batch,
        inputs: _,
        evidence: CorePostCommitCompoundMismatchEvidence::SettlementTrace { observed, .. },
    } = engine.prepare_post_commit_compound_completion(batch, inputs)
    else {
        panic!("completion preparation must reject an incomplete typed settlement trace")
    };
    assert_eq!(
        observed
            .expect("begin installs the initial typed trace")
            .progress(),
        CorePostCommitSettlementTraceProgress::OriginalResidual
    );
    assert_eq!(engine.snapshot(), before_incomplete);
    let first_expected = physical_authority.planned[0].clone();
    let mut wrong_source = first_expected.clone();
    wrong_source.physical_source = settlement_key(99, 1);
    let mut wrong_disposition = first_expected;
    wrong_disposition.disposition = Some(crate::trace::CleanupDisposition::AlreadyAbsent);
    let mut attempt = match engine.prepare_next_post_commit_settlement(batch) {
        CorePostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        CorePostCommitSettlementPermitOutcome::Mismatch { .. } => {
            panic!("the live compound owner must issue the first private permit")
        }
    };
    let foreign_authority = physical_authority.foreign_equivalent();
    let before_foreign_source = engine.snapshot();
    let foreign_dropped = std::rc::Rc::new(std::cell::Cell::new(0));
    let foreign_proof = foreign_authority.issue(0, foreign_dropped.clone());
    let CorePostCommitSettlementRecordOutcome::BindingMismatch {
        attempt: returned_attempt,
        physical,
        bind_error: TestPostCommitPhysicalBindError::WrongSource,
    } = engine.record_next_post_commit_settlement(attempt, foreign_proof, |proof, permit| {
        physical_authority.attest(proof, permit)
    })
    else {
        panic!("a proof issued by a foreign physical source must return whole")
    };
    assert_eq!(foreign_dropped.get(), 0);
    assert_eq!(engine.snapshot(), before_foreign_source);
    drop(physical);
    assert_eq!(foreign_dropped.get(), 1);
    attempt = returned_attempt;
    for wrong_fact in [wrong_source, wrong_disposition] {
        let before = engine.snapshot();
        let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
        let calls = std::cell::Cell::new(0usize);
        let proof = physical_authority.issue_fact(wrong_fact, dropped.clone());
        let CorePostCommitSettlementRecordOutcome::BindingMismatch {
            attempt: returned_attempt,
            physical,
            bind_error: TestPostCommitPhysicalBindError::WrongFact,
        } = engine.record_next_post_commit_settlement(attempt, proof, |proof, permit| {
            calls.set(calls.get() + 1);
            physical_authority.attest(proof, permit)
        })
        else {
            panic!("wrong physical facts must return the whole permit and proof")
        };
        assert_eq!(calls.get(), 1);
        assert_eq!(dropped.get(), 0);
        assert_eq!(engine.snapshot(), before);
        drop(physical);
        assert_eq!(dropped.get(), 1);
        attempt = returned_attempt;
    }
    let first_ordinal = attempt.permit.expected.ordinal;
    let mut batch = match engine.record_next_post_commit_settlement(
        attempt,
        physical_authority.issue(first_ordinal, std::rc::Rc::new(std::cell::Cell::new(0))),
        |proof, permit| physical_authority.attest(proof, permit),
    ) {
        CorePostCommitSettlementRecordOutcome::Recorded { owner, .. } => owner,
        _ => panic!("the exact independently frozen proof must consume the permit"),
    };

    let receiver_attempt = match engine.prepare_next_post_commit_settlement(batch) {
        CorePostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        CorePostCommitSettlementPermitOutcome::Mismatch { .. } => {
            panic!("the next exact permit must remain available")
        }
    };
    let receiver_ordinal = receiver_attempt.permit.expected.ordinal;
    engine.current.as_mut().unwrap().identity.branch_index += 1;
    let before_receiver = engine.snapshot();
    let receiver_dropped = std::rc::Rc::new(std::cell::Cell::new(0));
    let receiver_proof = physical_authority.issue(receiver_ordinal, receiver_dropped.clone());
    let receiver_calls = std::cell::Cell::new(0usize);
    let CorePostCommitSettlementRecordOutcome::Mismatch {
        attempt: receiver_attempt,
        physical,
        evidence,
    } = engine.record_next_post_commit_settlement(
        receiver_attempt,
        receiver_proof,
        |proof, permit| {
            receiver_calls.set(receiver_calls.get() + 1);
            physical_authority.attest(proof, permit)
        },
    )
    else {
        panic!("receiver drift must return whole owner and physical evidence")
    };
    assert_eq!(
        evidence.kind(),
        CorePostCommitSettlementTraceMismatchKind::Receiver
    );
    assert_eq!(receiver_calls.get(), 0);
    assert_eq!(receiver_dropped.get(), 0);
    assert_eq!(engine.snapshot(), before_receiver);
    engine.current.as_mut().unwrap().identity.branch_index -= 1;
    batch = match engine.record_next_post_commit_settlement(
        receiver_attempt,
        physical,
        |proof, permit| physical_authority.attest(proof, permit),
    ) {
        CorePostCommitSettlementRecordOutcome::Recorded { owner, .. } => owner,
        _ => panic!("the returned whole permit and proof must remain retryable"),
    };
    assert_eq!(receiver_dropped.get(), 1);

    let out_of_order_attempt = match engine.prepare_next_post_commit_settlement(batch) {
        CorePostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        CorePostCommitSettlementPermitOutcome::Mismatch { .. } => {
            panic!("the next exact permit must remain available")
        }
    };
    let expected_ordinal = out_of_order_attempt.permit.expected.ordinal;
    let later_authority_ordinal = physical_authority.planned.len() - 1;
    assert_eq!(
        physical_authority.planned[later_authority_ordinal]
            .obligation
            .identity(),
        &[7]
    );
    let out_of_order_ordinal = if expected_ordinal == later_authority_ordinal {
        0
    } else {
        later_authority_ordinal
    };
    let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
    let callback_calls = std::cell::Cell::new(0usize);
    let before_out_of_order = engine.snapshot();
    let proof = physical_authority.issue(out_of_order_ordinal, dropped.clone());
    let CorePostCommitSettlementRecordOutcome::BindingMismatch {
        attempt: out_of_order_attempt,
        physical,
        bind_error: TestPostCommitPhysicalBindError::WrongFact,
    } = engine.record_next_post_commit_settlement(out_of_order_attempt, proof, |proof, permit| {
        callback_calls.set(callback_calls.get() + 1);
        physical_authority.attest(proof, permit)
    })
    else {
        panic!("an out-of-order typed physical proof must be returned whole")
    };
    assert_ne!(
        physical.fact.obligation,
        out_of_order_attempt.permit.expected.obligation
    );
    assert_eq!(callback_calls.get(), 1);
    assert_eq!(dropped.get(), 0);
    assert_eq!(engine.snapshot(), before_out_of_order);
    drop(physical);
    assert_eq!(dropped.get(), 1);
    batch = match engine.record_next_post_commit_settlement(
        out_of_order_attempt,
        physical_authority.issue(expected_ordinal, std::rc::Rc::new(std::cell::Cell::new(0))),
        |proof, permit| physical_authority.attest(proof, permit),
    ) {
        CorePostCommitSettlementRecordOutcome::Recorded { owner, .. } => owner,
        _ => panic!("an out-of-order mismatch must preserve the exact permit for retry"),
    };

    let (
        _foreign_engine,
        _foreign_request,
        _foreign_normal,
        _foreign_authority,
        foreign_batch,
        _foreign_physical_authority,
    ) = active_post_commit_compound_fixture();
    let before_wrong_batch = engine.snapshot();
    let CorePostCommitSettlementPermitOutcome::Mismatch {
        owner: foreign_batch,
        evidence: wrong_owner,
    } = engine.prepare_next_post_commit_settlement(foreign_batch)
    else {
        panic!("a foreign whole owner must be rejected before requesting physical proof")
    };
    assert_eq!(
        wrong_owner.kind(),
        CorePostCommitSettlementTraceMismatchKind::WrongOwner
    );
    assert_eq!(engine.snapshot(), before_wrong_batch);
    drop(foreign_batch);

    let (batch, projection) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    assert_eq!(
        projection.progress(),
        CorePostCommitSettlementTraceProgress::Complete
    );
    assert_eq!(projection.cause(), batch.cause());
    let trace = engine
        .pending_post_commit_settlement_trace
        .as_ref()
        .expect("the complete typed trace remains linearly installed");
    assert_eq!(
        trace.expected.last().unwrap().role,
        CorePostCommitSettlementRole::AuthorityTop
    );
    assert_eq!(trace.expected.last().unwrap().obligation.identity(), &[7]);
    assert_eq!(
        trace.expected[trace.expected.len() - 2].role,
        CorePostCommitSettlementRole::RetiredHostEdge
    );
    let public_settlement_tail = engine
        .request(request)
        .unwrap()
        .trace()
        .get(trace.settlement_event_start..)
        .unwrap();
    let expected_public_settlement_tail = trace
        .recorded
        .iter()
        .map(|entry| NavigationTraceEvent::cleanup_settled(entry.disposition))
        .collect::<Vec<_>>();
    assert_eq!(
        public_settlement_tail,
        expected_public_settlement_tail.as_slice()
    );

    let before_terminal_replay = engine.snapshot();
    let CorePostCommitSettlementPermitOutcome::Mismatch {
        owner: _,
        evidence: terminal_replay,
    } = engine.prepare_next_post_commit_settlement(batch)
    else {
        panic!("the completed capability must reject replay")
    };
    assert_eq!(
        terminal_replay.kind(),
        CorePostCommitSettlementTraceMismatchKind::Replay
    );
    assert_eq!(engine.snapshot(), before_terminal_replay);
}

#[test]
fn post_commit_h_stage_requires_its_exact_typed_physical_proof() {
    let (mut engine, _request, _normal, _authority, mut batch, physical_authority) =
        active_post_commit_compound_fixture();
    while batch.settlement_projection().progress()
        != CorePostCommitSettlementTraceProgress::RetiredHostEdge
    {
        let attempt = match engine.prepare_next_post_commit_settlement(batch) {
            CorePostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
            CorePostCommitSettlementPermitOutcome::Mismatch { .. } => {
                panic!("the exact pre-H owner must issue its private permit")
            }
        };
        let ordinal = attempt.permit.expected.ordinal;
        batch = match engine.record_next_post_commit_settlement(
            attempt,
            physical_authority.issue(ordinal, std::rc::Rc::new(std::cell::Cell::new(0))),
            |proof, permit| physical_authority.attest(proof, permit),
        ) {
            CorePostCommitSettlementRecordOutcome::Recorded { owner, .. } => owner,
            _ => panic!("the exact pre-H typed proof must advance once"),
        };
    }
    let h_attempt = match engine.prepare_next_post_commit_settlement(batch) {
        CorePostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        CorePostCommitSettlementPermitOutcome::Mismatch { .. } => {
            panic!("the exact H permit must remain available")
        }
    };
    let h_expected = h_attempt.permit.expected.clone();
    assert_eq!(
        h_expected.role,
        CorePostCommitSettlementRole::RetiredHostEdge
    );
    let before = engine.snapshot();
    let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
    let proof = physical_authority.issue(physical_authority.planned.len() - 1, dropped.clone());
    let CorePostCommitSettlementRecordOutcome::BindingMismatch {
        attempt: h_attempt,
        physical,
        bind_error: TestPostCommitPhysicalBindError::WrongFact,
    } = engine.record_next_post_commit_settlement(h_attempt, proof, |proof, permit| {
        physical_authority.attest(proof, permit)
    })
    else {
        panic!("cleanup disposition cannot stand in for typed H evidence")
    };
    assert_eq!(engine.snapshot(), before);
    drop(physical);
    assert_eq!(dropped.get(), 1);
    let batch = match engine.record_next_post_commit_settlement(
        h_attempt,
        physical_authority.issue(
            h_expected.ordinal,
            std::rc::Rc::new(std::cell::Cell::new(0)),
        ),
        |proof, permit| physical_authority.attest(proof, permit),
    ) {
        CorePostCommitSettlementRecordOutcome::Recorded { owner, .. } => owner,
        _ => panic!("only the exact source-bound H proof may advance the permit"),
    };
    let (_batch, complete) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    assert_eq!(
        complete.progress(),
        CorePostCommitSettlementTraceProgress::Complete
    );
}

#[test]
fn post_commit_authority_cause_survives_compound_terminal_and_diagnostic() {
    let causes = [
        PostCommitAuthorityCause::HostRegistrationLost,
        PostCommitAuthorityCause::HostEntityMissing,
        PostCommitAuthorityCause::RequiredResourcesLost,
    ];
    let mut terminal_causes = Vec::new();
    let mut diagnostic_causes = Vec::new();

    for cause in causes.iter().copied() {
        let (mut engine, mut issuer, request, normal, prepared, authority) =
            prepared_post_commit_authority_fixture_for_cause(cause);
        assert_eq!(prepared.transfer.cause(), cause);
        let installed = std::cell::RefCell::new(None);
        assert!(matches!(
            engine.begin_post_commit_authority_transfer(
                &mut issuer,
                prepared,
                (),
                |(), _, plan| {
                    let prior_trace_start = plan.current_trace.len();
                    let (plan, physical_authority) = bind_test_settlement_plan_with_authority(
                        plan,
                        prior_trace_start,
                        Vec::new(),
                    );
                    Ok::<_, ((), ())>((((), physical_authority), plan))
                },
                |((), physical_authority), batch| {
                    *installed.borrow_mut() = Some((batch, physical_authority));
                },
            ),
            PostCommitAuthorityBeginCoreOutcome::Begun
        ));
        let (batch, physical_authority) = installed.into_inner().unwrap();
        assert_eq!(batch.cause(), cause);
        assert_eq!(batch.host_edge.expected.cause(), cause);
        let (batch, trace) =
            record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
        assert_eq!(trace.cause(), cause);
        let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
        let owner = match engine.prepare_post_commit_compound_completion(batch, inputs) {
            PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) => owner,
            PostCommitCompoundCompletionPreparationOutcome::Mismatch { .. } => {
                panic!("the complete exact trace must prepare for {cause:?}")
            }
        };
        let PostCommitCompoundCompletionCoreOutcome::Terminal {
            terminal,
            retired_host,
            physical: (),
        } = engine.complete_post_commit_authority_compound(owner, |_| ())
        else {
            panic!("the exact compound must terminalize for {cause:?}")
        };
        assert_eq!(terminal.cause(), cause);
        assert_eq!(retired_host.cause(), cause);
        assert_eq!(
            terminal.trace_digest(),
            navigation_trace_digest(engine.request(request).unwrap().trace())
        );
        let request_diagnostic = engine
            .request(request)
            .unwrap()
            .diagnostics()
            .iter()
            .find(|event| {
                event.kind() == crate::diagnostics::DiagnosticEventKind::PostCommitCleanupAnomaly
            })
            .unwrap();
        let router_diagnostic = engine
            .router_diagnostics()
            .iter()
            .find(|event| {
                event.kind() == crate::diagnostics::DiagnosticEventKind::PostCommitCleanupAnomaly
            })
            .unwrap();
        assert_eq!(request_diagnostic, router_diagnostic);
        let diagnostic_cause = PostCommitAuthorityCause::from(
            request_diagnostic
                .post_commit_cleanup_anomaly_cause()
                .expect("the typed severe diagnostic retains its exact cause"),
        );
        assert_eq!(diagnostic_cause, cause);
        terminal_causes.push(terminal.cause());
        diagnostic_causes.push(diagnostic_cause);
    }

    assert_eq!(terminal_causes, causes.to_vec());
    assert_eq!(diagnostic_causes, causes.to_vec());
}

#[test]
fn navigation_trace_digest_is_content_and_order_sensitive() {
    let published_then_recovering = [
        NavigationTraceEvent::commit_published(),
        NavigationTraceEvent::request_phase_changed(NavigationPhase::Recovering),
    ];
    let recovering_then_published = [
        NavigationTraceEvent::request_phase_changed(NavigationPhase::Recovering),
        NavigationTraceEvent::commit_published(),
    ];
    assert_eq!(
        published_then_recovering.len(),
        recovering_then_published.len()
    );
    assert_ne!(
        navigation_trace_digest(&published_then_recovering),
        navigation_trace_digest(&recovering_then_published)
    );

    let removed = [NavigationTraceEvent::cleanup_settled(Some(
        crate::trace::CleanupDisposition::Removed,
    ))];
    let already_absent = [NavigationTraceEvent::cleanup_settled(Some(
        crate::trace::CleanupDisposition::AlreadyAbsent,
    ))];
    assert_eq!(removed.len(), already_absent.len());
    assert_ne!(
        navigation_trace_digest(&removed),
        navigation_trace_digest(&already_absent)
    );

    let (_engine, _request, _normal, _authority, batch, _physical_authority) =
        active_post_commit_compound_fixture();
    let original = batch.settlement_projection();
    let mut different_content = original.clone();
    different_content.expected[0].physical_source = settlement_key(88, 1234);
    different_content.digest = super::post_commit_settlement_trace_digest(&different_content);
    assert_eq!(original.expected.len(), different_content.expected.len());
    assert_ne!(original.digest(), different_content.digest());

    let mut different_order = original.clone();
    different_order.expected.swap(0, 1);
    different_order.digest = super::post_commit_settlement_trace_digest(&different_order);
    assert_eq!(original.expected.len(), different_order.expected.len());
    assert_ne!(original.digest(), different_order.digest());

    let mut different_compound = original.clone();
    different_compound.binding.compound.receiver.stage =
        CorePostCommitCleanupStageProjection::AuthorityAccepted;
    different_compound.digest = super::post_commit_settlement_trace_digest(&different_compound);
    assert_ne!(original.digest(), different_compound.digest());

    let mut different_normal = original.clone();
    different_normal.binding.normal_batch = different_normal
        .binding
        .normal_batch
        .testing_with_binding_digest(different_normal.binding.normal_batch.binding_digest() ^ 1);
    different_normal.digest = super::post_commit_settlement_trace_digest(&different_normal);
    assert_ne!(original.digest(), different_normal.digest());

    let mut different_authority = original.clone();
    different_authority.binding.authority_batch = different_authority
        .binding
        .authority_batch
        .testing_with_binding_digest(
            different_authority.binding.authority_batch.binding_digest() ^ 1,
        );
    different_authority.digest = super::post_commit_settlement_trace_digest(&different_authority);
    assert_ne!(original.digest(), different_authority.digest());
}

#[test]
fn post_commit_compound_completion_preparation_returns_whole_inputs_on_mismatch() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        Normal,
        Authority,
        Host,
    }

    for drift in [Drift::Normal, Drift::Authority, Drift::Host] {
        let (mut engine, _request, normal, authority, batch, physical_authority) =
            active_post_commit_compound_fixture();
        let (batch, _) =
            record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
        let batch_projection = batch.projection();
        let mut inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
        match drift {
            Drift::Normal => {
                inputs.normal_facts.completion_digest =
                    CleanupCompletionDigest::from_opaque(0x1_bad);
            }
            Drift::Authority => {
                inputs.authority.completion_digest = CleanupCompletionDigest::from_opaque(0xa_bad);
            }
            Drift::Host => {
                inputs.host_edge.expected.receiver.stage =
                    CorePostCommitCleanupStageProjection::AuthorityAccepted;
                inputs.host_edge.physical_digest ^= 1;
            }
        }
        let returned_normal_owner = inputs.normal.token().source_projection();
        let returned_normal_completion_digest = inputs.normal.completion_digest();
        let returned_normal_settled_count = inputs.normal.settled_count();
        let returned_normal = inputs.normal_facts.clone();
        let returned_authority_expected = inputs.authority_expected.clone();
        let returned_authority = inputs.authority.clone();
        let returned_host = inputs.host_edge.clone();

        let PostCommitCompoundCompletionPreparationOutcome::Mismatch {
            batch,
            mut inputs,
            evidence: CorePostCommitCompoundMismatchEvidence::CompletionFacts { expected, observed },
        } = engine.prepare_post_commit_compound_completion(batch, inputs)
        else {
            panic!("each independently drifted L/A/H row must retain the whole source")
        };
        assert_eq!(batch.projection(), batch_projection);
        assert_eq!(
            inputs.normal.token().source_projection(),
            returned_normal_owner
        );
        assert_eq!(
            inputs.normal.completion_digest(),
            returned_normal_completion_digest
        );
        assert_eq!(inputs.normal.settled_count(), returned_normal_settled_count);
        assert_eq!(inputs.normal_facts, returned_normal);
        assert_eq!(inputs.authority_expected, returned_authority_expected);
        assert_eq!(inputs.authority, returned_authority);
        assert_eq!(inputs.host_edge, returned_host);
        match drift {
            Drift::Normal => {
                assert_eq!(expected.normal.batch, observed.normal.batch);
                assert_eq!(
                    expected.normal.manifest_digest,
                    observed.normal.manifest_digest
                );
                assert_eq!(expected.normal.settled_count, observed.normal.settled_count);
                assert_ne!(
                    expected.normal.completion_digest,
                    observed.normal.completion_digest
                );
                assert_eq!(expected.authority, observed.authority);
                assert_eq!(expected.host_edge, observed.host_edge);
            }
            Drift::Authority => {
                assert_eq!(expected.normal, observed.normal);
                assert_eq!(expected.authority.batch, observed.authority.batch);
                assert_eq!(
                    expected.authority.settled_count,
                    observed.authority.settled_count
                );
                assert_ne!(
                    expected.authority.completion_digest,
                    observed.authority.completion_digest
                );
                assert_eq!(expected.host_edge, observed.host_edge);
            }
            Drift::Host => {
                assert_eq!(expected.normal, observed.normal);
                assert_eq!(expected.authority, observed.authority);
                assert_ne!(expected.host_edge, observed.host_edge);
            }
        }
        inputs.normal_facts = expected.normal.clone();
        inputs.authority_expected = expected.authority.clone();
        inputs.authority = expected.authority;
        inputs.host_edge = expected.host_edge;
        assert!(matches!(
            engine.prepare_post_commit_compound_completion(batch, inputs),
            PostCommitCompoundCompletionPreparationOutcome::Prepared(_)
        ));
        assert_eq!(
            engine.availability(),
            &crate::presentation::RouterAvailability::RecoveringAuthority
        );
    }
}

#[test]
fn post_commit_compound_completion_requires_exact_l_a_h_and_terminals_once() {
    let (mut engine, request, normal, authority, batch, physical_authority) =
        active_post_commit_compound_fixture();
    let (batch, _) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let owner = match engine.prepare_post_commit_compound_completion(batch, inputs) {
        PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) => owner,
        PostCommitCompoundCompletionPreparationOutcome::Mismatch { .. } => {
            panic!("the exact L/A/H facts must prepare the unique completion owner")
        }
    };
    let calls = std::cell::Cell::new(0usize);
    engine
        .current
        .as_mut()
        .expect("the prepared compound receiver remains current")
        .identity
        .branch_index += 1;
    let owner = match engine.complete_post_commit_authority_compound(owner, |_| {
        calls.set(calls.get() + 1);
        0xbad_u64
    }) {
        PostCommitCompoundCompletionCoreOutcome::Mismatch { owner, .. } => owner,
        PostCommitCompoundCompletionCoreOutcome::Terminal { .. } => {
            panic!("completion receiver drift must return the unique owner")
        }
    };
    assert_eq!(calls.get(), 0);
    engine
        .current
        .as_mut()
        .expect("the mismatched compound receiver remains current")
        .identity
        .branch_index -= 1;
    let outcome = engine.complete_post_commit_authority_compound(owner, |_| {
        calls.set(calls.get() + 1);
        0xcafe_u64
    });
    let PostCommitCompoundCompletionCoreOutcome::Terminal {
        physical,
        terminal,
        retired_host: _,
    } = outcome
    else {
        panic!("the exact completion owner must terminalize")
    };
    assert_eq!(physical, 0xcafe);
    assert_eq!(terminal.request, request);
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        engine.request(request).unwrap().status(),
        RequestStatus::Succeeded
    ));
}

#[test]
fn post_commit_old_receipt_entry_is_stale_after_handoff_without_poisoning_trace() {
    let (mut engine, _request, normal, _authority, batch, physical_authority) =
        active_post_commit_compound_fixture();
    let before = engine.snapshot();
    assert_eq!(
        engine.observe_cleanup_receipt(&normal, crate::trace::CleanupDisposition::Removed,),
        Err(CleanupError::Stale)
    );
    assert_eq!(engine.snapshot(), before);

    let (batch, _) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    let authority = batch.authority_batch.projection();
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let owner = match engine.prepare_post_commit_compound_completion(batch, inputs) {
        PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) => owner,
        PostCommitCompoundCompletionPreparationOutcome::Mismatch { .. } => {
            panic!("the stale legacy receipt must not poison the linear trace")
        }
    };
    assert!(matches!(
        engine.complete_post_commit_authority_compound(owner, |_| ()),
        PostCommitCompoundCompletionCoreOutcome::Terminal { .. }
    ));
}

#[test]
fn post_commit_completion_refreezes_full_trace_source_before_physical_callback() {
    let (mut engine, request, normal, authority, batch, physical_authority) =
        active_post_commit_compound_fixture();
    let (batch, _) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let owner = match engine.prepare_post_commit_compound_completion(batch, inputs) {
        PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) => owner,
        PostCommitCompoundCompletionPreparationOutcome::Mismatch { .. } => {
            panic!("the exact full settlement source must prepare")
        }
    };
    let frozen_owner_source = owner.source.clone();
    let frozen_owner_trace = owner.batch.settlement_projection();
    RouterEngine::append_cleanup_settlement_trace_record(
        &mut engine.records,
        request,
        Some(crate::trace::CleanupDisposition::AlreadyAbsent),
    )
    .expect("the test models post-prepare live trace drift through the real trace writer");
    let live_trace = engine.request(request).unwrap().trace().to_vec();
    let live_capability = engine.pending_post_commit_settlement_trace.clone();
    let calls = std::cell::Cell::new(0usize);
    let PostCommitCompoundCompletionCoreOutcome::Mismatch {
        owner,
        evidence: CorePostCommitCompoundMismatchEvidence::Source { expected, observed },
    } = engine.complete_post_commit_authority_compound(owner, |_| {
        calls.set(calls.get() + 1);
    })
    else {
        panic!("post-prepare live trace drift must fail as typed Source mismatch")
    };
    assert_eq!(calls.get(), 0);
    assert_eq!(expected, frozen_owner_source);
    assert_ne!(expected, observed);
    assert_eq!(owner.source, frozen_owner_source);
    assert_eq!(owner.batch.settlement_projection(), frozen_owner_trace);
    assert_eq!(engine.pending_post_commit_settlement_trace, live_capability);
    assert_eq!(engine.request(request).unwrap().trace(), live_trace);
}

#[test]
fn host_loss_terminal_publishes_stable_unavailable_host_lost() {
    let (mut engine, mut issuer, request, normal, prepared, authority) =
        prepared_post_commit_authority_fixture();
    let former_host = engine
        .state
        .host()
        .cloned()
        .expect("the post-commit HostLost source starts with an exact host");
    let former_top_slot = engine
        .state
        .top_slot()
        .expect("the post-commit HostLost source starts with an exact top slot");
    let history = engine.history_snapshot();
    let queued = engine
        .push(RouteLocation::new("/items/2"))
        .expect("work submitted during post-commit cleanup must remain queued");
    let installed = std::cell::RefCell::new(None);
    assert!(matches!(
        engine.begin_post_commit_authority_transfer(
            &mut issuer,
            prepared,
            (),
            |(), _, plan| {
                let prior_trace_start = plan.current_trace.len();
                let (plan, physical_authority) =
                    bind_test_settlement_plan_with_authority(plan, prior_trace_start, Vec::new());
                Ok::<_, ((), ())>((((), physical_authority), plan))
            },
            |((), physical_authority), batch| {
                *installed.borrow_mut() = Some((batch, physical_authority));
            },
        ),
        PostCommitAuthorityBeginCoreOutcome::Begun
    ));
    let (batch, physical_authority) = installed
        .into_inner()
        .expect("the exact compound owner must be installed once");
    let expected_tombstones = vec![
        normal.source_projection(),
        batch.authority_batch.source_projection(),
    ];
    let (batch, _) =
        record_exact_post_commit_settlement_trace(&mut engine, batch, &physical_authority);
    let inputs = exact_post_commit_completion_inputs(&normal, &authority, &batch);
    let owner = match engine.prepare_post_commit_compound_completion(batch, inputs) {
        PostCommitCompoundCompletionPreparationOutcome::Prepared(owner) => owner,
        PostCommitCompoundCompletionPreparationOutcome::Mismatch { .. } => {
            panic!("the exact completion facts must prepare")
        }
    };
    let outcome = engine.complete_post_commit_authority_compound(owner, |_| ());
    assert!(matches!(
        outcome,
        PostCommitCompoundCompletionCoreOutcome::Terminal { .. }
    ));
    assert!(engine.pending_post_commit_terminal.is_none());
    assert!(engine.pending_post_commit_settlement_trace.is_none());

    assert_eq!(engine.history_snapshot(), history);
    assert_eq!(
        engine.request(request).unwrap().status(),
        &RequestStatus::Succeeded
    );
    assert!(matches!(
        engine.request(queued).unwrap().status(),
        RequestStatus::Failed(failure)
            if failure.kind() == NavigationFailureKind::HostLost
    ));
    assert_eq!(engine.active().kind(), ActivePresentationKind::None);
    assert_eq!(
        engine.availability(),
        &crate::presentation::RouterAvailability::Unavailable(
            crate::presentation::RouterUnavailableReason::HostLost
        )
    );
    let diagnostics = engine.request(request).unwrap().diagnostics();
    assert_eq!(
        diagnostics
            .iter()
            .filter(|event| event.kind()
                == crate::diagnostics::DiagnosticEventKind::PostCommitCleanupAnomaly)
            .count(),
        1
    );
    assert_eq!(
        diagnostics
            .iter()
            .find(|event| event.kind()
                == crate::diagnostics::DiagnosticEventKind::PostCommitCleanupAnomaly)
            .unwrap()
            .severity(),
        crate::diagnostics::DiagnosticSeverity::Severe
    );
    let router_diagnostics = engine
        .router_diagnostics()
        .iter()
        .filter(|event| {
            event.kind() == crate::diagnostics::DiagnosticEventKind::PostCommitCleanupAnomaly
        })
        .collect::<Vec<_>>();
    assert_eq!(router_diagnostics.len(), 1);
    assert_eq!(
        router_diagnostics[0].severity(),
        crate::diagnostics::DiagnosticSeverity::Severe
    );
    assert_eq!(
        router_diagnostics[0],
        diagnostics
            .iter()
            .find(|event| {
                event.kind() == crate::diagnostics::DiagnosticEventKind::PostCommitCleanupAnomaly
            })
            .unwrap()
    );

    let trace = engine.request(request).unwrap().trace();
    let aggregate_cleanup = trace
        .iter()
        .rposition(|event| {
            event.kind() == NavigationTraceEventKind::CleanupSettled
                && event.cleanup_disposition().is_none()
        })
        .expect("compound completion publishes its aggregate cleanup event");
    let request_terminal = trace
        .iter()
        .position(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
        .expect("compound completion publishes its request terminal event");
    assert!(aggregate_cleanup < request_terminal);
    assert_eq!(
        trace
            .iter()
            .filter(|event| event.kind() == NavigationTraceEventKind::RequestTerminal)
            .count(),
        1
    );

    let exact_tombstones = engine
        .completed_cleanup
        .iter()
        .filter(|tombstone| tombstone.request == request)
        .map(|tombstone| tombstone.token.source_projection())
        .collect::<Vec<_>>();
    assert_eq!(exact_tombstones, expected_tombstones);

    let source = match engine.issue_host_admission_source_permit() {
        HostAdmissionSourcePermitOutcome::Ineligible { source, evidence } => {
            assert_eq!(evidence.expected, source);
            assert_eq!(evidence.observed.as_ref(), Some(&source));
            source
        }
        HostAdmissionSourcePermitOutcome::Issued(_) => {
            panic!("an unchanged HostLost source must not issue a new permit")
        }
        HostAdmissionSourcePermitOutcome::Outstanding { .. } => {
            panic!("HostLost terminal must not retain an outstanding admission owner")
        }
    };
    assert_eq!(
        (
            source.availability_incarnation.0 > 0,
            source.kind,
            source.public_reason,
            source.host.as_ref(),
            source.top_slot,
        ),
        (
            true,
            HostAdmissionSourceKind::RetryableHostLost,
            Some(crate::presentation::RouterUnavailableReason::HostLost),
            Some(&former_host),
            Some(former_top_slot),
        )
    );
}
