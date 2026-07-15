use std::sync::atomic::{AtomicU64, Ordering};

use bevy_ecs::{prelude::World, resource::Resource};
use bevy_input_focus::InputFocus;
use bevy_ui_router_core::{
    integration::{
        CleanupManifest, CommitTicket, EngineAction, IntegrationError, RouterEngine,
        ViewFailureKind,
    },
    prelude::{NavigationRequestId, RouteLocation},
};
use bevy_ui_router_view::{RouteViewRegistry, integration as view_integration};

use crate::{
    app_ext::{RouterHandle, ViewDriverStore},
    cleanup::{CleanupLedger, cleanup_batch_matches},
    content::{ContentPlan, FrameContentWriter},
    driver::{EcsViewReportSession, ReportSeal, ViewBuildReportKind},
    host::{
        HostCandidateClassification, HostCandidateRevisionLatch, HostCandidateRevisionState,
        ValidatedHostCandidate, activate_granted_top_slot, classify_host_candidate,
        settle_rejected_granted_top_slot, stage_granted_top_slot,
        validated_host_candidate_is_current,
    },
    input::{
        RouteFocusState, RouterInputGate, prepare_focus_mutation, prepare_input_gate_mutation,
    },
    presentation::{
        RealizedFrame, prepare_presentation_mutation, realize_frame, settle_frame,
        validate_active_frame, validate_realized_frame,
    },
    registry::{EntityLifecycle, RouteEntityRegistry},
    snapshots::RouterSnapshots,
};

pub(crate) struct HostBinding {
    pub(crate) version: bevy_ui_router_core::HostVersion,
    pub(crate) slot_id: bevy_ui_router_core::ids::RouteSlotId,
    pub(crate) host: bevy_ecs::entity::Entity,
    pub(crate) top_slot: bevy_ecs::entity::Entity,
    pub(crate) top_slot_claim: crate::registry::ExactRegistryClaim,
    pub(crate) candidate: ValidatedHostCandidate,
}

struct PendingCommit {
    ticket: Option<CommitTicket>,
    realized: Option<RealizedFrame>,
}

pub(crate) struct RouterEntry {
    pub(crate) handle: RouterHandle,
    pub(crate) engine: RouterEngine,
    views: RouteViewRegistry,
    pub(crate) host: Option<HostBinding>,
    pending_commit: Option<PendingCommit>,
    pub(crate) active: Option<RealizedFrame>,
    cleanup_ledger: Option<CleanupLedger>,
    pub(crate) requests: Vec<NavigationRequestId>,
}

impl RouterEntry {
    #[cfg(feature = "test-support")]
    pub(crate) fn pending_frame(&self) -> Option<&RealizedFrame> {
        self.pending_commit
            .as_ref()
            .and_then(|pending| pending.realized.as_ref())
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn leaving_frame(&self) -> Option<&RealizedFrame> {
        self.cleanup_ledger
            .as_ref()
            .and_then(CleanupLedger::leaving)
    }
}

#[derive(Resource)]
pub struct RouterNavigation {
    domain: u64,
    next_router: u64,
    pub(crate) entries: Vec<RouterEntry>,
}

impl RouterNavigation {
    pub(crate) fn new() -> Self {
        static NEXT_DOMAIN: AtomicU64 = AtomicU64::new(1);

        let domain = NEXT_DOMAIN.fetch_add(1, Ordering::Relaxed);
        assert_ne!(domain, 0, "router navigation domain exhausted");
        Self {
            domain,
            next_router: 1,
            entries: Vec::new(),
        }
    }

    pub(crate) fn install(
        &mut self,
        routes: bevy_ui_router_core::prelude::RouteTree,
        views: RouteViewRegistry,
    ) -> Result<RouterHandle, IntegrationError> {
        let handle = RouterHandle::from_parts(self.domain, self.next_router);
        self.next_router = self
            .next_router
            .checked_add(1)
            .ok_or(IntegrationError::Exhausted)?;
        let engine = RouterEngine::install(routes)?;
        self.entries.push(RouterEntry {
            handle,
            engine,
            views,
            host: None,
            pending_commit: None,
            active: None,
            cleanup_ledger: None,
            requests: Vec::new(),
        });
        Ok(handle)
    }

    pub fn push(
        &mut self,
        router: RouterHandle,
        location: RouteLocation,
    ) -> Result<NavigationRequestId, NavigationEnqueueError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.handle == router)
            .ok_or_else(|| {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::UnknownRouter)
            })?;
        let request = entry.engine.push(location).map_err(|error| match error {
            IntegrationError::AwaitingHost => {
                NavigationEnqueueError::new(NavigationEnqueueErrorKind::HostUnavailable)
            }
            _ => NavigationEnqueueError::new(NavigationEnqueueErrorKind::AuthorityRecovering),
        })?;
        entry.requests.push(request);
        Ok(request)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationEnqueueErrorKind {
    UnknownRouter,
    HostUnavailable,
    AuthorityRecovering,
    NoBackEntry,
}

#[derive(Debug)]
pub struct NavigationEnqueueError {
    kind: NavigationEnqueueErrorKind,
}

impl NavigationEnqueueError {
    const fn new(kind: NavigationEnqueueErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> NavigationEnqueueErrorKind {
        self.kind
    }
}

impl std::fmt::Display for NavigationEnqueueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            NavigationEnqueueErrorKind::UnknownRouter => "router is not installed",
            NavigationEnqueueErrorKind::HostUnavailable => "router host is unavailable",
            NavigationEnqueueErrorKind::AuthorityRecovering => "router authority is recovering",
            NavigationEnqueueErrorKind::NoBackEntry => "router history has no back entry",
        })
    }
}

impl std::error::Error for NavigationEnqueueError {}

pub(crate) fn observe_authority_system(world: &mut World) {
    world.resource_scope(|world, mut revisions: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
        world.resource_scope(|world, mut latch: bevy_ecs::change_detection::Mut<HostCandidateRevisionLatch>| {
            world.resource_scope(|world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                for entry in &mut navigation.entries {
                    if entry.host.is_some() {
                        continue;
                    }
                    let HostCandidateClassification::Unique(candidate) =
                        classify_host_candidate(world, entry.handle, &mut revisions)
                    else {
                        continue;
                    };
                    let revision = candidate.revision();
                    if !latch.allows(revision) {
                        continue;
                    }
                    let host = candidate.host();
                    let Ok(token) = entry.engine.request_host_admission() else {
                        continue;
                    };
                    let Ok(grant) = entry.engine.grant_host_admission(&token) else {
                        let _ = entry.engine.reject_host_admission(token);
                        continue;
                    };
                    let version = grant.version().clone();
                    let slot_id = grant.top_slot();
                    match stage_granted_top_slot(world, candidate.clone(), &grant) {
                        Ok(staged) => {
                            let top_slot = staged.entity();
                            if activate_granted_top_slot(world, &candidate, &grant, staged) {
                                let top_slot_claim = world
                                    .resource::<RouteEntityRegistry>()
                                    .record(top_slot)
                                    .expect("activated top slot remains registered")
                                    .claim();
                                entry.engine.confirm_host(grant);
                                entry.host = Some(HostBinding {
                                    version,
                                    slot_id,
                                    host,
                                    top_slot,
                                    top_slot_claim,
                                    candidate,
                                });
                            } else {
                                let _ = settle_rejected_granted_top_slot(
                                    world,
                                    &candidate,
                                    staged,
                                );
                                let _ = entry.engine.reject_host_after_mutation(grant);
                                latch.reject(revision);
                            }
                        }
                        Err(_) => {
                            let _ = entry.engine.reject_host_after_mutation(grant);
                            latch.reject(revision);
                        }
                    }
                }
            });
        });
    });
    refresh_authority_audits(world);
}

pub(crate) fn collect_navigation_system(world: &mut World) {
    for entry in &mut world.resource_mut::<RouterNavigation>().entries {
        entry.engine.collect();
    }
}

pub(crate) fn drive_navigation_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |world, mut drivers: bevy_ecs::change_detection::Mut<ViewDriverStore>| {
                    for entry in &mut navigation.entries {
                        if entry.pending_commit.is_some() {
                            continue;
                        }
                        let Some(action) = entry.engine.action() else {
                            continue;
                        };
                        match action {
                            EngineAction::Commit(ticket) => {
                                entry.pending_commit = Some(PendingCommit {
                                    ticket: Some(ticket),
                                    realized: None,
                                });
                            }
                            EngineAction::FrameTask(mut task) => {
                                let Some(host) = entry.host.as_ref() else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "router host unavailable during frame apply",
                                    );
                                    continue;
                                };
                                if !validated_host_candidate_is_current(world, &host.candidate) {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "router host authority changed before frame realization",
                                    );
                                    continue;
                                }
                                let Some(driver_key) =
                                    view_integration::driver_key(&entry.views, task.view())
                                        .cloned()
                                else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Build,
                                        "route view binding unavailable",
                                    );
                                    continue;
                                };
                                let Some(driver) = drivers.get_mut(&driver_key) else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Build,
                                        "route view driver unavailable",
                                    );
                                    continue;
                                };
                                let effect = view_integration::build_effect(
                                    task.view().clone(),
                                    task.take_input(),
                                    task.view_request_sequence(),
                                );
                                let mut plan = ContentPlan::new();
                                let mut writer = FrameContentWriter::new(&mut plan);
                                let mut seal = ReportSeal::new();
                                let report = driver.apply(
                                    &effect,
                                    &mut writer,
                                    EcsViewReportSession::new(&mut seal),
                                );
                                if report.kind() != ViewBuildReportKind::Applied {
                                    let failure = match report.kind() {
                                        ViewBuildReportKind::BuildFailed => ViewFailureKind::Build,
                                        ViewBuildReportKind::ApplyFailed => ViewFailureKind::Apply,
                                        ViewBuildReportKind::Applied => unreachable!(),
                                    };
                                    let _ = entry.engine.report_failed(
                                        task,
                                        failure,
                                        "route view driver rejected the frame",
                                    );
                                    continue;
                                }
                                let Ok(realized) = realize_frame(
                                    world,
                                    host.candidate.clone(),
                                    host.top_slot,
                                    host.top_slot_claim.clone(),
                                    task.frame().clone(),
                                    task.view_request_sequence(),
                                    plan.accept(),
                                ) else {
                                    let _ = entry.engine.report_failed(
                                        task,
                                        ViewFailureKind::Apply,
                                        "frame realization rejected",
                                    );
                                    continue;
                                };
                                let Ok(ticket) = entry.engine.report_applied(task) else {
                                    let _ = settle_frame(world, realized);
                                    continue;
                                };
                                entry.pending_commit = Some(PendingCommit {
                                    ticket: Some(ticket),
                                    realized: Some(realized),
                                });
                            }
                        }
                    }
                },
            );
        },
    );
    refresh_authority_audits(world);
}

pub(crate) fn commit_presentation_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
                let Some(mut pending) = entry.pending_commit.take() else {
                    continue;
                };
                let Some(host) = entry.host.as_ref() else {
                    entry.pending_commit = Some(pending);
                    continue;
                };
                if world.get_entity(host.host).is_err()
                    || !validated_host_candidate_is_current(world, &host.candidate)
                    || pending
                        .realized
                        .as_ref()
                        .is_some_and(|frame| !validate_realized_frame(world, frame))
                    || entry
                        .active
                        .as_ref()
                        .is_some_and(|frame| !validate_active_frame(world, frame))
                {
                    entry.pending_commit = Some(pending);
                    continue;
                }

                let mut entering = pending.realized.take();
                let mut leaving = if entering.is_some() {
                    entry.active.take()
                } else {
                    None
                };
                let obligation_count = leaving.as_ref().map_or(0, RealizedFrame::obligation_count);
                let completion_digest = leaving.as_ref().map_or(0, frame_digest);
                let logical_roots = leaving
                    .as_ref()
                    .map(|frame| vec![frame.frame().clone()])
                    .unwrap_or_default();
                let gate_before = leaving
                    .as_ref()
                    .or(entry.active.as_ref())
                    .map(RealizedFrame::interactive_entities)
                    .unwrap_or_default();
                let gate_after = entering
                    .as_ref()
                    .or(entry.active.as_ref())
                    .map(RealizedFrame::interactive_entities)
                    .unwrap_or_default();
                let focus_leaving = leaving
                    .as_ref()
                    .map(|frame| {
                        frame
                            .claims()
                            .map(|claim| claim.entity())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let manifest =
                    CleanupManifest::new(logical_roots, completion_digest, obligation_count);
                let mut targets = Vec::new();
                let mut transitions = Vec::new();
                if let Some(frame) = leaving.as_ref() {
                    targets.extend(frame.mutation_targets(
                        EntityLifecycle::Active,
                        EntityLifecycle::Leaving,
                        false,
                    ));
                    transitions.extend(frame.lifecycle_transitions(EntityLifecycle::Leaving));
                }
                let leaving_count = transitions.len();
                if let Some(frame) = entering.as_ref() {
                    targets.extend(frame.mutation_targets(
                        EntityLifecycle::PreparedInert,
                        EntityLifecycle::Active,
                        true,
                    ));
                    transitions.extend(frame.lifecycle_transitions(EntityLifecycle::Active));
                }
                let committed =
                    world.resource_scope(
                        |world, gate: bevy_ecs::change_detection::Mut<RouterInputGate>| {
                            world.resource_scope(
                        |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
                            world.resource_scope(
                                |world,
                                 route_focus: bevy_ecs::change_detection::Mut<RouteFocusState>| {
                                    world.resource_scope(
                                        |world,
                                         mut registry: bevy_ecs::change_detection::Mut<
                                            RouteEntityRegistry,
                                        >| {
                                            let registry_plan = registry
                                                .prepare_lifecycle_mutation(&transitions)?;
                                            let resulting_claims =
                                                registry_plan.resulting_claims().to_vec();
                                            let gate_plan = prepare_input_gate_mutation(
                                                gate,
                                                &gate_before,
                                                gate_after,
                                            )?;
                                            let focus_plan = prepare_focus_mutation(
                                                input_focus,
                                                route_focus,
                                                &focus_leaving,
                                            );
                                            let presentation_plan =
                                                prepare_presentation_mutation(world, &targets)?;
                                            let ticket = pending.ticket.as_ref()?;
                                            let prepared = entry
                                                .engine
                                                .prepare_commit(ticket, manifest)
                                                .ok()?;
                                            if !cleanup_batch_matches(
                                                prepared.cleanup(),
                                                completion_digest,
                                                obligation_count,
                                            ) {
                                                return None;
                                            }
                                            pending.ticket.take();
                                            let cleanup = prepared.cleanup().clone();
                                            entry.engine.publish(prepared, || {
                                                presentation_plan.apply();
                                                registry_plan.apply(&mut registry);
                                                gate_plan.apply();
                                                focus_plan.apply();
                                            });
                                            Some((cleanup, resulting_claims))
                                        },
                                    )
                                },
                            )
                        },
                    )
                        },
                    );
                let Some((cleanup, resulting_claims)) = committed else {
                    if let Some(frame) = leaving {
                        entry.active = Some(frame);
                    }
                    pending.realized = entering;
                    if pending.ticket.is_some() {
                        entry.pending_commit = Some(pending);
                    }
                    continue;
                };
                if let Some(frame) = leaving.as_mut() {
                    frame.replace_claims(&resulting_claims[..leaving_count]);
                }
                if let Some(frame) = entering.as_mut() {
                    frame.replace_claims(&resulting_claims[leaving_count..]);
                }
                if let Some(frame) = entering {
                    entry.active = Some(frame);
                }
                entry.cleanup_ledger =
                    Some(CleanupLedger::new(cleanup, leaving, completion_digest));
            }
        },
    );
    refresh_authority_audits(world);
}

pub(crate) fn settle_cleanup_system(world: &mut World) {
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            for entry in &mut navigation.entries {
                let Some(mut ledger) = entry.cleanup_ledger.take() else {
                    continue;
                };
                let completion = if let Some(completion) = ledger.pending_completion() {
                    completion
                } else {
                    let Some(attempt) = ledger.attempt(world) else {
                        entry.cleanup_ledger = Some(ledger);
                        continue;
                    };
                    if !ledger.settle(world, attempt) {
                        entry.cleanup_ledger = Some(ledger);
                        continue;
                    }
                    ledger
                        .pending_completion()
                        .expect("successful settlement has a completion")
                };
                if entry.engine.complete_cleanup(completion).is_err() {
                    entry.cleanup_ledger = Some(ledger);
                }
            }
        },
    );
    refresh_authority_audits(world);
}

pub(crate) fn reap_quarantines_system(_world: &mut World) {}

pub(crate) fn publish_snapshots_system(world: &mut World) {
    world.resource_scope(
        |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |_world, mut snapshots: bevy_ecs::change_detection::Mut<RouterSnapshots>| {
                    snapshots.publish(&navigation.entries);
                },
            );
        },
    );
    refresh_authority_audits(world);
}

#[cfg(feature = "test-support")]
fn refresh_authority_audits(world: &mut World) {
    world.resource_scope(
        |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            world.resource_scope(
                |world,
                 mut audits: bevy_ecs::change_detection::Mut<
                    crate::audit::RouterAuthorityAudits,
                >| {
                    audits.refresh(world, &navigation.entries);
                },
            );
        },
    );
}

#[cfg(not(feature = "test-support"))]
fn refresh_authority_audits(_world: &mut World) {}

fn frame_digest(frame: &RealizedFrame) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for claim in frame.claims() {
        for byte in claim.entity().to_bits().to_le_bytes() {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        components::{RouteOwnershipSeal, RouterInputBarrier},
        content::{ContentColor, SolidRect},
        driver::{EcsViewDriver, ViewBuildReport},
        input::{RouteFocusState, RouterInputGate},
        plugin::BevyUiRouterPlugin,
        registry::{
            ComponentManifest, EntityOwner, ProvisionalTopSlotOwner, QuarantineLeaseTable,
            RegistryOwner,
        },
    };
    use bevy_app::{App, PostUpdate, PreUpdate};
    use bevy_camera::visibility::{InheritedVisibility, Visibility};
    use bevy_ecs::{entity::Entity, lifecycle::HookContext, prelude::Resource, query::With};
    use bevy_input_focus::InputFocus;
    use bevy_internal::{
        DefaultPlugins,
        camera::{Camera2d, NormalizedRenderTarget},
        math::Vec2,
        picking::{
            hover::HoverMap,
            pointer::{Location, PointerId, PointerLocation},
        },
        window::{PrimaryWindow, Window, WindowRef},
    };
    use bevy_picking::Pickable;
    use bevy_ui::{ComputedNode, Node, UiGlobalTransform, Val};
    use bevy_ui_router_core::prelude::{
        NavigationFailureKind, RequestStatus, RouteNodeOptions, RoutePattern, RouteTree,
        RouteViewKey,
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    struct AtomicDriver {
        key: ViewDriverKey,
    }

    impl EcsViewDriver for AtomicDriver {
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
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            report.applied()
        }
    }

    struct PanicDriver {
        key: ViewDriverKey,
    }

    impl EcsViewDriver for PanicDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            _content: &mut FrameContentWriter<'_>,
            _report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            panic!("stale host authority must be rejected before the driver runs")
        }
    }

    struct FailOnceDriver {
        key: ViewDriverKey,
        failed: bool,
    }

    #[derive(Resource)]
    struct PersistentOnePickableRemoval {
        remaining_before_target: usize,
        target: Option<Entity>,
    }

    impl EcsViewDriver for FailOnceDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            if !self.failed {
                self.failed = true;
                return report.build_failed("expected first-attempt failure");
            }
            content
                .spawn_solid_rect(
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            report.applied()
        }
    }

    fn install_atomic_router(app: &mut App, label: &str) -> RouterHandle {
        let driver = ViewDriverKey::new(format!("{label}-driver")).unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new(format!("{label}-view")).unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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

    #[test]
    fn enqueue_error_has_the_frozen_kinds_and_error_traits() {
        fn assert_error_traits<
            T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
        >() {
        }
        assert_error_traits::<NavigationEnqueueError>();
        for kind in [
            NavigationEnqueueErrorKind::UnknownRouter,
            NavigationEnqueueErrorKind::HostUnavailable,
            NavigationEnqueueErrorKind::AuthorityRecovering,
            NavigationEnqueueErrorKind::NoBackEntry,
        ] {
            assert_eq!(NavigationEnqueueError::new(kind).kind(), kind);
        }
    }

    #[test]
    fn commit_switches_registry_pickable_gate_and_core_history_in_one_system() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("atomic-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());

        let outputs = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .filter_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
                .then_some(record.entity())
            })
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1);
        let output = outputs[0];
        assert_eq!(app.world().get::<Pickable>(output), Some(&Pickable::IGNORE));
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            0
        );

        commit_presentation_system(app.world_mut());

        let record = app
            .world()
            .resource::<RouteEntityRegistry>()
            .record(output)
            .unwrap();
        assert_eq!(record.lifecycle(), EntityLifecycle::Active);
        assert_eq!(
            app.world().get::<Pickable>(output),
            Some(&Pickable {
                should_block_lower: true,
                is_hoverable: true,
            })
        );
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            1
        );
    }

    #[test]
    fn commits_preserve_other_router_gate_permissions() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let first = install_atomic_router(&mut app, "first");
        let second = install_atomic_router(&mut app, "second");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(first, RouteLocation::new("/"))
            .unwrap();
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(second, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());

        let outputs = app
            .world()
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .map(|entry| entry.active.as_ref().unwrap().interactive_entities()[0])
            .collect::<Vec<_>>();
        let gate = app.world().resource::<RouterInputGate>();
        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|entity| gate.allows_pointer(*entity)));
    }

    #[test]
    fn atomic_switch_releases_focus_from_the_leaving_frame() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "focus");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
        ));
        let leaving = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaving);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(leaving));

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("revision-2"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());

        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert_ne!(active, leaving);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving));
        assert!(gate.allows_pointer(active));
        assert_eq!(
            app.world().get::<Visibility>(leaving),
            Some(&Visibility::Hidden)
        );
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaving);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(leaving));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving));
        assert!(gate.allows_pointer(active));

        settle_cleanup_system(app.world_mut());
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(leaving);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(leaving));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        let gate = app.world().resource::<RouterInputGate>();
        assert!(!gate.allows_pointer(leaving));
        assert!(gate.allows_pointer(active));
    }

    #[test]
    fn refresh_clears_focus_from_an_active_claim_whose_frame_validation_failed() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "invalid-active-focus");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        let app_owned = app.world_mut().spawn_empty().id();
        assert!(validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
        ));

        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(app_owned));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(active));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(app_owned)
        );

        *app.world_mut().get_mut::<Visibility>(active).unwrap() = Visibility::Hidden;
        assert!(!validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
        ));
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(active));

        crate::input::refresh_input_gate_system(app.world_mut());

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn refresh_clears_focus_from_a_detached_quarantined_exact_entity() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "detached-quarantine-focus");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let quarantined = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/").with_fragment("replacement"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());
        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(quarantined)
                .is_some()
        );
        app.world_mut()
            .entity_mut(quarantined)
            .remove::<bevy_ecs::hierarchy::ChildOf>();
        let app_owned = app.world_mut().spawn_empty().id();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(quarantined);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(quarantined));

        crate::input::refresh_input_gate_system(app.world_mut());

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);

        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(active);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(app_owned));
        crate::input::refresh_input_gate_system(app.world_mut());
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(active));
        assert_eq!(
            app.world().resource::<RouteFocusState>().focused(),
            Some(app_owned)
        );
    }

    #[test]
    fn active_frame_and_input_gate_reject_reassigned_host_authority() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "reassigned-host");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            (
                entry.host.as_ref().unwrap().host,
                entry.active.as_ref().unwrap().interactive_entities()[0],
            )
        };
        assert!(
            app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );

        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap();
        assert!(!validate_active_frame(app.world(), active));

        crate::input::refresh_input_gate_system(app.world_mut());
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );
        assert!(app.world().get_entity(host).is_ok());
    }

    #[test]
    fn refresh_physically_isolates_an_exact_active_frame_after_host_incarnation_changes() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "isolate-invalid-active");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, claims, output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let active = entry.active.as_ref().unwrap();
            (
                entry.host.as_ref().unwrap().host,
                active.claims().cloned().collect::<Vec<_>>(),
                active.interactive_entities()[0],
            )
        };
        let foreign = app
            .world_mut()
            .spawn((
                Visibility::Visible,
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
                RouterInputBarrier::allow_all(),
            ))
            .id();
        *app.world_mut().resource_mut::<InputFocus>() = InputFocus::from_entity(output);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(output));

        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);
        assert!(!validate_active_frame(
            app.world(),
            app.world().resource::<RouterNavigation>().entries[0]
                .active
                .as_ref()
                .unwrap()
        ));

        crate::input::refresh_input_gate_system(app.world_mut());

        assert!(claims.iter().all(|claim| {
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
                && app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
        }));
        assert!(
            !app.world()
                .resource::<RouterInputGate>()
                .allows_pointer(output)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<RouteFocusState>().focused(), None);
        assert_eq!(
            app.world().get::<Visibility>(foreign),
            Some(&Visibility::Visible)
        );
        assert!(
            app.world()
                .get::<Pickable>(foreign)
                .is_some_and(|pickable| pickable.should_block_lower && pickable.is_hoverable)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(foreign)
                .is_some_and(RouterInputBarrier::allows_all)
        );
        assert!(app.world().get::<RouterHostRegistration>(host).is_some());
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            1
        );
    }

    #[test]
    fn refresh_narrows_every_existing_capability_when_one_exact_entity_lacks_pickable() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "isolate-partial-capability");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let (host, claims, output) = {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let active = entry.active.as_ref().unwrap();
            (
                entry.host.as_ref().unwrap().host,
                active.claims().cloned().collect::<Vec<_>>(),
                active.interactive_entities()[0],
            )
        };
        let missing_pickable = claims
            .iter()
            .map(|claim| claim.entity())
            .find(|entity| *entity != output)
            .unwrap();
        app.world_mut()
            .entity_mut(missing_pickable)
            .remove::<Pickable>();
        let foreign = app
            .world_mut()
            .spawn((
                Visibility::Visible,
                Pickable {
                    should_block_lower: true,
                    is_hoverable: true,
                },
                RouterInputBarrier::allow_all(),
            ))
            .id();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        crate::input::refresh_input_gate_system(app.world_mut());

        assert_eq!(app.world().get::<Pickable>(missing_pickable), None);
        assert!(claims.iter().all(|claim| {
            app.world().get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && app
                    .world()
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
                && (claim.entity() == missing_pickable
                    || app.world().get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE))
        }));
        assert_eq!(
            app.world().get::<Visibility>(foreign),
            Some(&Visibility::Visible)
        );
        assert!(
            app.world()
                .get::<Pickable>(foreign)
                .is_some_and(|pickable| pickable.should_block_lower && pickable.is_hoverable)
        );
        assert!(
            app.world()
                .get::<RouterInputBarrier>(foreign)
                .is_some_and(RouterInputBarrier::allows_all)
        );
        assert!(app.world().get::<RouterHostRegistration>(host).is_some());
    }

    #[test]
    fn active_frame_rejects_reregistered_top_slot_claim() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "reregistered-slot");
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let top_slot = app.world().resource::<RouterNavigation>().entries[0]
            .host
            .as_ref()
            .unwrap()
            .top_slot;
        let (old_claim, owner, manifest) = {
            let record = app
                .world()
                .resource::<RouteEntityRegistry>()
                .record(top_slot)
                .unwrap();
            (
                record.claim(),
                record.owner().clone(),
                record.manifest().clone(),
            )
        };
        let new_seal = {
            let mut registry = app.world_mut().resource_mut::<RouteEntityRegistry>();
            assert!(registry.remove_exact(&old_claim));
            let seal = registry.issue_seal();
            registry
                .register(
                    top_slot,
                    owner,
                    seal,
                    manifest,
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
            seal
        };
        app.world_mut()
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(new_seal));

        let active = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap();
        assert!(!validate_active_frame(app.world(), active));
    }

    #[test]
    fn locked_bevy_plugins_produce_visible_layout_and_native_picking() {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins);
        app.add_plugins(BevyUiRouterPlugin::default());

        let primary_windows = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        assert_eq!(primary_windows.len(), 1);
        app.world_mut()
            .get_mut::<Window>(primary_windows[0])
            .unwrap()
            .resolution
            .set(200.0, 120.0);
        app.world_mut().spawn(Camera2d);

        let driver = ViewDriverKey::new("physical-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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
        app.world_mut().spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            RouterHostRegistration::new(router),
        ));

        app.update();
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .is_some(),
            "physical host admission failed: records={:?} leases={:?}",
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| (
                    record.entity(),
                    record.owner(),
                    record.manifest().component_ids()
                ))
                .collect::<Vec<_>>(),
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .iter()
                .map(|lease| lease.owned_entities())
                .collect::<Vec<_>>()
        );
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        app.update();

        let output = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .find_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
                .then_some(record.entity())
            })
            .unwrap_or_else(|| {
                let navigation = app.world().resource::<RouterNavigation>();
                let entry = &navigation.entries[0];
                let request_state = match app
                    .world()
                    .resource::<RouterSnapshots>()
                    .request(router, request)
                {
                    crate::snapshots::RequestLookup::Found(snapshot) => {
                        format!("{:?} {:?}", snapshot.status(), snapshot.trace())
                    }
                    lookup => format!("{lookup:?}"),
                };
                panic!(
                    "physical output missing: host={} pending={} active={} cleanup={} history={} request={} records={:?}",
                    entry.host.is_some(),
                    entry.pending_commit.is_some(),
                    entry.active.is_some(),
                    entry.cleanup_ledger.is_some(),
                    entry.engine.history().len(),
                    request_state,
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .records()
                        .iter()
                        .map(|record| record.owner())
                        .collect::<Vec<_>>()
                )
            });
        app.update();
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .all(|record| record.manifest().matches(app.world(), record.entity()))
        );
        app.world_mut().resource_scope(
            |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                crate::presentation::assert_locked_component_manifest_closure(
                    world,
                    navigation.entries[0].active.as_ref().unwrap(),
                );
            },
        );
        let computed = app.world().get::<ComputedNode>(output).unwrap();
        assert!(
            computed.size().cmpgt(Vec2::ZERO).all(),
            "UI layout remained zero: {:?}",
            app.world()
                .iter_entities()
                .filter_map(|entity| {
                    let node = entity.get::<Node>()?;
                    Some((
                        entity.id(),
                        node.width,
                        node.height,
                        entity.get::<ComputedNode>().map(ComputedNode::size),
                        entity
                            .get::<bevy_ui::ComputedUiTargetCamera>()
                            .and_then(bevy_ui::ComputedUiTargetCamera::get),
                        entity
                            .get::<bevy_ecs::hierarchy::ChildOf>()
                            .map(|parent| parent.parent()),
                    ))
                })
                .collect::<Vec<_>>()
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(output)
                .is_some_and(|visibility| visibility.get())
        );

        let center = *app.world().get::<UiGlobalTransform>(output).unwrap() * Vec2::ZERO;
        let pointer = app
            .world_mut()
            .query::<(Entity, &PointerId)>()
            .iter(app.world())
            .find_map(|(entity, id)| (*id == PointerId::Mouse).then_some(entity))
            .unwrap();
        *app.world_mut().get_mut::<PointerLocation>(pointer).unwrap() =
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary
                        .normalize(Some(primary_windows[0]))
                        .unwrap(),
                ),
                position: center,
            });

        app.update();

        assert!(
            app.world()
                .resource::<HoverMap>()
                .get(&PointerId::Mouse)
                .is_some_and(|hits| hits.contains_key(&output))
        );

        let replacement_request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("native-picking-replacement"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let pending = app.world().resource::<RouterNavigation>().entries[0]
            .pending_commit
            .as_ref()
            .and_then(|pending| pending.realized.as_ref())
            .unwrap()
            .interactive_entities()[0];
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut().run_schedule(PreUpdate);
        let pending_hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(pending_hover.contains_key(&output));
        assert!(!pending_hover.contains_key(&pending));

        commit_presentation_system(app.world_mut());
        let leaving_hover = {
            app.world_mut().run_schedule(PreUpdate);
            app.world()
                .resource::<HoverMap>()
                .get(&PointerId::Mouse)
                .cloned()
                .unwrap_or_default()
        };
        assert!(!leaving_hover.contains_key(&output));
        assert!(!leaving_hover.contains_key(&pending));
        settle_cleanup_system(app.world_mut());
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut().run_schedule(PreUpdate);
        let replacement = app.world().resource::<RouterNavigation>().entries[0]
            .active
            .as_ref()
            .unwrap()
            .interactive_entities()[0];
        assert_ne!(
            replacement,
            output,
            "replacement did not publish: status={:?} pending={} cleanup={} active_valid={} pending_valid={} host_valid={} records={:?}",
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(replacement_request)
                .unwrap()
                .status(),
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .is_some(),
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_some(),
            validate_active_frame(
                app.world(),
                app.world().resource::<RouterNavigation>().entries[0]
                    .active
                    .as_ref()
                    .unwrap()
            ),
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .as_ref()
                .and_then(|pending| pending.realized.as_ref())
                .is_some_and(|frame| validate_realized_frame(app.world(), frame)),
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .as_ref()
                .is_some_and(|host| validated_host_candidate_is_current(
                    app.world(),
                    &host.candidate
                )),
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| {
                    (
                        record.entity(),
                        record.owner(),
                        record.lifecycle(),
                        ComponentManifest::from_world(app.world(), record.entity()).map(|actual| {
                            actual
                                .component_ids()
                                .iter()
                                .filter(|id| !record.manifest().component_ids().contains(id))
                                .filter_map(|id| {
                                    app.world()
                                        .components()
                                        .get_info(*id)
                                        .map(|info| info.name().to_owned())
                                })
                                .collect::<Vec<_>>()
                        }),
                        ComponentManifest::from_world(app.world(), record.entity()).map(|actual| {
                            record
                                .manifest()
                                .component_ids()
                                .iter()
                                .filter(|id| !actual.component_ids().contains(id))
                                .filter_map(|id| {
                                    app.world()
                                        .components()
                                        .get_info(*id)
                                        .map(|info| info.name().to_owned())
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                })
                .collect::<Vec<_>>()
        );
        let hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(!hover.contains_key(&output));
        assert!(hover.contains_key(&replacement));
        app.world_mut().resource_scope(
            |world, navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
                crate::presentation::assert_locked_component_manifest_closure(
                    world,
                    navigation.entries[0].active.as_ref().unwrap(),
                );
            },
        );
        assert_eq!(
            app.world().get::<Visibility>(output),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().get::<Pickable>(output), Some(&Pickable::IGNORE));
        assert!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .leases()
                .iter()
                .any(|lease| lease.owned_entities().contains(&output))
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn failed_host_revision_is_latched_before_the_next_observation() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("latch-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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
        let revision = app.world_mut().resource_scope(
            |world, mut revisions: bevy_ecs::change_detection::Mut<HostCandidateRevisionState>| {
                let HostCandidateClassification::Unique(candidate) =
                    classify_host_candidate(world, router, &mut revisions)
                else {
                    panic!("host should classify")
                };
                candidate.revision()
            },
        );
        let blocker = app.world_mut().spawn_empty().id();
        {
            let mut registry = app.world_mut().resource_mut::<RouteEntityRegistry>();
            let seal = registry.issue_seal();
            registry
                .register(
                    blocker,
                    RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(revision)),
                    seal,
                    ComponentManifest::new(Vec::new()),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
        }

        observe_authority_system(app.world_mut());

        assert!(
            !app.world()
                .resource::<crate::host::HostCandidateRevisionLatch>()
                .allows(revision)
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .host
                .is_none()
        );
    }

    #[test]
    fn changed_host_incarnation_rejects_commit_before_any_visible_write() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("incarnation-driver").unwrap();
        app.register_ui_view_driver(AtomicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("root").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        let output = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .iter()
            .find_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                )
                .then_some(record.entity())
            })
            .unwrap();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        commit_presentation_system(app.world_mut());

        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(output)
                .unwrap()
                .lifecycle(),
            EntityLifecycle::PreparedInert
        );
        assert_eq!(app.world().get::<Pickable>(output), Some(&Pickable::IGNORE));
        assert_eq!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .history()
                .len(),
            0
        );
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .pending_commit
                .is_some()
        );
        assert!(app.world().get_entity(host).is_ok());
    }

    #[test]
    fn changed_host_incarnation_rejects_before_driver_or_realization() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("stale-host-driver").unwrap();
        app.register_ui_view_driver(PanicDriver {
            key: driver.clone(),
        })
        .unwrap();
        let view = RouteViewKey::new("stale-host-view").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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
        let host = app
            .world_mut()
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        observe_authority_system(app.world_mut());
        let request = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        let records_before = app
            .world()
            .resource::<RouteEntityRegistry>()
            .records()
            .len();
        let registration = app
            .world_mut()
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .unwrap();
        app.world_mut().entity_mut(host).insert(registration);

        drive_navigation_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(entry.pending_commit.is_none());
        assert!(entry.active.is_none());
        assert_eq!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .len(),
            records_before
        );
        let request = entry.engine.request(request).unwrap();
        let RequestStatus::Failed(failure) = request.status() else {
            panic!("stale host apply must fail the request terminally")
        };
        assert_eq!(failure.kind(), NavigationFailureKind::Apply);
        assert_eq!(entry.engine.history().len(), 0);
    }

    #[test]
    fn driver_failure_terminates_one_request_and_preserves_the_next_action() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let driver = ViewDriverKey::new("fail-once-driver").unwrap();
        app.register_ui_view_driver(FailOnceDriver {
            key: driver.clone(),
            failed: false,
        })
        .unwrap();
        let view = RouteViewKey::new("fail-once-view").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
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
        observe_authority_system(app.world_mut());
        let first = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("second-request"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());

        drive_navigation_system(app.world_mut());

        {
            let navigation = app.world().resource::<RouterNavigation>();
            let entry = &navigation.entries[0];
            let first = entry.engine.request(first).unwrap();
            let RequestStatus::Failed(failure) = first.status() else {
                panic!("the rejected driver report must fail only its request")
            };
            assert_eq!(failure.kind(), NavigationFailureKind::Build);
            assert!(entry.pending_commit.is_none());
            assert_eq!(entry.engine.history().len(), 0);
        }

        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let navigation = app.world().resource::<RouterNavigation>();
        let entry = &navigation.entries[0];
        assert!(matches!(
            entry.engine.request(second).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(entry.active.is_some());
        assert_eq!(entry.engine.history().len(), 1);
    }

    #[test]
    fn unresolved_pending_residual_does_not_block_the_next_frame() {
        let mut app = App::new();
        app.world_mut()
            .register_component_hooks::<Pickable>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let remove = {
                    let Some(mut drift) = world.get_resource_mut::<PersistentOnePickableRemoval>()
                    else {
                        return;
                    };
                    if let Some(target) = drift.target {
                        target == entity
                    } else if drift.remaining_before_target == 0 {
                        drift.target = Some(entity);
                        true
                    } else {
                        drift.remaining_before_target -= 1;
                        false
                    }
                };
                if remove {
                    world.commands().entity(entity).remove::<Pickable>();
                }
            });
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_atomic_router(&mut app, "residual-retry");
        observe_authority_system(app.world_mut());
        let first = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        app.world_mut()
            .insert_resource(PersistentOnePickableRemoval {
                remaining_before_target: 2,
                target: None,
            });

        drive_navigation_system(app.world_mut());

        assert!(matches!(
            app.world().resource::<RouterNavigation>().entries[0]
                .engine
                .request(first)
                .unwrap()
                .status(),
            RequestStatus::Failed(_)
        ));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals()
                .len(),
            1
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .all(|record| !matches!(
                    record.owner(),
                    RegistryOwner::Committed(
                        EntityOwner::FrameShell(_)
                            | EntityOwner::FrameContentTarget(_)
                            | EntityOwner::FrameOutput { .. }
                    )
                ))
        );

        app.world_mut()
            .remove_resource::<PersistentOnePickableRemoval>();
        let second = app
            .world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/").with_fragment("after-residual"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_navigation_system(app.world_mut());
        commit_presentation_system(app.world_mut());
        settle_cleanup_system(app.world_mut());

        let entry = &app.world().resource::<RouterNavigation>().entries[0];
        assert!(matches!(
            entry.engine.request(second).unwrap().status(),
            RequestStatus::Succeeded
        ));
        assert!(entry.active.is_some());
        assert!(validate_active_frame(
            app.world(),
            entry.active.as_ref().unwrap()
        ));
    }
}
