use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::SystemSet;
use bevy_ecs::schedule::IntoScheduleConfigs;

use crate::state::{RouteFocusState, RouterEcsState};

pub struct BevyUiRouterPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum RouterScheduleSet {
    CollectNavigation,
    PlanFrames,
    BuildViews,
    ApplyViews,
    PrepareInert,
    ReportToCore,
    ActivateCommitted,
    Cleanup,
    AcknowledgeFailures,
}

impl Plugin for BevyUiRouterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RouterEcsState>()
            .init_resource::<RouteFocusState>()
            .configure_sets(
                Update,
                (
                    RouterScheduleSet::CollectNavigation,
                    RouterScheduleSet::PlanFrames,
                    RouterScheduleSet::BuildViews,
                    RouterScheduleSet::ApplyViews,
                    RouterScheduleSet::PrepareInert,
                    RouterScheduleSet::ReportToCore,
                    RouterScheduleSet::ActivateCommitted,
                    RouterScheduleSet::Cleanup,
                    RouterScheduleSet::AcknowledgeFailures,
                )
                    .chain(),
            );
    }
}
