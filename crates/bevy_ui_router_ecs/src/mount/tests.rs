use super::*;
use bevy_app::Inherited;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::Component,
    hierarchy::ChildOf,
    lifecycle::HookContext,
    prelude::{Resource, World},
};
use bevy_picking::Pickable;
use bevy_ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node};
use bevy_ui_router_core::{
    integration::{
        CleanupManifest, EngineAction, IntegrationError, MountGrant, MountMutationArtifact,
        MountRequest, RouterEngine, ViewFailureKind,
    },
    prelude::{
        FrameOrigin, NavigationPhase, RequestStatus, RouteLocation, RouteSlotKey, RouteViewKey,
    },
};

use crate::{
    app_ext::RouterHandle,
    components::RouterInputBarrier,
    content::{ContentPlan, FrameContentWriter},
    host::{
        HostCandidateClassification, HostCandidateRevisionState, ValidatedHostCandidate,
        classify_host_candidate,
    },
    input::{RouterInputGate, refresh_input_gate_system},
    navigation::RouterNavigation,
    presentation::{
        RealizedFrame, realize_child_frame_deferred, realize_frame, validate_realized_frame,
    },
    registry::{
        EntityLifecycle, EntityOwner, ExactRegistryClaim, ProvisionalOwner, QuarantineLeaseTable,
        RegistryOwner, RouteEntityRegistry,
    },
};

#[derive(Resource, Default)]
struct ChildSlotStageAudit(Vec<bool>);

#[derive(Component)]
struct ForeignChildSlotComponent;

#[derive(Component)]
struct DescendantFocusProbe;

#[derive(Resource)]
struct ForeignParent(Entity);

#[derive(Resource)]
struct AnchorOrderDrift {
    anchor: Entity,
    foreign: Entity,
}

#[derive(Resource, Default)]
struct RegistrationDriftArmed(bool);

fn focus_child_slot(world: &mut World, slot: Entity) {
    world.insert_resource(InputFocus::from_entity(slot));
    let mut route_focus = RouteFocusState::default();
    route_focus.set_focused(Some(slot));
    world.insert_resource(route_focus);
}

fn install_registry_first_audit(world: &mut World) {
    let audit = |mut world: bevy_ecs::world::DeferredWorld<'_>, HookContext { entity, .. }| {
        let valid = world
            .get_resource::<RouteEntityRegistry>()
            .and_then(|registry| registry.record(entity))
            .is_some_and(|record| {
                record.entity() == entity
                    && record.lifecycle() == EntityLifecycle::PreparedInert
                    && matches!(
                        record.owner(),
                        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
                    )
            });
        world
            .get_resource_mut::<ChildSlotStageAudit>()
            .expect("stage audit resource is installed")
            .0
            .push(valid);
    };
    world
        .register_component_hooks::<ChildSlotBarrierProbe>()
        .on_insert(audit);
    world
        .register_component_hooks::<ChildSlotNodeProbe>()
        .on_insert(audit);
    world
        .register_component_hooks::<ChildSlotAuthorityProbe>()
        .on_insert(audit);
    world
        .register_component_hooks::<ChildSlotRelationshipProbe>()
        .on_insert(audit);
}

fn install_anchor_order_drift<C: Component>(
    world: &mut World,
    frame: &RealizedFrame,
) -> (Entity, Entity, Entity) {
    let anchor = frame
        .outlet(&RouteSlotKey::default())
        .expect("default outlet exists")
        .anchor();
    let (anchor_binding, old_manifest) = {
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(anchor)
            .expect("outlet anchor stays registered");
        (record.claim(), record.manifest().clone())
    };
    let baseline = world.spawn(ChildOf(anchor)).id();
    world.flush();
    let actual_manifest = ComponentManifest::from_world(world, anchor).unwrap();
    assert!(
        world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_manifest_exact(&anchor_binding, &old_manifest, actual_manifest,)
    );
    let foreign = world.spawn_empty().id();
    world.insert_resource(AnchorOrderDrift { anchor, foreign });
    world
        .register_component_hooks::<C>()
        .on_insert(|mut world, HookContext { .. }| {
            let drift = world
                .get_resource::<AnchorOrderDrift>()
                .expect("anchor drift is armed");
            let anchor = drift.anchor;
            let foreign = drift.foreign;
            world.commands().entity(foreign).insert(ChildOf(anchor));
        });
    (anchor, baseline, foreign)
}

fn assert_residual_inert(world: &World, residual: &ChildSlotResidual) {
    if residual.state() == ChildSlotResidualState::Absent {
        assert!(world.get_entity(residual.entity()).is_err());
        return;
    }
    assert_eq!(
        world.get::<Visibility>(residual.entity()),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        world.get::<Pickable>(residual.entity()),
        Some(&Pickable::IGNORE)
    );
    assert!(!focus_is_in_slot(world, residual.entity()));
}

fn assert_residual_barrier_denied(world: &World, residual: &ChildSlotResidual) {
    assert!(
        world
            .get::<RouterInputBarrier>(residual.entity())
            .is_some_and(RouterInputBarrier::denies_all)
    );
}

fn pending_nested_parent_fixture() -> (
    World,
    ValidatedHostCandidate,
    bevy_ecs::prelude::Entity,
    ExactRegistryClaim,
    RouterEngine,
    bevy_ui_router_core::integration::FrameTask,
) {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let router = RouterHandle::fixture(41, 43);
    world.spawn((
        Node::default(),
        crate::app_ext::RouterHostRegistration::new(router),
    ));
    let mut revisions = HostCandidateRevisionState::default();
    let HostCandidateClassification::Unique(candidate) =
        classify_host_candidate(&world, router, &mut revisions)
    else {
        panic!("host should classify")
    };

    let mut engine = RouterEngine::install(
        bevy_ui_router_core::test_support::nested_items_view_route_tree(),
        RouteViewKey::new("fallback").unwrap(),
    )
    .unwrap();
    let admission = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&admission).unwrap();
    let staged =
        crate::host::stage_granted_top_slot(&mut world, candidate.clone(), &grant).unwrap();
    let top_slot = staged.entity();
    assert!(crate::host::activate_granted_top_slot(
        &mut world, &candidate, &grant, &staged,
    ));
    let top_slot_claim = world
        .resource::<RouteEntityRegistry>()
        .record(top_slot)
        .unwrap()
        .claim();
    engine.confirm_host(grant);
    engine.push(RouteLocation::new("/items/1")).unwrap();
    engine.collect();
    let EngineAction::FrameTask(parent_task) = engine.action().unwrap() else {
        panic!("parent frame task expected")
    };
    (
        world,
        candidate,
        top_slot,
        top_slot_claim,
        engine,
        parent_task,
    )
}

fn realized_parent_with_outlet() -> (
    World,
    ValidatedHostCandidate,
    bevy_ecs::prelude::Entity,
    RouterEngine,
    RealizedFrame,
    usize,
    MountGrant,
) {
    let (world, candidate, top_slot, engine, frame, local_index, request) =
        realized_parent_awaiting_mount();
    let mut engine = engine;
    let grant = engine.resolve_mount(request).unwrap();
    (
        world,
        candidate,
        top_slot,
        engine,
        frame,
        local_index,
        grant,
    )
}

fn realized_parent_awaiting_mount() -> (
    World,
    ValidatedHostCandidate,
    bevy_ecs::prelude::Entity,
    RouterEngine,
    RealizedFrame,
    usize,
    MountRequest,
) {
    let (mut world, candidate, top_slot, top_slot_claim, mut engine, task) =
        pending_nested_parent_fixture();
    let mut plan = ContentPlan::new();
    let anchor = {
        let mut writer = FrameContentWriter::new(&mut plan);
        writer.spawn_empty().unwrap()
    };
    FrameContentWriter::new(&mut plan)
        .declare_outlet(anchor, RouteSlotKey::default())
        .unwrap();
    let accepted = plan.accept();
    let local_index = accepted.outlets()[0].local_index();
    let frame = realize_frame(
        &mut world,
        candidate.clone(),
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        accepted,
    )
    .unwrap();
    engine
        .report_applied_with_outlets(task, vec![RouteSlotKey::default()])
        .unwrap();
    let EngineAction::MountRequest(request) = engine.action().unwrap() else {
        panic!("child mount request expected")
    };
    (
        world,
        candidate,
        top_slot,
        engine,
        frame,
        local_index,
        request,
    )
}

fn realized_parent_without_outlet() -> (World, RouterEngine, RealizedFrame, MountGrant) {
    let (mut world, candidate, top_slot, top_slot_claim, mut engine, task) =
        pending_nested_parent_fixture();
    let frame = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .unwrap();
    engine
        .report_applied_with_outlets(task, vec![RouteSlotKey::default()])
        .unwrap();
    let EngineAction::MountRequest(request) = engine.action().unwrap() else {
        panic!("declared logical outlet must issue one mount request")
    };
    let grant = engine.resolve_mount(request).unwrap();
    (world, engine, frame, grant)
}

#[test]
fn accepted_outlet_maps_to_exact_realized_output() {
    let (world, _candidate, _top_slot, _engine, frame, local_index, _grant) =
        realized_parent_with_outlet();
    let outlet = frame
        .outlet(&RouteSlotKey::default())
        .expect("accepted outlet must map to one exact output");
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(outlet.anchor())
        .expect("realized outlet anchor must stay registered");

    assert_eq!(record.claim(), *outlet.anchor_claim());
    assert!(matches!(
        record.owner(),
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            local_index: actual,
            ..
        }) if *actual == local_index
    ));
}

#[test]
fn wrong_outlet_grant_mismatch_is_world_zero_and_grant_remains_usable() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let records_before = world.resource::<RouteEntityRegistry>().records().to_vec();
    let entities_before = world.iter_entities().count();
    let seal_before = world.resource::<RouteEntityRegistry>().latest_issued_seal();

    let failure = stage_child_slot(
        &mut world,
        &frame,
        &RouteSlotKey::named("wrong").unwrap(),
        &grant,
    )
    .expect_err("a sealed grant cannot stage a different outlet");

    assert_eq!(failure.kind(), ChildSlotStageErrorKind::GrantMismatch);
    assert!(failure.into_residual().is_none());
    assert_eq!(world.iter_entities().count(), entities_before);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records(),
        records_before
    );
    assert_eq!(
        world.resource::<RouteEntityRegistry>().latest_issued_seal(),
        seal_before
    );
    stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant)
        .expect("the borrowed grant remains usable for its exact target");
}

#[test]
fn equal_raw_cross_router_grant_mismatch_is_world_zero_before_owner_cas() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, _grant) =
        realized_parent_with_outlet();
    let (
        mut foreign_world,
        _foreign_candidate,
        _foreign_top_slot,
        _foreign_engine,
        foreign_frame,
        _foreign_local_index,
        foreign_grant,
    ) = realized_parent_with_outlet();
    assert_eq!(frame.frame().router(), foreign_frame.frame().router());
    assert_eq!(frame.frame().host(), foreign_frame.frame().host());
    let records_before = world.resource::<RouteEntityRegistry>().records().to_vec();
    let entities_before = world.iter_entities().count();
    let seal_before = world.resource::<RouteEntityRegistry>().latest_issued_seal();

    let failure = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &foreign_grant)
        .expect_err("equal raw IDs from another router domain must fail closed");

    assert_eq!(failure.kind(), ChildSlotStageErrorKind::GrantMismatch);
    assert!(failure.into_residual().is_none());
    assert_eq!(world.iter_entities().count(), entities_before);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records(),
        records_before
    );
    assert_eq!(
        world.resource::<RouteEntityRegistry>().latest_issued_seal(),
        seal_before
    );
    stage_child_slot(
        &mut foreign_world,
        &foreign_frame,
        &RouteSlotKey::default(),
        &foreign_grant,
    )
    .expect("the borrowed grant remains usable in its owning router domain");
}

#[test]
fn child_slot_remains_provisional_before_registration_permit() {
    let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
        realized_parent_with_outlet();

    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(staged.slot())
        .expect("staged child slot must stay registered");

    assert_eq!(record.claim(), *staged.slot_claim());
    assert!(matches!(
        record.owner(),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) if parent == frame.frame()
            && outlet == &RouteSlotKey::default()
            && *slot == grant.slot()
    ));
    assert_eq!(record.lifecycle(), EntityLifecycle::PreparedInert);
    assert_eq!(
        world.get::<Visibility>(staged.slot()),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        world.get::<Pickable>(staged.slot()),
        Some(&Pickable::IGNORE)
    );
    assert!(
        world
            .get::<RouterInputBarrier>(staged.slot())
            .is_some_and(RouterInputBarrier::denies_all)
    );
    assert!(engine.action().is_none());
}

#[test]
fn child_slot_is_provisionally_registered_before_any_observer_capable_write() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world.insert_resource(ChildSlotStageAudit::default());
    install_registry_first_audit(&mut world);

    stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();

    assert_eq!(
        world.resource::<ChildSlotStageAudit>().0,
        vec![true, true, true, true]
    );
}

#[test]
fn barrier_batch_rejects_anchor_children_order_drift_in_the_same_batch() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let (anchor, baseline, foreign) =
        install_anchor_order_drift::<ChildSlotBarrierProbe>(&mut world, &frame);

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();

    assert_eq!(failure.kind(), ChildSlotStageErrorKind::BarrierDrift);
    let residual = failure.into_residual().unwrap();
    assert_eq!(children_of(&world, anchor), vec![baseline, foreign]);
    assert!(world.get::<ChildOf>(residual.entity()).is_none());
    assert_ne!(
        world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap()
            .lifecycle(),
        EntityLifecycle::Active
    );
}

#[test]
fn node_batch_rejects_anchor_children_order_drift_in_the_same_batch() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let (anchor, baseline, foreign) =
        install_anchor_order_drift::<ChildSlotNodeProbe>(&mut world, &frame);

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();

    assert_eq!(failure.kind(), ChildSlotStageErrorKind::NodeDrift);
    let residual = failure.into_residual().unwrap();
    assert_eq!(children_of(&world, anchor), vec![baseline, foreign]);
    assert!(world.get::<ChildOf>(residual.entity()).is_none());
    assert_ne!(
        world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap()
            .lifecycle(),
        EntityLifecycle::Active
    );
}

#[test]
fn authority_batch_rejects_anchor_children_order_drift_in_the_same_batch() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let (anchor, baseline, foreign) =
        install_anchor_order_drift::<ChildSlotAuthorityProbe>(&mut world, &frame);

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();

    assert_eq!(failure.kind(), ChildSlotStageErrorKind::AuthorityDrift);
    let residual = failure.into_residual().unwrap();
    assert_eq!(children_of(&world, anchor), vec![baseline, foreign]);
    assert!(world.get::<ChildOf>(residual.entity()).is_none());
    assert_ne!(
        world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap()
            .lifecycle(),
        EntityLifecycle::Active
    );
}

#[test]
fn child_slot_barrier_drift_is_fail_closed_without_activation() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotBarrierProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .remove::<RouterInputBarrier>();
        });

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
    assert_eq!(failure.kind(), ChildSlotStageErrorKind::BarrierDrift);
    let residual = failure.into_residual().unwrap();
    assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
    assert_eq!(
        world.get::<Visibility>(residual.entity()),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        world.get::<Pickable>(residual.entity()),
        Some(&Pickable::IGNORE)
    );
    assert!(!focus_is_in_slot(&world, residual.entity()));
    assert!(world.get::<RouterInputBarrier>(residual.entity()).is_none());
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(residual.entity())
        .unwrap();
    assert_ne!(record.lifecycle(), EntityLifecycle::Active);
    assert!(record.manifest().component_ids().is_empty());
    assert!(!record.manifest().matches(&world, residual.entity()));
}

#[test]
fn child_slot_node_closure_drift_is_rejected_and_inert() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotNodeProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .insert(ForeignChildSlotComponent);
        });

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
    assert_eq!(failure.kind(), ChildSlotStageErrorKind::NodeDrift);
    let residual = failure.into_residual().unwrap();
    assert_residual_inert(&world, &residual);
    assert_residual_barrier_denied(&world, &residual);
    assert!(
        world
            .get::<ForeignChildSlotComponent>(residual.entity())
            .is_some()
    );
    let node = world.components().component_id::<Node>().unwrap();
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(residual.entity())
        .unwrap();
    assert!(!record.manifest().component_ids().contains(&node));
}

#[test]
fn refresh_clears_focus_reassigned_to_a_foreign_descendant_of_residual() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotNodeProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .insert(ForeignChildSlotComponent);
        });
    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
    let residual = failure.into_residual().unwrap();
    let foreign = world.spawn(ChildOf(residual.entity())).id();
    world.flush();
    world.insert_resource(InputFocus::from_entity(foreign));
    let mut route_focus = RouteFocusState::default();
    route_focus.set_focused(Some(foreign));
    world.insert_resource(route_focus);
    world.insert_resource(RouterNavigation::new());
    world.insert_resource(RouterInputGate::new());

    refresh_input_gate_system(&mut world);

    assert_eq!(world.resource::<InputFocus>().get(), None);
    assert_eq!(world.resource::<RouteFocusState>().focused(), None);
}

#[test]
fn refresh_preserves_active_sibling_focus_below_the_nearest_registry_boundary() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotNodeProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .insert(ForeignChildSlotComponent);
        });
    let residual = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant)
        .unwrap_err()
        .into_residual()
        .unwrap();
    assert_ne!(
        world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap()
            .lifecycle(),
        EntityLifecycle::Active
    );
    let shell_claim = world
        .resource::<RouteEntityRegistry>()
        .record(frame.shell())
        .unwrap()
        .claim();
    world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_lifecycle(&shell_claim, EntityLifecycle::Active)
        .unwrap();
    let active_descendant = world.spawn(ChildOf(frame.shell())).id();
    world.flush();
    world.insert_resource(InputFocus::from_entity(active_descendant));
    let mut route_focus = RouteFocusState::default();
    route_focus.set_focused(Some(active_descendant));
    world.insert_resource(route_focus);
    world.insert_resource(RouterNavigation::new());
    world.insert_resource(RouterInputGate::new());

    refresh_input_gate_system(&mut world);

    assert_eq!(
        world.resource::<InputFocus>().get(),
        Some(active_descendant)
    );
    assert_eq!(
        world.resource::<RouteFocusState>().focused(),
        Some(active_descendant)
    );
}

#[test]
fn child_slot_authority_drift_is_rejected_and_inert() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotAuthorityProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .remove::<RouteOwnershipSeal>();
        });

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
    assert_eq!(failure.kind(), ChildSlotStageErrorKind::AuthorityDrift);
    let residual = failure.into_residual().unwrap();
    assert_residual_inert(&world, &residual);
    assert_residual_barrier_denied(&world, &residual);
    assert!(world.get::<RouteOwnershipSeal>(residual.entity()).is_none());
    let authority = world
        .components()
        .component_id::<ChildSlotAuthority>()
        .unwrap();
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(residual.entity())
        .unwrap();
    assert!(!record.manifest().component_ids().contains(&authority));
}

#[test]
fn child_slot_relationship_drift_is_rejected_and_inert() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let foreign_parent = world.spawn_empty().id();
    world.insert_resource(ForeignParent(foreign_parent));
    world
        .register_component_hooks::<ChildSlotRelationshipProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            let parent = world
                .get_resource::<ForeignParent>()
                .expect("foreign parent is installed")
                .0;
            world.commands().entity(entity).insert(ChildOf(parent));
        });

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
    assert_eq!(failure.kind(), ChildSlotStageErrorKind::RelationshipDrift);
    let residual = failure.into_residual().unwrap();
    assert_residual_inert(&world, &residual);
    assert_residual_barrier_denied(&world, &residual);
    assert_eq!(
        world.get::<ChildOf>(residual.entity()).map(ChildOf::parent),
        Some(foreign_parent)
    );
    let child_of = world.components().component_id::<ChildOf>().unwrap();
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(residual.entity())
        .unwrap();
    assert!(!record.manifest().component_ids().contains(&child_of));
}

#[test]
fn child_slot_relationship_keeps_derived_ui_out_of_structural_manifest() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotRelationshipProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world.commands().entity(entity).insert((
                Inherited(ComputedUiTargetCamera::default()),
                Inherited(ComputedUiRenderTargetInfo::default()),
            ));
        });

    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant)
        .expect("UI propagation must preserve the structural manifest");
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(staged.slot())
        .expect("staged slot remains registered");
    assert!(record.manifest().matches(&world, staged.slot()));
    assert!(
        world
            .get::<Inherited<ComputedUiTargetCamera>>(staged.slot())
            .is_some()
    );
    assert!(
        world
            .get::<Inherited<ComputedUiRenderTargetInfo>>(staged.slot())
            .is_some()
    );
    assert!(
        !record.manifest().component_ids().contains(
            &world
                .components()
                .component_id::<Inherited<ComputedUiTargetCamera>>()
                .unwrap()
        )
    );
    assert!(
        !record.manifest().component_ids().contains(
            &world
                .components()
                .component_id::<Inherited<ComputedUiRenderTargetInfo>>()
                .unwrap()
        )
    );
}

#[test]
fn child_slot_despawn_drift_is_tracked_absent_without_cleanup() {
    let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .register_component_hooks::<ChildSlotAuthorityProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            world.commands().entity(entity).despawn();
        });

    let failure =
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
    let residual = failure.into_residual().unwrap();
    assert_eq!(residual.state(), ChildSlotResidualState::Absent);
    assert_residual_inert(&world, &residual);
    assert_eq!(
        world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap()
            .claim(),
        *residual.claim()
    );
}

#[test]
fn child_slot_registration_rejection_leaves_no_empty_entity() {
    let (mut world, _candidate, _top_slot, _engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let _first = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let entities_before = world.iter_entities().count();
    let records_before = world.resource::<RouteEntityRegistry>().records().len();

    let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
        .expect_err("duplicate logical child owner must reject registration");

    assert_eq!(failure.kind(), ChildSlotStageErrorKind::RegistryRejected);
    assert!(failure.into_residual().is_none());
    assert_eq!(world.iter_entities().count(), entities_before);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records().len(),
        records_before
    );
}

#[test]
fn exhausted_seal_rejects_child_staging_without_allocating() {
    let (mut world, _candidate, _top_slot, _engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    world
        .resource_mut::<RouteEntityRegistry>()
        .exhaust_seal_sequence_for_test();
    let before = world_registry_snapshot(&world);
    for _ in 0..2 {
        let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
            .expect_err("exhausted ownership identity must reject before allocation");
        assert_eq!(failure.kind(), ChildSlotStageErrorKind::RegistryRejected);
        assert!(failure.into_residual().is_none());
        assert_eq!(world_registry_snapshot(&world), before);
    }
}

#[test]
fn child_slot_seal_and_slot_id_are_not_reused_after_failure() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let first_slot = grant.slot();
    let _first = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let first_seal = world
        .resource::<RouteEntityRegistry>()
        .latest_issued_seal()
        .expect("first child slot must issue an ownership seal");

    let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
        .expect_err("duplicate logical child owner must fail before allocating another seal");
    assert!(failure.into_residual().is_none());
    let failed_seal = world
        .resource::<RouteEntityRegistry>()
        .latest_issued_seal()
        .expect("the previously issued seal remains the latest after preflight rejection");
    // Preflight did not issue a seal. Previously issued identities remain
    // consumed; the next actual allocation must still receive a fresh one.
    assert_eq!(failed_seal, first_seal);
    let next_seal = world
        .resource_mut::<RouteEntityRegistry>()
        .issue_unregistered_seal_for_test();
    assert_ne!(next_seal, failed_seal);

    let cleanup_request = engine
        .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
        .unwrap();
    let cleanup = match engine.prepare_failure_cleanup_retaining(
        cleanup_request,
        CleanupManifest::new(vec![parent.frame().clone()], 701, 1),
    ) {
        FailureCleanupPrepareOutcome::Prepared(cleanup) => cleanup,
        FailureCleanupPrepareOutcome::Mismatch(_) | FailureCleanupPrepareOutcome::Permanent(_) => {
            panic!("exact failed mount cleanup must prepare")
        }
    };
    engine.complete_cleanup(cleanup.complete(709, 1)).unwrap();
    let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
        panic!("failed mount cleanup must continue through RouterFallback")
    };
    assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
    let recovery_failure = engine
        .report_failed(
            fallback,
            ViewFailureKind::Build,
            "fixture finishes fallback before checking the next mount identity",
        )
        .unwrap()
        .expect("fallback failure must retain cleanup authority before terminal");
    let recovery_cleanup = match engine.prepare_failure_cleanup_retaining(
        recovery_failure,
        CleanupManifest::new(Vec::new(), 0x4D4F_554E, 0),
    ) {
        FailureCleanupPrepareOutcome::Prepared(cleanup) => cleanup,
        FailureCleanupPrepareOutcome::Mismatch(_) | FailureCleanupPrepareOutcome::Permanent(_) => {
            panic!("exact fallback cleanup must prepare")
        }
    };
    engine
        .complete_cleanup(recovery_cleanup.complete_empty())
        .unwrap();
    engine.push(RouteLocation::new("/items/2")).unwrap();
    engine.collect();
    let EngineAction::FrameTask(next_parent) = engine.action().unwrap() else {
        panic!("next request must issue its parent frame task")
    };
    engine
        .report_applied_with_outlets(next_parent, vec![RouteSlotKey::default()])
        .unwrap();
    let EngineAction::MountRequest(next_request) = engine.action().unwrap() else {
        panic!("next parent report must issue a fresh mount request")
    };
    let next_grant = engine.resolve_mount(next_request).unwrap();

    assert_ne!(next_grant.slot(), first_slot);
    assert!(next_grant.slot().raw() > first_slot.raw());
}

#[test]
fn child_slot_is_registered_inert_after_permit_until_exact_completion_and_confirmation() {
    let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
    let slot = staged.slot();
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();

    assert!(matches!(
        world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .unwrap()
            .owner(),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
    ));
    assert!(engine.action().is_none());

    let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(mounted.slot())
        .unwrap();
    assert_eq!(record.claim(), *mounted.slot_claim());
    assert!(matches!(
        record.owner(),
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot: stored_slot,
        }) if parent == frame.frame()
            && outlet == &RouteSlotKey::default()
            && *stored_slot == mounted.slot_id()
    ));
    assert_eq!(
        world.get::<ChildSlotLifecycle>(mounted.slot()),
        Some(&ChildSlotLifecycle::RegisteredInert)
    );
    assert_eq!(
        world.get::<Visibility>(mounted.slot()),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        world.get::<Pickable>(mounted.slot()),
        Some(&Pickable::IGNORE)
    );
    assert!(
        world
            .get::<RouterInputBarrier>(mounted.slot())
            .is_some_and(RouterInputBarrier::denies_all)
    );
    let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
        panic!("child task may exist only after exact registration completion")
    };
    assert_eq!(child_task.frame().slot(), mounted.slot_id());
}

#[test]
fn child_frame_shell_uses_confirmed_child_slot_not_top_slot() {
    let (mut world, _candidate, top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
    let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
        panic!("confirmed child slot must issue one child frame task")
    };

    let child = realize_child_frame_deferred(
        &mut world,
        &parent,
        &mounted,
        &child_task,
        child_task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .unwrap_or_else(|_| panic!("confirmed child slot must accept its exact child task"));

    assert!(child.mount_binding().is_child_outlet());
    assert_eq!(child.top_slot(), mounted.slot());
    assert_ne!(child.top_slot(), top_slot);
    assert_eq!(
        world.get::<ChildOf>(child.shell()).map(ChildOf::parent),
        Some(mounted.slot())
    );
    assert_eq!(child.frame().slot(), mounted.slot_id());
    assert!(validate_realized_frame(&world, &child));
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .record(mounted.slot())
            .is_some_and(|record| record.manifest().matches(&world, mounted.slot()))
    );
}

fn realized_nested_branch() -> (World, crate::branch_transaction::RealizedBranch) {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
    let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
        panic!("confirmed child slot must issue one child frame task")
    };
    let child = realize_child_frame_deferred(
        &mut world,
        &parent,
        &mounted,
        &child_task,
        child_task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .unwrap_or_else(|_| panic!("exact child task must realize"));
    let mut branch = crate::branch_transaction::RealizedBranch::empty();
    branch.push_frame(parent).unwrap();
    assert!(branch.push_child_slot(mounted).is_ok());
    branch.push_frame(child).unwrap();
    (world, branch)
}

#[test]
fn publication_rebinds_frame_mount_outlet_and_child_slot_aliases() {
    let (world, mut branch) = realized_nested_branch();
    let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
    let resulting = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec();

    let prepared = branch
        .prepare_claim_rebind(
            &resulting,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        )
        .unwrap();
    branch.apply_claim_rebind(prepared);
    assert!(
        branch
            .claims()
            .all(|claim| claim.lifecycle() == EntityLifecycle::Active)
    );
    let parent = &branch.frames()[0];
    let outlet = parent.outlet(&RouteSlotKey::default()).unwrap();
    assert_eq!(outlet.anchor_claim().lifecycle(), EntityLifecycle::Active);
    let mounted = &branch.child_slots()[0];
    assert_eq!(mounted.slot_claim().lifecycle(), EntityLifecycle::Active);
    assert_eq!(mounted.anchor_claim().lifecycle(), EntityLifecycle::Active);
    let child_mount = branch.frames()[1].mount_binding();
    assert_eq!(
        child_mount.slot_claim().lifecycle(),
        EntityLifecycle::Active
    );
    assert_eq!(
        child_mount.physical_parent().1.unwrap().lifecycle(),
        EntityLifecycle::Active
    );
}

#[test]
fn publication_rebind_rejects_wrong_from_or_to_lifecycle_for_all_aliases() {
    let (world, branch) = realized_nested_branch();
    let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
    let correct = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec();
    let before = branch.claims().cloned().collect::<Vec<_>>();
    let parent = &branch.frames()[0];
    let checked_alias_entities = [
        parent.shell(),
        parent.outlet(&RouteSlotKey::default()).unwrap().anchor(),
        branch.child_slots()[0].slot(),
    ];

    for entity in checked_alias_entities {
        let current = branch
            .claims()
            .find(|claim| claim.entity() == entity)
            .unwrap()
            .clone();
        let wrong_next = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&[(current, EntityLifecycle::Leaving)])
            .unwrap()
            .resulting_claims()[0]
            .clone();
        let mut claims = correct.clone();
        *claims
            .iter_mut()
            .find(|claim| claim.entity() == entity)
            .unwrap() = wrong_next;
        assert!(
            branch
                .prepare_claim_rebind(
                    &claims,
                    EntityLifecycle::PreparedInert,
                    EntityLifecycle::Active,
                )
                .is_none()
        );
        assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
    }

    assert!(
        branch
            .prepare_claim_rebind(&correct, EntityLifecycle::Active, EntityLifecycle::Leaving,)
            .is_none()
    );
    assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn mounted_child_slot_alias_rebind_is_atomic() {
    let (world, branch) = realized_nested_branch();
    let mounted = branch
        .child_slots()
        .into_iter()
        .next()
        .unwrap()
        .clone_for_test();
    let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
    let mut resulting = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec();
    let anchor_index = resulting
        .iter()
        .position(|claim| claim.entity() == mounted.anchor_claim().entity())
        .unwrap();
    let anchor_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(mounted.anchor())
        .unwrap()
        .manifest()
        .clone();
    let mut foreign_registry = RouteEntityRegistry::new();
    resulting[anchor_index] = foreign_registry
        .register(
            mounted.anchor(),
            mounted.anchor_claim().owner().clone(),
            anchor_manifest,
            EntityLifecycle::Active,
        )
        .unwrap();
    let slot_before = mounted.slot_claim().clone();
    let anchor_before = mounted.anchor_claim().clone();

    assert!(!mounted.internal_claims_can_rebind(
        &resulting,
        EntityLifecycle::PreparedInert,
        EntityLifecycle::Active,
    ));
    assert_eq!(mounted.slot_claim(), &slot_before);
    assert_eq!(mounted.anchor_claim(), &anchor_before);
}

#[test]
fn missing_mounted_child_slot_alias_is_zero_partial_write() {
    let (mut world, branch) = realized_nested_branch();
    let mut mounted = branch
        .child_slots()
        .into_iter()
        .next()
        .unwrap()
        .clone_for_test();
    let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
    let resulting = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec();
    let foreign_entity = world.spawn_empty().id();
    let anchor_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(mounted.anchor())
        .unwrap()
        .manifest()
        .clone();
    let mut foreign_registry = RouteEntityRegistry::new();
    *mounted.anchor_binding.source_claim_mut() = foreign_registry
        .register(
            foreign_entity,
            mounted.anchor_claim().owner().clone(),
            anchor_manifest,
            EntityLifecycle::PreparedInert,
        )
        .unwrap();
    let slot_before = mounted.slot_claim().clone();
    let anchor_before = mounted.anchor_claim().clone();

    assert!(!mounted.internal_claims_can_rebind(
        &resulting,
        EntityLifecycle::PreparedInert,
        EntityLifecycle::Active,
    ));
    assert_eq!(mounted.slot_claim(), &slot_before);
    assert_eq!(mounted.anchor_claim(), &anchor_before);
}

#[test]
fn prepared_branch_rejects_mismatched_mounted_anchor_without_world_or_registry_write() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let digest = staged.receipt().digest();
    let (attestation, proof) = grant.attest_receipt_with_proof(digest);
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    let mut mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect("exact child slot should confirm");
    mounted.anchor = world.spawn_empty().id();
    let world_before = world
        .iter_entities()
        .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
        .collect::<Vec<_>>();
    let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
    let mut branch = crate::branch_transaction::PreparedBranch::empty();
    branch.push_frame(parent).unwrap();

    let rejected = branch.push_child_slot(mounted).unwrap_err();
    let rejected_entity = rejected.slot().slot();
    let canonical_anchor = rejected
        .canonical_anchor()
        .expect("the stored parent supplies the append-before canonical anchor");
    let (source_parent, source_outlet, source_slot) = match rejected.slot().slot_claim().owner() {
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => (parent.clone(), outlet.clone(), *slot),
        owner => {
            panic!("confirmed rejected slot must retain its exact source owner: {owner:?}")
        }
    };
    assert_eq!(branch.frames().len(), 1);
    assert!(branch.child_slots().is_empty());
    assert_eq!(
        world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>(),
        world_before
    );
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records(),
        registry_before
    );

    let failed =
        crate::branch_transaction::FailedPreparedBranch::with_rejected_child_slot(branch, rejected);
    assert_eq!(
        crate::cleanup::failed_prepared_rejected_topology_facts_for_test(
            &world,
            world.resource::<RouteEntityRegistry>(),
            &failed,
        )
        .expect("the builder must classify the rejected source before freezing cleanup"),
        vec![(
            rejected_entity,
            source_parent,
            source_outlet,
            source_slot,
            Some(canonical_anchor),
        )]
    );
    world.insert_resource(InputFocus::default());
    world.insert_resource(RouteFocusState::default());
    let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
        &world,
        world.resource::<RouteEntityRegistry>(),
        &failed,
    )
    .expect("canonical append-before anchor must authorize rejected slot cleanup");
    assert!(manifest.entities().contains(&rejected_entity));
    let mut ledger = match crate::cleanup::CleanupLedger::for_failed_prepared_branch(&world, failed)
    {
        Ok(ledger) => ledger,
        Err(_) => panic!("rejected slot must remain owned by one cleanup ledger"),
    };
    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .record(rejected_entity)
            .is_none()
    );
}

#[test]
fn canonical_absent_rejected_slot_preserves_unknown_parent_as_unresolved() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let digest = staged.receipt().digest();
    let (attestation, proof) = grant.attest_receipt_with_proof(digest);
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    let mut mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect("exact child slot should confirm");
    let slot = mounted.slot();
    let source_claim = mounted.slot_claim().clone();
    let (source_parent, source_outlet, source_slot) = match source_claim.owner() {
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot,
        }) => (parent.clone(), outlet.clone(), *slot),
        owner => {
            panic!("confirmed rejected slot must retain its exact source owner: {owner:?}")
        }
    };
    let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
        panic!("confirmed slot must issue its child task")
    };
    mounted.parent = child_task.frame().clone();
    let app_parent = world.spawn_empty().id();
    world
        .entity_mut(slot)
        .insert((ForeignChildSlotComponent, ChildOf(app_parent)));
    world.flush();

    let mut branch = crate::branch_transaction::PreparedBranch::empty();
    branch.push_frame(parent).unwrap();
    let rejected = branch.push_child_slot(mounted).unwrap_err();
    assert!(rejected.canonical_anchor().is_none());
    assert!(rejected.canonical_anchor_claim().is_none());
    let failed =
        crate::branch_transaction::FailedPreparedBranch::with_rejected_child_slot(branch, rejected);
    assert_eq!(
        crate::cleanup::failed_prepared_rejected_topology_facts_for_test(
            &world,
            world.resource::<RouteEntityRegistry>(),
            &failed,
        )
        .expect("the builder must keep the exact source owner with no trusted boundary"),
        vec![(slot, source_parent, source_outlet, source_slot, None,)]
    );
    world.insert_resource(InputFocus::default());
    world.insert_resource(RouteFocusState::default());
    let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
        &world,
        world.resource::<RouteEntityRegistry>(),
        &failed,
    )
    .expect("the exact source claim must freeze without trusting an intended anchor");
    assert_eq!(manifest.entities().first(), Some(&slot));
    let mut ledger = crate::cleanup::CleanupLedger::for_failed_prepared_branch(&world, failed)
        .unwrap_or_else(|_| panic!("the rejected slot must retain one cleanup owner"));
    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));

    assert!(world.get_entity(app_parent).is_ok());
    assert_eq!(
        world.get::<ChildOf>(slot).map(ChildOf::parent),
        Some(app_parent)
    );
    assert!(world.get::<ForeignChildSlotComponent>(slot).is_some());
    assert_eq!(
        world.get::<ChildSlotLifecycle>(slot),
        Some(&ChildSlotLifecycle::RegisteredInert)
    );
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .is_none()
    );
    assert!(
        world
            .resource::<QuarantineLeaseTable>()
            .unresolved_record(slot)
            .is_some_and(|record| record.claim() == source_claim)
    );
}

#[test]
fn invalid_child_mount_preflight_preserves_registry_manifest() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
    let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
        panic!("confirmed child slot must issue one child frame task")
    };
    let runtime = world.register_component::<Inherited<ComputedUiTargetCamera>>();
    let _other_runtime = world.register_component::<Inherited<ComputedUiRenderTargetInfo>>();
    let current = world
        .resource::<RouteEntityRegistry>()
        .record(mounted.slot())
        .unwrap()
        .manifest()
        .clone();
    assert!(!current.component_ids().contains(&runtime));
    world
        .entity_mut(mounted.slot())
        .insert(Inherited(ComputedUiTargetCamera::default()));
    world
        .entity_mut(mounted.slot())
        .insert(ChildSlotLifecycle::ProvisionalInert);

    let failure = realize_child_frame_deferred(
        &mut world,
        &parent,
        &mounted,
        &child_task,
        child_task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .expect_err("invalid lifecycle must fail before realization");
    let (error, residual) = failure.into_parts();

    assert_eq!(error, crate::presentation::RealizationError::InvalidMount);
    assert!(residual.is_none());
    assert_eq!(
        world
            .resource::<RouteEntityRegistry>()
            .record(mounted.slot())
            .unwrap()
            .manifest(),
        &current
    );
}

#[test]
fn descendant_focus_assigned_by_stage_observer_is_cleared() {
    let mut world = World::new();
    world.insert_resource(InputFocus::default());
    world.insert_resource(RouteFocusState::default());
    world
        .register_component_hooks::<DescendantFocusProbe>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            let descendant = world.commands().spawn(ChildOf(entity)).id();
            *world
                .get_resource_mut::<InputFocus>()
                .expect("input focus is installed") = InputFocus::from_entity(descendant);
            world
                .get_resource_mut::<RouteFocusState>()
                .expect("route focus is installed")
                .set_focused(Some(descendant));
        });
    let slot = world.spawn(DescendantFocusProbe).id();
    world.flush();

    assert!(focus_is_in_slot(&world, slot));
    clear_slot_focus(&mut world, slot);
    assert_eq!(world.resource::<InputFocus>().get(), None);
    assert_eq!(world.resource::<RouteFocusState>().focused(), None);
}

#[test]
fn child_slot_pre_cas_drift_rejects_session_with_provisional_residual() {
    let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
    let slot = staged.slot();
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    world.entity_mut(slot).remove::<RouterInputBarrier>();
    focus_child_slot(&mut world, slot);

    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect_err("pre-CAS drift must reject the borrowed session");
    assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::ReceiptDrift);
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(cleanup.is_some());
    assert!(begin_error.is_none());
    assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
    assert!(matches!(
        world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .unwrap()
            .owner(),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
    ));
    assert!(engine.action().is_none());
    assert!(!focus_is_in_slot(&world, slot));
}

#[test]
fn child_slot_foreign_anchor_child_drift_rejects_before_owner_cas() {
    let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
    let slot = staged.slot();
    let anchor = staged.receipt.anchor;
    let anchor_manifest = staged.receipt.anchor_manifest.clone();
    let foreign = world.spawn(ChildOf(anchor)).id();
    world.flush();
    assert_eq!(
        world
            .resource::<RouteEntityRegistry>()
            .record(anchor)
            .unwrap()
            .manifest(),
        &anchor_manifest,
        "foreign child order drift must not need an anchor manifest change",
    );
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    focus_child_slot(&mut world, slot);

    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect_err("foreign anchor child drift must reject the borrowed session");
    assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::ReceiptDrift);
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(cleanup.is_some());
    assert!(begin_error.is_none());
    assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
    assert!(matches!(
        world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .unwrap()
            .owner(),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
    ));
    assert_eq!(
        world.get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(anchor)
    );
    assert!(!focus_is_in_slot(&world, slot));
    assert!(engine.action().is_none());
}

#[test]
fn child_slot_post_cas_node_value_drift_rejects_registered_residual() {
    let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    world.insert_resource(RegistrationDriftArmed::default());
    world.insert_resource(InputFocus::default());
    world.insert_resource(RouteFocusState::default());
    world
        .register_component_hooks::<ChildSlotLifecycle>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            if world
                .get_resource::<RegistrationDriftArmed>()
                .is_some_and(|armed| armed.0)
            {
                world.commands().entity(entity).insert(Node {
                    width: Val::Percent(99.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                });
                *world
                    .get_resource_mut::<InputFocus>()
                    .expect("input focus resource is installed") = InputFocus::from_entity(entity);
                world
                    .get_resource_mut::<RouteFocusState>()
                    .expect("route focus resource is installed")
                    .set_focused(Some(entity));
            }
        });
    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
    let slot = staged.slot();
    let slot_id = grant.slot();
    let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    world.resource_mut::<RegistrationDriftArmed>().0 = true;
    assert_eq!(
        world.get::<Node>(slot).map(|node| node.width),
        Some(Val::Percent(100.0)),
    );
    assert!(receipt_matches_provisional(
        &world,
        &staged.provisional,
        &staged.receipt,
        slot_id,
    ));

    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect_err("post-CAS drift must reject the borrowed session");
    assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::PostCasDrift);
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(cleanup.is_some());
    assert!(begin_error.is_none());
    assert_eq!(residual.state(), ChildSlotResidualState::Registered);
    assert!(matches!(
        world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .unwrap()
            .owner(),
        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
    ));
    assert_eq!(
        world.get::<Node>(slot).map(|node| node.width),
        Some(Val::Percent(99.0)),
    );
    assert_residual_inert(&world, &residual);
    assert_residual_barrier_denied(&world, &residual);
    assert!(!focus_is_in_slot(&world, slot));
    assert!(engine.action().is_none());
}

#[test]
fn child_slot_foreign_proof_is_rejected_without_owner_cas_or_child_action() {
    let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
    let slot = staged.slot();
    let (attestation, _proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(attestation).unwrap();
    focus_child_slot(&mut world, slot);

    let (
        mut foreign_world,
        _candidate,
        _top_slot,
        mut foreign_engine,
        foreign_frame,
        _,
        foreign_grant,
    ) = realized_parent_with_outlet();
    let foreign_staged = stage_child_slot(
        &mut foreign_world,
        &foreign_frame,
        &RouteSlotKey::default(),
        &foreign_grant,
    )
    .unwrap();
    let (foreign_attestation, foreign_proof) =
        foreign_grant.attest_receipt_with_proof(foreign_staged.receipt().digest());
    let _foreign_permit = foreign_engine
        .accept_mount_receipt(foreign_attestation)
        .unwrap();

    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &foreign_proof)
        .expect_err("foreign proof must fail before owner CAS");
    assert_eq!(
        failure.kind(),
        ChildSlotConfirmationErrorKind::RegistrationBegin
    );
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(cleanup.is_some());
    assert!(begin_error.is_none());
    assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
    assert!(matches!(
        world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .unwrap()
            .owner(),
        RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
    ));
    assert!(engine.action().is_none());
    assert!(!focus_is_in_slot(&world, slot));
}

pub(crate) fn verify_mount_grant_observer_drift_contract() {
    authority_batch_rejects_anchor_children_order_drift_in_the_same_batch();
    child_slot_foreign_anchor_child_drift_rejects_before_owner_cas();
    child_slot_post_cas_node_value_drift_rejects_registered_residual();
    child_slot_seal_and_slot_id_are_not_reused_after_failure();
}

fn world_registry_snapshot(
    world: &World,
) -> (
    Vec<(Entity, Vec<bevy_ecs::component::ComponentId>)>,
    Vec<crate::registry::RouteEntityRecord>,
    Option<crate::registry::OwnershipSeal>,
) {
    let mut entities = world
        .iter_entities()
        .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
        .collect::<Vec<_>>();
    entities.sort_by_key(|(entity, _)| entity.to_bits());
    (
        entities,
        world.resource::<RouteEntityRegistry>().records().to_vec(),
        world.resource::<RouteEntityRegistry>().latest_issued_seal(),
    )
}

fn assert_terminal_mount_cleanup(
    world: &mut World,
    engine: &mut RouterEngine,
    parent: RealizedFrame,
    residual: Option<ChildSlotResidual>,
    request: bevy_ui_router_core::integration::FailureCleanupRequest,
) {
    let mut branch = crate::branch_transaction::PreparedBranch::empty();
    branch.push_frame(parent).unwrap();
    let failed = match residual {
        Some(residual) => {
            crate::branch_transaction::FailedPreparedBranch::with_mount_residual(branch, residual)
        }
        None => crate::branch_transaction::FailedPreparedBranch::new(branch),
    };
    world.insert_resource(InputFocus::default());
    world.insert_resource(RouteFocusState::default());
    let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
        world,
        world.resource::<RouteEntityRegistry>(),
        &failed,
    )
    .expect("exact failed mount state must freeze one physical manifest");
    let manifest_digest = manifest.digest();
    let obligation_count = manifest.len();
    let reserved = crate::cleanup::CleanupLedger::reserve(manifest)
        .expect("failed mount cleanup must reserve its receipt ledger");
    let request_id = request.request();
    let logical_roots = request.logical_roots().to_vec();
    let cleanup = match engine.prepare_failure_cleanup_retaining(
        request,
        CleanupManifest::new(logical_roots, manifest_digest, obligation_count),
    ) {
        FailureCleanupPrepareOutcome::Prepared(cleanup) => cleanup,
        FailureCleanupPrepareOutcome::Mismatch(_) | FailureCleanupPrepareOutcome::Permanent(_) => {
            panic!("exact failed mount cleanup must prepare")
        }
    };
    let mut ledger =
        crate::cleanup::CleanupLedger::activate_failed(reserved.bind_batch(cleanup), failed);
    let attempt = ledger.attempt(world).unwrap();
    assert!(ledger.settle(world, attempt));
    let completion = ledger
        .pending_completion()
        .expect("every failed mount obligation must have one completion receipt");
    let owner = engine
        .prepare_failure_cleanup_completion(completion)
        .expect("the settled mount cleanup must bind its exact Core receiver");
    engine
        .complete_prepared_failure_cleanup_with_ledger(owner, ledger, |mut ledger| {
            drop(
                ledger
                    .take_failed_prepared_owner()
                    .expect("mount cleanup completion retains its physical owner"),
            );
        })
        .unwrap();
    match engine.request(request_id).unwrap().status() {
        RequestStatus::Running(NavigationPhase::Recovering) => {
            let EngineAction::FrameTask(fallback) = engine.action().unwrap() else {
                panic!("ordinary failed mount cleanup must continue through RouterFallback")
            };
            assert_eq!(fallback.frame().origin(), &FrameOrigin::RouterFallback);
            let recovery_failure = engine
                .report_failed(
                    fallback,
                    ViewFailureKind::Build,
                    "mount cleanup fixture finishes its RouterFallback attempt",
                )
                .unwrap()
                .expect("fallback failure must retain cleanup authority before terminal");
            let recovery_cleanup = match engine.prepare_failure_cleanup_retaining(
                recovery_failure,
                CleanupManifest::new(Vec::new(), 0x4D4F_554F, 0),
            ) {
                FailureCleanupPrepareOutcome::Prepared(cleanup) => cleanup,
                FailureCleanupPrepareOutcome::Mismatch(_)
                | FailureCleanupPrepareOutcome::Permanent(_) => {
                    panic!("exact fallback cleanup must prepare")
                }
            };
            engine
                .complete_cleanup(recovery_cleanup.complete_empty())
                .unwrap();
        }
        RequestStatus::Failed(_) => assert!(engine.action().is_none()),
        status => panic!("failed mount cleanup produced unexpected status {status:?}"),
    }
    assert!(engine.snapshot().running_request().is_none());
    assert!(engine.action().is_none());
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .all(|record| !matches!(record.owner(), RegistryOwner::Provisional(_)))
    );
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .all(|record| !matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
            ))
    );
    let child_slot_entities = world
        .iter_entities()
        .filter(|entity| entity.get::<ChildSlotLifecycle>().is_some())
        .map(|entity| entity.id())
        .collect::<Vec<_>>();
    let leases = world.resource::<QuarantineLeaseTable>();
    for entity in child_slot_entities {
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .is_some()
                || leases.record(entity).is_some()
                || leases.unresolved_record(entity).is_some(),
            "a surviving child-slot entity must have an exact terminal owner",
        );
    }
}

pub(crate) fn verify_mount_artifact_replay_contract() {
    let (
        receiver_world,
        _candidate,
        _top_slot,
        mut request_receiver,
        _parent,
        _local_index,
        receiver_request,
    ) = realized_parent_awaiting_mount();
    let (
        _source_world,
        _candidate,
        _top_slot,
        _request_source,
        _parent,
        _local_index,
        foreign_request,
    ) = realized_parent_awaiting_mount();
    let before = world_registry_snapshot(&receiver_world);
    let core_before = request_receiver.snapshot();
    let foreign_error = match request_receiver.resolve_mount(foreign_request) {
        Ok(_) => panic!("a foreign request must not allocate a receiver slot"),
        Err(error) => error,
    };
    assert_eq!(foreign_error.kind(), IntegrationError::ForeignRouter);
    assert_eq!(request_receiver.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&receiver_world), before);
    assert!(request_receiver.action().is_none());
    let receiver_slot = request_receiver
        .resolve_mount(receiver_request)
        .unwrap()
        .slot();
    let (_world, _candidate, _top_slot, mut control, _parent, _index, control_request) =
        realized_parent_awaiting_mount();
    assert_eq!(
        receiver_slot,
        control.resolve_mount(control_request).unwrap().slot(),
        "foreign request rejection must not advance the receiver slot allocator"
    );

    let (
        mut grant_world,
        _candidate,
        _top_slot,
        mut grant_receiver,
        grant_parent,
        _index,
        receiver_grant,
    ) = realized_parent_with_outlet();
    let (_world, _candidate, _top_slot, _source, _parent, _index, foreign_grant) =
        realized_parent_with_outlet();
    let before = world_registry_snapshot(&grant_world);
    let core_before = grant_receiver.snapshot();
    let failure = stage_child_slot(
        &mut grant_world,
        &grant_parent,
        &RouteSlotKey::default(),
        &foreign_grant,
    )
    .expect_err("a foreign grant must fail before child-slot staging");
    assert_eq!(failure.kind(), ChildSlotStageErrorKind::GrantMismatch);
    assert!(failure.into_residual().is_none());
    assert_eq!(grant_receiver.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&grant_world), before);
    assert!(grant_receiver.action().is_none());
    let exact_staged = stage_child_slot(
        &mut grant_world,
        &grant_parent,
        &RouteSlotKey::default(),
        &receiver_grant,
    )
    .expect("the exact receiver grant must remain usable");
    assert!(grant_world.get_entity(exact_staged.slot()).is_ok());

    let (
        mut receipt_world,
        _candidate,
        _top_slot,
        mut receipt_receiver,
        receipt_parent,
        _index,
        receiver_grant,
    ) = realized_parent_with_outlet();
    let staged = stage_child_slot(
        &mut receipt_world,
        &receipt_parent,
        &RouteSlotKey::default(),
        &receiver_grant,
    )
    .unwrap();
    let (receiver_receipt, _receiver_proof) =
        receiver_grant.attest_receipt_with_proof(staged.receipt().digest());
    let (_world, _candidate, _top_slot, _source, _parent, _index, foreign_grant) =
        realized_parent_with_outlet();
    let (foreign_receipt, _foreign_proof) = foreign_grant.attest_receipt_with_proof(0x4652_4354);
    let before = world_registry_snapshot(&receipt_world);
    let core_before = receipt_receiver.snapshot();
    let foreign_error = match receipt_receiver.accept_mount_receipt(foreign_receipt) {
        Ok(_) => panic!("a foreign receipt must not issue a receiver permit"),
        Err(error) => error,
    };
    assert_eq!(foreign_error.kind(), IntegrationError::ForeignRouter);
    assert_eq!(receipt_receiver.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&receipt_world), before);
    assert!(receipt_receiver.action().is_none());
    assert!(
        receipt_receiver
            .accept_mount_receipt(receiver_receipt)
            .is_ok()
    );

    let (
        mut permit_world,
        _candidate,
        _top_slot,
        mut permit_receiver,
        permit_parent,
        _index,
        receiver_grant,
    ) = realized_parent_with_outlet();
    let staged = stage_child_slot(
        &mut permit_world,
        &permit_parent,
        &RouteSlotKey::default(),
        &receiver_grant,
    )
    .unwrap();
    let (receiver_receipt, receiver_proof) =
        receiver_grant.attest_receipt_with_proof(staged.receipt().digest());
    let receiver_permit = permit_receiver
        .accept_mount_receipt(receiver_receipt)
        .unwrap();
    let (
        mut source_world,
        _candidate,
        _top_slot,
        mut permit_source,
        source_parent,
        _index,
        source_grant,
    ) = realized_parent_with_outlet();
    let source_staged = stage_child_slot(
        &mut source_world,
        &source_parent,
        &RouteSlotKey::default(),
        &source_grant,
    )
    .unwrap();
    let (source_receipt, _source_proof) =
        source_grant.attest_receipt_with_proof(source_staged.receipt().digest());
    let foreign_permit = permit_source.accept_mount_receipt(source_receipt).unwrap();
    let before = world_registry_snapshot(&permit_world);
    let core_before = permit_receiver.snapshot();
    let foreign_error =
        match permit_receiver.begin_mount_registration(foreign_permit, &receiver_proof) {
            Ok(_) => panic!("a foreign permit must not open a receiver session"),
            Err(error) => error,
        };
    assert_eq!(foreign_error.kind(), IntegrationError::ForeignRouter);
    assert_eq!(permit_receiver.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&permit_world), before);
    assert!(permit_receiver.action().is_none());
    let exact_session = permit_receiver
        .begin_mount_registration(receiver_permit, &receiver_proof)
        .expect("the exact receiver permit must remain usable");
    drop(exact_session);

    fn completed_foreign_replays() -> bevy_ui_router_core::integration::MountRegistrationReplays {
        let (mut world, _candidate, _top_slot, mut engine, parent, _index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let (receipt, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(receipt).unwrap();
        engine
            .begin_mount_registration(permit, &proof)
            .unwrap()
            .testing_complete_with_replays(0x4652_504c)
            .unwrap()
    }

    let foreign_replays = completed_foreign_replays();
    let (
        mut confirmation_world,
        _candidate,
        _top_slot,
        mut confirmation_receiver,
        confirmation_parent,
        _index,
        receiver_grant,
    ) = realized_parent_with_outlet();
    let staged = stage_child_slot(
        &mut confirmation_world,
        &confirmation_parent,
        &RouteSlotKey::default(),
        &receiver_grant,
    )
    .unwrap();
    let (receiver_receipt, receiver_proof) =
        receiver_grant.attest_receipt_with_proof(staged.receipt().digest());
    let receiver_permit = confirmation_receiver
        .accept_mount_receipt(receiver_receipt)
        .unwrap();
    let before = world_registry_snapshot(&confirmation_world);
    let core_before = confirmation_receiver.snapshot();
    assert_eq!(
        confirmation_receiver.testing_replay_mount_completion(&foreign_replays),
        Err(IntegrationError::ForeignRouter)
    );
    assert_eq!(confirmation_receiver.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&confirmation_world), before);
    assert!(confirmation_receiver.action().is_none());
    assert_eq!(
        confirmation_receiver.testing_replay_mount_confirmation(&foreign_replays),
        Err(IntegrationError::ForeignRouter)
    );
    assert_eq!(confirmation_receiver.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&confirmation_world), before);
    assert!(confirmation_receiver.action().is_none());
    confirmation_receiver
        .begin_mount_registration(receiver_permit, &receiver_proof)
        .unwrap()
        .testing_complete_with_replays(0x4558_4143)
        .unwrap();
    assert!(matches!(
        confirmation_receiver.action(),
        Some(EngineAction::FrameTask(_))
    ));

    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, request) =
        realized_parent_awaiting_mount();
    let request_replay = request.testing_duplicate();
    let grant = engine.resolve_mount(request).unwrap();
    let first_slot = grant.slot();
    let before = world_registry_snapshot(&world);
    let core_before = engine.snapshot();
    let request_error = match engine.resolve_mount(request_replay) {
        Ok(_) => panic!("a consumed mount request must not allocate another grant"),
        Err(error) => error,
    };
    assert_eq!(request_error.kind(), IntegrationError::Stale);
    assert_eq!(engine.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&world), before);

    let grant_replay = grant.testing_duplicate();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
        .expect("the exact grant must stage one provisional child slot");
    let (receipt, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let receipt_replay = receipt.testing_duplicate();
    let permit = engine.accept_mount_receipt(receipt).unwrap();

    let before = world_registry_snapshot(&world);
    let core_before = engine.snapshot();
    let grant_error =
        match engine.reject_mount_after_mutation(MountMutationArtifact::Grant(grant_replay)) {
            Ok(_) => panic!("a consumed grant must not authorize another mutation terminal"),
            Err(error) => error,
        };
    assert_eq!(grant_error.kind(), IntegrationError::Stale);
    assert_eq!(engine.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&world), before);

    let before = world_registry_snapshot(&world);
    let core_before = engine.snapshot();
    let receipt_error = match engine.accept_mount_receipt(receipt_replay) {
        Ok(_) => panic!("a consumed receipt must not issue another permit"),
        Err(error) => error,
    };
    assert_eq!(receipt_error.kind(), IntegrationError::Stale);
    assert_eq!(engine.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&world), before);

    let permit_replay = permit.testing_duplicate();
    let session = engine.begin_mount_registration(permit, &proof).unwrap();
    let replays = session.testing_complete_with_replays(0x5250_4c59).unwrap();
    let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
        panic!("the exact completion must issue exactly one child action")
    };

    let before = world_registry_snapshot(&world);
    let core_before = engine.snapshot();
    let permit_error = match engine.begin_mount_registration(permit_replay, &proof) {
        Ok(_) => panic!("a consumed permit must fail before provisional-to-registered CAS"),
        Err(error) => error,
    };
    assert_eq!(permit_error.kind(), IntegrationError::Stale);
    assert_eq!(engine.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&world), before);

    let before = world_registry_snapshot(&world);
    let core_before = engine.snapshot();
    assert_eq!(
        engine.testing_replay_mount_completion(&replays),
        Err(IntegrationError::Stale),
    );
    assert_eq!(engine.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&world), before);

    let before = world_registry_snapshot(&world);
    let core_before = engine.snapshot();
    assert_eq!(
        engine.testing_replay_mount_confirmation(&replays),
        Err(IntegrationError::Stale),
    );
    assert_eq!(engine.snapshot(), core_before);
    assert_eq!(world_registry_snapshot(&world), before);
    assert!(engine.action().is_none());

    let cleanup_request = engine
        .report_failed(
            child_task,
            bevy_ui_router_core::integration::ViewFailureKind::Apply,
            "finish replay fixture",
        )
        .unwrap()
        .unwrap();
    let residual = staged.into_residual(&mut world);
    assert_terminal_mount_cleanup(
        &mut world,
        &mut engine,
        parent,
        Some(residual),
        cleanup_request,
    );

    engine.push(RouteLocation::new("/items/2")).unwrap();
    engine.collect();
    let EngineAction::FrameTask(parent_task) = engine.action().unwrap() else {
        panic!("the next request must issue a parent task")
    };
    engine
        .report_applied_with_outlets(parent_task, vec![RouteSlotKey::default()])
        .unwrap();
    let EngineAction::MountRequest(next_request) = engine.action().unwrap() else {
        panic!("the next request must issue one mount request")
    };
    let next_slot = engine.resolve_mount(next_request).unwrap().slot();
    assert_eq!(next_slot.raw(), first_slot.raw() + 1);
}

fn stage_failure_settles_without_orphan(
    install_drift: impl FnOnce(&mut World),
    expected: ChildSlotStageErrorKind,
) {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    install_drift(&mut world);
    let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
        .expect_err("the selected precommit stage drift must fail closed");
    assert_eq!(failure.kind(), expected);
    let residual = failure
        .into_residual()
        .expect("observer-capable stage drift must retain one cleanup residual");
    let cleanup = engine
        .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
        .unwrap();
    assert_terminal_mount_cleanup(&mut world, &mut engine, parent, Some(residual), cleanup);
}

fn registry_rejection_settles_without_orphan() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
        .expect("the first exact slot stages");
    let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
        .expect_err("the duplicate logical owner must fail before an empty entity survives");
    assert_eq!(failure.kind(), ChildSlotStageErrorKind::RegistryRejected);
    assert!(failure.into_residual().is_none());
    let residual = staged.into_residual(&mut world);
    let cleanup = engine
        .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
        .unwrap();
    assert_terminal_mount_cleanup(&mut world, &mut engine, parent, Some(residual), cleanup);
}

fn preallocation_failure_settles_without_orphan(scenario: ChildSlotStageErrorKind) {
    match scenario {
        ChildSlotStageErrorKind::MissingFoundation => {
            let (mut world, _candidate, _top_slot, mut engine, parent, _, grant) =
                realized_parent_with_outlet();
            let registry = world.remove_resource::<RouteEntityRegistry>().unwrap();
            let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
                .expect_err("missing registry foundation must fail before allocation");
            assert_eq!(failure.kind(), scenario);
            assert!(failure.into_residual().is_none());
            world.insert_resource(registry);
            let cleanup = engine
                .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
                .unwrap();
            assert_terminal_mount_cleanup(&mut world, &mut engine, parent, None, cleanup);
        }
        ChildSlotStageErrorKind::GrantMismatch => {
            let (mut world, _candidate, _top_slot, mut engine, parent, _, grant) =
                realized_parent_with_outlet();
            let failure = stage_child_slot(
                &mut world,
                &parent,
                &RouteSlotKey::named("wrong").unwrap(),
                &grant,
            )
            .expect_err("wrong outlet binding must reject the grant without allocation");
            assert_eq!(failure.kind(), scenario);
            assert!(failure.into_residual().is_none());
            let cleanup = engine
                .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
                .unwrap();
            assert_terminal_mount_cleanup(&mut world, &mut engine, parent, None, cleanup);
        }
        ChildSlotStageErrorKind::MissingOutlet => {
            let (mut world, mut engine, parent, grant) = realized_parent_without_outlet();
            let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
                .expect_err("a logical outlet without a realized anchor must fail closed");
            assert_eq!(failure.kind(), scenario);
            assert!(failure.into_residual().is_none());
            let cleanup = engine
                .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
                .unwrap();
            assert_terminal_mount_cleanup(&mut world, &mut engine, parent, None, cleanup);
        }
        ChildSlotStageErrorKind::InvalidOutlet => {
            let (mut world, _candidate, _top_slot, mut engine, parent, _, grant) =
                realized_parent_with_outlet();
            let anchor = parent
                .outlet(&RouteSlotKey::default())
                .expect("fixture has a default outlet")
                .anchor();
            world.entity_mut(anchor).insert(ForeignChildSlotComponent);
            let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
                .expect_err("drifted outlet anchor must fail before child-slot allocation");
            assert_eq!(failure.kind(), scenario);
            assert!(failure.into_residual().is_none());
            let cleanup = engine
                .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
                .unwrap();
            assert_terminal_mount_cleanup(&mut world, &mut engine, parent, None, cleanup);
        }
        _ => panic!("scenario must be one preallocation failure class"),
    }
}

fn owner_cas_rejection_settles_without_orphan() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let (receipt, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    reject_next_child_slot_owner_cas();
    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect_err("injected owner CAS rejection must retain terminal cleanup authority");
    assert_eq!(
        failure.kind(),
        ChildSlotConfirmationErrorKind::OwnerCasRejected
    );
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(begin_error.is_none());
    assert_terminal_mount_cleanup(
        &mut world,
        &mut engine,
        parent,
        Some(residual),
        cleanup.expect("owner CAS rejection retains exact cleanup authority"),
    );
}

fn pre_cas_receipt_drift_settles_without_orphan() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let slot = staged.slot();
    let (receipt, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    world.entity_mut(slot).remove::<RouterInputBarrier>();
    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect_err("pre-CAS receipt drift must reject registration");
    assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::ReceiptDrift);
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(begin_error.is_none());
    assert_terminal_mount_cleanup(
        &mut world,
        &mut engine,
        parent,
        Some(residual),
        cleanup.expect("receipt drift retains exact cleanup authority"),
    );
}

fn post_cas_drift_settles_without_orphan() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    world.insert_resource(RegistrationDriftArmed::default());
    world.insert_resource(InputFocus::default());
    world.insert_resource(RouteFocusState::default());
    world
        .register_component_hooks::<ChildSlotLifecycle>()
        .on_insert(|mut world, HookContext { entity, .. }| {
            if world
                .get_resource::<RegistrationDriftArmed>()
                .is_some_and(|armed| armed.0)
            {
                world.commands().entity(entity).insert(Node {
                    width: Val::Percent(99.0),
                    ..Node::default()
                });
            }
        });
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let (receipt, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(receipt).unwrap();
    world.resource_mut::<RegistrationDriftArmed>().0 = true;
    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
        .expect_err("post-CAS observer drift must reject registration");
    assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::PostCasDrift);
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(begin_error.is_none());
    assert_terminal_mount_cleanup(
        &mut world,
        &mut engine,
        parent,
        Some(residual),
        cleanup.expect("post-CAS drift retains exact cleanup authority"),
    );
}

fn foreign_registration_proof_settles_without_orphan() {
    let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
        realized_parent_with_outlet();
    let staged = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
    let (receipt, _proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
    let permit = engine.accept_mount_receipt(receipt).unwrap();

    let (
        mut foreign_world,
        _candidate,
        _top_slot,
        mut foreign_engine,
        foreign_parent,
        _,
        foreign_grant,
    ) = realized_parent_with_outlet();
    let foreign_staged = stage_child_slot(
        &mut foreign_world,
        &foreign_parent,
        &RouteSlotKey::default(),
        &foreign_grant,
    )
    .unwrap();
    let (foreign_receipt, foreign_proof) =
        foreign_grant.attest_receipt_with_proof(foreign_staged.receipt().digest());
    let _foreign_permit = foreign_engine
        .accept_mount_receipt(foreign_receipt)
        .unwrap();

    let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &foreign_proof)
        .expect_err("foreign proof must reject before owner CAS");
    assert_eq!(
        failure.kind(),
        ChildSlotConfirmationErrorKind::RegistrationBegin
    );
    let (cleanup, begin_error, residual) = failure.into_parts();
    assert!(begin_error.is_none());
    assert_terminal_mount_cleanup(
        &mut world,
        &mut engine,
        parent,
        Some(residual),
        cleanup.expect("foreign proof retains source cleanup authority"),
    );
}

pub(crate) fn verify_child_slot_precommit_lifecycle_contract() {
    preallocation_failure_settles_without_orphan(ChildSlotStageErrorKind::MissingFoundation);
    preallocation_failure_settles_without_orphan(ChildSlotStageErrorKind::GrantMismatch);
    preallocation_failure_settles_without_orphan(ChildSlotStageErrorKind::MissingOutlet);
    preallocation_failure_settles_without_orphan(ChildSlotStageErrorKind::InvalidOutlet);
    stage_failure_settles_without_orphan(
        |world| {
            world
                .register_component_hooks::<ChildSlotBarrierProbe>()
                .on_insert(|mut world, HookContext { entity, .. }| {
                    world
                        .commands()
                        .entity(entity)
                        .remove::<RouterInputBarrier>();
                });
        },
        ChildSlotStageErrorKind::BarrierDrift,
    );
    stage_failure_settles_without_orphan(
        |world| {
            world
                .register_component_hooks::<ChildSlotNodeProbe>()
                .on_insert(|mut world, HookContext { entity, .. }| {
                    world
                        .commands()
                        .entity(entity)
                        .insert(ForeignChildSlotComponent);
                });
        },
        ChildSlotStageErrorKind::NodeDrift,
    );
    stage_failure_settles_without_orphan(
        |world| {
            world
                .register_component_hooks::<ChildSlotAuthorityProbe>()
                .on_insert(|mut world, HookContext { entity, .. }| {
                    world
                        .commands()
                        .entity(entity)
                        .remove::<RouteOwnershipSeal>();
                });
        },
        ChildSlotStageErrorKind::AuthorityDrift,
    );
    stage_failure_settles_without_orphan(
        |world| {
            let foreign_parent = world.spawn_empty().id();
            world.insert_resource(ForeignParent(foreign_parent));
            world
                .register_component_hooks::<ChildSlotRelationshipProbe>()
                .on_insert(|mut world, HookContext { entity, .. }| {
                    let parent = world.resource::<ForeignParent>().0;
                    world.commands().entity(entity).insert(ChildOf(parent));
                });
        },
        ChildSlotStageErrorKind::RelationshipDrift,
    );
    stage_failure_settles_without_orphan(
        |world| {
            world
                .register_component_hooks::<ChildSlotAuthorityProbe>()
                .on_insert(|mut world, HookContext { entity, .. }| {
                    world.commands().entity(entity).despawn();
                });
        },
        ChildSlotStageErrorKind::AuthorityDrift,
    );
    registry_rejection_settles_without_orphan();
    pre_cas_receipt_drift_settles_without_orphan();
    foreign_registration_proof_settles_without_orphan();
    owner_cas_rejection_settles_without_orphan();
    post_cas_drift_settles_without_orphan();
}
