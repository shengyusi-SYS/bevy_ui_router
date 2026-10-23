use super::*;
use crate::{
    app_ext::{AppRouterExt, RouterConfig, RouterHandle, RouterHostRegistration, TestAppRouterExt},
    content::{ContentColor, ContentPlan, FrameContentWriter, SolidRect},
    driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
    host::{HostCandidateClassification, HostCandidateRevisionState, classify_host_candidate},
    navigation::{
        RouterNavigation, collect_navigation_for_test as collect_navigation_system,
        commit_presentation_system, drive_navigation_system, observe_authority_system,
        settle_cleanup_system,
    },
    plugin::BevyUiRouterPlugin,
    registry::QuarantineLeaseTable,
};
use bevy_app::{App, PostUpdate, PreUpdate};
use bevy_camera::visibility::InheritedVisibility;
use bevy_ecs::{
    component::Component,
    event::EntityEvent,
    lifecycle::{HookContext, Insert},
    observer::On,
    prelude::{Commands, Resource},
    query::With,
};
use bevy_internal::{
    DefaultPlugins,
    camera::{Camera2d, NormalizedRenderTarget},
    math::Vec2,
    picking::{
        backend::PointerHits,
        hover::HoverMap,
        pointer::{Location, PointerId, PointerLocation},
    },
    window::{PrimaryWindow, Window, WindowRef},
};
use bevy_picking::events::{Over, Pointer};
use bevy_ui::{ComputedNode, UiGlobalTransform};
use bevy_ui_router_core::{
    integration::{EngineAction, RootFrameTask, RouterEngine},
    prelude::{
        RouteLocation, RouteNodeOptions, RoutePattern, RouteSlotKey, RouteTree, RouteViewKey,
    },
};
use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey};

#[derive(Resource, Default)]
struct RealizationStageAudit(Vec<bool>);

#[derive(Resource)]
struct PersistentPickableRemoval;

#[derive(Component)]
struct ForeignRealizationComponent;

#[derive(Component)]
struct ProtectedIsolationMarker;

struct NestedPickingDriver {
    key: ViewDriverKey,
}

impl EcsViewDriver for NestedPickingDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        content
            .spawn_solid_rect(SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
            .unwrap();
        if effect.view() == &RouteViewKey::new("layout").unwrap() {
            let anchor = content.spawn_empty().unwrap();
            content
                .declare_outlet(anchor, RouteSlotKey::default())
                .unwrap();
        }
        report.applied()
    }
}

#[derive(Resource, Default)]
struct NestedTargetDeliveries(std::collections::BTreeMap<Entity, usize>);

fn install_nested_pointer_observer(app: &mut App) {
    app.world_mut().add_observer(
        |event: On<Pointer<Over>>,
         mut deliveries: bevy_ecs::prelude::ResMut<NestedTargetDeliveries>| {
            *deliveries.0.entry(event.event_target()).or_default() += 1;
        },
    );
}

struct ActiveNestedTopology {
    nodes: Vec<Entity>,
    parent_primitive: Entity,
    child_primitive: Entity,
}

fn active_nested_topology(world: &World, host: Entity) -> ActiveNestedTopology {
    let registry = world.resource::<RouteEntityRegistry>();
    let top_slot = registry
        .records()
        .iter()
        .find_map(|record| {
            matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            )
            .then_some(record.entity())
        })
        .expect("active router must retain its top slot");
    let child_slot = registry
        .records()
        .iter()
        .find_map(|record| {
            (record.lifecycle() == EntityLifecycle::Active
                && matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                ))
            .then_some(record.entity())
        })
        .expect("active nested branch must retain its child slot");

    let parent_shell = world
        .iter_entities()
        .find_map(|entity| {
            (entity.get::<FrameLifecycle>() == Some(&FrameLifecycle(EntityLifecycle::Active))
                && entity.get::<FrameShell>().is_some()
                && entity.get::<ChildOf>().map(ChildOf::parent) == Some(top_slot))
            .then_some(entity.id())
        })
        .expect("active parent shell must be mounted under the top slot");
    let parent_frame = world.get::<FrameShell>(parent_shell).unwrap().0.clone();
    let parent_content = world
        .iter_entities()
        .find_map(|entity| {
            entity
                .get::<FrameContentTarget>()
                .is_some_and(|target| target.matches_frame(&parent_frame))
                .then_some(entity.id())
        })
        .expect("active parent content target must exist");
    let outlet_anchor = world
        .get::<ChildOf>(child_slot)
        .map(ChildOf::parent)
        .expect("active child slot must retain its outlet anchor");
    let child_shell = world
        .iter_entities()
        .find_map(|entity| {
            (entity.get::<FrameLifecycle>() == Some(&FrameLifecycle(EntityLifecycle::Active))
                && entity.get::<FrameShell>().is_some()
                && entity.get::<ChildOf>().map(ChildOf::parent) == Some(child_slot))
            .then_some(entity.id())
        })
        .expect("active child shell must be mounted under the child slot");
    let child_frame = world.get::<FrameShell>(child_shell).unwrap().0.clone();
    let child_content = world
        .iter_entities()
        .find_map(|entity| {
            entity
                .get::<FrameContentTarget>()
                .is_some_and(|target| target.matches_frame(&child_frame))
                .then_some(entity.id())
        })
        .expect("active child content target must exist");
    let primitive_for = |frame: &RouteFrameVersion| {
        world
            .iter_entities()
            .find_map(|entity| {
                entity
                    .get::<FrameOutput>()
                    .is_some_and(|output| output.frame == *frame && output.solid_rect.is_some())
                    .then_some(entity.id())
            })
            .expect("each active nested frame must publish one primitive")
    };

    assert_eq!(
        world.get::<ChildOf>(top_slot).map(ChildOf::parent),
        Some(host)
    );
    assert_eq!(
        world.get::<ChildOf>(parent_content).map(ChildOf::parent),
        Some(parent_shell)
    );
    assert_eq!(
        world.get::<ChildOf>(outlet_anchor).map(ChildOf::parent),
        Some(parent_content)
    );
    assert_eq!(
        world.get::<ChildOf>(child_content).map(ChildOf::parent),
        Some(child_shell)
    );

    ActiveNestedTopology {
        nodes: vec![
            host,
            top_slot,
            parent_shell,
            parent_content,
            outlet_anchor,
            child_slot,
            child_shell,
            child_content,
        ],
        parent_primitive: primitive_for(&parent_frame),
        child_primitive: primitive_for(&child_frame),
    }
}

fn interactive_outputs(world: &World, lifecycle: EntityLifecycle) -> Vec<Entity> {
    let mut outputs = world
        .iter_entities()
        .filter_map(|entity| {
            (entity
                .get::<FrameOutput>()
                .is_some_and(|output| output.solid_rect.is_some())
                && entity.get::<FrameLifecycle>() == Some(&FrameLifecycle(lifecycle)))
            .then_some(entity.id())
        })
        .collect::<Vec<_>>();
    outputs.sort_by_key(|entity| entity.to_bits());
    outputs
}

#[derive(Debug)]
struct NestedDriveSample {
    prepared_outputs: usize,
    committed_slots: usize,
    provisional_slots: usize,
    pending: Option<(usize, usize, bool)>,
}

fn drive_nested_until_prepared(app: &mut App) {
    let mut trace = Vec::new();
    for _ in 0..8 {
        drive_navigation_system(app.world_mut());
        let registry = app.world().resource::<RouteEntityRegistry>();
        let sample = NestedDriveSample {
            prepared_outputs: interactive_outputs(app.world(), EntityLifecycle::PreparedInert)
                .len(),
            committed_slots: registry
                .records()
                .iter()
                .filter(|record| {
                    record.lifecycle() == EntityLifecycle::PreparedInert
                        && matches!(
                            record.owner(),
                            RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                        )
                })
                .count(),
            provisional_slots: registry
                .records()
                .iter()
                .filter(|record| {
                    matches!(
                        record.owner(),
                        RegistryOwner::Provisional(
                            crate::registry::ProvisionalOwner::ChildSlot { .. }
                        )
                    )
                })
                .count(),
            pending: app
                .world()
                .resource::<RouterNavigation>()
                .pending_shape_for_test(),
        };
        let ready = sample.pending.is_some_and(|(frames, slots, final_task)| {
            final_task
                && sample.prepared_outputs == frames
                && sample.committed_slots == slots
                && sample.provisional_slots == 0
        });
        trace.push(sample);
        if ready {
            return;
        }
    }
    panic!("Nested branch did not reach the prepared commit gate: {trace:?}");
}

fn move_mouse_to(app: &mut App, primary_window: Entity, target: Entity) {
    let center = *app.world().get::<UiGlobalTransform>(target).unwrap() * Vec2::ZERO;
    let pointer = app
        .world_mut()
        .query::<(Entity, &PointerId)>()
        .iter(app.world())
        .find_map(|(entity, id)| (*id == PointerId::Mouse).then_some(entity))
        .expect("DefaultPlugins must install a mouse pointer");
    *app.world_mut().get_mut::<PointerLocation>(pointer).unwrap() =
        PointerLocation::new(Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Primary.normalize(Some(primary_window)).unwrap(),
            ),
            position: center,
        });
    app.world_mut().run_schedule(PreUpdate);
}

fn pending_frame_fixture() -> (
    World,
    Entity,
    ValidatedHostCandidate,
    Entity,
    ExactRegistryClaim,
    RootFrameTask,
) {
    let mut world = World::new();
    world.register_component_hooks::<Pickable>().on_insert(
        |mut world, HookContext { entity, .. }| {
            if world.get_resource::<PersistentPickableRemoval>().is_some()
                && world.get::<FrameOutput>(entity).is_some()
            {
                world.commands().entity(entity).remove::<Pickable>();
            }
        },
    );
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let router = RouterHandle::fixture(17, 19);
    let host = world
        .spawn((
            Node::default(),
            crate::app_ext::RouterHostRegistration::new(router),
        ))
        .id();
    let mut revisions = HostCandidateRevisionState::default();
    let HostCandidateClassification::Unique(candidate) =
        classify_host_candidate(&world, router, &mut revisions)
    else {
        panic!("host should classify")
    };
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap()
        .build()
        .unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut engine = RouterEngine::install(tree, fallback).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
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
    engine
        .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
        .unwrap();
    engine.collect();
    let EngineAction::FrameTask(task) = engine.action().unwrap() else {
        panic!("root task expected")
    };
    (world, host, candidate, top_slot, top_slot_claim, task)
}

fn realized_empty_fixture() -> (World, Entity, Entity, RealizedFrame) {
    let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
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
    (world, host, top_slot, frame)
}

#[test]
fn guarded_settlement_stops_writes_after_protected_partition_drift() {
    let (mut world, _host, _top_slot, frame) = realized_empty_fixture();
    let target = frame.shell();
    let protected = world.spawn(ProtectedIsolationMarker).id();
    world.add_observer(
        move |event: On<Insert, Visibility>, mut commands: Commands| {
            if event.entity == target {
                commands
                    .entity(protected)
                    .remove::<ProtectedIsolationMarker>();
            }
        },
    );
    world.entity_mut(target).remove::<Visibility>();
    world.entity_mut(target).remove::<Pickable>();
    world.entity_mut(target).remove::<RouterInputBarrier>();

    let isolated = isolate_frame_for_settlement_guarded(
        &mut world,
        &frame,
        &|claim| claim.entity() == target,
        &|world| world.get::<ProtectedIsolationMarker>(protected).is_some(),
    );

    assert!(!isolated);
    assert!(world.get::<ProtectedIsolationMarker>(protected).is_none());
    assert_eq!(world.get::<Visibility>(target), Some(&Visibility::Hidden));
    assert!(world.get::<Pickable>(target).is_none());
    assert!(world.get::<RouterInputBarrier>(target).is_none());
}

fn activate_realized_frame_for_committed_preflight(world: &mut World, frame: &mut RealizedFrame) {
    let transitions = frame.lifecycle_transitions(EntityLifecycle::Active);
    let registry_mutation = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .expect("the exact realized-frame claims must prepare one active transition");
    let resulting_claims = registry_mutation.resulting_claims().to_vec();
    assert!(frame.claims_can_rebind(
        &resulting_claims,
        &resulting_claims,
        EntityLifecycle::PreparedInert,
        EntityLifecycle::Active,
    ));
    let presentation = prepare_presentation_mutation(
        world,
        &frame.mutation_targets(
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
            true,
        ),
    )
    .expect("the realized frame must prepare one exact presentation transition");

    presentation.apply();
    registry_mutation.apply(&mut world.resource_mut::<RouteEntityRegistry>());
    frame.apply_claim_rebind(&resulting_claims, &resulting_claims);
    assert!(validate_active_frame(world, frame));
}

fn insert_single_foreign_shell_component(
    world: &mut World,
    frame: &RealizedFrame,
    expected_lifecycle: EntityLifecycle,
) {
    let shell = frame.shell();
    let realized_shell = frame
        .entities
        .iter()
        .find(|realized| realized.entity == shell)
        .expect("the realized frame must retain its shell fact");
    let expected_claim = realized_shell.claim.clone();
    let expected_parent = realized_shell.parent;
    let expected_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(shell)
        .expect("the realized shell must remain registered")
        .manifest()
        .clone();

    world.entity_mut(shell).insert(ForeignRealizationComponent);

    let actual_manifest = ComponentManifest::from_world(world, shell)
        .expect("the realized shell must remain observable");
    let foreign_component = world
        .components()
        .component_id::<ForeignRealizationComponent>()
        .expect("insertion must register the test-private foreign component");
    let additions = actual_manifest
        .component_ids()
        .iter()
        .copied()
        .filter(|component| !expected_manifest.component_ids().contains(component))
        .collect::<Vec<_>>();
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(shell)
        .expect("the realized shell must retain its registry record");

    assert_eq!(additions, vec![foreign_component]);
    assert_eq!(record.claim(), expected_claim);
    assert_eq!(record.lifecycle(), expected_lifecycle);
    assert_eq!(record.manifest(), &expected_manifest);
    assert_eq!(
        world.get::<RouteOwnershipSeal>(shell),
        Some(&RouteOwnershipSeal::new(expected_claim.seal()))
    );
    assert_eq!(
        world.get::<FrameLifecycle>(shell),
        Some(&FrameLifecycle(expected_lifecycle))
    );
    assert_eq!(
        world.get::<ChildOf>(shell).map(ChildOf::parent),
        Some(expected_parent)
    );
    assert!(
        expected_manifest
            .component_ids()
            .iter()
            .all(|component| actual_manifest.component_ids().contains(component)),
        "the foreign addition must not remove any registry-required shell component"
    );
}

#[test]
fn committed_current_shell_foreign_component_is_preserved_ready() {
    let (mut world, _host, _top_slot, mut frame) = realized_empty_fixture();
    activate_realized_frame_for_committed_preflight(&mut world, &mut frame);
    insert_single_foreign_shell_component(&mut world, &frame, EntityLifecycle::Active);

    assert!(
        matches!(
            classify_committed_frame(&world, &frame, CommittedFrameRole::Preserved, &[], &[],),
            CommittedFramePreflight::Ready
        ),
        "RED: a preserved committed shell with required manifest coverage and one protected foreign component must remain Ready"
    );
}

#[test]
fn committed_current_shell_foreign_component_requires_leaving_quarantine() {
    let (mut world, _host, _top_slot, mut frame) = realized_empty_fixture();
    activate_realized_frame_for_committed_preflight(&mut world, &mut frame);
    insert_single_foreign_shell_component(&mut world, &frame, EntityLifecycle::Active);

    assert!(
        matches!(
            classify_committed_frame(&world, &frame, CommittedFrameRole::Leaving, &[], &[],),
            CommittedFramePreflight::ReadyWithQuarantine
        ),
        "RED: a leaving committed shell with one protected foreign component must be ReadyWithQuarantine"
    );
}

#[test]
fn committed_current_shell_missing_required_component_is_invalid() {
    let (mut world, _host, _top_slot, mut frame) = realized_empty_fixture();
    activate_realized_frame_for_committed_preflight(&mut world, &mut frame);
    let shell = frame.shell();
    let pickable = world
        .components()
        .component_id::<Pickable>()
        .expect("the committed shell manifest must register Pickable");
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .record(shell)
            .expect("the committed shell must remain registered")
            .manifest()
            .component_ids()
            .contains(&pickable)
    );

    world.entity_mut(shell).remove::<Pickable>();

    assert!(world.get::<Pickable>(shell).is_none());
    assert!(matches!(
        classify_committed_frame(&world, &frame, CommittedFrameRole::Preserved, &[], &[],),
        CommittedFramePreflight::Invalid
    ));
    assert!(matches!(
        classify_committed_frame(&world, &frame, CommittedFrameRole::Leaving, &[], &[],),
        CommittedFramePreflight::Invalid
    ));
}

#[test]
fn prepared_current_shell_foreign_component_remains_invalid() {
    let (mut world, _host, _top_slot, frame) = realized_empty_fixture();
    insert_single_foreign_shell_component(&mut world, &frame, EntityLifecycle::PreparedInert);

    assert!(
        !validate_realized_frame(&world, &frame),
        "prepared and entering shell validation must retain strict manifest equality"
    );
}

fn presentation_commit_fixture(
    world: &mut World,
) -> (Entity, Entity, Entity, Vec<FrameMutationTarget>) {
    let leaving = world
        .spawn((
            Visibility::Visible,
            ACTIVE_PICKABLE,
            RouterInputBarrier::allow_all(),
            FrameLifecycle(EntityLifecycle::Active),
        ))
        .id();
    let entering = world
        .spawn((
            Visibility::Hidden,
            Pickable::IGNORE,
            RouterInputBarrier::deny_all(),
            FrameLifecycle(EntityLifecycle::PreparedInert),
        ))
        .id();
    let child_slot = world
        .spawn((
            Visibility::Hidden,
            Pickable::IGNORE,
            RouterInputBarrier::deny_all(),
            ChildSlotLifecycle::RegisteredInert,
        ))
        .id();
    let targets = vec![
        FrameMutationTarget {
            entity: leaving,
            current: EntityLifecycle::Active,
            next: EntityLifecycle::Leaving,
            active_after: false,
            interactive: true,
            kind: PresentationTargetKind::Frame,
        },
        FrameMutationTarget {
            entity: entering,
            current: EntityLifecycle::PreparedInert,
            next: EntityLifecycle::Active,
            active_after: true,
            interactive: true,
            kind: PresentationTargetKind::Frame,
        },
        FrameMutationTarget {
            entity: child_slot,
            current: EntityLifecycle::PreparedInert,
            next: EntityLifecycle::Active,
            active_after: true,
            interactive: false,
            kind: PresentationTargetKind::ChildSlot,
        },
    ];
    (leaving, entering, child_slot, targets)
}

#[test]
fn staged_commit_exposes_each_infallible_closure_boundary() {
    #[derive(Debug, Eq, PartialEq)]
    enum ClosureBoundary {
        Focus,
        Registry,
        Gate,
    }

    let mut world = World::new();
    let (leaving, entering, child_slot, targets) = presentation_commit_fixture(&mut world);
    let prepared = prepare_presentation_mutation(&mut world, &targets).unwrap();
    let commit = prepared.seal_commit(1).unwrap();
    let trace = std::cell::RefCell::new(Vec::new());

    commit.apply_staged(
        |state| {
            trace.borrow_mut().push((
                ClosureBoundary::Focus,
                state.leaving_is_isolated(),
                state.entering_is_prepared_inert(),
                state.entering_base_is_active(),
                state.entering_capabilities_are_active(),
            ));
        },
        |state| {
            trace.borrow_mut().push((
                ClosureBoundary::Registry,
                state.leaving_is_isolated(),
                state.entering_is_prepared_inert(),
                state.entering_base_is_active(),
                state.entering_capabilities_are_active(),
            ));
        },
        |state| {
            trace.borrow_mut().push((
                ClosureBoundary::Gate,
                state.leaving_is_isolated(),
                state.entering_is_prepared_inert(),
                state.entering_base_is_active(),
                state.entering_capabilities_are_active(),
            ));
        },
    );

    assert_eq!(
        trace.into_inner(),
        vec![
            (ClosureBoundary::Focus, true, true, false, false),
            (ClosureBoundary::Registry, true, false, true, false),
            (ClosureBoundary::Gate, true, false, true, true),
        ]
    );
    assert_eq!(world.get::<Visibility>(leaving), Some(&Visibility::Hidden));
    assert_eq!(world.get::<Pickable>(leaving), Some(&Pickable::IGNORE));
    assert!(
        world
            .get::<RouterInputBarrier>(leaving)
            .is_some_and(RouterInputBarrier::denies_all)
    );
    assert_eq!(
        world.get::<FrameLifecycle>(leaving),
        Some(&FrameLifecycle(EntityLifecycle::Leaving))
    );
    assert_eq!(
        world.get::<Visibility>(entering),
        Some(&Visibility::Visible)
    );
    assert_eq!(world.get::<Pickable>(entering), Some(&ACTIVE_PICKABLE));
    assert!(
        world
            .get::<RouterInputBarrier>(entering)
            .is_some_and(RouterInputBarrier::allows_all)
    );
    assert_eq!(
        world.get::<FrameLifecycle>(entering),
        Some(&FrameLifecycle(EntityLifecycle::Active))
    );
    assert_eq!(
        world.get::<ChildSlotLifecycle>(child_slot),
        Some(&ChildSlotLifecycle::Active)
    );
    assert_eq!(
        world.get::<Visibility>(child_slot),
        Some(&Visibility::Visible)
    );
    assert_eq!(world.get::<Pickable>(child_slot), Some(&Pickable::IGNORE));
    assert!(
        world
            .get::<RouterInputBarrier>(child_slot)
            .is_some_and(RouterInputBarrier::denies_all)
    );
}

#[test]
fn pointer_preparation_keeps_disabled_presentation_rejected() {
    let mut world = World::new();
    let (leaving, entering, _, targets) = presentation_commit_fixture(&mut world);
    crate::input::tests::seed_pointer_history(&mut world, &[leaving]);
    let before = world.resource::<PointerState>().clone();
    world
        .entity_mut(entering)
        .insert(bevy_ecs::entity_disabling::Disabled);
    assert!(
        prepare_presentation_with_pointer_revocation(&mut world, &targets, &[leaving]).is_none()
    );
    crate::input::tests::assert_pointer_target_preserved(&world, leaving, &before);
    assert_eq!(world.get::<Visibility>(entering), Some(&Visibility::Hidden));
}

#[test]
fn commit_split_is_rejected_before_any_mutation() {
    let mut world = World::new();
    let (leaving, entering, child_slot, targets) = presentation_commit_fixture(&mut world);
    let prepared = prepare_presentation_mutation(&mut world, &targets).unwrap();

    assert!(prepared.seal_commit(0).is_none());
    assert_eq!(world.get::<Visibility>(leaving), Some(&Visibility::Visible));
    assert_eq!(world.get::<Visibility>(entering), Some(&Visibility::Hidden));
    assert_eq!(
        world.get::<Visibility>(child_slot),
        Some(&Visibility::Hidden)
    );
}

#[test]
fn reconciliation_nested_default_plugins_reaches_layout_hover_and_target_delivery() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(BevyUiRouterPlugin::default());
    app.world_mut()
        .insert_resource(NestedTargetDeliveries::default());
    install_nested_pointer_observer(&mut app);

    let primary_windows = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(app.world())
        .collect::<Vec<_>>();
    assert_eq!(primary_windows.len(), 1);
    let primary_window = primary_windows[0];
    app.world_mut()
        .get_mut::<Window>(primary_window)
        .unwrap()
        .resolution
        .set(320.0, 180.0);
    app.world_mut().spawn(Camera2d);

    let driver = ViewDriverKey::new("nested-picking-driver").unwrap();
    app.register_ui_view_driver(NestedPickingDriver {
        key: driver.clone(),
    })
    .unwrap();
    let layout = RouteViewKey::new("layout").unwrap();
    let item = RouteViewKey::new("item").unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut views = RouteViewRegistry::new();
    for view in [&layout, &item, &fallback] {
        views
            .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
            .unwrap();
    }
    let router = app
        .add_ui_router(RouterConfig::new(
            bevy_ui_router_core::test_support::nested_items_view_route_tree(),
            views,
            fallback,
        ))
        .unwrap();
    let host = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ))
        .id();

    app.update();
    observe_authority_system(app.world_mut());
    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(router, RouteLocation::new("/items/1"))
        .unwrap();
    collect_navigation_system(app.world_mut());
    drive_nested_until_prepared(&mut app);
    assert_eq!(
        interactive_outputs(app.world(), EntityLifecycle::PreparedInert).len(),
        2
    );
    commit_presentation_system(app.world_mut());
    assert!(
        app.world().resource::<RouterNavigation>().entries[0]
            .cleanup_ledger
            .is_some(),
        "suffix replacement must reserve one leaving cleanup ledger"
    );
    settle_cleanup_system(app.world_mut());
    app.world_mut().run_schedule(PostUpdate);

    let topology = active_nested_topology(app.world(), host);
    assert_eq!(
        app.world()
            .get::<ChildOf>(topology.parent_primitive)
            .map(ChildOf::parent),
        Some(topology.nodes[3])
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(topology.child_primitive)
            .map(ChildOf::parent),
        Some(topology.nodes[7])
    );
    for entity in topology
        .nodes
        .iter()
        .copied()
        .chain([topology.parent_primitive, topology.child_primitive])
    {
        assert!(app.world().get::<Node>(entity).is_some());
        assert!(
            app.world().get::<ComputedNode>(entity).is_some(),
            "nested Node {entity:?} is absent from the real layout pipeline"
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(entity)
                .is_some_and(|visibility| visibility.get()),
            "nested Node {entity:?} is not effectively visible"
        );
    }
    for primitive in [topology.parent_primitive, topology.child_primitive] {
        assert!(
            app.world()
                .get::<ComputedNode>(primitive)
                .is_some_and(|computed| computed.size().cmpgt(Vec2::ZERO).all()),
            "nested primitive {primitive:?} retained zero layout"
        );
    }

    move_mouse_to(&mut app, primary_window, topology.child_primitive);
    let active_before = interactive_outputs(app.world(), EntityLifecycle::Active);
    assert_eq!(active_before.len(), 2);
    let hovered_before = app
        .world()
        .resource::<HoverMap>()
        .get(&PointerId::Mouse)
        .cloned()
        .unwrap_or_default();
    assert!(hovered_before.contains_key(&topology.child_primitive));
    assert!(hovered_before.keys().all(|entity| {
        app.world().get::<FrameOutput>(*entity).is_none() || active_before.contains(entity)
    }));
    assert!(
        app.world()
            .resource::<bevy_ecs::message::Messages<PointerHits>>()
            .iter_current_update_messages()
            .flat_map(|hits| hits.picks.iter().map(|(entity, _)| entity))
            .any(|entity| active_before.contains(entity)),
        "locked UI backend did not produce a router-owned PointerHits candidate"
    );
    assert_eq!(
        app.world()
            .resource::<NestedTargetDeliveries>()
            .0
            .get(&topology.child_primitive),
        Some(&1)
    );

    app.world_mut()
        .resource_mut::<RouterNavigation>()
        .push(
            router,
            RouteLocation::new("/items/2").with_fragment("pending"),
        )
        .unwrap();
    collect_navigation_system(app.world_mut());
    drive_nested_until_prepared(&mut app);
    let pending = interactive_outputs(app.world(), EntityLifecycle::PreparedInert);
    assert_eq!(pending.len(), 1);
    app.world_mut().run_schedule(PostUpdate);
    move_mouse_to(&mut app, primary_window, topology.child_primitive);
    let pending_hover = app
        .world()
        .resource::<HoverMap>()
        .get(&PointerId::Mouse)
        .cloned()
        .unwrap_or_default();
    assert!(pending_hover.keys().all(|entity| !pending.contains(entity)));
    assert!(pending.iter().all(|entity| {
        app.world()
            .resource::<NestedTargetDeliveries>()
            .0
            .get(entity)
            .is_none()
    }));

    let old_deliveries = active_before
        .iter()
        .map(|entity| {
            (
                *entity,
                app.world()
                    .resource::<NestedTargetDeliveries>()
                    .0
                    .get(entity)
                    .copied()
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    commit_presentation_system(app.world_mut());
    assert_eq!(
        interactive_outputs(app.world(), EntityLifecycle::Leaving),
        vec![topology.child_primitive]
    );
    let mut expected_active = vec![topology.parent_primitive];
    expected_active.extend(&pending);
    expected_active.sort_by_key(|entity| entity.to_bits());
    assert_eq!(
        interactive_outputs(app.world(), EntityLifecycle::Active),
        expected_active
    );
    app.world_mut().run_schedule(PostUpdate);
    app.world_mut().run_schedule(PreUpdate);
    let committed_hover = app
        .world()
        .resource::<HoverMap>()
        .get(&PointerId::Mouse)
        .cloned()
        .unwrap_or_default();
    assert!(committed_hover.keys().all(|entity| {
        app.world().get::<FrameOutput>(*entity).is_none() || expected_active.contains(entity)
    }));
    for (entity, deliveries) in &old_deliveries {
        assert_eq!(
            app.world()
                .resource::<NestedTargetDeliveries>()
                .0
                .get(entity)
                .copied()
                .unwrap_or_default(),
            *deliveries,
            "leaving content received a final target event"
        );
    }

    settle_cleanup_system(app.world_mut());
    app.world_mut().run_schedule(PostUpdate);
    let replacement = active_nested_topology(app.world(), host);
    move_mouse_to(&mut app, primary_window, replacement.child_primitive);
    let final_hover = app
        .world()
        .resource::<HoverMap>()
        .get(&PointerId::Mouse)
        .cloned()
        .unwrap_or_default();
    assert!(
        app.world()
            .resource::<crate::input::RouterInputGate>()
            .allows_pointer(replacement.child_primitive),
        "committed replacement must retain its exact pointer capability"
    );
    assert!(final_hover.contains_key(&replacement.child_primitive));
    assert!(final_hover.keys().all(|entity| {
        app.world().get::<FrameOutput>(*entity).is_none() || expected_active.contains(entity)
    }));
    assert_eq!(
        app.world()
            .resource::<NestedTargetDeliveries>()
            .0
            .get(&replacement.child_primitive),
        Some(&1)
    );
    for (entity, deliveries) in old_deliveries {
        assert_eq!(
            app.world()
                .resource::<NestedTargetDeliveries>()
                .0
                .get(&entity)
                .copied()
                .unwrap_or_default(),
            deliveries
        );
        if entity == topology.parent_primitive {
            assert!(app.world().get_entity(entity).is_ok());
        } else {
            assert_eq!(entity, topology.child_primitive);
            assert!(app.world().get_entity(entity).is_err());
        }
    }
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Camera2d>>()
            .iter(app.world())
            .count(),
        1
    );
}

#[test]
fn equal_raw_cross_router_frame_cannot_reparent_into_child_mount() {
    let (_first_world, _, _, first_slot, first_claim, first_task) = pending_frame_fixture();
    let (_second_world, _, _, _, _, second_task) = pending_frame_fixture();
    let parent = first_task.frame().clone();
    let candidate = second_task.frame().clone();
    assert_eq!(parent.router(), candidate.router());
    assert_eq!(parent.host(), candidate.host());
    assert_eq!(parent.slot(), candidate.slot());

    let outlet = RouteSlotKey::default();
    let mount = FrameMountBinding {
        slot: first_slot,
        slot_claim: first_claim.clone(),
        kind: FrameMountBindingKind::ChildOutlet {
            parent: parent.clone(),
            outlet: outlet.clone(),
            anchor: first_slot,
            anchor_binding: OutletBinding::RegisteredOutput(first_claim),
            expected_anchor_children: Vec::new(),
            staging_lifecycle: EntityLifecycle::PreparedInert,
            allowed_existing_shells: Vec::new(),
            retained_parent: None,
        },
    };
    let owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
        parent,
        outlet,
        slot: candidate.slot(),
    });

    assert!(!frame_version_matches_mount(&mount, &owner, &candidate));
}

#[test]
fn invalid_outlet_alias_rebind_is_atomic() {
    let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    let mut content = ContentPlan::new();
    let anchor = FrameContentWriter::new(&mut content).spawn_empty().unwrap();
    FrameContentWriter::new(&mut content)
        .declare_outlet(anchor, RouteSlotKey::default())
        .unwrap();
    let mut frame = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap();
    let current_anchor = frame.outlets[0].anchor_claim().clone();
    let anchor_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(current_anchor.entity())
        .unwrap()
        .manifest()
        .clone();
    let mut foreign_registry = RouteEntityRegistry::new();
    *frame.outlets[0].anchor_binding.source_claim_mut() = foreign_registry
        .register(
            current_anchor.entity(),
            current_anchor.owner().clone(),
            anchor_manifest,
            current_anchor.lifecycle(),
        )
        .unwrap();

    let transitions = frame.lifecycle_transitions(EntityLifecycle::Active);
    let resulting = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec();
    let before = frame.claims().cloned().collect::<Vec<_>>();
    let branch = crate::branch_transaction::RealizedBranch::from_single_frame(frame);

    assert!(
        branch
            .prepare_claim_rebind(
                &resulting,
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
            )
            .is_none()
    );
    assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn missing_outlet_alias_rebind_is_atomic() {
    let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    let mut content = ContentPlan::new();
    let anchor = FrameContentWriter::new(&mut content).spawn_empty().unwrap();
    FrameContentWriter::new(&mut content)
        .declare_outlet(anchor, RouteSlotKey::default())
        .unwrap();
    let mut frame = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap();
    let current_anchor = frame.outlets[0].anchor_claim().clone();
    let foreign_entity = world.spawn_empty().id();
    let anchor_manifest = world
        .resource::<RouteEntityRegistry>()
        .record(current_anchor.entity())
        .unwrap()
        .manifest()
        .clone();
    let mut foreign_registry = RouteEntityRegistry::new();
    *frame.outlets[0].anchor_binding.source_claim_mut() = foreign_registry
        .register(
            foreign_entity,
            current_anchor.owner().clone(),
            anchor_manifest,
            current_anchor.lifecycle(),
        )
        .unwrap();

    let transitions = frame.lifecycle_transitions(EntityLifecycle::Active);
    let resulting = world
        .resource::<RouteEntityRegistry>()
        .prepare_lifecycle_mutation(&transitions)
        .unwrap()
        .resulting_claims()
        .to_vec();
    let before = frame.claims().cloned().collect::<Vec<_>>();
    let branch = crate::branch_transaction::RealizedBranch::from_single_frame(frame);

    assert!(
        branch
            .prepare_claim_rebind(
                &resulting,
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
            )
            .is_none()
    );
    assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn realization_is_fully_inert_before_any_relationship_is_written() {
    let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.insert_resource(RealizationStageAudit::default());
    world.register_component_hooks::<FrameLifecycle>().on_add(
        |mut world, HookContext { entity, .. }| {
            let valid = world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .is_some()
                && world.get::<Node>(entity).is_some()
                && world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
                && world.get::<Pickable>(entity) == Some(&Pickable::IGNORE)
                && world
                    .get::<RouterInputBarrier>(entity)
                    .is_some_and(RouterInputBarrier::denies_all)
                && world.get::<RouteOwnershipSeal>(entity).is_some()
                && world.get::<ChildOf>(entity).is_none();
            world.resource_mut::<RealizationStageAudit>().0.push(valid);
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap();

    assert_eq!(
        world.resource::<RealizationStageAudit>().0,
        vec![true, true, true]
    );
}

#[test]
fn stale_top_slot_claim_cannot_rewrite_the_new_manifest_incarnation() {
    let (mut world, _host, candidate, top_slot, stale_claim, task) = pending_frame_fixture();
    world
        .entity_mut(top_slot)
        .insert(Inherited(ComputedUiTargetCamera::default()));
    let (owner, manifest) = {
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap();
        (record.owner().clone(), record.manifest().clone())
    };
    let new_claim = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        assert!(registry.remove_exact(&stale_claim));
        registry
            .register(
                top_slot,
                owner,
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .unwrap()
    };
    world
        .entity_mut(top_slot)
        .insert(RouteOwnershipSeal::new(new_claim.seal()));
    let entity_count = world.iter_entities().count();

    let failure = realize_frame_deferred(
        &mut world,
        candidate,
        top_slot,
        stale_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .unwrap_err();
    let (error, residual) = failure.into_parts();

    assert_eq!(error, RealizationError::InvalidTopSlot);
    assert!(residual.is_none());
    let record = world
        .resource::<RouteEntityRegistry>()
        .record(top_slot)
        .unwrap();
    assert_eq!(record.claim(), new_claim);
    assert_eq!(record.manifest(), &manifest);
    assert_eq!(world.iter_entities().count(), entity_count);
}

#[test]
fn mount_slot_children_manifest_cas_rejects_registry_drift_after_relationship_write() {
    let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.insert_resource(MountChildrenManifestCasDriftProbe { entity: top_slot });

    let failure = realize_frame_deferred(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .expect_err("post-relationship registry drift must defeat the frozen manifest CAS");
    let (error, residual) = failure.into_parts();

    assert_eq!(error, RealizationError::InvalidMount);
    let residual = residual.expect("written frame entities remain a sealed cleanup residual");
    assert_eq!(
        world.get::<ChildOf>(residual.shell()).map(ChildOf::parent),
        Some(top_slot)
    );
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap()
            .manifest()
            .matches(&world, top_slot)
    );
}

#[test]
fn post_write_foreign_component_is_quarantined_without_despawn() {
    let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.register_component_hooks::<FrameOutput>().on_add(
        |mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .insert(ForeignRealizationComponent);
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    let error = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap_err();

    assert_eq!(error, RealizationError::PostWriteDrift);
    let foreign = world
        .query_filtered::<Entity, bevy_ecs::query::With<ForeignRealizationComponent>>()
        .single(&world)
        .unwrap();
    assert!(world.get_entity(foreign).is_ok());
    let lease = world
        .resource::<QuarantineLeaseTable>()
        .leases()
        .last()
        .expect("post-write drift must create one quarantine lease");
    assert!(lease.owned_entities().contains(&foreign));
    assert!(world.get_entity(host).is_ok());
}

#[test]
pub(crate) fn post_write_required_component_removal_is_rejected_by_the_frozen_manifest() {
    let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.register_component_hooks::<FrameOutput>().on_add(
        |mut world, HookContext { entity, .. }| {
            world.commands().entity(entity).remove::<Pickable>();
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    let error = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap_err();

    assert_eq!(error, RealizationError::PostWriteDrift);
    let lease = world
        .resource::<QuarantineLeaseTable>()
        .leases()
        .last()
        .expect("missing Pickable must be repaired before ordinary quarantine");
    for entity in lease.owned_entities() {
        assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::IGNORE));
        assert!(
            world
                .resource::<QuarantineLeaseTable>()
                .record(entity)
                .unwrap()
                .manifest()
                .matches(&world, entity)
        );
    }
    assert!(world.get_entity(host).is_ok());
}

#[test]
fn persistent_pickable_removal_never_enters_ordinary_quarantine() {
    let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.insert_resource(PersistentPickableRemoval);
    world.register_component_hooks::<FrameOutput>().on_add(
        |mut world, HookContext { entity, .. }| {
            world.commands().entity(entity).remove::<Pickable>();
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    let error = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap_err();

    assert_eq!(error, RealizationError::PostWriteDrift);
    let table = world.resource::<QuarantineLeaseTable>();
    assert!(table.leases().is_empty());
    assert_eq!(table.unresolved_residuals().len(), 1);
    assert_eq!(table.unresolved_residuals()[0].owned_entities().len(), 3);
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .all(|record| matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
            ))
    );
    assert!(world.get_entity(host).is_ok());
}

#[test]
fn pending_frame_focus_covers_exact_mount_slot_and_its_descendants() {
    let (mut world, _host, top_slot, frame) = realized_empty_fixture();
    world.insert_resource(InputFocus::from_entity(top_slot));
    let mut route_focus = RouteFocusState::default();
    route_focus.set_focused(Some(top_slot));
    world.insert_resource(route_focus);

    assert!(frame_focus_is_in_subtree(&world, &frame));
    clear_frame_focus(&mut world, &frame);
    assert_eq!(world.resource::<InputFocus>().get(), None);
    assert_eq!(world.resource::<RouteFocusState>().focused(), None);

    let descendant = world.spawn(ChildOf(top_slot)).id();
    *world.resource_mut::<InputFocus>() = InputFocus::from_entity(descendant);
    world
        .resource_mut::<RouteFocusState>()
        .set_focused(Some(descendant));
    assert!(frame_focus_is_in_subtree(&world, &frame));
    clear_frame_focus(&mut world, &frame);
    assert_eq!(world.resource::<InputFocus>().get(), None);
    assert_eq!(world.resource::<RouteFocusState>().focused(), None);
}

#[test]
fn pending_frame_focus_excludes_a_preexisting_mount_child_subtree() {
    let (mut world, _host, top_slot, mut frame) = realized_empty_fixture();
    let preexisting = world.spawn(ChildOf(top_slot)).id();
    let focused = world.spawn(ChildOf(preexisting)).id();
    frame.prior_mount_children.push(preexisting);
    world.insert_resource(InputFocus::from_entity(focused));
    let mut route_focus = RouteFocusState::default();
    route_focus.set_focused(Some(focused));
    world.insert_resource(route_focus);

    assert!(!frame_focus_is_in_subtree(&world, &frame));
    clear_frame_focus(&mut world, &frame);
    assert_eq!(world.resource::<InputFocus>().get(), Some(focused));
    assert_eq!(world.resource::<RouteFocusState>().focused(), Some(focused));
}

#[test]
fn pending_realization_releases_focus_assigned_by_an_observer() {
    let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.insert_resource(bevy_input_focus::InputFocus::default());
    world.insert_resource(crate::input::RouteFocusState::default());
    world.register_component_hooks::<FrameOutput>().on_add(
        |mut world, HookContext { entity, .. }| {
            *world.resource_mut::<bevy_input_focus::InputFocus>() =
                bevy_input_focus::InputFocus::from_entity(entity);
            world
                .resource_mut::<crate::input::RouteFocusState>()
                .set_focused(Some(entity));
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    let frame = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap();

    assert_eq!(world.resource::<bevy_input_focus::InputFocus>().get(), None);
    assert_eq!(
        world.resource::<crate::input::RouteFocusState>().focused(),
        None
    );
    assert!(validate_realized_frame(&world, &frame));
}

#[test]
fn unresolved_transfer_rejects_a_visible_descendant() {
    let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.insert_resource(PersistentPickableRemoval);
    world.register_component_hooks::<FrameOutput>().on_add(
        |mut world, HookContext { entity, .. }| {
            world
                .commands()
                .entity(entity)
                .remove::<Pickable>()
                .insert(Visibility::Visible);
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    let error = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap_err();

    assert_eq!(error, RealizationError::PostWriteDrift);
    assert!(
        world
            .resource::<QuarantineLeaseTable>()
            .unresolved_residuals()
            .is_empty()
    );
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .any(|record| matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
            ))
    );
}

#[test]
fn post_write_authored_value_drift_is_rejected() {
    let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
    world.register_component_hooks::<FrameOutput>().on_add(
        |mut world, HookContext { entity, .. }| {
            world.commands().entity(entity).insert(Node {
                width: Val::Px(7.0),
                height: Val::Px(9.0),
                ..Node::default()
            });
        },
    );
    let mut content = ContentPlan::new();
    FrameContentWriter::new(&mut content).spawn_empty().unwrap();

    let error = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap_err();

    assert_eq!(error, RealizationError::PostWriteDrift);
    assert_eq!(world.resource::<QuarantineLeaseTable>().leases().len(), 1);
    assert!(world.get_entity(host).is_ok());
}

#[test]
pub(crate) fn realization_is_stable_registered_and_inert() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let router = RouterHandle::fixture(3, 9);
    let host = world
        .spawn((
            Node::default(),
            crate::app_ext::RouterHostRegistration::new(router),
        ))
        .id();
    let mut revisions = HostCandidateRevisionState::default();
    let HostCandidateClassification::Unique(candidate) =
        classify_host_candidate(&world, router, &mut revisions)
    else {
        panic!("host should classify")
    };
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap()
        .build()
        .unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut engine = RouterEngine::install(tree, fallback).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
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
    let request = engine
        .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
        .unwrap();
    engine.collect();
    let bevy_ui_router_core::integration::EngineAction::FrameTask(task) = engine.action().unwrap()
    else {
        panic!("root task expected")
    };
    let mut content = ContentPlan::new();
    let mut writer = FrameContentWriter::new(&mut content);
    let root = writer.spawn_empty().unwrap();
    let rect = writer
        .spawn_solid_rect(SolidRect::new(20, 10, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
        .unwrap();
    writer.add_child(root, rect).unwrap();

    let frame = realize_frame(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        content.accept(),
    )
    .unwrap();

    assert_eq!(request, task.request());
    assert_eq!(frame.obligation_count(), 4);
    assert!(validate_realized_frame(&world, &frame));
    for realized in &frame.entities {
        let mut explicit = vec![
            world.register_component::<Node>(),
            world.register_component::<Visibility>(),
            world.register_component::<Pickable>(),
            world.register_component::<RouterInputBarrier>(),
            world.register_component::<RouteOwnershipSeal>(),
            world.register_component::<FrameLifecycle>(),
            world.register_component::<ChildOf>(),
        ];
        explicit.push(match realized.role {
            RealizedRole::Shell => world.register_component::<FrameShell>(),
            RealizedRole::ContentTarget => world.register_component::<FrameContentTarget>(),
            RealizedRole::Empty | RealizedRole::SolidRect => {
                world.register_component::<FrameOutput>()
            }
        });
        if realized.role == RealizedRole::SolidRect {
            explicit.push(world.register_component::<BackgroundColor>());
            explicit.push(world.register_component::<PickingInteraction>());
        }
        if frame
            .entities
            .iter()
            .any(|child| child.parent == realized.entity)
        {
            explicit.push(world.register_component::<Children>());
        }
        assert_manifest_is_exact(&world, realized.entity, explicit);
    }
    assert_eq!(
        world.get::<ChildOf>(frame.shell()).map(ChildOf::parent),
        Some(top_slot)
    );
    assert_eq!(
        world
            .get::<Children>(top_slot)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default(),
        vec![frame.shell()]
    );
    let expected_container = Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    };
    for realized in &frame.entities {
        assert_eq!(
            world.get::<ChildOf>(realized.entity).map(ChildOf::parent),
            Some(realized.parent)
        );
        assert!(exact_children_match(
            &world,
            realized.entity,
            expected_children(&frame, realized.entity)
        ));
        if matches!(
            realized.role,
            RealizedRole::Shell | RealizedRole::ContentTarget | RealizedRole::Empty
        ) {
            assert_eq!(
                world.get::<Node>(realized.entity),
                Some(&expected_container)
            );
            assert_eq!(
                world.get::<BackgroundColor>(realized.entity),
                Some(&BackgroundColor::default())
            );
            assert_eq!(
                world.get::<Pickable>(realized.entity),
                Some(&Pickable::IGNORE)
            );
            assert!(
                world
                    .get::<RouterInputBarrier>(realized.entity)
                    .is_some_and(RouterInputBarrier::denies_all)
            );
        }
    }
    assert!(world.get_entity(host).is_ok());
}

#[test]
fn realization_registration_failure_settles_every_prior_allocation() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let blocker = world.spawn_empty().id();
    let router = RouterHandle::fixture(5, 11);
    let host = world
        .spawn((
            Node::default(),
            crate::app_ext::RouterHostRegistration::new(router),
        ))
        .id();
    let mut revisions = HostCandidateRevisionState::default();
    let HostCandidateClassification::Unique(candidate) =
        classify_host_candidate(&world, router, &mut revisions)
    else {
        panic!("host should classify")
    };
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap()
        .build()
        .unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut engine = RouterEngine::install(tree, fallback).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
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
    engine
        .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
        .unwrap();
    engine.collect();
    let bevy_ui_router_core::integration::EngineAction::FrameTask(task) = engine.action().unwrap()
    else {
        panic!("root task expected")
    };
    {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        registry
            .register(
                blocker,
                RegistryOwner::Committed(EntityOwner::FrameContentTarget(task.frame().clone())),
                ComponentManifest::new(Vec::new()),
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
    }
    let entity_count_before = world.iter_entities().count();
    let registry_count_before = world.resource::<RouteEntityRegistry>().records().len();

    let failure = realize_frame_deferred(
        &mut world,
        candidate,
        top_slot,
        top_slot_claim,
        task.frame().clone(),
        task.view_request_sequence(),
        ContentPlan::new().accept(),
    )
    .unwrap_err();
    let (error, residual) = failure.into_parts();

    assert_eq!(error, RealizationError::RegistryRejected);
    let residual = residual.expect("prior registered allocations remain a cleanup residual");
    assert_eq!(residual.obligation_count(), 1);
    assert_eq!(world.iter_entities().count(), entity_count_before + 1);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records().len(),
        registry_count_before + 1
    );
    assert!(world.get_entity(residual.shell()).is_ok());
    assert!(world.get::<ChildOf>(residual.shell()).is_none());

    let mut ledger = crate::cleanup::CleanupLedger::for_failed_realization(&world, residual)
        .expect("partial realization has an exact foreground ledger");
    let attempt = ledger.attempt(&world).unwrap();
    assert!(ledger.settle(&mut world, attempt));
    assert_eq!(ledger.receipt_count(), 1);
    assert_eq!(world.iter_entities().count(), entity_count_before);
    assert_eq!(
        world.resource::<RouteEntityRegistry>().records().len(),
        registry_count_before,
        "only the independently registered blocker may remain"
    );
    assert!(
        world
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .all(|record| !matches!(
                record.owner(),
                RegistryOwner::Committed(EntityOwner::FrameShell(_))
            ))
    );
    assert!(world.get_entity(host).is_ok());
}

#[test]
fn failed_settlement_returns_the_complete_frame_for_a_fresh_attempt() {
    let mut world = World::new();
    world.insert_resource(RouteEntityRegistry::new());
    world.insert_resource(QuarantineLeaseTable::new());
    let router = RouterHandle::fixture(7, 13);
    let host = world
        .spawn((
            Node::default(),
            crate::app_ext::RouterHostRegistration::new(router),
        ))
        .id();
    let mut revisions = HostCandidateRevisionState::default();
    let HostCandidateClassification::Unique(candidate) =
        classify_host_candidate(&world, router, &mut revisions)
    else {
        panic!("host should classify")
    };
    let tree = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
        )
        .unwrap()
        .build()
        .unwrap();
    let fallback = RouteViewKey::new("fallback").unwrap();
    let mut engine = RouterEngine::install(tree, fallback).unwrap();
    let token = engine.request_host_admission().unwrap();
    let grant = engine.grant_host_admission(&token).unwrap();
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
    engine
        .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
        .unwrap();
    engine.collect();
    let bevy_ui_router_core::integration::EngineAction::FrameTask(task) = engine.action().unwrap()
    else {
        panic!("root task expected")
    };
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
    let count = frame.obligation_count();
    let first_claim = frame.claims().next().unwrap().clone();
    world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_lifecycle(&first_claim, EntityLifecycle::Quarantined)
        .unwrap();

    let retained = settle_frame(&mut world, frame).unwrap_err();

    assert_eq!(retained.obligation_count(), count);
    assert!(
        retained
            .claims()
            .all(|claim| world.get_entity(claim.entity()).is_ok())
    );
    assert!(world.get_entity(host).is_ok());
}

#[test]
fn task9_settlement_transfers_exact_owned_set_without_physical_despawn() {
    let (mut world, host, _top_slot, frame) = realized_empty_fixture();
    let owned = frame
        .entities
        .iter()
        .map(|realized| realized.entity)
        .collect::<Vec<_>>();
    let leases_before = world.resource::<QuarantineLeaseTable>().leases().len();

    settle_frame(&mut world, frame).unwrap();

    assert!(world.get_entity(host).is_ok());
    assert!(owned.iter().all(|entity| world.get_entity(*entity).is_ok()));
    assert!(owned.iter().all(|entity| {
        world.get::<Visibility>(*entity) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(*entity) == Some(&Pickable::IGNORE)
            && world
                .get::<RouterInputBarrier>(*entity)
                .is_some_and(RouterInputBarrier::denies_all)
            && world
                .resource::<RouteEntityRegistry>()
                .record(*entity)
                .is_none()
    }));
    let leases = world.resource::<QuarantineLeaseTable>().leases();
    assert_eq!(leases.len(), leases_before + 1);
    assert_eq!(leases.last().unwrap().owned_entities().len(), owned.len());
}

#[test]
fn foreign_child_transfers_the_owned_frame_to_quarantine_and_is_never_despawned() {
    let (mut world, host, _top_slot, frame) = realized_empty_fixture();
    let foreign = world.spawn_empty().id();
    world.entity_mut(frame.shell()).add_child(foreign);
    let leases_before = world.resource::<QuarantineLeaseTable>().leases().len();

    assert!(!validate_realized_frame(&world, &frame));
    settle_frame(&mut world, frame).unwrap();

    assert!(world.get_entity(foreign).is_ok());
    assert!(world.get_entity(host).is_ok());
    let leases = world.resource::<QuarantineLeaseTable>().leases();
    assert_eq!(leases.len(), leases_before + 1);
    assert_eq!(leases.last().unwrap().foreign_descendants(), &[foreign]);
}

#[test]
fn failed_realization_with_foreign_descendant_transfers_owned_residual_to_quarantine() {
    let (mut world, host, top_slot, frame) = realized_empty_fixture();
    let owned = frame
        .entities
        .iter()
        .map(|realized| realized.entity)
        .collect::<Vec<_>>();
    let foreign = world.spawn_empty().id();
    world.entity_mut(frame.shell()).add_child(foreign);
    let leases_before = world.resource::<QuarantineLeaseTable>().leases().len();

    settle_failed_realization(&mut world, top_slot, &frame);

    assert!(world.get_entity(host).is_ok());
    assert!(world.get_entity(foreign).is_ok());
    assert!(owned.iter().all(|entity| world.get_entity(*entity).is_ok()));
    assert!(owned.iter().all(|entity| {
        world.get::<Visibility>(*entity) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(*entity) == Some(&Pickable::IGNORE)
            && world
                .get::<RouterInputBarrier>(*entity)
                .is_some_and(RouterInputBarrier::denies_all)
    }));
    let leases = world.resource::<QuarantineLeaseTable>().leases();
    assert_eq!(leases.len(), leases_before + 1);
    assert_eq!(leases.last().unwrap().owned_entities().len(), owned.len());
    assert_eq!(leases.last().unwrap().foreign_descendants(), &[foreign]);
    assert!(owned.iter().all(|entity| {
        world
            .resource::<RouteEntityRegistry>()
            .record(*entity)
            .is_none()
    }));
}

#[test]
fn reparented_top_slot_invalidates_host_authority_without_repairing_app_topology() {
    let (mut world, host, top_slot, frame) = realized_empty_fixture();
    let foreign_parent = world.spawn((Node::default(),)).id();
    world.entity_mut(top_slot).insert(ChildOf(foreign_parent));

    assert!(!validate_realized_frame(&world, &frame));
    assert_eq!(
        world.get::<ChildOf>(top_slot).map(ChildOf::parent),
        Some(foreign_parent)
    );
    assert!(world.get_entity(host).is_ok());
}
