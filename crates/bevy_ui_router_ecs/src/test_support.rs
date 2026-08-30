pub use bevy_ui_router_core::test_support::{BackCapacityOrderEvent, BackCapacityOrderObserver};

use crate::{
    app_ext::RouterHandle,
    navigation::{
        BackRequestInventoryProjection, RouterNavigation, back_request_inventory_projection,
        install_back_capacity_order_observer_with, router_request_inventory_counts,
    },
    snapshots::RouterSnapshots,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackRequestInventory {
    inner: BackRequestInventoryProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouterRequestInventory {
    navigation_routers: usize,
    navigation_requests: usize,
    snapshot_routers: usize,
    snapshot_requests: usize,
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

#[doc(hidden)]
pub fn testing_router_request_inventory(
    navigation: &RouterNavigation,
    snapshots: &RouterSnapshots,
) -> RouterRequestInventory {
    let (navigation_routers, navigation_requests) = router_request_inventory_counts(navigation);
    let (snapshot_routers, snapshot_requests) = snapshots.inventory_counts();
    RouterRequestInventory {
        navigation_routers,
        navigation_requests,
        snapshot_routers,
        snapshot_requests,
    }
}

#[doc(hidden)]
pub fn testing_remove_host_registration_after_next_commit_seal(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
) -> bool {
    navigation.testing_remove_host_registration_after_next_commit_seal(router)
}

#[doc(hidden)]
pub fn testing_commit_sealed_registration_removal_fired(
    navigation: &RouterNavigation,
    router: RouterHandle,
) -> bool {
    navigation.testing_commit_sealed_registration_removal_fired(router)
}
