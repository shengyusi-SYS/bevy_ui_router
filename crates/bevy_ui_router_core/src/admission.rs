//! ```compile_fail
//! use bevy_ui_router_core::prelude::{HostSnapshot, HostTraceEvent, HostTraceEventKind};
//!
//! let _ = HostTraceEvent::project(HostTraceEventKind::AdmissionRequested, None, None);
//! let _ = HostSnapshot::project(None, None, Vec::new());
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

use crate::ids::{
    CoreIdAllocator, IdExhaustion, RouteHostId, RouteSlotId, RouterIdentity, RouterInstanceId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostVersion {
    router: RouterIdentity,
    host: RouteHostId,
    epoch: u64,
}

impl HostVersion {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn router(&self) -> RouterInstanceId {
        self.router.router()
    }

    pub(crate) const fn router_identity(&self) -> RouterIdentity {
        self.router
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn host(&self) -> RouteHostId {
        self.host
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostTraceEventKind {
    AdmissionRequested,
    AdmissionRejected,
    AdmissionRejectedAfterMutation,
    HostGranted,
    TopSlotRegistered,
    HostReady,
    SettlementStarted,
    SettlementCompleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostTraceEvent {
    kind: HostTraceEventKind,
    version: Option<HostVersion>,
    slot: Option<RouteSlotId>,
}

impl HostTraceEvent {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn project(
        kind: HostTraceEventKind,
        version: Option<HostVersion>,
        slot: Option<RouteSlotId>,
    ) -> Self {
        Self {
            kind,
            version,
            slot,
        }
    }

    pub fn kind(&self) -> HostTraceEventKind {
        self.kind
    }

    pub fn version(&self) -> Option<&HostVersion> {
        self.version.as_ref()
    }

    pub fn slot(&self) -> Option<RouteSlotId> {
        self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostSnapshot {
    version: Option<HostVersion>,
    top_slot: Option<RouteSlotId>,
    trace: Vec<HostTraceEvent>,
}

impl HostSnapshot {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn project(
        version: Option<HostVersion>,
        top_slot: Option<RouteSlotId>,
        trace: Vec<HostTraceEvent>,
    ) -> Self {
        Self {
            version,
            top_slot,
            trace,
        }
    }

    pub fn version(&self) -> Option<&HostVersion> {
        self.version.as_ref()
    }

    pub fn top_slot(&self) -> Option<RouteSlotId> {
        self.top_slot
    }

    pub fn trace(&self) -> &[HostTraceEvent] {
        &self.trace
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdmissionKey {
    issuer: AdmissionIssuerId,
    router: RouterIdentity,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdmissionIssuerId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionToken {
    key: AdmissionKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostGrant {
    key: AdmissionKey,
    version: HostVersion,
    top_slot: RouteSlotId,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostAdmissionConfirmation {
    key: AdmissionKey,
    version: HostVersion,
    top_slot: RouteSlotId,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostLease {
    identity: GrantIdentity,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RetiredHostAuthority {
    identity: GrantIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetiredHostAuthorityProjection {
    key: AdmissionKey,
    version: HostVersion,
    top_slot: RouteSlotId,
}

#[derive(Debug)]
pub(crate) struct RetireHostLeaseError {
    owner: HostLease,
}

impl RetireHostLeaseError {
    const fn new(owner: HostLease) -> Self {
        Self { owner }
    }

    pub(crate) fn into_owner(self) -> HostLease {
        self.owner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OutstandingAdmission {
    Requested(AdmissionKey),
    Granted(GrantIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GrantIdentity {
    key: AdmissionKey,
    version: HostVersion,
    top_slot: RouteSlotId,
}

impl GrantIdentity {
    fn from_grant(grant: &HostGrant) -> Self {
        Self {
            key: grant.key,
            version: grant.version.clone(),
            top_slot: grant.top_slot,
        }
    }

    fn from_confirmation(confirmation: &HostAdmissionConfirmation) -> Self {
        Self {
            key: confirmation.key,
            version: confirmation.version.clone(),
            top_slot: confirmation.top_slot,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdmissionError {
    OutstandingExists,
    AuthorityExists,
    ForeignRouter,
    Stale,
    Exhausted,
}

#[derive(Debug)]
pub(crate) struct HostAdmissionIssuer {
    issuer: AdmissionIssuerId,
    next_sequence: u64,
    next_epoch: u64,
    outstanding: Option<OutstandingAdmission>,
    current: Option<GrantIdentity>,
}

impl HostAdmissionIssuer {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new() -> Result<Self, IdExhaustion> {
        static NEXT_ISSUER: AtomicU64 = AtomicU64::new(1);

        Ok(Self {
            issuer: allocate_issuer(&NEXT_ISSUER)?,
            next_sequence: 1,
            next_epoch: 1,
            outstanding: None,
            current: None,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn issue(
        &mut self,
        router: RouterIdentity,
    ) -> Result<HostAdmissionToken, AdmissionError> {
        if self.current.is_some() {
            return Err(AdmissionError::AuthorityExists);
        }
        if self.outstanding.is_some() {
            return Err(AdmissionError::OutstandingExists);
        }
        let next = self
            .next_sequence
            .checked_add(1)
            .ok_or(AdmissionError::Exhausted)?;
        let token = HostAdmissionToken {
            key: AdmissionKey {
                issuer: self.issuer,
                router,
                sequence: self.next_sequence,
            },
        };
        self.next_sequence = next;
        self.outstanding = Some(OutstandingAdmission::Requested(token.key));
        Ok(token)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn grant(
        &mut self,
        token: &HostAdmissionToken,
        ids: &mut CoreIdAllocator,
    ) -> Result<HostGrant, AdmissionError> {
        if self.outstanding != Some(OutstandingAdmission::Requested(token.key)) {
            return Err(AdmissionError::Stale);
        }
        if !ids.owns_router_identity(token.key.router) {
            return Err(AdmissionError::ForeignRouter);
        }
        let next_epoch = self
            .next_epoch
            .checked_add(1)
            .ok_or(AdmissionError::Exhausted)?;
        let (host, top_slot) = ids
            .allocate_host_slot()
            .map_err(|_| AdmissionError::Exhausted)?;
        let epoch = self.next_epoch;
        let grant = HostGrant {
            key: token.key,
            version: HostVersion {
                router: token.key.router,
                host,
                epoch,
            },
            top_slot,
        };
        self.next_epoch = next_epoch;
        self.outstanding = Some(OutstandingAdmission::Granted(GrantIdentity::from_grant(
            &grant,
        )));
        Ok(grant)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn reject(&mut self, token: &HostAdmissionToken) -> Result<(), AdmissionError> {
        self.finish_requested(token)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cancel(&mut self, token: &HostAdmissionToken) -> Result<(), AdmissionError> {
        self.finish_requested(token)
    }

    fn finish_requested(&mut self, token: &HostAdmissionToken) -> Result<(), AdmissionError> {
        if self.outstanding != Some(OutstandingAdmission::Requested(token.key)) {
            return Err(AdmissionError::Stale);
        }
        self.outstanding = None;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn confirm(
        &mut self,
        grant: &HostGrant,
    ) -> Result<HostAdmissionConfirmation, AdmissionError> {
        let identity = GrantIdentity::from_grant(grant);
        if self.outstanding.as_ref() != Some(&OutstandingAdmission::Granted(identity.clone())) {
            return Err(AdmissionError::Stale);
        }
        if self.current.is_some() {
            return Err(AdmissionError::AuthorityExists);
        }
        let confirmation = HostAdmissionConfirmation {
            key: grant.key,
            version: grant.version.clone(),
            top_slot: grant.top_slot,
        };
        self.outstanding = None;
        self.current = Some(identity);
        Ok(confirmation)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn reject_after_mutation(
        &mut self,
        grant: &HostGrant,
    ) -> Result<(), AdmissionError> {
        let identity = GrantIdentity::from_grant(grant);
        if self.outstanding.as_ref() != Some(&OutstandingAdmission::Granted(identity)) {
            return Err(AdmissionError::Stale);
        }
        self.outstanding = None;
        Ok(())
    }

    pub(crate) fn is_current_confirmation(&self, confirmation: &HostAdmissionConfirmation) -> bool {
        self.current.as_ref() == Some(&GrantIdentity::from_confirmation(confirmation))
    }

    pub(crate) fn is_outstanding_grant(&self, grant: &HostGrant) -> bool {
        self.outstanding.as_ref().is_some_and(|outstanding| {
            outstanding == &OutstandingAdmission::Granted(GrantIdentity::from_grant(grant))
        })
    }

    pub(crate) fn retire_current(
        &mut self,
        lease: HostLease,
    ) -> Result<RetiredHostAuthority, RetireHostLeaseError> {
        if self.outstanding.is_some() || self.current.as_ref() != Some(&lease.identity) {
            return Err(RetireHostLeaseError::new(lease));
        }
        self.current = None;
        Ok(RetiredHostAuthority {
            identity: lease.identity,
        })
    }

    #[cfg(test)]
    pub(crate) fn has_outstanding(&self) -> bool {
        self.outstanding.is_some()
    }

    #[cfg(test)]
    pub(crate) fn has_current(&self) -> bool {
        self.current.is_some()
    }

    #[cfg(test)]
    fn snapshot(&self) -> HostAdmissionIssuerSnapshot {
        HostAdmissionIssuerSnapshot {
            next_sequence: self.next_sequence,
            next_epoch: self.next_epoch,
            outstanding: self.outstanding.clone(),
            current: self.current.clone(),
        }
    }
}

impl HostAdmissionToken {
    #[cfg(test)]
    fn router(&self) -> RouterInstanceId {
        self.key.router.router()
    }
}

impl HostGrant {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn version(&self) -> &HostVersion {
        &self.version
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn top_slot(&self) -> RouteSlotId {
        self.top_slot
    }
}

impl HostAdmissionConfirmation {
    pub(crate) fn version(&self) -> &HostVersion {
        &self.version
    }

    pub(crate) fn top_slot(&self) -> RouteSlotId {
        self.top_slot
    }

    pub(crate) fn into_lease(self) -> HostLease {
        HostLease {
            identity: GrantIdentity::from_confirmation(&self),
        }
    }

    #[cfg(test)]
    fn router(&self) -> RouterInstanceId {
        self.key.router.router()
    }
}

impl HostLease {
    pub(crate) fn matches(
        &self,
        router: RouterIdentity,
        version: &HostVersion,
        top_slot: RouteSlotId,
    ) -> bool {
        self.identity.key.router == router
            && &self.identity.version == version
            && self.identity.top_slot == top_slot
    }
}

impl RetiredHostAuthority {
    pub(crate) fn projection(&self) -> RetiredHostAuthorityProjection {
        RetiredHostAuthorityProjection {
            key: self.identity.key,
            version: self.identity.version.clone(),
            top_slot: self.identity.top_slot,
        }
    }

    pub(crate) fn matches(
        &self,
        router: RouterIdentity,
        version: &HostVersion,
        top_slot: RouteSlotId,
    ) -> bool {
        self.identity.key.router == router
            && &self.identity.version == version
            && self.identity.top_slot == top_slot
    }
}

impl RetiredHostAuthorityProjection {
    #[cfg(test)]
    pub(crate) fn matches(
        &self,
        router: RouterIdentity,
        version: &HostVersion,
        top_slot: RouteSlotId,
    ) -> bool {
        self.key.router == router && &self.version == version && self.top_slot == top_slot
    }
}

fn allocate_issuer(counter: &AtomicU64) -> Result<AdmissionIssuerId, IdExhaustion> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(AdmissionIssuerId)
        .map_err(|_| IdExhaustion)
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct HostAdmissionIssuerSnapshot {
    next_sequence: u64,
    next_epoch: u64,
    outstanding: Option<OutstandingAdmission>,
    current: Option<GrantIdentity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_snapshot_preserves_version_slot_and_ordered_trace() {
        fn assert_traits<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
        assert_traits::<HostSnapshot>();
        assert_traits::<HostTraceEvent>();

        let (mut ids, router, mut issuer) = setup();
        let token = issuer.issue(router).unwrap();
        let grant = issuer.grant(&token, &mut ids).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        let version = confirmation.version().clone();
        let slot = confirmation.top_slot();
        let trace = vec![
            HostTraceEvent::project(HostTraceEventKind::AdmissionRequested, None, None),
            HostTraceEvent::project(
                HostTraceEventKind::HostGranted,
                Some(version.clone()),
                Some(slot),
            ),
            HostTraceEvent::project(
                HostTraceEventKind::HostReady,
                Some(version.clone()),
                Some(slot),
            ),
        ];
        let snapshot = HostSnapshot::project(Some(version.clone()), Some(slot), trace);

        assert_eq!(snapshot.version(), Some(&version));
        assert_eq!(snapshot.top_slot(), Some(slot));
        assert_eq!(
            snapshot
                .trace()
                .iter()
                .map(HostTraceEvent::kind)
                .collect::<Vec<_>>(),
            vec![
                HostTraceEventKind::AdmissionRequested,
                HostTraceEventKind::HostGranted,
                HostTraceEventKind::HostReady,
            ]
        );
        assert_eq!(snapshot.trace()[0].version(), None);
        assert_eq!(snapshot.trace()[0].slot(), None);
        assert_eq!(snapshot.trace()[1].version(), Some(&version));
        assert_eq!(snapshot.trace()[1].slot(), Some(slot));
    }

    fn setup() -> (CoreIdAllocator, RouterIdentity, HostAdmissionIssuer) {
        let mut ids = CoreIdAllocator::new().unwrap();
        let router = ids.allocate_router_identity().unwrap();
        let issuer = HostAdmissionIssuer::new().unwrap();
        (ids, router, issuer)
    }

    #[test]
    fn issuer_allows_only_one_outstanding_admission() {
        let (_ids, router, mut issuer) = setup();
        let token = issuer.issue(router).unwrap();
        let before = issuer.snapshot();

        assert_eq!(issuer.issue(router), Err(AdmissionError::OutstandingExists));
        assert_eq!(issuer.snapshot(), before);
        assert_eq!(token.router(), router.router());
    }

    #[test]
    fn pre_write_reject_and_cancel_allocate_no_host_or_slot_ids() {
        for terminal in [HostAdmissionIssuer::reject, HostAdmissionIssuer::cancel] {
            let (ids, router, mut issuer) = setup();
            let token = issuer.issue(router).unwrap();
            let ids_before = ids.snapshot();

            terminal(&mut issuer, &token).unwrap();

            assert_eq!(ids.snapshot(), ids_before);
            assert!(!issuer.has_outstanding());
            assert_eq!(terminal(&mut issuer, &token), Err(AdmissionError::Stale));
            assert_eq!(ids.snapshot(), ids_before);
        }
    }

    #[test]
    fn grant_is_one_shot_and_confirm_rejects_replay() {
        let (mut ids, router, mut issuer) = setup();
        let token = issuer.issue(router).unwrap();
        let grant = issuer.grant(&token, &mut ids).unwrap();
        let granted = issuer.snapshot();

        assert_eq!(issuer.reject(&token), Err(AdmissionError::Stale));
        assert_eq!(issuer.snapshot(), granted);

        let confirmation = issuer.confirm(&grant).unwrap();
        let confirmed = issuer.snapshot();
        assert_eq!(confirmation.router(), router.router());
        assert_eq!(issuer.confirm(&grant), Err(AdmissionError::Stale));
        assert_eq!(issuer.snapshot(), confirmed);
    }

    #[test]
    fn confirmed_authority_blocks_new_issue_without_state_change() {
        let (mut ids, router, mut issuer) = setup();
        let token = issuer.issue(router).unwrap();
        let grant = issuer.grant(&token, &mut ids).unwrap();
        issuer.confirm(&grant).unwrap();
        let confirmed = issuer.snapshot();

        assert_eq!(issuer.issue(router), Err(AdmissionError::AuthorityExists));
        assert_eq!(issuer.snapshot(), confirmed);
    }

    #[test]
    fn exact_host_lease_retires_once_and_foreign_issuer_returns_the_owner() {
        let (mut ids, router, mut issuer) = setup();
        let token = issuer.issue(router).unwrap();
        let grant = issuer.grant(&token, &mut ids).unwrap();
        let confirmation = issuer.confirm(&grant).unwrap();
        let version = confirmation.version().clone();
        let top_slot = confirmation.top_slot();
        let lease = confirmation.into_lease();

        let (_foreign_ids, _foreign_router, mut foreign) = setup();
        let error = foreign.retire_current(lease).unwrap_err();
        let retired = issuer.retire_current(error.into_owner()).unwrap();

        assert!(retired.matches(router, &version, top_slot));
        assert!(issuer.snapshot().current.is_none());
        assert!(issuer.issue(router).is_ok());
    }

    #[test]
    fn host_versions_from_different_worlds_are_not_equal_when_raw_ids_match() {
        let (mut first_ids, first_router, mut first_issuer) = setup();
        let first_token = first_issuer.issue(first_router).unwrap();
        let first_grant = first_issuer.grant(&first_token, &mut first_ids).unwrap();
        let first = first_issuer.confirm(&first_grant).unwrap();

        let (mut second_ids, second_router, mut second_issuer) = setup();
        let second_token = second_issuer.issue(second_router).unwrap();
        let second_grant = second_issuer.grant(&second_token, &mut second_ids).unwrap();
        let second = second_issuer.confirm(&second_grant).unwrap();

        assert_eq!(first.version().router(), second.version().router());
        assert_eq!(first.version().host(), second.version().host());
        assert_eq!(first.version().epoch(), second.version().epoch());
        assert_ne!(first.version(), second.version());
    }

    #[test]
    fn rejection_after_mutation_burns_granted_host_and_slot_ids() {
        let (mut ids, router, mut issuer) = setup();
        let first_token = issuer.issue(router).unwrap();
        let first_grant = issuer.grant(&first_token, &mut ids).unwrap();
        let first_host = first_grant.version().host;
        let first_slot = first_grant.top_slot();

        issuer.reject_after_mutation(&first_grant).unwrap();
        assert_eq!(
            issuer.reject_after_mutation(&first_grant),
            Err(AdmissionError::Stale)
        );

        let second_token = issuer.issue(router).unwrap();
        let second_grant = issuer.grant(&second_token, &mut ids).unwrap();
        assert_ne!(second_grant.version().host, first_host);
        assert_ne!(second_grant.top_slot(), first_slot);
    }

    #[test]
    fn foreign_and_out_of_order_values_are_zero_change() {
        let (mut ids, router, mut issuer) = setup();
        let (_foreign_ids, _foreign_router, mut foreign) = setup();
        let token = issuer.issue(router).unwrap();
        let issuer_before = issuer.snapshot();
        let foreign_before = foreign.snapshot();

        assert_eq!(foreign.reject(&token), Err(AdmissionError::Stale));
        assert_eq!(issuer.snapshot(), issuer_before);
        assert_eq!(foreign.snapshot(), foreign_before);

        let grant = issuer.grant(&token, &mut ids).unwrap();
        let granted = issuer.snapshot();
        assert_eq!(foreign.confirm(&grant), Err(AdmissionError::Stale));
        assert_eq!(issuer.snapshot(), granted);
        assert_eq!(foreign.snapshot(), foreign_before);
    }

    #[test]
    fn foreign_world_allocator_cannot_grant_an_outstanding_token() {
        let (_ids, router, mut issuer) = setup();
        let token = issuer.issue(router).unwrap();
        let issuer_before = issuer.snapshot();

        let mut foreign_ids = CoreIdAllocator::new().unwrap();
        foreign_ids.allocate_router_identity().unwrap();
        let ids_before = foreign_ids.snapshot();

        assert_eq!(
            issuer.grant(&token, &mut foreign_ids),
            Err(AdmissionError::ForeignRouter)
        );
        assert_eq!(issuer.snapshot(), issuer_before);
        assert_eq!(foreign_ids.snapshot(), ids_before);
    }

    #[test]
    fn issuer_sequence_and_epoch_exhaustion_are_checked_and_zero_change() {
        let (mut ids, router, mut issuer) = setup();
        issuer.next_sequence = u64::MAX;
        let before = issuer.snapshot();
        assert_eq!(issuer.issue(router), Err(AdmissionError::Exhausted));
        assert_eq!(issuer.snapshot(), before);

        issuer.next_sequence = 1;
        let token = issuer.issue(router).unwrap();
        issuer.next_epoch = u64::MAX;
        let issuer_before = issuer.snapshot();
        let ids_before = ids.snapshot();
        assert_eq!(
            issuer.grant(&token, &mut ids),
            Err(AdmissionError::Exhausted)
        );
        assert_eq!(issuer.snapshot(), issuer_before);
        assert_eq!(ids.snapshot(), ids_before);

        let issuer_counter = AtomicU64::new(u64::MAX);
        assert_eq!(allocate_issuer(&issuer_counter), Err(IdExhaustion));
        assert_eq!(issuer_counter.load(Ordering::Relaxed), u64::MAX);
    }
}
