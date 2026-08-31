use bevy_app::{App, Update};
use bevy_ecs::{
    entity::Entity, hierarchy::ChildOf, prelude::Resource, schedule::IntoScheduleConfigs,
    world::World,
};
use bevy_input_focus::InputFocus;
use bevy_ui::{Node, Val};
use bevy_ui_router::ecs::RouterScheduleSet;
use bevy_ui_router::prelude::*;

const MODALITIES: [RouterInputModality; 3] = [
    RouterInputModality::Pointer,
    RouterInputModality::Keyboard,
    RouterInputModality::Gamepad,
];
const FIRST_WIDTH: u16 = 73;
const SECOND_WIDTH: u16 = 79;

struct IsolationDriver {
    key: ViewDriverKey,
    width: u16,
}

impl EcsViewDriver for IsolationDriver {
    fn key(&self) -> ViewDriverKey {
        self.key.clone()
    }

    fn apply(
        &mut self,
        _effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        content
            .spawn_solid_rect(
                SolidRect::new(
                    self.width,
                    23,
                    ContentColor::srgba8(self.width as u8, 31, 47, 255),
                )
                .expect("the public isolation rectangle is non-zero"),
            )
            .expect("the public driver stages one production primitive");
        report.applied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransitionProbeSample {
    delivered: AuditEntityId,
    blocked: AuditEntityId,
    delivered_results: [InputProbeResult; 3],
    blocked_results: [InputProbeResult; 3],
    delivered_counts: [u64; 3],
    blocked_counts: [u64; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BlockedProbeSample {
    target: AuditEntityId,
    counts_before: [u64; 3],
    results: [InputProbeResult; 3],
    counts_after: [u64; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QuarantineProbeSample {
    root: BlockedProbeSample,
    foreign: Option<BlockedProbeSample>,
    lease: QuarantineLeaseId,
    owner_kind: QuarantineOwnerKind,
    entity_count: usize,
    protected_parent: bool,
    inert: bool,
    opened_event_isolated: bool,
}

#[derive(Resource)]
struct IsolationBoundaryState {
    routers: [RouterHandle; 2],
    armed: bool,
    foreign_parent_target: Entity,
    foreign: Option<Entity>,
    protected_target: Entity,
    protected_parent: Entity,
    protected_parent_injected: bool,
    before: Option<[TransitionProbeSample; 2]>,
    after: Option<[TransitionProbeSample; 2]>,
    foreign_quarantine: Option<QuarantineProbeSample>,
    protected_quarantine: Option<QuarantineProbeSample>,
    focus_before_commit: Option<Option<Entity>>,
    focus_after_commit: Option<Option<Entity>>,
    focus_after_cleanup: Option<Option<Entity>>,
}

fn install_router(app: &mut App, label: &str, width: u16) -> RouterHandle {
    let driver = app
        .register_ui_view_driver(IsolationDriver {
            key: ViewDriverKey::new(format!("{label}-driver")).unwrap(),
            width,
        })
        .unwrap();
    let view = RouteViewKey::new(format!("{label}-view")).unwrap();
    let routes = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/items/:id").unwrap(),
            RouteNodeOptions::new().with_view(view.clone()),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut views = RouteViewRegistry::new();
    views
        .insert(view.clone(), RouteViewDescriptor::new(driver))
        .unwrap();
    let router = app
        .add_ui_router(RouterConfig::new(routes, views, view))
        .unwrap();
    app.world_mut()
        .spawn((Node::default(), RouterHostRegistration::new(router)));
    router
}

fn rect_entity(world: &World, width: u16) -> Entity {
    let expected = Val::Px(f32::from(width));
    let matches = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<Node>()
                .is_some_and(|node| node.width == expected)
                .then_some(entity.id())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "the width identifies one active primitive"
    );
    matches[0]
}

fn content_for_lifecycle(
    audit: &RouterAuthorityAuditSnapshot,
    lifecycle: AuthorityFrameLifecycle,
) -> AuditEntityId {
    let frame = audit
        .frames()
        .iter()
        .find(|frame| frame.lifecycle() == lifecycle)
        .unwrap_or_else(|| {
            panic!(
                "the boundary exposes {lifecycle:?}; observed {:?}",
                audit
                    .frames()
                    .iter()
                    .map(|frame| frame.lifecycle())
                    .collect::<Vec<_>>()
            )
        })
        .frame();
    audit
        .content()
        .iter()
        .find(|content| content.frame() == frame)
        .expect("the public driver produced one content mapping")
        .entity()
}

fn transition_targets(
    world: &World,
    routers: [RouterHandle; 2],
    delivered: AuthorityFrameLifecycle,
    blocked: AuthorityFrameLifecycle,
) -> [(AuditEntityId, AuditEntityId); 2] {
    routers.map(|router| {
        let audit = world
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .expect("the router has a real-time authority audit");
        (
            content_for_lifecycle(audit, delivered),
            content_for_lifecycle(audit, blocked),
        )
    })
}

fn sample_transition(
    probes: &mut RouterInputProbes,
    router: RouterHandle,
    delivered: AuditEntityId,
    blocked: AuditEntityId,
) -> TransitionProbeSample {
    let delivered_results = MODALITIES.map(|modality| probes.dispatch(router, delivered, modality));
    let blocked_results = MODALITIES.map(|modality| probes.dispatch(router, blocked, modality));
    TransitionProbeSample {
        delivered,
        blocked,
        delivered_results,
        blocked_results,
        delivered_counts: MODALITIES.map(|modality| probes.delivery_count(delivered, modality)),
        blocked_counts: MODALITIES.map(|modality| probes.delivery_count(blocked, modality)),
    }
}

fn sample_blocked_target(
    probes: &mut RouterInputProbes,
    router: RouterHandle,
    target: AuditEntityId,
) -> BlockedProbeSample {
    let counts_before = MODALITIES.map(|modality| probes.delivery_count(target, modality));
    let results = MODALITIES.map(|modality| probes.dispatch(router, target, modality));
    let counts_after = MODALITIES.map(|modality| probes.delivery_count(target, modality));
    BlockedProbeSample {
        target,
        counts_before,
        results,
        counts_after,
    }
}

fn capture_before_commit(world: &mut World) {
    let (routers, armed, already_captured) = {
        let state = world.resource::<IsolationBoundaryState>();
        (state.routers, state.armed, state.before.is_some())
    };
    if !armed || already_captured {
        return;
    }
    let targets = transition_targets(
        world,
        routers,
        AuthorityFrameLifecycle::Active,
        AuthorityFrameLifecycle::PreparedInert,
    );
    let samples = world.resource_scope(
        |_world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
            [
                sample_transition(&mut probes, routers[0], targets[0].0, targets[0].1),
                sample_transition(&mut probes, routers[1], targets[1].0, targets[1].1),
            ]
        },
    );
    let focus = world.resource::<InputFocus>().get();
    let mut state = world.resource_mut::<IsolationBoundaryState>();
    state.before = Some(samples);
    state.focus_before_commit = Some(focus);
}

fn capture_after_commit_and_inject_protected_parent(world: &mut World) {
    let (routers, armed, already_captured, foreign_parent, target, parent) = {
        let state = world.resource::<IsolationBoundaryState>();
        (
            state.routers,
            state.armed,
            state.after.is_some(),
            state.foreign_parent_target,
            state.protected_target,
            state.protected_parent,
        )
    };
    if !armed || already_captured {
        return;
    }
    let targets = transition_targets(
        world,
        routers,
        AuthorityFrameLifecycle::Active,
        AuthorityFrameLifecycle::LeavingInert,
    );
    let samples = world.resource_scope(
        |_world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
            [
                sample_transition(&mut probes, routers[0], targets[0].0, targets[0].1),
                sample_transition(&mut probes, routers[1], targets[1].0, targets[1].1),
            ]
        },
    );
    let focus = world.resource::<InputFocus>().get();
    let foreign = world.spawn((Node::default(), ChildOf(foreign_parent))).id();
    world.entity_mut(target).insert(ChildOf(parent));
    world.flush();
    let mut state = world.resource_mut::<IsolationBoundaryState>();
    state.after = Some(samples);
    state.focus_after_commit = Some(focus);
    state.foreign = Some(foreign);
    state.protected_parent_injected = true;
}

fn quarantine_opened_is_isolated(
    audit: &RouterAuthorityAuditSnapshot,
    target: AuditEntityId,
) -> bool {
    audit.events().iter().rev().any(|event| {
        event.kind() == RouterLifecycleAuditEventKind::QuarantineOpened
            && event.entity() == Some(target)
            && event.visibility_hidden()
            && !event.accepts_pointer()
            && !event.accepts_keyboard()
            && !event.accepts_gamepad()
            && !event.focused()
    })
}

fn capture_after_reaper(world: &mut World) {
    let (routers, armed, already_captured) = {
        let state = world.resource::<IsolationBoundaryState>();
        (
            state.routers,
            state.armed,
            state.foreign_quarantine.is_some() && state.protected_quarantine.is_some(),
        )
    };
    if !armed || already_captured {
        return;
    }
    let Some((foreign, protected)) = (|| {
        let audits = world.resource::<RouterAuthorityAudits>();
        let foreign_audit = audits.get(routers[0])?;
        let protected_audit = audits.get(routers[1])?;
        let foreign = foreign_audit
            .quarantines()
            .iter()
            .find(|snapshot| !snapshot.foreign_targets().is_empty())?;
        let protected = protected_audit
            .quarantines()
            .iter()
            .find(|snapshot| snapshot.protected_parent())?;
        Some((
            (
                foreign.root(),
                foreign.foreign_targets()[0],
                foreign.root_owner().lease_id(),
                foreign.root_owner().kind(),
                foreign.root_owner().entity_count(),
                foreign.protected_parent(),
                foreign.inert(),
                quarantine_opened_is_isolated(foreign_audit, foreign.root()),
            ),
            (
                protected.root(),
                protected.root_owner().lease_id(),
                protected.root_owner().kind(),
                protected.root_owner().entity_count(),
                protected.protected_parent(),
                protected.inert(),
                quarantine_opened_is_isolated(protected_audit, protected.root()),
            ),
        ))
    })() else {
        return;
    };

    let (foreign_sample, protected_sample) = world.resource_scope(
        |_world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
            (
                QuarantineProbeSample {
                    root: sample_blocked_target(&mut probes, routers[0], foreign.0),
                    foreign: Some(sample_blocked_target(&mut probes, routers[0], foreign.1)),
                    lease: foreign.2,
                    owner_kind: foreign.3,
                    entity_count: foreign.4,
                    protected_parent: foreign.5,
                    inert: foreign.6,
                    opened_event_isolated: foreign.7,
                },
                QuarantineProbeSample {
                    root: sample_blocked_target(&mut probes, routers[1], protected.0),
                    foreign: None,
                    lease: protected.1,
                    owner_kind: protected.2,
                    entity_count: protected.3,
                    protected_parent: protected.4,
                    inert: protected.5,
                    opened_event_isolated: protected.6,
                },
            )
        },
    );
    let focus = world.resource::<InputFocus>().get();
    let mut state = world.resource_mut::<IsolationBoundaryState>();
    state.foreign_quarantine = Some(foreign_sample);
    state.protected_quarantine = Some(protected_sample);
    state.focus_after_cleanup = Some(focus);
}

fn assert_transition_samples(before: TransitionProbeSample, after: TransitionProbeSample) {
    assert_eq!(before.delivered_results, [InputProbeResult::Delivered; 3]);
    assert_eq!(before.blocked_results, [InputProbeResult::Blocked; 3]);
    assert_eq!(before.delivered_counts, [1; 3]);
    assert_eq!(before.blocked_counts, [0; 3]);
    assert_eq!(after.delivered_results, [InputProbeResult::Delivered; 3]);
    assert_eq!(after.blocked_results, [InputProbeResult::Blocked; 3]);
    assert_eq!(after.delivered_counts, [1; 3]);
    assert_eq!(after.blocked_counts, [1; 3]);
    assert_eq!(before.delivered, after.blocked);
    assert_eq!(before.blocked, after.delivered);
}

fn assert_quarantine_sample(sample: &QuarantineProbeSample) {
    assert_eq!(sample.root.results, [InputProbeResult::Blocked; 3]);
    assert_eq!(sample.root.counts_after, sample.root.counts_before);
    if let Some(foreign) = sample.foreign {
        assert_eq!(foreign.results, [InputProbeResult::Blocked; 3]);
        assert_eq!(foreign.counts_after, foreign.counts_before);
    }
    assert!(sample.entity_count > 0);
    assert!(sample.inert);
    assert!(sample.opened_event_isolated);
}

fn assert_persistently_blocked(previous: BlockedProbeSample, refreshed: BlockedProbeSample) {
    assert_eq!(refreshed.target, previous.target);
    assert_eq!(refreshed.counts_before, previous.counts_after);
    assert_eq!(refreshed.results, [InputProbeResult::Blocked; 3]);
    assert_eq!(refreshed.counts_after, refreshed.counts_before);
}

#[test]
fn public_navigation_input_isolation_is_atomic_and_quarantines_remain_blocked() {
    let mut app = App::new();
    app.add_plugins(BevyUiRouterPlugin::default());
    let routers = [
        install_router(&mut app, "foreign-quarantine", FIRST_WIDTH),
        install_router(&mut app, "protected-quarantine", SECOND_WIDTH),
    ];
    app.update();

    for router in routers {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
    }
    app.update();

    let first_active = rect_entity(app.world(), FIRST_WIDTH);
    let second_active = rect_entity(app.world(), SECOND_WIDTH);
    let protected_parent = app.world_mut().spawn(Node::default()).id();
    *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(first_active);

    app.insert_resource(IsolationBoundaryState {
        routers,
        armed: true,
        foreign_parent_target: first_active,
        foreign: None,
        protected_target: second_active,
        protected_parent,
        protected_parent_injected: false,
        before: None,
        after: None,
        foreign_quarantine: None,
        protected_quarantine: None,
        focus_before_commit: None,
        focus_after_commit: None,
        focus_after_cleanup: None,
    });
    app.add_systems(
        Update,
        capture_before_commit
            .after(RouterScheduleSet::DriveNavigation)
            .before(RouterScheduleSet::CommitPresentation),
    );
    app.add_systems(
        Update,
        capture_after_commit_and_inject_protected_parent
            .after(RouterScheduleSet::CommitPresentation)
            .before(RouterScheduleSet::SettleCleanup),
    );
    app.add_systems(
        Update,
        capture_after_reaper
            .after(RouterScheduleSet::ReapQuarantines)
            .before(RouterScheduleSet::PublishSnapshots),
    );

    for router in routers {
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/items/2"))
            .unwrap();
    }
    for _ in 0..8 {
        app.update();
        let state = app.world().resource::<IsolationBoundaryState>();
        if state.foreign_quarantine.is_some() && state.protected_quarantine.is_some() {
            break;
        }
    }

    let state = app.world().resource::<IsolationBoundaryState>();
    let before = state
        .before
        .expect("system A sampled the pre-commit boundary");
    let after = state
        .after
        .expect("system B sampled the post-commit boundary");
    assert_transition_samples(before[0], after[0]);
    assert_transition_samples(before[1], after[1]);
    assert_eq!(state.focus_before_commit, Some(Some(first_active)));
    assert_eq!(state.focus_after_commit, Some(None));
    assert_eq!(state.focus_after_cleanup, Some(None));
    assert!(state.protected_parent_injected);

    let foreign_quarantine = state
        .foreign_quarantine
        .as_ref()
        .expect("system C observed the foreign-descendant quarantine");
    let protected_quarantine = state
        .protected_quarantine
        .as_ref()
        .expect("system C observed the protected-parent quarantine");
    assert_quarantine_sample(foreign_quarantine);
    assert_quarantine_sample(protected_quarantine);
    assert!(!foreign_quarantine.protected_parent);
    assert!(foreign_quarantine.foreign.is_some());
    assert!(protected_quarantine.protected_parent);
    assert!(protected_quarantine.foreign.is_none());
    assert_ne!(foreign_quarantine.lease, protected_quarantine.lease);
    let foreign = state
        .foreign
        .expect("system B injected the app-owned foreign descendant");
    assert!(app.world().get_entity(foreign).is_ok());
    assert_eq!(
        app.world().get::<ChildOf>(foreign).map(ChildOf::parent),
        Some(first_active)
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(second_active)
            .map(ChildOf::parent),
        Some(protected_parent)
    );

    let foreign_quarantine = foreign_quarantine.clone();
    let protected_quarantine = protected_quarantine.clone();
    app.update();
    let (refreshed_foreign, refreshed_protected) = app.world_mut().resource_scope(
        |_world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
            (
                (
                    sample_blocked_target(&mut probes, routers[0], foreign_quarantine.root.target),
                    foreign_quarantine.foreign.map(|foreign| {
                        sample_blocked_target(&mut probes, routers[0], foreign.target)
                    }),
                ),
                sample_blocked_target(&mut probes, routers[1], protected_quarantine.root.target),
            )
        },
    );
    assert_persistently_blocked(foreign_quarantine.root, refreshed_foreign.0);
    assert_persistently_blocked(
        foreign_quarantine
            .foreign
            .expect("the foreign quarantine retains its observed descendant"),
        refreshed_foreign
            .1
            .expect("the refreshed probes retain the observed descendant"),
    );
    assert_persistently_blocked(protected_quarantine.root, refreshed_protected);
}
