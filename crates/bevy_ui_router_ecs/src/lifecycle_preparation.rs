use bevy_ecs::prelude::World;
use bevy_input_focus::InputFocus;

use crate::{
    input::{RouteFocusState, RouterInputGate},
    registry::RouteEntityRegistry,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecyclePreflightSource {
    RequiredResources(RequiredResourceSet),
    HostBinding,
    PreservedCurrent,
    LeavingCurrent,
    PendingAdmission,
    PendingEntering,
    CrossRoleRebase,
    TargetPartition,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RequiredResourceSet(u8);

impl RequiredResourceSet {
    pub(crate) const ROUTE_ENTITY_REGISTRY: Self = Self(1 << 0);
    pub(crate) const ROUTER_INPUT_GATE: Self = Self(1 << 1);
    pub(crate) const INPUT_FOCUS: Self = Self(1 << 2);
    pub(crate) const ROUTE_FOCUS_STATE: Self = Self(1 << 3);

    pub(crate) fn missing_from(world: &World) -> Self {
        let mut missing = Self::default();
        if !world.contains_resource::<RouteEntityRegistry>() {
            missing.0 |= Self::ROUTE_ENTITY_REGISTRY.0;
        }
        if !world.contains_resource::<RouterInputGate>() {
            missing.0 |= Self::ROUTER_INPUT_GATE.0;
        }
        if !world.contains_resource::<InputFocus>() {
            missing.0 |= Self::INPUT_FOCUS.0;
        }
        if !world.contains_resource::<RouteFocusState>() {
            missing.0 |= Self::ROUTE_FOCUS_STATE.0;
        }
        missing
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecyclePreflightExpectation {
    ResourcePresent,
    ExactAuthority,
    ExactTaskShape,
    ExactRelationshipClosure,
    RebaseCompatible,
    DisjointClosure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecyclePreflightObserved {
    Missing,
    Drifted,
    Contradictory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LifecyclePreflightEvidence {
    source: LifecyclePreflightSource,
    expectation: LifecyclePreflightExpectation,
    observed: LifecyclePreflightObserved,
}

impl LifecyclePreflightEvidence {
    pub(crate) const fn new(
        source: LifecyclePreflightSource,
        expectation: LifecyclePreflightExpectation,
        observed: LifecyclePreflightObserved,
    ) -> Self {
        Self {
            source,
            expectation,
            observed,
        }
    }

    #[cfg(test)]
    pub(crate) const fn source(self) -> LifecyclePreflightSource {
        self.source
    }

    #[cfg(test)]
    pub(crate) const fn expectation(self) -> LifecyclePreflightExpectation {
        self.expectation
    }

    #[cfg(test)]
    pub(crate) const fn observed(self) -> LifecyclePreflightObserved {
        self.observed
    }

    pub(crate) const fn pending_message(self) -> &'static str {
        match self.source {
            LifecyclePreflightSource::PendingAdmission => {
                "projectionless fallback admission became stale"
            }
            LifecyclePreflightSource::PendingEntering => {
                "projectionless fallback entering branch drifted"
            }
            LifecyclePreflightSource::CrossRoleRebase => {
                "projectionless fallback entering branch cannot rebase"
            }
            LifecyclePreflightSource::TargetPartition => {
                "projectionless fallback target overlaps current authority"
            }
            LifecyclePreflightSource::RequiredResources(_)
            | LifecyclePreflightSource::HostBinding
            | LifecyclePreflightSource::PreservedCurrent
            | LifecyclePreflightSource::LeavingCurrent => {
                "projectionless fallback current authority drifted"
            }
        }
    }

    pub(crate) const fn is_missing_resource(self) -> bool {
        matches!(
            (self.source, self.expectation, self.observed),
            (
                LifecyclePreflightSource::RequiredResources(resources),
                LifecyclePreflightExpectation::ResourcePresent,
                LifecyclePreflightObserved::Missing
            ) if !resources.is_empty()
        )
    }

    pub(crate) const fn is_host_binding_authority_loss(self) -> bool {
        matches!(
            (self.source, self.expectation, self.observed),
            (
                LifecyclePreflightSource::HostBinding,
                LifecyclePreflightExpectation::ExactAuthority,
                LifecyclePreflightObserved::Missing | LifecyclePreflightObserved::Drifted
            )
        )
    }
}
