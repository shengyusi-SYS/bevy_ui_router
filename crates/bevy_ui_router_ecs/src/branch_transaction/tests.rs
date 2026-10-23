use super::*;
use crate::navigation::tests::failed_nested_branch_fixture;

fn reconciliation_authority(branch: &RealizedBranch) -> ReconciliationAuthorityIdentity {
    let root = branch.frames().first().expect("fixture has a root").frame();
    ReconciliationAuthorityIdentity::from_frame(root, root.host(), root.slot())
        .expect("root binds its own host and top slot")
}

fn active_sparse_branch_fixture() -> RealizedBranch {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    branch.pop_frame_for_test();
    transition_slot_claims(&mut branch, EntityLifecycle::PreparedInert);
    branch
}

#[test]
fn leaf_retention_uses_exact_leaving_identity_and_returns_original_owners_on_reject() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let parent = branch.frames()[0].frame().clone();
    let leaf = branch.frames()[1].frame().clone();
    let original = branch.claims().cloned().collect::<Vec<_>>();
    let existing =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let splice = SealedExistingReconciliation::prepare(branch, existing)
        .unwrap()
        .bind_entering(PreparedBranch::empty())
        .unwrap();
    assert!(splice.view().instance_retention_is_exact(&leaf));
    assert!(!splice.view().instance_retention_is_exact(&parent));
    let (restored, entering, _) = splice.reject().into_parts();
    assert!(entering.frames().is_empty());
    assert_eq!(restored.claims().cloned().collect::<Vec<_>>(), original);
}

#[test]
fn retained_disposal_keeps_external_active_mount_alias_and_rejects_stale_owned_claim() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let leaf = branch.frames.pop().unwrap();
    let mut retained_branch = RealizedBranch::from_single_frame(leaf);
    transition_frame_claims(&mut retained_branch, EntityLifecycle::Retained);
    let mut retained =
        InstancePayload::from_frame_for_test(retained_branch.into_single_frame().unwrap().unwrap());
    assert_eq!(
        retained.frames()[0]
            .mount_binding()
            .slot_claim()
            .lifecycle(),
        EntityLifecycle::Active
    );
    assert!(retained.retained_leaving_mutation_targets().is_some());
    let projected = retained
        .claims()
        .map(|claim| claim.with_lifecycle_for_test(EntityLifecycle::Leaving))
        .collect::<Vec<_>>();
    let mut stale = projected.clone();
    stale[0] = stale[0].with_seal_for_test(branch.frames()[0].claims().next().unwrap().seal());
    assert!(
        retained
            .prepare_retained_claim_rebind(&stale, EntityLifecycle::Leaving)
            .is_none()
    );
    let rebind = retained
        .prepare_retained_claim_rebind(&projected, EntityLifecycle::Leaving)
        .unwrap();
    retained.apply_claim_rebind(rebind);
    assert!(
        retained
            .claims()
            .all(|claim| claim.lifecycle() == EntityLifecycle::Leaving)
    );
    assert_eq!(
        retained.frames()[0]
            .mount_binding()
            .slot_claim()
            .lifecycle(),
        EntityLifecycle::Active
    );
    assert!(retained.retained_leaving_mutation_targets().is_none());
}

#[test]
fn active_sparse_leaving_accepts_active_frames_and_prepared_empty_slots() {
    let branch = active_sparse_branch_fixture();
    let targets = branch
        .active_sparse_leaving_mutation_targets(EntityLifecycle::Leaving)
        .expect("active sparse branch must retain its mixed frame and empty-slot roles");
    assert_eq!(targets.len(), branch.obligation_count());

    let resulting = branch
        .lifecycle_transitions(EntityLifecycle::Leaving, true)
        .into_iter()
        .map(|(claim, next)| claim.with_lifecycle_for_test(next))
        .collect::<Vec<_>>();
    assert!(
        branch
            .prepare_active_sparse_claim_rebind(&resulting, EntityLifecycle::Leaving)
            .is_some()
    );
}

#[test]
fn active_sparse_leaving_rejects_prepared_frame_authority() {
    let mut branch = active_sparse_branch_fixture();
    transition_frame_claims(&mut branch, EntityLifecycle::PreparedInert);
    assert!(
        branch
            .active_sparse_leaving_mutation_targets(EntityLifecycle::Leaving)
            .is_none()
    );

    let resulting = branch
        .lifecycle_transitions(EntityLifecycle::Leaving, true)
        .into_iter()
        .map(|(claim, next)| claim.with_lifecycle_for_test(next))
        .collect::<Vec<_>>();
    assert!(
        branch
            .prepare_active_sparse_claim_rebind(&resulting, EntityLifecycle::Leaving)
            .is_none()
    );
}

#[test]
fn retained_prepared_prefix_keeps_exact_parent_owned_empty_slot() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let root = branch.frames()[0].frame().clone();
    let mut failed = branch;

    assert!(failed.retain_prepared_prefix(std::slice::from_ref(&root)));
    let retained = failed
        .retained_prepared_prefix()
        .expect("the exact root prefix remains linearly retained");
    assert_eq!(retained.frames().len(), 1);
    assert_eq!(retained.frames()[0].frame(), &root);
    assert_eq!(retained.child_slots().len(), 1);
    assert_eq!(retained.child_slots()[0].parent(), &root);
    assert!(failed.child_slots().is_empty());
    assert_eq!(failed.frames().len(), 1);
}

fn activate_branch(app: &mut bevy_app::App, branch: &mut RealizedBranch) {
    let current = branch.claims().cloned().collect::<Vec<_>>();
    let next = current
        .iter()
        .map(|claim| {
            app.world_mut()
                .resource_mut::<crate::registry::RouteEntityRegistry>()
                .compare_exchange_lifecycle(claim, EntityLifecycle::Active)
                .expect("fixture claim remains exact")
        })
        .collect::<Vec<_>>();
    let prepared = branch
        .prepare_claim_rebind(
            &next,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        )
        .expect("all branch aliases rebind atomically");
    branch.apply_claim_rebind(prepared);
}

fn transition_frame_claims(branch: &mut RealizedBranch, next: EntityLifecycle) {
    let rebound = branch
        .frames
        .iter()
        .flat_map(RealizedFrame::claims)
        .map(|claim| claim.with_lifecycle_for_test(next))
        .collect::<Vec<_>>();
    let mut offset = 0;
    for frame in &mut branch.frames {
        let end = offset + frame.claims().len();
        frame.apply_claim_rebind(&rebound[offset..end], &rebound);
        offset = end;
    }
    for slot in &mut branch.child_slots {
        slot.apply_internal_claim_rebind(&rebound);
    }
}

fn transition_slot_claims(branch: &mut RealizedBranch, next: EntityLifecycle) {
    let rebound = branch
        .child_slots
        .iter()
        .map(|slot| slot.slot_claim().with_lifecycle_for_test(next))
        .collect::<Vec<_>>();
    for slot in &mut branch.child_slots {
        slot.apply_internal_claim_rebind(&rebound);
    }
    for frame in &mut branch.frames {
        let primary = frame.claims().cloned().collect::<Vec<_>>();
        frame.apply_claim_rebind(&primary, &rebound);
    }
}

fn transition_detached_frame_claims(
    frame: &mut RealizedFrame,
    next: EntityLifecycle,
    aliases: &[ExactRegistryClaim],
) {
    let primary = frame
        .claims()
        .map(|claim| claim.with_lifecycle_for_test(next))
        .collect::<Vec<_>>();
    let mut all = primary.clone();
    all.extend_from_slice(aliases);
    frame.apply_claim_rebind(&primary, &all);
}

fn reconciliation_existing_projection(
    branch: &RealizedBranch,
    slot_role: ReconciliationSlotRoleKind,
) -> ReconciliationExistingProjection {
    let parent = branch.frames()[0].frame().clone();
    let child = branch.frames()[1].frame().clone();
    let slot = &branch.child_slots()[0];
    ReconciliationExistingProjection {
        preserved: vec![ReconciliationExistingFrame {
            branch_index: 0,
            version: parent.clone(),
        }],
        leaving: vec![ReconciliationExistingFrame {
            branch_index: 1,
            version: child.clone(),
        }],
        child_slots: vec![ReconciliationExistingSlot {
            parent_branch_index: 0,
            parent,
            outlet: slot.outlet().clone(),
            slot: slot.slot_id(),
            child: Some(child),
            role: slot_role,
        }],
    }
}

#[test]
fn reconciliation_preserved_active_keeps_exact_boundary_slot() {
    let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::PreservedActive);

    let sealed = SealedExistingReconciliation::prepare(branch, projection)
        .expect("exact common-prefix boundary must seal");

    assert_eq!(sealed.preserved().frames().len(), 1);
    assert_eq!(sealed.leaving().frames().len(), 1);
    assert_eq!(sealed.preserved().child_slots()[0].slot(), slot_entity);
    assert_eq!(
        sealed.slot_roles()[0].kind(),
        ReconciliationSlotRoleKind::PreservedActive
    );
}

#[test]
fn reconciliation_retain_empty_truncate_keeps_registered_slot_out_of_leaving() {
    let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);

    let sealed = SealedExistingReconciliation::prepare(branch, projection)
        .expect("exact truncate boundary must seal");
    let splice = sealed
        .bind_entering(PreparedBranch::empty())
        .expect("truncate has no entering suffix");
    let outcome = splice.apply();
    assert_eq!(outcome.preserved().frames().len(), 1);
    assert_eq!(outcome.preserved().child_slots()[0].slot(), slot_entity);
    assert!(outcome.leaving().child_slots().is_empty());
    assert_eq!(outcome.leaving().frames().len(), 1);
}

#[test]
fn reconciliation_existing_reject_restores_old_active_and_role_owners() {
    let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let original_frames = branch
        .frames()
        .iter()
        .map(|frame| frame.frame().clone())
        .collect::<Vec<_>>();
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let sealed = SealedExistingReconciliation::prepare(branch, projection)
        .expect("exact truncate boundary must seal");

    let rejected = sealed.reject();
    let (restored, pending_entering, roles) = rejected.into_parts();
    assert_eq!(
        restored
            .frames()
            .iter()
            .map(|frame| frame.frame())
            .collect::<Vec<_>>(),
        original_frames.iter().collect::<Vec<_>>()
    );
    assert_eq!(restored.child_slots().len(), 1);
    assert!(pending_entering.frames().is_empty());
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].entity(), slot_entity);
    assert_eq!(roles[0].kind(), ReconciliationSlotRoleKind::RetainEmpty);
}

#[test]
fn reconciliation_role_rebind_keeps_preserved_primary_and_updates_mixed_aliases() {
    let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let mut splice = SealedExistingReconciliation::prepare(branch, projection)
        .expect("exact truncate boundary must seal")
        .bind_entering(PreparedBranch::empty())
        .expect("truncate has no entering suffix");
    let preserved_primary_before = splice.preserved().frames()[0]
        .claims()
        .cloned()
        .collect::<Vec<_>>();

    let transitions = splice.lifecycle_transitions();
    assert!(transitions.iter().all(|(claim, next)| {
        if claim.entity() == slot_entity {
            claim.lifecycle() == EntityLifecycle::Active && *next == EntityLifecycle::PreparedInert
        } else {
            claim.lifecycle() == EntityLifecycle::Active && *next == EntityLifecycle::Leaving
        }
    }));
    let resulting = transitions
        .iter()
        .map(|(claim, next)| claim.with_lifecycle_for_test(*next))
        .collect::<Vec<_>>();
    let prepared = splice
        .prepare_claim_rebind(&resulting)
        .expect("mixed role claims and aliases must prepare atomically");
    splice.apply_claim_rebind(prepared);

    assert_eq!(
        splice.preserved().frames()[0]
            .claims()
            .cloned()
            .collect::<Vec<_>>(),
        preserved_primary_before
    );
    assert_eq!(
        splice.preserved().child_slots()[0].slot_claim().lifecycle(),
        EntityLifecycle::PreparedInert
    );
    assert!(
        splice.leaving().frames()[0]
            .claims()
            .all(|claim| claim.lifecycle() == EntityLifecycle::Leaving)
    );
    assert_eq!(
        splice.leaving().frames()[0]
            .mount_binding()
            .slot_claim()
            .lifecycle(),
        EntityLifecycle::PreparedInert
    );
}

#[test]
fn reconciliation_rejects_duplicate_and_cross_boundary_roles() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let branch = branch.into_realized_for_test();
    let mut duplicate =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    duplicate.leaving.push(duplicate.preserved[0].clone());
    assert!(SealedExistingReconciliation::prepare(branch, duplicate).is_err());

    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let branch = branch.into_realized_for_test();
    let mut crossing =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    crossing.preserved.clear();
    crossing.leaving.insert(
        0,
        ReconciliationExistingFrame {
            branch_index: 0,
            version: branch.frames()[0].frame().clone(),
        },
    );
    assert!(SealedExistingReconciliation::prepare(branch, crossing).is_err());
}

#[test]
fn reconciliation_existing_frames_must_all_be_active_and_return_branch_on_failure() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let branch = branch.into_realized_for_test();
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);

    let failure = SealedExistingReconciliation::prepare(branch, projection)
        .expect_err("prepared fixture frames cannot masquerade as existing active frames");
    let (branch, error) = failure.into_parts();

    assert_eq!(error, ReconciliationSpliceError::InvalidFramePartition);
    assert_eq!(branch.frames().len(), 2);
    assert_eq!(branch.child_slots().len(), 1);
}

#[test]
fn reconciliation_occupied_existing_slot_must_be_active() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    transition_frame_claims(&mut branch, EntityLifecycle::Active);
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);

    let failure = SealedExistingReconciliation::prepare(branch, projection)
        .expect_err("occupied PreparedInert slot cannot retain an active child");
    let (branch, error) = failure.into_parts();

    assert_eq!(error, ReconciliationSpliceError::InvalidSlotPartition);
    assert_eq!(
        branch.child_slots()[0].slot_claim().lifecycle(),
        EntityLifecycle::PreparedInert
    );
}

#[test]
fn reconciliation_empty_existing_slot_must_be_prepared_inert() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    branch.frames.pop().expect("fixture has one child frame");
    let parent = branch.frames()[0].frame().clone();
    let slot = &branch.child_slots()[0];
    let projection = ReconciliationExistingProjection {
        preserved: vec![ReconciliationExistingFrame {
            branch_index: 0,
            version: parent.clone(),
        }],
        leaving: Vec::new(),
        child_slots: vec![ReconciliationExistingSlot {
            parent_branch_index: 0,
            parent,
            outlet: slot.outlet().clone(),
            slot: slot.slot_id(),
            child: None,
            role: ReconciliationSlotRoleKind::RetainEmpty,
        }],
    };

    let failure = SealedExistingReconciliation::prepare(branch, projection)
        .expect_err("empty Active slot cannot masquerade as retained inert state");
    let (branch, error) = failure.into_parts();

    assert_eq!(error, ReconciliationSpliceError::InvalidSlotPartition);
    assert_eq!(
        branch.child_slots()[0].slot_claim().lifecycle(),
        EntityLifecycle::Active
    );
}

#[test]
fn reconciliation_entering_frames_must_all_be_prepared_and_return_inputs_on_failure() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut entering = branch.into_realized_for_test();
    transition_frame_claims(&mut entering, EntityLifecycle::Active);
    let root = entering.frames()[0].frame().clone();
    let child = entering.frames()[1].frame().clone();
    let slot = &entering.child_slots()[0];
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&entering),
        existing: ReconciliationExistingProjection {
            preserved: Vec::new(),
            leaving: Vec::new(),
            child_slots: Vec::new(),
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![
                ReconciliationEnteringFrame {
                    branch_index: 0,
                    origin: FrameOrigin::MatchedRoute(matched_route(&root)),
                    frame: root.frame(),
                    generation: root.generation(),
                    slot: ReconciliationEnteringSlot::Existing(root.slot()),
                },
                ReconciliationEnteringFrame {
                    branch_index: 1,
                    origin: FrameOrigin::MatchedRoute(matched_route(&child)),
                    frame: child.frame(),
                    generation: child.generation(),
                    slot: ReconciliationEnteringSlot::MountNew,
                },
            ],
            mount_new_slots: vec![ReconciliationMountNewSlot {
                parent_branch_index: 0,
                outlet: slot.outlet().clone(),
            }],
        },
    };
    let sealed =
        SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
            .expect("empty existing partition seals independently");

    let failure = sealed
        .bind_entering(PreparedBranch::from_realized(entering))
        .expect_err("Active frames cannot masquerade as prepared entering frames");
    let (sealed, entering, error) = failure.into_parts();

    assert_eq!(error, ReconciliationSpliceError::InvalidEnteringBranch);
    assert!(sealed.preserved().frames().is_empty());
    assert_eq!(entering.frames().len(), 2);
}

#[test]
fn reconciliation_entering_child_slots_must_all_be_prepared_inert() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut entering = branch.into_realized_for_test();
    transition_slot_claims(&mut entering, EntityLifecycle::Active);
    let root = entering.frames()[0].frame().clone();
    let child = entering.frames()[1].frame().clone();
    let slot = &entering.child_slots()[0];
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&entering),
        existing: ReconciliationExistingProjection {
            preserved: Vec::new(),
            leaving: Vec::new(),
            child_slots: Vec::new(),
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![
                ReconciliationEnteringFrame {
                    branch_index: 0,
                    origin: FrameOrigin::MatchedRoute(matched_route(&root)),
                    frame: root.frame(),
                    generation: root.generation(),
                    slot: ReconciliationEnteringSlot::Existing(root.slot()),
                },
                ReconciliationEnteringFrame {
                    branch_index: 1,
                    origin: FrameOrigin::MatchedRoute(matched_route(&child)),
                    frame: child.frame(),
                    generation: child.generation(),
                    slot: ReconciliationEnteringSlot::MountNew,
                },
            ],
            mount_new_slots: vec![ReconciliationMountNewSlot {
                parent_branch_index: 0,
                outlet: slot.outlet().clone(),
            }],
        },
    };
    let sealed =
        SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
            .expect("empty existing partition seals independently");

    let failure = sealed
        .bind_entering(PreparedBranch::from_realized(entering))
        .expect_err("Active entering child slot cannot masquerade as MountNew");
    let (_, entering, error) = failure.into_parts();

    assert_eq!(error, ReconciliationSpliceError::InvalidEnteringBranch);
    assert_eq!(
        entering.as_realized().child_slots()[0]
            .slot_claim()
            .lifecycle(),
        EntityLifecycle::Active
    );
}

#[test]
fn reconciliation_boundary_mount_rejects_foreign_anchor_alias_and_returns_slot() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    let child = branch.frames.pop().expect("fixture has one child frame");
    let mut slot = branch
        .child_slots
        .pop()
        .expect("fixture has one boundary slot");
    let parent = &mut branch.frames[0];
    let primary = parent.claims().cloned().collect::<Vec<_>>();
    let foreign_anchor = slot
        .anchor_claim()
        .with_seal_for_test(child.claims().next().expect("child has claims").seal());
    parent.apply_claim_rebind(&primary, std::slice::from_ref(&foreign_anchor));
    slot.apply_internal_claim_rebind(std::slice::from_ref(&foreign_anchor));
    assert_eq!(
        parent.outlet(slot.outlet()).unwrap().anchor_claim(),
        &foreign_anchor
    );
    assert_eq!(slot.anchor_claim(), &foreign_anchor);
    assert!(!parent.claims().any(|claim| claim == &foreign_anchor));

    let mut entering = PreparedBranch::empty();
    let rejected = entering
        .push_reconciliation_child_slot(slot, parent)
        .expect_err("matching foreign aliases are not preserved-parent authority");

    assert_eq!(rejected.slot().anchor_claim(), &foreign_anchor);
    assert!(entering.as_realized().child_slots().is_empty());
}

fn matched_route(frame: &RouteFrameVersion) -> RouteId {
    match frame.origin() {
        FrameOrigin::MatchedRoute(route) => *route,
        _ => panic!("nested fixture must contain matched routes"),
    }
}

#[test]
fn reconciliation_append_activates_exact_retained_empty_slot() {
    let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let truncate =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let mut truncate =
        SealedExistingReconciliation::prepare(branch, truncate).expect("truncate must seal");
    let mut child = truncate.leaving.frames.pop().expect("one leaving child");
    let truncate = truncate
        .bind_entering(PreparedBranch::empty())
        .expect("truncate has no entering suffix")
        .apply();
    let (mut active, leaving) = truncate.into_active_and_leaving();
    assert!(leaving.frames().is_empty());
    transition_slot_claims(&mut active, EntityLifecycle::PreparedInert);
    let slot_claim = active.child_slots()[0].slot_claim().clone();
    transition_detached_frame_claims(
        &mut child,
        EntityLifecycle::PreparedInert,
        std::slice::from_ref(&slot_claim),
    );
    let parent = active.frames()[0].frame().clone();
    let child_version = child.frame().clone();
    let retained = &active.child_slots()[0];
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&active),
        existing: ReconciliationExistingProjection {
            preserved: vec![ReconciliationExistingFrame {
                branch_index: 0,
                version: parent.clone(),
            }],
            leaving: Vec::new(),
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent,
                outlet: retained.outlet().clone(),
                slot: retained.slot_id(),
                child: None,
                role: ReconciliationSlotRoleKind::ActivateExisting,
            }],
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![ReconciliationEnteringFrame {
                branch_index: 1,
                origin: FrameOrigin::MatchedRoute(matched_route(&child_version)),
                frame: child_version.frame(),
                generation: child_version.generation(),
                slot: ReconciliationEnteringSlot::Existing(retained.slot_id()),
            }],
            mount_new_slots: Vec::new(),
        },
    };
    let sealed = SealedExistingReconciliation::prepare_from_projection(active, facts)
        .expect("retained empty slot remains exact");
    let mut entering = PreparedBranch::empty();
    entering
        .push_frame(child)
        .expect("entering suffix stores its physical frame");

    let outcome = sealed
        .bind_entering(entering)
        .expect("existing slot and entering mount binding must agree")
        .apply();
    let (active, _) = outcome.into_active_and_leaving();

    assert_eq!(active.frames().len(), 2);
    assert_eq!(active.child_slots().len(), 1);
    assert_eq!(active.child_slots()[0].slot(), slot_entity);
}

#[test]
fn reconciliation_mount_new_accepts_only_exact_mounted_slot_binding() {
    let (_app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let entering = branch.into_realized_for_test();
    let root = entering.frames()[0].frame().clone();
    let child = entering.frames()[1].frame().clone();
    let slot = &entering.child_slots()[0];
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&entering),
        existing: ReconciliationExistingProjection {
            preserved: Vec::new(),
            leaving: Vec::new(),
            child_slots: Vec::new(),
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![
                ReconciliationEnteringFrame {
                    branch_index: 0,
                    origin: FrameOrigin::MatchedRoute(matched_route(&root)),
                    frame: root.frame(),
                    generation: root.generation(),
                    slot: ReconciliationEnteringSlot::Existing(root.slot()),
                },
                ReconciliationEnteringFrame {
                    branch_index: 1,
                    origin: FrameOrigin::MatchedRoute(matched_route(&child)),
                    frame: child.frame(),
                    generation: child.generation(),
                    slot: ReconciliationEnteringSlot::MountNew,
                },
            ],
            mount_new_slots: vec![ReconciliationMountNewSlot {
                parent_branch_index: 0,
                outlet: slot.outlet().clone(),
            }],
        },
    };
    let sealed =
        SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
            .expect("empty active partition must seal");

    let outcome = sealed
        .bind_entering(PreparedBranch::from_realized(entering))
        .expect("exact mounted slot and frame binding must agree")
        .apply();

    let (active, leaving) = outcome.into_active_and_leaving();
    assert_eq!(active.child_slots()[0].slot(), slot_entity);
    assert!(leaving.frames().is_empty());
}

#[test]
fn reconciliation_splice_reject_returns_independent_entering_and_role_owners() {
    let (_app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let entering = branch.into_realized_for_test();
    let root = entering.frames()[0].frame().clone();
    let child = entering.frames()[1].frame().clone();
    let slot = &entering.child_slots()[0];
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&entering),
        existing: ReconciliationExistingProjection {
            preserved: Vec::new(),
            leaving: Vec::new(),
            child_slots: Vec::new(),
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![
                ReconciliationEnteringFrame {
                    branch_index: 0,
                    origin: FrameOrigin::MatchedRoute(matched_route(&root)),
                    frame: root.frame(),
                    generation: root.generation(),
                    slot: ReconciliationEnteringSlot::Existing(root.slot()),
                },
                ReconciliationEnteringFrame {
                    branch_index: 1,
                    origin: FrameOrigin::MatchedRoute(matched_route(&child)),
                    frame: child.frame(),
                    generation: child.generation(),
                    slot: ReconciliationEnteringSlot::MountNew,
                },
            ],
            mount_new_slots: vec![ReconciliationMountNewSlot {
                parent_branch_index: 0,
                outlet: slot.outlet().clone(),
            }],
        },
    };
    let sealed =
        SealedExistingReconciliation::prepare_from_projection(RealizedBranch::empty(), facts)
            .expect("empty active partition must seal");
    let splice = sealed
        .bind_entering(PreparedBranch::from_realized(entering))
        .expect("exact mounted slot and frame binding must agree");

    let rejected = splice.reject();
    let (restored, pending_entering, roles) = rejected.into_parts();
    assert!(restored.frames().is_empty());
    assert_eq!(pending_entering.frames().len(), 2);
    assert_eq!(pending_entering.child_slots().len(), 1);
    assert_eq!(pending_entering.child_slots()[0].slot(), slot_entity);
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].entity(), slot_entity);
    assert_eq!(roles[0].kind(), ReconciliationSlotRoleKind::MountNew);
}

#[test]
fn reconciliation_mount_new_slot_only_failure_freezes_one_cleanup_obligation() {
    let (app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut realized = branch.into_realized_for_test();
    let mounted = realized
        .child_slots
        .pop()
        .expect("nested fixture has one mounted child slot");
    let parent = &realized.frames[0];
    let mut pending = PreparedBranch::empty();
    pending
        .push_reconciliation_child_slot(mounted, parent)
        .expect("MountNew boundary slot binds the preserved parent");
    let failed = FailedPreparedBranch::new(pending);

    let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
        app.world(),
        app.world()
            .resource::<crate::registry::RouteEntityRegistry>(),
        &failed,
    )
    .expect("slot-only MountNew residual is an independent cleanup owner");
    assert_eq!(manifest.len(), 1);
    assert_eq!(failed.child_slots()[0].slot(), slot_entity);
}

#[test]
fn reconciliation_mount_new_boundary_accepts_exact_preserved_parent() {
    let (_app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    let mut child = branch.frames.pop().expect("one entering child");
    let mut mounted = branch.child_slots.pop().expect("one boundary slot");
    transition_frame_claims(&mut branch, EntityLifecycle::Active);
    let parent_claims = branch.frames()[0].claims().cloned().collect::<Vec<_>>();
    mounted.apply_internal_claim_rebind(&parent_claims);
    let child_claims = child.claims().cloned().collect::<Vec<_>>();
    child.apply_claim_rebind(&child_claims, &parent_claims);
    let parent = branch.frames()[0].frame().clone();
    let child_version = child.frame().clone();
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&branch),
        existing: ReconciliationExistingProjection {
            preserved: vec![ReconciliationExistingFrame {
                branch_index: 0,
                version: parent,
            }],
            leaving: Vec::new(),
            child_slots: Vec::new(),
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![ReconciliationEnteringFrame {
                branch_index: 1,
                origin: FrameOrigin::MatchedRoute(matched_route(&child_version)),
                frame: child_version.frame(),
                generation: child_version.generation(),
                slot: ReconciliationEnteringSlot::MountNew,
            }],
            mount_new_slots: vec![ReconciliationMountNewSlot {
                parent_branch_index: 0,
                outlet: mounted.outlet().clone(),
            }],
        },
    };
    let sealed = SealedExistingReconciliation::prepare_from_projection(branch, facts)
        .expect("preserved parent must seal");
    let mut entering = PreparedBranch::empty();
    entering
        .push_reconciliation_child_slot(mounted, &sealed.preserved().frames()[0])
        .expect("exact preserved parent admits its new boundary slot");
    entering
        .push_frame(child)
        .expect("mounted boundary slot admits its entering child");

    let mut splice = sealed
        .bind_entering(entering)
        .expect("boundary MountNew binding must remain exact");
    assert_eq!(splice.preserved().frames().len(), 1);
    assert_eq!(splice.entering().frames().len(), 1);
    let mount_new = splice
        .slot_roles()
        .iter()
        .find(|role| role.kind() == ReconciliationSlotRoleKind::MountNew)
        .expect("boundary slot remains an entering obligation");
    assert_eq!(mount_new.entity(), slot_entity);
    assert_eq!(mount_new.claim().entity(), slot_entity);
    assert_eq!(mount_new.current(), EntityLifecycle::PreparedInert);
    assert_eq!(mount_new.next(), Some(EntityLifecycle::Active));
    let entering_slot = splice.entering().child_slots()[0].clone_for_test();
    assert_eq!(mount_new.claim(), entering_slot.slot_claim());
    assert_eq!(mount_new.parent(), entering_slot.parent());
    assert_eq!(mount_new.outlet(), entering_slot.outlet());
    assert_eq!(mount_new.slot(), entering_slot.slot_id());
    assert_eq!(mount_new.anchor(), entering_slot.anchor());
    assert_eq!(mount_new.anchor_claim(), entering_slot.anchor_claim());

    let transitions = splice.lifecycle_transitions();
    assert_eq!(
        transitions
            .iter()
            .filter(|(claim, next)| {
                claim.entity() == slot_entity && *next == EntityLifecycle::Active
            })
            .count(),
        1,
        "the sealed MountNew role owns exactly one PreparedInert-to-Active transition",
    );
    let resulting = transitions
        .iter()
        .map(|(claim, next)| claim.with_lifecycle_for_test(*next))
        .collect::<Vec<_>>();
    assert!(splice.prepare_claim_rebind(&resulting).is_some());

    assert!(crate::frame_local::sealed_mount_new_expectation_is_unique(
        splice.slot_roles(),
        &entering_slot,
    ));
    let wrong_role = sealed_slot_role(
        &entering_slot,
        ReconciliationSlotRoleKind::ActivateExisting,
        ReconciliationSlotEnteringUse::Required,
    );
    assert!(!crate::frame_local::sealed_mount_new_expectation_is_unique(
        std::slice::from_ref(&wrong_role),
        &entering_slot,
    ));
    let duplicate = sealed_slot_role(
        &entering_slot,
        ReconciliationSlotRoleKind::MountNew,
        ReconciliationSlotEnteringUse::Required,
    );
    let mut duplicated_roles = splice
        .slot_roles
        .drain(..)
        .collect::<Vec<SealedReconciliationSlotRole>>();
    duplicated_roles.push(duplicate);
    assert!(!crate::frame_local::sealed_mount_new_expectation_is_unique(
        &duplicated_roles,
        &entering_slot,
    ));
    splice.slot_roles = duplicated_roles;
    let duplicate_transitions = splice.lifecycle_transitions();
    let duplicate_resulting = duplicate_transitions
        .iter()
        .map(|(claim, next)| claim.with_lifecycle_for_test(*next))
        .collect::<Vec<_>>();
    assert!(
        splice.prepare_claim_rebind(&duplicate_resulting).is_none(),
        "duplicate role-owned transitions must fail closed before physical mutation",
    );
    let mut duplicated_roles = splice.slot_roles.drain(..).collect::<Vec<_>>();
    duplicated_roles.pop();
    splice.slot_roles = duplicated_roles;

    let outcome = splice.apply();

    assert_eq!(outcome.preserved().frames().len(), 1);
    assert_eq!(outcome.entering().frames().len(), 1);
    assert_eq!(outcome.entering().child_slots()[0].slot(), slot_entity);
    assert_eq!(
        outcome
            .slot_roles()
            .iter()
            .filter(|role| role.kind() == ReconciliationSlotRoleKind::MountNew)
            .count(),
        1
    );
}

#[test]
fn reconciliation_boundary_preserved_active_requires_entering_consumption() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::PreservedActive);
    let sealed = SealedExistingReconciliation::prepare(branch, projection)
        .expect("boundary role itself is exact");

    let failure = sealed
        .bind_entering(PreparedBranch::empty())
        .expect_err("boundary role cannot remain unconsumed");
    let (sealed, entering, error) = failure.into_parts();
    assert_eq!(error, ReconciliationSpliceError::InvalidEnteringBranch);
    assert_eq!(sealed.preserved().frames().len(), 1);
    assert_eq!(sealed.leaving().frames().len(), 1);
    assert!(entering.frames().is_empty());
}

#[test]
fn reconciliation_activate_existing_requires_entering_and_retain_empty_forbids_it() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let truncate =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let truncate = SealedExistingReconciliation::prepare(branch, truncate)
        .unwrap()
        .bind_entering(PreparedBranch::empty())
        .unwrap()
        .apply();
    let (mut active, mut leaving) = truncate.into_active_and_leaving();
    let mut child = leaving.frames.pop().unwrap();
    transition_slot_claims(&mut active, EntityLifecycle::PreparedInert);
    let slot_claim = active.child_slots()[0].slot_claim().clone();
    transition_detached_frame_claims(
        &mut child,
        EntityLifecycle::PreparedInert,
        std::slice::from_ref(&slot_claim),
    );
    let retained = &active.child_slots()[0];
    let parent = active.frames()[0].frame().clone();

    let activate = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&active),
        existing: ReconciliationExistingProjection {
            preserved: vec![ReconciliationExistingFrame {
                branch_index: 0,
                version: parent.clone(),
            }],
            leaving: Vec::new(),
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent: parent.clone(),
                outlet: retained.outlet().clone(),
                slot: retained.slot_id(),
                child: None,
                role: ReconciliationSlotRoleKind::ActivateExisting,
            }],
        },
        entering: ReconciliationEnteringProjection {
            frames: Vec::new(),
            mount_new_slots: Vec::new(),
        },
    };
    let failure = SealedExistingReconciliation::prepare_from_projection(active, activate)
        .unwrap()
        .bind_entering(PreparedBranch::empty())
        .expect_err("ActivateExisting must be consumed");
    let (sealed, _, _) = failure.into_parts();

    let retained_branch = sealed.preserved;
    let retained = &retained_branch.child_slots()[0];
    let child_version = child.frame().clone();
    let retain = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: reconciliation_authority(&retained_branch),
        existing: ReconciliationExistingProjection {
            preserved: vec![ReconciliationExistingFrame {
                branch_index: 0,
                version: parent.clone(),
            }],
            leaving: Vec::new(),
            child_slots: vec![ReconciliationExistingSlot {
                parent_branch_index: 0,
                parent,
                outlet: retained.outlet().clone(),
                slot: retained.slot_id(),
                child: None,
                role: ReconciliationSlotRoleKind::RetainEmpty,
            }],
        },
        entering: ReconciliationEnteringProjection {
            frames: vec![ReconciliationEnteringFrame {
                branch_index: 1,
                origin: FrameOrigin::MatchedRoute(matched_route(&child_version)),
                frame: child_version.frame(),
                generation: child_version.generation(),
                slot: ReconciliationEnteringSlot::Existing(retained.slot_id()),
            }],
            mount_new_slots: Vec::new(),
        },
    };
    let sealed =
        SealedExistingReconciliation::prepare_from_projection(retained_branch, retain).unwrap();
    let mut entering = PreparedBranch::empty();
    entering.push_frame(child).unwrap();
    assert!(sealed.bind_entering(entering).is_err());
}

#[test]
fn reconciliation_internal_preserved_active_forbids_entering_and_needs_no_consumption() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let root = branch.frames()[0].frame().clone();
    let child = branch.frames()[1].frame().clone();
    let slot = &branch.child_slots()[0];
    let projection = ReconciliationExistingProjection {
        preserved: vec![
            ReconciliationExistingFrame {
                branch_index: 0,
                version: root.clone(),
            },
            ReconciliationExistingFrame {
                branch_index: 1,
                version: child.clone(),
            },
        ],
        leaving: Vec::new(),
        child_slots: vec![ReconciliationExistingSlot {
            parent_branch_index: 0,
            parent: root,
            outlet: slot.outlet().clone(),
            slot: slot.slot_id(),
            child: Some(child),
            role: ReconciliationSlotRoleKind::PreservedActive,
        }],
    };
    let splice = SealedExistingReconciliation::prepare(branch, projection)
        .unwrap()
        .bind_entering(PreparedBranch::empty())
        .expect("internal preserved slot needs no entering consumer");
    assert_eq!(splice.preserved().frames().len(), 2);
    assert!(splice.entering().frames().is_empty());
}

#[test]
fn reconciliation_rejects_physical_duplicate_entity_owner_and_seal() {
    let (_app, branch, _, _, _) = failed_nested_branch_fixture();
    let branch = branch.into_realized_for_test();
    let claims = branch.frames()[0]
        .claims()
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 2);

    let duplicate_entity = claims[1].with_entity_for_test(claims[0].entity());
    assert_eq!(
        validate_unique_authorities(&[&claims[0], &duplicate_entity]),
        Err(ReconciliationSpliceError::DuplicateAuthority)
    );

    let duplicate_owner = claims[1].with_owner_for_test(claims[0].owner().clone());
    assert_eq!(
        validate_unique_authorities(&[&claims[0], &duplicate_owner]),
        Err(ReconciliationSpliceError::DuplicateAuthority)
    );

    let duplicate_seal = claims[1].with_seal_for_test(claims[0].seal());
    assert_eq!(
        validate_unique_authorities(&[&claims[0], &duplicate_seal]),
        Err(ReconciliationSpliceError::DuplicateAuthority)
    );
}

#[test]
fn reconciliation_realized_branch_rejects_duplicate_slot_id_and_parent_outlet() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    let original = &branch.child_slots()[0];
    let parent = original.parent().clone();
    let original_outlet = original.outlet().clone();
    let original_slot = original.slot_id();
    let mut duplicate_slot_id = original.clone_for_test();
    let mut duplicate_mapping = original.clone_for_test();
    let distinct_slot = branch.frames()[0].frame().slot();
    assert_ne!(original_slot, distinct_slot);
    let template = branch.frames()[0].claims().next().unwrap().clone();

    let entity = app.world_mut().spawn_empty().id();
    let different_outlet = RouteSlotKey::named("duplicate-id").unwrap();
    let owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
        parent: parent.clone(),
        outlet: different_outlet.clone(),
        slot: original_slot,
    });
    let claim = template
        .with_entity_for_test(entity)
        .with_owner_for_test(owner);
    duplicate_slot_id.replace_slot_authority_for_test(
        entity,
        claim,
        different_outlet,
        original_slot,
    );
    assert!(branch.push_child_slot(duplicate_slot_id).is_err());

    let entity = app.world_mut().spawn_empty().id();
    let owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
        parent,
        outlet: original_outlet.clone(),
        slot: distinct_slot,
    });
    let claim = template
        .with_entity_for_test(entity)
        .with_owner_for_test(owner);
    duplicate_mapping.replace_slot_authority_for_test(
        entity,
        claim,
        original_outlet,
        distinct_slot,
    );
    assert!(branch.push_child_slot(duplicate_mapping).is_err());
}

#[test]
fn reconciliation_rejects_foreign_router_and_wrong_top_slot_identity() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let (_foreign_app, foreign, _, _, _) = failed_nested_branch_fixture();
    let foreign = foreign.into_realized_for_test();
    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let foreign_authority = ReconciliationAuthorityIdentity::from_frame(
        foreign.frames()[0].frame(),
        branch.frames()[0].frame().host(),
        branch.frames()[0].frame().slot(),
    )
    .expect("fixture raw host ids may coincide while router provenance differs");
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: foreign_authority,
        existing: projection,
        entering: ReconciliationEnteringProjection {
            frames: Vec::new(),
            mount_new_slots: Vec::new(),
        },
    };
    let failure = SealedExistingReconciliation::prepare_from_projection(branch, facts)
        .expect_err("foreign opaque router identity must reject");
    let (branch, error) = failure.into_parts();
    assert_eq!(error, ReconciliationSpliceError::InvalidAuthorityIdentity);
    assert_eq!(branch.frames().len(), 2);

    let projection =
        reconciliation_existing_projection(&branch, ReconciliationSlotRoleKind::RetainEmpty);
    let root = branch.frames()[0].frame();
    let wrong_top = ReconciliationAuthorityIdentity::from_frame(
        root,
        root.host(),
        branch.child_slots()[0].slot_id(),
    )
    .unwrap();
    let facts = ReconciliationProjectionFacts {
        selected: Vec::new(),
        authority: wrong_top,
        existing: projection,
        entering: ReconciliationEnteringProjection {
            frames: Vec::new(),
            mount_new_slots: Vec::new(),
        },
    };
    assert!(SealedExistingReconciliation::prepare_from_projection(branch, facts).is_err());
}

#[test]
fn reconciliation_slot_role_lifecycle_pairs_are_closed() {
    assert_eq!(
        slot_role_next(
            ReconciliationSlotRoleKind::PreservedActive,
            EntityLifecycle::Active
        ),
        Some(None)
    );
    assert_eq!(
        slot_role_next(
            ReconciliationSlotRoleKind::ActivateExisting,
            EntityLifecycle::PreparedInert
        ),
        Some(Some(EntityLifecycle::Active))
    );
    assert_eq!(
        slot_role_next(
            ReconciliationSlotRoleKind::MountNew,
            EntityLifecycle::PreparedInert
        ),
        Some(Some(EntityLifecycle::Active))
    );
    assert!(
        slot_role_next(
            ReconciliationSlotRoleKind::PreservedActive,
            EntityLifecycle::PreparedInert
        )
        .is_none()
    );
    assert!(
        slot_role_next(
            ReconciliationSlotRoleKind::ActivateExisting,
            EntityLifecycle::Active
        )
        .is_none()
    );
    assert!(
        slot_role_next(
            ReconciliationSlotRoleKind::MountNew,
            EntityLifecycle::Active
        )
        .is_none()
    );
}

#[test]
fn reconciliation_leaving_parent_slot_obligation_is_between_child_and_parent() {
    let (mut app, branch, _, slot_entity, _) = failed_nested_branch_fixture();
    let mut branch = branch.into_realized_for_test();
    activate_branch(&mut app, &mut branch);
    let parent_entities = branch.frames()[0]
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let child_entities = branch.frames()[1]
        .claims()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    let slot = &branch.child_slots()[0];
    let projection = ReconciliationExistingProjection {
        preserved: Vec::new(),
        leaving: vec![
            ReconciliationExistingFrame {
                branch_index: 1,
                version: branch.frames()[1].frame().clone(),
            },
            ReconciliationExistingFrame {
                branch_index: 0,
                version: branch.frames()[0].frame().clone(),
            },
        ],
        child_slots: vec![ReconciliationExistingSlot {
            parent_branch_index: 0,
            parent: branch.frames()[0].frame().clone(),
            outlet: slot.outlet().clone(),
            slot: slot.slot_id(),
            child: Some(branch.frames()[1].frame().clone()),
            role: ReconciliationSlotRoleKind::Leaving,
        }],
    };
    let sealed = SealedExistingReconciliation::prepare(branch, projection)
        .expect("full leaving suffix must seal");
    let order = sealed
        .leaving()
        .lifecycle_transitions(EntityLifecycle::Leaving, true)
        .into_iter()
        .map(|(claim, _)| claim.entity())
        .collect::<Vec<_>>();
    let slot_index = order
        .iter()
        .position(|entity| *entity == slot_entity)
        .expect("slot obligation remains present");

    assert!(child_entities.iter().all(|entity| {
        order.iter().position(|current| current == entity).unwrap() < slot_index
    }));
    assert!(parent_entities.iter().all(|entity| {
        order.iter().position(|current| current == entity).unwrap() > slot_index
    }));
}

#[test]
fn mount_new_staging_allowance_rejects_wrong_parent_and_slot() {
    let (app, branch, _, _, _) = failed_nested_branch_fixture();
    let request = app
        .world()
        .resource::<crate::navigation::RouterNavigation>()
        .entries[0]
        .requests[0];
    let slot = &branch.child_slots()[0];
    let allowance = SealedMountNewStagingAllowance::for_test(
        request,
        slot.parent().clone(),
        slot.outlet().clone(),
        slot.slot_id(),
        slot.slot(),
        slot.slot_claim().clone(),
        slot.anchor(),
        slot.anchor_binding().clone(),
        slot.expected_anchor_children().to_vec(),
    );

    assert!(allowance.matches_binding(
        slot.parent(),
        slot.outlet(),
        slot.slot_id(),
        slot.slot(),
        slot.slot_claim(),
        slot.anchor(),
        slot.anchor_binding(),
    ));
    assert!(!allowance.matches_binding(
        branch.frames()[1].frame(),
        slot.outlet(),
        slot.slot_id(),
        slot.slot(),
        slot.slot_claim(),
        slot.anchor(),
        slot.anchor_binding(),
    ));
    assert!(!allowance.matches_binding(
        slot.parent(),
        slot.outlet(),
        branch.frames()[0].frame().slot(),
        slot.slot(),
        slot.slot_claim(),
        slot.anchor(),
        slot.anchor_binding(),
    ));
}

#[test]
fn mount_new_staging_allowance_rejects_stale_request() {
    let (mut app, branch, _, _, _) = failed_nested_branch_fixture();
    let (router, request) = {
        let navigation = app
            .world()
            .resource::<crate::navigation::RouterNavigation>();
        (
            navigation.entries[0].handle,
            navigation.entries[0].requests[0],
        )
    };
    let stale = app
        .world_mut()
        .resource_mut::<crate::navigation::RouterNavigation>()
        .push(
            router,
            bevy_ui_router_core::prelude::RouteLocation::new("/stale"),
        )
        .expect("same router can retain a later request identity");
    let slot = &branch.child_slots()[0];
    let allowance = SealedMountNewStagingAllowance::for_test(
        request,
        slot.parent().clone(),
        slot.outlet().clone(),
        slot.slot_id(),
        slot.slot(),
        slot.slot_claim().clone(),
        slot.anchor(),
        slot.anchor_binding().clone(),
        slot.expected_anchor_children().to_vec(),
    );

    assert!(allowance.matches_request(request));
    assert!(!allowance.matches_request(stale));
}
