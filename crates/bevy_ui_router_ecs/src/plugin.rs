use bevy_app::{App, Plugin, PreUpdate, Update};
use bevy_ecs::prelude::{Resource, SystemSet};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_input_focus::InputFocus;
use bevy_picking::{
    PickingSystems,
    hover::{generate_hovermap, update_interactions},
};

use crate::app_ext::ViewDriverStore;
use crate::host::{HostCandidateRevisionLatch, HostCandidateRevisionState};
use crate::input::{
    RouteFocusState, RouterInputGate, filter_inactive_router_pointer_state_system,
    input_ready_system, refresh_input_gate_system,
};
use crate::navigation::RouterNavigation;
use crate::navigation::{
    collect_navigation_system, commit_presentation_system, drive_navigation_system,
    observe_authority_system, publish_snapshots_system, settle_cleanup_system,
};
use crate::registry::{QuarantineLeaseTable, RouteEntityRegistry};
use crate::snapshots::RouterSnapshots;

#[derive(Default)]
pub struct BevyUiRouterPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum RouterScheduleSet {
    PublishSnapshots,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum RouterLifecycleSet {
    ObserveAuthority,
    CollectNavigation,
    DriveNavigation,
    CommitPresentation,
    SettleCleanup,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum RouterInputSet {
    Refresh,
    Ready,
}

#[derive(Resource)]
struct RouterInstallMarker;

pub(crate) fn install_foundations(app: &mut App) {
    if app.world().contains_resource::<RouterInstallMarker>() {
        return;
    }

    app.insert_resource(RouterInstallMarker)
        .init_resource::<InputFocus>()
        .init_resource::<RouteFocusState>()
        .init_resource::<HostCandidateRevisionState>()
        .init_resource::<HostCandidateRevisionLatch>()
        .init_resource::<RouteEntityRegistry>()
        .init_resource::<QuarantineLeaseTable>();

    if !app.world().contains_resource::<RouterInputGate>() {
        app.insert_resource(RouterInputGate::new());
    }
    if !app.world().contains_resource::<ViewDriverStore>() {
        app.insert_resource(ViewDriverStore::new());
    }
    if !app.world().contains_resource::<RouterNavigation>() {
        app.insert_resource(RouterNavigation::new());
    }
    if !app.world().contains_resource::<RouterSnapshots>() {
        app.insert_resource(RouterSnapshots::new());
    }

    app.configure_sets(
        Update,
        (
            RouterLifecycleSet::ObserveAuthority,
            RouterLifecycleSet::CollectNavigation,
            RouterLifecycleSet::DriveNavigation,
            RouterLifecycleSet::CommitPresentation,
            RouterLifecycleSet::SettleCleanup,
            RouterScheduleSet::PublishSnapshots,
        )
            .chain(),
    )
    .configure_sets(
        PreUpdate,
        (
            RouterInputSet::Refresh.before(PickingSystems::Hover),
            RouterInputSet::Ready.after(PickingSystems::Hover),
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            observe_authority_system.in_set(RouterLifecycleSet::ObserveAuthority),
            collect_navigation_system.in_set(RouterLifecycleSet::CollectNavigation),
            drive_navigation_system.in_set(RouterLifecycleSet::DriveNavigation),
            commit_presentation_system.in_set(RouterLifecycleSet::CommitPresentation),
            settle_cleanup_system.in_set(RouterLifecycleSet::SettleCleanup),
            publish_snapshots_system.in_set(RouterScheduleSet::PublishSnapshots),
        ),
    )
    .add_systems(
        PreUpdate,
        (
            refresh_input_gate_system.in_set(RouterInputSet::Refresh),
            filter_inactive_router_pointer_state_system
                .in_set(PickingSystems::Hover)
                .after(generate_hovermap)
                .before(update_interactions),
            input_ready_system.in_set(RouterInputSet::Ready),
        ),
    );
}

impl Plugin for BevyUiRouterPlugin {
    fn build(&self, app: &mut App) {
        install_foundations(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_ext::ViewDriverStore, navigation::RouterNavigation, snapshots::RouterSnapshots,
    };
    use crate::{
        host::{HostCandidateRevisionLatch, HostCandidateRevisionState},
        registry::{QuarantineLeaseTable, RouteEntityRegistry},
    };
    use bevy_app::PreUpdate;
    use bevy_ecs::schedule::{ApplyDeferred, ScheduleLabel, Schedules};
    use std::any::TypeId;

    #[test]
    fn lifecycle_installation_is_idempotent_and_registers_the_single_system_chain() {
        fn lifecycle_system_names(schedules: &Schedules, label: impl ScheduleLabel) -> Vec<String> {
            schedules.get(label).map_or_else(Vec::new, |schedule| {
                let mut names = schedule
                    .systems()
                    .expect("configured schedule should be initialized")
                    .filter(|(_, system)| system.system_type() != TypeId::of::<ApplyDeferred>())
                    .map(|(_, system)| system.name().to_string())
                    .collect::<Vec<_>>();
                names.sort();
                names
            })
        }
        let mut app = App::new();
        let router_structure_before = app
            .world()
            .iter_entities()
            .filter(|entity| {
                entity.contains::<crate::components::RouteOwnershipSeal>()
                    || entity.contains::<crate::components::ProvisionalSlotLifecycle>()
                    || entity.contains::<crate::app_ext::RouterHostRegistration>()
            })
            .count();

        install_foundations(&mut app);

        assert!(app.world().contains_resource::<RouterInstallMarker>());
        assert!(app.world().contains_resource::<RouteFocusState>());
        assert!(app.world().contains_resource::<RouterInputGate>());
        assert!(app.world().contains_resource::<ViewDriverStore>());
        assert!(app.world().contains_resource::<RouterNavigation>());
        assert!(app.world().contains_resource::<RouterSnapshots>());
        assert!(
            app.world()
                .contains_resource::<HostCandidateRevisionState>()
        );
        assert!(
            app.world()
                .contains_resource::<HostCandidateRevisionLatch>()
        );
        assert!(app.world().contains_resource::<RouteEntityRegistry>());
        assert!(app.world().contains_resource::<QuarantineLeaseTable>());

        app.update();
        let installed_systems = {
            let schedules = app.world().resource::<Schedules>();
            (
                lifecycle_system_names(schedules, Update),
                lifecycle_system_names(schedules, PreUpdate),
            )
        };
        assert_eq!(installed_systems.0.len(), 6);
        assert_eq!(installed_systems.1.len(), 3);
        let resource_addresses = (
            app.world().resource::<RouteFocusState>() as *const _,
            app.world().resource::<RouterInputGate>() as *const _,
            app.world().resource::<ViewDriverStore>() as *const _,
            app.world().resource::<RouterNavigation>() as *const _,
            app.world().resource::<RouterSnapshots>() as *const _,
            app.world().resource::<HostCandidateRevisionState>() as *const _,
            app.world().resource::<HostCandidateRevisionLatch>() as *const _,
            app.world().resource::<RouteEntityRegistry>() as *const _,
            app.world().resource::<QuarantineLeaseTable>() as *const _,
        );

        install_foundations(&mut app);

        assert_eq!(
            resource_addresses,
            (
                app.world().resource::<RouteFocusState>() as *const _,
                app.world().resource::<RouterInputGate>() as *const _,
                app.world().resource::<ViewDriverStore>() as *const _,
                app.world().resource::<RouterNavigation>() as *const _,
                app.world().resource::<RouterSnapshots>() as *const _,
                app.world().resource::<HostCandidateRevisionState>() as *const _,
                app.world().resource::<HostCandidateRevisionLatch>() as *const _,
                app.world().resource::<RouteEntityRegistry>() as *const _,
                app.world().resource::<QuarantineLeaseTable>() as *const _,
            ),
            "a repeated foundation install must not replace any resource"
        );
        assert_eq!(
            app.world()
                .iter_entities()
                .filter(|entity| {
                    entity.contains::<crate::components::RouteOwnershipSeal>()
                        || entity.contains::<crate::components::ProvisionalSlotLifecycle>()
                        || entity.contains::<crate::app_ext::RouterHostRegistration>()
                })
                .count(),
            router_structure_before
        );
        app.update();

        let schedules = app.world().resource::<Schedules>();
        assert_eq!(
            (
                lifecycle_system_names(schedules, Update),
                lifecycle_system_names(schedules, PreUpdate),
            ),
            installed_systems,
            "reinstalling must not add or replace scheduled systems"
        );
    }
}
