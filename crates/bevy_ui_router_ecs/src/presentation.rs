use std::collections::HashSet;

use bevy_app::Inherited;
use bevy_camera::visibility::Visibility;
use bevy_color::Color;
use bevy_ecs::{
    change_detection::Mut,
    component::{Component, ComponentId},
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_input_focus::InputFocus;
use bevy_picking::{Pickable, hover::PickingInteraction};
use bevy_ui::{BackgroundColor, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, Val};
use bevy_ui_router_core::{
    frame::RouteFrameVersion,
    integration::{
        FrameMountTarget, FrameTask, frame_versions_share_router, host_version_matches_frame,
    },
};

use crate::{
    components::{ChildSlotAuthority, ChildSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier},
    content::{AcceptedContentPlan, RealizedContentKind},
    host::{ValidatedHostCandidate, validated_host_candidate_is_current},
    input::RouteFocusState,
    mount::MountedChildSlot,
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim, QuarantineFacts,
        QuarantineLeaseTable, RegistryOwner, RouteEntityRegistry,
        apply_prepared_quarantine_transfer, apply_prepared_unresolved_residual_transfer,
        prepare_unresolved_residual_transfer, prepare_verified_frame_quarantine_transfer,
    },
};

#[cfg(test)]
use crate::components::{
    ChildSlotAuthorityProbe, ChildSlotBarrierProbe, ChildSlotNodeProbe, ChildSlotRelationshipProbe,
};

#[derive(Clone, Debug)]
pub(crate) struct FrameMountBinding {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    kind: FrameMountBindingKind,
}

#[derive(Clone, Debug)]
enum FrameMountBindingKind {
    HostTop,
    ChildOutlet {
        parent: RouteFrameVersion,
        outlet: bevy_ui_router_core::slot::RouteSlotKey,
        anchor: Entity,
        anchor_claim: ExactRegistryClaim,
        expected_anchor_children: Vec<Entity>,
    },
}

impl FrameMountBinding {
    fn host_top(slot: Entity, slot_claim: ExactRegistryClaim) -> Self {
        Self {
            slot,
            slot_claim,
            kind: FrameMountBindingKind::HostTop,
        }
    }

    fn from_mounted_child_slot(mounted: &MountedChildSlot) -> Self {
        Self {
            slot: mounted.slot(),
            slot_claim: mounted.slot_claim().clone(),
            kind: FrameMountBindingKind::ChildOutlet {
                parent: mounted.parent().clone(),
                outlet: mounted.outlet().clone(),
                anchor: mounted.anchor(),
                anchor_claim: mounted.anchor_claim().clone(),
                expected_anchor_children: mounted.expected_anchor_children().to_vec(),
            },
        }
    }

    pub(crate) const fn slot(&self) -> Entity {
        self.slot
    }

    pub(crate) const fn slot_claim(&self) -> &ExactRegistryClaim {
        &self.slot_claim
    }

    #[cfg(test)]
    pub(crate) const fn is_child_outlet(&self) -> bool {
        matches!(self.kind, FrameMountBindingKind::ChildOutlet { .. })
    }
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
struct FrameShell(RouteFrameVersion);

#[derive(Component, Clone, Debug, Eq, PartialEq)]
struct FrameContentTarget(RouteFrameVersion);

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
struct MountChildrenManifestCasDriftProbe {
    entity: Entity,
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameOutput {
    frame: RouteFrameVersion,
    local_index: usize,
    primitive: Option<crate::content::ContentPrimitiveKind>,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameLifecycle(pub(crate) EntityLifecycle);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealizedRole {
    Shell,
    ContentTarget,
    Empty,
    SolidRect,
}

#[derive(Clone, Debug)]
struct RealizedEntity {
    entity: Entity,
    claim: ExactRegistryClaim,
    role: RealizedRole,
    parent: Entity,
    solid_rect: Option<crate::content::SolidRect>,
}

#[derive(Clone, Debug)]
pub(crate) struct RealizedOutlet {
    parent: RouteFrameVersion,
    key: bevy_ui_router_core::slot::RouteSlotKey,
    anchor: Entity,
    anchor_claim: ExactRegistryClaim,
}

impl RealizedOutlet {
    pub(crate) const fn parent(&self) -> &RouteFrameVersion {
        &self.parent
    }

    pub(crate) const fn key(&self) -> &bevy_ui_router_core::slot::RouteSlotKey {
        &self.key
    }

    pub(crate) const fn anchor(&self) -> Entity {
        self.anchor
    }

    pub(crate) const fn anchor_claim(&self) -> &ExactRegistryClaim {
        &self.anchor_claim
    }
}

#[derive(Debug)]
pub(crate) struct RealizedFrame {
    frame: RouteFrameVersion,
    host_candidate: ValidatedHostCandidate,
    mount: FrameMountBinding,
    prior_mount_children: Vec<Entity>,
    shell: Entity,
    entities: Vec<RealizedEntity>,
    outlets: Vec<RealizedOutlet>,
    prewrite_complete: bool,
}

pub(crate) struct VerifiedFrameQuarantineTransfer {
    claims: Vec<ExactRegistryClaim>,
    manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

impl VerifiedFrameQuarantineTransfer {
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
}

#[derive(Clone, Copy)]
pub(crate) struct FrameMutationTarget {
    entity: Entity,
    current: EntityLifecycle,
    next: EntityLifecycle,
    active_after: bool,
    interactive: bool,
}

struct PreparedEntityMutation<'w> {
    visibility: Mut<'w, Visibility>,
    pickable: Mut<'w, Pickable>,
    barrier: Mut<'w, RouterInputBarrier>,
    lifecycle: Mut<'w, FrameLifecycle>,
    next: EntityLifecycle,
    active_after: bool,
    interactive: bool,
}

pub(crate) struct PreparedPresentationMutation<'w> {
    entities: Vec<(usize, PreparedEntityMutation<'w>)>,
}

impl PreparedPresentationMutation<'_> {
    pub(crate) fn apply(self) {
        let entity_count = self.entities.len();
        self.apply_with_split(entity_count, || {});
    }

    pub(crate) fn apply_with_split(self, split_at: usize, between: impl FnOnce()) {
        let mut entities = self.entities;
        let entering = entities.split_off(split_at);
        Self::apply_entities(entities);
        between();
        Self::apply_entities(entering);
    }

    fn apply_entities(entities: Vec<(usize, PreparedEntityMutation<'_>)>) {
        for (_, mut entity) in entities {
            *entity.visibility = if entity.active_after {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            *entity.pickable = if entity.active_after && entity.interactive {
                ACTIVE_PICKABLE
            } else {
                Pickable::IGNORE
            };
            *entity.barrier = if entity.active_after && entity.interactive {
                RouterInputBarrier::allow_all()
            } else {
                RouterInputBarrier::deny_all()
            };
            *entity.lifecycle = FrameLifecycle(entity.next);
        }
    }
}

impl RealizedFrame {
    pub(crate) const fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub(crate) const fn shell(&self) -> Entity {
        self.shell
    }

    pub(crate) const fn top_slot(&self) -> Entity {
        self.mount.slot
    }

    pub(crate) const fn mount_binding(&self) -> &FrameMountBinding {
        &self.mount
    }

    pub(crate) fn outlet(
        &self,
        key: &bevy_ui_router_core::slot::RouteSlotKey,
    ) -> Option<&RealizedOutlet> {
        self.outlets.iter().find(|outlet| outlet.key() == key)
    }

    pub(crate) fn outlets(&self) -> &[RealizedOutlet] {
        &self.outlets
    }

    pub(crate) fn cleanup_members(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ExactRegistryClaim, Entity)> {
        self.entities
            .iter()
            .map(|entity| (&entity.claim, entity.parent))
    }

    pub(crate) const fn prewrite_complete(&self) -> bool {
        self.prewrite_complete
    }

    pub(crate) fn claims(&self) -> impl ExactSizeIterator<Item = &ExactRegistryClaim> {
        self.entities.iter().map(|entity| &entity.claim)
    }

    pub(crate) fn obligation_count(&self) -> usize {
        self.entities.len()
    }

    pub(crate) fn interactive_entities(&self) -> Vec<Entity> {
        self.entities
            .iter()
            .filter_map(|realized| {
                (realized.role == RealizedRole::SolidRect).then_some(realized.entity)
            })
            .collect()
    }

    pub(crate) fn mutation_targets(
        &self,
        current: EntityLifecycle,
        next: EntityLifecycle,
        active_after: bool,
    ) -> Vec<FrameMutationTarget> {
        self.entities
            .iter()
            .map(|entity| FrameMutationTarget {
                entity: entity.entity,
                current,
                next,
                active_after,
                interactive: entity.role == RealizedRole::SolidRect,
            })
            .collect()
    }

    pub(crate) fn lifecycle_transitions(
        &self,
        next: EntityLifecycle,
    ) -> Vec<(ExactRegistryClaim, EntityLifecycle)> {
        self.entities
            .iter()
            .map(|entity| (entity.claim.clone(), next))
            .collect()
    }

    pub(crate) fn replace_claims(&mut self, claims: &[ExactRegistryClaim]) {
        for (entity, claim) in self.entities.iter_mut().zip(claims) {
            entity.claim = claim.clone();
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn audit_content(
        &self,
    ) -> impl Iterator<Item = (Entity, crate::content::SolidRect)> + '_ {
        self.entities
            .iter()
            .filter_map(|entity| entity.solid_rect.map(|rect| (entity.entity, rect)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RealizationError {
    MissingTopSlot,
    InvalidTopSlot,
    InvalidMount,
    RegistryRejected,
    PostWriteDrift,
}

pub(crate) struct DeferredRealizationFailure {
    error: RealizationError,
    residual: Option<RealizedFrame>,
}

impl DeferredRealizationFailure {
    fn before_complete_frame(error: RealizationError) -> Self {
        Self {
            error,
            residual: None,
        }
    }

    fn with_residual(error: RealizationError, residual: RealizedFrame) -> Self {
        Self {
            error,
            residual: Some(residual),
        }
    }

    pub(crate) fn into_parts(self) -> (RealizationError, Option<RealizedFrame>) {
        (self.error, self.residual)
    }
}

#[cfg(test)]
pub(crate) fn realize_frame(
    world: &mut World,
    host_candidate: ValidatedHostCandidate,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
    frame: RouteFrameVersion,
    output: u64,
    plan: AcceptedContentPlan,
) -> Result<RealizedFrame, RealizationError> {
    match realize_frame_deferred(
        world,
        host_candidate,
        top_slot,
        top_slot_claim,
        frame,
        output,
        plan,
    ) {
        Ok(frame) => Ok(frame),
        Err(failure) => {
            let (error, residual) = failure.into_parts();
            if let Some(residual) = residual {
                settle_failed_realization(world, top_slot, &residual);
            }
            Err(error)
        }
    }
}

pub(crate) fn realize_frame_deferred(
    world: &mut World,
    host_candidate: ValidatedHostCandidate,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
    frame: RouteFrameVersion,
    output: u64,
    plan: AcceptedContentPlan,
) -> Result<RealizedFrame, DeferredRealizationFailure> {
    realize_frame_with_binding_deferred(
        world,
        host_candidate,
        FrameMountBinding::host_top(top_slot, top_slot_claim),
        frame,
        output,
        plan,
    )
}

pub(crate) fn realize_child_frame_deferred(
    world: &mut World,
    parent: &RealizedFrame,
    mounted: &MountedChildSlot,
    task: &FrameTask,
    output: u64,
    plan: AcceptedContentPlan,
) -> Result<RealizedFrame, DeferredRealizationFailure> {
    let FrameMountTarget::ChildOutlet {
        parent: task_parent,
        outlet: task_outlet,
    } = task.mount_target()
    else {
        return Err(DeferredRealizationFailure::before_complete_frame(
            RealizationError::InvalidMount,
        ));
    };
    let parent_outlet_is_exact = parent.outlet(mounted.outlet()).is_some_and(|outlet| {
        outlet.parent() == mounted.parent()
            && outlet.anchor() == mounted.anchor()
            && outlet.anchor_claim() == mounted.anchor_claim()
    });
    if parent.frame() != mounted.parent()
        || task_parent != *mounted.parent()
        || task_outlet != *mounted.outlet()
        || task.frame().slot() != mounted.slot_id()
        || !parent_outlet_is_exact
    {
        return Err(DeferredRealizationFailure::before_complete_frame(
            RealizationError::InvalidMount,
        ));
    }
    realize_frame_with_binding_deferred(
        world,
        parent.host_candidate.clone(),
        FrameMountBinding::from_mounted_child_slot(mounted),
        task.frame().clone(),
        output,
        plan,
    )
}

fn realize_frame_with_binding_deferred(
    world: &mut World,
    host_candidate: ValidatedHostCandidate,
    mount: FrameMountBinding,
    frame: RouteFrameVersion,
    output: u64,
    plan: AcceptedContentPlan,
) -> Result<RealizedFrame, DeferredRealizationFailure> {
    let mount_slot = mount.slot();
    let runtime_components = runtime_ui_component_ids(world);
    validate_mount_binding_allowing_additions(
        world,
        &host_candidate,
        &mount,
        &frame,
        &runtime_components,
    )
    .map_err(DeferredRealizationFailure::before_complete_frame)?;
    synchronize_manifest_allowing_additions(
        world,
        mount_slot,
        mount.slot_claim(),
        &runtime_components,
    )
    .map_err(DeferredRealizationFailure::before_complete_frame)?;
    validate_mount_binding(world, &host_candidate, &mount, &frame)
        .map_err(DeferredRealizationFailure::before_complete_frame)?;
    let mount_manifest_before_children = world
        .resource::<RouteEntityRegistry>()
        .record(mount_slot)
        .expect("validated mount slot remains registered")
        .manifest()
        .clone();
    let mount_children_before = world
        .get::<Children>(mount_slot)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let inherited_runtime_components = world
        .resource::<RouteEntityRegistry>()
        .record(mount_slot)
        .expect("validated mount slot remains registered")
        .manifest()
        .component_ids()
        .iter()
        .copied()
        .filter(|id| runtime_components.contains(id))
        .collect::<Vec<_>>();

    let mut realized = Vec::with_capacity(plan.entries().len() + 2);
    let shell = allocate_registered(
        world,
        RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone())),
        RealizedRole::Shell,
        mount_slot,
        true,
        &inherited_runtime_components,
    )
    .map_err(DeferredRealizationFailure::before_complete_frame)?;
    let shell_entity = shell.entity;
    realized.push(shell);
    let content_target_entity = world.spawn_empty().id();
    let content_target = match register_allocated(
        world,
        content_target_entity,
        RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame.clone())),
        RealizedRole::ContentTarget,
        shell_entity,
        !plan.roots().is_empty(),
        &inherited_runtime_components,
    ) {
        Ok(content_target) => content_target,
        Err(error) => {
            let residual = partial_realized_frame(
                frame,
                host_candidate,
                mount,
                mount_children_before,
                shell_entity,
                realized,
            );
            return Err(DeferredRealizationFailure::with_residual(error, residual));
        }
    };
    realized.push(content_target);

    let mut outputs = Vec::with_capacity(plan.entries().len());
    for entry in plan.entries() {
        let role = match entry.kind() {
            RealizedContentKind::Empty => RealizedRole::Empty,
            RealizedContentKind::SolidRect(_) => RealizedRole::SolidRect,
        };
        let entity = world.spawn_empty().id();
        let owner = RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame: frame.clone(),
            output,
            local_index: entry.local_index(),
        });
        let parent = plan
            .relationships()
            .iter()
            .find_map(|(parent, child)| (*child == entry.local_index()).then_some(*parent))
            .and_then(|parent| {
                outputs
                    .iter()
                    .find_map(|(index, entity)| (*index == parent).then_some(*entity))
            })
            .unwrap_or(content_target_entity);
        let has_children = plan
            .relationships()
            .iter()
            .any(|(parent, _)| *parent == entry.local_index());
        let mut registered = match register_allocated(
            world,
            entity,
            owner,
            role,
            parent,
            has_children,
            &inherited_runtime_components,
        ) {
            Ok(registered) => registered,
            Err(error) => {
                let residual = partial_realized_frame(
                    frame,
                    host_candidate,
                    mount,
                    mount_children_before,
                    shell_entity,
                    realized,
                );
                return Err(DeferredRealizationFailure::with_residual(error, residual));
            }
        };
        registered.solid_rect = match entry.kind() {
            RealizedContentKind::SolidRect(rect) => Some(rect),
            RealizedContentKind::Empty => None,
        };
        outputs.push((entry.local_index(), entity));
        realized.push(registered);
    }

    insert_shell_components(world, &realized[0], &frame, RealizedRole::Shell);
    insert_shell_components(world, &realized[1], &frame, RealizedRole::ContentTarget);
    for (entry, realized) in plan.entries().iter().zip(&realized[2..]) {
        insert_realized_components(world, realized, &frame, entry.kind());
    }
    world.flush();
    for realized in &realized {
        world
            .entity_mut(realized.entity)
            .insert(ChildOf(realized.parent));
    }
    world.flush();

    let outlets = plan
        .outlets()
        .iter()
        .map(|outlet| {
            let anchor = outputs
                .iter()
                .find_map(|(index, entity)| (*index == outlet.local_index()).then_some(*entity))
                .expect("accepted outlet local index must map to a realized output");
            let anchor_claim = realized
                .iter()
                .find(|entity| entity.entity == anchor)
                .expect("realized outlet anchor must retain its exact claim")
                .claim
                .clone();
            RealizedOutlet {
                parent: frame.clone(),
                key: outlet.key().clone(),
                anchor,
                anchor_claim,
            }
        })
        .collect();
    let result = RealizedFrame {
        frame,
        host_candidate,
        mount,
        prior_mount_children: mount_children_before,
        shell: realized[0].entity,
        entities: realized,
        outlets,
        prewrite_complete: true,
    };
    clear_frame_focus(world, &result);
    let children = world.register_component::<Children>();
    let mount_slot_changes = [children];
    #[cfg(test)]
    apply_mount_children_manifest_cas_drift_probe(world, result.mount.slot());
    let manifests_valid = if validate_mount_binding_allowing_additions(
        world,
        &result.host_candidate,
        &result.mount,
        result.frame(),
        &mount_slot_changes,
    )
    .is_err()
        || !mount_slot_child_order_extended_exactly(
            world,
            result.mount.slot(),
            &result.prior_mount_children,
            result.shell(),
        )
        || !mount_slot_children_are_exact(world, &result)
    {
        Err(RealizationError::PostWriteDrift)
    } else {
        synchronize_manifest_allowing_from_expected(
            world,
            result.mount.slot(),
            result.mount.slot_claim(),
            &mount_manifest_before_children,
            &mount_slot_changes,
        )
        .and_then(|()| {
            if validate_mount_binding(world, &result.host_candidate, &result.mount, result.frame())
                .is_err()
                || !mount_slot_children_are_exact(world, &result)
            {
                return Err(RealizationError::PostWriteDrift);
            }
            for realized in &result.entities {
                if !world
                    .resource::<RouteEntityRegistry>()
                    .record(realized.entity)
                    .is_some_and(|record| record.manifest().matches(world, realized.entity))
                {
                    return Err(RealizationError::PostWriteDrift);
                }
            }
            Ok(())
        })
    };
    if let Err(error) = manifests_valid {
        return Err(DeferredRealizationFailure::with_residual(error, result));
    }

    if validate_realized_frame(world, &result) {
        Ok(result)
    } else {
        Err(DeferredRealizationFailure::with_residual(
            RealizationError::PostWriteDrift,
            result,
        ))
    }
}

fn partial_realized_frame(
    frame: RouteFrameVersion,
    host_candidate: ValidatedHostCandidate,
    mount: FrameMountBinding,
    prior_mount_children: Vec<Entity>,
    shell: Entity,
    entities: Vec<RealizedEntity>,
) -> RealizedFrame {
    debug_assert!(!entities.is_empty());
    RealizedFrame {
        frame,
        host_candidate,
        mount,
        prior_mount_children,
        shell,
        entities,
        outlets: Vec::new(),
        prewrite_complete: false,
    }
}

fn validate_mount_binding(
    world: &World,
    host_candidate: &ValidatedHostCandidate,
    mount: &FrameMountBinding,
    frame: &RouteFrameVersion,
) -> Result<(), RealizationError> {
    validate_mount_binding_allowing_additions(world, host_candidate, mount, frame, &[])
}

fn validate_mount_binding_allowing_additions(
    world: &World,
    host_candidate: &ValidatedHostCandidate,
    mount: &FrameMountBinding,
    frame: &RouteFrameVersion,
    allowed_manifest_additions: &[ComponentId],
) -> Result<(), RealizationError> {
    match &mount.kind {
        FrameMountBindingKind::HostTop => validate_top_slot(
            world,
            host_candidate,
            mount.slot,
            &mount.slot_claim,
            frame,
            allowed_manifest_additions,
        ),
        FrameMountBindingKind::ChildOutlet {
            parent,
            outlet,
            anchor,
            anchor_claim,
            expected_anchor_children,
        } => validate_child_mount_slot(
            world,
            host_candidate,
            mount.slot,
            &mount.slot_claim,
            parent,
            outlet,
            *anchor,
            anchor_claim,
            expected_anchor_children,
            frame,
            allowed_manifest_additions,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_child_mount_slot(
    world: &World,
    host_candidate: &ValidatedHostCandidate,
    slot_entity: Entity,
    slot_claim: &ExactRegistryClaim,
    parent: &RouteFrameVersion,
    outlet: &bevy_ui_router_core::slot::RouteSlotKey,
    anchor: Entity,
    anchor_claim: &ExactRegistryClaim,
    expected_anchor_children: &[Entity],
    frame: &RouteFrameVersion,
    allowed_manifest_additions: &[ComponentId],
) -> Result<(), RealizationError> {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(RealizationError::InvalidMount);
    };
    let Some(slot_record) = registry.record(slot_entity) else {
        return Err(RealizationError::InvalidMount);
    };
    let Some(anchor_record) = registry.record(anchor) else {
        return Err(RealizationError::InvalidMount);
    };
    let owner_is_exact = matches!(
        slot_record.owner(),
        RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent: owner_parent,
            outlet: owner_outlet,
            slot,
        }) if owner_parent == parent && owner_outlet == outlet && *slot == frame.slot()
    );
    let anchor_owner_is_exact = matches!(
        anchor_record.owner(),
        RegistryOwner::Committed(EntityOwner::FrameOutput {
            frame: owner_frame,
            ..
        }) if owner_frame == parent
    );
    let expected_node = Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    };
    if !frame_versions_share_router(frame, parent)
        || frame.host() != parent.host()
        || slot_record.claim() != *slot_claim
        || !owner_is_exact
        || slot_record.lifecycle() != EntityLifecycle::PreparedInert
        || !manifest_matches_allowing_additions(
            world,
            slot_entity,
            slot_record.manifest(),
            allowed_manifest_additions,
        )
        || world.get::<RouteOwnershipSeal>(slot_entity)
            != Some(&RouteOwnershipSeal::new(slot_record.seal()))
        || world.get::<Visibility>(slot_entity) != Some(&Visibility::Hidden)
        || world.get::<Pickable>(slot_entity) != Some(&Pickable::IGNORE)
        || !world
            .get::<RouterInputBarrier>(slot_entity)
            .is_some_and(RouterInputBarrier::denies_all)
        || world.get::<Node>(slot_entity) != Some(&expected_node)
        || world.get::<ChildSlotLifecycle>(slot_entity)
            != Some(&ChildSlotLifecycle::RegisteredInert)
        || !world
            .get::<ChildSlotAuthority>(slot_entity)
            .is_some_and(|authority| {
                authority.parent() == parent
                    && authority.outlet() == outlet
                    && authority.slot() == frame.slot()
            })
        || world.get::<ChildOf>(slot_entity).map(ChildOf::parent) != Some(anchor)
        || anchor_record.claim() != *anchor_claim
        || !anchor_owner_is_exact
        || !anchor_record.manifest().matches(world, anchor)
        || world.get::<RouteOwnershipSeal>(anchor)
            != Some(&RouteOwnershipSeal::new(anchor_record.seal()))
        || world
            .get::<Children>(anchor)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
            != expected_anchor_children
        || !validated_host_candidate_is_current(world, host_candidate)
    {
        return Err(RealizationError::InvalidMount);
    }
    Ok(())
}

fn validate_top_slot(
    world: &World,
    host_candidate: &ValidatedHostCandidate,
    top_slot: Entity,
    top_slot_claim: &ExactRegistryClaim,
    frame: &RouteFrameVersion,
    allowed_manifest_additions: &[ComponentId],
) -> Result<(), RealizationError> {
    let host_entity = host_candidate.host();
    let record = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(top_slot))
        .ok_or(RealizationError::MissingTopSlot)?;
    let RegistryOwner::Committed(EntityOwner::HostTopSlot {
        host: host_version,
        slot,
    }) = record.owner()
    else {
        return Err(RealizationError::InvalidTopSlot);
    };
    if record.claim() != *top_slot_claim
        || *slot != frame.slot()
        || !host_version_matches_frame(host_version, frame)
        || record.lifecycle() != EntityLifecycle::PreparedInert
        || !manifest_matches_allowing_additions(
            world,
            top_slot,
            record.manifest(),
            allowed_manifest_additions,
        )
        || world.get::<ChildOf>(top_slot).map(ChildOf::parent) != Some(host_entity)
        || world.get::<RouteOwnershipSeal>(top_slot)
            != Some(&RouteOwnershipSeal::new(record.seal()))
        || world.get::<Visibility>(top_slot) != Some(&Visibility::Visible)
        || world.get::<Pickable>(top_slot) != Some(&Pickable::IGNORE)
        || !world
            .get::<RouterInputBarrier>(top_slot)
            .is_some_and(RouterInputBarrier::denies_all)
        || world.get::<crate::components::ProvisionalSlotLifecycle>(top_slot)
            != Some(&crate::components::ProvisionalSlotLifecycle::PreparedInert)
        || world.get::<Node>(host_entity).is_none()
        || !validated_host_candidate_is_current(world, host_candidate)
    {
        return Err(RealizationError::InvalidTopSlot);
    }
    Ok(())
}

fn manifest_matches_allowing_additions(
    world: &World,
    entity: Entity,
    expected: &ComponentManifest,
    allowed_additions: &[ComponentId],
) -> bool {
    let Some(actual) = ComponentManifest::from_world(world, entity) else {
        return false;
    };
    expected
        .component_ids()
        .iter()
        .all(|id| actual.component_ids().contains(id))
        && actual
            .component_ids()
            .iter()
            .all(|id| expected.component_ids().contains(id) || allowed_additions.contains(id))
}

fn allocate_registered(
    world: &mut World,
    owner: RegistryOwner,
    role: RealizedRole,
    parent: Entity,
    has_children: bool,
    runtime_components: &[ComponentId],
) -> Result<RealizedEntity, RealizationError> {
    let entity = world.spawn_empty().id();
    register_allocated(
        world,
        entity,
        owner,
        role,
        parent,
        has_children,
        runtime_components,
    )
}

fn register_allocated(
    world: &mut World,
    entity: Entity,
    owner: RegistryOwner,
    role: RealizedRole,
    parent: Entity,
    has_children: bool,
    runtime_components: &[ComponentId],
) -> Result<RealizedEntity, RealizationError> {
    let manifest = realization_manifest(world, role, has_children, runtime_components);
    let claim = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        let seal = registry.issue_seal();
        if registry
            .register(
                entity,
                owner,
                seal,
                manifest,
                EntityLifecycle::PreparedInert,
            )
            .is_err()
        {
            world.despawn(entity);
            return Err(RealizationError::RegistryRejected);
        }
        let claim = registry
            .record(entity)
            .expect("registered realization entity is retained")
            .claim();
        claim
    };
    Ok(RealizedEntity {
        entity,
        claim,
        role,
        parent,
        solid_rect: None,
    })
}

fn insert_shell_components(
    world: &mut World,
    realized: &RealizedEntity,
    frame: &RouteFrameVersion,
    role: RealizedRole,
) {
    let marker = match role {
        RealizedRole::Shell => Some(FrameShell(frame.clone())),
        _ => None,
    };
    world.entity_mut(realized.entity).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
        RouteOwnershipSeal::new(realized.claim.seal()),
    ));
    world.entity_mut(realized.entity).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    });
    if let Some(marker) = marker {
        world.entity_mut(realized.entity).insert(marker);
    } else {
        world
            .entity_mut(realized.entity)
            .insert(FrameContentTarget(frame.clone()));
    }
    world
        .entity_mut(realized.entity)
        .insert(FrameLifecycle(EntityLifecycle::PreparedInert));
}

fn insert_realized_components(
    world: &mut World,
    realized: &RealizedEntity,
    frame: &RouteFrameVersion,
    kind: RealizedContentKind,
) {
    let (node, background, primitive) = match kind {
        RealizedContentKind::Empty => (
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            None,
            None,
        ),
        RealizedContentKind::SolidRect(rect) => {
            let color = rect.color();
            (
                Node {
                    width: Val::Px(f32::from(rect.width_px())),
                    height: Val::Px(f32::from(rect.height_px())),
                    flex_shrink: 0.0,
                    ..Node::default()
                },
                Some(BackgroundColor(Color::srgba_u8(
                    color.red(),
                    color.green(),
                    color.blue(),
                    color.alpha(),
                ))),
                Some(crate::content::ContentPrimitiveKind::SolidRect),
            )
        }
    };
    world.entity_mut(realized.entity).insert((
        Visibility::Hidden,
        Pickable::IGNORE,
        RouterInputBarrier::deny_all(),
        RouteOwnershipSeal::new(realized.claim.seal()),
    ));
    world.entity_mut(realized.entity).insert(node);
    if let Some(background) = background {
        world.entity_mut(realized.entity).insert(background);
    }
    if matches!(kind, RealizedContentKind::SolidRect(_)) {
        world
            .entity_mut(realized.entity)
            .insert(PickingInteraction::None);
    }
    world.entity_mut(realized.entity).insert(FrameOutput {
        frame: frame.clone(),
        local_index: match realized.claim.owner() {
            RegistryOwner::Committed(EntityOwner::FrameOutput { local_index, .. }) => *local_index,
            _ => unreachable!("output claim has output owner"),
        },
        primitive,
    });
    world
        .entity_mut(realized.entity)
        .insert(FrameLifecycle(EntityLifecycle::PreparedInert));
}

fn realization_manifest(
    world: &mut World,
    role: RealizedRole,
    has_children: bool,
    runtime_components: &[ComponentId],
) -> ComponentManifest {
    let mut explicit = vec![
        world.register_component::<Node>(),
        world.register_component::<Visibility>(),
        world.register_component::<Pickable>(),
        world.register_component::<RouterInputBarrier>(),
        world.register_component::<RouteOwnershipSeal>(),
        world.register_component::<FrameLifecycle>(),
        world.register_component::<ChildOf>(),
    ];
    explicit.push(match role {
        RealizedRole::Shell => world.register_component::<FrameShell>(),
        RealizedRole::ContentTarget => world.register_component::<FrameContentTarget>(),
        RealizedRole::Empty | RealizedRole::SolidRect => world.register_component::<FrameOutput>(),
    });
    if role == RealizedRole::SolidRect {
        explicit.push(world.register_component::<BackgroundColor>());
        explicit.push(world.register_component::<PickingInteraction>());
    }
    if has_children {
        explicit.push(world.register_component::<Children>());
    }
    explicit.extend(runtime_components.iter().copied());
    let mut complete = explicit.clone();
    for component in explicit {
        if let Some(required) = world.get_required_components_by_id(component) {
            complete.extend(required.iter_ids());
        }
    }
    ComponentManifest::new(complete)
}

fn runtime_ui_component_ids(world: &mut World) -> [ComponentId; 2] {
    [
        world.register_component::<Inherited<ComputedUiTargetCamera>>(),
        world.register_component::<Inherited<ComputedUiRenderTargetInfo>>(),
    ]
}

#[cfg(test)]
pub(crate) fn assert_locked_component_manifest_closure(world: &mut World, frame: &RealizedFrame) {
    let runtime = runtime_ui_component_ids(world);
    let mount_slot_explicit = match &frame.mount.kind {
        FrameMountBindingKind::HostTop => vec![
            world.register_component::<Visibility>(),
            world.register_component::<Pickable>(),
            world.register_component::<RouterInputBarrier>(),
            world.register_component::<RouteOwnershipSeal>(),
            world.register_component::<crate::components::ProvisionalSlotLifecycle>(),
            world.register_component::<crate::components::ProvisionalStagingProbe>(),
            world.register_component::<crate::components::ProvisionalPostWriteProbe>(),
            world.register_component::<Node>(),
            world.register_component::<ChildOf>(),
            world.register_component::<Children>(),
            runtime[0],
            runtime[1],
        ],
        FrameMountBindingKind::ChildOutlet { .. } => vec![
            world.register_component::<Visibility>(),
            world.register_component::<Pickable>(),
            world.register_component::<RouterInputBarrier>(),
            world.register_component::<ChildSlotBarrierProbe>(),
            world.register_component::<Node>(),
            world.register_component::<ChildSlotNodeProbe>(),
            world.register_component::<RouteOwnershipSeal>(),
            world.register_component::<ChildSlotLifecycle>(),
            world.register_component::<ChildSlotAuthority>(),
            world.register_component::<ChildSlotAuthorityProbe>(),
            world.register_component::<ChildOf>(),
            world.register_component::<ChildSlotRelationshipProbe>(),
            world.register_component::<Children>(),
            runtime[0],
            runtime[1],
        ],
    };
    assert_manifest_is_exact(world, frame.mount.slot(), mount_slot_explicit);

    for realized in &frame.entities {
        let mut explicit = vec![
            world.register_component::<Node>(),
            world.register_component::<Visibility>(),
            world.register_component::<Pickable>(),
            world.register_component::<RouterInputBarrier>(),
            world.register_component::<RouteOwnershipSeal>(),
            world.register_component::<FrameLifecycle>(),
            world.register_component::<ChildOf>(),
            runtime[0],
            runtime[1],
        ];
        explicit.push(match realized.role {
            RealizedRole::Shell => world.register_component::<FrameShell>(),
            RealizedRole::ContentTarget => world.register_component::<FrameContentTarget>(),
            RealizedRole::Empty | RealizedRole::SolidRect => {
                world.register_component::<FrameOutput>()
            }
        });
        if realized.role == RealizedRole::SolidRect {
            explicit.push(world.register_component::<BackgroundColor>());
            explicit.push(world.register_component::<PickingInteraction>());
        }
        if frame
            .entities
            .iter()
            .any(|child| child.parent == realized.entity)
        {
            explicit.push(world.register_component::<Children>());
        }
        assert_manifest_is_exact(world, realized.entity, explicit);
    }
}

#[cfg(test)]
fn assert_manifest_is_exact(world: &World, entity: Entity, explicit: Vec<ComponentId>) {
    let mut expected = explicit.clone();
    for component in explicit {
        if let Some(required) = world.get_required_components_by_id(component) {
            expected.extend(required.iter_ids());
        }
    }
    expected.sort_unstable();
    expected.dedup();
    let mut actual = world
        .get_entity(entity)
        .expect("frozen manifest entity must exist")
        .archetype()
        .components()
        .to_vec();
    actual.sort_unstable();
    actual.dedup();
    let registry = world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .expect("frozen manifest entity must retain an exact registry record")
        .manifest()
        .component_ids();
    assert_eq!(
        actual, expected,
        "locked Bevy archetype drifted for {entity:?}"
    );
    assert_eq!(
        registry, expected,
        "registry manifest drifted for {entity:?}"
    );
}

fn synchronize_manifest_allowing_additions(
    world: &mut World,
    entity: Entity,
    claim: &ExactRegistryClaim,
    allowed_additions: &[ComponentId],
) -> Result<(), RealizationError> {
    let expected = world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .filter(|record| record.claim() == *claim)
        .map(|record| record.manifest().clone())
        .ok_or(RealizationError::InvalidTopSlot)?;
    let actual =
        ComponentManifest::from_world(world, entity).ok_or(RealizationError::InvalidTopSlot)?;
    if actual
        .component_ids()
        .iter()
        .any(|id| !expected.component_ids().contains(id) && !allowed_additions.contains(id))
        || expected
            .component_ids()
            .iter()
            .any(|id| !actual.component_ids().contains(id))
    {
        return Err(RealizationError::PostWriteDrift);
    }
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifest_exact(claim, &expected, actual.clone())
    {
        return Err(RealizationError::InvalidMount);
    }
    exact_manifest_cas_is_current(world, entity, claim, &actual)
        .then_some(())
        .ok_or(RealizationError::PostWriteDrift)
}

fn synchronize_manifest_allowing(
    world: &mut World,
    entity: Entity,
    claim: &ExactRegistryClaim,
    allowed_changes: &[ComponentId],
) -> Result<(), RealizationError> {
    let expected = world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .filter(|record| record.claim() == *claim)
        .map(|record| record.manifest().clone())
        .ok_or(RealizationError::InvalidTopSlot)?;
    synchronize_manifest_allowing_from_expected(world, entity, claim, &expected, allowed_changes)
}

fn synchronize_manifest_allowing_from_expected(
    world: &mut World,
    entity: Entity,
    claim: &ExactRegistryClaim,
    expected: &ComponentManifest,
    allowed_changes: &[ComponentId],
) -> Result<(), RealizationError> {
    let actual =
        ComponentManifest::from_world(world, entity).ok_or(RealizationError::InvalidTopSlot)?;
    let unexpected_change = actual
        .component_ids()
        .iter()
        .any(|id| !expected.component_ids().contains(id) && !allowed_changes.contains(id))
        || expected
            .component_ids()
            .iter()
            .any(|id| !actual.component_ids().contains(id) && !allowed_changes.contains(id));
    if unexpected_change {
        return Err(RealizationError::PostWriteDrift);
    }
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifest_exact(claim, expected, actual.clone())
    {
        return Err(RealizationError::InvalidMount);
    }
    exact_manifest_cas_is_current(world, entity, claim, &actual)
        .then_some(())
        .ok_or(RealizationError::PostWriteDrift)
}

fn exact_manifest_cas_is_current(
    world: &World,
    entity: Entity,
    claim: &ExactRegistryClaim,
    expected: &ComponentManifest,
) -> bool {
    world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(entity))
        .is_some_and(|record| {
            record.claim() == *claim
                && record.manifest() == expected
                && expected.matches(world, entity)
        })
}

pub(crate) fn validate_realized_frame(world: &World, frame: &RealizedFrame) -> bool {
    if validate_mount_binding(world, &frame.host_candidate, &frame.mount, frame.frame()).is_err()
        || !mount_slot_children_are_exact(world, frame)
        || frame_focus_is_in_subtree(world, frame)
    {
        return false;
    }
    frame.entities.iter().all(|realized| {
        let Some(record) = world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
        else {
            return false;
        };
        let checks = [
            record.claim() == realized.claim,
            record.lifecycle() == EntityLifecycle::PreparedInert,
            record.manifest().matches(world, realized.entity),
            world.get::<RouteOwnershipSeal>(realized.entity)
                == Some(&RouteOwnershipSeal::new(realized.claim.seal())),
            world.get::<Visibility>(realized.entity) == Some(&Visibility::Hidden),
            world.get::<Pickable>(realized.entity) == Some(&Pickable::IGNORE),
            world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(RouterInputBarrier::denies_all),
            world.get::<FrameLifecycle>(realized.entity)
                == Some(&FrameLifecycle(EntityLifecycle::PreparedInert)),
            world.get::<ChildOf>(realized.entity).map(ChildOf::parent) == Some(realized.parent),
            exact_children_match(
                world,
                realized.entity,
                expected_children(frame, realized.entity),
            ),
            realized_values_match(world, realized, frame.frame()),
        ];
        checks.into_iter().all(|valid| valid)
    })
}

pub(crate) fn prepare_presentation_mutation<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
) -> Option<PreparedPresentationMutation<'w>> {
    let mut query = world.query::<(
        Entity,
        &mut Visibility,
        Option<&mut Pickable>,
        &mut RouterInputBarrier,
        &mut FrameLifecycle,
    )>();
    let mut found = vec![false; targets.len()];
    let mut entities = Vec::with_capacity(targets.len());
    for (entity, visibility, pickable, barrier, lifecycle) in query.iter_mut(world) {
        let Some((index, target)) = targets
            .iter()
            .enumerate()
            .find(|(_, target)| target.entity == entity)
        else {
            continue;
        };
        let expected_pickable = if target.current == EntityLifecycle::Active && target.interactive {
            ACTIVE_PICKABLE
        } else {
            Pickable::IGNORE
        };
        let pickable_valid = pickable
            .as_deref()
            .is_some_and(|pickable| *pickable == expected_pickable);
        if found[index]
            || lifecycle.0 != target.current
            || !pickable_valid
            || (matches!(
                target.current,
                EntityLifecycle::PreparedInert | EntityLifecycle::Leaving
            ) && (*visibility != Visibility::Hidden || !barrier.denies_all()))
            || (target.current == EntityLifecycle::Active
                && (*visibility != Visibility::Visible
                    || if target.interactive {
                        !barrier.allows_all()
                    } else {
                        !barrier.denies_all()
                    }))
        {
            return None;
        }
        found[index] = true;
        entities.push((
            index,
            PreparedEntityMutation {
                visibility,
                pickable: pickable.expect("validated Pickable remains borrowed"),
                barrier,
                lifecycle,
                next: target.next,
                active_after: target.active_after,
                interactive: target.interactive,
            },
        ));
    }
    entities.sort_by_key(|(index, _)| *index);
    found
        .into_iter()
        .all(|present| present)
        .then_some(PreparedPresentationMutation { entities })
}

pub(crate) fn prepare_cleanup_quarantine_mutation<'w>(
    world: &'w mut World,
    claims: &[ExactRegistryClaim],
) -> Option<PreparedPresentationMutation<'w>> {
    let targets = claims
        .iter()
        .map(|claim| FrameMutationTarget {
            entity: claim.entity(),
            current: claim.lifecycle(),
            next: EntityLifecycle::Quarantined,
            active_after: false,
            interactive: false,
        })
        .collect::<Vec<_>>();
    prepare_presentation_mutation(world, &targets)
}

pub(crate) fn repair_missing_pickable_for_cleanup_quarantine(
    world: &mut World,
    claims: &[ExactRegistryClaim],
) -> bool {
    if claims.iter().any(|claim| {
        !matches!(
            claim.lifecycle(),
            EntityLifecycle::PreparedInert | EntityLifecycle::Leaving
        )
    }) {
        return false;
    }
    let missing = claims
        .iter()
        .filter_map(|claim| {
            world
                .get::<Pickable>(claim.entity())
                .is_none()
                .then_some(claim.entity())
        })
        .collect::<Vec<_>>();
    let claims_are_inert_and_exact = |world: &World| {
        claims.iter().all(|claim| {
            world
                .resource::<RouteEntityRegistry>()
                .record(claim.entity())
                .is_some_and(|record| record.claim() == *claim)
                && world.get::<RouteOwnershipSeal>(claim.entity())
                    == Some(&RouteOwnershipSeal::new(claim.seal()))
                && world.get::<Visibility>(claim.entity()) == Some(&Visibility::Hidden)
                && world
                    .get::<RouterInputBarrier>(claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
                && world.get::<FrameLifecycle>(claim.entity())
                    == Some(&FrameLifecycle(claim.lifecycle()))
        })
    };
    if !claims_are_inert_and_exact(world) {
        return false;
    }
    for entity in missing {
        world.entity_mut(entity).insert(Pickable::IGNORE);
    }
    world.flush();
    claims_are_inert_and_exact(world)
        && claims
            .iter()
            .all(|claim| world.get::<Pickable>(claim.entity()) == Some(&Pickable::IGNORE))
}

pub(crate) fn validate_active_frame(world: &World, frame: &RealizedFrame) -> bool {
    matches!(
        classify_committed_frame(world, frame, CommittedFrameRole::Preserved),
        CommittedFramePreflight::Ready
    )
}

pub(crate) fn validate_leaving_frame(world: &World, frame: &RealizedFrame) -> bool {
    matches!(
        classify_committed_frame(world, frame, CommittedFrameRole::Leaving),
        CommittedFramePreflight::Ready | CommittedFramePreflight::ReadyWithQuarantine
    )
}

#[derive(Clone, Copy)]
enum CommittedFrameRole {
    Preserved,
    Leaving,
}

#[derive(Clone, Copy)]
enum CommittedFramePreflight {
    Invalid,
    Ready,
    ReadyWithQuarantine,
}

fn classify_committed_frame(
    world: &World,
    frame: &RealizedFrame,
    role: CommittedFrameRole,
) -> CommittedFramePreflight {
    if validate_mount_binding(world, &frame.host_candidate, &frame.mount, frame.frame()).is_ok()
        && mount_slot_children_are_exact(world, frame)
    {
        // Continue with the role-sensitive closure checks below.
    } else {
        return CommittedFramePreflight::Invalid;
    }

    let mut quarantine_on_leave = false;
    for realized in &frame.entities {
        let interactive = realized.role == RealizedRole::SolidRect;
        let Some(record) = world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
        else {
            return CommittedFramePreflight::Invalid;
        };
        if record.claim() != realized.claim
            || record.lifecycle() != EntityLifecycle::Active
            || world.get::<RouteOwnershipSeal>(realized.entity)
                != Some(&RouteOwnershipSeal::new(realized.claim.seal()))
            || world.get::<Visibility>(realized.entity) != Some(&Visibility::Visible)
            || world.get::<Pickable>(realized.entity)
                != Some(if interactive {
                    &ACTIVE_PICKABLE
                } else {
                    &Pickable::IGNORE
                })
            || !world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(|barrier| {
                    if interactive {
                        barrier.allows_all()
                    } else {
                        barrier.denies_all()
                    }
                })
            || world.get::<FrameLifecycle>(realized.entity)
                != Some(&FrameLifecycle(EntityLifecycle::Active))
            || world.get::<ChildOf>(realized.entity).map(ChildOf::parent) != Some(realized.parent)
            || !realized_values_match(world, realized, frame.frame())
        {
            return CommittedFramePreflight::Invalid;
        }
        let Some(has_foreign_components) =
            manifest_foreign_additions(world, realized.entity, record.manifest())
        else {
            return CommittedFramePreflight::Invalid;
        };
        let Some(has_foreign_descendants) =
            owned_children_with_foreign_descendants(world, frame, realized.entity)
        else {
            return CommittedFramePreflight::Invalid;
        };
        quarantine_on_leave |= has_foreign_components || has_foreign_descendants;
    }

    match (role, quarantine_on_leave) {
        (CommittedFrameRole::Leaving, true) => CommittedFramePreflight::ReadyWithQuarantine,
        // Protected additions are observed but untouched while this frame is preserved.
        (CommittedFrameRole::Preserved, _) | (CommittedFrameRole::Leaving, false) => {
            CommittedFramePreflight::Ready
        }
    }
}

fn manifest_foreign_additions(
    world: &World,
    entity: Entity,
    expected: &ComponentManifest,
) -> Option<bool> {
    let Some(actual) = ComponentManifest::from_world(world, entity) else {
        return None;
    };
    if expected
        .component_ids()
        .iter()
        .any(|component| !actual.component_ids().contains(component))
    {
        return None;
    }
    let children = world.components().component_id::<Children>();
    Some(actual.component_ids().iter().any(|component| {
        !expected.component_ids().contains(component) && Some(*component) != children
    }))
}

fn owned_children_with_foreign_descendants(
    world: &World,
    frame: &RealizedFrame,
    parent: Entity,
) -> Option<bool> {
    let expected = expected_children(frame, parent);
    let actual = world
        .get::<Children>(parent)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let owned_actual = actual
        .iter()
        .copied()
        .filter(|child| expected.contains(child))
        .collect::<Vec<_>>();
    if owned_actual != expected
        || actual.iter().any(|child| {
            !expected.contains(child)
                && (world
                    .resource::<RouteEntityRegistry>()
                    .record(*child)
                    .is_some()
                    || world
                        .resource::<QuarantineLeaseTable>()
                        .record(*child)
                        .is_some()
                    || world
                        .resource::<QuarantineLeaseTable>()
                        .unresolved_record(*child)
                        .is_some())
        })
    {
        return None;
    }
    Some(actual.iter().any(|child| !expected.contains(child)))
}

pub(crate) fn isolate_frame_for_settlement(world: &mut World, frame: &RealizedFrame) -> bool {
    if !frame
        .entities
        .iter()
        .all(|realized| frame_claim_is_current(world, realized))
    {
        return false;
    }
    for realized in &frame.entities {
        if world.get::<Visibility>(realized.entity).is_none() {
            world.entity_mut(realized.entity).insert(Visibility::Hidden);
            world.flush();
            if !frame_claim_is_current(world, realized) {
                return false;
            }
        }
        if world.get::<Pickable>(realized.entity).is_none() {
            world.entity_mut(realized.entity).insert(Pickable::IGNORE);
            world.flush();
            if !frame_claim_is_current(world, realized) {
                return false;
            }
        }
        if world.get::<RouterInputBarrier>(realized.entity).is_none() {
            world
                .entity_mut(realized.entity)
                .insert(RouterInputBarrier::deny_all());
            world.flush();
            if !frame_claim_is_current(world, realized) {
                return false;
            }
        }
        *world.get_mut::<Visibility>(realized.entity).unwrap() = Visibility::Hidden;
        *world.get_mut::<Pickable>(realized.entity).unwrap() = Pickable::IGNORE;
        *world
            .get_mut::<RouterInputBarrier>(realized.entity)
            .unwrap() = RouterInputBarrier::deny_all();
    }
    clear_frame_focus(world, frame);
    frame.entities.iter().all(|realized| {
        frame_claim_is_current(world, realized)
            && world.get::<Visibility>(realized.entity) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(realized.entity) == Some(&Pickable::IGNORE)
            && world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(RouterInputBarrier::denies_all)
    })
}

fn frame_claim_is_current(world: &World, realized: &RealizedEntity) -> bool {
    world
        .resource::<RouteEntityRegistry>()
        .record(realized.entity)
        .is_some_and(|record| record.claim() == realized.claim)
        && world.get::<RouteOwnershipSeal>(realized.entity)
            == Some(&RouteOwnershipSeal::new(realized.claim.seal()))
        && world.get::<FrameLifecycle>(realized.entity)
            == Some(&FrameLifecycle(realized.claim.lifecycle()))
}

pub(crate) fn settle_frame(world: &mut World, frame: RealizedFrame) -> Result<(), RealizedFrame> {
    if quarantine_frame(world, &frame) {
        Ok(())
    } else {
        Err(frame)
    }
}

const ACTIVE_PICKABLE: Pickable = Pickable {
    should_block_lower: true,
    is_hoverable: true,
};

fn realized_values_match(
    world: &World,
    realized: &RealizedEntity,
    frame: &RouteFrameVersion,
) -> bool {
    let expected_container = Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    };
    match realized.role {
        RealizedRole::Shell => {
            world.get::<Node>(realized.entity) == Some(&expected_container)
                && world.get::<BackgroundColor>(realized.entity)
                    == Some(&BackgroundColor::default())
                && world.get::<FrameShell>(realized.entity) == Some(&FrameShell(frame.clone()))
        }
        RealizedRole::ContentTarget => {
            world.get::<Node>(realized.entity) == Some(&expected_container)
                && world.get::<BackgroundColor>(realized.entity)
                    == Some(&BackgroundColor::default())
                && world.get::<FrameContentTarget>(realized.entity)
                    == Some(&FrameContentTarget(frame.clone()))
        }
        RealizedRole::Empty | RealizedRole::SolidRect => {
            let Some(RegistryOwner::Committed(EntityOwner::FrameOutput { local_index, .. })) =
                Some(realized.claim.owner())
            else {
                return false;
            };
            let (node, background, primitive) = match realized.solid_rect {
                None => (expected_container, Some(BackgroundColor::default()), None),
                Some(rect) => {
                    let color = rect.color();
                    (
                        Node {
                            width: Val::Px(f32::from(rect.width_px())),
                            height: Val::Px(f32::from(rect.height_px())),
                            flex_shrink: 0.0,
                            ..Node::default()
                        },
                        Some(BackgroundColor(Color::srgba_u8(
                            color.red(),
                            color.green(),
                            color.blue(),
                            color.alpha(),
                        ))),
                        Some(crate::content::ContentPrimitiveKind::SolidRect),
                    )
                }
            };
            world.get::<Node>(realized.entity) == Some(&node)
                && world.get::<BackgroundColor>(realized.entity) == background.as_ref()
                && world.get::<FrameOutput>(realized.entity)
                    == Some(&FrameOutput {
                        frame: frame.clone(),
                        local_index: *local_index,
                        primitive,
                    })
        }
    }
}

fn expected_children(frame: &RealizedFrame, parent: Entity) -> Vec<Entity> {
    frame
        .entities
        .iter()
        .filter_map(|realized| (realized.parent == parent).then_some(realized.entity))
        .collect()
}

fn exact_children_match(world: &World, parent: Entity, expected: Vec<Entity>) -> bool {
    let actual = world
        .get::<Children>(parent)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    actual == expected
}

fn mount_slot_child_order_extended_exactly(
    world: &World,
    mount_slot: Entity,
    prior: &[Entity],
    shell: Entity,
) -> bool {
    let actual = world
        .get::<Children>(mount_slot)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    actual.len() == prior.len() + 1
        && actual[..prior.len()] == *prior
        && actual.last() == Some(&shell)
}

#[cfg(test)]
fn apply_mount_children_manifest_cas_drift_probe(world: &mut World, mount_slot: Entity) {
    if !world
        .get_resource::<MountChildrenManifestCasDriftProbe>()
        .is_some_and(|probe| probe.entity == mount_slot)
    {
        return;
    }
    let Some(actual) = ComponentManifest::from_world(world, mount_slot) else {
        return;
    };
    let Some(claim) = world
        .resource::<RouteEntityRegistry>()
        .record(mount_slot)
        .map(|record| record.claim())
    else {
        return;
    };
    let _ = world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifest(&claim, actual);
}

fn mount_slot_children_are_exact(world: &World, frame: &RealizedFrame) -> bool {
    let slot_entity = frame.mount.slot();
    let registry = world.resource::<RouteEntityRegistry>();
    let Some(mount_record) = registry.record(slot_entity) else {
        return false;
    };
    let mount_owner = mount_record.owner();
    if !frame_version_matches_mount(&frame.mount, mount_owner, frame.frame()) {
        return false;
    }
    let children = world
        .get::<Children>(slot_entity)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    if children.is_empty() || !children.contains(&frame.shell) {
        return false;
    }
    let mut active_shells = 0;
    let mut prepared_shells = 0;
    let mut leaving_shells = 0;
    for child in children {
        if let Some(record) = registry.record(child) {
            let RegistryOwner::Committed(EntityOwner::FrameShell(child_frame)) = record.owner()
            else {
                return false;
            };
            if !frame_version_matches_mount(&frame.mount, mount_owner, child_frame)
                || (child == frame.shell && child_frame != &frame.frame)
                || (child != frame.shell && child_frame == &frame.frame)
                || !record.manifest().matches(world, child)
                || world
                    .get::<RouteOwnershipSeal>(child)
                    .map(|seal| seal.seal())
                    != Some(record.seal())
                || world.get::<FrameLifecycle>(child) != Some(&FrameLifecycle(record.lifecycle()))
                || world.get::<ChildOf>(child).map(ChildOf::parent) != Some(slot_entity)
                || world.get::<Pickable>(child) != Some(&Pickable::IGNORE)
                || !world
                    .get::<RouterInputBarrier>(child)
                    .is_some_and(RouterInputBarrier::denies_all)
            {
                return false;
            }
            match record.lifecycle() {
                EntityLifecycle::Active => {
                    active_shells += 1;
                    if world.get::<Visibility>(child) != Some(&Visibility::Visible) {
                        return false;
                    }
                }
                EntityLifecycle::PreparedInert => {
                    prepared_shells += 1;
                    if world.get::<Visibility>(child) != Some(&Visibility::Hidden) {
                        return false;
                    }
                }
                EntityLifecycle::Leaving => {
                    leaving_shells += 1;
                    if world.get::<Visibility>(child) != Some(&Visibility::Hidden) {
                        return false;
                    }
                }
                EntityLifecycle::Quarantined => return false,
            }
            continue;
        }
        if world
            .resource::<QuarantineLeaseTable>()
            .unresolved_record(child)
            .is_some_and(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(child_frame))
                        if child_frame != &frame.frame
                            && frame_version_matches_mount(&frame.mount, mount_owner, child_frame)
                ) && matches!(
                    record.lifecycle(),
                    EntityLifecycle::PreparedInert | EntityLifecycle::Leaving
                ) && record.manifest().matches(world, child)
                    && world.get::<RouteOwnershipSeal>(child)
                        == Some(&RouteOwnershipSeal::new(record.seal()))
                    && world.get::<FrameLifecycle>(child)
                        == Some(&FrameLifecycle(record.lifecycle()))
                    && world.get::<Visibility>(child) == Some(&Visibility::Hidden)
                    && world.get::<Pickable>(child) == Some(&Pickable::IGNORE)
                    && world
                        .get::<RouterInputBarrier>(child)
                        .is_some_and(RouterInputBarrier::denies_all)
                    && world.get::<ChildOf>(child).map(ChildOf::parent) == Some(slot_entity)
            })
        {
            continue;
        }
        if !world
            .resource::<QuarantineLeaseTable>()
            .record(child)
            .is_some_and(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(child_frame))
                        if frame_version_matches_mount(&frame.mount, mount_owner, child_frame)
                ) && record.lifecycle() == EntityLifecycle::Quarantined
                    && record.manifest().matches(world, child)
                    && world
                        .get::<RouteOwnershipSeal>(child)
                        .map(|seal| seal.seal())
                        == Some(record.seal())
                    && world.get::<FrameLifecycle>(child)
                        == Some(&FrameLifecycle(EntityLifecycle::Quarantined))
                    && world.get::<Visibility>(child) == Some(&Visibility::Hidden)
                    && world.get::<Pickable>(child) == Some(&Pickable::IGNORE)
                    && world
                        .get::<RouterInputBarrier>(child)
                        .is_some_and(RouterInputBarrier::denies_all)
                    && world.get::<ChildOf>(child).map(ChildOf::parent) == Some(slot_entity)
            })
        {
            return false;
        }
    }
    active_shells <= 1
        && prepared_shells <= 1
        && leaving_shells <= 1
        && !(prepared_shells == 1 && leaving_shells == 1)
}

fn frame_version_matches_mount(
    mount: &FrameMountBinding,
    mount_owner: &RegistryOwner,
    candidate: &RouteFrameVersion,
) -> bool {
    match (&mount.kind, mount_owner) {
        (
            FrameMountBindingKind::HostTop,
            RegistryOwner::Committed(EntityOwner::HostTopSlot { host, slot }),
        ) => candidate.slot() == *slot && host_version_matches_frame(host, candidate),
        (
            FrameMountBindingKind::ChildOutlet { parent, outlet, .. },
            RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: owner_parent,
                outlet: owner_outlet,
                slot,
            }),
        ) => {
            owner_parent == parent
                && owner_outlet == outlet
                && candidate.slot() == *slot
                && frame_versions_share_router(candidate, parent)
                && candidate.host() == parent.host()
        }
        _ => false,
    }
}

#[cfg(test)]
fn settle_failed_realization(world: &mut World, top_slot: Entity, frame: &RealizedFrame) {
    if quarantine_failed_realization(world, frame) {
        return;
    }
    let children = world.register_component::<Children>();
    let _ = synchronize_manifest_allowing(world, top_slot, frame.mount.slot_claim(), &[children]);
}

#[cfg(test)]
fn quarantine_failed_realization(world: &mut World, frame: &RealizedFrame) -> bool {
    quarantine_frame(world, frame)
}

fn quarantine_frame(world: &mut World, frame: &RealizedFrame) -> bool {
    let claims_are_current = frame.entities.iter().all(|realized| {
        world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
            .is_some_and(|record| record.claim() == realized.claim)
            && world.get_entity(realized.entity).is_ok()
    });
    if !claims_are_current {
        return false;
    }
    let Some(current_lifecycle) = frame
        .entities
        .first()
        .map(|entity| entity.claim.lifecycle())
    else {
        return false;
    };
    if frame
        .entities
        .iter()
        .any(|entity| entity.claim.lifecycle() != current_lifecycle)
    {
        return false;
    }
    if !repair_missing_pickable_for_quarantine(world, frame, current_lifecycle) {
        return transfer_unresolved_residual(world, frame, current_lifecycle);
    }

    let owned = frame
        .entities
        .iter()
        .map(|realized| realized.entity)
        .collect::<HashSet<_>>();
    let mut foreign_descendants = Vec::new();
    let mut visited = owned.clone();
    let mut stack = frame
        .entities
        .iter()
        .flat_map(|realized| {
            world
                .get::<Children>(realized.entity)
                .into_iter()
                .flat_map(|children| children.iter().copied())
        })
        .collect::<Vec<_>>();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            continue;
        }
        if !owned.contains(&entity) {
            foreign_descendants.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }

    let mut foreign_components = Vec::new();
    let mut missing_components = Vec::new();
    let mut actual_manifests = Vec::with_capacity(frame.entities.len());
    let mut protected_parent = None;
    for realized in &frame.entities {
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
            .expect("current realization claim retains its record");
        let Some(actual) = ComponentManifest::from_world(world, realized.entity) else {
            return false;
        };
        foreign_components.extend(
            actual
                .component_ids()
                .iter()
                .copied()
                .filter(|id| !record.manifest().component_ids().contains(id)),
        );
        missing_components.extend(
            record
                .manifest()
                .component_ids()
                .iter()
                .copied()
                .filter(|id| !actual.component_ids().contains(id)),
        );
        actual_manifests.push((realized.entity, actual));
        let actual_parent = world.get::<ChildOf>(realized.entity).map(ChildOf::parent);
        if actual_parent != Some(realized.parent)
            && actual_parent.is_some_and(|parent| !owned.contains(&parent))
        {
            protected_parent.get_or_insert(actual_parent.unwrap());
        }
    }

    let targets = frame.mutation_targets(current_lifecycle, EntityLifecycle::Quarantined, false);
    let facts = QuarantineFacts::with_component_drift(
        None,
        protected_parent,
        foreign_descendants,
        foreign_components,
        missing_components,
    );
    world.resource_scope(
        |world, mut registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
            let transfer = VerifiedFrameQuarantineTransfer::after_world_validation(
                frame
                    .entities
                    .iter()
                    .map(|realized| realized.claim.clone())
                    .collect(),
                actual_manifests,
                facts,
            );
            world.resource_scope(
                |world, mut table: bevy_ecs::prelude::Mut<QuarantineLeaseTable>| {
                    let Ok(transfer_plan) =
                        prepare_verified_frame_quarantine_transfer(&mut table, &registry, transfer)
                    else {
                        return false;
                    };
                    let Some(presentation_plan) = prepare_presentation_mutation(world, &targets)
                    else {
                        return false;
                    };
                    presentation_plan.apply();
                    apply_prepared_quarantine_transfer(&mut table, &mut registry, transfer_plan);
                    true
                },
            )
        },
    )
}

fn transfer_unresolved_residual(
    world: &mut World,
    frame: &RealizedFrame,
    current_lifecycle: EntityLifecycle,
) -> bool {
    let Some(shell) = frame
        .entities
        .iter()
        .find(|realized| realized.role == RealizedRole::Shell)
    else {
        return false;
    };
    if frame_focus_is_in_subtree(world, frame)
        || world.get::<RouteOwnershipSeal>(shell.entity)
            != Some(&RouteOwnershipSeal::new(shell.claim.seal()))
        || world.get::<Visibility>(shell.entity) != Some(&Visibility::Hidden)
        || world.get::<Pickable>(shell.entity) != Some(&Pickable::IGNORE)
        || !world
            .get::<RouterInputBarrier>(shell.entity)
            .is_some_and(RouterInputBarrier::denies_all)
        || world.get::<FrameLifecycle>(shell.entity) != Some(&FrameLifecycle(current_lifecycle))
    {
        return false;
    }
    let claims_are_current = frame.entities.iter().all(|realized| {
        world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
            .is_some_and(|record| record.claim() == realized.claim)
            && world.get_entity(realized.entity).is_ok()
    });
    if !claims_are_current {
        return false;
    }
    let all_entities_are_inert = frame.entities.iter().all(|realized| {
        world.get::<RouteOwnershipSeal>(realized.entity)
            == Some(&RouteOwnershipSeal::new(realized.claim.seal()))
            && world.get::<Visibility>(realized.entity) == Some(&Visibility::Hidden)
            && matches!(
                world.get::<Pickable>(realized.entity),
                None | Some(&Pickable::IGNORE)
            )
            && world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(RouterInputBarrier::denies_all)
            && world.get::<FrameLifecycle>(realized.entity)
                == Some(&FrameLifecycle(current_lifecycle))
            && world.get::<ChildOf>(realized.entity).map(ChildOf::parent) == Some(realized.parent)
    });
    if !all_entities_are_inert {
        return false;
    }
    let owned = frame
        .entities
        .iter()
        .map(|realized| realized.entity)
        .collect::<HashSet<_>>();
    let mut foreign_descendants = Vec::new();
    let mut visited = owned.clone();
    let mut stack = frame
        .entities
        .iter()
        .flat_map(|realized| {
            world
                .get::<Children>(realized.entity)
                .into_iter()
                .flat_map(|children| children.iter().copied())
        })
        .collect::<Vec<_>>();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            continue;
        }
        if !owned.contains(&entity) {
            foreign_descendants.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }
    let mut actual_manifests = Vec::with_capacity(frame.entities.len());
    let mut foreign_components = Vec::new();
    let mut missing_components = Vec::new();
    let mut protected_parent = None;
    for realized in &frame.entities {
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
            .expect("current residual claim retains its record");
        let Some(actual) = ComponentManifest::from_world(world, realized.entity) else {
            return false;
        };
        foreign_components.extend(
            actual
                .component_ids()
                .iter()
                .copied()
                .filter(|id| !record.manifest().component_ids().contains(id)),
        );
        missing_components.extend(
            record
                .manifest()
                .component_ids()
                .iter()
                .copied()
                .filter(|id| !actual.component_ids().contains(id)),
        );
        actual_manifests.push((realized.entity, actual));
        let actual_parent = world.get::<ChildOf>(realized.entity).map(ChildOf::parent);
        if actual_parent != Some(realized.parent)
            && actual_parent.is_some_and(|parent| !owned.contains(&parent))
        {
            protected_parent.get_or_insert(actual_parent.unwrap());
        }
    }
    let facts = QuarantineFacts::with_component_drift(
        None,
        protected_parent,
        foreign_descendants,
        foreign_components,
        missing_components,
    );
    let claims = frame
        .entities
        .iter()
        .map(|realized| realized.claim.clone())
        .collect();
    world.resource_scope(
        |world, mut registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |_world, mut table: bevy_ecs::prelude::Mut<QuarantineLeaseTable>| {
                    let Ok(prepared) = prepare_unresolved_residual_transfer(
                        &mut table,
                        &registry,
                        claims,
                        actual_manifests,
                        facts,
                    ) else {
                        return false;
                    };
                    apply_prepared_unresolved_residual_transfer(
                        &mut table,
                        &mut registry,
                        prepared,
                    );
                    true
                },
            )
        },
    )
}

fn frame_focus_is_in_subtree(world: &World, frame: &RealizedFrame) -> bool {
    world
        .get_resource::<InputFocus>()
        .and_then(InputFocus::get)
        .is_some_and(|focused| entity_is_in_frame_subtree(world, frame, focused))
        || world
            .get_resource::<RouteFocusState>()
            .and_then(RouteFocusState::focused)
            .is_some_and(|focused| entity_is_in_frame_subtree(world, frame, focused))
}

fn entity_is_in_frame_subtree(world: &World, frame: &RealizedFrame, focused: Entity) -> bool {
    let mut current = focused;
    let mut visited = HashSet::new();
    let mut child_below_mount = None;
    loop {
        if frame
            .entities
            .iter()
            .any(|realized| realized.entity == current)
        {
            return true;
        }
        if current == frame.mount.slot() {
            return child_below_mount
                .is_none_or(|child| !frame.prior_mount_children.contains(&child));
        }
        if !visited.insert(current) {
            return false;
        }
        let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) else {
            return false;
        };
        if parent == frame.mount.slot() {
            child_below_mount = Some(current);
        }
        current = parent;
    }
}

fn clear_frame_focus(world: &mut World, frame: &RealizedFrame) {
    let input = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    if input.is_some_and(|focused| entity_is_in_frame_subtree(world, frame, focused)) {
        world.resource_mut::<InputFocus>().clear();
    }
    let route = world
        .get_resource::<RouteFocusState>()
        .and_then(RouteFocusState::focused);
    if route.is_some_and(|focused| entity_is_in_frame_subtree(world, frame, focused)) {
        world.resource_mut::<RouteFocusState>().set_focused(None);
    }
}

fn repair_missing_pickable_for_quarantine(
    world: &mut World,
    frame: &RealizedFrame,
    current_lifecycle: EntityLifecycle,
) -> bool {
    if !matches!(
        current_lifecycle,
        EntityLifecycle::PreparedInert | EntityLifecycle::Leaving
    ) {
        return false;
    }
    let missing = frame
        .entities
        .iter()
        .filter_map(|realized| {
            world
                .get::<Pickable>(realized.entity)
                .is_none()
                .then_some(realized.entity)
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return frame.entities.iter().all(|realized| {
            world.get::<RouteOwnershipSeal>(realized.entity)
                == Some(&RouteOwnershipSeal::new(realized.claim.seal()))
        });
    }
    let repair_is_safe = missing.iter().all(|entity| {
        let realized = frame
            .entities
            .iter()
            .find(|realized| realized.entity == *entity)
            .expect("missing entity came from the realized frame");
        world
            .resource::<RouteEntityRegistry>()
            .record(*entity)
            .is_some_and(|record| record.claim() == realized.claim)
            && world.get::<RouteOwnershipSeal>(*entity)
                == Some(&RouteOwnershipSeal::new(realized.claim.seal()))
            && world.get::<Visibility>(*entity) == Some(&Visibility::Hidden)
            && world
                .get::<RouterInputBarrier>(*entity)
                .is_some_and(RouterInputBarrier::denies_all)
            && world.get::<FrameLifecycle>(*entity) == Some(&FrameLifecycle(current_lifecycle))
    });
    if !repair_is_safe {
        return false;
    }
    for entity in missing {
        world.entity_mut(entity).insert(Pickable::IGNORE);
    }
    world.flush();
    frame.entities.iter().all(|realized| {
        world
            .resource::<RouteEntityRegistry>()
            .record(realized.entity)
            .is_some_and(|record| record.claim() == realized.claim)
            && world.get::<RouteOwnershipSeal>(realized.entity)
                == Some(&RouteOwnershipSeal::new(realized.claim.seal()))
            && world.get::<Visibility>(realized.entity) == Some(&Visibility::Hidden)
            && world.get::<Pickable>(realized.entity) == Some(&Pickable::IGNORE)
            && world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(RouterInputBarrier::denies_all)
            && world.get::<FrameLifecycle>(realized.entity)
                == Some(&FrameLifecycle(current_lifecycle))
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        app_ext::RouterHandle,
        content::{ContentColor, ContentPlan, FrameContentWriter, SolidRect},
        host::{HostCandidateClassification, HostCandidateRevisionState, classify_host_candidate},
        registry::QuarantineLeaseTable,
    };
    use bevy_ecs::{component::Component, lifecycle::HookContext, prelude::Resource};
    use bevy_ui_router_core::{
        integration::{EngineAction, RootFrameTask, RouterEngine},
        prelude::{RouteNodeOptions, RoutePattern, RouteSlotKey, RouteTree, RouteViewKey},
    };

    #[derive(Resource, Default)]
    struct RealizationStageAudit(Vec<bool>);

    #[derive(Resource)]
    struct PersistentPickableRemoval;

    #[derive(Component)]
    struct ForeignRealizationComponent;

    fn pending_frame_fixture() -> (
        World,
        Entity,
        ValidatedHostCandidate,
        Entity,
        ExactRegistryClaim,
        RootFrameTask,
    ) {
        let mut world = World::new();
        world.register_component_hooks::<Pickable>().on_insert(
            |mut world, HookContext { entity, .. }| {
                if world.get_resource::<PersistentPickableRemoval>().is_some()
                    && world.get::<FrameOutput>(entity).is_some()
                {
                    world.commands().entity(entity).remove::<Pickable>();
                }
            },
        );
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let router = RouterHandle::fixture(17, 19);
        let host = world
            .spawn((
                Node::default(),
                crate::app_ext::RouterHostRegistration::new(router),
            ))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(&world, router, &mut revisions)
        else {
            panic!("host should classify")
        };
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree, fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
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
        engine
            .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
            .unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("root task expected")
        };
        (world, host, candidate, top_slot, top_slot_claim, task)
    }

    fn realized_empty_fixture() -> (World, Entity, Entity, RealizedFrame) {
        let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        let frame = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap();
        (world, host, top_slot, frame)
    }

    #[test]
    fn equal_raw_cross_router_frame_cannot_reparent_into_child_mount() {
        let (_first_world, _, _, first_slot, first_claim, first_task) = pending_frame_fixture();
        let (_second_world, _, _, _, _, second_task) = pending_frame_fixture();
        let parent = first_task.frame().clone();
        let candidate = second_task.frame().clone();
        assert_eq!(parent.router(), candidate.router());
        assert_eq!(parent.host(), candidate.host());
        assert_eq!(parent.slot(), candidate.slot());

        let outlet = RouteSlotKey::default();
        let mount = FrameMountBinding {
            slot: first_slot,
            slot_claim: first_claim.clone(),
            kind: FrameMountBindingKind::ChildOutlet {
                parent: parent.clone(),
                outlet: outlet.clone(),
                anchor: first_slot,
                anchor_claim: first_claim,
                expected_anchor_children: Vec::new(),
            },
        };
        let owner = RegistryOwner::Committed(EntityOwner::ChildSlot {
            parent,
            outlet,
            slot: candidate.slot(),
        });

        assert!(!frame_version_matches_mount(&mount, &owner, &candidate));
    }

    #[test]
    fn realization_is_fully_inert_before_any_relationship_is_written() {
        let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.insert_resource(RealizationStageAudit::default());
        world.register_component_hooks::<FrameLifecycle>().on_add(
            |mut world, HookContext { entity, .. }| {
                let valid = world
                    .resource::<RouteEntityRegistry>()
                    .record(entity)
                    .is_some()
                    && world.get::<Node>(entity).is_some()
                    && world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
                    && world.get::<Pickable>(entity) == Some(&Pickable::IGNORE)
                    && world
                        .get::<RouterInputBarrier>(entity)
                        .is_some_and(RouterInputBarrier::denies_all)
                    && world.get::<RouteOwnershipSeal>(entity).is_some()
                    && world.get::<ChildOf>(entity).is_none();
                world.resource_mut::<RealizationStageAudit>().0.push(valid);
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap();

        assert_eq!(
            world.resource::<RealizationStageAudit>().0,
            vec![true, true, true]
        );
    }

    #[test]
    fn stale_top_slot_claim_cannot_rewrite_the_new_manifest_incarnation() {
        let (mut world, _host, candidate, top_slot, stale_claim, task) = pending_frame_fixture();
        world
            .entity_mut(top_slot)
            .insert(Inherited(ComputedUiTargetCamera::default()));
        let (owner, manifest) = {
            let record = world
                .resource::<RouteEntityRegistry>()
                .record(top_slot)
                .unwrap();
            (record.owner().clone(), record.manifest().clone())
        };
        let new_claim = {
            let mut registry = world.resource_mut::<RouteEntityRegistry>();
            assert!(registry.remove_exact(&stale_claim));
            let seal = registry.issue_seal();
            registry
                .register(
                    top_slot,
                    owner,
                    seal,
                    manifest.clone(),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
            registry.record(top_slot).unwrap().claim()
        };
        world
            .entity_mut(top_slot)
            .insert(RouteOwnershipSeal::new(new_claim.seal()));
        let entity_count = world.iter_entities().count();

        let failure = realize_frame_deferred(
            &mut world,
            candidate,
            top_slot,
            stale_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap_err();
        let (error, residual) = failure.into_parts();

        assert_eq!(error, RealizationError::InvalidTopSlot);
        assert!(residual.is_none());
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap();
        assert_eq!(record.claim(), new_claim);
        assert_eq!(record.manifest(), &manifest);
        assert_eq!(world.iter_entities().count(), entity_count);
    }

    #[test]
    fn mount_slot_children_manifest_cas_rejects_registry_drift_after_relationship_write() {
        let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.insert_resource(MountChildrenManifestCasDriftProbe { entity: top_slot });

        let failure = realize_frame_deferred(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .expect_err("post-relationship registry drift must defeat the frozen manifest CAS");
        let (error, residual) = failure.into_parts();

        assert_eq!(error, RealizationError::InvalidMount);
        let residual = residual.expect("written frame entities remain a sealed cleanup residual");
        assert_eq!(
            world.get::<ChildOf>(residual.shell()).map(ChildOf::parent),
            Some(top_slot)
        );
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(top_slot)
                .unwrap()
                .manifest()
                .matches(&world, top_slot)
        );
    }

    #[test]
    fn post_write_foreign_component_is_quarantined_without_despawn() {
        let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.register_component_hooks::<FrameOutput>().on_add(
            |mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .insert(ForeignRealizationComponent);
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        let error = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap_err();

        assert_eq!(error, RealizationError::PostWriteDrift);
        let foreign = world
            .query_filtered::<Entity, bevy_ecs::query::With<ForeignRealizationComponent>>()
            .single(&world)
            .unwrap();
        assert!(world.get_entity(foreign).is_ok());
        let lease = world
            .resource::<QuarantineLeaseTable>()
            .leases()
            .last()
            .expect("post-write drift must create one quarantine lease");
        assert!(lease.owned_entities().contains(&foreign));
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    pub(crate) fn post_write_required_component_removal_is_rejected_by_the_frozen_manifest() {
        let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.register_component_hooks::<FrameOutput>().on_add(
            |mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).remove::<Pickable>();
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        let error = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap_err();

        assert_eq!(error, RealizationError::PostWriteDrift);
        let lease = world
            .resource::<QuarantineLeaseTable>()
            .leases()
            .last()
            .expect("missing Pickable must be repaired before ordinary quarantine");
        for entity in lease.owned_entities() {
            assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::IGNORE));
            assert!(
                world
                    .resource::<QuarantineLeaseTable>()
                    .record(entity)
                    .unwrap()
                    .manifest()
                    .matches(&world, entity)
            );
        }
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    fn persistent_pickable_removal_never_enters_ordinary_quarantine() {
        let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.insert_resource(PersistentPickableRemoval);
        world.register_component_hooks::<FrameOutput>().on_add(
            |mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).remove::<Pickable>();
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        let error = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap_err();

        assert_eq!(error, RealizationError::PostWriteDrift);
        let table = world.resource::<QuarantineLeaseTable>();
        assert!(table.leases().is_empty());
        assert_eq!(table.unresolved_residuals().len(), 1);
        assert_eq!(table.unresolved_residuals()[0].owned_entities().len(), 3);
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .all(|record| matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
                ))
        );
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    fn pending_frame_focus_covers_exact_mount_slot_and_its_descendants() {
        let (mut world, _host, top_slot, frame) = realized_empty_fixture();
        world.insert_resource(InputFocus::from_entity(top_slot));
        let mut route_focus = RouteFocusState::default();
        route_focus.set_focused(Some(top_slot));
        world.insert_resource(route_focus);

        assert!(frame_focus_is_in_subtree(&world, &frame));
        clear_frame_focus(&mut world, &frame);
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);

        let descendant = world.spawn(ChildOf(top_slot)).id();
        *world.resource_mut::<InputFocus>() = InputFocus::from_entity(descendant);
        world
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(descendant));
        assert!(frame_focus_is_in_subtree(&world, &frame));
        clear_frame_focus(&mut world, &frame);
        assert_eq!(world.resource::<InputFocus>().get(), None);
        assert_eq!(world.resource::<RouteFocusState>().focused(), None);
    }

    #[test]
    fn pending_frame_focus_excludes_a_preexisting_mount_child_subtree() {
        let (mut world, _host, top_slot, mut frame) = realized_empty_fixture();
        let preexisting = world.spawn(ChildOf(top_slot)).id();
        let focused = world.spawn(ChildOf(preexisting)).id();
        frame.prior_mount_children.push(preexisting);
        world.insert_resource(InputFocus::from_entity(focused));
        let mut route_focus = RouteFocusState::default();
        route_focus.set_focused(Some(focused));
        world.insert_resource(route_focus);

        assert!(!frame_focus_is_in_subtree(&world, &frame));
        clear_frame_focus(&mut world, &frame);
        assert_eq!(world.resource::<InputFocus>().get(), Some(focused));
        assert_eq!(world.resource::<RouteFocusState>().focused(), Some(focused));
    }

    #[test]
    fn pending_realization_releases_focus_assigned_by_an_observer() {
        let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.insert_resource(bevy_input_focus::InputFocus::default());
        world.insert_resource(crate::input::RouteFocusState::default());
        world.register_component_hooks::<FrameOutput>().on_add(
            |mut world, HookContext { entity, .. }| {
                *world.resource_mut::<bevy_input_focus::InputFocus>() =
                    bevy_input_focus::InputFocus::from_entity(entity);
                world
                    .resource_mut::<crate::input::RouteFocusState>()
                    .set_focused(Some(entity));
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        let frame = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap();

        assert_eq!(world.resource::<bevy_input_focus::InputFocus>().get(), None);
        assert_eq!(
            world.resource::<crate::input::RouteFocusState>().focused(),
            None
        );
        assert!(validate_realized_frame(&world, &frame));
    }

    #[test]
    fn unresolved_transfer_rejects_a_visible_descendant() {
        let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.insert_resource(PersistentPickableRemoval);
        world.register_component_hooks::<FrameOutput>().on_add(
            |mut world, HookContext { entity, .. }| {
                world
                    .commands()
                    .entity(entity)
                    .remove::<Pickable>()
                    .insert(Visibility::Visible);
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        let error = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap_err();

        assert_eq!(error, RealizationError::PostWriteDrift);
        assert!(
            world
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals()
                .is_empty()
        );
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .any(|record| matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameOutput { .. })
                ))
        );
    }

    #[test]
    fn post_write_authored_value_drift_is_rejected() {
        let (mut world, host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        world.register_component_hooks::<FrameOutput>().on_add(
            |mut world, HookContext { entity, .. }| {
                world.commands().entity(entity).insert(Node {
                    width: Val::Px(7.0),
                    height: Val::Px(9.0),
                    ..Node::default()
                });
            },
        );
        let mut content = ContentPlan::new();
        FrameContentWriter::new(&mut content).spawn_empty().unwrap();

        let error = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap_err();

        assert_eq!(error, RealizationError::PostWriteDrift);
        assert_eq!(world.resource::<QuarantineLeaseTable>().leases().len(), 1);
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    pub(crate) fn realization_is_stable_registered_and_inert() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let router = RouterHandle::fixture(3, 9);
        let host = world
            .spawn((
                Node::default(),
                crate::app_ext::RouterHostRegistration::new(router),
            ))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(&world, router, &mut revisions)
        else {
            panic!("host should classify")
        };
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree, fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
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
        let request = engine
            .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
            .unwrap();
        engine.collect();
        let bevy_ui_router_core::integration::EngineAction::FrameTask(task) =
            engine.action().unwrap()
        else {
            panic!("root task expected")
        };
        let mut content = ContentPlan::new();
        let mut writer = FrameContentWriter::new(&mut content);
        let root = writer.spawn_empty().unwrap();
        let rect = writer
            .spawn_solid_rect(SolidRect::new(20, 10, ContentColor::srgba8(1, 2, 3, 255)).unwrap())
            .unwrap();
        writer.add_child(root, rect).unwrap();

        let frame = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap();

        assert_eq!(request, task.request());
        assert_eq!(frame.obligation_count(), 4);
        assert!(validate_realized_frame(&world, &frame));
        for realized in &frame.entities {
            let mut explicit = vec![
                world.register_component::<Node>(),
                world.register_component::<Visibility>(),
                world.register_component::<Pickable>(),
                world.register_component::<RouterInputBarrier>(),
                world.register_component::<RouteOwnershipSeal>(),
                world.register_component::<FrameLifecycle>(),
                world.register_component::<ChildOf>(),
            ];
            explicit.push(match realized.role {
                RealizedRole::Shell => world.register_component::<FrameShell>(),
                RealizedRole::ContentTarget => world.register_component::<FrameContentTarget>(),
                RealizedRole::Empty | RealizedRole::SolidRect => {
                    world.register_component::<FrameOutput>()
                }
            });
            if realized.role == RealizedRole::SolidRect {
                explicit.push(world.register_component::<BackgroundColor>());
                explicit.push(world.register_component::<PickingInteraction>());
            }
            if frame
                .entities
                .iter()
                .any(|child| child.parent == realized.entity)
            {
                explicit.push(world.register_component::<Children>());
            }
            assert_manifest_is_exact(&world, realized.entity, explicit);
        }
        assert_eq!(
            world.get::<ChildOf>(frame.shell()).map(ChildOf::parent),
            Some(top_slot)
        );
        assert_eq!(
            world
                .get::<Children>(top_slot)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default(),
            vec![frame.shell()]
        );
        let expected_container = Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        };
        for realized in &frame.entities {
            assert_eq!(
                world.get::<ChildOf>(realized.entity).map(ChildOf::parent),
                Some(realized.parent)
            );
            assert!(exact_children_match(
                &world,
                realized.entity,
                expected_children(&frame, realized.entity)
            ));
            if matches!(
                realized.role,
                RealizedRole::Shell | RealizedRole::ContentTarget | RealizedRole::Empty
            ) {
                assert_eq!(
                    world.get::<Node>(realized.entity),
                    Some(&expected_container)
                );
                assert_eq!(
                    world.get::<BackgroundColor>(realized.entity),
                    Some(&BackgroundColor::default())
                );
                assert_eq!(
                    world.get::<Pickable>(realized.entity),
                    Some(&Pickable::IGNORE)
                );
                assert!(
                    world
                        .get::<RouterInputBarrier>(realized.entity)
                        .is_some_and(RouterInputBarrier::denies_all)
                );
            }
        }
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    fn realization_registration_failure_settles_every_prior_allocation() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let blocker = world.spawn_empty().id();
        let router = RouterHandle::fixture(5, 11);
        let host = world
            .spawn((
                Node::default(),
                crate::app_ext::RouterHostRegistration::new(router),
            ))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(&world, router, &mut revisions)
        else {
            panic!("host should classify")
        };
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree, fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
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
        engine
            .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
            .unwrap();
        engine.collect();
        let bevy_ui_router_core::integration::EngineAction::FrameTask(task) =
            engine.action().unwrap()
        else {
            panic!("root task expected")
        };
        {
            let mut registry = world.resource_mut::<RouteEntityRegistry>();
            let seal = registry.issue_seal();
            registry
                .register(
                    blocker,
                    RegistryOwner::Committed(EntityOwner::FrameContentTarget(task.frame().clone())),
                    seal,
                    ComponentManifest::new(Vec::new()),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap();
        }
        let entity_count_before = world.iter_entities().count();
        let registry_count_before = world.resource::<RouteEntityRegistry>().records().len();

        let failure = realize_frame_deferred(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap_err();
        let (error, residual) = failure.into_parts();

        assert_eq!(error, RealizationError::RegistryRejected);
        let residual = residual.expect("prior registered allocations remain a cleanup residual");
        assert_eq!(residual.obligation_count(), 1);
        assert_eq!(world.iter_entities().count(), entity_count_before + 1);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records().len(),
            registry_count_before + 1
        );

        let mut ledger = crate::cleanup::CleanupLedger::for_failed_realization(&world, residual)
            .expect("partial realization has an exact foreground ledger");
        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));
        assert_eq!(ledger.receipt_count(), 1);
        assert_eq!(world.iter_entities().count(), entity_count_before);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records().len(),
            registry_count_before,
            "only the independently registered blocker may remain"
        );
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .all(|record| !matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(_))
                ))
        );
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    fn failed_settlement_returns_the_complete_frame_for_a_fresh_attempt() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let router = RouterHandle::fixture(7, 13);
        let host = world
            .spawn((
                Node::default(),
                crate::app_ext::RouterHostRegistration::new(router),
            ))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(&world, router, &mut revisions)
        else {
            panic!("host should classify")
        };
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut engine = RouterEngine::install(tree, fallback).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
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
        engine
            .push(bevy_ui_router_core::prelude::RouteLocation::new("/"))
            .unwrap();
        engine.collect();
        let bevy_ui_router_core::integration::EngineAction::FrameTask(task) =
            engine.action().unwrap()
        else {
            panic!("root task expected")
        };
        let frame = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap();
        let count = frame.obligation_count();
        let first_claim = frame.claims().next().unwrap().clone();
        world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_lifecycle(&first_claim, EntityLifecycle::Quarantined)
            .unwrap();

        let retained = settle_frame(&mut world, frame).unwrap_err();

        assert_eq!(retained.obligation_count(), count);
        assert!(
            retained
                .claims()
                .all(|claim| world.get_entity(claim.entity()).is_ok())
        );
        assert!(world.get_entity(host).is_ok());
    }

    #[test]
    fn task9_settlement_transfers_exact_owned_set_without_physical_despawn() {
        let (mut world, host, _top_slot, frame) = realized_empty_fixture();
        let owned = frame
            .entities
            .iter()
            .map(|realized| realized.entity)
            .collect::<Vec<_>>();
        let leases_before = world.resource::<QuarantineLeaseTable>().leases().len();

        settle_frame(&mut world, frame).unwrap();

        assert!(world.get_entity(host).is_ok());
        assert!(owned.iter().all(|entity| world.get_entity(*entity).is_ok()));
        assert!(owned.iter().all(|entity| {
            world.get::<Visibility>(*entity) == Some(&Visibility::Hidden)
                && world.get::<Pickable>(*entity) == Some(&Pickable::IGNORE)
                && world
                    .get::<RouterInputBarrier>(*entity)
                    .is_some_and(RouterInputBarrier::denies_all)
                && world
                    .resource::<RouteEntityRegistry>()
                    .record(*entity)
                    .is_none()
        }));
        let leases = world.resource::<QuarantineLeaseTable>().leases();
        assert_eq!(leases.len(), leases_before + 1);
        assert_eq!(leases.last().unwrap().owned_entities().len(), owned.len());
    }

    #[test]
    fn foreign_child_transfers_the_owned_frame_to_quarantine_and_is_never_despawned() {
        let (mut world, host, _top_slot, frame) = realized_empty_fixture();
        let foreign = world.spawn_empty().id();
        world.entity_mut(frame.shell()).add_child(foreign);
        let leases_before = world.resource::<QuarantineLeaseTable>().leases().len();

        assert!(!validate_realized_frame(&world, &frame));
        settle_frame(&mut world, frame).unwrap();

        assert!(world.get_entity(foreign).is_ok());
        assert!(world.get_entity(host).is_ok());
        let leases = world.resource::<QuarantineLeaseTable>().leases();
        assert_eq!(leases.len(), leases_before + 1);
        assert_eq!(leases.last().unwrap().foreign_descendants(), &[foreign]);
    }

    #[test]
    fn failed_realization_with_foreign_descendant_transfers_owned_residual_to_quarantine() {
        let (mut world, host, top_slot, frame) = realized_empty_fixture();
        let owned = frame
            .entities
            .iter()
            .map(|realized| realized.entity)
            .collect::<Vec<_>>();
        let foreign = world.spawn_empty().id();
        world.entity_mut(frame.shell()).add_child(foreign);
        let leases_before = world.resource::<QuarantineLeaseTable>().leases().len();

        settle_failed_realization(&mut world, top_slot, &frame);

        assert!(world.get_entity(host).is_ok());
        assert!(world.get_entity(foreign).is_ok());
        assert!(owned.iter().all(|entity| world.get_entity(*entity).is_ok()));
        assert!(owned.iter().all(|entity| {
            world.get::<Visibility>(*entity) == Some(&Visibility::Hidden)
                && world.get::<Pickable>(*entity) == Some(&Pickable::IGNORE)
                && world
                    .get::<RouterInputBarrier>(*entity)
                    .is_some_and(RouterInputBarrier::denies_all)
        }));
        let leases = world.resource::<QuarantineLeaseTable>().leases();
        assert_eq!(leases.len(), leases_before + 1);
        assert_eq!(leases.last().unwrap().owned_entities().len(), owned.len());
        assert_eq!(leases.last().unwrap().foreign_descendants(), &[foreign]);
        assert!(owned.iter().all(|entity| {
            world
                .resource::<RouteEntityRegistry>()
                .record(*entity)
                .is_none()
        }));
    }

    #[test]
    fn reparented_top_slot_invalidates_host_authority_without_repairing_app_topology() {
        let (mut world, host, top_slot, frame) = realized_empty_fixture();
        let foreign_parent = world.spawn((Node::default(),)).id();
        world.entity_mut(top_slot).insert(ChildOf(foreign_parent));

        assert!(!validate_realized_frame(&world, &frame));
        assert_eq!(
            world.get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(foreign_parent)
        );
        assert!(world.get_entity(host).is_ok());
    }
}
