use std::fmt::Debug;

use bevy_ui_router_core::integration::{
    CanonicalHistoryView, HistoryEntryKey, HistoryExecutionBindingView, HistoryFrameFact,
    HistoryPublicationFacts,
};

fn assert_opaque_projection_contract<T: Clone + Debug + Eq + PartialEq>() {}

#[test]
fn history_execution_binding_view_is_an_opaque_cloneable_integration_projection() {
    assert_opaque_projection_contract::<HistoryExecutionBindingView>();
}

#[test]
fn canonical_history_projections_are_read_only_values_for_integration_consumers() {
    fn assert_entry_key_contract<T: Copy + std::hash::Hash + Eq>() {}
    assert_entry_key_contract::<HistoryEntryKey>();
    assert_opaque_projection_contract::<CanonicalHistoryView>();
    assert_opaque_projection_contract::<HistoryPublicationFacts>();
    assert_opaque_projection_contract::<HistoryFrameFact>();
}
