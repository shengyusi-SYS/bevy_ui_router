use bevy_ui_router::{
    ecs::{RouterInputSet, RouterScheduleSet},
    prelude::{BevyUiRouterPlugin, RouterNavigation, RouterSnapshots},
};

#[test]
fn ecs_facade_exposes_root_runtime_and_module_only_schedule_labels() {
    fn public<T>() {}
    public::<BevyUiRouterPlugin>();
    public::<RouterNavigation>();
    public::<RouterSnapshots>();
    public::<RouterInputSet>();
    public::<RouterScheduleSet>();
    let _ = RouterInputSet::Refresh;
    let _ = RouterInputSet::Ready;
    let _ = RouterScheduleSet::PublishSnapshots;
}
