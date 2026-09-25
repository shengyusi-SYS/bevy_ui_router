#![allow(dead_code)]

use std::{
    collections::HashSet,
    hash::{DefaultHasher, Hash as _, Hasher},
    sync::Arc,
};

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::{Component, ComponentId},
    hierarchy::Children,
};
use bevy_ecs::{
    hierarchy::ChildOf,
    prelude::{Entity, Resource, World},
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::Node;

use crate::app_ext::{
    HostRegistrationIncarnationExhaustionEvidence, RouterHandle, RouterHostRegistration,
};
use crate::components::{
    ProvisionalPostWriteProbe, ProvisionalSlotLifecycle, ProvisionalStagingProbe,
    RouteOwnershipSeal, RouterInputBarrier,
};
use crate::input::{RouteFocusState, RouterInputGate};
use crate::registry::{
    ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim,
    ProtectedRegistryProjection, ProvisionalOwner, ProvisionalTopSlotOwner, QuarantineFacts,
    QuarantineLeaseTable, QuarantineOrigin, RegistryOwner, RouteEntityRegistry,
    apply_prepared_quarantine_transfer, prepare_verified_quarantine_transfer,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HostCandidateRevision(u64);

impl HostCandidateRevision {
    pub(crate) const fn raw(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn fixture(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostTopologyFingerprint {
    candidate: Entity,
    registration_incarnation: u64,
    ancestry: Vec<HostAncestryFact>,
    outcome: TopologyOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HostAncestryFact {
    entity: Entity,
    exists: bool,
    has_node: bool,
    actual_parent: Option<Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CandidateFingerprint {
    None,
    Single(HostTopologyFingerprint),
    Conflict(Vec<(Entity, u64)>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TopologyOutcome {
    Valid,
    Invalid(InvalidHostTopologyReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InvalidHostTopologyReason {
    MissingEntity,
    MissingNode,
    RelationshipCycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedHostCandidate {
    router: RouterHandle,
    host: Entity,
    ancestry: Vec<Entity>,
    fingerprint: HostTopologyFingerprint,
    revision: HostCandidateRevision,
    observation_source: HostCandidateObservationSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedHostCandidateObservation(Option<HostTopologyFingerprint>);

pub(crate) fn observe_validated_host_candidate(
    world: &World,
    candidate: &ValidatedHostCandidate,
) -> ValidatedHostCandidateObservation {
    ValidatedHostCandidateObservation(inspect_topology(world, candidate.host))
}

impl ValidatedHostCandidate {
    pub(crate) const fn host(&self) -> Entity {
        self.host
    }
    pub(crate) fn ancestry(&self) -> &[Entity] {
        &self.ancestry
    }
    pub(crate) fn revision(&self) -> HostCandidateRevision {
        self.revision
    }

    pub(crate) const fn registration_incarnation(&self) -> u64 {
        self.fingerprint.registration_incarnation
    }

    pub(crate) fn observation_source(&self) -> HostCandidateObservationSource {
        self.observation_source.clone()
    }

    fn try_clone_for_confirmation_failure(&self) -> Result<Self, usize> {
        let ancestry = try_clone_slice(&self.ancestry).map_err(|_| self.ancestry.len())?;
        let fingerprint =
            try_clone_topology(&self.fingerprint).map_err(|_| self.fingerprint.ancestry.len())?;
        Ok(Self {
            router: self.router,
            host: self.host,
            ancestry,
            fingerprint,
            revision: self.revision,
            observation_source: self.observation_source.clone(),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedHostCandidateLatch {
    router: RouterHandle,
    revision: HostCandidateRevision,
    registration_incarnation: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedEcsHostCandidateSource {
    pub(crate) candidate: ValidatedHostCandidate,
    pub(crate) registration_incarnation: u64,
    pub(crate) revision: HostCandidateRevision,
    latch: PreparedHostCandidateLatch,
    provisional_staging: Option<PreparedProvisionalStaging>,
    confirmation_failure_candidate: Option<ValidatedHostCandidate>,
}

impl PreparedEcsHostCandidateSource {
    pub(crate) fn prepare(candidate: ValidatedHostCandidate) -> Self {
        let registration_incarnation = candidate.registration_incarnation();
        let revision = candidate.revision();
        let latch = PreparedHostCandidateLatch {
            router: candidate.router,
            revision,
            registration_incarnation,
        };
        Self {
            candidate,
            registration_incarnation,
            revision,
            latch,
            provisional_staging: None,
            confirmation_failure_candidate: None,
        }
    }

    pub(crate) fn is_current(&self, world: &World) -> bool {
        candidate_is_current(world, &self.candidate)
            && self.registration_incarnation == self.candidate.registration_incarnation()
            && self.revision == self.candidate.revision()
            && self.latch.router == self.candidate.router
            && self.latch.revision == self.revision
            && self.latch.registration_incarnation == self.registration_incarnation
    }

    pub(crate) fn observation_source(&self) -> HostCandidateObservationSource {
        self.candidate.observation_source()
    }

    pub(crate) fn prepare_staging_revalidation(
        &mut self,
        world: &mut World,
    ) -> Result<bool, HostCandidateCapacityEvidence> {
        match observe_candidate_current_with_capacity(world, &self.candidate) {
            HostCandidateCurrentObservation::Current => {
                if !world.contains_resource::<RouteEntityRegistry>()
                    || !world.contains_resource::<QuarantineLeaseTable>()
                {
                    self.provisional_staging = None;
                    self.confirmation_failure_candidate = None;
                    return Ok(true);
                }
                let confirmation_failure_candidate = self
                    .candidate
                    .try_clone_for_confirmation_failure()
                    .map_err(|requested_slots| {
                        staging_capacity_evidence(world, &self.candidate, requested_slots)
                    })?;
                let prepared = PreparedProvisionalStaging::prepare(world, &self.candidate)?;
                if !prepared.partition.prewrite_matches(world, &self.candidate) {
                    return Ok(false);
                }
                self.provisional_staging = Some(prepared);
                self.confirmation_failure_candidate = Some(confirmation_failure_candidate);
                Ok(true)
            }
            HostCandidateCurrentObservation::Stale => Ok(false),
            HostCandidateCurrentObservation::RetryableCapacity(evidence) => Err(evidence),
        }
    }

    fn take_provisional_staging(&mut self) -> Option<PreparedProvisionalStaging> {
        self.provisional_staging.take()
    }

    pub(crate) fn take_confirmation_failure_candidate(&mut self) -> Option<ValidatedHostCandidate> {
        self.confirmation_failure_candidate.take()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InvalidHostCandidate {
    router: RouterHandle,
    host: Entity,
    fingerprint: HostTopologyFingerprint,
    revision: HostCandidateRevision,
    observation_source: HostCandidateObservationSource,
}

impl InvalidHostCandidate {
    pub(crate) const fn reason(&self) -> InvalidHostTopologyReason {
        match self.fingerprint.outcome {
            TopologyOutcome::Invalid(reason) => reason,
            TopologyOutcome::Valid => unreachable!(),
        }
    }
    pub(crate) fn revision(&self) -> HostCandidateRevision {
        self.revision
    }

    pub(crate) fn observation_source(&self) -> HostCandidateObservationSource {
        self.observation_source.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConflictingHostCandidates {
    router: RouterHandle,
    candidates: Vec<Entity>,
    facts: Vec<(Entity, u64)>,
    revision: HostCandidateRevision,
    observation_source: HostCandidateObservationSource,
}

impl ConflictingHostCandidates {
    pub(crate) fn candidates(&self) -> &[Entity] {
        &self.candidates
    }
    pub(crate) const fn revision(&self) -> HostCandidateRevision {
        self.revision
    }

    pub(crate) fn observation_source(&self) -> HostCandidateObservationSource {
        self.observation_source.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostCandidateClassification {
    None(HostCandidateObservationSource),
    Unique(ValidatedHostCandidate),
    InvalidTopology(InvalidHostCandidate),
    Conflict(ConflictingHostCandidates),
    RetryableCapacity(HostCandidateCapacityEvidence),
    IdentityExhausted {
        source: HostCandidateObservationSource,
        counter: HostCandidateIdentityCounter,
        current: u64,
        attempted: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostCandidateIdentityCounter {
    RegistrationIncarnation,
    CandidateRevision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostCandidateCapacityStage {
    CandidateScan,
    ConflictFacts,
    CandidateAncestry,
    TopologyAncestry,
    TopologyVisited,
    ObservationSource,
    FingerprintClone,
    RevisionTable,
    CurrentSource,
    RejectionLatch,
    StagingRevalidation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostCandidateCapacityEvidence {
    router: RouterHandle,
    stage: HostCandidateCapacityStage,
    requested_slots: usize,
    observed_len: usize,
    observed_revision: HostCandidateRevision,
}

impl HostCandidateCapacityEvidence {
    const fn new(
        router: RouterHandle,
        stage: HostCandidateCapacityStage,
        requested_slots: usize,
        observed_len: usize,
        observed_revision: HostCandidateRevision,
    ) -> Self {
        Self {
            router,
            stage,
            requested_slots,
            observed_len,
            observed_revision,
        }
    }

    fn digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.router.hash(&mut hasher);
        (self.stage as u64).hash(&mut hasher);
        self.requested_slots.hash(&mut hasher);
        self.observed_len.hash(&mut hasher);
        self.observed_revision.hash(&mut hasher);
        hasher.finish().max(1)
    }
}

#[derive(Debug, Eq, PartialEq)]
enum HostCandidateObservationProjection {
    None {
        router: RouterHandle,
        revision: HostCandidateRevision,
        set_fingerprint: CandidateFingerprint,
    },
    Conflict {
        router: RouterHandle,
        candidates: Vec<(Entity, u64)>,
        revision: HostCandidateRevision,
        set_fingerprint: CandidateFingerprint,
    },
    Unique {
        router: RouterHandle,
        registration_incarnation: u64,
        revision: HostCandidateRevision,
        fingerprint: CandidateFingerprint,
        topology: HostTopologyFingerprint,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCandidateObservationSource {
    inner: Arc<HostCandidateObservationProjection>,
}

impl HostCandidateObservationSource {
    fn new(inner: HostCandidateObservationProjection) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub(crate) fn revision(&self) -> HostCandidateRevision {
        match self.inner.as_ref() {
            HostCandidateObservationProjection::None { revision, .. }
            | HostCandidateObservationProjection::Conflict { revision, .. }
            | HostCandidateObservationProjection::Unique { revision, .. } => *revision,
        }
    }

    pub(crate) fn digest(&self) -> u64 {
        fn mix(state: u64, value: u64) -> u64 {
            state.rotate_left(17) ^ value.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        }
        fn router_digest(router: &RouterHandle) -> u64 {
            let mut hasher = DefaultHasher::new();
            router.hash(&mut hasher);
            hasher.finish()
        }
        match self.inner.as_ref() {
            HostCandidateObservationProjection::None {
                router, revision, ..
            } => mix(router_digest(router), revision.raw()),
            HostCandidateObservationProjection::Conflict {
                router,
                candidates,
                revision,
                ..
            } => candidates.iter().fold(
                mix(router_digest(router), revision.raw()),
                |digest, (entity, incarnation)| mix(mix(digest, entity.to_bits()), *incarnation),
            ),
            HostCandidateObservationProjection::Unique {
                router,
                registration_incarnation,
                revision,
                topology,
                ..
            } => topology.ancestry.iter().fold(
                mix(
                    mix(router_digest(router), *registration_incarnation),
                    revision.raw(),
                ),
                |digest, fact| {
                    mix(
                        mix(
                            mix(digest, fact.entity.to_bits()),
                            u64::from(fact.exists) | (u64::from(fact.has_node) << 1),
                        ),
                        fact.actual_parent.map(Entity::to_bits).unwrap_or(0),
                    )
                },
            ),
        }
    }

    #[cfg(test)]
    fn unique_projection(
        &self,
    ) -> Option<(
        RouterHandle,
        u64,
        HostCandidateRevision,
        &CandidateFingerprint,
        &HostTopologyFingerprint,
    )> {
        match self.inner.as_ref() {
            HostCandidateObservationProjection::Unique {
                router,
                registration_incarnation,
                revision,
                fingerprint,
                topology,
            } => Some((
                *router,
                *registration_incarnation,
                *revision,
                fingerprint,
                topology,
            )),
            HostCandidateObservationProjection::None { .. }
            | HostCandidateObservationProjection::Conflict { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostCandidateRevisionObservation {
    Current(HostCandidateObservationSource),
    Fresh(HostCandidateObservationSource),
    RetryableCapacity(HostCandidateCapacityEvidence),
    IdentityExhausted {
        source: HostCandidateObservationSource,
        attempted_counter: u64,
    },
}

#[derive(Default, Resource)]
pub(crate) struct HostCandidateRevisionState {
    next: u64,
    observed: Vec<(RouterHandle, CandidateFingerprint, HostCandidateRevision)>,
}

impl HostCandidateRevisionState {
    pub(crate) fn current_source(
        &self,
        router: RouterHandle,
    ) -> Result<Option<HostCandidateObservationSource>, HostCandidateCapacityEvidence> {
        let Some((_, fingerprint, revision)) = self
            .observed
            .iter()
            .find(|(stored, _, _)| *stored == router)
        else {
            return Ok(None);
        };
        try_candidate_observation_source(router, fingerprint, *revision)
            .map(Some)
            .map_err(|_| {
                HostCandidateCapacityEvidence::new(
                    router,
                    HostCandidateCapacityStage::CurrentSource,
                    fingerprint_item_count(fingerprint),
                    self.observed.len(),
                    *revision,
                )
            })
    }

    fn observe(
        &mut self,
        router: RouterHandle,
        fingerprint: &CandidateFingerprint,
    ) -> HostCandidateRevisionObservation {
        if let Some((_, _, revision)) = self
            .observed
            .iter()
            .find(|(stored, prior, _)| *stored == router && prior == fingerprint)
        {
            return match try_candidate_observation_source(router, fingerprint, *revision) {
                Ok(source) => HostCandidateRevisionObservation::Current(source),
                Err(()) => HostCandidateRevisionObservation::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        router,
                        HostCandidateCapacityStage::ObservationSource,
                        fingerprint_item_count(fingerprint),
                        self.observed.len(),
                        self.next.into(),
                    ),
                ),
            };
        }
        let Some(next) = self.next.checked_add(1) else {
            return match try_candidate_observation_source(
                router,
                fingerprint,
                HostCandidateRevision(self.next),
            ) {
                Ok(source) => HostCandidateRevisionObservation::IdentityExhausted {
                    source,
                    attempted_counter: self.next,
                },
                Err(()) => HostCandidateRevisionObservation::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        router,
                        HostCandidateCapacityStage::ObservationSource,
                        fingerprint_item_count(fingerprint),
                        self.observed.len(),
                        HostCandidateRevision(self.next),
                    ),
                ),
            };
        };
        let source = match try_candidate_observation_source(
            router,
            fingerprint,
            HostCandidateRevision(next),
        ) {
            Ok(source) => source,
            Err(()) => {
                return HostCandidateRevisionObservation::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        router,
                        HostCandidateCapacityStage::ObservationSource,
                        fingerprint_item_count(fingerprint),
                        self.observed.len(),
                        HostCandidateRevision(self.next),
                    ),
                );
            }
        };
        let stored_fingerprint = match try_clone_candidate_fingerprint(fingerprint) {
            Ok(fingerprint) => fingerprint,
            Err(()) => {
                return HostCandidateRevisionObservation::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        router,
                        HostCandidateCapacityStage::FingerprintClone,
                        fingerprint_item_count(fingerprint),
                        self.observed.len(),
                        HostCandidateRevision(self.next),
                    ),
                );
            }
        };
        if !self.observed.iter().any(|(stored, _, _)| *stored == router)
            && self.observed.try_reserve(1).is_err()
        {
            return HostCandidateRevisionObservation::RetryableCapacity(
                HostCandidateCapacityEvidence::new(
                    router,
                    HostCandidateCapacityStage::RevisionTable,
                    1,
                    self.observed.len(),
                    HostCandidateRevision(self.next),
                ),
            );
        }
        self.next = next;
        let revision = HostCandidateRevision(next);
        if let Some(entry) = self
            .observed
            .iter_mut()
            .find(|(stored, _, _)| *stored == router)
        {
            *entry = (router, stored_fingerprint, revision);
        } else {
            self.observed.push((router, stored_fingerprint, revision));
        }
        HostCandidateRevisionObservation::Fresh(source)
    }
}

impl From<u64> for HostCandidateRevision {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

fn try_clone_slice<T: Clone>(source: &[T]) -> Result<Vec<T>, ()> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(source.len()).map_err(|_| ())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn try_clone_topology(source: &HostTopologyFingerprint) -> Result<HostTopologyFingerprint, ()> {
    Ok(HostTopologyFingerprint {
        candidate: source.candidate,
        registration_incarnation: source.registration_incarnation,
        ancestry: try_clone_slice(&source.ancestry)?,
        outcome: source.outcome,
    })
}

fn fingerprint_item_count(fingerprint: &CandidateFingerprint) -> usize {
    match fingerprint {
        CandidateFingerprint::None => 0,
        CandidateFingerprint::Single(topology) => topology.ancestry.len(),
        CandidateFingerprint::Conflict(candidates) => candidates.len(),
    }
}

fn try_clone_candidate_fingerprint(
    source: &CandidateFingerprint,
) -> Result<CandidateFingerprint, ()> {
    match source {
        CandidateFingerprint::None => Ok(CandidateFingerprint::None),
        CandidateFingerprint::Single(topology) => {
            try_clone_topology(topology).map(CandidateFingerprint::Single)
        }
        CandidateFingerprint::Conflict(candidates) => {
            try_clone_slice(candidates).map(CandidateFingerprint::Conflict)
        }
    }
}

fn try_candidate_observation_source(
    router: RouterHandle,
    fingerprint: &CandidateFingerprint,
    revision: HostCandidateRevision,
) -> Result<HostCandidateObservationSource, ()> {
    match fingerprint {
        CandidateFingerprint::None => Ok(HostCandidateObservationSource::new(
            HostCandidateObservationProjection::None {
                router,
                revision,
                set_fingerprint: CandidateFingerprint::None,
            },
        )),
        CandidateFingerprint::Single(topology) => Ok(HostCandidateObservationSource::new(
            HostCandidateObservationProjection::Unique {
                router,
                registration_incarnation: topology.registration_incarnation,
                revision,
                fingerprint: CandidateFingerprint::Single(try_clone_topology(topology)?),
                topology: try_clone_topology(topology)?,
            },
        )),
        CandidateFingerprint::Conflict(candidates) => Ok(HostCandidateObservationSource::new(
            HostCandidateObservationProjection::Conflict {
                router,
                candidates: try_clone_slice(candidates)?,
                revision,
                set_fingerprint: CandidateFingerprint::Conflict(try_clone_slice(candidates)?),
            },
        )),
    }
}

fn observation_source_or_exhaustion(
    observation: HostCandidateRevisionObservation,
) -> Result<HostCandidateObservationSource, HostCandidateClassification> {
    match observation {
        HostCandidateRevisionObservation::Current(source)
        | HostCandidateRevisionObservation::Fresh(source) => Ok(source),
        HostCandidateRevisionObservation::RetryableCapacity(source) => {
            Err(HostCandidateClassification::RetryableCapacity(source))
        }
        HostCandidateRevisionObservation::IdentityExhausted {
            source,
            attempted_counter,
        } => Err(HostCandidateClassification::IdentityExhausted {
            source,
            counter: HostCandidateIdentityCounter::CandidateRevision,
            current: attempted_counter,
            attempted: attempted_counter,
        }),
    }
}

fn registration_exhaustion_classification(
    router: RouterHandle,
    fingerprint: &CandidateFingerprint,
    revisions: &HostCandidateRevisionState,
    evidence: HostRegistrationIncarnationExhaustionEvidence,
) -> HostCandidateClassification {
    match try_candidate_observation_source(
        router,
        fingerprint,
        HostCandidateRevision(revisions.next),
    ) {
        Ok(source) => HostCandidateClassification::IdentityExhausted {
            source,
            counter: HostCandidateIdentityCounter::RegistrationIncarnation,
            current: evidence.current,
            attempted: evidence.attempted,
        },
        Err(()) => {
            HostCandidateClassification::RetryableCapacity(HostCandidateCapacityEvidence::new(
                router,
                HostCandidateCapacityStage::ObservationSource,
                fingerprint_item_count(fingerprint),
                revisions.observed.len(),
                HostCandidateRevision(revisions.next),
            ))
        }
    }
}

#[derive(Default, Resource)]
pub(crate) struct HostCandidateRevisionLatch {
    rejected: Vec<HostCandidateRevision>,
}

pub(crate) struct PreparedHostCandidateRejection {
    revision: HostCandidateRevision,
    already_rejected: bool,
}

impl HostCandidateRevisionLatch {
    pub(crate) fn allows(&self, revision: HostCandidateRevision) -> bool {
        !self.rejected.contains(&revision)
    }
    pub(crate) fn prepare_rejection(
        &mut self,
        router: RouterHandle,
        revision: HostCandidateRevision,
    ) -> Result<PreparedHostCandidateRejection, HostCandidateCapacityEvidence> {
        let already_rejected = self.rejected.contains(&revision);
        if !already_rejected {
            self.rejected.try_reserve(1).map_err(|_| {
                HostCandidateCapacityEvidence::new(
                    router,
                    HostCandidateCapacityStage::RejectionLatch,
                    1,
                    self.rejected.len(),
                    revision,
                )
            })?;
        }
        Ok(PreparedHostCandidateRejection {
            revision,
            already_rejected,
        })
    }

    pub(crate) fn commit_rejection(&mut self, prepared: PreparedHostCandidateRejection) {
        if !prepared.already_rejected {
            debug_assert!(!self.rejected.contains(&prepared.revision));
            debug_assert!(self.rejected.len() < self.rejected.capacity());
            self.rejected.push(prepared.revision);
        }
    }

    #[cfg(test)]
    pub(crate) fn reject(
        &mut self,
        router: RouterHandle,
        revision: HostCandidateRevision,
    ) -> Result<(), HostCandidateCapacityEvidence> {
        let prepared = self.prepare_rejection(router, revision)?;
        self.commit_rejection(prepared);
        Ok(())
    }
}

pub(crate) fn classify_host_candidate(
    world: &World,
    router: RouterHandle,
    revisions: &mut HostCandidateRevisionState,
) -> HostCandidateClassification {
    let candidate_count = world
        .iter_entities()
        .filter(|entity| {
            entity
                .get::<RouterHostRegistration>()
                .is_some_and(|registration| registration.router() == router)
        })
        .count();
    let mut candidates = Vec::new();
    if candidates.try_reserve_exact(candidate_count).is_err() {
        return HostCandidateClassification::RetryableCapacity(HostCandidateCapacityEvidence::new(
            router,
            HostCandidateCapacityStage::CandidateScan,
            candidate_count,
            0,
            HostCandidateRevision(revisions.next),
        ));
    }
    candidates.extend(world.iter_entities().filter_map(|entity| {
        entity
            .get::<RouterHostRegistration>()
            .is_some_and(|registration| registration.router() == router)
            .then_some(entity.id())
    }));
    candidates.sort_by_key(|entity| entity.to_bits());

    match candidates.as_slice() {
        [] => {
            let observed = revisions.observe(router, &CandidateFingerprint::None);
            match observation_source_or_exhaustion(observed) {
                Ok(source) => HostCandidateClassification::None(source),
                Err(exhausted) => exhausted,
            }
        }
        [candidate] => {
            let fingerprint = match inspect_topology_with_capacity(world, *candidate) {
                Ok(Some(fingerprint)) => fingerprint,
                Ok(None) => unreachable!("candidate came from the registration scan"),
                Err(stage) => {
                    return HostCandidateClassification::RetryableCapacity(
                        HostCandidateCapacityEvidence::new(
                            router,
                            stage,
                            world.iter_entities().count().saturating_add(1),
                            0,
                            HostCandidateRevision(revisions.next),
                        ),
                    );
                }
            };
            let mut ancestry = Vec::new();
            if ancestry
                .try_reserve_exact(fingerprint.ancestry.len())
                .is_err()
            {
                return HostCandidateClassification::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        router,
                        HostCandidateCapacityStage::CandidateAncestry,
                        fingerprint.ancestry.len(),
                        0,
                        HostCandidateRevision(revisions.next),
                    ),
                );
            }
            ancestry.extend(fingerprint.ancestry.iter().map(|fact| fact.entity));
            let observation_fingerprint = match try_clone_topology(&fingerprint) {
                Ok(fingerprint) => CandidateFingerprint::Single(fingerprint),
                Err(()) => {
                    return HostCandidateClassification::RetryableCapacity(
                        HostCandidateCapacityEvidence::new(
                            router,
                            HostCandidateCapacityStage::FingerprintClone,
                            fingerprint.ancestry.len(),
                            0,
                            HostCandidateRevision(revisions.next),
                        ),
                    );
                }
            };
            if let Some(evidence) = world
                .get::<RouterHostRegistration>(*candidate)
                .and_then(RouterHostRegistration::incarnation_exhaustion)
            {
                return registration_exhaustion_classification(
                    router,
                    &observation_fingerprint,
                    revisions,
                    evidence,
                );
            }
            let observation = revisions.observe(router, &observation_fingerprint);
            let source = match observation_source_or_exhaustion(observation) {
                Ok(source) => source,
                Err(exhausted) => return exhausted,
            };
            let revision = source.revision();
            match fingerprint.outcome {
                TopologyOutcome::Valid => {
                    HostCandidateClassification::Unique(ValidatedHostCandidate {
                        router,
                        host: *candidate,
                        ancestry,
                        fingerprint,
                        revision,
                        observation_source: source,
                    })
                }
                TopologyOutcome::Invalid(_) => {
                    HostCandidateClassification::InvalidTopology(InvalidHostCandidate {
                        router,
                        host: *candidate,
                        fingerprint,
                        revision,
                        observation_source: source,
                    })
                }
            }
        }
        _ => {
            let mut facts = Vec::new();
            if facts.try_reserve_exact(candidates.len()).is_err() {
                return HostCandidateClassification::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        router,
                        HostCandidateCapacityStage::ConflictFacts,
                        candidates.len(),
                        0,
                        HostCandidateRevision(revisions.next),
                    ),
                );
            }
            facts.extend(candidates.iter().map(|candidate| {
                let registration = world
                    .entity(*candidate)
                    .get::<RouterHostRegistration>()
                    .expect("candidate came from registration scan");
                (*candidate, registration.incarnation())
            }));
            let observation_fingerprint = match try_clone_slice(&facts) {
                Ok(facts) => CandidateFingerprint::Conflict(facts),
                Err(()) => {
                    return HostCandidateClassification::RetryableCapacity(
                        HostCandidateCapacityEvidence::new(
                            router,
                            HostCandidateCapacityStage::FingerprintClone,
                            candidates.len(),
                            0,
                            HostCandidateRevision(revisions.next),
                        ),
                    );
                }
            };
            if let Some(evidence) = candidates.iter().find_map(|candidate| {
                world
                    .get::<RouterHostRegistration>(*candidate)
                    .and_then(RouterHostRegistration::incarnation_exhaustion)
            }) {
                return registration_exhaustion_classification(
                    router,
                    &observation_fingerprint,
                    revisions,
                    evidence,
                );
            }
            let observation = revisions.observe(router, &observation_fingerprint);
            let source = match observation_source_or_exhaustion(observation) {
                Ok(source) => source,
                Err(exhausted) => return exhausted,
            };
            let revision = source.revision();
            HostCandidateClassification::Conflict(ConflictingHostCandidates {
                router,
                candidates,
                facts,
                revision,
                observation_source: source,
            })
        }
    }
}

fn inspect_topology(world: &World, candidate: Entity) -> Option<HostTopologyFingerprint> {
    inspect_topology_with_capacity(world, candidate)
        .ok()
        .flatten()
}

fn inspect_topology_with_capacity(
    world: &World,
    candidate: Entity,
) -> Result<Option<HostTopologyFingerprint>, HostCandidateCapacityStage> {
    let mut ancestry = Vec::new();
    let mut visited = HashSet::new();
    let maximum_ancestry = world.iter_entities().count().saturating_add(1);
    ancestry
        .try_reserve_exact(maximum_ancestry)
        .map_err(|_| HostCandidateCapacityStage::TopologyAncestry)?;
    visited
        .try_reserve(maximum_ancestry)
        .map_err(|_| HostCandidateCapacityStage::TopologyVisited)?;
    let mut current = candidate;
    let Ok(candidate_entity) = world.get_entity(candidate) else {
        return Ok(None);
    };
    let Some(registration) = candidate_entity.get::<RouterHostRegistration>() else {
        return Ok(None);
    };
    let registration_incarnation = registration.incarnation();
    loop {
        if !visited.insert(current) {
            return Ok(Some(HostTopologyFingerprint {
                candidate,
                registration_incarnation,
                ancestry,
                outcome: TopologyOutcome::Invalid(InvalidHostTopologyReason::RelationshipCycle),
            }));
        }
        let Ok(entity) = world.get_entity(current) else {
            ancestry.push(HostAncestryFact {
                entity: current,
                exists: false,
                has_node: false,
                actual_parent: None,
            });
            return Ok(Some(HostTopologyFingerprint {
                candidate,
                registration_incarnation,
                ancestry,
                outcome: TopologyOutcome::Invalid(InvalidHostTopologyReason::MissingEntity),
            }));
        };
        let parent = entity.get::<ChildOf>().map(ChildOf::parent);
        let has_node = entity.get::<Node>().is_some();
        ancestry.push(HostAncestryFact {
            entity: current,
            exists: true,
            has_node,
            actual_parent: parent,
        });
        if entity.get::<Node>().is_none() {
            return Ok(Some(HostTopologyFingerprint {
                candidate,
                registration_incarnation,
                ancestry,
                outcome: TopologyOutcome::Invalid(InvalidHostTopologyReason::MissingNode),
            }));
        }
        let Some(parent) = parent else { break };
        current = parent;
    }
    Ok(Some(HostTopologyFingerprint {
        candidate,
        registration_incarnation,
        ancestry,
        outcome: TopologyOutcome::Valid,
    }))
}

fn topology_matches_exact(world: &World, expected: &HostTopologyFingerprint) -> bool {
    if expected.outcome != TopologyOutcome::Valid {
        return false;
    }
    let Some(registration) = world.get::<RouterHostRegistration>(expected.candidate) else {
        return false;
    };
    if registration.incarnation() != expected.registration_incarnation {
        return false;
    }
    let mut current = expected.candidate;
    for expected_fact in &expected.ancestry {
        if expected_fact.entity != current || !expected_fact.exists {
            return false;
        }
        let Ok(observed) = world.get_entity(current) else {
            return false;
        };
        let observed_parent = observed.get::<ChildOf>().map(ChildOf::parent);
        if observed.get::<Node>().is_some() != expected_fact.has_node
            || observed_parent != expected_fact.actual_parent
        {
            return false;
        }
        let Some(parent) = observed_parent else {
            return expected_fact
                == expected
                    .ancestry
                    .last()
                    .expect("valid topology is non-empty");
        };
        current = parent;
    }
    false
}

#[derive(Component)]
struct StagingForeignComponent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StagingTestDrift {
    None,
    Despawn,
    Reparent(Entity),
    RemoveBarrier,
    AddForeignComponent,
    AddForeignDescendant,
    FocusSlot,
    ReattachAuthorizedHostOnDetach,
}

pub(crate) struct VerifiedQuarantineTransfer {
    claims: Vec<ExactRegistryClaim>,
    manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
    origin: Option<QuarantineOrigin>,
}

impl VerifiedQuarantineTransfer {
    fn after_world_validation(
        claims: Vec<ExactRegistryClaim>,
        manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
        origin: Option<QuarantineOrigin>,
    ) -> Self {
        Self {
            claims,
            manifests,
            facts,
            origin,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<ExactRegistryClaim>,
        Vec<(Entity, ComponentManifest)>,
        QuarantineFacts,
        Option<QuarantineOrigin>,
    ) {
        (self.claims, self.manifests, self.facts, self.origin)
    }

    #[cfg(test)]
    pub(crate) fn fixture(
        claims: Vec<ExactRegistryClaim>,
        manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
    ) -> Self {
        Self {
            claims,
            manifests,
            facts,
            origin: None,
        }
    }
}

pub(crate) struct SealedProvisionalTopSlotSpec<'a> {
    candidate: &'a ValidatedHostCandidate,
    test_drift: StagingTestDrift,
}

impl<'a> SealedProvisionalTopSlotSpec<'a> {
    pub(crate) const fn for_candidate(candidate: &'a ValidatedHostCandidate) -> Self {
        Self {
            candidate,
            test_drift: StagingTestDrift::None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn new(
        candidate: &'a ValidatedHostCandidate,
        test_drift: StagingTestDrift,
    ) -> Self {
        Self {
            candidate,
            test_drift,
        }
    }
}

#[cfg(test)]
pub(crate) fn stage_granted_top_slot(
    world: &mut World,
    candidate: ValidatedHostCandidate,
    grant: &bevy_ui_router_core::integration::HostGrant,
) -> Result<StagedProvisionalTopSlot, ProvisionalStageError> {
    let mut staged = stage_provisional_top_slot(
        world,
        SealedProvisionalTopSlotSpec::for_candidate(&candidate),
    )?;
    let claim = world
        .resource::<RouteEntityRegistry>()
        .record(staged.entity())
        .expect("validated staged slot remains registered")
        .claim();
    let committed = RegistryOwner::Committed(EntityOwner::HostTopSlot {
        host: grant.version().clone(),
        slot: grant.top_slot(),
    });
    if world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_owner(&claim, committed)
        .is_none()
    {
        let entity = staged.entity();
        let settlement = retain_staged_top_slot_for_settlement(world, &candidate, staged);
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::RegistryRejected,
            entity: Some(entity),
            settlement: Some(settlement),
        });
    }
    staged.claim = world
        .resource::<RouteEntityRegistry>()
        .record(staged.entity())
        .expect("committed staged slot remains registered")
        .claim();
    Ok(staged)
}

#[cfg(test)]
pub(crate) fn activate_granted_top_slot(
    world: &mut World,
    candidate: &ValidatedHostCandidate,
    grant: &bevy_ui_router_core::integration::HostGrant,
    staged: &StagedProvisionalTopSlot,
) -> bool {
    if !staged.partition.protected_matches(world, true)
        || !validate_granted_top_slot(world, candidate, grant, staged.entity, Visibility::Hidden)
    {
        return false;
    }
    *world
        .get_mut::<Visibility>(staged.entity)
        .expect("validated granted slot retains Visibility") = Visibility::Visible;
    world.flush();
    staged.partition.protected_matches(world, true)
        && validate_granted_top_slot(world, candidate, grant, staged.entity, Visibility::Visible)
}

#[cfg(test)]
pub(crate) fn settle_rejected_granted_top_slot(
    world: &mut World,
    candidate: &ValidatedHostCandidate,
    staged: StagedProvisionalTopSlot,
) -> bool {
    let mut settlement = staged.into_settlement(
        world,
        candidate,
        ProvisionalTopSlotSettlementStage::ProbeWritten,
    );
    matches!(
        settle_failed_provisional(world, &mut settlement, StagingTestDrift::None),
        ProvisionalSettlementAttempt::Terminal(_)
    )
}

#[cfg(test)]
fn validate_granted_top_slot(
    world: &World,
    candidate: &ValidatedHostCandidate,
    grant: &bevy_ui_router_core::integration::HostGrant,
    slot: Entity,
    visibility: Visibility,
) -> bool {
    let Some(record) = world.resource::<RouteEntityRegistry>().record(slot) else {
        return false;
    };
    matches!(
        record.owner(),
        RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot: stored_slot })
            if host == grant.version() && *stored_slot == grant.top_slot()
    ) && record.lifecycle() == EntityLifecycle::PreparedInert
        && manifest_matches_world_no_alloc(world, slot, record.manifest())
        && candidate_is_current(world, candidate)
        && world.get::<ChildOf>(slot).map(ChildOf::parent) == Some(candidate.host)
        && world
            .get::<Children>(candidate.host)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|child| **child == slot)
            .count()
            == 1
        && world.get::<Visibility>(slot) == Some(&visibility)
        && world.get::<Pickable>(slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot)
            .is_some_and(RouterInputBarrier::denies_all)
        && world.get::<ProvisionalSlotLifecycle>(slot)
            == Some(&ProvisionalSlotLifecycle::PreparedInert)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenProvisionalStagingPartition {
    slot_target: Option<Entity>,
    allowed_writes: Vec<Entity>,
    candidate_host: Entity,
    candidate_host_registration: Option<(RouterHandle, u64)>,
    candidate_host_topology: Option<HostCandidateObservationSource>,
    candidate_host_parent: Option<Entity>,
    candidate_other_children: Vec<Entity>,
    input_focus: Option<Option<Entity>>,
    route_focus: Option<Option<Entity>>,
    input_gate: Option<Vec<Entity>>,
    protected_registry: ProtectedRegistryProjection,
    protected_quarantine: QuarantineLeaseTable,
}

#[derive(Debug, Eq, PartialEq)]
struct PreparedProvisionalStaging {
    partition: Arc<FrozenProvisionalStagingPartition>,
    failure_evidence: Arc<ProvisionalStagingFailureEvidence>,
    registry_manifest: Option<ComponentManifest>,
    staged_manifest: ComponentManifest,
}

impl PreparedProvisionalStaging {
    fn prepare(
        world: &mut World,
        candidate: &ValidatedHostCandidate,
    ) -> Result<Self, HostCandidateCapacityEvidence> {
        let manifest = try_provisional_top_slot_manifest(world)
            .map_err(|_| staging_capacity_evidence(world, candidate, 1))?;
        let registry_manifest = manifest.try_clone().map_err(|_| {
            staging_capacity_evidence(world, candidate, manifest.component_ids().len())
        })?;
        let partition = Arc::new(FrozenProvisionalStagingPartition::try_freeze(
            world, candidate, None,
        )?);
        let failure_evidence = Arc::new(ProvisionalStagingFailureEvidence {
            stage: ProvisionalTopSlotSettlementStage::OwnershipRegistered,
            expected: None,
            observed: ProvisionalStagingPartitionObservation::Missing,
        });
        Ok(Self {
            partition,
            failure_evidence,
            registry_manifest: Some(registry_manifest),
            staged_manifest: manifest,
        })
    }

    fn bind_registration(&mut self, slot: Entity, seal: crate::registry::OwnershipSeal) -> bool {
        let Some(partition) = Arc::get_mut(&mut self.partition) else {
            return false;
        };
        if partition.slot_target.is_some()
            || partition.allowed_writes.len() == partition.allowed_writes.capacity()
        {
            return false;
        }
        let registry_bound = partition.protected_registry.bind_single_registration(seal);
        partition.slot_target = Some(slot);
        partition.allowed_writes.push(slot);
        let expected = Arc::clone(&self.partition);
        let Some(evidence) = Arc::get_mut(&mut self.failure_evidence) else {
            return false;
        };
        evidence.expected = Some(expected);
        registry_bound
    }
}

#[cfg(test)]
std::thread_local! {
    static FAIL_NEXT_STAGING_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn fail_next_staging_reservation_for_test() {
    FAIL_NEXT_STAGING_RESERVATION.with(|value| value.set(true));
}

fn staging_reservation_capacity_input(count: usize) -> usize {
    #[cfg(test)]
    if FAIL_NEXT_STAGING_RESERVATION.with(|value| value.replace(false)) {
        return usize::MAX;
    }
    count
}

impl FrozenProvisionalStagingPartition {
    fn try_freeze(
        world: &World,
        candidate: &ValidatedHostCandidate,
        slot_target: Option<Entity>,
    ) -> Result<Self, HostCandidateCapacityEvidence> {
        let protected_count = world.iter_entities().count();
        let mut candidate_other_children = try_children_of(world, candidate.host)
            .map_err(|_| staging_capacity_evidence(world, candidate, protected_count))?;
        candidate_other_children.retain(|entity| Some(*entity) != slot_target);
        let mut allowed_writes = Vec::new();
        allowed_writes
            .try_reserve_exact(staging_reservation_capacity_input(1))
            .map_err(|_| staging_capacity_evidence(world, candidate, 1))?;
        if let Some(slot_target) = slot_target {
            allowed_writes.push(slot_target);
        }
        let input_gate = world
            .get_resource::<RouterInputGate>()
            .map(|gate| {
                try_clone_slice(gate.allowed_projection()).map_err(|_| {
                    staging_capacity_evidence(world, candidate, gate.allowed_projection().len())
                })
            })
            .transpose()?;
        let protected_registry = world
            .get_resource::<RouteEntityRegistry>()
            .ok_or_else(|| staging_capacity_evidence(world, candidate, 0))?
            .try_protected_projection_excluding(slot_target.as_slice())
            .map_err(|_| staging_capacity_evidence(world, candidate, protected_count))?;
        let protected_quarantine = world
            .get_resource::<QuarantineLeaseTable>()
            .ok_or_else(|| staging_capacity_evidence(world, candidate, 0))?
            .try_clone_projection()
            .map_err(|_| staging_capacity_evidence(world, candidate, protected_count))?;
        Ok(Self {
            slot_target,
            allowed_writes,
            candidate_host: candidate.host,
            candidate_host_registration: Some((
                candidate.router,
                candidate.registration_incarnation(),
            )),
            candidate_host_topology: Some(candidate.observation_source()),
            candidate_host_parent: world.get::<ChildOf>(candidate.host).map(ChildOf::parent),
            candidate_other_children,
            input_focus: world.get_resource::<InputFocus>().map(InputFocus::get),
            route_focus: world
                .get_resource::<RouteFocusState>()
                .map(RouteFocusState::focused),
            input_gate,
            protected_registry,
            protected_quarantine,
        })
    }

    fn observe_live(
        world: &World,
        candidate: &ValidatedHostCandidate,
        slot_target: Entity,
    ) -> ProvisionalStagingPartitionObservation {
        let topology = match inspect_topology_with_capacity(world, candidate.host) {
            Ok(topology) => topology,
            Err(_) => {
                return ProvisionalStagingPartitionObservation::RetryableCapacity(
                    HostCandidateCapacityEvidence::new(
                        candidate.router,
                        HostCandidateCapacityStage::StagingRevalidation,
                        world.iter_entities().count().saturating_add(1),
                        0,
                        candidate.revision,
                    ),
                );
            }
        };
        let mut observed = match Self::try_freeze(world, candidate, Some(slot_target)) {
            Ok(observed) => observed,
            Err(evidence) => {
                return ProvisionalStagingPartitionObservation::RetryableCapacity(evidence);
            }
        };
        observed.candidate_host_registration = world
            .get::<RouterHostRegistration>(candidate.host)
            .map(|registration| (registration.router(), registration.incarnation()));
        observed.candidate_host_topology = match topology {
            Some(topology) => {
                let fingerprint = CandidateFingerprint::Single(topology);
                match try_candidate_observation_source(
                    candidate.router,
                    &fingerprint,
                    candidate.revision,
                ) {
                    Ok(source) => Some(source),
                    Err(()) => {
                        return ProvisionalStagingPartitionObservation::RetryableCapacity(
                            staging_capacity_evidence(
                                world,
                                candidate,
                                fingerprint_item_count(&fingerprint),
                            ),
                        );
                    }
                }
            }
            None => None,
        };
        ProvisionalStagingPartitionObservation::Observed(observed)
    }

    fn prewrite_matches(&self, world: &World, candidate: &ValidatedHostCandidate) -> bool {
        self.slot_target.is_none()
            && self.allowed_writes.is_empty()
            && candidate_is_current(world, candidate)
            && self.protected_matches_inner(world, false, None)
    }

    fn protected_matches(&self, world: &World, edge_written: bool) -> bool {
        let Some(slot_target) = self.slot_target else {
            return false;
        };
        self.protected_matches_inner(world, edge_written, Some(slot_target))
    }

    fn protected_matches_inner(
        &self,
        world: &World,
        edge_written: bool,
        slot_target: Option<Entity>,
    ) -> bool {
        if slot_target.is_some_and(|slot| self.allowed_writes.as_slice() != [slot])
            || slot_target.is_none() && !self.allowed_writes.is_empty()
            || world
                .get::<RouterHostRegistration>(self.candidate_host)
                .map(|registration| (registration.router(), registration.incarnation()))
                != self.candidate_host_registration
            || self.candidate_host_topology.as_ref().is_none_or(|source| {
                let HostCandidateObservationProjection::Unique { topology, .. } =
                    source.inner.as_ref()
                else {
                    return true;
                };
                !topology_matches_exact(world, topology)
            })
            || world
                .get::<ChildOf>(self.candidate_host)
                .map(ChildOf::parent)
                != self.candidate_host_parent
            || world.get_resource::<InputFocus>().map(InputFocus::get) != self.input_focus
            || world
                .get_resource::<RouteFocusState>()
                .map(RouteFocusState::focused)
                != self.route_focus
            || world
                .get_resource::<RouterInputGate>()
                .map(RouterInputGate::allowed_projection)
                != self.input_gate.as_deref()
        {
            return false;
        }
        // App-owned host components are not router ownership authority. Its
        // registration, Node topology and relationships are checked above/below.
        let expected_children = self
            .candidate_other_children
            .iter()
            .copied()
            .filter(|entity| {
                slot_target.is_none_or(|slot| !is_descendant_or_self_no_alloc(world, slot, *entity))
            })
            .chain((edge_written).then_some(slot_target).flatten());
        let observed_candidate_children = world
            .get::<Children>(self.candidate_host)
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter(|entity| {
                slot_target.is_none_or(|slot| {
                    *entity == slot || !is_descendant_or_self_no_alloc(world, slot, *entity)
                })
            });
        let excluded_storage = slot_target.map(|slot| [slot]);
        let excluded_target = excluded_storage
            .as_ref()
            .map_or(&[][..], |value| &value[..]);
        if !observed_candidate_children.eq(expected_children)
            || world
                .get_resource::<RouteEntityRegistry>()
                .is_none_or(|registry| {
                    !registry.matches_protected_projection_excluding(
                        &self.protected_registry,
                        excluded_target,
                    )
                })
            || world.get_resource::<QuarantineLeaseTable>() != Some(&self.protected_quarantine)
        {
            return false;
        }
        true
    }

    fn digest(&self) -> u64 {
        struct DigestWriter(DefaultHasher);

        impl std::fmt::Write for DigestWriter {
            fn write_str(&mut self, value: &str) -> std::fmt::Result {
                self.0.write(value.as_bytes());
                Ok(())
            }
        }

        use std::fmt::Write as _;
        let mut writer = DigestWriter(DefaultHasher::new());
        write!(&mut writer, "{self:?}").expect("hash formatter is infallible");
        writer.0.finish().max(1)
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ProvisionalStagingPartitionObservation {
    Observed(FrozenProvisionalStagingPartition),
    Missing,
    RetryableCapacity(HostCandidateCapacityEvidence),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StagedProvisionalTopSlot {
    entity: Entity,
    claim: ExactRegistryClaim,
    manifest: ComponentManifest,
    owner: ProvisionalTopSlotOwner,
    seal: crate::registry::OwnershipSeal,
    partition: Arc<FrozenProvisionalStagingPartition>,
    failure_evidence: Arc<ProvisionalStagingFailureEvidence>,
}

impl StagedProvisionalTopSlot {
    pub(crate) const fn entity(&self) -> Entity {
        self.entity
    }

    fn into_settlement(
        mut self,
        world: &World,
        candidate: &ValidatedHostCandidate,
        stage: ProvisionalTopSlotSettlementStage,
    ) -> ProvisionalHostSettlement {
        let evidence = Arc::get_mut(&mut self.failure_evidence)
            .expect("staging failure evidence remains uniquely owned before settlement");
        evidence.stage = stage;
        evidence.observed =
            FrozenProvisionalStagingPartition::observe_live(world, candidate, self.entity);
        ProvisionalHostSettlement {
            staged: self,
            authorized_host: candidate.host,
            candidate_router: candidate.router,
            quarantine_origin: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalStageErrorKind {
    MissingFoundation,
    MissingPreparation,
    StaleCandidate,
    RegistryRejected,
    PostWriteDrift,
    QuarantineDetachBlocked,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalStageError {
    kind: ProvisionalStageErrorKind,
    entity: Option<Entity>,
    settlement: Option<ProvisionalHostSettlement>,
}

impl ProvisionalStageError {
    pub(crate) const fn entity(&self) -> Option<Entity> {
        self.entity
    }

    pub(crate) fn into_settlement(self) -> Option<ProvisionalHostSettlement> {
        self.settlement
    }

    pub(crate) const fn kind(&self) -> ProvisionalStageErrorKind {
        self.kind
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalHostSettlement {
    staged: StagedProvisionalTopSlot,
    authorized_host: Entity,
    candidate_router: RouterHandle,
    quarantine_origin: Option<QuarantineOrigin>,
}

impl ProvisionalHostSettlement {
    pub(crate) const fn slot(&self) -> Entity {
        self.staged.entity
    }

    pub(crate) fn claim(&self) -> ExactRegistryClaim {
        self.staged.claim.clone()
    }

    pub(crate) fn failure_evidence(&self) -> Arc<ProvisionalStagingFailureEvidence> {
        Arc::clone(&self.staged.failure_evidence)
    }

    pub(crate) fn bind_quarantine_origin(
        &mut self,
        router: RouterHandle,
        former_host: bevy_ui_router_core::HostVersion,
    ) -> bool {
        if router != self.candidate_router {
            return false;
        }
        let origin = QuarantineOrigin::provisional(router, former_host);
        match self.quarantine_origin.as_ref() {
            Some(bound) => bound == &origin,
            None => {
                self.quarantine_origin = Some(origin);
                true
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalStagingFailureEvidence {
    stage: ProvisionalTopSlotSettlementStage,
    expected: Option<Arc<FrozenProvisionalStagingPartition>>,
    observed: ProvisionalStagingPartitionObservation,
}

impl ProvisionalStagingFailureEvidence {
    pub(crate) fn expected_digest(&self) -> u64 {
        self.expected
            .as_ref()
            .map_or(1, |expected| expected.digest())
    }

    pub(crate) fn observed_digest(&self) -> u64 {
        match &self.observed {
            ProvisionalStagingPartitionObservation::Observed(observed) => observed.digest(),
            ProvisionalStagingPartitionObservation::Missing => 1,
            ProvisionalStagingPartitionObservation::RetryableCapacity(evidence) => {
                evidence.digest()
            }
        }
    }

    pub(crate) fn digest(&self) -> u64 {
        self.expected_digest()
            .rotate_left(17)
            .wrapping_add(self.observed_digest().rotate_left(31))
            .wrapping_add((self.stage as u64).rotate_left(47))
            .max(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalHostSettlementDisposition {
    Removed,
    AlreadyRemoved,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalHostSettlementReceipt {
    claim: ExactRegistryClaim,
    disposition: ProvisionalHostSettlementDisposition,
    terminal_digest: u64,
}

impl ProvisionalHostSettlementReceipt {
    pub(crate) const fn terminal_digest(&self) -> u64 {
        self.terminal_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalSettlementTargetExpectation {
    claim: ExactRegistryClaim,
    registry_manifest: ComponentManifest,
    route_seal: Option<crate::registry::OwnershipSeal>,
    lifecycle: Option<ProvisionalSlotLifecycle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedProvisionalSettlementTarget {
    entity_present: bool,
    claim: Option<ExactRegistryClaim>,
    registry_manifest: Option<ComponentManifest>,
    route_seal: Option<crate::registry::OwnershipSeal>,
    lifecycle: Option<ProvisionalSlotLifecycle>,
    visibility: Option<Visibility>,
    pickable: Option<Pickable>,
    barrier: Option<RouterInputBarrier>,
    parent: Option<Entity>,
    descendants: Vec<Entity>,
    actual_manifest: Option<ComponentManifest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalHostSettlementMismatch {
    MissingPartition,
    AuthorityLost {
        expected: ExactRegistryClaim,
        observed: Option<ExactRegistryClaim>,
    },
    TargetDrift {
        expected: ProvisionalSettlementTargetExpectation,
        observed: ObservedProvisionalSettlementTarget,
    },
    ProtectedDrift,
    DetachBlocked,
    QuarantineRejected,
    DespawnRejected,
}

enum ProvisionalSettlementAttempt {
    Terminal(ProvisionalHostSettlementReceipt),
    Retryable,
    Inert(ProvisionalHostSettlementMismatch),
}

pub(crate) struct ProvisionalHostSettlementTerminal {
    settlement: ProvisionalHostSettlement,
    receipt: ProvisionalHostSettlementReceipt,
}

impl ProvisionalHostSettlementTerminal {
    pub(crate) const fn slot(&self) -> Entity {
        self.settlement.staged.entity
    }

    pub(crate) fn claim(&self) -> ExactRegistryClaim {
        self.receipt.claim.clone()
    }

    pub(crate) const fn receipt(&self) -> &ProvisionalHostSettlementReceipt {
        &self.receipt
    }
}

pub(crate) struct ProvisionalHostCleanupLedger {
    settlement: Option<ProvisionalHostSettlement>,
    receipt: Option<ProvisionalHostSettlementReceipt>,
    mismatch: Option<ProvisionalHostSettlementMismatch>,
    attempt_count: u64,
}

impl ProvisionalHostCleanupLedger {
    pub(crate) const fn new(settlement: ProvisionalHostSettlement) -> Self {
        Self {
            settlement: Some(settlement),
            receipt: None,
            mismatch: None,
            attempt_count: 0,
        }
    }

    pub(crate) fn slot(&self) -> Entity {
        self.settlement.as_ref().map_or_else(
            || self.receipt.as_ref().unwrap().claim.entity(),
            |owner| owner.slot(),
        )
    }

    pub(crate) fn claim(&self) -> ExactRegistryClaim {
        self.settlement.as_ref().map_or_else(
            || self.receipt.as_ref().unwrap().claim.clone(),
            ProvisionalHostSettlement::claim,
        )
    }

    pub(crate) fn is_inert(&self) -> bool {
        self.mismatch.is_some()
    }

    pub(crate) fn settle(&mut self, world: &mut World) -> bool {
        if self.receipt.is_some() || self.mismatch.is_some() {
            return false;
        }
        let Some(owner) = self.settlement.as_mut() else {
            self.mismatch = Some(ProvisionalHostSettlementMismatch::MissingPartition);
            return false;
        };
        self.attempt_count = self.attempt_count.saturating_add(1);
        match settle_failed_provisional(world, owner, StagingTestDrift::None) {
            ProvisionalSettlementAttempt::Terminal(receipt) => {
                self.receipt = Some(receipt);
                true
            }
            ProvisionalSettlementAttempt::Retryable => false,
            ProvisionalSettlementAttempt::Inert(evidence) => {
                self.mismatch = Some(evidence);
                false
            }
        }
    }

    pub(crate) fn into_terminal(mut self) -> Result<ProvisionalHostSettlementTerminal, Self> {
        let (Some(settlement), Some(receipt), None) = (
            self.settlement.take(),
            self.receipt.take(),
            self.mismatch.as_ref(),
        ) else {
            return Err(self);
        };
        if receipt.claim != settlement.staged.claim {
            self.settlement = Some(settlement);
            self.receipt = Some(receipt);
            self.mismatch = Some(ProvisionalHostSettlementMismatch::AuthorityLost {
                expected: self.settlement.as_ref().unwrap().staged.claim.clone(),
                observed: None,
            });
            return Err(self);
        }
        Ok(ProvisionalHostSettlementTerminal {
            settlement,
            receipt,
        })
    }
}

pub(crate) fn retain_staged_top_slot_for_settlement(
    world: &World,
    candidate: &ValidatedHostCandidate,
    staged: StagedProvisionalTopSlot,
) -> ProvisionalHostSettlement {
    staged.into_settlement(
        world,
        candidate,
        ProvisionalTopSlotSettlementStage::ProbeWritten,
    )
}

pub(crate) fn stage_source_bound_granted_top_slot(
    world: &mut World,
    candidate: &mut PreparedEcsHostCandidateSource,
    grant: &bevy_ui_router_core::integration::SourceBoundHostGrant,
) -> Result<StagedProvisionalTopSlot, ProvisionalStageError> {
    let Some(prepared) = candidate.take_provisional_staging() else {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::MissingPreparation,
            entity: None,
            settlement: None,
        });
    };
    let mut staged = stage_prepared_provisional_top_slot(
        world,
        SealedProvisionalTopSlotSpec::for_candidate(&candidate.candidate),
        prepared,
    )?;
    let claim = staged.claim.clone();
    let committed_claim = world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_host_top_slot_owner(&claim, grant.version().clone(), grant.top_slot());
    let Some(committed_claim) = committed_claim else {
        let entity = staged.entity();
        let settlement = retain_staged_top_slot_for_settlement(world, &candidate.candidate, staged);
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::RegistryRejected,
            entity: Some(entity),
            settlement: Some(settlement),
        });
    };
    staged.claim = committed_claim;
    Ok(staged)
}

pub(crate) fn activate_source_bound_granted_top_slot(
    world: &mut World,
    candidate: &PreparedEcsHostCandidateSource,
    grant: &bevy_ui_router_core::integration::SourceBoundHostGrant,
    staged: &StagedProvisionalTopSlot,
) -> bool {
    if !staged.partition.protected_matches(world, true)
        || !validate_source_bound_granted_top_slot(
            world,
            &candidate.candidate,
            grant,
            staged.entity,
            Visibility::Hidden,
        )
    {
        return false;
    }
    *world
        .get_mut::<Visibility>(staged.entity)
        .expect("validated granted slot retains Visibility") = Visibility::Visible;
    world.flush();
    activated_source_bound_granted_top_slot_is_current(world, candidate, grant, staged)
}

pub(crate) fn activated_source_bound_granted_top_slot_is_current(
    world: &World,
    candidate: &PreparedEcsHostCandidateSource,
    grant: &bevy_ui_router_core::integration::SourceBoundHostGrant,
    staged: &StagedProvisionalTopSlot,
) -> bool {
    staged.partition.protected_matches(world, true)
        && world
            .resource::<RouteEntityRegistry>()
            .record(staged.entity)
            .is_some_and(|record| record.claim() == staged.claim)
        && validate_source_bound_granted_top_slot(
            world,
            &candidate.candidate,
            grant,
            staged.entity,
            Visibility::Visible,
        )
}

#[derive(Debug)]
pub(crate) struct PostScopeHostConfirmationFailure {
    expected_partition: Arc<FrozenProvisionalStagingPartition>,
    observed_partition: ProvisionalStagingPartitionObservation,
    expected: HostConfirmationSlotObservation,
    observed: HostConfirmationSlotObservation,
}

#[derive(Debug)]
struct HostConfirmationSlotObservation {
    entity: Entity,
    claim: Option<ExactRegistryClaim>,
    manifest: Option<ComponentManifest>,
    candidate: ValidatedHostCandidateObservation,
    parent: Option<Entity>,
    visibility: Option<Visibility>,
    pickable: Option<Pickable>,
    barrier: Option<RouterInputBarrier>,
    lifecycle: Option<ProvisionalSlotLifecycle>,
}

impl PostScopeHostConfirmationFailure {
    pub(crate) fn expected_digest(&self) -> u64 {
        crate::cleanup::admission_evidence_digest(&(&self.expected_partition, &self.expected))
    }
    pub(crate) fn observed_digest(&self) -> u64 {
        crate::cleanup::admission_evidence_digest(&(&self.observed_partition, &self.observed))
    }
}

pub(crate) fn post_scope_host_confirmation_failure(
    world: &World,
    candidate: &PreparedEcsHostCandidateSource,
    grant: &bevy_ui_router_core::integration::SourceBoundHostGrant,
    staged: &StagedProvisionalTopSlot,
) -> Option<PostScopeHostConfirmationFailure> {
    if activated_source_bound_granted_top_slot_is_current(world, candidate, grant, staged) {
        return None;
    }
    Some(PostScopeHostConfirmationFailure {
        expected_partition: Arc::clone(&staged.partition),
        observed_partition: FrozenProvisionalStagingPartition::observe_live(
            world,
            &candidate.candidate,
            staged.entity,
        ),
        expected: HostConfirmationSlotObservation {
            entity: staged.entity,
            claim: Some(staged.claim.clone()),
            manifest: Some(staged.manifest.clone()),
            candidate: ValidatedHostCandidateObservation(Some(
                candidate.candidate.fingerprint.clone(),
            )),
            parent: Some(candidate.candidate.host),
            visibility: Some(Visibility::Visible),
            pickable: Some(Pickable::IGNORE),
            barrier: Some(RouterInputBarrier::deny_all()),
            lifecycle: Some(ProvisionalSlotLifecycle::PreparedInert),
        },
        observed: HostConfirmationSlotObservation {
            entity: staged.entity,
            claim: world
                .resource::<RouteEntityRegistry>()
                .record(staged.entity)
                .map(|record| record.claim()),
            manifest: ComponentManifest::from_world(world, staged.entity),
            candidate: observe_validated_host_candidate(world, &candidate.candidate),
            parent: world.get::<ChildOf>(staged.entity).map(ChildOf::parent),
            visibility: world.get::<Visibility>(staged.entity).copied(),
            pickable: world.get::<Pickable>(staged.entity).copied(),
            barrier: world.get::<RouterInputBarrier>(staged.entity).copied(),
            lifecycle: world
                .get::<ProvisionalSlotLifecycle>(staged.entity)
                .copied(),
        },
    })
}

pub(crate) fn hide_source_bound_granted_top_slot(
    world: &mut World,
    candidate: &PreparedEcsHostCandidateSource,
    grant: &bevy_ui_router_core::integration::SourceBoundHostGrant,
    staged: &StagedProvisionalTopSlot,
) -> bool {
    if !staged.partition.protected_matches(world, true)
        || !validate_source_bound_granted_top_slot(
            world,
            &candidate.candidate,
            grant,
            staged.entity,
            Visibility::Visible,
        )
    {
        return false;
    }
    *world
        .get_mut::<Visibility>(staged.entity)
        .expect("validated granted slot retains Visibility") = Visibility::Hidden;
    world.flush();
    staged.partition.protected_matches(world, true)
        && validate_source_bound_granted_top_slot(
            world,
            &candidate.candidate,
            grant,
            staged.entity,
            Visibility::Hidden,
        )
}

fn validate_source_bound_granted_top_slot(
    world: &World,
    candidate: &ValidatedHostCandidate,
    grant: &bevy_ui_router_core::integration::SourceBoundHostGrant,
    slot: Entity,
    visibility: Visibility,
) -> bool {
    let Some(record) = world.resource::<RouteEntityRegistry>().record(slot) else {
        return false;
    };
    matches!(
        record.owner(),
        RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot: stored_slot })
            if host == grant.version() && *stored_slot == grant.top_slot()
    ) && record.lifecycle() == EntityLifecycle::PreparedInert
        && manifest_matches_world_no_alloc(world, slot, record.manifest())
        && candidate_is_current(world, candidate)
        && world.get::<ChildOf>(slot).map(ChildOf::parent) == Some(candidate.host)
        && world
            .get::<Children>(candidate.host)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|child| **child == slot)
            .count()
            == 1
        && world.get::<Visibility>(slot) == Some(&visibility)
        && world.get::<Pickable>(slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot)
            .is_some_and(RouterInputBarrier::denies_all)
        && world.get::<ProvisionalSlotLifecycle>(slot)
            == Some(&ProvisionalSlotLifecycle::PreparedInert)
}

pub(crate) fn stage_provisional_top_slot(
    world: &mut World,
    spec: SealedProvisionalTopSlotSpec<'_>,
) -> Result<StagedProvisionalTopSlot, ProvisionalStageError> {
    if !world.contains_resource::<RouteEntityRegistry>()
        || !world.contains_resource::<QuarantineLeaseTable>()
    {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::MissingFoundation,
            entity: None,
            settlement: None,
        });
    }
    let prepared = PreparedProvisionalStaging::prepare(world, spec.candidate).map_err(|_| {
        ProvisionalStageError {
            kind: ProvisionalStageErrorKind::MissingPreparation,
            entity: None,
            settlement: None,
        }
    })?;
    stage_prepared_provisional_top_slot(world, spec, prepared)
}

fn stage_prepared_provisional_top_slot(
    world: &mut World,
    spec: SealedProvisionalTopSlotSpec<'_>,
    mut prepared: PreparedProvisionalStaging,
) -> Result<StagedProvisionalTopSlot, ProvisionalStageError> {
    if !world.contains_resource::<RouteEntityRegistry>()
        || !world.contains_resource::<QuarantineLeaseTable>()
    {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::MissingFoundation,
            entity: None,
            settlement: None,
        });
    }
    if !prepared.partition.prewrite_matches(world, spec.candidate) {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::StaleCandidate,
            entity: None,
            settlement: None,
        });
    }

    let host = spec.candidate.host;
    let owner = ProvisionalTopSlotOwner::new(spec.candidate.revision);
    let provisional_owner =
        RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone()));
    if world
        .resource_mut::<RouteEntityRegistry>()
        .try_reserve_registration(&provisional_owner)
        .is_err()
    {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::RegistryRejected,
            entity: None,
            settlement: None,
        });
    }
    let slot = world.spawn_empty().id();
    let (seal, owner, claim) = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        let claim = registry
            .register(
                slot,
                provisional_owner,
                prepared
                    .registry_manifest
                    .take()
                    .expect("prevalidated staging retains its registry manifest"),
                EntityLifecycle::PreparedInert,
            )
            .expect("pre-reserved fresh provisional registration cannot fail");
        (claim.seal(), owner, claim)
    };
    let partition_bound = prepared.bind_registration(slot, seal);
    let staged = StagedProvisionalTopSlot {
        entity: slot,
        claim: claim.clone(),
        manifest: prepared.staged_manifest,
        owner: owner.clone(),
        seal,
        partition: prepared.partition,
        failure_evidence: prepared.failure_evidence,
    };

    if !partition_bound {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::PostWriteDrift,
            entity: Some(slot),
            settlement: Some(staged.into_settlement(
                world,
                spec.candidate,
                ProvisionalTopSlotSettlementStage::OwnershipRegistered,
            )),
        });
    }

    world.entity_mut(slot).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
        RouteOwnershipSeal::new(seal),
        ProvisionalSlotLifecycle::PreparedInert,
        ProvisionalStagingProbe,
    ));
    world.flush();
    if !validate_provisional_staging_prefix(
        world,
        spec.candidate,
        slot,
        host,
        &staged.partition.candidate_other_children,
        &owner,
        seal,
        &claim,
        &staged.manifest,
        &staged.partition,
        ProvisionalTopSlotSettlementStage::BundleWritten,
    ) {
        return Err(finish_provisional_stage_failure(
            world,
            spec.candidate,
            staged,
            ProvisionalTopSlotSettlementStage::BundleWritten,
        ));
    }
    if world.get_entity(slot).is_ok() {
        world.entity_mut(slot).insert(Node {
            width: bevy_ui::Val::Percent(100.0),
            height: bevy_ui::Val::Percent(100.0),
            ..Node::default()
        });
    }
    world.flush();
    if !validate_provisional_staging_prefix(
        world,
        spec.candidate,
        slot,
        host,
        &staged.partition.candidate_other_children,
        &owner,
        seal,
        &claim,
        &staged.manifest,
        &staged.partition,
        ProvisionalTopSlotSettlementStage::NodeWritten,
    ) {
        return Err(finish_provisional_stage_failure(
            world,
            spec.candidate,
            staged,
            ProvisionalTopSlotSettlementStage::NodeWritten,
        ));
    }
    if world.get_entity(slot).is_ok() {
        world.entity_mut(slot).insert(ChildOf(host));
    }
    world.flush();
    if !validate_provisional_staging_prefix(
        world,
        spec.candidate,
        slot,
        host,
        &staged.partition.candidate_other_children,
        &owner,
        seal,
        &claim,
        &staged.manifest,
        &staged.partition,
        ProvisionalTopSlotSettlementStage::HostEdgeWritten,
    ) {
        return Err(finish_provisional_stage_failure(
            world,
            spec.candidate,
            staged,
            ProvisionalTopSlotSettlementStage::HostEdgeWritten,
        ));
    }
    if world.get_entity(slot).is_ok() {
        world.entity_mut(slot).insert(ProvisionalPostWriteProbe);
        world.flush();
    }
    if !validate_provisional_staging_prefix(
        world,
        spec.candidate,
        slot,
        host,
        &staged.partition.candidate_other_children,
        &owner,
        seal,
        &claim,
        &staged.manifest,
        &staged.partition,
        ProvisionalTopSlotSettlementStage::ProbeWritten,
    ) {
        return Err(finish_provisional_stage_failure(
            world,
            spec.candidate,
            staged,
            ProvisionalTopSlotSettlementStage::ProbeWritten,
        ));
    }
    apply_staging_test_drift(world, slot, spec.test_drift);
    clear_focus_in_subtree(world, slot);

    let valid = candidate_is_current(world, spec.candidate)
        && staged.partition.protected_matches(world, true)
        && validate_provisional_slot(
            world,
            slot,
            host,
            &staged.partition.candidate_other_children,
            &owner,
            seal,
            &claim,
            &staged.manifest,
        );
    if valid {
        return Ok(staged);
    }
    Err(ProvisionalStageError {
        kind: ProvisionalStageErrorKind::PostWriteDrift,
        entity: Some(slot),
        settlement: Some(staged.into_settlement(
            world,
            spec.candidate,
            ProvisionalTopSlotSettlementStage::ProbeWritten,
        )),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalTopSlotSettlementStage {
    OwnershipRegistered,
    BundleWritten,
    NodeWritten,
    HostEdgeWritten,
    ProbeWritten,
}

fn validate_provisional_staging_prefix(
    world: &World,
    candidate: &ValidatedHostCandidate,
    slot: Entity,
    host: Entity,
    host_children_baseline: &[Entity],
    owner: &ProvisionalTopSlotOwner,
    seal: crate::registry::OwnershipSeal,
    claim: &ExactRegistryClaim,
    manifest: &ComponentManifest,
    partition: &FrozenProvisionalStagingPartition,
    stage: ProvisionalTopSlotSettlementStage,
) -> bool {
    let edge_written = matches!(
        stage,
        ProvisionalTopSlotSettlementStage::HostEdgeWritten
            | ProvisionalTopSlotSettlementStage::ProbeWritten
    );
    let Some(record) = world.resource::<RouteEntityRegistry>().record(slot) else {
        return false;
    };
    if !candidate_is_current(world, candidate)
        || !partition.protected_matches(world, edge_written)
        || !record.matches_claim(claim)
        || record.owner()
            != &RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone()))
        || record.seal() != seal
        || record.lifecycle() != EntityLifecycle::PreparedInert
        || record.manifest() != manifest
        || world
            .get::<RouteOwnershipSeal>(slot)
            .is_none_or(|stored| stored.seal() != seal)
        || world.get::<ProvisionalSlotLifecycle>(slot)
            != Some(&ProvisionalSlotLifecycle::PreparedInert)
        || world.get::<Visibility>(slot) != Some(&Visibility::Hidden)
        || world.get::<Pickable>(slot) != Some(&Pickable::IGNORE)
        || !world
            .get::<RouterInputBarrier>(slot)
            .is_some_and(RouterInputBarrier::denies_all)
    {
        return false;
    }

    let node_written = matches!(
        stage,
        ProvisionalTopSlotSettlementStage::NodeWritten
            | ProvisionalTopSlotSettlementStage::HostEdgeWritten
            | ProvisionalTopSlotSettlementStage::ProbeWritten
    );
    if world.get::<Node>(slot).is_some() != node_written
        || world.get::<ChildOf>(slot).is_some() != edge_written
    {
        return false;
    }
    if node_written
        && !world.get::<Node>(slot).is_some_and(|node| {
            node.width == bevy_ui::Val::Percent(100.0)
                && node.height == bevy_ui::Val::Percent(100.0)
        })
    {
        return false;
    }
    if edge_written {
        if world.get::<ChildOf>(slot).map(ChildOf::parent) != Some(host) {
            return false;
        }
        world
            .get::<Children>(host)
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .eq(host_children_baseline
                .iter()
                .copied()
                .chain(std::iter::once(slot)))
    } else {
        world
            .get::<Children>(host)
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .eq(host_children_baseline.iter().copied())
    }
}

fn finish_provisional_stage_failure(
    world: &mut World,
    candidate: &ValidatedHostCandidate,
    staged: StagedProvisionalTopSlot,
    stage: ProvisionalTopSlotSettlementStage,
) -> ProvisionalStageError {
    let slot = staged.entity();
    ProvisionalStageError {
        kind: ProvisionalStageErrorKind::PostWriteDrift,
        entity: Some(slot),
        settlement: Some(staged.into_settlement(world, candidate, stage)),
    }
}

enum HostCandidateCurrentObservation {
    Current,
    Stale,
    RetryableCapacity(HostCandidateCapacityEvidence),
}

fn observe_candidate_current_with_capacity(
    world: &World,
    candidate: &ValidatedHostCandidate,
) -> HostCandidateCurrentObservation {
    let observed = match inspect_topology_with_capacity(world, candidate.host) {
        Ok(observed) => observed,
        Err(_) => {
            return HostCandidateCurrentObservation::RetryableCapacity(
                HostCandidateCapacityEvidence::new(
                    candidate.router,
                    HostCandidateCapacityStage::StagingRevalidation,
                    world.iter_entities().count().saturating_add(1),
                    0,
                    candidate.revision,
                ),
            );
        }
    };
    if world
        .get::<RouterHostRegistration>(candidate.host)
        .is_some_and(|registration| {
            registration.router() == candidate.router
                && registration.incarnation() == candidate.registration_incarnation()
        })
        && observed.as_ref() == Some(&candidate.fingerprint)
    {
        HostCandidateCurrentObservation::Current
    } else {
        HostCandidateCurrentObservation::Stale
    }
}

fn candidate_is_current(world: &World, candidate: &ValidatedHostCandidate) -> bool {
    world
        .get_entity(candidate.host)
        .ok()
        .and_then(|entity| entity.get::<RouterHostRegistration>())
        .is_some_and(|registration| {
            registration.router() == candidate.router
                && registration.incarnation() == candidate.registration_incarnation()
        })
        && topology_matches_exact(world, &candidate.fingerprint)
}

fn manifest_matches_world_no_alloc(
    world: &World,
    entity: Entity,
    manifest: &ComponentManifest,
) -> bool {
    manifest.matches_without_materialization(world, entity)
}

pub(crate) fn validated_host_candidate_is_current(
    world: &World,
    candidate: &ValidatedHostCandidate,
) -> bool {
    candidate_is_current(world, candidate)
}

fn try_provisional_top_slot_manifest(world: &mut World) -> Result<ComponentManifest, ()> {
    let explicit = [
        world.register_component::<Visibility>(),
        world.register_component::<Pickable>(),
        world.register_component::<RouterInputBarrier>(),
        world.register_component::<RouteOwnershipSeal>(),
        world.register_component::<ProvisionalSlotLifecycle>(),
        world.register_component::<ProvisionalStagingProbe>(),
        world.register_component::<ProvisionalPostWriteProbe>(),
        world.register_component::<Node>(),
        world.register_component::<ChildOf>(),
    ];
    let required_count = explicit
        .iter()
        .filter_map(|component| world.get_required_components_by_id(*component))
        .map(|required| required.iter_ids().count())
        .sum::<usize>();
    let mut complete = Vec::new();
    complete
        .try_reserve_exact(explicit.len().saturating_add(required_count))
        .map_err(|_| ())?;
    complete.extend_from_slice(&explicit);
    for component in explicit {
        if let Some(required) = world.get_required_components_by_id(component) {
            complete.extend(required.iter_ids());
        }
    }
    ComponentManifest::try_new(complete)
}

fn validate_provisional_slot(
    world: &World,
    slot: Entity,
    host: Entity,
    host_children_baseline: &[Entity],
    owner: &ProvisionalTopSlotOwner,
    seal: crate::registry::OwnershipSeal,
    claim: &ExactRegistryClaim,
    manifest: &ComponentManifest,
) -> bool {
    let Some(record) = world.resource::<RouteEntityRegistry>().record(slot) else {
        return false;
    };
    let registry_valid = record.matches_claim(claim)
        && record.owner()
            == &RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone()))
        && record.seal() == seal
        && record.lifecycle() == EntityLifecycle::PreparedInert
        && record.manifest() == manifest
        && manifest_matches_world_no_alloc(world, slot, manifest);
    let entity_valid = world
        .get::<RouteOwnershipSeal>(slot)
        .is_some_and(|stored| stored.seal() == seal)
        && world.get::<ProvisionalSlotLifecycle>(slot)
            == Some(&ProvisionalSlotLifecycle::PreparedInert)
        && world.get::<Visibility>(slot) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot)
            .is_some_and(RouterInputBarrier::denies_all)
        && world.get::<Node>(slot).is_some_and(|node| {
            node.width == bevy_ui::Val::Percent(100.0)
                && node.height == bevy_ui::Val::Percent(100.0)
        })
        && world.get::<ChildOf>(slot).map(ChildOf::parent) == Some(host)
        && world
            .get::<Children>(slot)
            .is_none_or(|children| children.is_empty())
        && !focus_is_in_subtree(world, slot);
    let children_valid = world
        .get::<Children>(host)
        .into_iter()
        .flat_map(|children| children.iter().copied())
        .eq(host_children_baseline
            .iter()
            .copied()
            .chain(std::iter::once(slot)));
    registry_valid && entity_valid && children_valid
}

fn apply_staging_test_drift(world: &mut World, slot: Entity, drift: StagingTestDrift) {
    match drift {
        StagingTestDrift::None => {}
        StagingTestDrift::Despawn => {
            world.despawn(slot);
        }
        StagingTestDrift::Reparent(parent) if world.get_entity(slot).is_ok() => {
            world.entity_mut(slot).insert(ChildOf(parent));
        }
        StagingTestDrift::RemoveBarrier if world.get_entity(slot).is_ok() => {
            world.entity_mut(slot).remove::<RouterInputBarrier>();
        }
        StagingTestDrift::AddForeignComponent if world.get_entity(slot).is_ok() => {
            world.entity_mut(slot).insert(StagingForeignComponent);
        }
        StagingTestDrift::AddForeignDescendant if world.get_entity(slot).is_ok() => {
            let foreign = world.spawn_empty().id();
            world.entity_mut(slot).add_child(foreign);
        }
        StagingTestDrift::FocusSlot if world.get_entity(slot).is_ok() => {
            if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                *focus = InputFocus::from_entity(slot);
            }
            if let Some(mut focus) = world.get_resource_mut::<RouteFocusState>() {
                focus.set_focused(Some(slot));
            }
        }
        StagingTestDrift::ReattachAuthorizedHostOnDetach if world.get_entity(slot).is_ok() => {
            world.entity_mut(slot).remove::<RouterInputBarrier>();
        }
        _ => {}
    }
}

fn provisional_settlement_receipt(
    claim: ExactRegistryClaim,
    disposition: ProvisionalHostSettlementDisposition,
) -> ProvisionalHostSettlementReceipt {
    let terminal_digest =
        claim
            .opaque_digest_part()
            .rotate_left(17)
            .wrapping_add(match disposition {
                ProvisionalHostSettlementDisposition::Removed => 1,
                ProvisionalHostSettlementDisposition::AlreadyRemoved => 2,
                ProvisionalHostSettlementDisposition::Quarantined => 3,
            });
    ProvisionalHostSettlementReceipt {
        claim,
        disposition,
        terminal_digest: terminal_digest.max(1),
    }
}

fn provisional_settlement_target_expectation(
    settlement: &ProvisionalHostSettlement,
    lifecycle: ProvisionalSlotLifecycle,
) -> ProvisionalSettlementTargetExpectation {
    let bundle_was_written = settlement.staged.failure_evidence.stage
        != ProvisionalTopSlotSettlementStage::OwnershipRegistered;
    ProvisionalSettlementTargetExpectation {
        claim: settlement.staged.claim.clone(),
        registry_manifest: settlement.staged.manifest.clone(),
        route_seal: bundle_was_written.then_some(settlement.staged.seal),
        lifecycle: (bundle_was_written || lifecycle == ProvisionalSlotLifecycle::Quarantined)
            .then_some(lifecycle),
    }
}

fn observe_provisional_settlement_target(
    world: &World,
    slot: Entity,
) -> Result<ObservedProvisionalSettlementTarget, ()> {
    let (claim, registry_manifest) = match world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(slot))
    {
        Some(record) => (Some(record.claim()), Some(record.manifest().try_clone()?)),
        None => (None, None),
    };
    let entity_present = world.get_entity(slot).is_ok();
    Ok(ObservedProvisionalSettlementTarget {
        entity_present,
        claim,
        registry_manifest,
        route_seal: world
            .get::<RouteOwnershipSeal>(slot)
            .copied()
            .map(RouteOwnershipSeal::seal),
        lifecycle: world.get::<ProvisionalSlotLifecycle>(slot).copied(),
        visibility: world.get::<Visibility>(slot).cloned(),
        pickable: world.get::<Pickable>(slot).cloned(),
        barrier: world.get::<RouterInputBarrier>(slot).copied(),
        parent: world.get::<ChildOf>(slot).map(ChildOf::parent),
        descendants: entity_present
            .then(|| collect_descendants(world, slot))
            .unwrap_or_default(),
        actual_manifest: ComponentManifest::try_from_world(world, slot)?,
    })
}

fn validate_observed_provisional_settlement_target(
    settlement: &ProvisionalHostSettlement,
    lifecycle: ProvisionalSlotLifecycle,
    require_isolated: bool,
    observed: ObservedProvisionalSettlementTarget,
) -> Result<ObservedProvisionalSettlementTarget, ProvisionalHostSettlementMismatch> {
    let expected = provisional_settlement_target_expectation(settlement, lifecycle);
    if observed.claim.as_ref() != Some(&expected.claim) {
        return Err(ProvisionalHostSettlementMismatch::AuthorityLost {
            expected: expected.claim,
            observed: observed.claim,
        });
    }
    let isolated = observed.visibility == Some(Visibility::Hidden)
        && observed.pickable == Some(Pickable::IGNORE)
        && observed.barrier.is_some_and(|barrier| barrier.denies_all());
    if !observed.entity_present
        || observed.registry_manifest.as_ref() != Some(&expected.registry_manifest)
        || observed.route_seal != expected.route_seal
        || observed.lifecycle != expected.lifecycle
        || observed.actual_manifest.is_none()
        || require_isolated && !isolated
    {
        return Err(ProvisionalHostSettlementMismatch::TargetDrift { expected, observed });
    }
    Ok(observed)
}

fn settle_missing_provisional_target(
    world: &mut World,
    partition: &FrozenProvisionalStagingPartition,
    prepared_claim: &ExactRegistryClaim,
) -> ProvisionalSettlementAttempt {
    if !partition.protected_matches(world, false) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::ProtectedDrift,
        );
    }
    let observed = world
        .resource::<RouteEntityRegistry>()
        .record(prepared_claim.entity())
        .map(|record| record.claim());
    if observed.as_ref() != Some(prepared_claim)
        || !world
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(prepared_claim)
    {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::AuthorityLost {
                expected: prepared_claim.clone(),
                observed,
            },
        );
    }
    ProvisionalSettlementAttempt::Terminal(provisional_settlement_receipt(
        prepared_claim.clone(),
        ProvisionalHostSettlementDisposition::AlreadyRemoved,
    ))
}

fn settle_failed_provisional(
    world: &mut World,
    settlement: &mut ProvisionalHostSettlement,
    test_drift: StagingTestDrift,
) -> ProvisionalSettlementAttempt {
    let slot = settlement.staged.entity;
    let authorized_host = settlement.authorized_host;
    let manifest = settlement.staged.manifest.clone();
    let prepared_claim = settlement.staged.claim.clone();
    let partition = Arc::clone(&settlement.staged.partition);
    let initial = match observe_provisional_settlement_target(world, slot) {
        Ok(observed) => observed,
        Err(()) => return ProvisionalSettlementAttempt::Retryable,
    };
    let edge_written = initial.entity_present && initial.parent == Some(authorized_host);
    if !partition.protected_matches(world, edge_written) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::ProtectedDrift,
        );
    }
    if !initial.entity_present {
        return settle_missing_provisional_target(world, &partition, &prepared_claim);
    }
    if let Err(mismatch) = validate_observed_provisional_settlement_target(
        settlement,
        ProvisionalSlotLifecycle::PreparedInert,
        false,
        initial,
    ) {
        return ProvisionalSettlementAttempt::Inert(mismatch);
    }

    world.entity_mut(slot).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
    ));
    world.flush();
    let first_isolation = match observe_provisional_settlement_target(world, slot) {
        Ok(observed) => observed,
        Err(()) => return ProvisionalSettlementAttempt::Retryable,
    };
    if !first_isolation.entity_present {
        return settle_missing_provisional_target(world, &partition, &prepared_claim);
    }
    let first_isolation = match validate_observed_provisional_settlement_target(
        settlement,
        ProvisionalSlotLifecycle::PreparedInert,
        true,
        first_isolation,
    ) {
        Ok(observed) => observed,
        Err(mismatch) => return ProvisionalSettlementAttempt::Inert(mismatch),
    };
    let first_edge_written = first_isolation.parent == Some(authorized_host);
    if !partition.protected_matches(world, first_edge_written) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::ProtectedDrift,
        );
    }
    clear_focus_in_subtree(world, slot);

    if first_edge_written {
        world.entity_mut(slot).remove::<ChildOf>();
        world.flush();
        if test_drift == StagingTestDrift::ReattachAuthorizedHostOnDetach
            && world.get_entity(slot).is_ok()
        {
            world.entity_mut(slot).insert(ChildOf(authorized_host));
            world.flush();
        }
    }
    let after_detach = match observe_provisional_settlement_target(world, slot) {
        Ok(observed) => observed,
        Err(()) => return ProvisionalSettlementAttempt::Retryable,
    };
    if !after_detach.entity_present {
        return settle_missing_provisional_target(world, &partition, &prepared_claim);
    }
    let after_detach = match validate_observed_provisional_settlement_target(
        settlement,
        ProvisionalSlotLifecycle::PreparedInert,
        true,
        after_detach,
    ) {
        Ok(observed) => observed,
        Err(mismatch) => return ProvisionalSettlementAttempt::Inert(mismatch),
    };
    if after_detach.parent == Some(authorized_host) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::DetachBlocked,
        );
    }
    if !partition.protected_matches(world, false) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::ProtectedDrift,
        );
    }
    world.entity_mut(slot).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
    ));
    world.flush();
    let second_isolation = match observe_provisional_settlement_target(world, slot) {
        Ok(observed) => observed,
        Err(()) => return ProvisionalSettlementAttempt::Retryable,
    };
    if !second_isolation.entity_present {
        return settle_missing_provisional_target(world, &partition, &prepared_claim);
    }
    let second_isolation = match validate_observed_provisional_settlement_target(
        settlement,
        ProvisionalSlotLifecycle::PreparedInert,
        true,
        second_isolation,
    ) {
        Ok(observed) => observed,
        Err(mismatch) => return ProvisionalSettlementAttempt::Inert(mismatch),
    };
    if second_isolation.parent == Some(authorized_host) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::DetachBlocked,
        );
    }
    if !partition.protected_matches(world, false) {
        return ProvisionalSettlementAttempt::Inert(
            ProvisionalHostSettlementMismatch::ProtectedDrift,
        );
    }
    clear_focus_in_subtree(world, slot);
    let protected_parent = second_isolation.parent;
    let foreign_descendants = second_isolation.descendants;
    let actual_manifest = second_isolation
        .actual_manifest
        .expect("validated present target retains an exact component manifest");
    let foreign_components = actual_manifest
        .component_ids()
        .iter()
        .copied()
        .filter(|id| !manifest.component_ids().contains(id))
        .collect::<Vec<ComponentId>>();

    if protected_parent.is_some()
        || !foreign_descendants.is_empty()
        || !foreign_components.is_empty()
    {
        world
            .entity_mut(slot)
            .insert(ProvisionalSlotLifecycle::Quarantined);
        world.flush();
        let quarantined = match observe_provisional_settlement_target(world, slot) {
            Ok(observed) => observed,
            Err(()) => return ProvisionalSettlementAttempt::Retryable,
        };
        if !quarantined.entity_present {
            let expected = provisional_settlement_target_expectation(
                settlement,
                ProvisionalSlotLifecycle::Quarantined,
            );
            return ProvisionalSettlementAttempt::Inert(
                ProvisionalHostSettlementMismatch::TargetDrift {
                    expected,
                    observed: quarantined,
                },
            );
        }
        let quarantined = match validate_observed_provisional_settlement_target(
            settlement,
            ProvisionalSlotLifecycle::Quarantined,
            true,
            quarantined,
        ) {
            Ok(observed) => observed,
            Err(mismatch) => return ProvisionalSettlementAttempt::Inert(mismatch),
        };
        if quarantined.parent == Some(authorized_host) {
            return ProvisionalSettlementAttempt::Inert(
                ProvisionalHostSettlementMismatch::DetachBlocked,
            );
        }
        if !partition.protected_matches(world, false) {
            return ProvisionalSettlementAttempt::Inert(
                ProvisionalHostSettlementMismatch::ProtectedDrift,
            );
        }
        let protected_parent = quarantined.parent;
        let foreign_descendants = quarantined.descendants;
        let actual_manifest = quarantined
            .actual_manifest
            .expect("validated quarantined target retains an exact component manifest");
        let foreign_components = actual_manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|id| !manifest.component_ids().contains(id))
            .collect::<Vec<ComponentId>>();
        let facts = QuarantineFacts::new(
            Some(authorized_host),
            protected_parent,
            foreign_descendants,
            foreign_components,
        );
        let transfer = VerifiedQuarantineTransfer::after_world_validation(
            vec![prepared_claim.clone()],
            vec![(slot, actual_manifest)],
            facts,
            settlement.quarantine_origin.clone(),
        );
        let prepared = world.resource_scope(
            |world, registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
                world.resource_scope(
                    |_world, mut table: bevy_ecs::prelude::Mut<QuarantineLeaseTable>| {
                        prepare_verified_quarantine_transfer(&mut table, &registry, transfer)
                    },
                )
            },
        );
        let transfer_plan = match prepared {
            Ok(prepared) => prepared,
            Err(crate::registry::QuarantineTransferError::CapacityExhausted) => {
                return ProvisionalSettlementAttempt::Retryable;
            }
            Err(_) => {
                return ProvisionalSettlementAttempt::Inert(
                    ProvisionalHostSettlementMismatch::QuarantineRejected,
                );
            }
        };
        world.resource_scope(
            |world, mut registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
                world.resource_scope(
                    |_world, mut table: bevy_ecs::prelude::Mut<QuarantineLeaseTable>| {
                        apply_prepared_quarantine_transfer(
                            &mut table,
                            &mut registry,
                            transfer_plan,
                        );
                    },
                )
            },
        );
        ProvisionalSettlementAttempt::Terminal(provisional_settlement_receipt(
            prepared_claim,
            ProvisionalHostSettlementDisposition::Quarantined,
        ))
    } else {
        let removed = world.despawn(slot);
        world.flush();
        if !removed {
            return ProvisionalSettlementAttempt::Inert(
                ProvisionalHostSettlementMismatch::DespawnRejected,
            );
        }
        if !partition.protected_matches(world, false) {
            return ProvisionalSettlementAttempt::Inert(
                ProvisionalHostSettlementMismatch::ProtectedDrift,
            );
        }
        let observed = world
            .resource::<RouteEntityRegistry>()
            .record(slot)
            .map(|record| record.claim());
        if observed.as_ref() != Some(&prepared_claim)
            || !world
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(&prepared_claim)
        {
            return ProvisionalSettlementAttempt::Inert(
                ProvisionalHostSettlementMismatch::AuthorityLost {
                    expected: prepared_claim,
                    observed,
                },
            );
        }
        ProvisionalSettlementAttempt::Terminal(provisional_settlement_receipt(
            prepared_claim,
            ProvisionalHostSettlementDisposition::Removed,
        ))
    }
}

fn children_of(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map_or_else(Vec::new, |children| children.iter().copied().collect())
}

fn try_children_of(world: &World, entity: Entity) -> Result<Vec<Entity>, ()> {
    let Some(children) = world.get::<Children>(entity) else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    result.try_reserve_exact(children.len()).map_err(|_| ())?;
    result.extend(children.iter().copied());
    Ok(result)
}

fn staging_capacity_evidence(
    world: &World,
    candidate: &ValidatedHostCandidate,
    requested_slots: usize,
) -> HostCandidateCapacityEvidence {
    HostCandidateCapacityEvidence::new(
        candidate.router,
        HostCandidateCapacityStage::StagingRevalidation,
        requested_slots,
        world.iter_entities().count(),
        candidate.revision,
    )
}

fn is_descendant_or_self_no_alloc(world: &World, root: Entity, mut current: Entity) -> bool {
    for _ in 0..=world.iter_entities().count() {
        if current == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
    false
}

fn collect_descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut result = Vec::new();
    let mut visited = HashSet::from([root]);
    let mut stack = children_of(world, root);
    stack.sort_by_key(|entity| entity.to_bits());
    stack.reverse();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            continue;
        }
        result.push(entity);
        let mut children = children_of(world, entity);
        children.sort_by_key(|child| child.to_bits());
        stack.extend(children.into_iter().rev());
    }
    result
}

fn focus_is_in_subtree(world: &World, root: Entity) -> bool {
    let focused = world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .or_else(|| {
            world
                .get_resource::<RouteFocusState>()
                .and_then(RouteFocusState::focused)
        });
    focused.is_some_and(|focused| is_descendant_or_self(world, root, focused))
}

fn clear_focus_in_subtree(world: &mut World, root: Entity) {
    let input = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    if input.is_some_and(|focused| is_descendant_or_self(world, root, focused)) {
        world.resource_mut::<InputFocus>().clear();
    }
    let route = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused);
    if route.is_some_and(|focused| is_descendant_or_self(world, root, focused)) {
        world.resource_mut::<RouteFocusState>().set_focused(None);
    }
}

fn is_descendant_or_self(world: &World, root: Entity, mut current: Entity) -> bool {
    for _ in 0..=world.iter_entities().count() {
        if current == root {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
    false
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app_ext::{
        HostRegistrationIncarnationExhaustionEvidence, RouterHandle, RouterHostRegistration,
    };
    use crate::{
        components::{ProvisionalPostWriteProbe, ProvisionalStagingProbe, RouterInputBarrier},
        registry::{QuarantineLeaseTable, RouteEntityRegistry},
    };
    use bevy_camera::visibility::Visibility;
    use bevy_ecs::{
        component::Component,
        hierarchy::{ChildOf, Children},
        lifecycle::HookContext,
        prelude::{Resource, World},
    };
    use bevy_input_focus::InputFocus;
    use bevy_picking::Pickable;
    use bevy_ui::Node;

    #[derive(Component)]
    struct AppSentinel;

    #[derive(Resource, Default)]
    struct ProbeAudit(Vec<bool>);

    #[derive(Resource, Default)]
    struct FirstStagingWriteAudit {
        host: Option<Entity>,
        observations: Vec<(bool, bool)>,
    }

    fn router() -> RouterHandle {
        RouterHandle::fixture(7, 11)
    }

    #[test]
    fn classifies_none_unique_invalid_and_conflicting_candidates_without_world_mutation() {
        let mut world = World::new();
        let mut revisions = HostCandidateRevisionState::default();
        assert!(matches!(
            classify_host_candidate(&world, router(), &mut revisions),
            HostCandidateClassification::None(_)
        ));

        let ancestor = world.spawn((Node::default(),)).id();
        let sentinel = world.spawn_empty().id();
        let host = world
            .spawn((
                Node::default(),
                RouterHostRegistration::new(router()),
                ChildOf(ancestor),
            ))
            .id();
        let unique = classify_host_candidate(&world, router(), &mut revisions);
        let HostCandidateClassification::Unique(candidate) = unique else {
            panic!("expected unique")
        };
        assert_eq!(candidate.host(), host);
        assert_eq!(candidate.ancestry(), &[host, ancestor]);
        assert!(world.get_entity(sentinel).is_ok());

        world.entity_mut(ancestor).remove::<Node>();
        assert!(matches!(
            classify_host_candidate(&world, router(), &mut revisions),
            HostCandidateClassification::InvalidTopology(_)
        ));
        world.entity_mut(ancestor).insert(Node::default());
        let competing = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let HostCandidateClassification::Conflict(conflict) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected conflict")
        };
        let mut expected = vec![host, competing];
        expected.sort_by_key(|entity| entity.to_bits());
        assert_eq!(conflict.candidates(), expected.as_slice());
    }

    #[test]
    pub(crate) fn rejects_non_node_host_and_cycles_and_revisions_are_monotonic() {
        let mut world = World::new();
        let host = world.spawn((RouterHostRegistration::new(router()),)).id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::InvalidTopology(first) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected invalid")
        };
        assert_eq!(first.reason(), InvalidHostTopologyReason::MissingNode);

        world.entity_mut(host).insert(Node::default());
        let HostCandidateClassification::Unique(valid) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected unique")
        };
        assert!(valid.revision().raw() > first.revision().raw());

        let grandparent = world.spawn((Node::default(),)).id();
        let intermediate = world.spawn((ChildOf(grandparent),)).id();
        world.entity_mut(host).insert(ChildOf(intermediate));
        let HostCandidateClassification::InvalidTopology(missing_intermediate_node) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected invalid intermediate ancestry")
        };
        assert_eq!(
            missing_intermediate_node.reason(),
            InvalidHostTopologyReason::MissingNode
        );
        assert!(missing_intermediate_node.revision().raw() > valid.revision().raw());

        world.entity_mut(intermediate).insert(Node::default());
        let HostCandidateClassification::Unique(deep_valid) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected valid all-Node ancestry")
        };
        assert_eq!(deep_valid.ancestry(), &[host, intermediate, grandparent]);
        assert!(deep_valid.revision().raw() > missing_intermediate_node.revision().raw());

        world.entity_mut(grandparent).insert(ChildOf(host));
        let HostCandidateClassification::InvalidTopology(cyclic) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected cycle")
        };
        assert_eq!(
            cyclic.reason(),
            InvalidHostTopologyReason::RelationshipCycle
        );
        assert!(cyclic.revision().raw() > deep_valid.revision().raw());
    }

    #[test]
    fn rejection_latch_attempts_a_revision_once_and_reopens_only_on_change() {
        let first = HostCandidateRevision::fixture(3);
        let second = HostCandidateRevision::fixture(4);
        let mut latch = HostCandidateRevisionLatch::default();
        assert!(latch.allows(first));
        latch.reject(router(), first).unwrap();
        assert!(!latch.allows(first));
        assert!(latch.allows(second));
        latch.reject(router(), second).unwrap();
        assert!(!latch.allows(second));
    }

    #[test]
    fn registration_reinsertion_and_conflict_membership_produce_new_revisions() {
        let mut world = World::new();
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(first) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected unique")
        };

        let registration = world
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .expect("registration should be present");
        world.entity_mut(host).insert(registration);
        let HostCandidateClassification::Unique(reinserted) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected unique")
        };
        assert!(reinserted.revision().raw() > first.revision().raw());

        let second = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let HostCandidateClassification::Conflict(conflict) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected conflict")
        };
        let conflict_revision = conflict.revision();
        world.entity_mut(second).remove::<RouterHostRegistration>();
        let HostCandidateClassification::Unique(restored) =
            classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("expected unique")
        };
        assert!(restored.revision().raw() > conflict_revision.raw());
    }

    #[test]
    fn candidate_revision_exhaustion_is_bounded_world_zero() {
        let mut world = World::new();
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let registration_incarnation = world
            .get::<RouterHostRegistration>(host)
            .unwrap()
            .incarnation();
        let topology = inspect_topology(&world, host).unwrap();
        let candidate_fingerprint = CandidateFingerprint::Single(topology.clone());
        let host_manifest = ComponentManifest::from_world(&world, host).unwrap();
        let entity_count = world.iter_entities().count();
        let mut revisions = HostCandidateRevisionState {
            next: u64::MAX,
            observed: Vec::new(),
        };

        let HostCandidateRevisionObservation::IdentityExhausted {
            source,
            attempted_counter,
        } = revisions.observe(router(), &candidate_fingerprint)
        else {
            panic!("candidate revision exhaustion must return typed permanent source evidence")
        };
        let Some((observed_router, observed_incarnation, revision, fingerprint, observed_topology)) =
            source.unique_projection()
        else {
            panic!("the real unique candidate must remain the exhaustion source")
        };
        assert_eq!(observed_router, router());
        assert_eq!(observed_incarnation, registration_incarnation);
        assert_eq!(revision, HostCandidateRevision::fixture(u64::MAX));
        assert_eq!(fingerprint, &candidate_fingerprint);
        assert_eq!(observed_topology, &topology);
        assert_eq!(attempted_counter, u64::MAX);
        assert_eq!(world.iter_entities().count(), entity_count);
        assert_eq!(
            ComponentManifest::from_world(&world, host),
            Some(host_manifest)
        );
        assert!(children_of(&world, host).is_empty());
    }

    #[test]
    fn exhausted_registration_incarnation_never_becomes_a_candidate_identity() {
        let mut world = World::new();
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let evidence = HostRegistrationIncarnationExhaustionEvidence {
            current: u64::MAX,
            attempted: u64::MAX,
        };
        world
            .get_mut::<RouterHostRegistration>(host)
            .unwrap()
            .testing_mark_incarnation_exhausted(evidence);
        let entity_count = world.iter_entities().count();
        let mut revisions = HostCandidateRevisionState::default();

        let HostCandidateClassification::IdentityExhausted {
            source,
            counter,
            current,
            attempted,
        } = classify_host_candidate(&world, router(), &mut revisions)
        else {
            panic!("an exhausted registration must fail permanently before candidate admission")
        };

        assert_eq!(
            counter,
            HostCandidateIdentityCounter::RegistrationIncarnation
        );
        assert_eq!(current, u64::MAX);
        assert_eq!(attempted, u64::MAX);
        assert_eq!(source.revision(), HostCandidateRevision::fixture(0));
        assert_eq!(world.iter_entities().count(), entity_count);
        assert_eq!(revisions.next, 0);
        assert!(revisions.observed.is_empty());
    }

    #[test]
    fn same_tick_registration_reinsertion_invalidates_the_sealed_candidate() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);
        let entity_count = world.entities().len();

        let registration = world
            .entity_mut(host)
            .take::<RouterHostRegistration>()
            .expect("registration should be present");
        world.entity_mut(host).insert(registration);

        let error = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::None),
        )
        .unwrap_err();
        assert_eq!(error.kind, ProvisionalStageErrorKind::StaleCandidate);
        assert_eq!(world.entities().len(), entity_count);
        assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
        assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
        assert!(children_of(&world, host).is_empty());
    }

    #[test]
    fn exhausted_seal_rejects_host_staging_without_allocating() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let candidate = unique_candidate(&world, &mut HostCandidateRevisionState::default());
        world
            .resource_mut::<RouteEntityRegistry>()
            .exhaust_seal_sequence_for_test();
        let entities_before = world
            .iter_entities()
            .map(|entity| entity.id())
            .collect::<Vec<_>>();
        for _ in 0..2 {
            let error = stage_provisional_top_slot(
                &mut world,
                SealedProvisionalTopSlotSpec::for_candidate(&candidate),
            )
            .expect_err("exhausted ownership identity must reject before allocation");
            assert_eq!(error.kind, ProvisionalStageErrorKind::RegistryRejected);
            assert!(error.entity.is_none());
            assert!(error.settlement.is_none());
            assert_eq!(
                world
                    .iter_entities()
                    .map(|entity| entity.id())
                    .collect::<Vec<_>>(),
                entities_before
            );
            assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
            assert!(children_of(&world, host).is_empty());
        }
    }

    fn unique_candidate(
        world: &World,
        revisions: &mut HostCandidateRevisionState,
    ) -> ValidatedHostCandidate {
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(world, router(), revisions)
        else {
            panic!("expected unique")
        };
        candidate
    }

    #[test]
    pub(crate) fn staging_registers_and_blocks_input_before_probe_then_attaches_only_the_exact_host_edge()
     {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        world.insert_resource(ProbeAudit::default());
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
        world
            .register_component_hooks::<ProvisionalStagingProbe>()
            .on_add(|mut world, HookContext { entity, .. }| {
                let registered = world
                    .resource::<RouteEntityRegistry>()
                    .record(entity)
                    .is_some();
                let inert = world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
                    && world.get::<Pickable>(entity) == Some(&Pickable::IGNORE)
                    && world
                        .get::<RouterInputBarrier>(entity)
                        .is_some_and(RouterInputBarrier::denies_all);
                world
                    .resource_mut::<ProbeAudit>()
                    .0
                    .push(registered && inert);
            });
        let existing_a = world.spawn_empty().id();
        let existing_b = world.spawn_empty().id();
        let host = world.spawn((Node::default(), AppSentinel)).id();
        world
            .entity_mut(host)
            .add_children(&[existing_a, existing_b]);
        world
            .entity_mut(host)
            .insert(RouterHostRegistration::new(router()));
        let before = world
            .get::<Children>(host)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let host_manifest = ComponentManifest::from_world(&world, host).unwrap();
        let host_node = world.get::<Node>(host).unwrap().clone();
        let host_router = world.get::<RouterHostRegistration>(host).unwrap().router();
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);

        let staged = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::FocusSlot),
        )
        .unwrap();

        assert_eq!(world.resource::<ProbeAudit>().0, vec![true]);
        assert_eq!(
            world.get::<Visibility>(staged.entity()),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(staged.entity()),
            Some(&Pickable::IGNORE)
        );
        assert!(
            world
                .get::<RouterInputBarrier>(staged.entity())
                .unwrap()
                .denies_all()
        );
        let node = world.get::<Node>(staged.entity()).unwrap();
        assert_eq!(node.width, bevy_ui::Val::Percent(100.0));
        assert_eq!(node.height, bevy_ui::Val::Percent(100.0));
        assert_eq!(
            world.get::<ChildOf>(staged.entity()).map(ChildOf::parent),
            Some(host)
        );
        let mut expected = before.clone();
        expected.push(staged.entity());
        assert_eq!(
            world
                .get::<Children>(host)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            expected
        );
        assert!(world.get::<AppSentinel>(host).is_some());
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(staged.entity())
            .unwrap();
        assert!(record.manifest().matches(&world, staged.entity()));

        let slot = staged.entity();
        assert!(settle_rejected_granted_top_slot(
            &mut world, &candidate, staged
        ));
        assert!(world.get_entity(slot).is_err());
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .is_none()
        );
        assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
        assert_eq!(children_of(&world, host), before);
        assert!(world.get_entity(existing_a).is_ok());
        assert!(world.get_entity(existing_b).is_ok());
        assert_eq!(
            ComponentManifest::from_world(&world, host),
            Some(host_manifest)
        );
        assert_eq!(world.get::<Node>(host), Some(&host_node));
        assert_eq!(
            world
                .get::<RouterHostRegistration>(host)
                .map(|value| value.router()),
            Some(host_router)
        );
        assert!(world.get::<AppSentinel>(host).is_some());
    }

    #[test]
    fn staging_first_write_registration_drift_stops_before_node_and_host_edge() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        world.insert_resource(FirstStagingWriteAudit::default());
        world
            .register_component_hooks::<ProvisionalStagingProbe>()
            .on_add(|mut world, HookContext { entity, .. }| {
                let host = world
                    .resource::<FirstStagingWriteAudit>()
                    .host
                    .expect("the real staging hook is armed for one exact host");
                let observation = (
                    world.get::<Node>(entity).is_some(),
                    world.get::<ChildOf>(entity).is_some(),
                );
                world
                    .resource_mut::<FirstStagingWriteAudit>()
                    .observations
                    .push(observation);
                world
                    .commands()
                    .entity(host)
                    .insert(RouterHostRegistration::new(router()));
            });
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        world.resource_mut::<FirstStagingWriteAudit>().host = Some(host);
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);

        let result = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::None),
        );

        assert!(
            result.is_err(),
            "the exact registration changed during staging"
        );
        assert_eq!(
            world.resource::<FirstStagingWriteAudit>().observations,
            vec![(false, false)],
            "the first hook-capable bundle write must flush and refreeze before Node or ChildOf"
        );
        assert!(
            world.get_entity(host).is_ok(),
            "staging cannot mutate the app host"
        );
    }

    #[test]
    fn staging_releases_focus_and_fails_closed_under_post_write_drift() {
        for drift in [
            StagingTestDrift::Despawn,
            StagingTestDrift::RemoveBarrier,
            StagingTestDrift::AddForeignComponent,
            StagingTestDrift::AddForeignDescendant,
        ] {
            let mut world = World::new();
            world.insert_resource(RouteEntityRegistry::new());
            world.insert_resource(QuarantineLeaseTable::new());
            world.insert_resource(InputFocus::default());
            let sentinel = world.spawn((AppSentinel,)).id();
            let host = world
                .spawn((Node::default(), RouterHostRegistration::new(router())))
                .id();
            world.entity_mut(host).add_child(sentinel);
            let baseline = world
                .get::<Children>(host)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let mut revisions = HostCandidateRevisionState::default();
            let candidate = unique_candidate(&world, &mut revisions);
            let result = stage_provisional_top_slot(
                &mut world,
                SealedProvisionalTopSlotSpec::new(&candidate, drift),
            );
            let error = result.unwrap_err();
            let mut ledger = ProvisionalHostCleanupLedger::new(
                error
                    .into_settlement()
                    .expect("every post-allocation staging failure retains its physical owner"),
            );
            assert!(ledger.settle(&mut world));
            let Ok(_terminal) = ledger.into_terminal() else {
                panic!("exact provisional cleanup produces one consuming terminal")
            };
            assert!(world.get_entity(host).is_ok());
            assert!(world.get::<AppSentinel>(sentinel).is_some());
            assert_eq!(
                world
                    .get::<Children>(host)
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                baseline
            );
            assert_eq!(world.resource::<InputFocus>().get(), None);
            if matches!(
                drift,
                StagingTestDrift::AddForeignComponent | StagingTestDrift::AddForeignDescendant
            ) {
                let leases = world.resource::<QuarantineLeaseTable>().leases();
                assert_eq!(leases.len(), 1);
                let slot = leases[0].owned_entities()[0];
                assert_eq!(world.get::<Visibility>(slot), Some(&Visibility::Hidden));
                assert_eq!(world.get::<Pickable>(slot), Some(&Pickable::IGNORE));
                assert!(world.get::<RouterInputBarrier>(slot).unwrap().denies_all());
                if drift == StagingTestDrift::AddForeignComponent {
                    assert!(world.get::<StagingForeignComponent>(slot).is_some());
                } else {
                    let foreign = leases[0].foreign_descendants()[0];
                    assert!(world.get_entity(foreign).is_ok());
                    assert_eq!(
                        world.get::<ChildOf>(foreign).map(ChildOf::parent),
                        Some(slot)
                    );
                }
            }
        }
    }

    #[test]
    fn stale_candidate_is_rejected_before_allocation_or_app_topology_mutation() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let sentinel = world.spawn((AppSentinel,)).id();
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        world.entity_mut(host).add_child(sentinel);
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);
        let baseline = children_of(&world, host);
        world.entity_mut(host).remove::<Node>();

        let error = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::None),
        )
        .unwrap_err();

        assert_eq!(error.kind, ProvisionalStageErrorKind::StaleCandidate);
        assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
        assert_eq!(children_of(&world, host), baseline);
        assert!(world.get::<AppSentinel>(sentinel).is_some());
        assert!(
            world.get::<Node>(host).is_none(),
            "Router must not repair app topology"
        );
    }

    #[test]
    fn removed_or_foreign_router_registration_is_stale_and_zero_write() {
        for replacement in [None, Some(RouterHandle::fixture(99, 101))] {
            let mut world = World::new();
            world.insert_resource(RouteEntityRegistry::new());
            world.insert_resource(QuarantineLeaseTable::new());
            let sentinel = world.spawn((AppSentinel,)).id();
            let host = world
                .spawn((Node::default(), RouterHostRegistration::new(router())))
                .id();
            world.entity_mut(host).add_child(sentinel);
            let baseline_children = children_of(&world, host);
            let mut revisions = HostCandidateRevisionState::default();
            let candidate = unique_candidate(&world, &mut revisions);
            world.entity_mut(host).remove::<RouterHostRegistration>();
            if let Some(replacement) = replacement {
                world
                    .entity_mut(host)
                    .insert(RouterHostRegistration::new(replacement));
            }
            let entity_count = world.iter_entities().count();

            let error = stage_provisional_top_slot(
                &mut world,
                SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::None),
            )
            .unwrap_err();

            assert_eq!(error.kind, ProvisionalStageErrorKind::StaleCandidate);
            assert_eq!(world.iter_entities().count(), entity_count);
            assert_eq!(children_of(&world, host), baseline_children);
            assert!(world.get::<AppSentinel>(sentinel).is_some());
            assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
            assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
        }
    }

    #[test]
    fn quarantine_detach_gate_failure_is_zero_write_for_the_lease_table() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);

        let error = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(
                &candidate,
                StagingTestDrift::ReattachAuthorizedHostOnDetach,
            ),
        )
        .unwrap_err();

        assert_eq!(error.kind, ProvisionalStageErrorKind::PostWriteDrift);
        let slot = error.entity().unwrap();
        let mut settlement = error
            .into_settlement()
            .expect("the post-write rejection retains its physical owner");
        assert!(matches!(
            settle_failed_provisional(
                &mut world,
                &mut settlement,
                StagingTestDrift::ReattachAuthorizedHostOnDetach,
            ),
            ProvisionalSettlementAttempt::Inert(ProvisionalHostSettlementMismatch::DetachBlocked)
        ));
        assert_eq!(world.get::<ChildOf>(slot).map(ChildOf::parent), Some(host));
        assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .is_some()
        );
        assert_eq!(
            world.get::<ProvisionalSlotLifecycle>(slot),
            Some(&ProvisionalSlotLifecycle::PreparedInert)
        );
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .map(|record| record.lifecycle()),
            Some(EntityLifecycle::PreparedInert)
        );
    }

    #[test]
    fn actual_post_write_component_hook_drift_is_detected_and_settled() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        world
            .register_component_hooks::<ProvisionalPostWriteProbe>()
            .on_add(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .remove::<RouterInputBarrier>();
            });
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let baseline = children_of(&world, host);
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);

        let error = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::None),
        )
        .unwrap_err();

        assert_eq!(error.kind, ProvisionalStageErrorKind::PostWriteDrift);
        let mut ledger = ProvisionalHostCleanupLedger::new(
            error
                .into_settlement()
                .expect("the post-write rejection retains its physical owner"),
        );
        assert!(ledger.settle(&mut world));
        let Ok(_terminal) = ledger.into_terminal() else {
            panic!("exact cleanup consumes the retained provisional owner")
        };
        assert_eq!(children_of(&world, host), baseline);
        assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
        assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
    }

    #[test]
    fn settlement_refreezes_parent_after_second_isolation_write() {
        #[derive(Resource, Default)]
        struct ReparentOnSecondIsolation {
            armed: bool,
            target: Option<Entity>,
            protected_parent: Option<Entity>,
            insertions: usize,
        }

        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        world.insert_resource(ReparentOnSecondIsolation::default());
        world
            .register_component_hooks::<RouterInputBarrier>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let reparent = {
                    let mut drift = world.resource_mut::<ReparentOnSecondIsolation>();
                    if !drift.armed || drift.target != Some(entity) {
                        None
                    } else {
                        drift.insertions += 1;
                        (drift.insertions == 2).then_some(
                            drift
                                .protected_parent
                                .expect("the armed cleanup hook has one protected parent"),
                        )
                    }
                };
                if let Some(parent) = reparent {
                    world.commands().entity(parent).add_child(entity);
                }
            });
        let protected = world.spawn((AppSentinel, Node::default())).id();
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);
        let staged = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::None),
        )
        .expect("the cleanup fixture starts from one valid provisional slot");
        let slot = staged.entity();
        let mut settlement = staged.into_settlement(
            &world,
            &candidate,
            ProvisionalTopSlotSettlementStage::ProbeWritten,
        );
        world.resource_mut::<ReparentOnSecondIsolation>().armed = true;
        world.resource_mut::<ReparentOnSecondIsolation>().target = Some(slot);
        world
            .resource_mut::<ReparentOnSecondIsolation>()
            .protected_parent = Some(protected);

        let attempt =
            settle_failed_provisional(&mut world, &mut settlement, StagingTestDrift::None);

        assert!(matches!(
            attempt,
            ProvisionalSettlementAttempt::Terminal(ProvisionalHostSettlementReceipt {
                disposition: ProvisionalHostSettlementDisposition::Quarantined,
                ..
            })
        ));
        assert_eq!(world.resource::<ReparentOnSecondIsolation>().insertions, 2);
        assert_eq!(
            world.get::<ChildOf>(slot).map(ChildOf::parent),
            Some(protected)
        );
        assert!(world.get::<AppSentinel>(protected).is_some());
        assert_eq!(world.resource::<QuarantineLeaseTable>().leases().len(), 1);
    }

    #[test]
    fn protected_parent_drift_is_quarantined_without_rewriting_the_parent_edge() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let protected = world.spawn((AppSentinel, Node::default())).id();
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router())))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let candidate = unique_candidate(&world, &mut revisions);
        let error = stage_provisional_top_slot(
            &mut world,
            SealedProvisionalTopSlotSpec::new(&candidate, StagingTestDrift::Reparent(protected)),
        )
        .unwrap_err();
        let slot = error.entity().unwrap();
        let mut ledger = ProvisionalHostCleanupLedger::new(
            error
                .into_settlement()
                .expect("the reparented target retains its physical owner"),
        );
        assert!(ledger.settle(&mut world));
        let Ok(_terminal) = ledger.into_terminal() else {
            panic!("the protected-parent disposition terminalizes as quarantine")
        };
        assert!(world.get_entity(host).is_ok());
        assert!(children_of(&world, host).is_empty());
        assert_eq!(
            world.get::<ChildOf>(slot).map(ChildOf::parent),
            Some(protected)
        );
        assert!(world.get::<AppSentinel>(protected).is_some());
        assert_eq!(world.resource::<QuarantineLeaseTable>().leases().len(), 1);
    }
}
