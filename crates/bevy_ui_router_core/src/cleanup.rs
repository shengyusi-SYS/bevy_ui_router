#![allow(dead_code)]

use crate::engine::RootTaskIdentity;
use crate::ids::FrameTaskId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupBatchToken {
    identity: RootTaskIdentity,
    batch: FrameTaskId,
    manifest_digest: u64,
    obligation_count: usize,
}

impl CleanupBatchToken {
    pub(crate) fn sealed(
        identity: RootTaskIdentity,
        batch: FrameTaskId,
        obligation_count: usize,
    ) -> Self {
        Self {
            manifest_digest: cleanup_digest(&identity, batch, obligation_count),
            identity,
            batch,
            obligation_count,
        }
    }

    pub(crate) const fn obligation_count(&self) -> usize {
        self.obligation_count
    }

    pub(crate) fn completion(&self) -> CleanupCompletion {
        CleanupCompletion {
            token: self.clone(),
            completion_digest: self.manifest_digest,
        }
    }

    #[cfg(test)]
    pub(crate) fn testing_with_identity(&self, identity: RootTaskIdentity) -> Self {
        Self::sealed(identity, self.batch, self.obligation_count)
    }

    #[cfg(test)]
    pub(crate) const fn testing_identity(&self) -> &RootTaskIdentity {
        &self.identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupCompletion {
    token: CleanupBatchToken,
    completion_digest: u64,
}

impl CleanupCompletion {
    pub(crate) fn matches(&self, expected: &CleanupBatchToken) -> bool {
        &self.token == expected && self.completion_digest == expected.manifest_digest
    }

    #[cfg(test)]
    pub(crate) fn testing_set_digest(&mut self, digest: u64) {
        self.completion_digest = digest;
    }
}

fn cleanup_digest(
    identity: &RootTaskIdentity,
    _batch: FrameTaskId,
    obligation_count: usize,
) -> u64 {
    identity.request().router().raw()
        ^ identity.frame().raw().rotate_left(11)
        ^ identity.generation().raw().rotate_left(23)
        ^ (obligation_count as u64).rotate_left(37)
}
