use bevy_app::{App, Plugin, PreUpdate, Update};
use bevy_ecs::prelude::{Resource, SystemSet};
use bevy_ecs::schedule::IntoScheduleConfigs;

use crate::app_ext::ViewDriverStore;
use crate::navigation::RouterNavigation;
use crate::snapshots::RouterSnapshots;
use crate::state::{RouteFocusState, RouterEcsState};

#[derive(Default)]
pub struct BevyUiRouterPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum RouterScheduleSet {
    ObserveAuthority,
    CollectNavigation,
    DriveNavigation,
    CommitPresentation,
    SettleCleanup,
    ReapQuarantines,
    PublishSnapshots,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum RouterInputSet {
    Refresh,
    Ready,
}

#[derive(Resource)]
struct RouterInstallMarker;

fn install_foundations(app: &mut App) {
    if app.world().contains_resource::<RouterInstallMarker>() {
        return;
    }

    app.insert_resource(RouterInstallMarker)
        .init_resource::<RouterEcsState>()
        .init_resource::<RouteFocusState>();

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
            RouterScheduleSet::ObserveAuthority,
            RouterScheduleSet::CollectNavigation,
            RouterScheduleSet::DriveNavigation,
            RouterScheduleSet::CommitPresentation,
            RouterScheduleSet::SettleCleanup,
            RouterScheduleSet::ReapQuarantines,
            RouterScheduleSet::PublishSnapshots,
        )
            .chain(),
    )
    .configure_sets(
        PreUpdate,
        (RouterInputSet::Refresh, RouterInputSet::Ready).chain(),
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
    use bevy_app::PreUpdate;
    use bevy_ecs::schedule::{ApplyDeferred, ScheduleLabel, Schedules};
    use std::any::TypeId;

    #[test]
    fn foundation_installation_is_idempotent_and_adds_no_lifecycle_systems_or_entities() {
        fn lifecycle_system_names(schedules: &Schedules, label: impl ScheduleLabel) -> Vec<String> {
            schedules.get(label).map_or_else(Vec::new, |schedule| {
                schedule
                    .systems()
                    .expect("configured schedule should be initialized")
                    .filter(|(_, system)| system.system_type() != TypeId::of::<ApplyDeferred>())
                    .map(|(_, system)| system.name().to_string())
                    .collect()
            })
        }
        let mut app = App::new();

        install_foundations(&mut app);

        assert!(app.world().contains_resource::<RouterInstallMarker>());
        assert!(app.world().contains_resource::<RouterEcsState>());
        assert!(app.world().contains_resource::<RouteFocusState>());
        assert!(app.world().contains_resource::<ViewDriverStore>());
        assert!(app.world().contains_resource::<RouterNavigation>());
        assert!(app.world().contains_resource::<RouterSnapshots>());

        let resource_addresses = (
            app.world().resource::<RouterEcsState>() as *const _,
            app.world().resource::<RouteFocusState>() as *const _,
            app.world().resource::<ViewDriverStore>() as *const _,
            app.world().resource::<RouterNavigation>() as *const _,
            app.world().resource::<RouterSnapshots>() as *const _,
        );

        install_foundations(&mut app);

        assert_eq!(
            resource_addresses,
            (
                app.world().resource::<RouterEcsState>() as *const _,
                app.world().resource::<RouteFocusState>() as *const _,
                app.world().resource::<ViewDriverStore>() as *const _,
                app.world().resource::<RouterNavigation>() as *const _,
                app.world().resource::<RouterSnapshots>() as *const _,
            ),
            "a repeated foundation install must not replace any resource"
        );
        app.update();

        let schedules = app.world().resource::<Schedules>();
        assert!(
            lifecycle_system_names(schedules, Update).is_empty(),
            "Update contains lifecycle systems: {:?}",
            lifecycle_system_names(schedules, Update)
        );
        assert!(
            lifecycle_system_names(schedules, PreUpdate).is_empty(),
            "PreUpdate contains lifecycle systems: {:?}",
            lifecycle_system_names(schedules, PreUpdate)
        );
    }
}
