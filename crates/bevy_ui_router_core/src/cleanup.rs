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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchToken {
    identity: RootTaskIdentity,
    batch: CleanupBatchId,
    logical_roots_binding: u64,
    manifest_digest: CleanupManifestDigest,
    obligation_count: usize,
    binding: u64,
}

impl CleanupBatchToken {
    pub(crate) fn sealed(
        identity: RootTaskIdentity,
        batch: CleanupBatchId,
        logical_roots: &[RouteFrameVersion],
        manifest_digest: CleanupManifestDigest,
        obligation_count: usize,
    ) -> Self {
        let logical_roots_binding = logical_roots_digest(logical_roots);
        let binding = token_binding(
            &identity,
            batch,
            logical_roots_binding,
            manifest_digest,
            obligation_count,
        );
        Self {
            identity,
            batch,
            logical_roots_binding,
            manifest_digest,
            obligation_count,
            binding,
        }
    }

    pub(crate) const fn identity(&self) -> &RootTaskIdentity {
        &self.identity
    }

    pub(crate) const fn batch(&self) -> CleanupBatchId {
        self.batch
    }

    pub(crate) const fn manifest_digest(&self) -> CleanupManifestDigest {
        self.manifest_digest
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    fn has_valid_binding(&self) -> bool {
        self.binding
            == token_binding(
                &self.identity,
                self.batch,
                self.logical_roots_binding,
                self.manifest_digest,
                self.obligation_count,
            )
    }

    #[cfg(test)]
    pub(crate) fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        let mut token = self.clone();
        token.identity = identity;
        token.binding = token_binding(
            &token.identity,
            token.batch,
            token.logical_roots_binding,
            token.manifest_digest,
            token.obligation_count,
        );
        token
    }

    #[cfg(test)]
    pub(crate) fn testing_with_batch(&self, batch: CleanupBatchId) -> Self {
        let mut token = self.clone();
        token.batch = batch;
        token.binding = token_binding(
            &token.identity,
            token.batch,
            token.logical_roots_binding,
            token.manifest_digest,
            token.obligation_count,
        );
        token
    }

    #[cfg(test)]
    pub(crate) const fn testing_identity(&self) -> &RootTaskIdentity {
        &self.identity
    }

    #[cfg(test)]
    pub(crate) const fn testing_batch(&self) -> CleanupBatchId {
        self.batch
    }

    #[cfg(test)]
    pub(crate) const fn testing_binding(&self) -> u64 {
        self.binding
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

    pub(crate) fn is_well_formed(&self) -> bool {
        self.token.has_valid_binding()
            && self.settled_count == self.token.obligation_count
            && (self.settled_count == 0) == self.completion_digest.is_canonical_empty()
            && self.binding
                == completion_binding(&self.token, self.completion_digest, self.settled_count)
    }

    #[cfg(test)]
    pub(crate) fn testing_set_binding(&mut self, binding: u64) {
        self.binding = binding;
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
    digest.push(token.binding);
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
