//! The one-shot creation payload stays with the failed branch until its real
//! continuation settles. Terminal physical responsibility carries no data lease.

use bevy_asset::UntypedHandle;
use bevy_ecs::{entity::Entity, world::World};
use bevy_ui_router_core::{
    frame::RouteFrameVersion,
    integration::{FrameMountTarget, RootFrameTask, ViewReportIdentity},
};

use super::native_scene::{
    NativeSceneCreationError, NativeSceneCreationFailure, NativeSceneIsolationFailure,
    NativeSceneResidual, NativeSceneSettlement,
};
use crate::{
    bsn::{BsnPreparationError, BsnPreparationFailure},
    registry::ExactRegistryClaim,
};

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct NativeFailureSource {
    pub(crate) report: ViewReportIdentity,
    pub(crate) frame: RouteFrameVersion,
    pub(crate) mount: FrameMountTarget,
}

impl std::fmt::Debug for NativeFailureSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeFailureSource")
            .field("frame", &self.frame)
            .field("mount", &self.mount)
            .finish_non_exhaustive()
    }
}

impl NativeFailureSource {
    pub(crate) fn from_task(task: &RootFrameTask) -> Self {
        Self {
            report: task.report_identity(),
            frame: task.frame().clone(),
            mount: task.mount_target(),
        }
    }
}

pub(crate) enum NativeFailureError {
    Preparation(BsnPreparationError),
    Creation(NativeSceneCreationError),
}

impl std::fmt::Display for NativeFailureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparation(error) => error.fmt(formatter),
            Self::Creation(error) => error.fmt(formatter),
        }
    }
}

pub(crate) struct NativeFailureOwner {
    pub(crate) source: NativeFailureSource,
    pub(crate) error: NativeFailureError,
    settlement: Option<NativeSceneSettlement>,
    #[allow(
        dead_code,
        reason = "Strong asset handles keep resolved dependencies alive during failure settlement."
    )]
    dependencies: Vec<UntypedHandle>,
    focus_error: Option<crate::native_focus::NativeFocusError>,
}

impl NativeFailureOwner {
    pub(crate) fn preparation(task: &RootFrameTask, failure: BsnPreparationFailure) -> Self {
        Self {
            source: NativeFailureSource::from_task(task),
            error: NativeFailureError::Preparation(failure.error),
            settlement: None,
            dependencies: failure.dependencies,
            focus_error: None,
        }
    }

    pub(crate) fn creation(task: &RootFrameTask, failure: NativeSceneCreationFailure) -> Self {
        let focus_error = failure.focus_error();
        let (error, settlement, dependencies) = failure.into_parts();
        Self {
            source: NativeFailureSource::from_task(task),
            error: NativeFailureError::Creation(error),
            settlement: Some(settlement),
            dependencies,
            focus_error,
        }
    }

    pub(crate) fn residual(&self) -> Option<&NativeSceneResidual> {
        self.settlement
            .as_ref()
            .and_then(NativeSceneSettlement::residual)
    }

    pub(crate) fn claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.residual()
            .into_iter()
            .flat_map(NativeSceneResidual::claims)
    }

    pub(crate) fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.claims().map(ExactRegistryClaim::entity)
    }

    pub(crate) fn validate_settled(
        &self,
        world: &World,
    ) -> Result<(), NativeSceneIsolationFailure> {
        if let Some(error) = self.focus_error {
            return Err(NativeSceneIsolationFailure::Focus(error));
        }
        self.residual()
            .map_or(Ok(()), |residual| residual.validate_inert(world))
    }

    /// Called only after the receiver prepared capacity/current proof and Core
    /// accepted completion. This move does not repeat creation destruction.
    pub(crate) fn into_terminal(self) -> Option<NativeTerminalResidual> {
        self.settlement
            .and_then(NativeSceneSettlement::into_residual)
            .map(|residual| NativeTerminalResidual {
                source: self.source,
                residual,
            })
    }
}

pub(crate) struct NativeTerminalResidual {
    pub(crate) source: NativeFailureSource,
    pub(crate) residual: NativeSceneResidual,
}

impl NativeTerminalResidual {
    pub(crate) fn shell_claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.residual.claims().filter(|claim| matches!(claim.owner(), crate::registry::RegistryOwner::Committed(crate::registry::EntityOwner::FrameShell(frame)) if frame == &self.source.frame))
    }

    pub(crate) fn input_targets<'a>(
        &'a self,
        world: &'a World,
    ) -> impl Iterator<Item = Entity> + 'a {
        // The source frame is part of identity, never a collection position.
        self.residual
            .claims()
            .filter(move |claim| {
                let frame = match claim.owner() {
                    crate::registry::RegistryOwner::Committed(
                        crate::registry::EntityOwner::FrameShell(frame)
                        | crate::registry::EntityOwner::FrameContentTarget(frame)
                        | crate::registry::EntityOwner::FrameOutput { frame, .. },
                    ) => frame,
                    _ => return false,
                };
                frame == &self.source.frame
                    && world
                        .get_resource::<crate::registry::RouteEntityRegistry>()
                        .and_then(|registry| registry.record(claim.entity()))
                        .is_some_and(|record| record.claim() == **claim)
            })
            .map(ExactRegistryClaim::entity)
    }
}
