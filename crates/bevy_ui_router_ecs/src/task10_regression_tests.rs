#[test]
fn partial_receipt_then_missing_pickable_still_quarantines_and_terminals() {
    crate::authority_regression_tests::partial_receipt_then_missing_pickable_still_quarantines_and_terminals();
}

#[test]
fn pickable_repair_observer_drift_supersedes_attempt_before_quarantine_transfer() {
    crate::authority_regression_tests::pickable_repair_observer_drift_supersedes_attempt_before_quarantine_transfer();
}
