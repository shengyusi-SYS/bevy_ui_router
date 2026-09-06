use bevy_ui_router_core::test_support::BackCapacityOrderObserver;

use crate::{
    app_ext::RouterHandle,
    navigation::{RouterNavigation, install_back_capacity_order_observer_with},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackCapacityOrderInstallError {
    UnknownRouter,
    ForeignRouter,
    AlreadyInstalled,
}

pub(crate) fn testing_install_back_capacity_order_observer(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
) -> Result<BackCapacityOrderObserver, BackCapacityOrderInstallError> {
    install_back_capacity_order_observer_with(
        navigation,
        router,
        bevy_ui_router_core::test_support::issue_back_capacity_order_channels,
    )
}
