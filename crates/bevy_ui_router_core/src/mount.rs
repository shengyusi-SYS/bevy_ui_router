#![cfg_attr(not(test), allow(dead_code))]

use crate::HostVersion;
use crate::frame::{FrameOrigin, RouteFrameVersion};
use crate::ids::{
    CoreIdAllocator, NavigationRequestId, RouteSlotId, RouterIdentity, TransactionId,
};
use crate::slot::RouteSlotKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountRequestToken {
    identity: MountIdentity,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountGrant {
    identity: MountIdentity,
    slot: RouteSlotId,
    binding: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MountReceiptDigest(u64);

impl MountReceiptDigest {
    pub(crate) const fn from_opaque(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MountRegistrationDigest(u64);

impl MountRegistrationDigest {
    pub(crate) const fn from_opaque(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountReceiptAttestation {
    identity: MountIdentity,
    slot: RouteSlotId,
    digest: MountReceiptDigest,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountRegistrationPermit {
    identity: MountIdentity,
    slot: RouteSlotId,
    receipt_digest: MountReceiptDigest,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountRegistrationCompletion {
    identity: MountIdentity,
    slot: RouteSlotId,
    receipt_digest: MountReceiptDigest,
    registration_digest: MountRegistrationDigest,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountConfirmation {
    identity: MountIdentity,
    slot: RouteSlotId,
    receipt_digest: MountReceiptDigest,
    registration_digest: MountRegistrationDigest,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MountIdentity {
    router: RouterIdentity,
    host: HostVersion,
    request: NavigationRequestId,
    transaction: TransactionId,
    sequence: u64,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutstandingMount {
    Requested(MountIdentity),
    Granted {
        identity: MountIdentity,
        slot: RouteSlotId,
    },
    Confirming {
        identity: MountIdentity,
        slot: RouteSlotId,
        receipt_digest: MountReceiptDigest,
    },
}

impl OutstandingMount {
    fn identity(&self) -> &MountIdentity {
        match self {
            Self::Requested(identity)
            | Self::Granted { identity, .. }
            | Self::Confirming { identity, .. } => identity,
        }
    }
}

#[derive(Debug)]
pub(crate) struct MountIssuer {
    router: RouterIdentity,
    outstanding: Option<OutstandingMount>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MountError {
    Foreign,
    Stale,
    WrongPhase,
    Exhausted,
}

impl MountIssuer {
    pub(crate) const fn new(router: RouterIdentity) -> Self {
        Self {
            router,
            outstanding: None,
        }
    }

    pub(crate) fn issue(
        &mut self,
        ids: &mut CoreIdAllocator,
        host: HostVersion,
        request: NavigationRequestId,
        transaction: TransactionId,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    ) -> Result<MountRequestToken, MountError> {
        if self.outstanding.is_some() {
            return Err(MountError::WrongPhase);
        }
        if !ids.owns_router_identity(self.router)
            || host.router_identity() != self.router
            || request.router_identity() != self.router
            || parent.router_identity() != self.router
            || parent.host() != host.host()
        {
            return Err(MountError::Foreign);
        }

        let sequence = ids
            .allocate_mount_sequence()
            .map_err(|_| MountError::Exhausted)?;
        let identity = MountIdentity {
            router: self.router,
            host,
            request,
            transaction,
            sequence,
            parent,
            outlet,
        };
        let token = MountRequestToken {
            binding: artifact_binding(MountArtifactStage::Requested, &identity, None, None, None),
            identity: identity.clone(),
        };
        self.outstanding = Some(OutstandingMount::Requested(identity));
        Ok(token)
    }

    pub(crate) fn grant(
        &mut self,
        token: &MountRequestToken,
        ids: &mut CoreIdAllocator,
    ) -> Result<MountGrant, MountError> {
        if !token.has_valid_binding() || !identity_is_coherent(&token.identity) {
            return Err(MountError::Foreign);
        }
        self.validate_requested(&token.identity)?;
        if !ids.owns_router_identity(token.identity.router) {
            return Err(MountError::Foreign);
        }

        let slot = ids.allocate_slot().map_err(|_| MountError::Exhausted)?;
        let grant = MountGrant {
            identity: token.identity.clone(),
            slot,
            binding: artifact_binding(
                MountArtifactStage::Granted,
                &token.identity,
                Some(slot),
                None,
                None,
            ),
        };
        self.outstanding = Some(OutstandingMount::Granted {
            identity: token.identity.clone(),
            slot,
        });
        Ok(grant)
    }

    pub(crate) fn reject(&mut self, token: &MountRequestToken) -> Result<(), MountError> {
        self.finish_requested(token)
    }

    pub(crate) fn cancel(&mut self, token: &MountRequestToken) -> Result<(), MountError> {
        self.finish_requested(token)
    }

    fn finish_requested(&mut self, token: &MountRequestToken) -> Result<(), MountError> {
        if !token.has_valid_binding() || !identity_is_coherent(&token.identity) {
            return Err(MountError::Foreign);
        }
        self.validate_requested(&token.identity)?;
        self.outstanding = None;
        Ok(())
    }

    pub(crate) fn accept_receipt(
        &mut self,
        receipt: &MountReceiptAttestation,
    ) -> Result<MountRegistrationPermit, MountError> {
        if !receipt.has_valid_binding() || !identity_is_coherent(&receipt.identity) {
            return Err(MountError::Foreign);
        }
        self.validate_granted(&receipt.identity, receipt.slot)?;

        let permit = MountRegistrationPermit {
            identity: receipt.identity.clone(),
            slot: receipt.slot,
            receipt_digest: receipt.digest,
            binding: artifact_binding(
                MountArtifactStage::RegistrationPermit,
                &receipt.identity,
                Some(receipt.slot),
                Some(receipt.digest),
                None,
            ),
        };
        self.outstanding = Some(OutstandingMount::Confirming {
            identity: receipt.identity.clone(),
            slot: receipt.slot,
            receipt_digest: receipt.digest,
        });
        Ok(permit)
    }

    pub(crate) fn complete_registration(
        &mut self,
        completion: &MountRegistrationCompletion,
    ) -> Result<MountConfirmation, MountError> {
        if !completion.has_valid_binding() || !identity_is_coherent(&completion.identity) {
            return Err(MountError::Foreign);
        }
        self.validate_confirming(
            &completion.identity,
            completion.slot,
            completion.receipt_digest,
        )?;

        let confirmation = MountConfirmation {
            identity: completion.identity.clone(),
            slot: completion.slot,
            receipt_digest: completion.receipt_digest,
            registration_digest: completion.registration_digest,
            binding: artifact_binding(
                MountArtifactStage::Confirmed,
                &completion.identity,
                Some(completion.slot),
                Some(completion.receipt_digest),
                Some(completion.registration_digest),
            ),
        };
        self.outstanding = None;
        Ok(confirmation)
    }

    pub(crate) fn reject_after_mutation<A: MountMutationArtifact + ?Sized>(
        &mut self,
        artifact: &A,
    ) -> Result<(), MountError> {
        if !artifact.has_valid_binding() || !identity_is_coherent(artifact.identity()) {
            return Err(MountError::Foreign);
        }
        match artifact.phase() {
            MountMutationPhase::Granted => {
                self.validate_granted(artifact.identity(), artifact.slot())?;
            }
            MountMutationPhase::Confirming(receipt_digest) => {
                self.validate_confirming(artifact.identity(), artifact.slot(), receipt_digest)?;
            }
        }
        self.outstanding = None;
        Ok(())
    }

    fn validate_requested(&self, identity: &MountIdentity) -> Result<(), MountError> {
        match self.outstanding.as_ref() {
            Some(OutstandingMount::Requested(expected)) if expected == identity => Ok(()),
            _ => Err(self.mismatch(identity)),
        }
    }

    fn validate_granted(
        &self,
        identity: &MountIdentity,
        slot: RouteSlotId,
    ) -> Result<(), MountError> {
        match self.outstanding.as_ref() {
            Some(OutstandingMount::Granted {
                identity: expected,
                slot: expected_slot,
            }) if expected == identity && *expected_slot == slot => Ok(()),
            Some(outstanding) if outstanding.identity() == identity => Err(MountError::Stale),
            _ => Err(self.mismatch(identity)),
        }
    }

    fn validate_confirming(
        &self,
        identity: &MountIdentity,
        slot: RouteSlotId,
        receipt_digest: MountReceiptDigest,
    ) -> Result<(), MountError> {
        match self.outstanding.as_ref() {
            Some(OutstandingMount::Confirming {
                identity: expected,
                slot: expected_slot,
                receipt_digest: expected_digest,
            }) if expected == identity
                && *expected_slot == slot
                && *expected_digest == receipt_digest =>
            {
                Ok(())
            }
            Some(outstanding) if outstanding.identity() == identity => Err(MountError::Stale),
            _ => Err(self.mismatch(identity)),
        }
    }

    fn mismatch(&self, identity: &MountIdentity) -> MountError {
        if identity.router != self.router {
            return MountError::Foreign;
        }
        match self.outstanding.as_ref() {
            None => MountError::Stale,
            Some(outstanding) if outstanding.identity() == identity => MountError::Stale,
            Some(_) => MountError::Foreign,
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> MountIssuerSnapshot {
        MountIssuerSnapshot {
            router: self.router,
            outstanding: self.outstanding.clone(),
        }
    }
}

impl MountRequestToken {
    fn has_valid_binding(&self) -> bool {
        self.binding
            == artifact_binding(
                MountArtifactStage::Requested,
                &self.identity,
                None,
                None,
                None,
            )
    }
}

impl MountGrant {
    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) fn attest_receipt(self, digest: MountReceiptDigest) -> MountReceiptAttestation {
        MountReceiptAttestation {
            binding: artifact_binding(
                MountArtifactStage::Receipt,
                &self.identity,
                Some(self.slot),
                Some(digest),
                None,
            ),
            identity: self.identity,
            slot: self.slot,
            digest,
        }
    }

    fn has_valid_binding(&self) -> bool {
        self.binding
            == artifact_binding(
                MountArtifactStage::Granted,
                &self.identity,
                Some(self.slot),
                None,
                None,
            )
    }
}

impl MountReceiptAttestation {
    fn has_valid_binding(&self) -> bool {
        self.binding
            == artifact_binding(
                MountArtifactStage::Receipt,
                &self.identity,
                Some(self.slot),
                Some(self.digest),
                None,
            )
    }
}

impl MountRegistrationPermit {
    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) fn complete(
        self,
        registration_digest: MountRegistrationDigest,
    ) -> MountRegistrationCompletion {
        MountRegistrationCompletion {
            binding: artifact_binding(
                MountArtifactStage::RegistrationCompletion,
                &self.identity,
                Some(self.slot),
                Some(self.receipt_digest),
                Some(registration_digest),
            ),
            identity: self.identity,
            slot: self.slot,
            receipt_digest: self.receipt_digest,
            registration_digest,
        }
    }

    fn has_valid_binding(&self) -> bool {
        self.binding
            == artifact_binding(
                MountArtifactStage::RegistrationPermit,
                &self.identity,
                Some(self.slot),
                Some(self.receipt_digest),
                None,
            )
    }
}

impl MountRegistrationCompletion {
    fn has_valid_binding(&self) -> bool {
        self.binding
            == artifact_binding(
                MountArtifactStage::RegistrationCompletion,
                &self.identity,
                Some(self.slot),
                Some(self.receipt_digest),
                Some(self.registration_digest),
            )
    }
}

impl MountConfirmation {
    pub(crate) const fn slot(&self) -> RouteSlotId {
        self.slot
    }

    pub(crate) fn has_valid_binding(&self) -> bool {
        self.binding
            == artifact_binding(
                MountArtifactStage::Confirmed,
                &self.identity,
                Some(self.slot),
                Some(self.receipt_digest),
                Some(self.registration_digest),
            )
    }
}

pub(crate) trait MountMutationArtifact {
    fn identity(&self) -> &MountIdentity;
    fn slot(&self) -> RouteSlotId;
    fn phase(&self) -> MountMutationPhase;
    fn has_valid_binding(&self) -> bool;
}

impl MountMutationArtifact for MountGrant {
    fn identity(&self) -> &MountIdentity {
        &self.identity
    }

    fn slot(&self) -> RouteSlotId {
        self.slot
    }

    fn phase(&self) -> MountMutationPhase {
        MountMutationPhase::Granted
    }

    fn has_valid_binding(&self) -> bool {
        MountGrant::has_valid_binding(self)
    }
}

impl MountMutationArtifact for MountRegistrationPermit {
    fn identity(&self) -> &MountIdentity {
        &self.identity
    }

    fn slot(&self) -> RouteSlotId {
        self.slot
    }

    fn phase(&self) -> MountMutationPhase {
        MountMutationPhase::Confirming(self.receipt_digest)
    }

    fn has_valid_binding(&self) -> bool {
        MountRegistrationPermit::has_valid_binding(self)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum MountMutationPhase {
    Granted,
    Confirming(MountReceiptDigest),
}

#[derive(Clone, Copy)]
enum MountArtifactStage {
    Requested = 1,
    Granted = 2,
    Receipt = 3,
    RegistrationPermit = 4,
    RegistrationCompletion = 5,
    Confirmed = 6,
}

fn identity_is_coherent(identity: &MountIdentity) -> bool {
    identity.host.router_identity() == identity.router
        && identity.request.router_identity() == identity.router
        && identity.parent.router_identity() == identity.router
        && identity.parent.host() == identity.host.host()
}

fn artifact_binding(
    stage: MountArtifactStage,
    identity: &MountIdentity,
    slot: Option<RouteSlotId>,
    receipt_digest: Option<MountReceiptDigest>,
    registration_digest: Option<MountRegistrationDigest>,
) -> u64 {
    let mut digest = StructuredDigest::new(0x6d6f_756e_745f_7631);
    digest.push(stage as u64);
    digest.mount_identity(identity);
    digest.optional(slot.map(RouteSlotId::raw));
    digest.optional(receipt_digest.map(|value| value.0));
    digest.optional(registration_digest.map(|value| value.0));
    digest.finish()
}

struct StructuredDigest(u64);

impl StructuredDigest {
    const fn new(domain: u64) -> Self {
        Self(0xcbf2_9ce4_8422_2325 ^ domain)
    }

    fn push(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn optional(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.push(1);
                self.push(value);
            }
            None => self.push(0),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.push(value.len() as u64);
        for byte in value {
            self.push(u64::from(*byte));
        }
    }

    fn mount_identity(&mut self, identity: &MountIdentity) {
        let (domain, router) = identity.router.provenance();
        self.push(domain);
        self.push(router);

        let (host_domain, host_router) = identity.host.router_identity().provenance();
        self.push(host_domain);
        self.push(host_router);
        self.push(identity.host.host().raw());
        self.push(identity.host.epoch());

        let (request_domain, request_router) = identity.request.router_identity().provenance();
        self.push(request_domain);
        self.push(request_router);
        self.push(identity.request.sequence());
        self.push(identity.transaction.raw());
        self.push(identity.sequence);
        self.frame(&identity.parent);
        self.bytes(identity.outlet.as_str().as_bytes());
    }

    fn frame(&mut self, frame: &RouteFrameVersion) {
        let (domain, router) = frame.router_identity().provenance();
        self.push(domain);
        self.push(router);
        self.push(frame.host().raw());
        self.push(frame.slot().raw());
        match frame.origin() {
            FrameOrigin::MatchedRoute(route) => {
                self.push(1);
                self.push(route.raw());
            }
            FrameOrigin::RouteError(route) => {
                self.push(2);
                self.push(route.raw());
            }
            FrameOrigin::RouterFallback => self.push(3),
        }
        self.push(frame.frame().raw());
        self.push(frame.generation().raw());
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct MountIssuerSnapshot {
    router: RouterIdentity,
    outstanding: Option<OutstandingMount>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HostVersion;
    use crate::admission::HostAdmissionIssuer;
    use crate::frame::{FrameOrigin, RouteFrameVersion};
    use crate::ids::{
        CoreIdAllocator, NavigationRequestId, RouteGeneration, RouterIdentity, TransactionId,
    };
    use crate::slot::RouteSlotKey;

    struct Fixture {
        ids: CoreIdAllocator,
        router: RouterIdentity,
        issuer: MountIssuer,
        host: HostVersion,
        request: NavigationRequestId,
        transaction: TransactionId,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
    }

    impl Fixture {
        fn new() -> Self {
            let mut ids = CoreIdAllocator::new().unwrap();
            let router = ids.allocate_router_identity().unwrap();
            let mut admission = HostAdmissionIssuer::new().unwrap();
            let admission_token = admission.issue(router).unwrap();
            let host_grant = admission.grant(&admission_token, &mut ids).unwrap();
            let host_confirmation = admission.confirm(&host_grant).unwrap();
            let host = host_confirmation.version().clone();
            let request = ids.allocate_request(router).unwrap();
            let transaction = ids.allocate_transaction().unwrap();
            let parent = RouteFrameVersion::new(
                router,
                host.host(),
                host_confirmation.top_slot(),
                FrameOrigin::RouterFallback,
                ids.allocate_frame().unwrap(),
                RouteGeneration::initial(),
            );

            Self {
                ids,
                router,
                issuer: MountIssuer::new(router),
                host,
                request,
                transaction,
                parent,
                outlet: RouteSlotKey::named("details").unwrap(),
            }
        }

        fn issue(&mut self) -> MountRequestToken {
            self.issuer
                .issue(
                    &mut self.ids,
                    self.host.clone(),
                    self.request,
                    self.transaction,
                    self.parent.clone(),
                    self.outlet.clone(),
                )
                .unwrap()
        }

        fn grant(&mut self) -> MountGrant {
            let request = self.issue();
            self.issuer.grant(&request, &mut self.ids).unwrap()
        }

        fn permit(&mut self, digest: u64) -> MountRegistrationPermit {
            let grant = self.grant();
            let receipt = grant.attest_receipt(MountReceiptDigest::from_opaque(digest));
            self.issuer.accept_receipt(&receipt).unwrap()
        }
    }

    fn assert_requested_replay_is_stale(
        terminal: fn(&mut MountIssuer, &MountRequestToken) -> Result<(), MountError>,
    ) {
        let mut fixture = Fixture::new();
        let request = fixture.issue();
        let replay = request.clone();
        terminal(&mut fixture.issuer, &request).unwrap();
        let issuer_after = fixture.issuer.snapshot();
        let ids_after = fixture.ids.snapshot();

        assert_eq!(
            terminal(&mut fixture.issuer, &replay),
            Err(MountError::Stale)
        );
        assert_eq!(fixture.issuer.snapshot(), issuer_after);
        assert_eq!(fixture.ids.snapshot(), ids_after);
    }

    fn reseal_request(token: &mut MountRequestToken) {
        token.binding = artifact_binding(
            MountArtifactStage::Requested,
            &token.identity,
            None,
            None,
            None,
        );
    }

    fn assert_foreign_request_is_zero_change(fixture: &mut Fixture, request: &MountRequestToken) {
        let issuer_before = fixture.issuer.snapshot();
        let ids_before = fixture.ids.snapshot();
        assert_eq!(
            fixture.issuer.grant(request, &mut fixture.ids),
            Err(MountError::Foreign)
        );
        assert_eq!(fixture.issuer.snapshot(), issuer_before);
        assert_eq!(fixture.ids.snapshot(), ids_before);
    }

    #[test]
    fn mount_request_has_no_slot_until_exact_resolution() {
        let mut fixture = Fixture::new();
        let before = fixture.ids.snapshot();

        let request = fixture.issue();
        let requested = fixture.ids.snapshot();
        assert_eq!(requested.next_slot(), before.next_slot());
        assert_eq!(
            requested.next_mount_sequence(),
            before.next_mount_sequence() + 1
        );

        let grant = fixture.issuer.grant(&request, &mut fixture.ids).unwrap();
        let granted = fixture.ids.snapshot();
        assert_eq!(granted.next_slot(), requested.next_slot() + 1);
        assert_eq!(grant.slot().raw(), requested.next_slot());
    }

    #[test]
    fn mount_request_reject_cancel_accept_receipt_complete_registration_and_replay_are_single_use()
    {
        assert_requested_replay_is_stale(MountIssuer::reject);
        assert_requested_replay_is_stale(MountIssuer::cancel);

        let mut fixture = Fixture::new();
        let request = fixture.issue();
        let request_replay = request.clone();
        let grant = fixture.issuer.grant(&request, &mut fixture.ids).unwrap();
        let granted = fixture.issuer.snapshot();
        let ids_after_grant = fixture.ids.snapshot();
        assert_eq!(
            fixture.issuer.grant(&request_replay, &mut fixture.ids),
            Err(MountError::Stale)
        );
        assert_eq!(fixture.issuer.snapshot(), granted);
        assert_eq!(fixture.ids.snapshot(), ids_after_grant);

        let receipt = grant
            .clone()
            .attest_receipt(MountReceiptDigest::from_opaque(11));
        let receipt_replay = receipt.clone();
        let permit = fixture.issuer.accept_receipt(&receipt).unwrap();
        let confirming = fixture.issuer.snapshot();
        assert_eq!(
            fixture.issuer.accept_receipt(&receipt_replay),
            Err(MountError::Stale)
        );
        assert_eq!(fixture.issuer.snapshot(), confirming);
        assert_eq!(fixture.ids.snapshot(), ids_after_grant);

        let permit_replay = permit.clone();
        let completion = permit.complete(MountRegistrationDigest::from_opaque(17));
        let completion_replay = completion.clone();
        let confirmation = fixture.issuer.complete_registration(&completion).unwrap();
        let completed = fixture.issuer.snapshot();
        assert_eq!(confirmation.slot(), grant.slot());
        assert_eq!(
            fixture.issuer.complete_registration(&completion_replay),
            Err(MountError::Stale)
        );
        let second_completion = permit_replay.complete(MountRegistrationDigest::from_opaque(17));
        assert_eq!(
            fixture.issuer.complete_registration(&second_completion),
            Err(MountError::Stale)
        );
        assert_eq!(fixture.issuer.snapshot(), completed);
        assert_eq!(fixture.ids.snapshot(), ids_after_grant);

        let mut rejected_grant = Fixture::new();
        let grant = rejected_grant.grant();
        let replay = grant.clone();
        rejected_grant.issuer.reject_after_mutation(&grant).unwrap();
        let rejected = rejected_grant.issuer.snapshot();
        let rejected_ids = rejected_grant.ids.snapshot();
        assert_eq!(
            rejected_grant.issuer.reject_after_mutation(&replay),
            Err(MountError::Stale)
        );
        assert_eq!(rejected_grant.issuer.snapshot(), rejected);
        assert_eq!(rejected_grant.ids.snapshot(), rejected_ids);

        let mut rejected_permit = Fixture::new();
        let permit = rejected_permit.permit(23);
        let replay = permit.clone();
        rejected_permit
            .issuer
            .reject_after_mutation(&permit)
            .unwrap();
        let rejected = rejected_permit.issuer.snapshot();
        let rejected_ids = rejected_permit.ids.snapshot();
        assert_eq!(
            rejected_permit.issuer.reject_after_mutation(&replay),
            Err(MountError::Stale)
        );
        assert_eq!(rejected_permit.issuer.snapshot(), rejected);
        assert_eq!(rejected_permit.ids.snapshot(), rejected_ids);
    }

    #[test]
    fn mount_grant_rejection_burns_slot_id() {
        let mut fixture = Fixture::new();
        let first = fixture.grant();
        let first_slot = first.slot();
        fixture.issuer.reject_after_mutation(&first).unwrap();

        let second = fixture.grant();
        assert_eq!(second.slot().raw(), first_slot.raw() + 1);
        assert_ne!(second.slot(), first_slot);
    }

    #[test]
    fn mount_identity_rejects_foreign_router_host_request_transaction_sequence_parent_outlet_or_stage()
     {
        let mut fixture = Fixture::new();
        let request = fixture.issue();
        let mut foreign = Fixture::new();

        let mut altered = request.clone();
        altered.identity.router = foreign.router;
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.identity.host = foreign.host.clone();
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.identity.request = foreign.request;
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.identity.transaction = foreign.ids.allocate_transaction().unwrap();
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.identity.sequence += 1;
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.identity.parent = foreign.parent.clone();
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.identity.outlet = RouteSlotKey::named("foreign").unwrap();
        reseal_request(&mut altered);
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        let mut altered = request.clone();
        altered.binding = artifact_binding(
            MountArtifactStage::Granted,
            &altered.identity,
            None,
            None,
            None,
        );
        assert_foreign_request_is_zero_change(&mut fixture, &altered);

        fixture.issuer.grant(&request, &mut fixture.ids).unwrap();
    }

    #[test]
    fn mount_receipt_and_registration_reject_wrong_digest_foreign_permit_and_duplicate_or_stale_completion()
     {
        let mut fixture = Fixture::new();
        let grant = fixture.grant();
        let mut wrong_receipt = grant
            .clone()
            .attest_receipt(MountReceiptDigest::from_opaque(29));
        wrong_receipt.digest = MountReceiptDigest::from_opaque(31);
        let issuer_before = fixture.issuer.snapshot();
        assert_eq!(
            fixture.issuer.accept_receipt(&wrong_receipt),
            Err(MountError::Foreign)
        );
        assert_eq!(fixture.issuer.snapshot(), issuer_before);

        let receipt = grant.attest_receipt(MountReceiptDigest::from_opaque(29));
        let permit = fixture.issuer.accept_receipt(&receipt).unwrap();
        let mut wrong_registration = permit
            .clone()
            .complete(MountRegistrationDigest::from_opaque(37));
        wrong_registration.registration_digest = MountRegistrationDigest::from_opaque(41);
        let confirming = fixture.issuer.snapshot();
        assert_eq!(
            fixture.issuer.complete_registration(&wrong_registration),
            Err(MountError::Foreign)
        );
        assert_eq!(fixture.issuer.snapshot(), confirming);

        let mut foreign = Fixture::new();
        let foreign_permit = foreign.permit(43);
        let stale_permit = foreign_permit.clone();
        foreign
            .issuer
            .reject_after_mutation(&foreign_permit)
            .unwrap();
        let foreign_completion = stale_permit.complete(MountRegistrationDigest::from_opaque(47));
        assert_eq!(
            foreign.issuer.complete_registration(&foreign_completion),
            Err(MountError::Stale)
        );
        assert_eq!(
            fixture.issuer.complete_registration(&foreign_completion),
            Err(MountError::Foreign)
        );
        assert_eq!(fixture.issuer.snapshot(), confirming);

        let completion = permit.complete(MountRegistrationDigest::from_opaque(37));
        let replay = completion.clone();
        fixture.issuer.complete_registration(&completion).unwrap();
        let completed = fixture.issuer.snapshot();
        assert_eq!(
            fixture.issuer.complete_registration(&replay),
            Err(MountError::Stale)
        );
        assert_eq!(fixture.issuer.snapshot(), completed);
    }

    #[test]
    fn mount_registration_permit_cannot_issue_child_before_exact_completion() {
        fn child_slot(confirmation: &MountConfirmation) -> crate::ids::RouteSlotId {
            confirmation.slot()
        }

        let mut fixture = Fixture::new();
        let permit = fixture.permit(53);
        assert!(matches!(
            fixture.issuer.outstanding,
            Some(OutstandingMount::Confirming { .. })
        ));

        let mut malformed = permit
            .clone()
            .complete(MountRegistrationDigest::from_opaque(59));
        malformed.binding ^= 1;
        let confirming = fixture.issuer.snapshot();
        assert_eq!(
            fixture.issuer.complete_registration(&malformed),
            Err(MountError::Foreign)
        );
        assert_eq!(fixture.issuer.snapshot(), confirming);

        let expected_slot = permit.slot();
        let completion = permit.complete(MountRegistrationDigest::from_opaque(59));
        let confirmation = fixture.issuer.complete_registration(&completion).unwrap();
        assert!(confirmation.has_valid_binding());
        assert_eq!(child_slot(&confirmation), expected_slot);
        assert!(fixture.issuer.outstanding.is_none());
    }

    #[test]
    fn foreign_router_artifacts_stay_foreign_for_idle_and_terminal_issuer() {
        let mut receiver = Fixture::new();
        let mut foreign = Fixture::new();
        let foreign_request = foreign.issue();
        let foreign_grant = foreign
            .issuer
            .grant(&foreign_request, &mut foreign.ids)
            .unwrap();
        let foreign_receipt = foreign_grant
            .clone()
            .attest_receipt(MountReceiptDigest::from_opaque(61));
        let foreign_permit = foreign.issuer.accept_receipt(&foreign_receipt).unwrap();
        let foreign_completion = foreign_permit.complete(MountRegistrationDigest::from_opaque(67));

        let idle_issuer = receiver.issuer.snapshot();
        let idle_ids = receiver.ids.snapshot();
        assert_eq!(
            receiver.issuer.grant(&foreign_request, &mut receiver.ids),
            Err(MountError::Foreign)
        );
        assert_eq!(receiver.issuer.snapshot(), idle_issuer);
        assert_eq!(receiver.ids.snapshot(), idle_ids);
        assert_eq!(
            receiver.issuer.reject_after_mutation(&foreign_grant),
            Err(MountError::Foreign)
        );
        assert_eq!(receiver.issuer.snapshot(), idle_issuer);
        assert_eq!(receiver.ids.snapshot(), idle_ids);

        let receiver_permit = receiver.permit(71);
        let receiver_completion =
            receiver_permit.complete(MountRegistrationDigest::from_opaque(73));
        receiver
            .issuer
            .complete_registration(&receiver_completion)
            .unwrap();
        let terminal_issuer = receiver.issuer.snapshot();
        let terminal_ids = receiver.ids.snapshot();

        assert_eq!(
            receiver.issuer.accept_receipt(&foreign_receipt),
            Err(MountError::Foreign)
        );
        assert_eq!(receiver.issuer.snapshot(), terminal_issuer);
        assert_eq!(receiver.ids.snapshot(), terminal_ids);
        assert_eq!(
            receiver.issuer.complete_registration(&foreign_completion),
            Err(MountError::Foreign)
        );
        assert_eq!(receiver.issuer.snapshot(), terminal_issuer);
        assert_eq!(receiver.ids.snapshot(), terminal_ids);
    }
}
