#![allow(dead_code)]

use crate::content::FrameContentWriter;
use bevy_ui_router_core::integration::ViewReportIdentity;
use bevy_ui_router_view::{ViewBuildEffect, ViewDriverKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewBuildReportKind {
    Applied,
    BuildFailed,
    ApplyFailed,
}

pub struct ViewBuildReport {
    kind: ViewBuildReportKind,
    message: Option<String>,
    identity: ViewReportIdentity,
}

impl std::fmt::Debug for ViewBuildReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ViewBuildReport")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .finish()
    }
}

impl ViewBuildReport {
    pub fn kind(&self) -> ViewBuildReportKind {
        self.kind
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub(crate) fn identity(&self) -> &ViewReportIdentity {
        &self.identity
    }
}

pub(crate) struct ReportSeal;

impl ReportSeal {
    pub(crate) const fn new() -> Self {
        Self
    }
}

pub struct EcsViewReportSession<'a> {
    _seal: &'a mut ReportSeal,
    identity: ViewReportIdentity,
}

impl<'a> EcsViewReportSession<'a> {
    pub(crate) fn new(seal: &'a mut ReportSeal, identity: ViewReportIdentity) -> Self {
        Self {
            _seal: seal,
            identity,
        }
    }

    pub fn applied(self) -> ViewBuildReport {
        ViewBuildReport {
            kind: ViewBuildReportKind::Applied,
            message: None,
            identity: self.identity,
        }
    }

    pub fn build_failed(self, message: impl Into<String>) -> ViewBuildReport {
        ViewBuildReport {
            kind: ViewBuildReportKind::BuildFailed,
            message: Some(message.into()),
            identity: self.identity,
        }
    }

    pub fn apply_failed(self, message: impl Into<String>) -> ViewBuildReport {
        ViewBuildReport {
            kind: ViewBuildReportKind::ApplyFailed,
            message: Some(message.into()),
            identity: self.identity,
        }
    }
}

pub trait EcsViewDriver: Send + Sync + 'static {
    fn key(&self) -> ViewDriverKey;

    fn apply(
        &mut self,
        effect: &ViewBuildEffect,
        content: &mut FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::{
        integration::{EngineAction, RouterEngine},
        prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
    };

    #[derive(bevy_ecs::component::Component)]
    struct SentinelComponent(u32);

    #[derive(bevy_ecs::resource::Resource)]
    struct SentinelResource(u32);

    #[derive(Debug, Eq, PartialEq)]
    struct WorldSnapshot {
        entities: Vec<(
            bevy_ecs::entity::Entity,
            Vec<bevy_ecs::component::ComponentId>,
        )>,
        sentinel_component: u32,
        sentinel_resource: u32,
    }

    fn world_snapshot(
        world: &bevy_ecs::world::World,
        sentinel: bevy_ecs::entity::Entity,
    ) -> WorldSnapshot {
        WorldSnapshot {
            entities: world
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect(),
            sentinel_component: world.get::<SentinelComponent>(sentinel).unwrap().0,
            sentinel_resource: world.resource::<SentinelResource>().0,
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MockRegistry {
        entries: Vec<(&'static str, u32)>,
    }

    fn execute_data_only(
        content: &mut crate::content::FrameContentWriter<'_>,
        report: EcsViewReportSession<'_>,
    ) -> ViewBuildReport {
        let _ = content.spawn_empty().unwrap();
        report.applied()
    }

    fn test_report_identity() -> ViewReportIdentity {
        let view = RouteViewKey::new("driver-report").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(routes, view).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("collected root push must issue a frame task")
        };
        task.report_identity()
    }

    fn test_report_session(seal: &mut ReportSeal) -> EcsViewReportSession<'_> {
        EcsViewReportSession::new(seal, test_report_identity())
    }

    #[test]
    fn report_session_is_terminal_and_report_is_data_only() {
        let mut seal = ReportSeal::new();
        let report = test_report_session(&mut seal).applied();
        assert_eq!(report.kind(), ViewBuildReportKind::Applied);
        assert_eq!(report.message(), None);
        assert_eq!(
            format!("{report:?}"),
            "ViewBuildReport { kind: Applied, message: None }"
        );

        let mut seal = ReportSeal::new();
        let report = test_report_session(&mut seal).build_failed("redacted");
        assert_eq!(report.kind(), ViewBuildReportKind::BuildFailed);
        assert_eq!(report.message(), Some("redacted"));

        let mut seal = ReportSeal::new();
        let report = test_report_session(&mut seal).apply_failed("rejected");
        assert_eq!(report.kind(), ViewBuildReportKind::ApplyFailed);
        assert_eq!(report.message(), Some("rejected"));
    }

    #[test]
    fn driver_trait_has_the_final_immediate_shape() {
        fn assert_driver<D: EcsViewDriver>() {}
        let _ = assert_driver::<ShapeDriver>;
    }

    #[test]
    fn report_session_is_not_clone_copy_or_default() {
        trait AmbiguousIfClone<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfClone<()> for T {}
        impl<T: ?Sized + Clone> AmbiguousIfClone<u8> for T {}

        trait AmbiguousIfCopy<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfCopy<()> for T {}
        impl<T: ?Sized + Copy> AmbiguousIfCopy<u8> for T {}

        trait AmbiguousIfDefault<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfDefault<()> for T {}
        impl<T: ?Sized + Default> AmbiguousIfDefault<u8> for T {}

        let _ = <EcsViewReportSession<'static> as AmbiguousIfClone<_>>::check;
        let _ = <EcsViewReportSession<'static> as AmbiguousIfCopy<_>>::check;
        let _ = <EcsViewReportSession<'static> as AmbiguousIfDefault<_>>::check;
    }

    #[test]
    fn content_writer_and_report_session_do_not_mutate_world() {
        let mut world = bevy_ecs::world::World::new();
        let sentinel = world.spawn(SentinelComponent(41)).id();
        world.spawn_empty();
        world.insert_resource(SentinelResource(73));
        let world_before = world_snapshot(&world, sentinel);
        let registry = MockRegistry {
            entries: vec![("shape", 7), ("fallback", 11)],
        };
        let registry_before = registry.clone();

        let mut plan = crate::content::ContentPlan::new();
        let mut writer = crate::content::FrameContentWriter::new(&mut plan);
        let mut seal = ReportSeal::new();
        let report = execute_data_only(&mut writer, test_report_session(&mut seal));

        assert_eq!(report.kind(), ViewBuildReportKind::Applied);
        assert_eq!(report.message(), None);
        assert_eq!(world_snapshot(&world, sentinel), world_before);
        assert_eq!(registry, registry_before);
    }

    struct ShapeDriver;

    impl EcsViewDriver for ShapeDriver {
        fn key(&self) -> bevy_ui_router_view::ViewDriverKey {
            bevy_ui_router_view::ViewDriverKey::new("shape").unwrap()
        }

        fn apply(
            &mut self,
            _effect: &bevy_ui_router_view::ViewBuildEffect,
            _content: &mut crate::content::FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            execute_data_only(_content, report)
        }
    }
}
