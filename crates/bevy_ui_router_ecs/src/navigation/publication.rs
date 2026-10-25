//! Prepared publication responsibilities, independent of retained storage.

use bevy_ui_router_core::integration as core;
use bevy_ui_router_core::integration::RetainedLeafOwner;

pub(crate) enum PreparedPublication {
    Ordinary(bevy_ui_router_core::integration::PreparedCommit),
    Retained(bevy_ui_router_core::integration::PreparedRetainedCommit),
    Exit(core::PreparedExitCommit),
}

/// A rejected preparation still owns its original capabilities. This sum keeps
/// recovery independent of which disposition had been prepared.
pub(crate) enum ReturnedPublication {
    Retained(core::PreparedRetainedCommit),
    Exit(core::PreparedExitCommit),
}

impl ReturnedPublication {
    pub(crate) fn cancel(
        self,
        engine: &mut core::RouterEngine,
    ) -> Result<Vec<RetainedLeafOwner>, (Self, core::IntegrationError)> {
        match self {
            Self::Retained(prepared) => engine
                .cancel_prepared_retained_commit_for_authority_settlement(prepared)
                .map_err(|(prepared, error)| (Self::Retained(prepared), error)),
            Self::Exit(prepared) => engine
                .cancel_prepared_exit_commit(prepared)
                .map_err(|(prepared, error)| (Self::Exit(prepared), error)),
        }
    }
}

impl PreparedPublication {
    pub(crate) fn validate_for_back(
        self,
        engine: &mut bevy_ui_router_core::integration::RouterEngine,
        continuation: bevy_ui_router_core::integration::LifecycleContinuationCapacity,
    ) -> Result<
        ValidatedPublication<'_>,
        (
            ReturnedPublication,
            bevy_ui_router_core::integration::LifecycleContinuationCapacity,
            bevy_ui_router_core::integration::RetainedPublicationError,
        ),
    > {
        match self {
            Self::Retained(prepared) => engine
                .validate_retained_publication_for_back(prepared, continuation)
                .map(ValidatedPublication::Retained)
                .map_err(|(prepared, continuation, error)| {
                    (ReturnedPublication::Retained(prepared), continuation, error)
                }),
            Self::Exit(prepared) => engine
                .validate_exit_publication_for_back(prepared, continuation)
                .map(ValidatedPublication::Exit)
                .map_err(|(prepared, continuation, error)| {
                    (ReturnedPublication::Exit(prepared), continuation, error)
                }),
            Self::Ordinary(_) => unreachable!("Back carries its original bound continuation"),
        }
    }

    pub(crate) fn validate(
        self,
        engine: &mut bevy_ui_router_core::integration::RouterEngine,
    ) -> Result<
        ValidatedPublication<'_>,
        (
            Option<ReturnedPublication>,
            bevy_ui_router_core::integration::IntegrationError,
        ),
    > {
        match self {
            Self::Ordinary(prepared) => engine
                .validate_publication(prepared)
                .map(ValidatedPublication::Ordinary)
                .map_err(|error| (None, error)),
            Self::Retained(prepared) => engine
                .validate_retained_publication(prepared)
                .map(ValidatedPublication::Retained)
                .map_err(|(prepared, error)| {
                    (Some(ReturnedPublication::Retained(prepared)), error)
                }),
            Self::Exit(prepared) => engine
                .validate_exit_publication(prepared)
                .map(ValidatedPublication::Exit)
                .map_err(|(prepared, error)| (Some(ReturnedPublication::Exit(prepared)), error)),
        }
    }
}

pub(crate) enum ValidatedPublication<'a> {
    Ordinary(bevy_ui_router_core::integration::ValidatedCommit<'a>),
    Retained(bevy_ui_router_core::integration::ValidatedRetainedCommit<'a>),
    Exit(core::ValidatedExitCommit<'a>),
}

pub(crate) enum PublishedResponsibilities {
    Immediate(Vec<RetainedLeafOwner>),
    Exit(core::DeferredExitOwner),
}

impl ValidatedPublication<'_> {
    pub(crate) fn history_publication(
        &self,
    ) -> &bevy_ui_router_core::integration::HistoryPublicationFacts {
        match self {
            Self::Ordinary(commit) => commit.history_publication(),
            Self::Retained(commit) => commit.history_publication(),
            Self::Exit(commit) => commit.history_publication(),
        }
    }

    pub(crate) fn cleanup(&self) -> Option<&core::CleanupBatch> {
        match self {
            Self::Ordinary(commit) => Some(commit.cleanup()),
            Self::Retained(commit) => Some(commit.cleanup()),
            Self::Exit(_) => None,
        }
    }

    pub(crate) fn publish(self, mutation: impl FnOnce(PublishedResponsibilities)) {
        match self {
            Self::Ordinary(commit) => {
                commit.publish(|| mutation(PublishedResponsibilities::Immediate(Vec::new())))
            }
            Self::Retained(commit) => {
                commit.publish(|owners| mutation(PublishedResponsibilities::Immediate(owners)))
            }
            Self::Exit(commit) => {
                commit.publish(|owner| mutation(PublishedResponsibilities::Exit(owner)))
            }
        }
    }
}
