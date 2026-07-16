#![allow(dead_code)]

use std::collections::HashSet;

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

use crate::app_ext::{RouterHandle, RouterHostRegistration};
use crate::components::{
    ProvisionalPostWriteProbe, ProvisionalSlotLifecycle, ProvisionalStagingProbe,
    RouteOwnershipSeal, RouterInputBarrier,
};
use crate::input::RouteFocusState;
use crate::registry::{
    ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim, ProvisionalOwner,
    ProvisionalTopSlotOwner, QuarantineFacts, QuarantineLeaseTable, RegistryOwner,
    RouteEntityRegistry, apply_prepared_quarantine_transfer, prepare_verified_quarantine_transfer,
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
struct HostTopologyFingerprint {
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
enum CandidateFingerprint {
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
}

impl ValidatedHostCandidate {
    pub(crate) const fn host(&self) -> Entity {
        self.host
    }
    pub(crate) fn ancestry(&self) -> &[Entity] {
        &self.ancestry
    }
    pub(crate) const fn revision(&self) -> HostCandidateRevision {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InvalidHostCandidate {
    host: Entity,
    fingerprint: HostTopologyFingerprint,
    revision: HostCandidateRevision,
}

impl InvalidHostCandidate {
    pub(crate) const fn reason(&self) -> InvalidHostTopologyReason {
        match self.fingerprint.outcome {
            TopologyOutcome::Invalid(reason) => reason,
            TopologyOutcome::Valid => unreachable!(),
        }
    }
    pub(crate) const fn revision(&self) -> HostCandidateRevision {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConflictingHostCandidates {
    candidates: Vec<Entity>,
    revision: HostCandidateRevision,
}

impl ConflictingHostCandidates {
    pub(crate) fn candidates(&self) -> &[Entity] {
        &self.candidates
    }
    pub(crate) const fn revision(&self) -> HostCandidateRevision {
        self.revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostCandidateClassification {
    None,
    Unique(ValidatedHostCandidate),
    InvalidTopology(InvalidHostCandidate),
    Conflict(ConflictingHostCandidates),
}

#[derive(Default, Resource)]
pub(crate) struct HostCandidateRevisionState {
    next: u64,
    observed: Vec<(RouterHandle, CandidateFingerprint, HostCandidateRevision)>,
}

impl HostCandidateRevisionState {
    fn observe(
        &mut self,
        router: RouterHandle,
        fingerprint: &CandidateFingerprint,
    ) -> HostCandidateRevision {
        if let Some((_, _, revision)) = self
            .observed
            .iter()
            .find(|(stored, prior, _)| *stored == router && prior == fingerprint)
        {
            return *revision;
        }
        self.next = self
            .next
            .checked_add(1)
            .expect("host candidate revision exhausted");
        let revision = HostCandidateRevision(self.next);
        if let Some(entry) = self
            .observed
            .iter_mut()
            .find(|(stored, _, _)| *stored == router)
        {
            *entry = (router, fingerprint.clone(), revision);
        } else {
            self.observed.push((router, fingerprint.clone(), revision));
        }
        revision
    }
}

#[derive(Default, Resource)]
pub(crate) struct HostCandidateRevisionLatch {
    rejected: Vec<HostCandidateRevision>,
}

impl HostCandidateRevisionLatch {
    pub(crate) fn allows(&self, revision: HostCandidateRevision) -> bool {
        !self.rejected.contains(&revision)
    }
    pub(crate) fn reject(&mut self, revision: HostCandidateRevision) {
        if !self.rejected.contains(&revision) {
            self.rejected.push(revision);
        }
    }
}

pub(crate) fn classify_host_candidate(
    world: &World,
    router: RouterHandle,
    revisions: &mut HostCandidateRevisionState,
) -> HostCandidateClassification {
    let mut candidates = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<RouterHostRegistration>()
                .is_some_and(|registration| registration.router() == router)
                .then_some(entity.id())
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entity| entity.to_bits());

    match candidates.as_slice() {
        [] => HostCandidateClassification::None,
        [candidate] => {
            let fingerprint = inspect_topology(world, *candidate)
                .expect("candidate came from the registration scan");
            let revision =
                revisions.observe(router, &CandidateFingerprint::Single(fingerprint.clone()));
            let ancestry = fingerprint
                .ancestry
                .iter()
                .map(|fact| fact.entity)
                .collect();
            match fingerprint.outcome {
                TopologyOutcome::Valid => {
                    HostCandidateClassification::Unique(ValidatedHostCandidate {
                        router,
                        host: *candidate,
                        ancestry,
                        fingerprint,
                        revision,
                    })
                }
                TopologyOutcome::Invalid(_) => {
                    HostCandidateClassification::InvalidTopology(InvalidHostCandidate {
                        host: *candidate,
                        fingerprint,
                        revision,
                    })
                }
            }
        }
        _ => {
            let facts = candidates
                .iter()
                .map(|candidate| {
                    let registration = world
                        .entity(*candidate)
                        .get::<RouterHostRegistration>()
                        .expect("candidate came from registration scan");
                    (*candidate, registration.incarnation())
                })
                .collect::<Vec<_>>();
            let revision = revisions.observe(router, &CandidateFingerprint::Conflict(facts));
            HostCandidateClassification::Conflict(ConflictingHostCandidates {
                candidates,
                revision,
            })
        }
    }
}

fn inspect_topology(world: &World, candidate: Entity) -> Option<HostTopologyFingerprint> {
    let mut ancestry = Vec::new();
    let mut visited = HashSet::new();
    let mut current = candidate;
    let registration = world
        .get_entity(candidate)
        .ok()?
        .get::<RouterHostRegistration>()?;
    let registration_incarnation = registration.incarnation();
    loop {
        if !visited.insert(current) {
            return Some(HostTopologyFingerprint {
                candidate,
                registration_incarnation,
                ancestry,
                outcome: TopologyOutcome::Invalid(InvalidHostTopologyReason::RelationshipCycle),
            });
        }
        let Ok(entity) = world.get_entity(current) else {
            ancestry.push(HostAncestryFact {
                entity: current,
                exists: false,
                has_node: false,
                actual_parent: None,
            });
            return Some(HostTopologyFingerprint {
                candidate,
                registration_incarnation,
                ancestry,
                outcome: TopologyOutcome::Invalid(InvalidHostTopologyReason::MissingEntity),
            });
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
            return Some(HostTopologyFingerprint {
                candidate,
                registration_incarnation,
                ancestry,
                outcome: TopologyOutcome::Invalid(InvalidHostTopologyReason::MissingNode),
            });
        }
        let Some(parent) = parent else { break };
        current = parent;
    }
    Some(HostTopologyFingerprint {
        candidate,
        registration_incarnation,
        ancestry,
        outcome: TopologyOutcome::Valid,
    })
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
}

impl VerifiedQuarantineTransfer {
    fn after_world_validation(
        claims: Vec<ExactRegistryClaim>,
        manifests: Vec<(Entity, ComponentManifest)>,
        facts: QuarantineFacts,
    ) -> Self {
        Self {
            claims,
            manifests,
            facts,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<ExactRegistryClaim>,
        Vec<(Entity, ComponentManifest)>,
        QuarantineFacts,
    ) {
        (self.claims, self.manifests, self.facts)
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
        }
    }
}

pub(crate) struct SealedProvisionalTopSlotSpec {
    candidate: ValidatedHostCandidate,
    test_drift: StagingTestDrift,
}

impl SealedProvisionalTopSlotSpec {
    pub(crate) const fn for_candidate(candidate: ValidatedHostCandidate) -> Self {
        Self {
            candidate,
            test_drift: StagingTestDrift::None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn new(
        candidate: ValidatedHostCandidate,
        test_drift: StagingTestDrift,
    ) -> Self {
        Self {
            candidate,
            test_drift,
        }
    }
}

pub(crate) fn stage_granted_top_slot(
    world: &mut World,
    candidate: ValidatedHostCandidate,
    grant: &bevy_ui_router_core::integration::HostGrant,
) -> Result<StagedProvisionalTopSlot, ProvisionalStageError> {
    let staged = stage_provisional_top_slot(
        world,
        SealedProvisionalTopSlotSpec::for_candidate(candidate),
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
        world
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(&claim);
        world.despawn(staged.entity());
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::RegistryRejected,
            entity: Some(staged.entity()),
            settlement: None,
        });
    }
    Ok(staged)
}

pub(crate) fn activate_granted_top_slot(
    world: &mut World,
    candidate: &ValidatedHostCandidate,
    grant: &bevy_ui_router_core::integration::HostGrant,
    staged: StagedProvisionalTopSlot,
) -> bool {
    if !validate_granted_top_slot(world, candidate, grant, staged.entity, Visibility::Hidden) {
        return false;
    }
    *world
        .get_mut::<Visibility>(staged.entity)
        .expect("validated granted slot retains Visibility") = Visibility::Visible;
    validate_granted_top_slot(world, candidate, grant, staged.entity, Visibility::Visible)
}

pub(crate) fn settle_rejected_granted_top_slot(
    world: &mut World,
    candidate: &ValidatedHostCandidate,
    staged: StagedProvisionalTopSlot,
) -> bool {
    let Some(record) = world
        .resource::<RouteEntityRegistry>()
        .record(staged.entity)
    else {
        return world.get_entity(staged.entity).is_err();
    };
    let claim = record.claim();
    let manifest = record.manifest().clone();
    settle_failed_provisional(
        world,
        staged.entity,
        candidate.host,
        &manifest,
        claim,
        StagingTestDrift::None,
    )
}

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
        && record.manifest().matches(world, slot)
        && candidate_is_current(world, candidate)
        && world.get::<ChildOf>(slot).map(ChildOf::parent) == Some(candidate.host)
        && children_of(world, candidate.host)
            .iter()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StagedProvisionalTopSlot {
    entity: Entity,
}

impl StagedProvisionalTopSlot {
    pub(crate) const fn entity(self) -> Entity {
        self.entity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalStageErrorKind {
    MissingFoundation,
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
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProvisionalHostSettlement {
    slot: Entity,
    authorized_host: Entity,
    manifest: ComponentManifest,
    claim: ExactRegistryClaim,
}

pub(crate) fn retry_provisional_host_settlement(
    world: &mut World,
    settlement: &ProvisionalHostSettlement,
) -> bool {
    if world.get_entity(settlement.slot).is_err() {
        world
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(&settlement.claim);
        return true;
    }
    let still_exact = world
        .resource::<RouteEntityRegistry>()
        .record(settlement.slot)
        .is_some_and(|record| {
            record.claim() == settlement.claim && record.manifest() == &settlement.manifest
        });
    if !still_exact {
        return true;
    }
    settle_failed_provisional(
        world,
        settlement.slot,
        settlement.authorized_host,
        &settlement.manifest,
        settlement.claim.clone(),
        StagingTestDrift::None,
    )
}

pub(crate) fn stage_provisional_top_slot(
    world: &mut World,
    spec: SealedProvisionalTopSlotSpec,
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
    if !candidate_is_current(world, &spec.candidate) {
        return Err(ProvisionalStageError {
            kind: ProvisionalStageErrorKind::StaleCandidate,
            entity: None,
            settlement: None,
        });
    }

    let host = spec.candidate.host;
    let host_children_baseline = children_of(world, host);
    let manifest = provisional_top_slot_manifest(world);
    let slot = world.spawn_empty().id();
    let (seal, owner, claim) = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        let seal = registry.issue_seal();
        let owner = ProvisionalTopSlotOwner::new(spec.candidate.revision);
        if registry
            .register(
                slot,
                RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone())),
                seal,
                manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .is_err()
        {
            world.despawn(slot);
            return Err(ProvisionalStageError {
                kind: ProvisionalStageErrorKind::RegistryRejected,
                entity: Some(slot),
                settlement: None,
            });
        }
        let claim = registry
            .record(slot)
            .expect("newly registered provisional slot must exist")
            .claim();
        (seal, owner, claim)
    };

    world.entity_mut(slot).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
        RouteOwnershipSeal::new(seal),
        ProvisionalSlotLifecycle::PreparedInert,
        ProvisionalStagingProbe,
    ));
    if world.get_entity(slot).is_ok() {
        world.entity_mut(slot).insert(Node {
            width: bevy_ui::Val::Percent(100.0),
            height: bevy_ui::Val::Percent(100.0),
            ..Node::default()
        });
    }
    if world.get_entity(slot).is_ok() {
        world.entity_mut(slot).insert(ChildOf(host));
    }
    if world.get_entity(slot).is_ok() {
        world.entity_mut(slot).insert(ProvisionalPostWriteProbe);
        world.flush();
    }
    apply_staging_test_drift(world, slot, spec.test_drift);
    clear_focus_in_subtree(world, slot);

    let valid = candidate_is_current(world, &spec.candidate)
        && validate_provisional_slot(
            world,
            slot,
            host,
            &host_children_baseline,
            &owner,
            seal,
            &claim,
            &manifest,
        );
    if valid {
        return Ok(StagedProvisionalTopSlot { entity: slot });
    }

    let settlement = ProvisionalHostSettlement {
        slot,
        authorized_host: host,
        manifest: manifest.clone(),
        claim: claim.clone(),
    };
    let settled = settle_failed_provisional(world, slot, host, &manifest, claim, spec.test_drift);
    Err(ProvisionalStageError {
        kind: if settled {
            ProvisionalStageErrorKind::PostWriteDrift
        } else {
            ProvisionalStageErrorKind::QuarantineDetachBlocked
        },
        entity: Some(slot),
        settlement: (!settled).then_some(settlement),
    })
}

fn candidate_is_current(world: &World, candidate: &ValidatedHostCandidate) -> bool {
    world
        .get_entity(candidate.host)
        .ok()
        .and_then(|entity| entity.get::<RouterHostRegistration>())
        .is_some_and(|registration| registration.router() == candidate.router)
        && inspect_topology(world, candidate.host).as_ref() == Some(&candidate.fingerprint)
}

pub(crate) fn validated_host_candidate_is_current(
    world: &World,
    candidate: &ValidatedHostCandidate,
) -> bool {
    candidate_is_current(world, candidate)
}

fn provisional_top_slot_manifest(world: &mut World) -> ComponentManifest {
    let explicit = vec![
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
    let mut complete = explicit.clone();
    for component in explicit {
        if let Some(required) = world.get_required_components_by_id(component) {
            complete.extend(required.iter_ids());
        }
    }
    ComponentManifest::new(complete)
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
    let registry_valid = record.claim() == *claim
        && record.owner()
            == &RegistryOwner::Provisional(ProvisionalOwner::HostTopSlot(owner.clone()))
        && record.seal() == seal
        && record.lifecycle() == EntityLifecycle::PreparedInert
        && record.manifest() == manifest
        && manifest.matches(world, slot);
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
        && children_of(world, slot).is_empty()
        && !focus_is_in_subtree(world, slot);
    let mut expected_children = host_children_baseline.to_vec();
    expected_children.push(slot);
    let children_valid = children_of(world, host) == expected_children;
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

fn settle_failed_provisional(
    world: &mut World,
    slot: Entity,
    authorized_host: Entity,
    manifest: &ComponentManifest,
    prepared_claim: ExactRegistryClaim,
    test_drift: StagingTestDrift,
) -> bool {
    if world.get_entity(slot).is_err() {
        world
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(&prepared_claim);
        return true;
    }

    let still_exact = world
        .resource::<RouteEntityRegistry>()
        .record(slot)
        .is_some_and(|record| record.claim() == prepared_claim && record.manifest() == manifest);
    if !still_exact {
        return true;
    }

    world.entity_mut(slot).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
        ProvisionalSlotLifecycle::Quarantined,
    ));
    clear_focus_in_subtree(world, slot);

    let initial_parent = world.get::<ChildOf>(slot).map(ChildOf::parent);
    if initial_parent == Some(authorized_host) {
        world.entity_mut(slot).remove::<ChildOf>();
        if test_drift == StagingTestDrift::ReattachAuthorizedHostOnDetach
            && world.get_entity(slot).is_ok()
        {
            world.entity_mut(slot).insert(ChildOf(authorized_host));
        }
    }
    if world.get_entity(slot).is_err() {
        world
            .resource_mut::<RouteEntityRegistry>()
            .remove_exact(&prepared_claim);
        return true;
    }
    let actual_parent = world.get::<ChildOf>(slot).map(ChildOf::parent);
    if actual_parent == Some(authorized_host) {
        world
            .entity_mut(slot)
            .insert(ProvisionalSlotLifecycle::PreparedInert);
        return false;
    }
    world.entity_mut(slot).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
        ProvisionalSlotLifecycle::Quarantined,
    ));
    clear_focus_in_subtree(world, slot);
    let foreign_descendants = collect_descendants(world, slot);
    let actual_manifest =
        ComponentManifest::from_world(world, slot).unwrap_or_else(|| ComponentManifest::new([]));
    let foreign_components = actual_manifest
        .component_ids()
        .iter()
        .copied()
        .filter(|id| !manifest.component_ids().contains(id))
        .collect::<Vec<ComponentId>>();
    let protected_parent = actual_parent;

    if protected_parent.is_some()
        || !foreign_descendants.is_empty()
        || !foreign_components.is_empty()
    {
        let facts = QuarantineFacts::new(
            Some(authorized_host),
            protected_parent,
            foreign_descendants,
            foreign_components,
        );
        let transfer = VerifiedQuarantineTransfer::after_world_validation(
            vec![prepared_claim],
            vec![(slot, actual_manifest)],
            facts,
        );
        world.resource_scope(
            |world, mut registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
                world.resource_scope(
                    |_world, mut table: bevy_ecs::prelude::Mut<QuarantineLeaseTable>| {
                        let Ok(transfer_plan) =
                            prepare_verified_quarantine_transfer(&mut table, &registry, transfer)
                        else {
                            return false;
                        };
                        apply_prepared_quarantine_transfer(
                            &mut table,
                            &mut registry,
                            transfer_plan,
                        );
                        true
                    },
                )
            },
        )
    } else {
        let removed = world.despawn(slot);
        removed
            && world
                .resource_mut::<RouteEntityRegistry>()
                .remove_exact(&prepared_claim)
    }
}

fn children_of(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map_or_else(Vec::new, |children| children.iter().copied().collect())
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
    let mut visited = HashSet::new();
    loop {
        if current == root {
            return true;
        }
        if !visited.insert(current) {
            return false;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        current = parent;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::app_ext::{RouterHandle, RouterHostRegistration};
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

    fn router() -> RouterHandle {
        RouterHandle::fixture(7, 11)
    }

    #[test]
    fn classifies_none_unique_invalid_and_conflicting_candidates_without_world_mutation() {
        let mut world = World::new();
        let mut revisions = HostCandidateRevisionState::default();
        assert!(matches!(
            classify_host_candidate(&world, router(), &mut revisions),
            HostCandidateClassification::None
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
        latch.reject(first);
        assert!(!latch.allows(first));
        assert!(latch.allows(second));
        latch.reject(second);
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
            SealedProvisionalTopSlotSpec::new(candidate, StagingTestDrift::None),
        )
        .unwrap_err();
        assert_eq!(error.kind, ProvisionalStageErrorKind::StaleCandidate);
        assert_eq!(world.entities().len(), entity_count);
        assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
        assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
        assert!(children_of(&world, host).is_empty());
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
            SealedProvisionalTopSlotSpec::new(candidate.clone(), StagingTestDrift::FocusSlot),
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
                SealedProvisionalTopSlotSpec::new(candidate, drift),
            );
            assert!(result.is_err());
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
            SealedProvisionalTopSlotSpec::new(candidate, StagingTestDrift::None),
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
                SealedProvisionalTopSlotSpec::new(candidate, StagingTestDrift::None),
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
                candidate,
                StagingTestDrift::ReattachAuthorizedHostOnDetach,
            ),
        )
        .unwrap_err();

        assert_eq!(
            error.kind,
            ProvisionalStageErrorKind::QuarantineDetachBlocked
        );
        let slot = error.entity().unwrap();
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
            SealedProvisionalTopSlotSpec::new(candidate, StagingTestDrift::None),
        )
        .unwrap_err();

        assert_eq!(error.kind, ProvisionalStageErrorKind::PostWriteDrift);
        assert_eq!(children_of(&world, host), baseline);
        assert!(world.resource::<RouteEntityRegistry>().records().is_empty());
        assert!(world.resource::<QuarantineLeaseTable>().leases().is_empty());
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
            SealedProvisionalTopSlotSpec::new(candidate, StagingTestDrift::Reparent(protected)),
        )
        .unwrap_err();
        let slot = error.entity().unwrap();
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
