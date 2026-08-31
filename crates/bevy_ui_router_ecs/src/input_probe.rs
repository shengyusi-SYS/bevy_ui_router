use bevy_ecs::{prelude::World, resource::Resource};

use crate::{
    app_ext::RouterHandle,
    audit::{AuditEntityId, RouterAuthorityAudits},
    input::RouterInputGate,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterInputModality {
    Pointer,
    Keyboard,
    Gamepad,
}

impl RouterInputModality {
    const fn index(self) -> usize {
        match self {
            Self::Pointer => 0,
            Self::Keyboard => 1,
            Self::Gamepad => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputProbeResult {
    Delivered,
    Blocked,
    Unknown,
}

struct InputProbeTarget {
    router: RouterHandle,
    target: AuditEntityId,
    allowed: [bool; 3],
}

struct InputProbeDeliveryCounter {
    target: AuditEntityId,
    counts: [u64; 3],
}

#[derive(Resource)]
pub struct RouterInputProbes {
    targets: Vec<InputProbeTarget>,
    deliveries: Vec<InputProbeDeliveryCounter>,
}

impl RouterInputProbes {
    pub(crate) const fn new() -> Self {
        Self {
            targets: Vec::new(),
            deliveries: Vec::new(),
        }
    }

    pub fn dispatch(
        &mut self,
        router: RouterHandle,
        target: AuditEntityId,
        modality: RouterInputModality,
    ) -> InputProbeResult {
        let Some(resolved) = self
            .targets
            .iter()
            .find(|resolved| resolved.router == router && resolved.target == target)
        else {
            return InputProbeResult::Unknown;
        };
        if !resolved.allowed[modality.index()] {
            return InputProbeResult::Blocked;
        }
        let counter = match self
            .deliveries
            .iter_mut()
            .find(|counter| counter.target == target)
        {
            Some(counter) => counter,
            None => {
                self.deliveries.push(InputProbeDeliveryCounter {
                    target,
                    counts: [0; 3],
                });
                self.deliveries
                    .last_mut()
                    .expect("the input probe delivery counter was just inserted")
            }
        };
        counter.counts[modality.index()] = counter.counts[modality.index()]
            .checked_add(1)
            .expect("input probe delivery counter exhausted");
        InputProbeResult::Delivered
    }

    pub fn delivery_count(&self, target: AuditEntityId, modality: RouterInputModality) -> u64 {
        self.deliveries
            .iter()
            .find(|counter| counter.target == target)
            .map_or(0, |counter| counter.counts[modality.index()])
    }

    pub(crate) fn refresh(&mut self, gate: &RouterInputGate, audits: &RouterAuthorityAudits) {
        self.targets.clear();
        self.targets.extend(
            audits
                .input_probe_targets()
                .map(|(router, target, entity, owned)| InputProbeTarget {
                    router,
                    target,
                    allowed: [
                        owned && gate.allows_pointer(entity),
                        owned && gate.allows_keyboard(entity),
                        owned && gate.allows_gamepad(entity),
                    ],
                }),
        );
    }
}

pub(crate) fn refresh_input_probes(world: &mut World) {
    if !world.contains_resource::<RouterAuthorityAudits>()
        || !world.contains_resource::<RouterInputProbes>()
        || !world.contains_resource::<RouterInputGate>()
    {
        return;
    }
    world.resource_scope(
        |world, audits: bevy_ecs::prelude::Mut<RouterAuthorityAudits>| {
            world.resource_scope(
                |world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
                    probes.refresh(world.resource::<RouterInputGate>(), &audits);
                },
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::{AppRouterExt, RouterConfig, RouterHostRegistration},
        audit::{AuthorityFrameLifecycle, RouterAuthorityAuditSnapshot},
        content::{ContentColor, FrameContentWriter, SolidRect},
        driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
        navigation::RouterNavigation,
        plugin::{BevyUiRouterPlugin, RouterScheduleSet},
    };
    use bevy_app::{App, PreUpdate, Update};
    use bevy_ecs::{prelude::Resource, schedule::IntoScheduleConfigs};
    use bevy_ui::Node;
    use bevy_ui_router_core::prelude::{
        RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    const MODALITIES: [RouterInputModality; 3] = [
        RouterInputModality::Pointer,
        RouterInputModality::Keyboard,
        RouterInputModality::Gamepad,
    ];

    struct ProbeDriver(ViewDriverKey);

    impl EcsViewDriver for ProbeDriver {
        fn key(&self) -> ViewDriverKey {
            self.0.clone()
        }

        fn apply(
            &mut self,
            _effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            content
                .spawn_solid_rect(
                    SolidRect::new(29, 17, ContentColor::srgba8(5, 13, 29, 255)).unwrap(),
                )
                .unwrap();
            report.applied()
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct BoundarySample {
        allowed: AuditEntityId,
        blocked: AuditEntityId,
        allowed_results: [InputProbeResult; 3],
        blocked_results: [InputProbeResult; 3],
        allowed_counts: [u64; 3],
        blocked_counts: [u64; 3],
    }

    #[derive(Resource)]
    struct InputProbeBoundaryState {
        router: RouterHandle,
        armed: bool,
        before: Option<BoundarySample>,
        after: Option<BoundarySample>,
    }

    fn content_for_lifecycle(
        audit: &RouterAuthorityAuditSnapshot,
        lifecycle: AuthorityFrameLifecycle,
    ) -> AuditEntityId {
        let frame = audit
            .frames()
            .iter()
            .find(|frame| frame.lifecycle() == lifecycle)
            .expect("the boundary exposes the expected frame lifecycle")
            .frame();
        audit
            .content()
            .iter()
            .find(|content| content.frame() == frame)
            .expect("the public driver produced one content target for the frame")
            .entity()
    }

    fn sample_dispatch(
        probes: &mut RouterInputProbes,
        router: RouterHandle,
        allowed: AuditEntityId,
        blocked: AuditEntityId,
    ) -> BoundarySample {
        let allowed_results = MODALITIES.map(|modality| probes.dispatch(router, allowed, modality));
        let blocked_results = MODALITIES.map(|modality| probes.dispatch(router, blocked, modality));
        BoundarySample {
            allowed,
            blocked,
            allowed_results,
            blocked_results,
            allowed_counts: MODALITIES.map(|modality| probes.delivery_count(allowed, modality)),
            blocked_counts: MODALITIES.map(|modality| probes.delivery_count(blocked, modality)),
        }
    }

    fn capture_before_commit(world: &mut World) {
        let (router, armed) = {
            let state = world.resource::<InputProbeBoundaryState>();
            (state.router, state.armed)
        };
        if !armed {
            return;
        }
        let (active, pending) = {
            let audit = world
                .resource::<RouterAuthorityAudits>()
                .get(router)
                .expect("the real-time authority audit exists before commit");
            (
                content_for_lifecycle(audit, AuthorityFrameLifecycle::Active),
                content_for_lifecycle(audit, AuthorityFrameLifecycle::PreparedInert),
            )
        };
        let sample = world.resource_scope(
            |_world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
                sample_dispatch(&mut probes, router, active, pending)
            },
        );
        world.resource_mut::<InputProbeBoundaryState>().before = Some(sample);
    }

    fn capture_after_commit(world: &mut World) {
        let (router, armed) = {
            let state = world.resource::<InputProbeBoundaryState>();
            (state.router, state.armed)
        };
        if !armed {
            return;
        }
        let (active, leaving) = {
            let audit = world
                .resource::<RouterAuthorityAudits>()
                .get(router)
                .expect("the real-time authority audit exists after commit");
            (
                content_for_lifecycle(audit, AuthorityFrameLifecycle::Active),
                content_for_lifecycle(audit, AuthorityFrameLifecycle::LeavingInert),
            )
        };
        let sample = world.resource_scope(
            |_world, mut probes: bevy_ecs::prelude::Mut<RouterInputProbes>| {
                sample_dispatch(&mut probes, router, active, leaving)
            },
        );
        world.resource_mut::<InputProbeBoundaryState>().after = Some(sample);
    }

    fn install_probe_router(app: &mut App) -> RouterHandle {
        let driver = app
            .register_ui_view_driver(ProbeDriver(
                ViewDriverKey::new("input-probe-driver").unwrap(),
            ))
            .unwrap();
        let view = RouteViewKey::new("input-probe-view").unwrap();
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
        app.update();
        router
    }

    #[test]
    fn app_update_keeps_input_probe_results_atomic_across_commit() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_probe_router(&mut app);
        app.insert_resource(InputProbeBoundaryState {
            router,
            armed: false,
            before: None,
            after: None,
        });
        app.add_systems(
            Update,
            capture_before_commit
                .after(RouterScheduleSet::DriveNavigation)
                .before(RouterScheduleSet::CommitPresentation),
        );
        app.add_systems(
            Update,
            capture_after_commit
                .after(RouterScheduleSet::CommitPresentation)
                .before(RouterScheduleSet::SettleCleanup),
        );

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        app.update();

        app.world_mut()
            .resource_mut::<InputProbeBoundaryState>()
            .armed = true;
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .replace(router, RouteLocation::new("/items/2"))
            .unwrap();
        app.update();

        let state = app.world().resource::<InputProbeBoundaryState>();
        let before = state.before.expect("the pre-commit boundary was sampled");
        let after = state.after.expect("the post-commit boundary was sampled");
        assert_eq!(before.allowed_results, [InputProbeResult::Delivered; 3]);
        assert_eq!(before.blocked_results, [InputProbeResult::Blocked; 3]);
        assert_eq!(before.allowed_counts, [1; 3]);
        assert_eq!(before.blocked_counts, [0; 3]);
        assert_eq!(after.allowed_results, [InputProbeResult::Delivered; 3]);
        assert_eq!(after.blocked_results, [InputProbeResult::Blocked; 3]);
        assert_eq!(after.allowed_counts, [1; 3]);
        assert_eq!(after.blocked_counts, [1; 3]);
        assert_eq!(before.allowed, after.blocked);
        assert_eq!(before.blocked, after.allowed);
    }

    #[test]
    fn preupdate_authority_drift_revokes_probe_delivery_without_replaying_the_counter() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = install_probe_router(&mut app);
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        app.update();

        let target = app
            .world()
            .resource::<RouterAuthorityAudits>()
            .get(router)
            .unwrap()
            .content()[0]
            .entity();
        assert_eq!(
            app.world_mut()
                .resource_mut::<RouterInputProbes>()
                .dispatch(router, target, RouterInputModality::Pointer),
            InputProbeResult::Delivered
        );

        let entity = app
            .world()
            .resource::<RouterNavigation>()
            .entries
            .iter()
            .find(|entry| entry.handle == router)
            .and_then(|entry| entry.active.as_ref())
            .and_then(|active| active.interactive_entities().into_iter().next())
            .expect("the active public SolidRect has one exact interactive Entity");
        assert!(
            app.world_mut()
                .entity_mut(entity)
                .take::<crate::components::RouteOwnershipSeal>()
                .is_some()
        );
        app.world_mut().run_schedule(PreUpdate);

        let mut probes = app.world_mut().resource_mut::<RouterInputProbes>();
        assert_eq!(
            probes.dispatch(router, target, RouterInputModality::Pointer),
            InputProbeResult::Blocked
        );
        assert_eq!(
            probes.delivery_count(target, RouterInputModality::Pointer),
            1
        );
    }
}
