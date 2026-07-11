#![allow(dead_code)]

use crate::ids::{CoreIdAllocator, RouteHostId, RouteSlotId, RouterInstanceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostVersion {
    router: RouterInstanceId,
    host: RouteHostId,
    epoch: u64,
}

#[derive(Debug)]
pub struct HostAdmissionToken {
    router: RouterInstanceId,
    sequence: u64,
}

#[derive(Debug)]
pub struct HostGrant {
    version: HostVersion,
    top_slot: RouteSlotId,
}

#[derive(Debug)]
pub struct HostAdmissionConfirmation {
    version: HostVersion,
    top_slot: RouteSlotId,
}

#[derive(Debug)]
pub(crate) struct HostAdmissionIssuer {
    next_sequence: u64,
    next_epoch: u64,
}

impl HostAdmissionIssuer {
    pub(crate) const fn new() -> Self {
        Self {
            next_sequence: 1,
            next_epoch: 1,
        }
    }

    pub(crate) fn issue(&mut self, router: RouterInstanceId) -> HostAdmissionToken {
        let token = HostAdmissionToken::new(router, self.next_sequence);
        self.next_sequence += 1;
        token
    }

    pub(crate) fn grant(
        &mut self,
        token: HostAdmissionToken,
        ids: &mut CoreIdAllocator,
    ) -> HostGrant {
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        HostGrant {
            version: HostVersion {
                router: token.router,
                host: ids.allocate_host(),
                epoch,
            },
            top_slot: ids.allocate_slot(),
        }
    }

    pub(crate) fn confirm(&mut self, grant: HostGrant) -> HostAdmissionConfirmation {
        HostAdmissionConfirmation {
            version: grant.version,
            top_slot: grant.top_slot,
        }
    }
}

impl HostAdmissionToken {
    fn new(router: RouterInstanceId, sequence: u64) -> Self {
        Self { router, sequence }
    }
}

impl HostAdmissionConfirmation {
    pub(crate) const fn version(&self) -> &HostVersion {
        &self.version
    }

    pub(crate) const fn top_slot(&self) -> RouteSlotId {
        self.top_slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_and_slot_ids_are_allocated_only_after_token_issue() {
        let mut ids = CoreIdAllocator::new();
        let router = ids.allocate_router();
        let mut issuer = HostAdmissionIssuer::new();
        let token = issuer.issue(router);
        let grant = issuer.grant(token, &mut ids);
        let confirmation = issuer.confirm(grant);

        assert_eq!(confirmation.version().router, router);
        assert_ne!(confirmation.top_slot(), RouteSlotId::new(0));
    }
}
