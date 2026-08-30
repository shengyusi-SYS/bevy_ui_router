pub use bevy_ui_router_core::test_support::{BackCapacityOrderEvent, BackCapacityOrderObserver};

use crate::{
    app_ext::RouterHandle,
    navigation::{
        BackRequestInventoryProjection, RouterNavigation,
        TestingDelayedDriverReportReplayControlError, back_request_inventory_projection,
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
#[derive(Debug, Eq, PartialEq)]
pub struct TestingDelayedDriverReportReplay {
    router: RouterHandle,
    control: u64,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestingDelayedDriverReportReplayError {
    UnknownRouter,
    AlreadyArmed,
    NothingCaptured,
    ForeignControl,
}

fn map_delayed_driver_report_replay_error(
    error: TestingDelayedDriverReportReplayControlError,
) -> TestingDelayedDriverReportReplayError {
    match error {
        TestingDelayedDriverReportReplayControlError::UnknownRouter => {
            TestingDelayedDriverReportReplayError::UnknownRouter
        }
        TestingDelayedDriverReportReplayControlError::AlreadyArmed => {
            TestingDelayedDriverReportReplayError::AlreadyArmed
        }
        TestingDelayedDriverReportReplayControlError::NothingCaptured => {
            TestingDelayedDriverReportReplayError::NothingCaptured
        }
        TestingDelayedDriverReportReplayControlError::ForeignControl => {
            TestingDelayedDriverReportReplayError::ForeignControl
        }
    }
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

#[doc(hidden)]
pub fn testing_arm_next_applied_driver_report_replay(
    navigation: &mut RouterNavigation,
    router: RouterHandle,
) -> Result<TestingDelayedDriverReportReplay, TestingDelayedDriverReportReplayError> {
    navigation
        .testing_arm_next_applied_driver_report_replay(router)
        .map(|control| TestingDelayedDriverReportReplay { router, control })
        .map_err(map_delayed_driver_report_replay_error)
}

#[doc(hidden)]
pub fn testing_release_applied_driver_report_replay(
    navigation: &mut RouterNavigation,
    control: TestingDelayedDriverReportReplay,
) -> Result<(), TestingDelayedDriverReportReplayError> {
    let TestingDelayedDriverReportReplay { router, control } = control;
    navigation
        .testing_release_applied_driver_report_replay(router, control)
        .map_err(map_delayed_driver_report_replay_error)
}
