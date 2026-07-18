use std::collections::HashSet;

use bevy_app::Inherited;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::ComponentId,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, Val};
use bevy_ui_router_core::{
    frame::RouteFrameVersion,
    ids::RouteSlotId,
    integration::{
        FailureCleanupRequest, MountGrant, MountReceiptProof, MountRegistrationBeginError,
        MountRegistrationPermit, RouterEngine, mount_grant_matches_target,
    },
    slot::RouteSlotKey,
};

use crate::{
    components::{
        ChildSlotAuthority, ChildSlotAuthorityProbe, ChildSlotBarrierProbe, ChildSlotLifecycle,
        ChildSlotNodeProbe, ChildSlotRelationshipProbe, RouteOwnershipSeal, RouterInputBarrier,
    },
    input::RouteFocusState,
    presentation::{RealizedFrame, RealizedOutlet},
    registry::{
        ComponentManifest, EntityLifecycle, ExactRegistryClaim, ProvisionalOwner, RegistryOwner,
        RouteEntityRegistry,
    },
};

#[derive(Debug)]
pub(crate) struct ProvisionalChildSlot {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    anchor: Entity,
    anchor_claim: ExactRegistryClaim,
    prior_anchor_children: Vec<Entity>,
}

impl ProvisionalChildSlot {
    pub(crate) const fn slot(&self) -> Entity {
        self.slot
    }

    pub(crate) const fn slot_claim(&self) -> &ExactRegistryClaim {
        &self.slot_claim
    }

    pub(crate) fn prior_anchor_children(&self) -> &[Entity] {
        &self.prior_anchor_children
    }
}

#[derive(Debug)]
pub(crate) struct MountReceipt {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    slot_manifest: ComponentManifest,
    anchor: Entity,
    anchor_claim: ExactRegistryClaim,
    anchor_manifest: ComponentManifest,
    expected_anchor_children: Vec<Entity>,
    digest: u64,
}

impl MountReceipt {
    pub(crate) const fn digest(&self) -> u64 {
        self.digest
    }
}

#[derive(Debug)]
pub(crate) struct StagedChildSlot {
    provisional: ProvisionalChildSlot,
    receipt: MountReceipt,
}

impl StagedChildSlot {
    pub(crate) const fn slot(&self) -> Entity {
        self.provisional.slot()
    }

    pub(crate) const fn slot_claim(&self) -> &ExactRegistryClaim {
        self.provisional.slot_claim()
    }

    pub(crate) const fn receipt(&self) -> &MountReceipt {
        &self.receipt
    }

    pub(crate) fn into_residual(self, world: &mut World) -> ChildSlotResidual {
        let Self { provisional, .. } = self;
        clear_slot_focus(world, provisional.slot);
        let state = if world.get_entity(provisional.slot).is_err() {
            ChildSlotResidualState::Absent
        } else if matches!(
            world
                .resource::<RouteEntityRegistry>()
                .record(provisional.slot)
                .map(|record| record.owner()),
            Some(RegistryOwner::Committed(
                crate::registry::EntityOwner::ChildSlot { .. }
            ))
        ) {
            ChildSlotResidualState::Registered
        } else {
            ChildSlotResidualState::Provisional
        };
        ChildSlotResidual {
            state,
            provisional,
            authorized_manifest: child_slot_authorized_manifest(world),
        }
    }
}

#[derive(Debug)]
pub(crate) struct MountedChildSlot {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    anchor: Entity,
    anchor_claim: ExactRegistryClaim,
    expected_anchor_children: Vec<Entity>,
    slot_id: RouteSlotId,
}

impl MountedChildSlot {
    pub(crate) const fn slot(&self) -> Entity {
        self.slot
    }

    pub(crate) const fn slot_claim(&self) -> &ExactRegistryClaim {
        &self.slot_claim
    }

    pub(crate) const fn slot_id(&self) -> RouteSlotId {
        self.slot_id
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.outlet
    }

    pub(crate) const fn anchor(&self) -> Entity {
        self.anchor
    }

    pub(crate) const fn anchor_claim(&self) -> &ExactRegistryClaim {
        &self.anchor_claim
    }

    pub(crate) fn expected_anchor_children(&self) -> &[Entity] {
        &self.expected_anchor_children
    }

    #[cfg(test)]
    fn clone_for_test(&self) -> Self {
        Self {
            slot: self.slot,
            slot_claim: self.slot_claim.clone(),
            parent: self.parent.clone(),
            outlet: self.outlet.clone(),
            anchor: self.anchor,
            anchor_claim: self.anchor_claim.clone(),
            expected_anchor_children: self.expected_anchor_children.clone(),
            slot_id: self.slot_id,
        }
    }

    pub(crate) fn internal_claims_can_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> bool {
        self.claim_alias_can_rebind(&self.slot_claim, claims, expected_current, expected_next)
            && self.claim_alias_can_rebind(
                &self.anchor_claim,
                claims,
                expected_current,
                expected_next,
            )
    }

    pub(crate) fn apply_internal_claim_rebind(&mut self, claims: &[ExactRegistryClaim]) {
        if let Some(next) = claims
            .iter()
            .find(|claim| claim.entity() == self.slot_claim.entity())
        {
            self.slot_claim = next.clone();
        }
        if let Some(next) = claims
            .iter()
            .find(|claim| claim.entity() == self.anchor_claim.entity())
        {
            self.anchor_claim = next.clone();
        }
    }

    fn claim_alias_can_rebind(
        &self,
        current: &ExactRegistryClaim,
        claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> bool {
        claims
            .iter()
            .find(|claim| claim.entity() == current.entity())
            .is_some_and(|next| {
                crate::presentation::claim_rebind_is_exact(
                    current,
                    next,
                    expected_current,
                    expected_next,
                )
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildSlotStageErrorKind {
    MissingFoundation,
    GrantMismatch,
    MissingOutlet,
    InvalidOutlet,
    RegistryRejected,
    BarrierDrift,
    NodeDrift,
    AuthorityDrift,
    RelationshipDrift,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildSlotResidualState {
    Provisional,
    Registered,
    Absent,
}

#[derive(Debug)]
pub(crate) struct ChildSlotResidual {
    state: ChildSlotResidualState,
    provisional: ProvisionalChildSlot,
    authorized_manifest: ComponentManifest,
}

impl ChildSlotResidual {
    pub(crate) const fn state(&self) -> ChildSlotResidualState {
        self.state
    }

    pub(crate) const fn entity(&self) -> Entity {
        self.provisional.slot
    }

    pub(crate) const fn claim(&self) -> &ExactRegistryClaim {
        &self.provisional.slot_claim
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.provisional.parent
    }

    pub(crate) const fn outlet(&self) -> &RouteSlotKey {
        &self.provisional.outlet
    }

    pub(crate) const fn anchor(&self) -> Entity {
        self.provisional.anchor
    }

    pub(crate) const fn anchor_claim(&self) -> &ExactRegistryClaim {
        &self.provisional.anchor_claim
    }

    pub(crate) const fn authorized_manifest(&self) -> &ComponentManifest {
        &self.authorized_manifest
    }

    pub(crate) fn prior_anchor_children(&self) -> &[Entity] {
        self.provisional.prior_anchor_children()
    }
}

#[derive(Debug)]
pub(crate) struct ChildSlotStageFailure {
    kind: ChildSlotStageErrorKind,
    residual: Option<ChildSlotResidual>,
}

impl ChildSlotStageFailure {
    pub(crate) const fn kind(&self) -> ChildSlotStageErrorKind {
        self.kind
    }

    pub(crate) fn into_residual(self) -> Option<ChildSlotResidual> {
        self.residual
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChildSlotConfirmationErrorKind {
    RegistrationBegin,
    ReceiptDrift,
    OwnerCasRejected,
    PostCasDrift,
}

pub(crate) struct ChildSlotConfirmationFailure {
    kind: ChildSlotConfirmationErrorKind,
    terminal: ChildSlotConfirmationTerminal,
    residual: ChildSlotResidual,
}

enum ChildSlotConfirmationTerminal {
    Cleanup(FailureCleanupRequest),
    BeginError(MountRegistrationBeginError),
}

impl std::fmt::Debug for ChildSlotConfirmationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChildSlotConfirmationFailure")
            .field("kind", &self.kind)
            .field(
                "terminal",
                &match &self.terminal {
                    ChildSlotConfirmationTerminal::Cleanup(_) => "cleanup",
                    ChildSlotConfirmationTerminal::BeginError(_) => "begin_error",
                },
            )
            .field("residual_state", &self.residual.state)
            .finish()
    }
}

impl ChildSlotConfirmationFailure {
    pub(crate) const fn kind(&self) -> ChildSlotConfirmationErrorKind {
        self.kind
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<FailureCleanupRequest>,
        Option<MountRegistrationBeginError>,
        ChildSlotResidual,
    ) {
        match self.terminal {
            ChildSlotConfirmationTerminal::Cleanup(cleanup) => (Some(cleanup), None, self.residual),
            ChildSlotConfirmationTerminal::BeginError(error) => (None, Some(error), self.residual),
        }
    }
}

pub(crate) fn stage_child_slot(
    world: &mut World,
    parent: &RealizedFrame,
    outlet_key: &RouteSlotKey,
    grant: &MountGrant,
) -> Result<StagedChildSlot, ChildSlotStageFailure> {
    if !world.contains_resource::<RouteEntityRegistry>() {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::MissingFoundation,
            residual: None,
        });
    }
    if !mount_grant_matches_target(grant, parent.frame(), outlet_key) {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::GrantMismatch,
            residual: None,
        });
    }
    let Some(outlet) = parent.outlet(outlet_key).cloned() else {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::MissingOutlet,
            residual: None,
        });
    };
    if outlet.parent() != parent.frame() || !validate_outlet_anchor(world, &outlet) {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::InvalidOutlet,
            residual: None,
        });
    }

    let prior_anchor_children = children_of(world, outlet.anchor());
    let slot = world.spawn_empty().id();
    let initial_manifest = ComponentManifest::from_world(world, slot)
        .expect("a newly allocated empty Entity must have an archetype");
    let slot_claim = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        let seal = registry.issue_seal();
        let owner = RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
            parent: parent.frame().clone(),
            outlet: outlet_key.clone(),
            slot: grant.slot(),
        });
        if registry
            .register(
                slot,
                owner,
                seal,
                initial_manifest.clone(),
                EntityLifecycle::PreparedInert,
            )
            .is_err()
        {
            world.despawn(slot);
            return Err(ChildSlotStageFailure {
                kind: ChildSlotStageErrorKind::RegistryRejected,
                residual: None,
            });
        }
        registry
            .record(slot)
            .expect("new child slot registry record must exist")
            .claim()
    };
    let provisional = ProvisionalChildSlot {
        slot,
        slot_claim,
        parent: parent.frame().clone(),
        outlet: outlet_key.clone(),
        anchor: outlet.anchor(),
        anchor_claim: outlet.anchor_claim().clone(),
        prior_anchor_children: prior_anchor_children.clone(),
    };
    if !validate_stage_registry(world, &provisional, &initial_manifest)
        || !initial_manifest.matches(world, slot)
    {
        return Err(stage_failure(
            world,
            ChildSlotStageErrorKind::RegistryRejected,
            provisional,
        ));
    }

    let barrier_ids = vec![
        world.register_component::<Visibility>(),
        world.register_component::<Pickable>(),
        world.register_component::<RouterInputBarrier>(),
        world.register_component::<ChildSlotBarrierProbe>(),
    ];
    if !advance_single_manifest_batch(
        world,
        &provisional,
        &prior_anchor_children,
        barrier_ids,
        |world, slot| {
            world.entity_mut(slot).insert((
                Visibility::Hidden,
                Pickable::IGNORE,
                RouterInputBarrier::deny_all(),
                ChildSlotBarrierProbe,
            ));
        },
    ) || !validate_child_slot_inert(world, &provisional)
    {
        return Err(stage_failure(
            world,
            ChildSlotStageErrorKind::BarrierDrift,
            provisional,
        ));
    }

    let node_ids = vec![
        world.register_component::<Node>(),
        world.register_component::<ChildSlotNodeProbe>(),
    ];
    if !advance_single_manifest_batch(
        world,
        &provisional,
        &prior_anchor_children,
        node_ids,
        |world, slot| {
            world.entity_mut(slot).insert((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                },
                ChildSlotNodeProbe,
            ));
        },
    ) || !validate_child_slot_node(world, &provisional)
    {
        return Err(stage_failure(
            world,
            ChildSlotStageErrorKind::NodeDrift,
            provisional,
        ));
    }

    let authority_ids = vec![
        world.register_component::<RouteOwnershipSeal>(),
        world.register_component::<ChildSlotLifecycle>(),
        world.register_component::<ChildSlotAuthority>(),
        world.register_component::<ChildSlotAuthorityProbe>(),
    ];
    if !advance_single_manifest_batch(
        world,
        &provisional,
        &prior_anchor_children,
        authority_ids,
        |world, slot| {
            world.entity_mut(slot).insert((
                RouteOwnershipSeal::new(provisional.slot_claim.seal()),
                ChildSlotLifecycle::ProvisionalInert,
                ChildSlotAuthority::new(
                    provisional.parent.clone(),
                    provisional.outlet.clone(),
                    grant.slot(),
                ),
                ChildSlotAuthorityProbe,
            ));
        },
    ) || !validate_child_slot_authority(world, &provisional, grant.slot())
    {
        return Err(stage_failure(
            world,
            ChildSlotStageErrorKind::AuthorityDrift,
            provisional,
        ));
    }

    let mut expected_anchor_children = prior_anchor_children;
    expected_anchor_children.push(slot);
    if !advance_relationship_batch(
        world,
        &provisional,
        &expected_anchor_children[..expected_anchor_children.len() - 1],
    ) || !validate_complete_provisional_slot(
        world,
        &provisional,
        grant.slot(),
        &expected_anchor_children,
    ) {
        return Err(stage_failure(
            world,
            ChildSlotStageErrorKind::RelationshipDrift,
            provisional,
        ));
    }

    let registry = world.resource::<RouteEntityRegistry>();
    let slot_manifest = registry
        .record(slot)
        .expect("validated child slot record must exist")
        .manifest()
        .clone();
    let anchor_manifest = registry
        .record(provisional.anchor)
        .expect("validated outlet anchor record must exist")
        .manifest()
        .clone();
    let digest = mount_receipt_digest(
        &provisional,
        grant.slot(),
        &slot_manifest,
        &anchor_manifest,
        &expected_anchor_children,
    );
    let receipt = MountReceipt {
        slot,
        slot_claim: provisional.slot_claim.clone(),
        slot_manifest,
        anchor: provisional.anchor,
        anchor_claim: provisional.anchor_claim.clone(),
        anchor_manifest,
        expected_anchor_children,
        digest,
    };
    Ok(StagedChildSlot {
        provisional,
        receipt,
    })
}

pub(crate) fn confirm_child_slot(
    world: &mut World,
    engine: &mut RouterEngine,
    staged: StagedChildSlot,
    permit: MountRegistrationPermit,
    proof: &MountReceiptProof,
) -> Result<MountedChildSlot, ChildSlotConfirmationFailure> {
    let StagedChildSlot {
        mut provisional,
        receipt,
    } = staged;
    let session = match engine.begin_mount_registration(permit, proof) {
        Ok(session) => session,
        Err(error) => {
            return match error.reject(engine) {
                Ok(cleanup) => Err(confirmation_failure(
                    world,
                    ChildSlotConfirmationErrorKind::RegistrationBegin,
                    provisional,
                    ChildSlotConfirmationTerminal::Cleanup(cleanup),
                )),
                Err(begin_error) => Err(confirmation_failure(
                    world,
                    ChildSlotConfirmationErrorKind::RegistrationBegin,
                    provisional,
                    ChildSlotConfirmationTerminal::BeginError(begin_error),
                )),
            };
        }
    };
    let slot_id = session.slot();
    if !receipt_matches_provisional(world, &provisional, &receipt, slot_id) {
        let cleanup = session.reject();
        return Err(confirmation_failure(
            world,
            ChildSlotConfirmationErrorKind::ReceiptDrift,
            provisional,
            ChildSlotConfirmationTerminal::Cleanup(cleanup),
        ));
    }

    let committed_owner = RegistryOwner::Committed(crate::registry::EntityOwner::ChildSlot {
        parent: provisional.parent.clone(),
        outlet: provisional.outlet.clone(),
        slot: slot_id,
    });
    let committed_claim = world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_owner(&provisional.slot_claim, committed_owner);
    let Some(committed_claim) = committed_claim else {
        let cleanup = session.reject();
        return Err(confirmation_failure(
            world,
            ChildSlotConfirmationErrorKind::OwnerCasRejected,
            provisional,
            ChildSlotConfirmationTerminal::Cleanup(cleanup),
        ));
    };
    provisional.slot_claim = committed_claim.clone();
    if world.get_entity(provisional.slot).is_ok() {
        world
            .entity_mut(provisional.slot)
            .insert(ChildSlotLifecycle::RegisteredInert);
        world.flush();
        clear_slot_focus(world, provisional.slot);
    }
    let mounted = MountedChildSlot {
        slot: provisional.slot,
        slot_claim: committed_claim,
        parent: provisional.parent.clone(),
        outlet: provisional.outlet.clone(),
        anchor: provisional.anchor,
        anchor_claim: provisional.anchor_claim.clone(),
        expected_anchor_children: receipt.expected_anchor_children.clone(),
        slot_id,
    };
    if !validate_registered_slot(world, &mounted, &receipt) {
        let cleanup = session.reject();
        return Err(confirmation_failure(
            world,
            ChildSlotConfirmationErrorKind::PostCasDrift,
            provisional,
            ChildSlotConfirmationTerminal::Cleanup(cleanup),
        ));
    }
    let registration_digest = mounted_registration_digest(&mounted, &receipt);
    session.complete(registration_digest);
    Ok(mounted)
}

fn receipt_matches_provisional(
    world: &World,
    slot: &ProvisionalChildSlot,
    receipt: &MountReceipt,
    slot_id: RouteSlotId,
) -> bool {
    receipt.slot == slot.slot
        && receipt.slot_claim == slot.slot_claim
        && receipt.slot_manifest.matches(world, slot.slot)
        && receipt.anchor == slot.anchor
        && receipt.anchor_claim == slot.anchor_claim
        && receipt.anchor_manifest.matches(world, slot.anchor)
        && children_of(world, slot.anchor) == receipt.expected_anchor_children
        && receipt.digest
            == mount_receipt_digest(
                slot,
                slot_id,
                &receipt.slot_manifest,
                &receipt.anchor_manifest,
                &receipt.expected_anchor_children,
            )
        && matches!(
            slot.slot_claim.owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                parent,
                outlet,
                slot: stored_slot,
            }) if parent == &slot.parent
                && outlet == &slot.outlet
                && *stored_slot == slot_id
        )
        && validate_complete_provisional_slot(
            world,
            slot,
            slot_id,
            &receipt.expected_anchor_children,
        )
        && world
            .resource::<RouteEntityRegistry>()
            .record(slot.slot)
            .is_some_and(|record| {
                record.claim() == slot.slot_claim
                    && record.manifest() == &receipt.slot_manifest
                    && record.manifest().matches(world, slot.slot)
            })
        && world
            .resource::<RouteEntityRegistry>()
            .record(slot.anchor)
            .is_some_and(|record| {
                record.claim() == slot.anchor_claim
                    && record.manifest() == &receipt.anchor_manifest
                    && record.manifest().matches(world, slot.anchor)
            })
}

fn validate_registered_slot(
    world: &World,
    slot: &MountedChildSlot,
    receipt: &MountReceipt,
) -> bool {
    let Some(record) = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(slot.slot))
    else {
        return false;
    };
    record.claim() == slot.slot_claim
        && record.owner()
            == &RegistryOwner::Committed(crate::registry::EntityOwner::ChildSlot {
                parent: slot.parent.clone(),
                outlet: slot.outlet.clone(),
                slot: slot.slot_id,
            })
        && record.lifecycle() == EntityLifecycle::PreparedInert
        && record.manifest() == &receipt.slot_manifest
        && record.manifest().matches(world, slot.slot)
        && world.get::<Visibility>(slot.slot) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(slot.slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot.slot)
            .is_some_and(RouterInputBarrier::denies_all)
        && world.get::<Node>(slot.slot)
            == Some(&Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            })
        && world.get::<RouteOwnershipSeal>(slot.slot)
            == Some(&RouteOwnershipSeal::new(slot.slot_claim.seal()))
        && world.get::<ChildSlotLifecycle>(slot.slot) == Some(&ChildSlotLifecycle::RegisteredInert)
        && world
            .get::<ChildSlotAuthority>(slot.slot)
            .is_some_and(|authority| {
                authority.parent() == &slot.parent
                    && authority.outlet() == &slot.outlet
                    && authority.slot() == slot.slot_id
            })
        && world.get::<ChildOf>(slot.slot).map(ChildOf::parent) == Some(slot.anchor)
        && children_of(world, slot.anchor) == receipt.expected_anchor_children
        && world
            .resource::<RouteEntityRegistry>()
            .record(slot.anchor)
            .is_some_and(|record| {
                record.claim() == slot.anchor_claim
                    && record.manifest() == &receipt.anchor_manifest
                    && record.manifest().matches(world, slot.anchor)
                    && world.get::<RouteOwnershipSeal>(slot.anchor)
                        == Some(&RouteOwnershipSeal::new(record.seal()))
            })
        && !focus_is_in_slot(world, slot.slot)
}

pub(crate) fn mounted_child_slot_is_isolated(world: &World, slot: &MountedChildSlot) -> bool {
    validate_mounted_child_slot(world, slot, EntityLifecycle::PreparedInert)
}

pub(crate) fn validate_mounted_child_slot(
    world: &World,
    slot: &MountedChildSlot,
    lifecycle: EntityLifecycle,
) -> bool {
    validate_mounted_child_slot_with_policy(world, slot, lifecycle, false)
}

pub(crate) fn validate_leaving_mounted_child_slot(world: &World, slot: &MountedChildSlot) -> bool {
    validate_mounted_child_slot_with_policy(world, slot, EntityLifecycle::Active, true)
}

fn validate_mounted_child_slot_with_policy(
    world: &World,
    slot: &MountedChildSlot,
    lifecycle: EntityLifecycle,
    allow_foreign_additions: bool,
) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let Some(record) = registry.record(slot.slot) else {
        return false;
    };
    let Some(anchor_record) = registry.record(slot.anchor) else {
        return false;
    };
    let (component_lifecycle, visibility) = match lifecycle {
        EntityLifecycle::PreparedInert => (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden),
        EntityLifecycle::Active => (ChildSlotLifecycle::Active, Visibility::Visible),
        EntityLifecycle::Leaving => (ChildSlotLifecycle::Leaving, Visibility::Hidden),
        EntityLifecycle::Quarantined => (ChildSlotLifecycle::Quarantined, Visibility::Hidden),
    };
    record.claim() == slot.slot_claim
        && record.lifecycle() == lifecycle
        && manifest_matches_policy(world, slot.slot, record.manifest(), allow_foreign_additions)
        && record.owner()
            == &RegistryOwner::Committed(crate::registry::EntityOwner::ChildSlot {
                parent: slot.parent.clone(),
                outlet: slot.outlet.clone(),
                slot: slot.slot_id,
            })
        && anchor_record.claim() == slot.anchor_claim
        && manifest_matches_policy(
            world,
            slot.anchor,
            anchor_record.manifest(),
            allow_foreign_additions,
        )
        && world.get::<RouteOwnershipSeal>(slot.anchor)
            == Some(&RouteOwnershipSeal::new(anchor_record.seal()))
        && world.get::<Visibility>(slot.slot) == Some(&visibility)
        && world.get::<Pickable>(slot.slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot.slot)
            .is_some_and(RouterInputBarrier::denies_all)
        && world.get::<RouteOwnershipSeal>(slot.slot)
            == Some(&RouteOwnershipSeal::new(slot.slot_claim.seal()))
        && world.get::<ChildSlotLifecycle>(slot.slot) == Some(&component_lifecycle)
        && world
            .get::<ChildSlotAuthority>(slot.slot)
            .is_some_and(|authority| {
                authority.parent() == &slot.parent
                    && authority.outlet() == &slot.outlet
                    && authority.slot() == slot.slot_id
            })
        && world.get::<ChildOf>(slot.slot).map(ChildOf::parent) == Some(slot.anchor)
        && children_of(world, slot.anchor) == slot.expected_anchor_children
        && (lifecycle == EntityLifecycle::Active || !focus_is_in_slot(world, slot.slot))
}

fn manifest_matches_policy(
    world: &World,
    entity: Entity,
    expected: &ComponentManifest,
    allow_foreign_additions: bool,
) -> bool {
    if !allow_foreign_additions {
        return expected.matches(world, entity);
    }
    ComponentManifest::from_world(world, entity).is_some_and(|actual| {
        expected
            .component_ids()
            .iter()
            .all(|component| actual.component_ids().contains(component))
    })
}

pub(crate) fn residual_child_slot_is_isolated(world: &World, residual: &ChildSlotResidual) -> bool {
    if world.get_entity(residual.entity()).is_err() {
        return residual.state == ChildSlotResidualState::Absent;
    }
    let Some(record) = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(residual.entity()))
    else {
        return false;
    };
    if record.claim() != *residual.claim() {
        return false;
    }
    let visibility_is_safe = world
        .get::<Visibility>(residual.entity())
        .is_none_or(|visibility| *visibility == Visibility::Hidden);
    let pickable_is_safe = world
        .get::<Pickable>(residual.entity())
        .is_none_or(|pickable| *pickable == Pickable::IGNORE);
    let barrier_is_safe = world
        .get::<RouterInputBarrier>(residual.entity())
        .is_none_or(RouterInputBarrier::denies_all);
    let seal_is_exact = world
        .get::<RouteOwnershipSeal>(residual.entity())
        .is_none_or(|seal| *seal == RouteOwnershipSeal::new(residual.claim().seal()));
    let lifecycle_is_exact = world
        .get::<ChildSlotLifecycle>(residual.entity())
        .is_none_or(|lifecycle| match residual.state {
            ChildSlotResidualState::Provisional => {
                *lifecycle == ChildSlotLifecycle::ProvisionalInert
            }
            ChildSlotResidualState::Registered => *lifecycle == ChildSlotLifecycle::RegisteredInert,
            ChildSlotResidualState::Absent => false,
        });
    let authority_is_exact = world
        .get::<ChildSlotAuthority>(residual.entity())
        .is_none_or(|authority| {
            let slot = match residual.claim().owner() {
                RegistryOwner::Committed(crate::registry::EntityOwner::ChildSlot {
                    slot, ..
                })
                | RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { slot, .. }) => *slot,
                _ => return false,
            };
            authority.parent() == residual.parent()
                && authority.outlet() == residual.outlet()
                && authority.slot() == slot
        });
    let relationship_is_exact = world
        .get::<ChildOf>(residual.entity())
        .is_none_or(|parent| parent.parent() == residual.anchor());
    visibility_is_safe
        && pickable_is_safe
        && barrier_is_safe
        && seal_is_exact
        && lifecycle_is_exact
        && authority_is_exact
        && relationship_is_exact
}

fn confirmation_failure(
    world: &mut World,
    kind: ChildSlotConfirmationErrorKind,
    provisional: ProvisionalChildSlot,
    terminal: ChildSlotConfirmationTerminal,
) -> ChildSlotConfirmationFailure {
    clear_slot_focus(world, provisional.slot);
    let state = if world.get_entity(provisional.slot).is_err() {
        ChildSlotResidualState::Absent
    } else if matches!(
        world
            .resource::<RouteEntityRegistry>()
            .record(provisional.slot)
            .map(|record| record.owner()),
        Some(RegistryOwner::Committed(
            crate::registry::EntityOwner::ChildSlot { .. }
        ))
    ) {
        ChildSlotResidualState::Registered
    } else {
        ChildSlotResidualState::Provisional
    };
    let authorized_manifest = child_slot_authorized_manifest(world);
    ChildSlotConfirmationFailure {
        kind,
        terminal,
        residual: ChildSlotResidual {
            state,
            provisional,
            authorized_manifest,
        },
    }
}

fn mounted_registration_digest(slot: &MountedChildSlot, receipt: &MountReceipt) -> u64 {
    let mut digest = slot.slot_claim.opaque_digest_part().rotate_left(17)
        ^ slot.anchor_claim.opaque_digest_part().rotate_left(37)
        ^ slot.slot_id.raw().rotate_left(53)
        ^ slot.slot.to_bits()
        ^ slot.anchor.to_bits().rotate_left(29);
    digest = fold_manifest_digest(digest, 0x4d4f_554e_5453_4c4f, &receipt.slot_manifest);
    digest = fold_manifest_digest(digest, 0x4d4f_554e_5441_4e43, &receipt.anchor_manifest);
    fold_entity_order_digest(
        digest,
        0x4d4f_554e_544f_5244,
        &receipt.expected_anchor_children,
    )
}

fn advance_single_manifest_batch(
    world: &mut World,
    slot: &ProvisionalChildSlot,
    frozen_anchor_children: &[Entity],
    explicit: Vec<ComponentId>,
    write: impl FnOnce(&mut World, Entity),
) -> bool {
    let Some(old) = current_exact_manifest(world, slot) else {
        return false;
    };
    if !old.matches(world, slot.slot)
        || !validate_frozen_anchor_children(world, slot, frozen_anchor_children)
    {
        return false;
    }
    let expected = manifest_with_required(world, &old, explicit);
    write(world, slot.slot);
    world.flush();
    clear_slot_focus(world, slot.slot);
    let Some(actual) = ComponentManifest::from_world(world, slot.slot) else {
        return false;
    };
    if actual != expected
        || !validate_stage_registry(world, slot, &old)
        || !validate_frozen_anchor_children(world, slot, frozen_anchor_children)
    {
        return false;
    }
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifest_exact(&slot.slot_claim, &old, expected.clone())
    {
        return false;
    }
    validate_stage_registry(world, slot, &expected)
        && expected.matches(world, slot.slot)
        && validate_frozen_anchor_children(world, slot, frozen_anchor_children)
}

fn validate_frozen_anchor_children(
    world: &World,
    slot: &ProvisionalChildSlot,
    expected: &[Entity],
) -> bool {
    validate_outlet_claim(world, slot) && children_of(world, slot.anchor) == expected
}

fn advance_relationship_batch(
    world: &mut World,
    slot: &ProvisionalChildSlot,
    prior_anchor_children: &[Entity],
) -> bool {
    let Some(old_slot) = current_exact_manifest(world, slot) else {
        return false;
    };
    let Some(old_anchor) = world
        .resource::<RouteEntityRegistry>()
        .record(slot.anchor)
        .filter(|record| record.claim() == slot.anchor_claim)
        .map(|record| record.manifest().clone())
    else {
        return false;
    };
    if !old_slot.matches(world, slot.slot)
        || !old_anchor.matches(world, slot.anchor)
        || children_of(world, slot.anchor) != prior_anchor_children
        || !validate_outlet_claim(world, slot)
    {
        return false;
    }
    let child_of = world.register_component::<ChildOf>();
    let relationship_probe = world.register_component::<ChildSlotRelationshipProbe>();
    let children = world.register_component::<Children>();
    let runtime_ui_components = child_slot_relationship_runtime_ui_component_ids(world);
    let required_slot =
        manifest_with_required(world, &old_slot, vec![child_of, relationship_probe]);
    let authorized_slot = manifest_with_required(
        world,
        &old_slot,
        vec![
            child_of,
            relationship_probe,
            runtime_ui_components[0],
            runtime_ui_components[1],
        ],
    );
    let expected_anchor = manifest_with_required(world, &old_anchor, vec![children]);
    world
        .entity_mut(slot.slot)
        .insert((ChildOf(slot.anchor), ChildSlotRelationshipProbe));
    world.flush();
    clear_slot_focus(world, slot.slot);

    let mut expected_children = prior_anchor_children.to_vec();
    expected_children.push(slot.slot);
    let actual_slot = ComponentManifest::from_world(world, slot.slot);
    let actual_anchor = ComponentManifest::from_world(world, slot.anchor);
    let Some(expected_slot) = actual_slot.filter(|actual| {
        required_slot
            .component_ids()
            .iter()
            .all(|id| actual.component_ids().contains(id))
            && actual
                .component_ids()
                .iter()
                .all(|id| authorized_slot.component_ids().contains(id))
    }) else {
        return false;
    };
    if actual_anchor.as_ref() != Some(&expected_anchor)
        || children_of(world, slot.anchor) != expected_children
        || world.get::<ChildOf>(slot.slot).map(ChildOf::parent) != Some(slot.anchor)
        || !validate_stage_registry(world, slot, &old_slot)
        || !world
            .resource::<RouteEntityRegistry>()
            .record(slot.anchor)
            .is_some_and(|record| {
                record.claim() == slot.anchor_claim && record.manifest() == &old_anchor
            })
    {
        return false;
    }
    let exchanges = [
        (slot.slot_claim.clone(), old_slot, expected_slot.clone()),
        (
            slot.anchor_claim.clone(),
            old_anchor,
            expected_anchor.clone(),
        ),
    ];
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifests_exact(&exchanges)
    {
        return false;
    }
    validate_stage_registry(world, slot, &expected_slot)
        && expected_slot.matches(world, slot.slot)
        && world
            .resource::<RouteEntityRegistry>()
            .record(slot.anchor)
            .is_some_and(|record| {
                record.claim() == slot.anchor_claim
                    && record.manifest() == &expected_anchor
                    && expected_anchor.matches(world, slot.anchor)
            })
        && children_of(world, slot.anchor) == expected_children
        && world.get::<ChildOf>(slot.slot).map(ChildOf::parent) == Some(slot.anchor)
}

fn child_slot_relationship_runtime_ui_component_ids(world: &mut World) -> [ComponentId; 2] {
    [
        world.register_component::<Inherited<ComputedUiTargetCamera>>(),
        world.register_component::<Inherited<ComputedUiRenderTargetInfo>>(),
    ]
}

fn current_exact_manifest(world: &World, slot: &ProvisionalChildSlot) -> Option<ComponentManifest> {
    world
        .resource::<RouteEntityRegistry>()
        .record(slot.slot)
        .filter(|record| record.claim() == slot.slot_claim)
        .map(|record| record.manifest().clone())
}

fn manifest_with_required(
    world: &World,
    old: &ComponentManifest,
    explicit: Vec<ComponentId>,
) -> ComponentManifest {
    let mut ids = old.component_ids().to_vec();
    ids.extend(explicit);
    loop {
        let prior_len = ids.len();
        let current = ids.clone();
        for component in current {
            if let Some(required) = world.get_required_components_by_id(component) {
                ids.extend(required.iter_ids());
            }
        }
        ids.sort_unstable();
        ids.dedup();
        if ids.len() == prior_len {
            break;
        }
    }
    ComponentManifest::new(ids)
}

fn validate_stage_registry(
    world: &World,
    slot: &ProvisionalChildSlot,
    manifest: &ComponentManifest,
) -> bool {
    world
        .resource::<RouteEntityRegistry>()
        .record(slot.slot)
        .is_some_and(|record| {
            record.claim() == slot.slot_claim
                && record.owner()
                    == &RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                        parent: slot.parent.clone(),
                        outlet: slot.outlet.clone(),
                        slot: match slot.slot_claim.owner() {
                            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                                slot,
                                ..
                            }) => *slot,
                            _ => return false,
                        },
                    })
                && record.seal() == slot.slot_claim.seal()
                && record.lifecycle() == EntityLifecycle::PreparedInert
                && record.manifest() == manifest
        })
}

fn validate_outlet_anchor(world: &World, outlet: &RealizedOutlet) -> bool {
    world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(outlet.anchor()))
        .is_some_and(|record| {
            record.claim() == *outlet.anchor_claim()
                && matches!(
                    record.owner(),
                    RegistryOwner::Committed(crate::registry::EntityOwner::FrameOutput {
                        frame,
                        ..
                    }) if frame == outlet.parent()
                )
                && record.manifest().matches(world, outlet.anchor())
                && world.get::<RouteOwnershipSeal>(outlet.anchor())
                    == Some(&RouteOwnershipSeal::new(record.seal()))
        })
}

fn validate_outlet_claim(world: &World, slot: &ProvisionalChildSlot) -> bool {
    world
        .resource::<RouteEntityRegistry>()
        .record(slot.anchor)
        .is_some_and(|record| {
            record.claim() == slot.anchor_claim
                && matches!(
                    record.owner(),
                    RegistryOwner::Committed(crate::registry::EntityOwner::FrameOutput {
                        frame,
                        ..
                    }) if frame == &slot.parent
                )
                && record.manifest().matches(world, slot.anchor)
                && world.get::<RouteOwnershipSeal>(slot.anchor)
                    == Some(&RouteOwnershipSeal::new(record.seal()))
        })
}

fn validate_child_slot_inert(world: &World, slot: &ProvisionalChildSlot) -> bool {
    world.get::<Visibility>(slot.slot) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(slot.slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot.slot)
            .is_some_and(RouterInputBarrier::denies_all)
        && !focus_is_in_slot(world, slot.slot)
}

fn validate_child_slot_node(world: &World, slot: &ProvisionalChildSlot) -> bool {
    validate_child_slot_inert(world, slot)
        && world.get::<Node>(slot.slot).is_some_and(|node| {
            node.width == Val::Percent(100.0) && node.height == Val::Percent(100.0)
        })
}

fn validate_child_slot_authority(
    world: &World,
    slot: &ProvisionalChildSlot,
    slot_id: RouteSlotId,
) -> bool {
    validate_child_slot_node(world, slot)
        && world.get::<RouteOwnershipSeal>(slot.slot)
            == Some(&RouteOwnershipSeal::new(slot.slot_claim.seal()))
        && world.get::<ChildSlotLifecycle>(slot.slot) == Some(&ChildSlotLifecycle::ProvisionalInert)
        && world
            .get::<ChildSlotAuthority>(slot.slot)
            .is_some_and(|authority| {
                authority.parent() == &slot.parent
                    && authority.outlet() == &slot.outlet
                    && authority.slot() == slot_id
            })
}

fn validate_complete_provisional_slot(
    world: &World,
    slot: &ProvisionalChildSlot,
    slot_id: RouteSlotId,
    expected_anchor_children: &[Entity],
) -> bool {
    validate_child_slot_authority(world, slot, slot_id)
        && world.get::<ChildOf>(slot.slot).map(ChildOf::parent) == Some(slot.anchor)
        && children_of(world, slot.anchor) == expected_anchor_children
}

fn stage_failure(
    world: &mut World,
    kind: ChildSlotStageErrorKind,
    provisional: ProvisionalChildSlot,
) -> ChildSlotStageFailure {
    let state = if world.get_entity(provisional.slot).is_ok() {
        clear_slot_focus(world, provisional.slot);
        ChildSlotResidualState::Provisional
    } else {
        ChildSlotResidualState::Absent
    };
    let authorized_manifest = child_slot_authorized_manifest(world);
    ChildSlotStageFailure {
        kind,
        residual: Some(ChildSlotResidual {
            state,
            provisional,
            authorized_manifest,
        }),
    }
}

fn child_slot_authorized_manifest(world: &World) -> ComponentManifest {
    let mut explicit = [
        world.components().component_id::<Visibility>(),
        world.components().component_id::<Pickable>(),
        world.components().component_id::<RouterInputBarrier>(),
        world.components().component_id::<ChildSlotBarrierProbe>(),
        world.components().component_id::<Node>(),
        world.components().component_id::<ChildSlotNodeProbe>(),
        world.components().component_id::<RouteOwnershipSeal>(),
        world.components().component_id::<ChildSlotLifecycle>(),
        world.components().component_id::<ChildSlotAuthority>(),
        world.components().component_id::<ChildSlotAuthorityProbe>(),
        world.components().component_id::<ChildOf>(),
        world
            .components()
            .component_id::<ChildSlotRelationshipProbe>(),
        world.components().component_id::<Children>(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let registered = explicit.clone();
    for component in registered {
        if let Some(required) = world.get_required_components_by_id(component) {
            explicit.extend(required.iter_ids());
        }
    }
    ComponentManifest::new(explicit)
}

fn children_of(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map(|children| children.iter().copied().collect())
        .unwrap_or_default()
}

fn focus_is_in_slot(world: &World, slot: Entity) -> bool {
    world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_some_and(|focused| entity_is_in_slot_subtree(world, slot, focused))
        || world
            .get_resource::<RouteFocusState>()
            .and_then(RouteFocusState::focused)
            .is_some_and(|focused| entity_is_in_slot_subtree(world, slot, focused))
}

fn clear_slot_focus(world: &mut World, slot: Entity) {
    if world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_some_and(|focused| entity_is_in_slot_subtree(world, slot, focused))
    {
        world.resource_mut::<InputFocus>().clear();
    }
    if world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused)
        .is_some_and(|focused| entity_is_in_slot_subtree(world, slot, focused))
    {
        world.resource_mut::<RouteFocusState>().set_focused(None);
    }
}

fn entity_is_in_slot_subtree(world: &World, slot: Entity, focused: Entity) -> bool {
    let mut current = focused;
    let mut visited = HashSet::new();
    loop {
        if current == slot {
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

fn mount_receipt_digest(
    slot: &ProvisionalChildSlot,
    slot_id: RouteSlotId,
    slot_manifest: &ComponentManifest,
    anchor_manifest: &ComponentManifest,
    expected_anchor_children: &[Entity],
) -> u64 {
    let mut digest = slot.slot_claim.opaque_digest_part().rotate_left(7)
        ^ slot.anchor_claim.opaque_digest_part().rotate_left(23)
        ^ slot_id.raw().rotate_left(41)
        ^ slot.parent.router().raw()
        ^ slot.parent.host().raw().rotate_left(13)
        ^ slot.parent.frame().raw().rotate_left(31)
        ^ slot.parent.generation().raw().rotate_left(47);
    for byte in slot.outlet.as_str().as_bytes() {
        digest = digest.wrapping_mul(0x100000001b3) ^ u64::from(*byte);
    }
    digest = fold_manifest_digest(digest, 0x5245_4345_4950_5453, slot_manifest);
    digest = fold_manifest_digest(digest, 0x5245_4345_4950_5441, anchor_manifest);
    fold_entity_order_digest(digest, 0x5245_4345_4950_544f, expected_anchor_children)
}

fn fold_manifest_digest(mut digest: u64, domain: u64, manifest: &ComponentManifest) -> u64 {
    digest ^= domain;
    digest = digest
        .wrapping_mul(0x100000001b3)
        .wrapping_add(manifest.component_ids().len() as u64);
    for component in manifest.component_ids() {
        digest = digest
            .wrapping_mul(0x100000001b3)
            .wrapping_add(component.index() as u64);
    }
    digest
}

fn fold_entity_order_digest(mut digest: u64, domain: u64, entities: &[Entity]) -> u64 {
    digest ^= domain;
    digest = digest
        .wrapping_mul(0x100000001b3)
        .wrapping_add(entities.len() as u64);
    for entity in entities {
        digest = digest
            .wrapping_mul(0x100000001b3)
            .wrapping_add(entity.to_bits());
    }
    digest
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy_app::Inherited;
    use bevy_camera::visibility::Visibility;
    use bevy_ecs::{
        component::Component,
        hierarchy::ChildOf,
        lifecycle::HookContext,
        prelude::{Resource, World},
    };
    use bevy_picking::Pickable;
    use bevy_ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node};
    use bevy_ui_router_core::{
        integration::{
            CleanupManifest, EngineAction, MountGrant, MountMutationArtifact, RouterEngine,
        },
        prelude::{RouteLocation, RouteSlotKey, RouteViewKey},
    };

    use crate::{
        app_ext::RouterHandle,
        components::RouterInputBarrier,
        content::{ContentPlan, FrameContentWriter},
        host::{
            HostCandidateClassification, HostCandidateRevisionState, ValidatedHostCandidate,
            classify_host_candidate,
        },
        input::{RouterInputGate, refresh_input_gate_system},
        navigation::RouterNavigation,
        presentation::{
            RealizedFrame, realize_child_frame_deferred, realize_frame, validate_realized_frame,
        },
        registry::{
            EntityLifecycle, EntityOwner, ExactRegistryClaim, ProvisionalOwner,
            QuarantineLeaseTable, RegistryOwner, RouteEntityRegistry,
        },
    };

    #[derive(Resource, Default)]
    struct ChildSlotStageAudit(Vec<bool>);

    #[derive(Component)]
    struct ForeignChildSlotComponent;

    #[derive(Component)]
    struct DescendantFocusProbe;

    #[derive(Resource)]
    struct ForeignParent(Entity);

    #[derive(Resource)]
    struct AnchorOrderDrift {
        anchor: Entity,
        foreign: Entity,
    }

    #[derive(Resource, Default)]
    struct RegistrationDriftArmed(bool);

    fn focus_child_slot(world: &mut World, slot: Entity) {
        world.insert_resource(InputFocus::from_entity(slot));
        let mut route_focus = RouteFocusState::default();
        route_focus.set_focused(Some(slot));
        world.insert_resource(route_focus);
    }

    fn install_registry_first_audit(world: &mut World) {
        let audit = |mut world: bevy_ecs::world::DeferredWorld<'_>, HookContext { entity, .. }| {
            let valid = world
                .get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(entity))
                .is_some_and(|record| {
                    record.entity() == entity
                        && record.lifecycle() == EntityLifecycle::PreparedInert
                        && matches!(
                            record.owner(),
                            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
                        )
                });
            world
                .get_resource_mut::<ChildSlotStageAudit>()
                .expect("stage audit resource is installed")
                .0
                .push(valid);
        };
        world
            .register_component_hooks::<ChildSlotBarrierProbe>()
            .on_insert(audit);
        world
            .register_component_hooks::<ChildSlotNodeProbe>()
            .on_insert(audit);
        world
            .register_component_hooks::<ChildSlotAuthorityProbe>()
            .on_insert(audit);
        world
            .register_component_hooks::<ChildSlotRelationshipProbe>()
            .on_insert(audit);
    }

    fn install_anchor_order_drift<C: Component>(
        world: &mut World,
        frame: &RealizedFrame,
    ) -> (Entity, Entity, Entity) {
        let anchor = frame
            .outlet(&RouteSlotKey::default())
            .expect("default outlet exists")
            .anchor();
        let (anchor_claim, old_manifest) = {
            let record = world
                .resource::<RouteEntityRegistry>()
                .record(anchor)
                .expect("outlet anchor stays registered");
            (record.claim(), record.manifest().clone())
        };
        let baseline = world.spawn(ChildOf(anchor)).id();
        world.flush();
        let actual_manifest = ComponentManifest::from_world(world, anchor).unwrap();
        assert!(
            world
                .resource_mut::<RouteEntityRegistry>()
                .compare_exchange_manifest_exact(&anchor_claim, &old_manifest, actual_manifest,)
        );
        let foreign = world.spawn_empty().id();
        world.insert_resource(AnchorOrderDrift { anchor, foreign });
        world
            .register_component_hooks::<C>()
            .on_insert(|mut world, HookContext { .. }| {
                let drift = world
                    .get_resource::<AnchorOrderDrift>()
                    .expect("anchor drift is armed");
                let anchor = drift.anchor;
                let foreign = drift.foreign;
                world.commands().entity(foreign).insert(ChildOf(anchor));
            });
        (anchor, baseline, foreign)
    }

    fn assert_residual_inert(world: &World, residual: &ChildSlotResidual) {
        if residual.state() == ChildSlotResidualState::Absent {
            assert!(world.get_entity(residual.entity()).is_err());
            return;
        }
        assert_eq!(
            world.get::<Visibility>(residual.entity()),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(residual.entity()),
            Some(&Pickable::IGNORE)
        );
        assert!(!focus_is_in_slot(world, residual.entity()));
    }

    fn assert_residual_barrier_denied(world: &World, residual: &ChildSlotResidual) {
        assert!(
            world
                .get::<RouterInputBarrier>(residual.entity())
                .is_some_and(RouterInputBarrier::denies_all)
        );
    }

    fn pending_nested_parent_fixture() -> (
        World,
        ValidatedHostCandidate,
        bevy_ecs::prelude::Entity,
        ExactRegistryClaim,
        RouterEngine,
        bevy_ui_router_core::integration::FrameTask,
    ) {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let router = RouterHandle::fixture(41, 43);
        world.spawn((
            Node::default(),
            crate::app_ext::RouterHostRegistration::new(router),
        ));
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(&world, router, &mut revisions)
        else {
            panic!("host should classify")
        };

        let mut engine = RouterEngine::install(
            bevy_ui_router_core::test_support::nested_items_view_route_tree(),
            RouteViewKey::new("fallback").unwrap(),
        )
        .unwrap();
        let admission = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&admission).unwrap();
        let staged =
            crate::host::stage_granted_top_slot(&mut world, candidate.clone(), &grant).unwrap();
        let top_slot = staged.entity();
        assert!(crate::host::activate_granted_top_slot(
            &mut world, &candidate, &grant, staged,
        ));
        let top_slot_claim = world
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap()
            .claim();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/items/1")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(parent_task) = engine.action().unwrap() else {
            panic!("parent frame task expected")
        };
        (
            world,
            candidate,
            top_slot,
            top_slot_claim,
            engine,
            parent_task,
        )
    }

    fn realized_parent_with_outlet() -> (
        World,
        ValidatedHostCandidate,
        bevy_ecs::prelude::Entity,
        RouterEngine,
        RealizedFrame,
        usize,
        MountGrant,
    ) {
        let (mut world, candidate, top_slot, top_slot_claim, mut engine, task) =
            pending_nested_parent_fixture();
        let mut plan = ContentPlan::new();
        let anchor = {
            let mut writer = FrameContentWriter::new(&mut plan);
            writer.spawn_empty().unwrap()
        };
        FrameContentWriter::new(&mut plan)
            .declare_outlet(anchor, RouteSlotKey::default())
            .unwrap();
        let accepted = plan.accept();
        let local_index = accepted.outlets()[0].local_index();
        let frame = realize_frame(
            &mut world,
            candidate.clone(),
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            accepted,
        )
        .unwrap();
        engine
            .report_applied_with_outlets(task, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(request) = engine.action().unwrap() else {
            panic!("child mount request expected")
        };
        let grant = engine.resolve_mount(request).unwrap();
        (
            world,
            candidate,
            top_slot,
            engine,
            frame,
            local_index,
            grant,
        )
    }

    #[test]
    fn accepted_outlet_maps_to_exact_realized_output() {
        let (world, _candidate, _top_slot, _engine, frame, local_index, _grant) =
            realized_parent_with_outlet();
        let outlet = frame
            .outlet(&RouteSlotKey::default())
            .expect("accepted outlet must map to one exact output");
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(outlet.anchor())
            .expect("realized outlet anchor must stay registered");

        assert_eq!(record.claim(), *outlet.anchor_claim());
        assert!(matches!(
            record.owner(),
            RegistryOwner::Committed(EntityOwner::FrameOutput {
                local_index: actual,
                ..
            }) if *actual == local_index
        ));
    }

    #[test]
    fn wrong_outlet_grant_mismatch_is_world_zero_and_grant_remains_usable() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let records_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let entities_before = world.iter_entities().count();
        let seal_before = world.resource::<RouteEntityRegistry>().latest_issued_seal();

        let failure = stage_child_slot(
            &mut world,
            &frame,
            &RouteSlotKey::named("wrong").unwrap(),
            &grant,
        )
        .expect_err("a sealed grant cannot stage a different outlet");

        assert_eq!(failure.kind(), ChildSlotStageErrorKind::GrantMismatch);
        assert!(failure.into_residual().is_none());
        assert_eq!(world.iter_entities().count(), entities_before);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records(),
            records_before
        );
        assert_eq!(
            world.resource::<RouteEntityRegistry>().latest_issued_seal(),
            seal_before
        );
        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant)
            .expect("the borrowed grant remains usable for its exact target");
    }

    #[test]
    fn equal_raw_cross_router_grant_mismatch_is_world_zero_before_owner_cas() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, _grant) =
            realized_parent_with_outlet();
        let (
            mut foreign_world,
            _foreign_candidate,
            _foreign_top_slot,
            _foreign_engine,
            foreign_frame,
            _foreign_local_index,
            foreign_grant,
        ) = realized_parent_with_outlet();
        assert_eq!(frame.frame().router(), foreign_frame.frame().router());
        assert_eq!(frame.frame().host(), foreign_frame.frame().host());
        let records_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let entities_before = world.iter_entities().count();
        let seal_before = world.resource::<RouteEntityRegistry>().latest_issued_seal();

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &foreign_grant)
                .expect_err("equal raw IDs from another router domain must fail closed");

        assert_eq!(failure.kind(), ChildSlotStageErrorKind::GrantMismatch);
        assert!(failure.into_residual().is_none());
        assert_eq!(world.iter_entities().count(), entities_before);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records(),
            records_before
        );
        assert_eq!(
            world.resource::<RouteEntityRegistry>().latest_issued_seal(),
            seal_before
        );
        stage_child_slot(
            &mut foreign_world,
            &foreign_frame,
            &RouteSlotKey::default(),
            &foreign_grant,
        )
        .expect("the borrowed grant remains usable in its owning router domain");
    }

    #[test]
    fn child_slot_remains_provisional_before_registration_permit() {
        let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
            realized_parent_with_outlet();

        let staged =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(staged.slot())
            .expect("staged child slot must stay registered");

        assert_eq!(record.claim(), *staged.slot_claim());
        assert!(matches!(
            record.owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
                parent,
                outlet,
                slot,
            }) if parent == frame.frame()
                && outlet == &RouteSlotKey::default()
                && *slot == grant.slot()
        ));
        assert_eq!(record.lifecycle(), EntityLifecycle::PreparedInert);
        assert_eq!(
            world.get::<Visibility>(staged.slot()),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(staged.slot()),
            Some(&Pickable::IGNORE)
        );
        assert!(
            world
                .get::<RouterInputBarrier>(staged.slot())
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert!(engine.action().is_none());
    }

    #[test]
    fn child_slot_is_provisionally_registered_before_any_observer_capable_write() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world.insert_resource(ChildSlotStageAudit::default());
        install_registry_first_audit(&mut world);

        stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();

        assert_eq!(
            world.resource::<ChildSlotStageAudit>().0,
            vec![true, true, true, true]
        );
    }

    #[test]
    fn barrier_batch_rejects_anchor_children_order_drift_in_the_same_batch() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let (anchor, baseline, foreign) =
            install_anchor_order_drift::<ChildSlotBarrierProbe>(&mut world, &frame);

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();

        assert_eq!(failure.kind(), ChildSlotStageErrorKind::BarrierDrift);
        let residual = failure.into_residual().unwrap();
        assert_eq!(children_of(&world, anchor), vec![baseline, foreign]);
        assert!(world.get::<ChildOf>(residual.entity()).is_none());
        assert_ne!(
            world
                .resource::<RouteEntityRegistry>()
                .record(residual.entity())
                .unwrap()
                .lifecycle(),
            EntityLifecycle::Active
        );
    }

    #[test]
    fn node_batch_rejects_anchor_children_order_drift_in_the_same_batch() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let (anchor, baseline, foreign) =
            install_anchor_order_drift::<ChildSlotNodeProbe>(&mut world, &frame);

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();

        assert_eq!(failure.kind(), ChildSlotStageErrorKind::NodeDrift);
        let residual = failure.into_residual().unwrap();
        assert_eq!(children_of(&world, anchor), vec![baseline, foreign]);
        assert!(world.get::<ChildOf>(residual.entity()).is_none());
        assert_ne!(
            world
                .resource::<RouteEntityRegistry>()
                .record(residual.entity())
                .unwrap()
                .lifecycle(),
            EntityLifecycle::Active
        );
    }

    #[test]
    fn authority_batch_rejects_anchor_children_order_drift_in_the_same_batch() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let (anchor, baseline, foreign) =
            install_anchor_order_drift::<ChildSlotAuthorityProbe>(&mut world, &frame);

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();

        assert_eq!(failure.kind(), ChildSlotStageErrorKind::AuthorityDrift);
        let residual = failure.into_residual().unwrap();
        assert_eq!(children_of(&world, anchor), vec![baseline, foreign]);
        assert!(world.get::<ChildOf>(residual.entity()).is_none());
        assert_ne!(
            world
                .resource::<RouteEntityRegistry>()
                .record(residual.entity())
                .unwrap()
                .lifecycle(),
            EntityLifecycle::Active
        );
    }

    #[test]
    fn child_slot_barrier_drift_is_fail_closed_without_activation() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotBarrierProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .remove::<RouterInputBarrier>();
            });

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
        assert_eq!(failure.kind(), ChildSlotStageErrorKind::BarrierDrift);
        let residual = failure.into_residual().unwrap();
        assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
        assert_eq!(
            world.get::<Visibility>(residual.entity()),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(residual.entity()),
            Some(&Pickable::IGNORE)
        );
        assert!(!focus_is_in_slot(&world, residual.entity()));
        assert!(world.get::<RouterInputBarrier>(residual.entity()).is_none());
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap();
        assert_ne!(record.lifecycle(), EntityLifecycle::Active);
        assert!(record.manifest().component_ids().is_empty());
        assert!(!record.manifest().matches(&world, residual.entity()));
    }

    #[test]
    fn child_slot_node_closure_drift_is_rejected_and_inert() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotNodeProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .insert(ForeignChildSlotComponent);
            });

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
        assert_eq!(failure.kind(), ChildSlotStageErrorKind::NodeDrift);
        let residual = failure.into_residual().unwrap();
        assert_residual_inert(&world, &residual);
        assert_residual_barrier_denied(&world, &residual);
        assert!(
            world
                .get::<ForeignChildSlotComponent>(residual.entity())
                .is_some()
        );
        let node = world.components().component_id::<Node>().unwrap();
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap();
        assert!(!record.manifest().component_ids().contains(&node));
    }

    #[test]
    fn refresh_clears_focus_reassigned_to_a_foreign_descendant_of_residual() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotNodeProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .insert(ForeignChildSlotComponent);
            });
        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
        let residual = failure.into_residual().unwrap();
        let foreign = world.spawn(ChildOf(residual.entity())).id();
        world.flush();
        world.insert_resource(InputFocus::from_entity(foreign));
        let mut route_focus = RouteFocusState::default();
        route_focus.set_focused(Some(foreign));
        world.insert_resource(route_focus);
        world.insert_resource(RouterNavigation::new());
        world.insert_resource(RouterInputGate::new());

        refresh_input_gate_system(&mut world);

        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn refresh_preserves_active_sibling_focus_below_the_nearest_registry_boundary() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotNodeProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .insert(ForeignChildSlotComponent);
            });
        let residual = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant)
            .unwrap_err()
            .into_residual()
            .unwrap();
        assert_ne!(
            world
                .resource::<RouteEntityRegistry>()
                .record(residual.entity())
                .unwrap()
                .lifecycle(),
            EntityLifecycle::Active
        );
        let shell_claim = world
            .resource::<RouteEntityRegistry>()
            .record(frame.shell())
            .unwrap()
            .claim();
        world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&shell_claim, EntityLifecycle::Active)
            .unwrap();
        let active_descendant = world.spawn(ChildOf(frame.shell())).id();
        world.flush();
        world.insert_resource(InputFocus::from_entity(active_descendant));
        let mut route_focus = RouteFocusState::default();
        route_focus.set_focused(Some(active_descendant));
        world.insert_resource(route_focus);
        world.insert_resource(RouterNavigation::new());
        world.insert_resource(RouterInputGate::new());

        refresh_input_gate_system(&mut world);

        assert_eq!(
            world.resource::<InputFocus>().get(),
            Some(active_descendant)
        );
        assert_eq!(
            world.resource::<RouteFocusState>().focused(),
            Some(active_descendant)
        );
    }

    #[test]
    fn child_slot_authority_drift_is_rejected_and_inert() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotAuthorityProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .remove::<RouteOwnershipSeal>();
            });

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
        assert_eq!(failure.kind(), ChildSlotStageErrorKind::AuthorityDrift);
        let residual = failure.into_residual().unwrap();
        assert_residual_inert(&world, &residual);
        assert_residual_barrier_denied(&world, &residual);
        assert!(world.get::<RouteOwnershipSeal>(residual.entity()).is_none());
        let authority = world
            .components()
            .component_id::<ChildSlotAuthority>()
            .unwrap();
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap();
        assert!(!record.manifest().component_ids().contains(&authority));
    }

    #[test]
    fn child_slot_relationship_drift_is_rejected_and_inert() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let foreign_parent = world.spawn_empty().id();
        world.insert_resource(ForeignParent(foreign_parent));
        world
            .register_component_hooks::<ChildSlotRelationshipProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let parent = world
                    .get_resource::<ForeignParent>()
                    .expect("foreign parent is installed")
                    .0;
                world.commands().entity(entity).insert(ChildOf(parent));
            });

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
        assert_eq!(failure.kind(), ChildSlotStageErrorKind::RelationshipDrift);
        let residual = failure.into_residual().unwrap();
        assert_residual_inert(&world, &residual);
        assert_residual_barrier_denied(&world, &residual);
        assert_eq!(
            world.get::<ChildOf>(residual.entity()).map(ChildOf::parent),
            Some(foreign_parent)
        );
        let child_of = world.components().component_id::<ChildOf>().unwrap();
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(residual.entity())
            .unwrap();
        assert!(!record.manifest().component_ids().contains(&child_of));
    }

    #[test]
    fn child_slot_relationship_runtime_ui_closure_is_registered_exactly() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotRelationshipProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).insert((
                    Inherited(ComputedUiTargetCamera::default()),
                    Inherited(ComputedUiRenderTargetInfo::default()),
                ));
            });

        let staged = stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant)
            .expect("known UI propagation components must be included in the locked manifest");
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(staged.slot())
            .expect("staged slot remains registered");
        assert!(record.manifest().matches(&world, staged.slot()));
        assert!(
            record.manifest().component_ids().contains(
                &world
                    .components()
                    .component_id::<Inherited<ComputedUiTargetCamera>>()
                    .unwrap()
            )
        );
        assert!(
            record.manifest().component_ids().contains(
                &world
                    .components()
                    .component_id::<Inherited<ComputedUiRenderTargetInfo>>()
                    .unwrap()
            )
        );
    }

    #[test]
    fn child_slot_despawn_drift_is_tracked_absent_without_cleanup() {
        let (mut world, _candidate, _top_slot, _engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world
            .register_component_hooks::<ChildSlotAuthorityProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).despawn();
            });

        let failure =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap_err();
        let residual = failure.into_residual().unwrap();
        assert_eq!(residual.state(), ChildSlotResidualState::Absent);
        assert_residual_inert(&world, &residual);
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .record(residual.entity())
                .unwrap()
                .claim(),
            *residual.claim()
        );
    }

    #[test]
    fn child_slot_registration_rejection_leaves_no_empty_entity() {
        let (mut world, _candidate, _top_slot, _engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let _first =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let entities_before = world.iter_entities().count();
        let records_before = world.resource::<RouteEntityRegistry>().records().len();

        let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
            .expect_err("duplicate logical child owner must reject registration");

        assert_eq!(failure.kind(), ChildSlotStageErrorKind::RegistryRejected);
        assert!(failure.into_residual().is_none());
        assert_eq!(world.iter_entities().count(), entities_before);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records().len(),
            records_before
        );
    }

    #[test]
    fn child_slot_seal_and_slot_id_are_not_reused_after_failure() {
        let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let first_slot = grant.slot();
        let _first =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let first_seal = world
            .resource::<RouteEntityRegistry>()
            .latest_issued_seal()
            .expect("first child slot must issue an ownership seal");

        let failure = stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant)
            .expect_err("duplicate logical child owner must fail after issuing its seal");
        assert!(failure.into_residual().is_none());
        let failed_seal = world
            .resource::<RouteEntityRegistry>()
            .latest_issued_seal()
            .expect("failed child slot registration must still burn its issued seal");
        assert_ne!(failed_seal, first_seal);
        let next_seal = world.resource_mut::<RouteEntityRegistry>().issue_seal();
        assert_ne!(next_seal, failed_seal);

        let cleanup_request = engine
            .reject_mount_after_mutation(MountMutationArtifact::Grant(grant))
            .unwrap();
        let cleanup = engine
            .prepare_failure_cleanup_retaining(
                cleanup_request,
                CleanupManifest::new(vec![parent.frame().clone()], 701, 1),
            )
            .unwrap();
        engine.complete_cleanup(cleanup.complete(709, 1)).unwrap();
        engine.push(RouteLocation::new("/items/2")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(next_parent) = engine.action().unwrap() else {
            panic!("next request must issue its parent frame task")
        };
        engine
            .report_applied_with_outlets(next_parent, vec![RouteSlotKey::default()])
            .unwrap();
        let EngineAction::MountRequest(next_request) = engine.action().unwrap() else {
            panic!("next parent report must issue a fresh mount request")
        };
        let next_grant = engine.resolve_mount(next_request).unwrap();

        assert_ne!(next_grant.slot(), first_slot);
        assert!(next_grant.slot().raw() > first_slot.raw());
    }

    #[test]
    fn child_slot_is_registered_inert_after_permit_until_exact_completion_and_confirmation() {
        let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
        let slot = staged.slot();
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();

        assert!(matches!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .unwrap()
                .owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
        ));
        assert!(engine.action().is_none());

        let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(mounted.slot())
            .unwrap();
        assert_eq!(record.claim(), *mounted.slot_claim());
        assert!(matches!(
            record.owner(),
            RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent,
                outlet,
                slot: stored_slot,
            }) if parent == frame.frame()
                && outlet == &RouteSlotKey::default()
                && *stored_slot == mounted.slot_id()
        ));
        assert_eq!(
            world.get::<ChildSlotLifecycle>(mounted.slot()),
            Some(&ChildSlotLifecycle::RegisteredInert)
        );
        assert_eq!(
            world.get::<Visibility>(mounted.slot()),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            world.get::<Pickable>(mounted.slot()),
            Some(&Pickable::IGNORE)
        );
        assert!(
            world
                .get::<RouterInputBarrier>(mounted.slot())
                .is_some_and(RouterInputBarrier::denies_all)
        );
        let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
            panic!("child task may exist only after exact registration completion")
        };
        assert_eq!(child_task.frame().slot(), mounted.slot_id());
    }

    #[test]
    fn child_frame_shell_uses_confirmed_child_slot_not_top_slot() {
        let (mut world, _candidate, top_slot, mut engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
        let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
            panic!("confirmed child slot must issue one child frame task")
        };

        let child = realize_child_frame_deferred(
            &mut world,
            &parent,
            &mounted,
            &child_task,
            child_task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap_or_else(|_| panic!("confirmed child slot must accept its exact child task"));

        assert!(child.mount_binding().is_child_outlet());
        assert_eq!(child.top_slot(), mounted.slot());
        assert_ne!(child.top_slot(), top_slot);
        assert_eq!(
            world.get::<ChildOf>(child.shell()).map(ChildOf::parent),
            Some(mounted.slot())
        );
        assert_eq!(child.frame().slot(), mounted.slot_id());
        assert!(validate_realized_frame(&world, &child));
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(mounted.slot())
                .is_some_and(|record| record.manifest().matches(&world, mounted.slot()))
        );
    }

    fn realized_nested_branch() -> (World, crate::branch_transaction::RealizedBranch) {
        let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
        let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
            panic!("confirmed child slot must issue one child frame task")
        };
        let child = realize_child_frame_deferred(
            &mut world,
            &parent,
            &mounted,
            &child_task,
            child_task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap_or_else(|_| panic!("exact child task must realize"));
        let mut branch = crate::branch_transaction::RealizedBranch::empty();
        branch.push_frame(parent).unwrap();
        assert!(branch.push_child_slot(mounted).is_ok());
        branch.push_frame(child).unwrap();
        (world, branch)
    }

    #[test]
    fn publication_rebinds_frame_mount_outlet_and_child_slot_aliases() {
        let (world, mut branch) = realized_nested_branch();
        let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
        let resulting = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec();

        let prepared = branch
            .prepare_claim_rebind(
                &resulting,
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
            )
            .unwrap();
        branch.apply_claim_rebind(prepared);
        assert!(
            branch
                .claims()
                .all(|claim| claim.lifecycle() == EntityLifecycle::Active)
        );
        let parent = &branch.frames()[0];
        let outlet = parent.outlet(&RouteSlotKey::default()).unwrap();
        assert_eq!(outlet.anchor_claim().lifecycle(), EntityLifecycle::Active);
        let mounted = &branch.child_slots()[0];
        assert_eq!(mounted.slot_claim().lifecycle(), EntityLifecycle::Active);
        assert_eq!(mounted.anchor_claim().lifecycle(), EntityLifecycle::Active);
        let child_mount = branch.frames()[1].mount_binding();
        assert_eq!(
            child_mount.slot_claim().lifecycle(),
            EntityLifecycle::Active
        );
        assert_eq!(
            child_mount.physical_parent().1.unwrap().lifecycle(),
            EntityLifecycle::Active
        );
    }

    #[test]
    fn publication_rebind_rejects_wrong_from_or_to_lifecycle_for_all_aliases() {
        let (world, branch) = realized_nested_branch();
        let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
        let correct = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec();
        let before = branch.claims().cloned().collect::<Vec<_>>();
        let parent = &branch.frames()[0];
        let checked_alias_entities = [
            parent.shell(),
            parent.outlet(&RouteSlotKey::default()).unwrap().anchor(),
            branch.child_slots()[0].slot(),
        ];

        for entity in checked_alias_entities {
            let current = branch
                .claims()
                .find(|claim| claim.entity() == entity)
                .unwrap()
                .clone();
            let wrong_next = world
                .resource::<RouteEntityRegistry>()
                .prepare_lifecycle_mutation(&[(current, EntityLifecycle::Leaving)])
                .unwrap()
                .resulting_claims()[0]
                .clone();
            let mut claims = correct.clone();
            *claims
                .iter_mut()
                .find(|claim| claim.entity() == entity)
                .unwrap() = wrong_next;
            assert!(
                branch
                    .prepare_claim_rebind(
                        &claims,
                        EntityLifecycle::PreparedInert,
                        EntityLifecycle::Active,
                    )
                    .is_none()
            );
            assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
        }

        assert!(
            branch
                .prepare_claim_rebind(&correct, EntityLifecycle::Active, EntityLifecycle::Leaving,)
                .is_none()
        );
        assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn mounted_child_slot_alias_rebind_is_atomic() {
        let (world, branch) = realized_nested_branch();
        let mounted = branch
            .child_slots()
            .into_iter()
            .next()
            .unwrap()
            .clone_for_test();
        let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
        let mut resulting = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec();
        let anchor_index = resulting
            .iter()
            .position(|claim| claim.entity() == mounted.anchor_claim().entity())
            .unwrap();
        let anchor_manifest = world
            .resource::<RouteEntityRegistry>()
            .record(mounted.anchor())
            .unwrap()
            .manifest()
            .clone();
        let mut foreign_registry = RouteEntityRegistry::new();
        let foreign_seal = foreign_registry.issue_seal();
        foreign_registry
            .register(
                mounted.anchor(),
                mounted.anchor_claim().owner().clone(),
                foreign_seal,
                anchor_manifest,
                EntityLifecycle::Active,
            )
            .unwrap();
        resulting[anchor_index] = foreign_registry.record(mounted.anchor()).unwrap().claim();
        let slot_before = mounted.slot_claim().clone();
        let anchor_before = mounted.anchor_claim().clone();

        assert!(!mounted.internal_claims_can_rebind(
            &resulting,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        ));
        assert_eq!(mounted.slot_claim(), &slot_before);
        assert_eq!(mounted.anchor_claim(), &anchor_before);
    }

    #[test]
    fn missing_mounted_child_slot_alias_is_zero_partial_write() {
        let (mut world, branch) = realized_nested_branch();
        let mut mounted = branch
            .child_slots()
            .into_iter()
            .next()
            .unwrap()
            .clone_for_test();
        let transitions = branch.lifecycle_transitions(EntityLifecycle::Active, false);
        let resulting = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec();
        let foreign_entity = world.spawn_empty().id();
        let anchor_manifest = world
            .resource::<RouteEntityRegistry>()
            .record(mounted.anchor())
            .unwrap()
            .manifest()
            .clone();
        let mut foreign_registry = RouteEntityRegistry::new();
        let foreign_seal = foreign_registry.issue_seal();
        foreign_registry
            .register(
                foreign_entity,
                mounted.anchor_claim().owner().clone(),
                foreign_seal,
                anchor_manifest,
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        mounted.anchor_claim = foreign_registry.record(foreign_entity).unwrap().claim();
        let slot_before = mounted.slot_claim().clone();
        let anchor_before = mounted.anchor_claim().clone();

        assert!(!mounted.internal_claims_can_rebind(
            &resulting,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        ));
        assert_eq!(mounted.slot_claim(), &slot_before);
        assert_eq!(mounted.anchor_claim(), &anchor_before);
    }

    #[test]
    fn prepared_branch_rejects_mismatched_mounted_anchor_without_world_or_registry_write() {
        let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let digest = staged.receipt().digest();
        let (attestation, proof) = grant.attest_receipt_with_proof(digest);
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        let mut mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
            .expect("exact child slot should confirm");
        mounted.anchor = world.spawn_empty().id();
        let world_before = world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let mut branch = crate::branch_transaction::PreparedBranch::empty();
        branch.push_frame(parent).unwrap();

        let rejected = branch.push_child_slot(mounted).unwrap_err();
        let rejected_entity = rejected.slot().slot();
        assert_eq!(branch.frames().len(), 1);
        assert!(branch.child_slots().is_empty());
        assert_eq!(
            world
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>(),
            world_before
        );
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records(),
            registry_before
        );

        let failed = crate::branch_transaction::FailedPreparedBranch::with_rejected_child_slot(
            branch, rejected,
        );
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
        let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
            &world,
            world.resource::<RouteEntityRegistry>(),
            &failed,
        )
        .expect("canonical append-before anchor must authorize rejected slot cleanup");
        assert!(manifest.entities().contains(&rejected_entity));
        let mut ledger =
            match crate::cleanup::CleanupLedger::for_failed_prepared_branch(&world, failed) {
                Ok(ledger) => ledger,
                Err(_) => panic!("rejected slot must remain owned by one cleanup ledger"),
            };
        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(rejected_entity)
                .is_none()
        );
    }

    #[test]
    fn canonical_absent_rejected_slot_preserves_unknown_parent_as_unresolved() {
        let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let digest = staged.receipt().digest();
        let (attestation, proof) = grant.attest_receipt_with_proof(digest);
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        let mut mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
            .expect("exact child slot should confirm");
        let slot = mounted.slot();
        let source_claim = mounted.slot_claim().clone();
        let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
            panic!("confirmed slot must issue its child task")
        };
        mounted.parent = child_task.frame().clone();
        let app_parent = world.spawn_empty().id();
        world
            .entity_mut(slot)
            .insert((ForeignChildSlotComponent, ChildOf(app_parent)));
        world.flush();

        let mut branch = crate::branch_transaction::PreparedBranch::empty();
        branch.push_frame(parent).unwrap();
        let rejected = branch.push_child_slot(mounted).unwrap_err();
        assert!(rejected.canonical_anchor().is_none());
        assert!(rejected.canonical_anchor_claim().is_none());
        let failed = crate::branch_transaction::FailedPreparedBranch::with_rejected_child_slot(
            branch, rejected,
        );
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
        let manifest = crate::cleanup::FrozenCleanupManifest::from_failed_prepared_branch(
            &world,
            world.resource::<RouteEntityRegistry>(),
            &failed,
        )
        .expect("the exact source claim must freeze without trusting an intended anchor");
        assert_eq!(manifest.entities().first(), Some(&slot));
        let mut ledger = crate::cleanup::CleanupLedger::for_failed_prepared_branch(&world, failed)
            .unwrap_or_else(|_| panic!("the rejected slot must retain one cleanup owner"));
        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get_entity(app_parent).is_ok());
        assert_eq!(
            world.get::<ChildOf>(slot).map(ChildOf::parent),
            Some(app_parent)
        );
        assert!(world.get::<ForeignChildSlotComponent>(slot).is_some());
        assert_eq!(
            world.get::<ChildSlotLifecycle>(slot),
            Some(&ChildSlotLifecycle::RegisteredInert)
        );
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .is_none()
        );
        assert!(
            world
                .resource::<QuarantineLeaseTable>()
                .unresolved_record(slot)
                .is_some_and(|record| record.claim() == source_claim)
        );
    }

    #[test]
    fn invalid_child_mount_preflight_cannot_advance_runtime_manifest() {
        let (mut world, _candidate, _top_slot, mut engine, parent, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &parent, &RouteSlotKey::default(), &grant).unwrap();
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        let mounted = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof).unwrap();
        let EngineAction::FrameTask(child_task) = engine.action().unwrap() else {
            panic!("confirmed child slot must issue one child frame task")
        };
        let runtime = world.register_component::<Inherited<ComputedUiTargetCamera>>();
        let _other_runtime = world.register_component::<Inherited<ComputedUiRenderTargetInfo>>();
        let current = world
            .resource::<RouteEntityRegistry>()
            .record(mounted.slot())
            .unwrap()
            .manifest()
            .clone();
        assert!(!current.component_ids().contains(&runtime));
        world
            .entity_mut(mounted.slot())
            .insert(Inherited(ComputedUiTargetCamera::default()));
        world
            .entity_mut(mounted.slot())
            .insert(ChildSlotLifecycle::ProvisionalInert);

        let failure = realize_child_frame_deferred(
            &mut world,
            &parent,
            &mounted,
            &child_task,
            child_task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .expect_err("invalid lifecycle must fail before the runtime manifest CAS");
        let (error, residual) = failure.into_parts();

        assert_eq!(error, crate::presentation::RealizationError::InvalidMount);
        assert!(residual.is_none());
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .record(mounted.slot())
                .unwrap()
                .manifest(),
            &current
        );
    }

    #[test]
    fn descendant_focus_assigned_by_stage_observer_is_cleared() {
        let mut world = World::new();
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
        world
            .register_component_hooks::<DescendantFocusProbe>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                let descendant = world.commands().spawn(ChildOf(entity)).id();
                *world
                    .get_resource_mut::<InputFocus>()
                    .expect("input focus is installed") = InputFocus::from_entity(descendant);
                world
                    .get_resource_mut::<RouteFocusState>()
                    .expect("route focus is installed")
                    .set_focused(Some(descendant));
            });
        let slot = world.spawn(DescendantFocusProbe).id();
        world.flush();

        assert!(focus_is_in_slot(&world, slot));
        clear_slot_focus(&mut world, slot);
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn child_slot_pre_cas_drift_rejects_session_with_provisional_residual() {
        let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
        let slot = staged.slot();
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        world.entity_mut(slot).remove::<RouterInputBarrier>();
        focus_child_slot(&mut world, slot);

        let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
            .expect_err("pre-CAS drift must reject the borrowed session");
        assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::ReceiptDrift);
        let (cleanup, begin_error, residual) = failure.into_parts();
        assert!(cleanup.is_some());
        assert!(begin_error.is_none());
        assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
        assert!(matches!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .unwrap()
                .owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
        ));
        assert!(engine.action().is_none());
        assert!(!focus_is_in_slot(&world, slot));
    }

    #[test]
    fn child_slot_foreign_anchor_child_drift_rejects_before_owner_cas() {
        let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
        let slot = staged.slot();
        let anchor = staged.receipt.anchor;
        let anchor_manifest = staged.receipt.anchor_manifest.clone();
        let foreign = world.spawn(ChildOf(anchor)).id();
        world.flush();
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .record(anchor)
                .unwrap()
                .manifest(),
            &anchor_manifest,
            "foreign child order drift must not need an anchor manifest change",
        );
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        focus_child_slot(&mut world, slot);

        let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
            .expect_err("foreign anchor child drift must reject the borrowed session");
        assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::ReceiptDrift);
        let (cleanup, begin_error, residual) = failure.into_parts();
        assert!(cleanup.is_some());
        assert!(begin_error.is_none());
        assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
        assert!(matches!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .unwrap()
                .owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
        ));
        assert_eq!(
            world.get::<ChildOf>(foreign).map(ChildOf::parent),
            Some(anchor)
        );
        assert!(!focus_is_in_slot(&world, slot));
        assert!(engine.action().is_none());
    }

    #[test]
    fn child_slot_post_cas_node_value_drift_rejects_registered_residual() {
        let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        world.insert_resource(RegistrationDriftArmed::default());
        world.insert_resource(InputFocus::default());
        world.insert_resource(RouteFocusState::default());
        world
            .register_component_hooks::<ChildSlotLifecycle>()
            .on_insert(|mut world, HookContext { entity, .. }| {
                if world
                    .get_resource::<RegistrationDriftArmed>()
                    .is_some_and(|armed| armed.0)
                {
                    world.commands().entity(entity).insert(Node {
                        width: Val::Percent(99.0),
                        height: Val::Percent(100.0),
                        ..Node::default()
                    });
                    *world
                        .get_resource_mut::<InputFocus>()
                        .expect("input focus resource is installed") =
                        InputFocus::from_entity(entity);
                    world
                        .get_resource_mut::<RouteFocusState>()
                        .expect("route focus resource is installed")
                        .set_focused(Some(entity));
                }
            });
        let staged =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
        let slot = staged.slot();
        let slot_id = grant.slot();
        let (attestation, proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        world.resource_mut::<RegistrationDriftArmed>().0 = true;
        assert_eq!(
            world.get::<Node>(slot).map(|node| node.width),
            Some(Val::Percent(100.0)),
        );
        assert!(receipt_matches_provisional(
            &world,
            &staged.provisional,
            &staged.receipt,
            slot_id,
        ));

        let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &proof)
            .expect_err("post-CAS drift must reject the borrowed session");
        assert_eq!(failure.kind(), ChildSlotConfirmationErrorKind::PostCasDrift);
        let (cleanup, begin_error, residual) = failure.into_parts();
        assert!(cleanup.is_some());
        assert!(begin_error.is_none());
        assert_eq!(residual.state(), ChildSlotResidualState::Registered);
        assert!(matches!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .unwrap()
                .owner(),
            RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
        ));
        assert_eq!(
            world.get::<Node>(slot).map(|node| node.width),
            Some(Val::Percent(99.0)),
        );
        assert_residual_inert(&world, &residual);
        assert_residual_barrier_denied(&world, &residual);
        assert!(!focus_is_in_slot(&world, slot));
        assert!(engine.action().is_none());
    }

    #[test]
    fn child_slot_foreign_proof_is_rejected_without_owner_cas_or_child_action() {
        let (mut world, _candidate, _top_slot, mut engine, frame, _local_index, grant) =
            realized_parent_with_outlet();
        let staged =
            stage_child_slot(&mut world, &frame, &RouteSlotKey::default(), &grant).unwrap();
        let slot = staged.slot();
        let (attestation, _proof) = grant.attest_receipt_with_proof(staged.receipt().digest());
        let permit = engine.accept_mount_receipt(attestation).unwrap();
        focus_child_slot(&mut world, slot);

        let (
            mut foreign_world,
            _candidate,
            _top_slot,
            mut foreign_engine,
            foreign_frame,
            _,
            foreign_grant,
        ) = realized_parent_with_outlet();
        let foreign_staged = stage_child_slot(
            &mut foreign_world,
            &foreign_frame,
            &RouteSlotKey::default(),
            &foreign_grant,
        )
        .unwrap();
        let (foreign_attestation, foreign_proof) =
            foreign_grant.attest_receipt_with_proof(foreign_staged.receipt().digest());
        let _foreign_permit = foreign_engine
            .accept_mount_receipt(foreign_attestation)
            .unwrap();

        let failure = confirm_child_slot(&mut world, &mut engine, staged, permit, &foreign_proof)
            .expect_err("foreign proof must fail before owner CAS");
        assert_eq!(
            failure.kind(),
            ChildSlotConfirmationErrorKind::RegistrationBegin
        );
        let (cleanup, begin_error, residual) = failure.into_parts();
        assert!(cleanup.is_some());
        assert!(begin_error.is_none());
        assert_eq!(residual.state(), ChildSlotResidualState::Provisional);
        assert!(matches!(
            world
                .resource::<RouteEntityRegistry>()
                .record(slot)
                .unwrap()
                .owner(),
            RegistryOwner::Provisional(ProvisionalOwner::ChildSlot { .. })
        ));
        assert!(engine.action().is_none());
        assert!(!focus_is_in_slot(&world, slot));
    }
}
