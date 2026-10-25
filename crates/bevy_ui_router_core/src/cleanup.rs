#![allow(dead_code)]

//! ```compile_fail
//! use bevy_ui_router_core::prelude::{CleanupDisposition, CleanupSummary};
//!
//! let _ = CleanupSummary::project(false, 0, Some(CleanupDisposition::AlreadyAbsent));
//! ```

use crate::engine::RootTaskIdentity;
use crate::frame::{FrameOrigin, RouteFrameVersion};
use crate::ids::CleanupBatchId;
use crate::trace::CleanupDisposition;

const LOGICAL_ROOT_DOMAIN: u64 = 0x4c4f_4749_4341_4c52;
const TOKEN_DOMAIN: u64 = 0x434c_4541_4e54_4f4b;
const COMPLETION_DOMAIN: u64 = 0x434c_4541_4e44_4f4e;
const CANONICAL_EMPTY_COMPLETION: u64 = 0x454d_5054_594c_4544;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupSummary {
    foreground_pending: bool,
    quarantine_count: usize,
    last_disposition: Option<CleanupDisposition>,
}

impl CleanupSummary {
    pub(crate) const fn project(
        foreground_pending: bool,
        quarantine_count: usize,
        last_disposition: Option<CleanupDisposition>,
    ) -> Self {
        Self {
            foreground_pending,
            quarantine_count,
            last_disposition,
        }
    }

    pub fn foreground_pending(&self) -> bool {
        self.foreground_pending
    }

    pub fn quarantine_count(&self) -> usize {
        self.quarantine_count
    }

    pub fn last_disposition(&self) -> Option<CleanupDisposition> {
        self.last_disposition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CleanupManifestDigest(u64);

impl CleanupManifestDigest {
    pub(crate) const fn from_opaque(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CleanupCompletionDigest(u64);

impl CleanupCompletionDigest {
    pub(crate) const fn from_opaque(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn canonical_empty() -> Self {
        Self(CANONICAL_EMPTY_COMPLETION)
    }

    const fn is_canonical_empty(self) -> bool {
        self.0 == CANONICAL_EMPTY_COMPLETION
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupManifestSeal {
    logical_roots: Vec<RouteFrameVersion>,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
}

impl CleanupManifestSeal {
    pub(crate) fn new(
        logical_roots: Vec<RouteFrameVersion>,
        manifest_digest: CleanupManifestDigest,
        obligation_count: usize,
    ) -> Self {
        Self {
            logical_roots,
            manifest_digest,
            obligation_count,
        }
    }

    pub(crate) fn logical_roots(&self) -> &[RouteFrameVersion] {
        &self.logical_roots
    }

    pub(crate) const fn manifest_digest(&self) -> CleanupManifestDigest {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }
}

/// Stable task, host, transaction, batch and logical-root binding, independent of
/// a particular physical manifest. This is a comparison value, not an owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchIdentity {
    identity: RootTaskIdentity,
    batch: CleanupBatchId,
    logical_roots_binding: u64,
}

impl CleanupBatchIdentity {
    fn new(
        identity: RootTaskIdentity,
        batch: CleanupBatchId,
        logical_roots: &[RouteFrameVersion],
    ) -> Self {
        Self {
            identity,
            batch,
            logical_roots_binding: logical_roots_digest(logical_roots),
        }
    }
}

/// The concrete physical obligations sealed for a previously identified batch.
/// Neither this projection nor the batch identity carries lifecycle ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchSeal {
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
    binding: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchToken {
    identity: CleanupBatchIdentity,
    seal: CleanupBatchSeal,
}

/// An original post-commit batch reservation before its physical manifest is
/// sealed. The exact roots remain available for comparison and are returned to
/// the caller after sealing; a copied identity cannot recreate this reservation.
/// The engine still requires the original work owner before binding its token.
#[derive(Debug)]
pub(crate) struct PostCommitCleanupReservation {
    identity: CleanupBatchIdentity,
    logical_roots: Vec<RouteFrameVersion>,
}

impl PostCommitCleanupReservation {
    pub(crate) fn new(
        identity: RootTaskIdentity,
        batch: CleanupBatchId,
        logical_roots: Vec<RouteFrameVersion>,
    ) -> Self {
        Self {
            identity: CleanupBatchIdentity::new(identity, batch, &logical_roots),
            logical_roots,
        }
    }

    pub(crate) const fn identity(&self) -> &CleanupBatchIdentity {
        &self.identity
    }

    pub(crate) fn into_logical_roots(self) -> Vec<RouteFrameVersion> {
        self.logical_roots
    }

    pub(crate) fn seal(
        self,
        manifest: CleanupManifestSeal,
    ) -> Result<(CleanupBatchToken, Vec<RouteFrameVersion>), (Self, CleanupManifestSeal)> {
        if self.logical_roots != manifest.logical_roots {
            return Err((self, manifest));
        }
        let token = CleanupBatchToken::from_identity(
            self.identity,
            manifest.manifest_digest,
            manifest.obligation_count,
        );
        Ok((token, self.logical_roots))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchSourceProjection {
    identity: CleanupBatchIdentity,
    seal: CleanupBatchSeal,
}

impl CleanupBatchSourceProjection {
    pub(crate) const fn binding_digest(&self) -> u64 {
        self.seal.binding
    }

    #[cfg(test)]
    pub(crate) fn testing_with_binding_digest(&self, binding: u64) -> Self {
        let mut projection = self.clone();
        projection.seal.binding = binding;
        projection
    }
}

impl CleanupBatchToken {
    pub(crate) fn sealed(
        identity: RootTaskIdentity,
        batch: CleanupBatchId,
        logical_roots: &[RouteFrameVersion],
        manifest_digest: CleanupManifestDigest,
        obligation_count: usize,
    ) -> Self {
        Self::from_identity(
            CleanupBatchIdentity::new(identity, batch, logical_roots),
            manifest_digest,
            obligation_count,
        )
    }

    fn from_identity(
        identity: CleanupBatchIdentity,
        manifest_digest: CleanupManifestDigest,
        obligation_count: usize,
    ) -> Self {
        let binding = token_binding(
            &identity.identity,
            identity.batch,
            identity.logical_roots_binding,
            manifest_digest,
            obligation_count,
        );
        Self {
            identity,
            seal: CleanupBatchSeal {
                manifest_digest,
                obligation_count,
                binding,
            },
        }
    }

    pub(crate) const fn identity(&self) -> &RootTaskIdentity {
        &self.identity.identity
    }

    pub(crate) const fn batch(&self) -> CleanupBatchId {
        self.identity.batch
    }

    pub(crate) const fn manifest_digest(&self) -> CleanupManifestDigest {
        self.seal.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.seal.obligation_count
    }

    pub(crate) const fn batch_identity(&self) -> &CleanupBatchIdentity {
        &self.identity
    }

    pub(crate) const fn manifest_seal(&self) -> &CleanupBatchSeal {
        &self.seal
    }

    pub(crate) fn into_parts(self) -> (CleanupBatchIdentity, CleanupBatchSeal) {
        (self.identity, self.seal)
    }

    pub(crate) fn source_projection(&self) -> CleanupBatchSourceProjection {
        CleanupBatchSourceProjection {
            identity: self.identity.clone(),
            seal: self.seal.clone(),
        }
    }

    fn has_valid_binding(&self) -> bool {
        self.seal.binding
            == token_binding(
                &self.identity.identity,
                self.identity.batch,
                self.identity.logical_roots_binding,
                self.seal.manifest_digest,
                self.seal.obligation_count,
            )
    }

    #[cfg(test)]
    pub(crate) fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        let mut token = self.clone();
        token.identity.identity = identity;
        token.seal.binding = token_binding(
            &token.identity.identity,
            token.identity.batch,
            token.identity.logical_roots_binding,
            token.seal.manifest_digest,
            token.seal.obligation_count,
        );
        token
    }

    #[cfg(test)]
    pub(crate) fn testing_with_batch(&self, batch: CleanupBatchId) -> Self {
        let mut token = self.clone();
        token.identity.batch = batch;
        token.seal.binding = token_binding(
            &token.identity.identity,
            token.identity.batch,
            token.identity.logical_roots_binding,
            token.seal.manifest_digest,
            token.seal.obligation_count,
        );
        token
    }

    #[cfg(test)]
    pub(crate) const fn testing_identity(&self) -> &RootTaskIdentity {
        &self.identity.identity
    }

    #[cfg(test)]
    pub(crate) const fn testing_batch(&self) -> CleanupBatchId {
        self.identity.batch
    }

    #[cfg(test)]
    pub(crate) const fn testing_binding(&self) -> u64 {
        self.seal.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupCompletion {
    token: CleanupBatchToken,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
    binding: u64,
}

impl CleanupCompletion {
    pub(crate) fn attest(
        token: CleanupBatchToken,
        completion_digest: CleanupCompletionDigest,
        settled_count: usize,
    ) -> Self {
        let binding = completion_binding(&token, completion_digest, settled_count);
        Self {
            token,
            completion_digest,
            settled_count,
            binding,
        }
    }

    pub(crate) fn matches(&self, expected: &CleanupBatchToken) -> bool {
        self.token == *expected && self.is_well_formed()
    }

    pub(crate) const fn token(&self) -> &CleanupBatchToken {
        &self.token
    }

    pub(crate) const fn completion_digest(&self) -> CleanupCompletionDigest {
        self.completion_digest
    }

    pub(crate) const fn settled_count(&self) -> usize {
        self.settled_count
    }

    pub(crate) fn is_well_formed(&self) -> bool {
        self.token.has_valid_binding()
            && self.settled_count == self.token.obligation_count()
            && (self.settled_count == 0) == self.completion_digest.is_canonical_empty()
            && self.binding
                == completion_binding(&self.token, self.completion_digest, self.settled_count)
    }

    #[cfg(test)]
    pub(crate) fn testing_set_binding(&mut self, binding: u64) {
        self.binding = binding;
    }

    #[cfg(test)]
    pub(crate) fn testing_invalidate_binding(&mut self) {
        self.binding = self.binding.wrapping_add(1);
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use crate::trace::CleanupDisposition;

    #[test]
    fn cleanup_summary_is_a_frozen_read_only_projection() {
        fn assert_traits<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
        assert_traits::<CleanupSummary>();

        let summary = CleanupSummary::project(
            true,
            2,
            Some(CleanupDisposition::QuarantinedForeignComponent),
        );

        assert!(summary.foreground_pending());
        assert_eq!(summary.quarantine_count(), 2);
        assert_eq!(
            summary.last_disposition(),
            Some(CleanupDisposition::QuarantinedForeignComponent)
        );
    }
}

fn logical_roots_digest(logical_roots: &[RouteFrameVersion]) -> u64 {
    let mut digest = StructuredDigest::new(LOGICAL_ROOT_DOMAIN);
    digest.push(logical_roots.len() as u64);
    for root in logical_roots {
        digest.frame(root);
    }
    digest.finish()
}

fn token_binding(
    identity: &RootTaskIdentity,
    batch: CleanupBatchId,
    logical_roots_binding: u64,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
) -> u64 {
    let mut digest = StructuredDigest::new(TOKEN_DOMAIN);
    digest.identity(identity);
    digest.push(batch.raw());
    digest.push(logical_roots_binding);
    digest.push(manifest_digest.0);
    digest.push(obligation_count as u64);
    digest.finish()
}

fn completion_binding(
    token: &CleanupBatchToken,
    completion_digest: CleanupCompletionDigest,
    settled_count: usize,
) -> u64 {
    let mut digest = StructuredDigest::new(COMPLETION_DOMAIN);
    digest.push(token.seal.binding);
    digest.push(completion_digest.0);
    digest.push(settled_count as u64);
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

    fn identity(&mut self, identity: &RootTaskIdentity) {
        let (domain, router) = identity.router_identity().provenance();
        self.push(domain);
        self.push(router);
        self.push(identity.host().raw());
        self.push(identity.host_epoch());
        self.push(identity.slot().raw());
        self.push(identity.branch_index() as u64);
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
