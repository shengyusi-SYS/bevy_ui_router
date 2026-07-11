use bevy_ui_router_core::prelude::{
    NavigationPhase, RequestStatus, RouterAvailability, RouterUnavailableReason,
};

#[test]
fn read_only_root_state_variants_are_available_to_consumers() {
    assert_eq!(RouterAvailability::AwaitingHost.unavailable_reason(), None);
    assert_eq!(
        RouterAvailability::Unavailable(RouterUnavailableReason::HostLost).unavailable_reason(),
        Some(RouterUnavailableReason::HostLost)
    );
    assert_eq!(
        RequestStatus::Running(NavigationPhase::Matching),
        RequestStatus::Running(NavigationPhase::Matching)
    );
}
