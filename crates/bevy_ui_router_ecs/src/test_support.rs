pub use bevy_ui_router_core::test_support::{BackCapacityOrderEvent, BackCapacityOrderObserver};

use crate::{
    app_ext::RouterHandle,
    navigation::{
        BackRequestInventoryProjection, RouterNavigation, back_request_inventory_projection,
        install_back_capacity_order_observer_with,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackRequestInventory {
    inner: BackRequestInventoryProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackCapacityOrderInstallError {
    UnknownRouter,
    ForeignRouter,
    AlreadyInstalled,
}

#[doc(hidden)]
pub fn testing_install_back_capacity_order_observer(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
) -> Result<BackCapacityOrderObserver, BackCapacityOrderInstallError> {
    install_back_capacity_order_observer_with(
        navigation,
        router,
        bevy_ui_router_core::test_support::issue_back_capacity_order_channels,
    )
}

#[doc(hidden)]
pub fn testing_back_request_inventory(
    navigation: &RouterNavigation,
    router: RouterHandle,
) -> Option<BackRequestInventory> {
    back_request_inventory_projection(navigation, router)
        .map(|inner| BackRequestInventory { inner })
}
