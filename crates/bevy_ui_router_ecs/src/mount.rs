use std::collections::HashSet;

use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::ComponentId,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;
use bevy_ui::{Node, Val};
use bevy_ui_router_core::{
    frame::RouteFrameVersion,
    ids::RouteSlotId,
    integration::{
        FailureCleanupPrepareOutcome, FailureCleanupRequest, IntegrationError, MountGrant,
        MountReceiptProof, MountRegistrationBeginError, MountRegistrationPermit,
        MountRegistrationSession, RouterEngine, mount_grant_matches_target,
    },
    slot::RouteSlotKey,
};

use crate::{
    components::{
        ChildSlotAuthority, ChildSlotAuthorityProbe, ChildSlotBarrierProbe, ChildSlotLifecycle,
        ChildSlotNodeProbe, ChildSlotRelationshipProbe, RouteOwnershipSeal, RouterInputBarrier,
    },
    input::RouteFocusState,
    presentation::{
        OutletBinding, RealizedFrame, RealizedOutlet, RetainedInvalidationEvidence,
        RetainedPayloadClassification,
    },
    registry::{
        ComponentManifest, EntityLifecycle, ExactRegistryClaim, ProvisionalOwner, RegistryOwner,
        RouteEntityRegistry,
    },
};

#[cfg(test)]
std::thread_local! {
    static REJECT_NEXT_CHILD_SLOT_OWNER_CAS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn reject_next_child_slot_owner_cas() {
    REJECT_NEXT_CHILD_SLOT_OWNER_CAS.with(|fault| fault.set(true));
}

#[cfg(test)]
fn child_slot_owner_cas_expected_claim(claim: &ExactRegistryClaim) -> ExactRegistryClaim {
    if REJECT_NEXT_CHILD_SLOT_OWNER_CAS.with(|fault| fault.replace(false)) {
        claim.with_lifecycle_for_test(EntityLifecycle::Active)
    } else {
        claim.clone()
    }
}

#[derive(Debug)]
pub(crate) struct ProvisionalChildSlot {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    parent: RouteFrameVersion,
    outlet: RouteSlotKey,
    anchor: Entity,
    anchor_binding: OutletBinding,
    prior_anchor_children: Vec<Entity>,
}

impl ProvisionalChildSlot {
    pub(crate) const fn slot(&self) -> Entity {
        self.slot
    }

    pub(crate) const fn slot_claim(&self) -> &ExactRegistryClaim {
        &self.slot_claim
    }

    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }
    pub(crate) const fn anchor_binding(&self) -> &OutletBinding {
        &self.anchor_binding
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
    anchor_binding: OutletBinding,
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
    anchor_binding: OutletBinding,
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
        self.anchor_binding.source_claim()
    }

    pub(crate) const fn anchor_binding(&self) -> &OutletBinding {
        &self.anchor_binding
    }

    pub(crate) fn expected_anchor_children(&self) -> &[Entity] {
        &self.expected_anchor_children
    }

    pub(crate) fn current_anchor_children(&self, world: &World) -> Vec<Entity> {
        if self.anchor_binding.is_native() {
            children_of(world, self.anchor)
        } else {
            self.expected_anchor_children.clone()
        }
    }

    #[cfg(test)]
    pub(crate) fn clone_for_test(&self) -> Self {
        Self {
            slot: self.slot,
            slot_claim: self.slot_claim.clone(),
            parent: self.parent.clone(),
            outlet: self.outlet.clone(),
            anchor: self.anchor,
            anchor_binding: self.anchor_binding.clone(),
            expected_anchor_children: self.expected_anchor_children.clone(),
            slot_id: self.slot_id,
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_slot_authority_for_test(
        &mut self,
        slot: Entity,
        slot_claim: ExactRegistryClaim,
        outlet: RouteSlotKey,
        slot_id: RouteSlotId,
    ) {
        self.slot = slot;
        self.slot_claim = slot_claim;
        self.outlet = outlet;
        self.slot_id = slot_id;
    }

    #[cfg(test)]
    pub(crate) fn replace_slot_binding_for_test(
        &mut self,
        slot: Entity,
        slot_claim: ExactRegistryClaim,
        parent: RouteFrameVersion,
        outlet: RouteSlotKey,
        slot_id: RouteSlotId,
    ) {
        self.slot = slot;
        self.slot_claim = slot_claim;
        self.parent = parent;
        self.outlet = outlet;
        self.slot_id = slot_id;
    }

    pub(crate) fn internal_claims_can_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> bool {
        self.claim_alias_can_rebind(&self.slot_claim, claims, expected_current, expected_next)
            && self.claim_alias_can_rebind(
                self.anchor_binding.source_claim(),
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
            .find(|claim| claim.entity() == self.anchor_binding.source_claim().entity())
        {
            *self.anchor_binding.source_claim_mut() = next.clone();
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
        self.provisional.anchor_binding.source_claim()
    }

    pub(crate) const fn anchor_binding(&self) -> &OutletBinding {
        &self.provisional.anchor_binding
    }

    pub(crate) const fn authorized_manifest(&self) -> &ComponentManifest {
        &self.authorized_manifest
    }

    pub(crate) fn prior_anchor_children(&self) -> &[Entity] {
        self.provisional.prior_anchor_children()
    }

    #[cfg(test)]
    pub(crate) fn from_mounted_with_boundary_for_test(
        world: &World,
        mounted: &MountedChildSlot,
        anchor: Entity,
        anchor_claim: ExactRegistryClaim,
    ) -> Self {
        Self {
            state: ChildSlotResidualState::Registered,
            provisional: ProvisionalChildSlot {
                slot: mounted.slot,
                slot_claim: mounted.slot_claim.clone(),
                parent: mounted.parent.clone(),
                outlet: mounted.outlet.clone(),
                anchor,
                anchor_binding: OutletBinding::RegisteredOutput(anchor_claim),
                prior_anchor_children: world
                    .get::<Children>(anchor)
                    .map(|children| children.iter().copied().collect())
                    .unwrap_or_default(),
            },
            authorized_manifest: ComponentManifest::from_world(world, mounted.slot)
                .expect("mounted test slot remains present"),
        }
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
    RetainedInvalidationRejected(IntegrationError),
}

pub(crate) struct ChildSlotConfirmationFailure {
    kind: ChildSlotConfirmationErrorKind,
    terminal: ChildSlotConfirmationTerminal,
    residual: ChildSlotResidual,
    retained_invalidation: Option<RetainedInvalidationEvidence>,
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

    pub(crate) fn take_retained_invalidation(&mut self) -> Option<RetainedInvalidationEvidence> {
        self.retained_invalidation.take()
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

/// Only the selected original stock parent can authorize this fresh mount.
/// The ordinary slot staging machinery still observes and rechecks each write.
pub(crate) fn stage_retained_child_slot(
    world: &mut World,
    projection: &bevy_ui_router_core::integration::RetainedLeafProjection,
    payload: &crate::instance::InstancePayload,
    outlet: &RouteSlotKey,
    grant: &MountGrant,
) -> Result<StagedChildSlot, ChildSlotStageFailure> {
    if payload.frame().frame() != projection.frame()
        || crate::instance::validate_retained_entering_payload(
            world,
            payload.as_payload_ref(),
            projection,
        )
        .is_err()
    {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::InvalidOutlet,
            residual: None,
        });
    }
    let staged = stage_child_slot(world, payload.frame(), outlet, grant)?;
    if !world
        .resource::<RouteEntityRegistry>()
        .retained_payload_is_current_with_staged_slot(
            world,
            projection,
            payload,
            &staged.provisional,
        )
    {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::InvalidOutlet,
            residual: Some(staged.into_residual(world)),
        });
    }
    Ok(staged)
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
    let owner = RegistryOwner::Provisional(ProvisionalOwner::ChildSlot {
        parent: parent.frame().clone(),
        outlet: outlet_key.clone(),
        slot: grant.slot(),
    });
    if world
        .resource_mut::<RouteEntityRegistry>()
        .try_reserve_registration(&owner)
        .is_err()
    {
        return Err(ChildSlotStageFailure {
            kind: ChildSlotStageErrorKind::RegistryRejected,
            residual: None,
        });
    }
    let slot = world.spawn_empty().id();
    let initial_manifest = ComponentManifest::from_world(world, slot)
        .expect("a newly allocated empty Entity must have an archetype");
    let slot_claim = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        match registry.register(
            slot,
            owner,
            initial_manifest.clone(),
            EntityLifecycle::PreparedInert,
        ) {
            Ok(claim) => claim,
            Err(_) => {
                world.despawn(slot);
                return Err(ChildSlotStageFailure {
                    kind: ChildSlotStageErrorKind::RegistryRejected,
                    residual: None,
                });
            }
        }
    };
    let provisional = ProvisionalChildSlot {
        slot,
        slot_claim,
        parent: parent.frame().clone(),
        outlet: outlet_key.clone(),
        anchor: outlet.anchor(),
        anchor_binding: outlet.anchor_binding().clone(),
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
    let anchor_manifest = ComponentManifest::from_world(world, provisional.anchor)
        .expect("validated physical outlet anchor must exist");
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
        anchor_binding: provisional.anchor_binding.clone(),
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
    confirm_child_slot_with_parent(world, engine, staged, permit, proof, None)
}

pub(crate) fn confirm_child_slot_with_retained_parent(
    world: &mut World,
    engine: &mut RouterEngine,
    staged: StagedChildSlot,
    permit: MountRegistrationPermit,
    proof: &MountReceiptProof,
    projection: &bevy_ui_router_core::integration::RetainedLeafProjection,
    payload: &crate::instance::InstancePayload,
) -> Result<MountedChildSlot, ChildSlotConfirmationFailure> {
    confirm_child_slot_with_parent(
        world,
        engine,
        staged,
        permit,
        proof,
        Some((projection, payload)),
    )
}

fn confirm_child_slot_with_parent(
    world: &mut World,
    engine: &mut RouterEngine,
    staged: StagedChildSlot,
    permit: MountRegistrationPermit,
    proof: &MountReceiptProof,
    retained_parent: Option<(
        &bevy_ui_router_core::integration::RetainedLeafProjection,
        &crate::instance::InstancePayload,
    )>,
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
    if !receipt_matches_provisional(world, &provisional, &receipt, slot_id)
        || retained_parent.is_some_and(|(projection, payload)| {
            !world
                .resource::<RouteEntityRegistry>()
                .retained_payload_is_current_with_staged_slot(
                    world,
                    projection,
                    payload,
                    &provisional,
                )
        })
    {
        return Err(reject_child_slot_confirmation(
            world,
            ChildSlotConfirmationErrorKind::ReceiptDrift,
            provisional,
            session,
            retained_parent,
        ));
    }

    let committed_owner = RegistryOwner::Committed(crate::registry::EntityOwner::ChildSlot {
        parent: provisional.parent.clone(),
        outlet: provisional.outlet.clone(),
        slot: slot_id,
    });
    #[cfg(test)]
    let expected_claim = child_slot_owner_cas_expected_claim(&provisional.slot_claim);
    #[cfg(not(test))]
    let expected_claim = provisional.slot_claim.clone();
    let committed_claim = world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_owner(&expected_claim, committed_owner);
    let Some(committed_claim) = committed_claim else {
        return Err(reject_child_slot_confirmation(
            world,
            ChildSlotConfirmationErrorKind::OwnerCasRejected,
            provisional,
            session,
            retained_parent,
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
        anchor_binding: provisional.anchor_binding.clone(),
        expected_anchor_children: receipt.expected_anchor_children.clone(),
        slot_id,
    };
    if !validate_registered_slot(world, &mounted, &receipt)
        || retained_parent.is_some_and(|(projection, payload)| {
            !world
                .resource::<RouteEntityRegistry>()
                .retained_payload_is_current_with_staged_slot(
                    world,
                    projection,
                    payload,
                    &provisional,
                )
        })
    {
        return Err(reject_child_slot_confirmation(
            world,
            ChildSlotConfirmationErrorKind::PostCasDrift,
            provisional,
            session,
            retained_parent,
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
        && receipt.anchor_binding == slot.anchor_binding
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
        && slot.anchor_binding.is_current(world, &slot.parent, true)
        && physical_anchor_manifest_is_current(
            world,
            &slot.anchor_binding,
            &receipt.anchor_manifest,
        )
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
        && slot.anchor_binding.is_current(world, &slot.parent, true)
        && physical_anchor_manifest_is_current(
            world,
            &slot.anchor_binding,
            &receipt.anchor_manifest,
        )
        && !focus_is_in_slot(world, slot.slot)
}

pub(crate) fn mounted_child_slot_is_isolated(world: &World, slot: &MountedChildSlot) -> bool {
    validate_mounted_child_slot(world, slot, EntityLifecycle::PreparedInert)
}

pub(crate) fn mounted_child_slot_structure_is_isolated(
    world: &World,
    slot: &MountedChildSlot,
) -> bool {
    validate_mounted_child_slot_with_policy(
        world,
        slot,
        EntityLifecycle::PreparedInert,
        false,
        false,
    )
}

pub(crate) fn mounted_child_slot_local_is_isolated(world: &World, slot: &MountedChildSlot) -> bool {
    world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(slot.slot))
        .is_some_and(|record| {
            record.claim() == slot.slot_claim
                && record.lifecycle() == EntityLifecycle::PreparedInert
        })
        && world.get::<Visibility>(slot.slot) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(slot.slot) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(slot.slot)
            .is_some_and(RouterInputBarrier::denies_all)
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
}

pub(crate) fn validate_mounted_child_slot(
    world: &World,
    slot: &MountedChildSlot,
    lifecycle: EntityLifecycle,
) -> bool {
    validate_mounted_child_slot_with_policy(world, slot, lifecycle, false, true)
}

pub(crate) fn validate_leaving_mounted_child_slot(
    world: &World,
    slot: &MountedChildSlot,
    lifecycle: EntityLifecycle,
) -> bool {
    validate_mounted_child_slot_with_policy(world, slot, lifecycle, true, true)
}

fn validate_mounted_child_slot_with_policy(
    world: &World,
    slot: &MountedChildSlot,
    lifecycle: EntityLifecycle,
    allow_foreign_additions: bool,
    require_focus_outside: bool,
) -> bool {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    let Some(record) = registry.record(slot.slot) else {
        return false;
    };
    let (component_lifecycle, mut visibility) = match lifecycle {
        EntityLifecycle::PreparedInert => (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden),
        EntityLifecycle::Active => (ChildSlotLifecycle::Active, Visibility::Visible),
        EntityLifecycle::Retained => (ChildSlotLifecycle::Retained, Visibility::Hidden),
        EntityLifecycle::Leaving => (ChildSlotLifecycle::Leaving, Visibility::Hidden),
        EntityLifecycle::Quarantined => (ChildSlotLifecycle::Quarantined, Visibility::Hidden),
    };
    if matches!(
        lifecycle,
        EntityLifecycle::Retained | EntityLifecycle::Leaving
    ) && crate::transition::holds_visible_claim(world, &slot.slot_claim)
    {
        visibility = Visibility::Visible;
    }
    record.claim() == slot.slot_claim
        && record.lifecycle() == lifecycle
        && manifest_matches_policy(world, slot.slot, record.manifest(), allow_foreign_additions)
        && record.owner()
            == &RegistryOwner::Committed(crate::registry::EntityOwner::ChildSlot {
                parent: slot.parent.clone(),
                outlet: slot.outlet.clone(),
                slot: slot.slot_id,
            })
        && slot.anchor_binding.is_current_with_manifest_policy(
            world,
            &slot.parent,
            true,
            allow_foreign_additions,
        )
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
        && slot.anchor_binding.children_are_current(
            world,
            slot.slot,
            &slot.expected_anchor_children,
        )
        && (lifecycle == EntityLifecycle::Active
            || !require_focus_outside
            || !focus_is_in_slot(world, slot.slot))
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

pub(crate) fn residual_child_slot_local_is_isolated(
    world: &World,
    residual: &ChildSlotResidual,
) -> bool {
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
    visibility_is_safe
        && pickable_is_safe
        && barrier_is_safe
        && seal_is_exact
        && lifecycle_is_exact
        && authority_is_exact
}

fn reject_child_slot_confirmation(
    world: &mut World,
    kind: ChildSlotConfirmationErrorKind,
    provisional: ProvisionalChildSlot,
    session: MountRegistrationSession<'_>,
    retained_parent: Option<(
        &bevy_ui_router_core::integration::RetainedLeafProjection,
        &crate::instance::InstancePayload,
    )>,
) -> ChildSlotConfirmationFailure {
    let classification = retained_parent.and_then(|(projection, payload)| {
        world
            .resource::<RouteEntityRegistry>()
            .classify_retained_payload_with_staged_slot(world, projection, payload, &provisional)
    });
    match classification {
        Some(RetainedPayloadClassification::OwnInvalid(evidence)) => {
            match session.reject_retained_invalidation(evidence.projection().frame().clone()) {
                Ok(cleanup) => {
                    let mut failure = confirmation_failure(
                        world,
                        kind,
                        provisional,
                        ChildSlotConfirmationTerminal::Cleanup(cleanup),
                    );
                    failure.retained_invalidation = Some(evidence);
                    failure
                }
                Err(error) => {
                    let rejection_kind = error.kind();
                    let cleanup = error.into_session().reject();
                    confirmation_failure(
                        world,
                        ChildSlotConfirmationErrorKind::RetainedInvalidationRejected(
                            rejection_kind,
                        ),
                        provisional,
                        ChildSlotConfirmationTerminal::Cleanup(cleanup),
                    )
                }
            }
        }
        Some(
            RetainedPayloadClassification::Current
            | RetainedPayloadClassification::AuthorityLost(_),
        )
        | None => {
            let cleanup = session.reject();
            confirmation_failure(
                world,
                kind,
                provisional,
                ChildSlotConfirmationTerminal::Cleanup(cleanup),
            )
        }
    }
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
        retained_invalidation: None,
    }
}

fn mounted_registration_digest(slot: &MountedChildSlot, receipt: &MountReceipt) -> u64 {
    let mut digest = slot.slot_claim.opaque_digest_part().rotate_left(17)
        ^ slot.anchor_binding.opaque_digest_part().rotate_left(37)
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
    let Some(old_anchor) = ComponentManifest::from_world(world, slot.anchor) else {
        return false;
    };
    let registered_anchor = if let Some(claim) = slot.anchor_binding.physical_claim() {
        let Some(record) = world
            .resource::<RouteEntityRegistry>()
            .record(slot.anchor)
            .filter(|record| record.claim() == *claim)
        else {
            return false;
        };
        Some((claim.clone(), record.manifest().clone()))
    } else {
        None
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
    let expected_slot =
        manifest_with_required(world, &old_slot, vec![child_of, relationship_probe]);
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
    if actual_slot.as_ref() != Some(&expected_slot)
        || actual_anchor.as_ref() != Some(&expected_anchor)
        || children_of(world, slot.anchor) != expected_children
        || world.get::<ChildOf>(slot.slot).map(ChildOf::parent) != Some(slot.anchor)
        || !validate_stage_registry(world, slot, &old_slot)
        // The exact physical delta above authorizes the newly inserted Children.
        // Its registry manifest is exchanged below, so this intermediate check
        // must validate the original source without requiring that exchange early.
        || !slot.anchor_binding.is_current_with_manifest_policy(world, &slot.parent, true, true)
        || registered_anchor.as_ref().is_some_and(|(claim, manifest)| !world.resource::<RouteEntityRegistry>().record(slot.anchor)
            .is_some_and(|record| record.claim() == *claim && record.manifest() == manifest))
    {
        return false;
    }
    let mut exchanges = vec![(slot.slot_claim.clone(), old_slot, expected_slot.clone())];
    if let Some((claim, manifest)) = registered_anchor {
        exchanges.push((claim, manifest, expected_anchor.clone()));
    }
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifests_exact(&exchanges)
    {
        return false;
    }
    validate_stage_registry(world, slot, &expected_slot)
        && expected_slot.matches(world, slot.slot)
        && physical_anchor_manifest_is_current(world, &slot.anchor_binding, &expected_anchor)
        && children_of(world, slot.anchor) == expected_children
        && world.get::<ChildOf>(slot.slot).map(ChildOf::parent) == Some(slot.anchor)
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
    ComponentManifest::new(ComponentManifest::observed_component_ids(world, &ids))
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
    outlet.anchor_binding().anchor() == outlet.anchor()
        && outlet
            .anchor_binding()
            .is_current(world, outlet.parent(), true)
}

fn validate_outlet_claim(world: &World, slot: &ProvisionalChildSlot) -> bool {
    slot.anchor_binding.anchor() == slot.anchor
        && slot.anchor_binding.is_current(world, &slot.parent, true)
}

fn physical_anchor_manifest_is_current(
    world: &World,
    binding: &OutletBinding,
    manifest: &ComponentManifest,
) -> bool {
    manifest.matches(world, binding.anchor())
        && binding.physical_claim().is_none_or(|claim| {
            world
                .resource::<RouteEntityRegistry>()
                .record(binding.anchor())
                .is_some_and(|record| record.claim() == *claim && record.manifest() == manifest)
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
    ComponentManifest::new(ComponentManifest::observed_component_ids(world, &explicit))
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
        ^ slot.anchor_binding.opaque_digest_part().rotate_left(23)
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
pub(crate) mod tests;
