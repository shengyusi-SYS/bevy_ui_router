use std::fmt::Debug;

use bevy_ui_router_core::integration::HistoryExecutionBindingView;

fn assert_opaque_projection_contract<T: Clone + Debug + Eq + PartialEq>() {}

#[test]
fn history_execution_binding_view_is_an_opaque_cloneable_integration_projection() {
    assert_opaque_projection_contract::<HistoryExecutionBindingView>();
}
