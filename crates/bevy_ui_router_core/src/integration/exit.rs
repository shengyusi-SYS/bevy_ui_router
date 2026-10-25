//! Controlled deferred-exit ownership for the ECS bridge.

use super::*;
use crate::engine::exit::{
    CoreDeferredExitOwner, CoreExitReleaseDemand, CorePreparedExitCommit, CoreValidatedExitCommit,
};
pub use crate::engine::exit::{ExitAuthorityBeginErrorKind, ExitResidualErrorKind};

/// Original responsibility for instances whose final disposition is deferred.
/// This is not an application animation session and cannot be cloned.
#[derive(Debug)]
pub struct DeferredExitOwner {
    inner: CoreDeferredExitOwner,
}

impl DeferredExitOwner {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn outgoing(&self) -> &[RouteFrameVersion] {
        self.inner.projection().outgoing()
    }

    pub fn target(&self) -> &[RouteFrameVersion] {
        self.inner.projection().target()
    }

    pub fn cleanup_roots(&self) -> &[RouteFrameVersion] {
        self.inner.cleanup_roots()
    }

    pub fn retained(&self) -> &[RetainedLeafProjection] {
        self.inner.retained()
    }

    pub fn retiring(&self) -> &[RetainedLeafProjection] {
        self.inner.retiring()
    }
}

#[derive(Debug)]
pub struct PreparedExitCommit {
    inner: CorePreparedExitCommit,
    back_target: Option<(
        BackResolutionScopeProjection,
        HistoryExecutionBindingProjection,
    )>,
}

impl PreparedExitCommit {
    pub fn request(&self) -> NavigationRequestId {
        self.inner.request()
    }

    pub fn history_publication(&self) -> &HistoryPublicationFacts {
        self.inner.history_publication()
    }

    pub fn outgoing(&self) -> &[RouteFrameVersion] {
        self.inner.projection().outgoing()
    }

    pub fn cleanup_roots(&self) -> &[RouteFrameVersion] {
        self.inner.projection().cleanup_roots()
    }

    pub fn retained(&self) -> &[RetainedLeafProjection] {
        self.inner.projection().retained()
    }

    pub fn retiring(&self) -> &[RetainedLeafProjection] {
        self.inner.projection().retiring()
    }
}

pub struct ValidatedExitCommit<'a> {
    inner: CoreValidatedExitCommit<'a>,
    back_continuation: Option<LifecycleContinuationCapacity>,
}

impl ValidatedExitCommit<'_> {
    pub fn history_publication(&self) -> &HistoryPublicationFacts {
        self.inner.history_publication()
    }

    pub fn publish(self, install: impl FnOnce(DeferredExitOwner)) {
        let Self {
            inner,
            back_continuation,
        } = self;
        inner.publish(|inner| install(DeferredExitOwner { inner }));
        drop(back_continuation);
    }
}

pub struct ExitCommitPreparationError {
    plan: Option<RetainedCommitPlan>,
    owners: Vec<RetainedLeafOwner>,
    error: RetainedPublicationError,
}

impl ExitCommitPreparationError {
    pub fn into_parts(
        self,
    ) -> (
        Option<RetainedCommitPlan>,
        Vec<RetainedLeafOwner>,
        RetainedPublicationError,
    ) {
        (self.plan, self.owners, self.error)
    }
}

impl std::fmt::Debug for ExitCommitPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExitCommitPreparationError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

pub struct ExitCleanupBeginError {
    owner: DeferredExitOwner,
    manifest: CleanupManifest,
    error: IntegrationError,
}

impl ExitCleanupBeginError {
    pub fn into_parts(self) -> (DeferredExitOwner, CleanupManifest, IntegrationError) {
        (self.owner, self.manifest, self.error)
    }
}

impl std::fmt::Debug for ExitCleanupBeginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExitCleanupBeginError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Rejection returns every original authority and retained-instance owner.
pub struct ExitAuthorityBeginError {
    owner: DeferredExitOwner,
    lease: HostLease,
    manifest: CleanupManifest,
    retained: Vec<RetainedLeafOwner>,
    kind: ExitAuthorityBeginErrorKind,
}

impl ExitAuthorityBeginError {
    pub fn kind(&self) -> ExitAuthorityBeginErrorKind {
        self.kind
    }

    pub fn into_parts(
        self,
    ) -> (
        DeferredExitOwner,
        HostLease,
        CleanupManifest,
        Vec<RetainedLeafOwner>,
        ExitAuthorityBeginErrorKind,
    ) {
        (
            self.owner,
            self.lease,
            self.manifest,
            self.retained,
            self.kind,
        )
    }
}

impl std::fmt::Debug for ExitAuthorityBeginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExitAuthorityBeginError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// Private physical residual authority. It has no request-driving or cleanup
/// completion API and retains the original unsealed obligations.
#[derive(Debug)]
pub struct DeferredExitResidual {
    _inner: crate::engine::exit::CoreDeferredExitResidual,
}

pub struct ExitResidualError {
    owner: DeferredExitOwner,
    lease: HostLease,
    retained: Vec<RetainedLeafOwner>,
    kind: ExitResidualErrorKind,
}

impl ExitResidualError {
    pub fn into_parts(
        self,
    ) -> (
        DeferredExitOwner,
        HostLease,
        Vec<RetainedLeafOwner>,
        ExitResidualErrorKind,
    ) {
        (self.owner, self.lease, self.retained, self.kind)
    }
}

impl std::fmt::Debug for ExitResidualError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExitResidualError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitReleaseDemand {
    Current,
    QueuedRequest(NavigationRequestId),
}

impl RouterEngine {
    #[doc(hidden)]
    pub fn retire_exit_authority_to_residual(
        &mut self,
        owner: DeferredExitOwner,
        lease: HostLease,
        retained: Vec<RetainedLeafOwner>,
        cause: PostCommitAuthorityCause,
        install: impl FnOnce(DeferredExitResidual),
    ) -> Result<(), ExitResidualError> {
        self.inner
            .retire_exit_authority_to_residual(
                &mut self.admission,
                owner.inner,
                lease.inner,
                retained,
                cause.into(),
                |inner| install(DeferredExitResidual { _inner: inner }),
            )
            .map_err(|error| ExitResidualError {
                owner: DeferredExitOwner { inner: error.owner },
                lease: HostLease { inner: error.lease },
                retained: error.retained,
                kind: error.kind,
            })
    }

    pub fn prepare_exit_commit(
        &mut self,
        ticket: &CommitTicket,
        plan: Option<RetainedCommitPlan>,
        owners: Vec<RetainedLeafOwner>,
        back: Option<(&HistoryExecutionBindingView, &LifecycleContinuationCapacity)>,
    ) -> Result<PreparedExitCommit, ExitCommitPreparationError> {
        let back_target = match self.prepare_retained_back_binding(
            back,
            CoreBackTargetPhysicalReceiverRef::Commit(&ticket.inner),
        ) {
            Ok(binding) => binding,
            Err(error) => {
                return Err(ExitCommitPreparationError {
                    plan,
                    owners,
                    error,
                });
            }
        };
        self.inner
            .prepare_exit_commit(&ticket.inner, plan, owners)
            .map(|inner| PreparedExitCommit { inner, back_target })
            .map_err(|error| ExitCommitPreparationError {
                plan: error.plan,
                owners: error.owners,
                error: RetainedPublicationError::Integration(map_commit_error(error.kind)),
            })
    }

    pub fn validate_exit_publication(
        &mut self,
        prepared: PreparedExitCommit,
    ) -> Result<ValidatedExitCommit<'_>, (PreparedExitCommit, IntegrationError)> {
        if prepared.back_target.is_some() || prepared.history_publication().restores_back() {
            return Err((prepared, IntegrationError::Stale));
        }
        let PreparedExitCommit { inner, back_target } = prepared;
        self.inner
            .validate_exit_publication(inner)
            .map(|inner| ValidatedExitCommit {
                inner,
                back_continuation: None,
            })
            .map_err(|(inner, error)| {
                (
                    PreparedExitCommit { inner, back_target },
                    map_commit_error(error),
                )
            })
    }

    pub fn validate_exit_publication_for_back(
        &mut self,
        prepared: PreparedExitCommit,
        continuation: LifecycleContinuationCapacity,
    ) -> Result<
        ValidatedExitCommit<'_>,
        (
            PreparedExitCommit,
            LifecycleContinuationCapacity,
            RetainedPublicationError,
        ),
    > {
        let Some((scope, execution)) = prepared.back_target.as_ref() else {
            return Err((
                prepared,
                continuation,
                RetainedPublicationError::Integration(IntegrationError::Stale),
            ));
        };
        if &continuation.inner.scope != scope {
            let mismatch = PreparationReceiverMismatchView {
                inner: CorePreparationReceiverMismatchProjection::Scope {
                    expected: scope.clone(),
                    observed: continuation.inner.scope.clone(),
                },
            };
            return Err((
                prepared,
                continuation,
                RetainedPublicationError::BackTargetMismatch(mismatch),
            ));
        }
        if prepared.inner.history_execution() != Some(execution)
            || !scope.matches_execution(execution)
        {
            return Err((
                prepared,
                continuation,
                RetainedPublicationError::Integration(IntegrationError::Stale),
            ));
        }
        let PreparedExitCommit { inner, back_target } = prepared;
        match self.inner.validate_exit_publication(inner) {
            Ok(inner) => Ok(ValidatedExitCommit {
                inner,
                back_continuation: Some(continuation),
            }),
            Err((inner, error)) => Err((
                PreparedExitCommit { inner, back_target },
                continuation,
                RetainedPublicationError::Integration(map_commit_error(error)),
            )),
        }
    }

    /// Returns the original inactive owners and restores the unpublished history
    /// capability. No copied source or new task replaces the rejected owner.
    pub fn cancel_prepared_exit_commit(
        &mut self,
        prepared: PreparedExitCommit,
    ) -> Result<Vec<RetainedLeafOwner>, (PreparedExitCommit, IntegrationError)> {
        let PreparedExitCommit { inner, back_target } = prepared;
        self.inner
            .cancel_prepared_exit_commit(inner)
            .map_err(|(inner, error)| {
                (
                    PreparedExitCommit { inner, back_target },
                    map_commit_error(error),
                )
            })
    }

    pub fn exit_release_demand(
        &self,
        owner: &DeferredExitOwner,
    ) -> Result<ExitReleaseDemand, IntegrationError> {
        self.inner
            .exit_release_demand(&owner.inner)
            .map(|demand| match demand {
                CoreExitReleaseDemand::Current => ExitReleaseDemand::Current,
                CoreExitReleaseDemand::QueuedRequest(request) => {
                    ExitReleaseDemand::QueuedRequest(request)
                }
            })
            .map_err(map_commit_error)
    }

    pub fn begin_exit_cleanup(
        &mut self,
        owner: DeferredExitOwner,
        manifest: CleanupManifest,
        install: impl FnOnce(CleanupBatch, Vec<RetainedLeafOwner>),
    ) -> Result<(), ExitCleanupBeginError> {
        let digest = manifest.manifest_digest;
        let count = manifest.obligation_count;
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots.clone(),
            CleanupManifestDigest::from_opaque(digest),
            count,
        );
        self.inner
            .begin_exit_cleanup(owner.inner, seal, |inner, retained| {
                install(
                    CleanupBatch {
                        inner,
                        manifest_digest: digest,
                        obligation_count: count,
                    },
                    retained,
                );
            })
            .map_err(|error| ExitCleanupBeginError {
                owner: DeferredExitOwner { inner: error.owner },
                manifest,
                error: map_commit_error(error.kind),
            })
    }

    /// Seals the observed old-instance obligations and transfers the original
    /// exit into the existing post-commit authority protocol in one tail.
    /// `live_retained` must contain the entire current Live cache inventory;
    /// the callback also receives the original newly retained exit instances.
    #[doc(hidden)]
    pub fn begin_exit_authority_transfer(
        &mut self,
        owner: DeferredExitOwner,
        lease: HostLease,
        manifest: CleanupManifest,
        cause: PostCommitAuthorityCause,
        live_retained: Vec<RetainedLeafOwner>,
        install: impl FnOnce(CleanupBatch, PostCommitAuthorityPreparationSource, Vec<RetainedLeafOwner>),
    ) -> Result<(), ExitAuthorityBeginError> {
        let digest = manifest.manifest_digest;
        let count = manifest.obligation_count;
        let seal = CleanupManifestSeal::new(
            manifest.logical_roots.clone(),
            CleanupManifestDigest::from_opaque(digest),
            count,
        );
        self.inner
            .begin_exit_authority_transfer(
                owner.inner,
                lease.inner,
                seal,
                cause.into(),
                live_retained,
                |inner, source, retained| {
                    install(
                        CleanupBatch {
                            inner,
                            manifest_digest: digest,
                            obligation_count: count,
                        },
                        PostCommitAuthorityPreparationSource {
                            inner: source,
                            normal_manifest_digest: digest,
                            normal_obligation_count: count,
                        },
                        retained,
                    );
                },
            )
            .map_err(|error| ExitAuthorityBeginError {
                owner: DeferredExitOwner { inner: error.owner },
                lease: HostLease { inner: error.lease },
                manifest,
                retained: error.retained,
                kind: error.kind,
            })
    }
}
