#![allow(dead_code)]

use crate::engine::RootTaskIdentity;
use crate::frame::{FrameOrigin, RouteFrameVersion};
use crate::ids::CleanupBatchId;
#[cfg(test)]
use crate::ids::RouteFrameId;
use crate::trace::CleanupDisposition;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchToken {
    identity: RootTaskIdentity,
    batch: CleanupBatchId,
    manifest: Vec<RouteFrameVersion>,
    manifest_digest: u64,
}

impl CleanupBatchToken {
    pub(crate) fn sealed(
        identity: RootTaskIdentity,
        batch: CleanupBatchId,
        manifest: Vec<RouteFrameVersion>,
    ) -> Self {
        let manifest_digest = cleanup_digest(&identity, batch, &manifest);
        Self {
            identity,
            batch,
            manifest,
            manifest_digest,
        }
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.manifest.len()
    }
    pub(crate) fn obligations(&self) -> &[RouteFrameVersion] {
        &self.manifest
    }

    pub(crate) fn zero_completion(&self) -> Result<CleanupCompletion, CleanupLedgerError> {
        if !self.manifest.is_empty() {
            return Err(CleanupLedgerError::MissingReceipt);
        }
        Ok(CleanupCompletion::new(self.clone(), Vec::new()))
    }

    pub(crate) fn receipt(
        &self,
        index: usize,
        disposition: CleanupDisposition,
    ) -> Result<CleanupReceipt, CleanupLedgerError> {
        let obligation = self
            .manifest
            .get(index)
            .cloned()
            .ok_or(CleanupLedgerError::UnknownObligation)?;
        Ok(CleanupReceipt {
            batch: self.batch,
            obligation,
            disposition,
        })
    }

    pub(crate) fn completion_from_receipts(
        &self,
        receipts: Vec<CleanupReceipt>,
    ) -> Result<CleanupCompletion, CleanupLedgerError> {
        validate_receipts(self, &receipts)?;
        Ok(CleanupCompletion::new(self.clone(), receipts))
    }

    #[cfg(test)]
    pub(crate) fn testing_removed_completion(
        &self,
    ) -> Result<CleanupCompletion, CleanupLedgerError> {
        let receipts = (0..self.manifest.len())
            .map(|index| self.receipt(index, CleanupDisposition::Removed))
            .collect::<Result<Vec<_>, _>>()?;
        self.completion_from_receipts(receipts)
    }

    #[cfg(test)]
    pub(crate) fn testing_missing_completion(&self) -> CleanupCompletion {
        CleanupCompletion::new(self.clone(), Vec::new())
    }

    #[cfg(test)]
    pub(crate) fn testing_duplicate_completion(
        &self,
        disposition: CleanupDisposition,
    ) -> CleanupCompletion {
        let receipt = self
            .receipt(0, disposition)
            .expect("test token is nonempty");
        CleanupCompletion::new(self.clone(), vec![receipt.clone(), receipt])
    }

    #[cfg(test)]
    pub(crate) fn testing_foreign_completion(
        &self,
        disposition: CleanupDisposition,
    ) -> CleanupCompletion {
        let mut receipt = self
            .receipt(0, disposition)
            .expect("test token is nonempty");
        let frame = &receipt.obligation;
        receipt.obligation = RouteFrameVersion::new(
            frame.router_identity(),
            frame.host(),
            frame.slot(),
            frame.origin().clone(),
            RouteFrameId::new(frame.frame().raw() + 10_000),
            frame.generation(),
        );
        CleanupCompletion::new(self.clone(), vec![receipt])
    }

    #[cfg(test)]
    pub(crate) fn testing_out_of_order_completion(
        &self,
        disposition: CleanupDisposition,
    ) -> CleanupCompletion {
        let receipt = self
            .receipt(0, disposition)
            .expect("test token is nonempty");
        let mut foreign = receipt.clone();
        foreign.obligation = RouteFrameVersion::new(
            receipt.obligation.router_identity(),
            receipt.obligation.host(),
            receipt.obligation.slot(),
            receipt.obligation.origin().clone(),
            RouteFrameId::new(receipt.obligation.frame().raw() + 1),
            receipt.obligation.generation(),
        );
        CleanupCompletion::new(self.clone(), vec![foreign, receipt])
    }

    #[cfg(test)]
    pub(crate) fn testing_foreign_batch_completion(
        &self,
        disposition: CleanupDisposition,
    ) -> CleanupCompletion {
        let mut receipt = self
            .receipt(0, disposition)
            .expect("test token is nonempty");
        receipt.batch = CleanupBatchId::new(self.batch.raw() + 1);
        CleanupCompletion::new(self.clone(), vec![receipt])
    }

    #[cfg(test)]
    pub(crate) fn testing_with_manifest(&self, manifest: Vec<RouteFrameVersion>) -> Self {
        Self::sealed(self.identity.clone(), self.batch, manifest)
    }

    #[cfg(test)]
    pub(crate) fn testing_with_next_batch(&self) -> Self {
        Self::sealed(
            self.identity.clone(),
            CleanupBatchId::new(self.batch.raw() + 1),
            self.manifest.clone(),
        )
    }

    #[cfg(test)]
    pub(crate) const fn testing_manifest_digest(&self) -> u64 {
        self.manifest_digest
    }

    #[cfg(test)]
    pub(crate) fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        Self::sealed(identity, self.batch, self.manifest.clone())
    }
    #[cfg(test)]
    pub(crate) const fn testing_identity(&self) -> &RootTaskIdentity {
        &self.identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupReceipt {
    batch: CleanupBatchId,
    obligation: RouteFrameVersion,
    disposition: CleanupDisposition,
}
impl CleanupReceipt {
    pub(crate) const fn disposition(&self) -> CleanupDisposition {
        self.disposition
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupCompletion {
    token: CleanupBatchToken,
    receipts: Vec<CleanupReceipt>,
    completion_digest: u64,
}

impl CleanupCompletion {
    fn new(token: CleanupBatchToken, receipts: Vec<CleanupReceipt>) -> Self {
        let completion_digest = completion_digest(&token, &receipts);
        Self {
            token,
            receipts,
            completion_digest,
        }
    }
    pub(crate) fn matches(&self, expected: &CleanupBatchToken) -> bool {
        &self.token == expected
            && validate_receipts(expected, &self.receipts).is_ok()
            && self.completion_digest == completion_digest(expected, &self.receipts)
    }
    pub(crate) fn receipts(&self) -> &[CleanupReceipt] {
        &self.receipts
    }
    #[cfg(test)]
    pub(crate) fn testing_set_digest(&mut self, digest: u64) {
        self.completion_digest = digest;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupLedgerError {
    MissingReceipt,
    DuplicateOrOutOfOrderReceipt,
    ForeignBatch,
    UnknownObligation,
}

fn validate_receipts(
    token: &CleanupBatchToken,
    receipts: &[CleanupReceipt],
) -> Result<(), CleanupLedgerError> {
    if receipts.len() != token.manifest.len() {
        return Err(if receipts.len() < token.manifest.len() {
            CleanupLedgerError::MissingReceipt
        } else {
            CleanupLedgerError::DuplicateOrOutOfOrderReceipt
        });
    }
    for (receipt, obligation) in receipts.iter().zip(&token.manifest) {
        if receipt.batch != token.batch {
            return Err(CleanupLedgerError::ForeignBatch);
        }
        if &receipt.obligation != obligation {
            return Err(CleanupLedgerError::DuplicateOrOutOfOrderReceipt);
        }
    }
    Ok(())
}

fn cleanup_digest(
    identity: &RootTaskIdentity,
    batch: CleanupBatchId,
    manifest: &[RouteFrameVersion],
) -> u64 {
    let mut digest = StructuredDigest::new();
    digest.identity(identity);
    digest.push(batch.raw());
    digest.push(manifest.len() as u64);
    for obligation in manifest {
        digest.frame(obligation);
    }
    digest.finish()
}

fn completion_digest(token: &CleanupBatchToken, receipts: &[CleanupReceipt]) -> u64 {
    let mut digest = StructuredDigest::new();
    digest.push(token.manifest_digest);
    digest.push(receipts.len() as u64);
    for receipt in receipts {
        digest.push(receipt.batch.raw());
        digest.frame(&receipt.obligation);
        digest.push(disposition_tag(receipt.disposition));
    }
    digest.finish()
}

struct StructuredDigest(u64);
impl StructuredDigest {
    const fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn push(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
    fn identity(&mut self, identity: &RootTaskIdentity) {
        let (domain, router) = identity.router_identity().provenance();
        self.push(domain);
        self.push(router);
        self.push(identity.host().raw());
        self.push(identity.host_epoch());
        self.push(identity.top_slot().raw());
        let (request_domain, request_router) = identity.request().router_identity().provenance();
        self.push(request_domain);
        self.push(request_router);
        self.push(identity.request().sequence());
        self.push(identity.transaction().raw());
        self.push(identity.frame_task().raw());
        self.push(identity.frame().raw());
        self.push(identity.generation().raw());
        self.origin(identity.origin());
    }
    fn frame(&mut self, frame: &RouteFrameVersion) {
        let (domain, router) = frame.router_identity().provenance();
        self.push(domain);
        self.push(router);
        self.push(frame.host().raw());
        self.push(frame.slot().raw());
        self.origin(frame.origin());
        self.push(frame.frame().raw());
        self.push(frame.generation().raw());
    }
    fn origin(&mut self, origin: &FrameOrigin) {
        match origin {
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
    }
    const fn finish(self) -> u64 {
        self.0
    }
}

const fn disposition_tag(disposition: CleanupDisposition) -> u64 {
    match disposition {
        CleanupDisposition::Removed => 1,
        CleanupDisposition::RemovedDetached => 2,
        CleanupDisposition::AlreadyAbsent => 3,
        CleanupDisposition::RejectedProtected => 4,
        CleanupDisposition::QuarantinedForeignDescendant => 5,
        CleanupDisposition::QuarantinedProtectedParent => 6,
        CleanupDisposition::QuarantinedForeignComponent => 7,
    }
}
