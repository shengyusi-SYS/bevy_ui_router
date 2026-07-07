#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackIntent {
    NavigateBack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackPolicy {
    PopHistory,
    Block,
    DelegateToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackCapabilitySnapshot {
    can_pop_history: bool,
    has_local_layer: bool,
    host_may_handle: bool,
}

impl BackCapabilitySnapshot {
    pub const fn new(can_pop_history: bool, has_local_layer: bool, host_may_handle: bool) -> Self {
        Self {
            can_pop_history,
            has_local_layer,
            host_may_handle,
        }
    }

    pub const fn can_pop_history(self) -> bool {
        self.can_pop_history
    }

    pub const fn has_local_layer(self) -> bool {
        self.has_local_layer
    }

    pub const fn host_may_handle(self) -> bool {
        self.host_may_handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_capability_snapshot_is_router_neutral() {
        let snapshot = BackCapabilitySnapshot::new(true, false, true);

        assert!(snapshot.can_pop_history());
        assert!(!snapshot.has_local_layer());
        assert!(snapshot.host_may_handle());
    }
}
