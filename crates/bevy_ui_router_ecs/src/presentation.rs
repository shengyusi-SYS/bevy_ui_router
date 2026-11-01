use std::collections::HashSet;

#[cfg(test)]
use bevy_app::Inherited;
use bevy_camera::visibility::Visibility;
use bevy_color::Color;
use bevy_ecs::{
    change_detection::Mut,
    component::{Component, ComponentId},
    entity::EntityHashMap,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_input_focus::InputFocus;
use bevy_picking::{
    Pickable,
    events::PointerState,
    hover::{HoverMap, PickingInteraction, PreviousHoverMap},
};
#[cfg(feature = "bsn")]
use bevy_ui::ComputedNode;
use bevy_ui::{BackgroundColor, Node, Val};
#[cfg(test)]
use bevy_ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera};
use bevy_ui_router_core::{
    frame::RouteFrameVersion,
    integration::{
        FrameMountTarget, FrameTask, RetainedLeafProjection, frame_versions_share_router,
        host_version_matches_frame,
    },
};

use crate::{
    components::{ChildSlotAuthority, ChildSlotLifecycle, RouteOwnershipSeal, RouterInputBarrier},
    content::{AcceptedContentPlan, RealizedContentKind},
    host::{
        ValidatedHostCandidate, ValidatedHostCandidateObservation,
        observe_validated_host_candidate, validated_host_candidate_is_current,
    },
    input::{
        PointerStateResources, PreparedPointerMutation, RouteFocusState, pointer_resource_entity,
        query_including_disabled,
    },
    mount::MountedChildSlot,
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim, QuarantineFacts,
        QuarantineLeaseTable, RegistryOwner, RouteEntityRegistry,
    },
};

#[cfg(feature = "bsn")]
pub(crate) mod native_failure;
#[cfg(feature = "bsn")]
pub(crate) mod native_frame;
#[cfg(feature = "bsn")]
pub(crate) mod native_scene;

#[cfg(test)]
use crate::registry::{
    apply_prepared_quarantine_transfer, apply_prepared_unresolved_residual_transfer,
    prepare_unresolved_residual_transfer, prepare_verified_frame_quarantine_transfer,
};

#[cfg(test)]
use crate::components::{
    ChildSlotAuthorityProbe, ChildSlotBarrierProbe, ChildSlotNodeProbe, ChildSlotRelationshipProbe,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutletBinding {
    RegisteredOutput(ExactRegistryClaim),
    #[cfg(feature = "bsn")]
    NativeContent {
        anchor: Entity,
        root: ExactRegistryClaim,
    },
}

impl OutletBinding {
    pub(crate) fn anchor(&self) -> Entity {
        match self {
            Self::RegisteredOutput(claim) => claim.entity(),
            #[cfg(feature = "bsn")]
            Self::NativeContent { anchor, .. } => *anchor,
        }
    }

    pub(crate) const fn source_claim(&self) -> &ExactRegistryClaim {
        match self {
            Self::RegisteredOutput(claim) => claim,
            #[cfg(feature = "bsn")]
            Self::NativeContent { root, .. } => root,
        }
    }

    pub(crate) fn source_claim_mut(&mut self) -> &mut ExactRegistryClaim {
        match self {
            Self::RegisteredOutput(claim) => claim,
            #[cfg(feature = "bsn")]
            Self::NativeContent { root, .. } => root,
        }
    }

    pub(crate) fn is_native(&self) -> bool {
        match self {
            Self::RegisteredOutput(_) => false,
            #[cfg(feature = "bsn")]
            Self::NativeContent { .. } => true,
        }
    }

    pub(crate) fn opaque_digest_part(&self) -> u64 {
        self.source_claim().opaque_digest_part()
            ^ self.anchor().to_bits().rotate_left(17)
            ^ if self.is_native() {
                0x4e4154495645414e
            } else {
                0x4f5554505554414e
            }
    }

    /// The binding names a physical anchor and an independently exact authority source.
    /// Native authored components and siblings remain open between operations.
    pub(crate) fn is_current(
        &self,
        world: &World,
        parent: &RouteFrameVersion,
        require_outlet: bool,
    ) -> bool {
        self.is_current_with_manifest_policy(world, parent, require_outlet, false)
    }

    pub(crate) fn is_current_with_manifest_policy(
        &self,
        world: &World,
        parent: &RouteFrameVersion,
        _require_outlet: bool,
        allow_additions: bool,
    ) -> bool {
        match self {
            Self::RegisteredOutput(claim) => world.get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(claim.entity()))
                .is_some_and(|record| record.claim() == *claim
                    && matches!(record.owner(), RegistryOwner::Committed(EntityOwner::FrameOutput { frame, .. }) if frame == parent)
                    && (if allow_additions { manifest_contains_expected(world, claim.entity(), record.manifest()) } else { record.manifest().matches(world, claim.entity()) })
                    && world.get::<RouteOwnershipSeal>(claim.entity()) == Some(&RouteOwnershipSeal::new(claim.seal()))),
            #[cfg(feature = "bsn")]
            Self::NativeContent { anchor, root } => world.get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(root.entity()))
                .is_some_and(|record| record.claim() == *root)
                && native_frame::collect_content(world, parent, root.entity()).is_ok_and(|content| content.contains(anchor)
                    && (!_require_outlet || content.iter().filter(|entity| world.get::<bevy_ui_router_bsn::DefaultOutlet>(**entity).is_some()).count() == 1))
                && (!_require_outlet || world.get::<Node>(*anchor).is_some()
                    && world.get::<bevy_ui_router_bsn::DefaultOutlet>(*anchor).is_some()),
        }
    }

    pub(crate) fn physical_claim(&self) -> Option<&ExactRegistryClaim> {
        (self.anchor() == self.source_claim().entity()).then_some(self.source_claim())
    }

    pub(crate) fn children_are_current(
        &self,
        world: &World,
        slot: Entity,
        legacy_expected: &[Entity],
    ) -> bool {
        let children = world
            .get::<Children>(self.anchor())
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        if self.is_native() {
            children.iter().filter(|entity| **entity == slot).count() == 1
        } else {
            children == legacy_expected
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameMountBinding {
    slot: Entity,
    slot_claim: ExactRegistryClaim,
    kind: FrameMountBindingKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FrameMountBindingKind {
    HostTop {
        host: Entity,
    },
    ChildOutlet {
        parent: RouteFrameVersion,
        outlet: bevy_ui_router_core::slot::RouteSlotKey,
        anchor: Entity,
        anchor_binding: OutletBinding,
        expected_anchor_children: Vec<Entity>,
        staging_lifecycle: EntityLifecycle,
        allowed_existing_shells: Vec<ExactRegistryClaim>,
        retained_parent: Option<RetainedLeafProjection>,
    },
}

impl FrameMountBinding {
    pub(crate) fn matches_retained_scope(&self, projection: &RetainedLeafProjection) -> bool {
        self.slot_claim.owner()
            == &RegistryOwner::Committed(EntityOwner::ChildSlot {
                parent: projection.parent().clone(),
                outlet: projection.outlet().clone(),
                slot: projection.slot(),
            })
            && matches!(&self.kind, FrameMountBindingKind::ChildOutlet { parent, outlet, .. }
            if parent == projection.parent() && outlet == projection.outlet())
            && projection.frame().slot() == projection.slot()
            && host_version_matches_frame(projection.host(), projection.frame())
            && frame_versions_share_router(projection.parent(), projection.frame())
    }
    fn host_top(slot: Entity, slot_claim: ExactRegistryClaim, host: Entity) -> Self {
        Self {
            slot,
            slot_claim,
            kind: FrameMountBindingKind::HostTop { host },
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
                anchor_binding: mounted.anchor_binding().clone(),
                expected_anchor_children: mounted.expected_anchor_children().to_vec(),
                staging_lifecycle: EntityLifecycle::PreparedInert,
                allowed_existing_shells: Vec::new(),
                retained_parent: None,
            },
        }
    }

    fn from_reconciliation_staging_mount(
        mount: &crate::branch_transaction::SealedReconciliationStagingMount<'_>,
    ) -> Self {
        let mounted = mount.slot();
        Self {
            slot: mounted.slot(),
            slot_claim: mounted.slot_claim().clone(),
            kind: FrameMountBindingKind::ChildOutlet {
                parent: mounted.parent().clone(),
                outlet: mounted.outlet().clone(),
                anchor: mounted.anchor(),
                anchor_binding: mounted.anchor_binding().clone(),
                expected_anchor_children: mounted.expected_anchor_children().to_vec(),
                staging_lifecycle: mount.expected_slot_lifecycle(),
                allowed_existing_shells: mount.allowed_existing_shells().to_vec(),
                retained_parent: mount.retained_parent().cloned(),
            },
        }
    }

    pub(crate) const fn slot(&self) -> Entity {
        self.slot
    }

    pub(crate) const fn slot_claim(&self) -> &ExactRegistryClaim {
        &self.slot_claim
    }

    pub(crate) const fn physical_parent(&self) -> (Entity, Option<&ExactRegistryClaim>) {
        match &self.kind {
            FrameMountBindingKind::HostTop { host } => (*host, None),
            FrameMountBindingKind::ChildOutlet {
                anchor,
                anchor_binding,
                ..
            } => (*anchor, Some(anchor_binding.source_claim())),
        }
    }

    pub(crate) fn outlet_binding(&self) -> Option<&OutletBinding> {
        match &self.kind {
            FrameMountBindingKind::HostTop { .. } => None,
            FrameMountBindingKind::ChildOutlet { anchor_binding, .. } => Some(anchor_binding),
        }
    }

    #[cfg(test)]
    fn replace_host_top_host_for_test(&mut self, replacement: Entity) -> bool {
        let FrameMountBindingKind::HostTop { host } = &mut self.kind else {
            return false;
        };
        *host = replacement;
        true
    }

    pub(crate) fn matches_mounted_child_slot(&self, mounted: &MountedChildSlot) -> bool {
        self.slot == mounted.slot()
            && self.slot_claim == *mounted.slot_claim()
            && matches!(
                &self.kind,
                FrameMountBindingKind::ChildOutlet {
                    parent,
                    outlet,
                    anchor,
                    anchor_binding,
                    expected_anchor_children,
                    ..
                } if parent == mounted.parent()
                    && outlet == mounted.outlet()
                    && *anchor == mounted.anchor()
                    && anchor_binding == mounted.anchor_binding()
                    && (anchor_binding.is_native() || expected_anchor_children == mounted.expected_anchor_children())
            )
    }

    /// Stock parent roles may change independently while this original child
    /// remains inactive. Activation separately proves the current exact roles.
    pub(crate) fn matches_mounted_child_slot_identity(&self, mounted: &MountedChildSlot) -> bool {
        self.slot == mounted.slot()
            && self.slot_claim.owner() == mounted.slot_claim().owner()
            && self.slot_claim.seal() == mounted.slot_claim().seal()
            && matches!(&self.kind, FrameMountBindingKind::ChildOutlet { parent, outlet, anchor, anchor_binding, expected_anchor_children, .. }
                if parent == mounted.parent() && outlet == mounted.outlet() && *anchor == mounted.anchor()
                    && anchor_binding.anchor() == mounted.anchor_binding().anchor()
                    && anchor_binding.source_claim().entity() == mounted.anchor_claim().entity()
                    && anchor_binding.source_claim().owner() == mounted.anchor_claim().owner()
                    && anchor_binding.source_claim().seal() == mounted.anchor_claim().seal()
                    && (anchor_binding.is_native() || expected_anchor_children == mounted.expected_anchor_children()))
    }

    pub(crate) fn same_physical_mount(&self, other: &Self) -> bool {
        if self.slot != other.slot || self.slot_claim != other.slot_claim {
            return false;
        }
        match (&self.kind, &other.kind) {
            (
                FrameMountBindingKind::HostTop { host: left },
                FrameMountBindingKind::HostTop { host: right },
            ) => left == right,
            (
                FrameMountBindingKind::ChildOutlet {
                    parent: left_parent,
                    outlet: left_outlet,
                    anchor: left_anchor,
                    anchor_binding: left_anchor_claim,
                    expected_anchor_children: left_children,
                    ..
                },
                FrameMountBindingKind::ChildOutlet {
                    parent: right_parent,
                    outlet: right_outlet,
                    anchor: right_anchor,
                    anchor_binding: right_anchor_claim,
                    expected_anchor_children: right_children,
                    ..
                },
            ) => {
                left_parent == right_parent
                    && left_outlet == right_outlet
                    && left_anchor == right_anchor
                    && left_anchor_claim == right_anchor_claim
                    && (left_anchor_claim.is_native() || left_children == right_children)
            }
            _ => false,
        }
    }

    fn allows_existing_shell(&self, claim: &ExactRegistryClaim) -> bool {
        matches!(
            &self.kind,
            FrameMountBindingKind::ChildOutlet {
                allowed_existing_shells,
                ..
            } if allowed_existing_shells.iter().any(|allowed| allowed == claim)
        )
    }

    pub(crate) fn staging_existing_shells(&self) -> &[ExactRegistryClaim] {
        match &self.kind {
            FrameMountBindingKind::HostTop { .. } => &[],
            FrameMountBindingKind::ChildOutlet {
                allowed_existing_shells,
                ..
            } => allowed_existing_shells,
        }
    }

    fn internal_claims_can_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> bool {
        match &self.kind {
            FrameMountBindingKind::HostTop { .. } => optional_claim_alias_can_rebind(
                &self.slot_claim,
                claims,
                expected_current,
                expected_next,
            ),
            FrameMountBindingKind::ChildOutlet { anchor_binding, .. } => {
                required_claim_alias_can_rebind(
                    &self.slot_claim,
                    claims,
                    expected_current,
                    expected_next,
                ) && required_claim_alias_can_rebind(
                    anchor_binding.source_claim(),
                    claims,
                    expected_current,
                    expected_next,
                )
            }
        }
    }

    fn apply_internal_claim_rebind(&mut self, claims: &[ExactRegistryClaim]) {
        apply_claim_alias_rebind(&mut self.slot_claim, claims);
        if let FrameMountBindingKind::ChildOutlet {
            anchor_binding,
            allowed_existing_shells,
            ..
        } = &mut self.kind
        {
            apply_claim_alias_rebind(anchor_binding.source_claim_mut(), claims);
            for claim in allowed_existing_shells {
                apply_claim_alias_rebind(claim, claims);
            }
        }
    }

    pub(crate) const fn is_child_outlet(&self) -> bool {
        matches!(self.kind, FrameMountBindingKind::ChildOutlet { .. })
    }
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameShell(RouteFrameVersion);

impl FrameShell {
    pub(crate) fn matches_frame(&self, frame: &RouteFrameVersion) -> bool {
        self.0 == *frame
    }
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameContentTarget(RouteFrameVersion);

impl FrameContentTarget {
    pub(crate) fn matches_frame(&self, frame: &RouteFrameVersion) -> bool {
        self.0 == *frame
    }
}

#[cfg(test)]
#[derive(bevy_ecs::prelude::Resource)]
struct MountChildrenManifestCasDriftProbe {
    entity: Entity,
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrameOutput {
    frame: RouteFrameVersion,
    local_index: usize,
    solid_rect: Option<crate::content::SolidRect>,
}

impl FrameOutput {
    pub(crate) fn matches_owner(&self, frame: &RouteFrameVersion, local_index: usize) -> bool {
        self.frame == *frame && self.local_index == local_index
    }

    pub(crate) fn canonical_values_match(
        &self,
        world: &World,
        entity: Entity,
        lifecycle: EntityLifecycle,
    ) -> bool {
        let (node, background, interactive) = match self.solid_rect {
            None => (
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                },
                BackgroundColor::default(),
                false,
            ),
            Some(rect) => {
                let color = rect.color();
                (
                    Node {
                        width: Val::Px(f32::from(rect.width_px())),
                        height: Val::Px(f32::from(rect.height_px())),
                        flex_shrink: 0.0,
                        ..Node::default()
                    },
                    BackgroundColor(Color::srgba_u8(
                        color.red(),
                        color.green(),
                        color.blue(),
                        color.alpha(),
                    )),
                    true,
                )
            }
        };
        let (visibility, pickable, input_allowed) = match lifecycle {
            EntityLifecycle::PreparedInert | EntityLifecycle::Retained => {
                (Visibility::Hidden, Pickable::IGNORE, false)
            }
            EntityLifecycle::Active => (
                Visibility::Visible,
                if interactive {
                    ACTIVE_PICKABLE
                } else {
                    Pickable::IGNORE
                },
                interactive,
            ),
            EntityLifecycle::Leaving | EntityLifecycle::Quarantined => return false,
        };
        world.get::<Node>(entity) == Some(&node)
            && world.get::<BackgroundColor>(entity) == Some(&background)
            && world.get::<Visibility>(entity) == Some(&visibility)
            && world.get::<Pickable>(entity) == Some(&pickable)
            && world
                .get::<RouterInputBarrier>(entity)
                .is_some_and(|barrier| {
                    if input_allowed {
                        barrier.allows_all()
                    } else {
                        barrier.denies_all()
                    }
                })
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
    anchor_binding: OutletBinding,
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
        self.anchor_binding.source_claim()
    }

    pub(crate) const fn anchor_binding(&self) -> &OutletBinding {
        &self.anchor_binding
    }

    fn claim_can_rebind(
        &self,
        claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> bool {
        required_claim_alias_can_rebind(
            self.anchor_binding.source_claim(),
            claims,
            expected_current,
            expected_next,
        )
    }

    fn apply_claim_rebind(&mut self, claims: &[ExactRegistryClaim]) {
        apply_claim_alias_rebind(self.anchor_binding.source_claim_mut(), claims);
    }
}

#[derive(Debug)]
pub(crate) struct RealizedFrame {
    frame: RouteFrameVersion,
    host_candidate: ValidatedHostCandidate,
    mount: FrameMountBinding,
    prior_mount_children: Vec<Entity>,
    shell: Entity,
    mount_stage: FrameMountStage,
    entities: Vec<RealizedEntity>,
    outlets: Vec<RealizedOutlet>,
    prewrite_complete: bool,
    #[cfg(feature = "bsn")]
    native: Option<native_frame::NativeFrameContent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedEmptySlotBoundary {
    claim: ExactRegistryClaim,
    parent: RouteFrameVersion,
    outlet: bevy_ui_router_core::slot::RouteSlotKey,
    slot: bevy_ui_router_core::ids::RouteSlotId,
    anchor_binding: OutletBinding,
    expected_anchor_children: Vec<Entity>,
}

/// A preservation proof, never a physical or logical lifecycle capability.
/// Authored native descendants stay open and are re-observed on every use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetainedBoundaryProof {
    projection: RetainedLeafProjection,
    host_candidate: ValidatedHostCandidate,
    mount: FrameMountBinding,
    shell: Entity,
    native_root: Option<Entity>,
    entities: Vec<RealizedEntity>,
    empty_slots: Vec<RetainedEmptySlotBoundary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedEntityObservation {
    entity: Entity,
    present: bool,
    claim: Option<ExactRegistryClaim>,
    manifest: Option<ComponentManifest>,
    seal: Option<RouteOwnershipSeal>,
    lifecycle: Option<FrameLifecycle>,
    parent: Option<Entity>,
    children: Vec<Entity>,
}

impl RetainedEntityObservation {
    fn observe(world: &World, entity: Entity) -> Self {
        Self {
            entity,
            present: world.get_entity(entity).is_ok(),
            claim: world
                .get_resource::<RouteEntityRegistry>()
                .and_then(|registry| registry.record(entity))
                .map(|record| record.claim()),
            manifest: ComponentManifest::from_world(world, entity),
            seal: world.get::<RouteOwnershipSeal>(entity).copied(),
            lifecycle: world.get::<FrameLifecycle>(entity).copied(),
            parent: world.get::<ChildOf>(entity).map(ChildOf::parent),
            children: world
                .get::<Children>(entity)
                .map(|children| children.iter().copied().collect())
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RetainedInvalidationEvidence {
    expected: RetainedBoundaryProof,
    observed: Vec<RetainedEntityObservation>,
    host: ValidatedHostCandidateObservation,
}

impl RetainedInvalidationEvidence {
    pub(crate) const fn projection(&self) -> &RetainedLeafProjection {
        &self.expected.projection
    }

    pub(crate) fn matches_payload(
        &self,
        frame: &RealizedFrame,
        slots: &[MountedChildSlot],
    ) -> bool {
        self.expected.matches_payload(frame, slots)
    }

    pub(crate) fn is_current(&self, world: &World) -> bool {
        self.expected.shared_authority_is_current(world) && self.observed_source_is_current(world)
    }

    pub(crate) fn observed_source_is_current(&self, world: &World) -> bool {
        self.host == observe_validated_host_candidate(world, &self.expected.host_candidate)
            && self.observed.iter().all(|observed| {
                *observed == RetainedEntityObservation::observe(world, observed.entity)
            })
    }

    pub(crate) fn preservation_is_current(&self, world: &World) -> bool {
        self.expected.preservation_is_current(world)
    }
}

#[derive(Debug)]
pub(crate) enum RetainedPayloadClassification {
    Current,
    OwnInvalid(RetainedInvalidationEvidence),
    AuthorityLost(RetainedInvalidationEvidence),
}

impl RetainedBoundaryProof {
    pub(crate) fn prepare<'a>(
        projection: &RetainedLeafProjection,
        frame: &RealizedFrame,
        slots: impl Iterator<Item = &'a MountedChildSlot>,
        projected: &[ExactRegistryClaim],
    ) -> Option<Self> {
        if frame.frame() != projection.frame() || !frame.mount.matches_retained_scope(projection) {
            return None;
        }
        let project = |source: &ExactRegistryClaim| -> Option<ExactRegistryClaim> {
            let next = projected
                .iter()
                .find(|next| next.entity() == source.entity())
                .unwrap_or(source);
            (source.entity() == next.entity()
                && source.owner() == next.owner()
                && source.seal() == next.seal())
            .then(|| next.clone())
        };
        let mut entities = frame.entities.clone();
        for entity in &mut entities {
            entity.claim = project(&entity.claim)?;
            if entity.claim.lifecycle() != EntityLifecycle::Retained {
                return None;
            }
        }
        if entities
            .iter()
            .filter(|entity| {
                entity.entity == frame.shell()
                    && entity.role == RealizedRole::Shell
                    && entity.claim.owner()
                        == &RegistryOwner::Committed(EntityOwner::FrameShell(
                            projection.frame().clone(),
                        ))
            })
            .count()
            != 1
            || entities.iter().enumerate().any(|(index, entity)| {
                entities[index + 1..]
                    .iter()
                    .any(|other| entity.entity == other.entity)
            })
        {
            return None;
        }
        let mut mount = frame.mount.clone();
        mount.slot_claim = project(&mount.slot_claim)?;
        if let FrameMountBindingKind::ChildOutlet {
            anchor_binding,
            allowed_existing_shells,
            ..
        } = &mut mount.kind
        {
            *anchor_binding.source_claim_mut() = project(anchor_binding.source_claim())?;
            // Old staging peers are not an authority source for later retention.
            allowed_existing_shells.clear();
        }
        let mut empty_slots = Vec::new();
        for slot in slots {
            if slot.parent() != projection.frame() {
                return None;
            }
            let claim = project(slot.slot_claim())?;
            if claim.lifecycle() != EntityLifecycle::Retained
                || claim.owner()
                    != &RegistryOwner::Committed(EntityOwner::ChildSlot {
                        parent: projection.frame().clone(),
                        outlet: slot.outlet().clone(),
                        slot: slot.slot_id(),
                    })
                || empty_slots
                    .iter()
                    .any(|prior: &RetainedEmptySlotBoundary| prior.claim.entity() == claim.entity())
            {
                return None;
            }
            let mut anchor_binding = slot.anchor_binding().clone();
            *anchor_binding.source_claim_mut() = project(anchor_binding.source_claim())?;
            empty_slots.push(RetainedEmptySlotBoundary {
                claim,
                parent: slot.parent().clone(),
                outlet: slot.outlet().clone(),
                slot: slot.slot_id(),
                anchor_binding,
                expected_anchor_children: slot.expected_anchor_children().to_vec(),
            });
        }
        Some(Self {
            projection: projection.clone(),
            host_candidate: frame.host_candidate.clone(),
            mount,
            shell: frame.shell(),
            native_root: frame.native_root(),
            entities,
            empty_slots,
        })
    }

    pub(crate) fn matches_payload(
        &self,
        frame: &RealizedFrame,
        slots: &[MountedChildSlot],
    ) -> bool {
        frame.frame() == self.projection.frame()
            && frame.shell == self.shell
            && frame.native_root() == self.native_root
            && frame.mount.matches_retained_scope(&self.projection)
            && frame.entities == self.entities
            && slots.len() == self.empty_slots.len()
            && slots.iter().all(|slot| {
                self.empty_slots
                    .iter()
                    .filter(|expected| {
                        expected.claim == *slot.slot_claim()
                            && expected.parent == *slot.parent()
                            && expected.outlet == *slot.outlet()
                            && expected.slot == slot.slot_id()
                            && expected.anchor_binding == *slot.anchor_binding()
                            && expected.expected_anchor_children == slot.expected_anchor_children()
                    })
                    .count()
                    == 1
            })
    }

    pub(crate) const fn projection(&self) -> &RetainedLeafProjection {
        &self.projection
    }
    pub(crate) const fn shell(&self) -> Entity {
        self.shell
    }
    pub(crate) const fn slot_entity(&self) -> Entity {
        self.mount.slot
    }
    pub(crate) fn shell_claim(&self) -> &ExactRegistryClaim {
        &self
            .entities
            .iter()
            .find(|entity| entity.entity == self.shell)
            .expect("prepared retained boundary has its exact shell")
            .claim
    }

    fn shared_authority_is_current(&self, world: &World) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        let frame = self.projection.frame();
        if !self.mount.matches_retained_scope(&self.projection)
            || !validated_host_candidate_is_current(world, &self.host_candidate)
        {
            return false;
        }
        let FrameMountBindingKind::ChildOutlet {
            parent,
            outlet,
            anchor,
            anchor_binding,
            expected_anchor_children,
            ..
        } = &self.mount.kind
        else {
            return false;
        };
        let Some(slot_record) = registry.record(self.mount.slot) else {
            return false;
        };
        // The shared parent slot can independently become empty or active
        // while this leaf remains inactive. Its instance identity is frozen;
        // its two permitted current roles are checked against the World.
        if slot_record.entity() != self.mount.slot_claim.entity()
            || slot_record.owner() != self.mount.slot_claim.owner()
            || slot_record.seal() != self.mount.slot_claim.seal()
        {
            return false;
        }
        let slot_claim = slot_record.claim();
        let slot_lifecycle = slot_claim.lifecycle();
        let (slot_component, slot_visibility) = match slot_lifecycle {
            EntityLifecycle::Active => (ChildSlotLifecycle::Active, Visibility::Visible),
            EntityLifecycle::PreparedInert => {
                (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden)
            }
            EntityLifecycle::Retained
                if registry.retained_parent_owns_slot(world, parent, &slot_claim) =>
            {
                (ChildSlotLifecycle::Retained, Visibility::Hidden)
            }
            EntityLifecycle::Retained | EntityLifecycle::Leaving | EntityLifecycle::Quarantined => {
                return false;
            }
        };
        let mut current_anchor = anchor_binding.clone();
        let source = current_anchor.source_claim();
        let Some(current) = registry
            .record(source.entity())
            .map(|record| record.claim())
        else {
            return false;
        };
        if current.owner() != source.owner()
            || current.seal() != source.seal()
            || !matches!(
                current.lifecycle(),
                EntityLifecycle::Active | EntityLifecycle::Retained
            )
        {
            return false;
        }
        *current_anchor.source_claim_mut() = current;
        if validate_child_mount_slot(
            world,
            &self.host_candidate,
            self.mount.slot,
            &slot_claim,
            parent,
            outlet,
            *anchor,
            &current_anchor,
            expected_anchor_children,
            frame,
            &[],
            true,
            slot_lifecycle,
            slot_component,
            slot_visibility,
        )
        .is_err()
        {
            return false;
        }
        true
    }

    pub(crate) fn classify_payload_with_entering(
        &self,
        world: &World,
        fresh: crate::instance::PayloadRef<'_>,
        allowance: Option<&crate::branch_transaction::SealedMountNewStagingAllowance>,
    ) -> RetainedPayloadClassification {
        if self.shared_authority_is_current(world)
            && self.own_payload_is_current_with_context(world, Some(fresh), allowance, None)
        {
            RetainedPayloadClassification::Current
        } else {
            self.classify_payload(world)
        }
    }

    pub(crate) fn classify_payload_with_staged_slot(
        &self,
        world: &World,
        staged: &crate::mount::ProvisionalChildSlot,
    ) -> RetainedPayloadClassification {
        let shared = self.shared_authority_is_current(world);
        if shared && self.own_payload_is_current_with_context(world, None, None, Some(staged)) {
            RetainedPayloadClassification::Current
        } else {
            self.classify_invalid_payload(world, shared, Some(staged.slot()))
        }
    }

    pub(crate) fn classify_payload(&self, world: &World) -> RetainedPayloadClassification {
        let shared = self.shared_authority_is_current(world);
        if shared && self.own_payload_is_current(world) {
            return RetainedPayloadClassification::Current;
        }
        self.classify_invalid_payload(world, shared, None)
    }

    fn classify_invalid_payload(
        &self,
        world: &World,
        shared: bool,
        staged_slot: Option<Entity>,
    ) -> RetainedPayloadClassification {
        let mut observed_entities = self
            .entities
            .iter()
            .map(|entity| entity.entity)
            .chain(self.empty_slots.iter().map(|slot| slot.claim.entity()))
            .chain([
                self.host_candidate.host(),
                self.mount.slot(),
                self.mount.physical_parent().0,
            ])
            .chain(
                self.mount
                    .outlet_binding()
                    .map(|binding| binding.source_claim().entity()),
            )
            .chain(staged_slot)
            .collect::<Vec<_>>();
        observed_entities.sort_by_key(|entity| entity.to_bits());
        observed_entities.dedup();
        let evidence = RetainedInvalidationEvidence {
            expected: self.clone(),
            observed: observed_entities
                .into_iter()
                .map(|entity| RetainedEntityObservation::observe(world, entity))
                .collect(),
            host: observe_validated_host_candidate(world, &self.host_candidate),
        };
        if shared {
            RetainedPayloadClassification::OwnInvalid(evidence)
        } else {
            RetainedPayloadClassification::AuthorityLost(evidence)
        }
    }

    pub(crate) fn is_current(&self, world: &World) -> bool {
        self.shared_authority_is_current(world) && self.own_payload_is_current(world)
    }

    /// Certifies only an inert, protected sibling boundary. Its content may
    /// need retirement and is never authorized for activation by this proof.
    pub(crate) fn preservation_is_current(&self, world: &World) -> bool {
        let Some(shell) = self
            .entities
            .iter()
            .find(|entity| entity.entity == self.shell)
        else {
            return false;
        };
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        if !self.shared_authority_is_current(world)
            || !registry
                .record(self.shell)
                .is_some_and(|record| record.claim() == shell.claim)
            || world.get::<RouteOwnershipSeal>(self.shell)
                != Some(&RouteOwnershipSeal::new(shell.claim.seal()))
            || world.get::<FrameLifecycle>(self.shell)
                != Some(&FrameLifecycle(EntityLifecycle::Retained))
            || world.get::<FrameShell>(self.shell)
                != Some(&FrameShell(self.projection.frame().clone()))
            || world.get::<ChildOf>(self.shell).map(ChildOf::parent) != Some(self.mount.slot)
            || world
                .get::<Children>(self.mount.slot)
                .into_iter()
                .flat_map(|children| children.iter())
                .filter(|child| **child == self.shell)
                .count()
                != 1
            || !shell_visual_values_are_current(
                world,
                self.shell,
                self.projection.frame(),
                EntityLifecycle::Retained,
            )
            || world.get::<Pickable>(self.shell) != Some(&Pickable::IGNORE)
            || !world
                .get::<RouterInputBarrier>(self.shell)
                .is_some_and(RouterInputBarrier::denies_all)
        {
            return false;
        }
        let mut pending = vec![self.shell];
        let mut seen = HashSet::new();
        while let Some(entity) = pending.pop() {
            if !seen.insert(entity) || world.get_entity(entity).is_err() {
                return false;
            }
            if let Some(children) = world.get::<Children>(entity) {
                for child in children.iter().copied() {
                    if world.get::<ChildOf>(child).map(ChildOf::parent) != Some(entity) {
                        return false;
                    }
                    pending.push(child);
                }
            }
        }
        // The complete live closure is already needed for structural validation.
        // Intersect it with the gate once instead of scanning all permissions for
        // every retained descendant. No observation survives this World borrow.
        !world
            .get_resource::<crate::input::RouterInputGate>()
            .is_some_and(|gate| {
                gate.allowed_projection()
                    .iter()
                    .any(|entity| seen.contains(entity))
            })
            && !world
                .get_resource::<InputFocus>()
                .is_some_and(|focus| focus.get().is_some_and(|entity| seen.contains(&entity)))
            && !world
                .get_resource::<RouteFocusState>()
                .is_some_and(|focus| focus.focused().is_some_and(|entity| seen.contains(&entity)))
    }

    pub(crate) fn is_shared_authority_current(&self, world: &World) -> bool {
        self.shared_authority_is_current(world)
    }
    pub(crate) fn own_slot_claim(&self, entity: Entity) -> Option<&ExactRegistryClaim> {
        self.empty_slots
            .iter()
            .find(|slot| slot.claim.entity() == entity)
            .map(|slot| &slot.claim)
    }

    pub(crate) fn own_parent_identity_is_current(&self, world: &World) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        self.entities.iter().all(|entity| {
            registry
                .record(entity.entity)
                .is_some_and(|record| record.claim() == entity.claim)
                && world.get::<RouteOwnershipSeal>(entity.entity)
                    == Some(&RouteOwnershipSeal::new(entity.claim.seal()))
                && world.get::<FrameLifecycle>(entity.entity)
                    == Some(&FrameLifecycle(EntityLifecycle::Retained))
                && world.get::<ChildOf>(entity.entity).map(ChildOf::parent) == Some(entity.parent)
        }) && self.empty_slots.iter().all(|slot| {
            registry
                .record(slot.claim.entity())
                .is_some_and(|record| record.claim() == slot.claim)
                && world.get::<RouteOwnershipSeal>(slot.claim.entity())
                    == Some(&RouteOwnershipSeal::new(slot.claim.seal()))
                && world
                    .get::<ChildOf>(slot.claim.entity())
                    .map(ChildOf::parent)
                    == Some(slot.anchor_binding.anchor())
        })
    }

    pub(crate) fn owns_slot(&self, claim: &ExactRegistryClaim) -> bool {
        self.empty_slots.iter().any(|slot| {
            slot.claim.entity() == claim.entity()
                && slot.claim.owner() == claim.owner()
                && slot.claim.seal() == claim.seal()
                && claim.lifecycle() == EntityLifecycle::Retained
        })
    }

    pub(crate) fn is_current_with_staged_slot(
        &self,
        world: &World,
        staged: &crate::mount::ProvisionalChildSlot,
    ) -> bool {
        self.shared_authority_is_current(world)
            && self.own_payload_is_current_with_context(world, None, None, Some(staged))
    }

    pub(crate) fn is_current_with_fresh(
        &self,
        world: &World,
        fresh: crate::instance::PayloadRef<'_>,
    ) -> bool {
        self.shared_authority_is_current(world)
            && self.own_payload_is_current_with_fresh(world, Some(fresh))
    }

    fn own_payload_is_current(&self, world: &World) -> bool {
        self.own_payload_is_current_with_fresh(world, None)
    }

    fn own_payload_is_current_with_fresh(
        &self,
        world: &World,
        fresh: Option<crate::instance::PayloadRef<'_>>,
    ) -> bool {
        self.own_payload_is_current_with_context(world, fresh, None, None)
    }

    fn own_payload_is_current_with_context(
        &self,
        world: &World,
        fresh: Option<crate::instance::PayloadRef<'_>>,
        allowance: Option<&crate::branch_transaction::SealedMountNewStagingAllowance>,
        staged: Option<&crate::mount::ProvisionalChildSlot>,
    ) -> bool {
        let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
            return false;
        };
        let frame = self.projection.frame();
        for entity in &self.entities {
            if !registry.record(entity.entity).is_some_and(|record| {
                record.claim() == entity.claim
                    && record.lifecycle() == EntityLifecycle::Retained
                    && (self.native_root.is_some()
                        || manifest_contains_expected(world, entity.entity, record.manifest()))
            }) || world.get::<RouteOwnershipSeal>(entity.entity)
                != Some(&RouteOwnershipSeal::new(entity.claim.seal()))
                || world.get::<FrameLifecycle>(entity.entity)
                    != Some(&FrameLifecycle(EntityLifecycle::Retained))
                || world.get::<ChildOf>(entity.entity).map(ChildOf::parent) != Some(entity.parent)
            {
                return false;
            }
            let role_matches = match entity.role {
                RealizedRole::Shell => {
                    world.get::<FrameShell>(entity.entity) == Some(&FrameShell(frame.clone()))
                        && shell_visual_values_are_current(
                            world,
                            entity.entity,
                            frame,
                            EntityLifecycle::Retained,
                        )
                }
                RealizedRole::ContentTarget if self.native_root.is_some() => {
                    world.get::<FrameContentTarget>(entity.entity)
                        == Some(&FrameContentTarget(frame.clone()))
                }
                _ => realized_values_match(world, entity, frame),
            };
            if !role_matches {
                return false;
            }
            if self.native_root.is_none() || entity.entity == self.shell {
                if world.get::<Visibility>(entity.entity) != Some(&Visibility::Hidden)
                    || world.get::<Pickable>(entity.entity) != Some(&Pickable::IGNORE)
                    || !world
                        .get::<RouterInputBarrier>(entity.entity)
                        .is_some_and(RouterInputBarrier::denies_all)
                {
                    return false;
                }
            }
        }
        if self.native_root.is_some()
            && !world.get::<Children>(self.shell).is_some_and(|children| {
                children.len() == 1 && Some(children[0]) == self.native_root
            })
        {
            return false;
        }
        #[cfg(feature = "bsn")]
        if self
            .native_root
            .is_some_and(|root| native_frame::collect_content(world, frame, root).is_err())
        {
            return false;
        }
        for slot in &self.empty_slots {
            if !registry.record(slot.claim.entity()).is_some_and(|record| {
                record.claim() == slot.claim
                    && record.manifest().matches(world, slot.claim.entity())
            }) || world.get::<RouteOwnershipSeal>(slot.claim.entity())
                != Some(&RouteOwnershipSeal::new(slot.claim.seal()))
                || world.get::<ChildSlotLifecycle>(slot.claim.entity())
                    != Some(&ChildSlotLifecycle::Retained)
                || world.get::<Visibility>(slot.claim.entity()) != Some(&Visibility::Hidden)
                || world.get::<Pickable>(slot.claim.entity()) != Some(&Pickable::IGNORE)
                || !world
                    .get::<RouterInputBarrier>(slot.claim.entity())
                    .is_some_and(RouterInputBarrier::denies_all)
                || world
                    .get::<ChildOf>(slot.claim.entity())
                    .map(ChildOf::parent)
                    != Some(slot.anchor_binding.anchor())
                || !world
                    .get::<ChildSlotAuthority>(slot.claim.entity())
                    .is_some_and(|authority| {
                        authority.parent() == &slot.parent
                            && authority.outlet() == &slot.outlet
                            && authority.slot() == slot.slot
                    })
                || !slot.anchor_binding.is_current_with_manifest_policy(
                    world,
                    &slot.parent,
                    true,
                    true,
                )
                || !slot.anchor_binding.children_are_current(
                    world,
                    slot.claim.entity(),
                    &slot.expected_anchor_children,
                )
            {
                return false;
            }
        }
        let mut pending = vec![self.shell];
        let mut seen = HashSet::new();
        while let Some(entity) = pending.pop() {
            if !seen.insert(entity) || world.get_entity(entity).is_err() {
                return false;
            }
            if let Some(record) = registry.record(entity) {
                let owned =
                    self.entities
                        .iter()
                        .any(|binding| binding.entity == entity && record.claim() == binding.claim)
                        || self.empty_slots.iter().any(|slot| {
                            slot.claim.entity() == entity && record.claim() == slot.claim
                        });
                let exact_staging = allowance.is_some_and(|allowance| {
                    allowance.parent() == frame
                        && allowance.entity() == entity
                        && allowance.claim() == &record.claim()
                        && allowance.matches_binding(
                            frame,
                            allowance.outlet(),
                            allowance.slot(),
                            entity,
                            allowance.claim(),
                            allowance.anchor(),
                            allowance.anchor_binding(),
                        )
                });
                let exact_staged_slot = staged.is_some_and(|staged| {
                    staged.parent() == frame
                        && staged.slot() == entity
                        && staged.slot_claim() == &record.claim()
                        && staged
                            .anchor_binding()
                            .is_current_with_manifest_policy(world, frame, true, true)
                });
                if !owned
                    && (fresh
                        .is_some_and(|fresh| fresh.claims().any(|claim| *claim == record.claim()))
                        || exact_staging
                        || exact_staged_slot)
                {
                    if !matches!(record.lifecycle(), EntityLifecycle::PreparedInert) {
                        return false;
                    }
                    // The mixed fresh validator proves this complete original payload separately.
                    continue;
                }
                if !owned
                    && matches!(
                        record.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameShell(_))
                    )
                    && world.get::<ChildOf>(entity).is_some_and(|parent| {
                        self.empty_slots
                            .iter()
                            .any(|slot| slot.claim.entity() == parent.parent())
                    })
                    && registry.retained_shell_is_current(
                        world,
                        entity,
                        world
                            .get::<ChildOf>(entity)
                            .expect("parent observed")
                            .parent(),
                    )
                {
                    continue;
                }
                if !self
                    .entities
                    .iter()
                    .any(|binding| binding.entity == entity && record.claim() == binding.claim)
                    && !self
                        .empty_slots
                        .iter()
                        .any(|slot| slot.claim.entity() == entity && record.claim() == slot.claim)
                {
                    return false;
                }
            } else if world
                .get_resource::<QuarantineLeaseTable>()
                .is_some_and(|table| {
                    table.record(entity).is_some() || table.unresolved_record(entity).is_some()
                })
            {
                return false;
            }
            if world
                .get_resource::<crate::input::RouterInputGate>()
                .is_some_and(|gate| {
                    gate.allows_pointer(entity)
                        || gate.allows_keyboard(entity)
                        || gate.allows_gamepad(entity)
                })
                || world
                    .get_resource::<InputFocus>()
                    .is_some_and(|focus| focus.get() == Some(entity))
                || world
                    .get_resource::<RouteFocusState>()
                    .is_some_and(|focus| focus.focused() == Some(entity))
            {
                return false;
            }
            if let Some(children) = world.get::<Children>(entity) {
                for child in children.iter().copied() {
                    if world.get::<ChildOf>(child).map(ChildOf::parent) != Some(entity) {
                        return false;
                    }
                    pending.push(child);
                }
            }
        }
        self.entities
            .iter()
            .all(|entity| seen.contains(&entity.entity))
            && self
                .empty_slots
                .iter()
                .all(|slot| seen.contains(&slot.claim.entity()))
    }
}

/// Fresh readonly mount facts for an existing retained physical owner.
/// Consuming this packet only refreshes aliases; it creates no ownership.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedRetainedFrameActivation {
    mount: FrameMountBinding,
    prior_mount_children: Vec<Entity>,
}

impl PreparedRetainedFrameActivation {
    pub(crate) const fn mount(&self) -> &FrameMountBinding {
        &self.mount
    }
}

pub(crate) struct PreparedFrameMountRebase {
    prior_mount_children: Vec<Entity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameMountStage {
    ShellAllocated,
    ShellAttached,
}

pub(crate) struct VerifiedFrameQuarantineTransfer {
    claims: Vec<ExactRegistryClaim>,
    manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

impl VerifiedFrameQuarantineTransfer {
    #[cfg(test)]
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
    visible_before: bool,
    visible_after: bool,
    interactive: bool,
    kind: PresentationTargetKind,
}

/// Layout can be application-controlled only for an exact live transition
/// surface. Presence stays explicit; ordinary shells retain their full value
/// contract. Callers still validate structure, input and ownership separately.
fn shell_visual_values_are_current(
    world: &World,
    shell: Entity,
    frame: &RouteFrameVersion,
    lifecycle: EntityLifecycle,
) -> bool {
    let authorized_display =
        crate::transition::transition_surface_display(world, frame, shell, lifecycle);
    let held_presence = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(shell))
        .is_some_and(|record| crate::transition::holds_visible_claim(world, &record.claim()));
    let display =
        authorized_display.unwrap_or(if lifecycle == EntityLifecycle::Active || held_presence {
            bevy_ui::Display::Flex
        } else {
            bevy_ui::Display::None
        });
    let expected = Node {
        display,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    };
    world.get::<Node>(shell).is_some_and(|node| {
        node.display == display && (authorized_display.is_some() || *node == expected)
    }) && world.get::<Visibility>(shell)
        == Some(if display == bevy_ui::Display::None {
            &Visibility::Hidden
        } else {
            &Visibility::Visible
        })
}

impl FrameMutationTarget {
    pub(crate) const fn entity(&self) -> Entity {
        self.entity
    }

    /// A visual hold changes presence only. The caller must derive this target
    /// from its original lifecycle owner; input and activity remain unchanged.
    pub(crate) fn with_visual_presence(mut self, before: bool, after: bool) -> Self {
        self.visible_before = before;
        self.visible_after = after;
        self
    }

    pub(crate) const fn is_active(&self) -> bool {
        matches!(self.current, EntityLifecycle::Active)
    }

    pub(crate) const fn has_inactive_presence(&self) -> bool {
        self.visible_after && !self.active_after
    }

    /// Freeze the release target from the already prepared next role, before
    /// publication. Releasing presence never reactivates a target.
    pub(crate) fn presence_release(mut self) -> Self {
        self.current = self.next;
        self.visible_before = self.visible_after;
        self.visible_after = false;
        self.active_after = false;
        self
    }
}

#[derive(Clone, Copy)]
enum PresentationTargetKind {
    Frame,
    Shell,
    NativeShell,
    NativeContent,
    ChildSlot,
}

enum PreparedLifecycleMutation<'w> {
    Frame(Mut<'w, FrameLifecycle>),
    ChildSlot(Mut<'w, ChildSlotLifecycle>),
}

struct PreparedEntityMutation<'w> {
    #[cfg(test)]
    entity: Entity,
    visual: Option<PreparedVisualMutation<'w>>,
    shell_node: Option<Mut<'w, Node>>,
    lifecycle: PreparedLifecycleMutation<'w>,
    current: EntityLifecycle,
    next: EntityLifecycle,
    active_after: bool,
    visible_after: bool,
    interactive: bool,
}

struct PreparedVisualMutation<'w> {
    visibility: Mut<'w, Visibility>,
    pickable: Mut<'w, Pickable>,
    barrier: Mut<'w, RouterInputBarrier>,
}

pub(crate) struct PreparedPresentationMutation<'w> {
    entities: Vec<(usize, PreparedEntityMutation<'w>)>,
    pointer: PreparedPointerMutation<'w>,
}

pub(crate) struct PreparedCommitPresentationMutation<'w> {
    entities: Vec<(usize, PreparedEntityMutation<'w>)>,
    pointer: PreparedPointerMutation<'w>,
    split_at: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TestCommitMutation {
    Leaving(Entity),
    Entering(Entity),
    PreparedInert(Entity),
}

#[cfg(test)]
thread_local! {
    static TEST_COMMIT_MUTATIONS: std::cell::RefCell<Option<Vec<TestCommitMutation>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn arm_test_commit_mutation_trace() {
    TEST_COMMIT_MUTATIONS.with(|trace| *trace.borrow_mut() = Some(Vec::new()));
}

#[cfg(test)]
pub(crate) fn take_test_commit_mutation_trace() -> Vec<TestCommitMutation> {
    TEST_COMMIT_MUTATIONS.with(|trace| trace.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
fn record_test_commit_mutation(entity: Entity, next: EntityLifecycle) {
    TEST_COMMIT_MUTATIONS.with(|trace| {
        let mut trace = trace.borrow_mut();
        let Some(trace) = trace.as_mut() else {
            return;
        };
        match next {
            EntityLifecycle::Leaving | EntityLifecycle::Retained => {
                trace.push(TestCommitMutation::Leaving(entity))
            }
            EntityLifecycle::Active => trace.push(TestCommitMutation::Entering(entity)),
            EntityLifecycle::PreparedInert => {
                trace.push(TestCommitMutation::PreparedInert(entity));
            }
            EntityLifecycle::Quarantined => {}
        }
    });
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct CommitPresentationState<'a, 'w> {
    leaving: &'a [(usize, PreparedEntityMutation<'w>)],
    entering: &'a [(usize, PreparedEntityMutation<'w>)],
}

impl PreparedLifecycleMutation<'_> {
    fn apply(&mut self, next: EntityLifecycle) {
        match self {
            Self::Frame(lifecycle) => **lifecycle = FrameLifecycle(next),
            Self::ChildSlot(lifecycle) => **lifecycle = child_slot_lifecycle(next),
        }
    }

    #[cfg(test)]
    fn matches(&self, expected: EntityLifecycle) -> bool {
        match self {
            Self::Frame(lifecycle) => lifecycle.0 == expected,
            Self::ChildSlot(lifecycle) => **lifecycle == child_slot_lifecycle(expected),
        }
    }
}

impl PreparedEntityMutation<'_> {
    fn apply_final(&mut self) {
        self.apply_base_activation();
        self.apply_capabilities();
    }

    fn apply_base_activation(&mut self) {
        if let Some(visual) = &mut self.visual {
            *visual.visibility = if self.visible_after {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if let Some(node) = &mut self.shell_node {
            node.display = if self.visible_after {
                bevy_ui::Display::Flex
            } else {
                bevy_ui::Display::None
            };
        }
        self.lifecycle.apply(self.next);
    }

    fn apply_capabilities(&mut self) {
        let Some(visual) = &mut self.visual else {
            return;
        };
        *visual.pickable = if self.active_after && self.interactive {
            ACTIVE_PICKABLE
        } else {
            Pickable::IGNORE
        };
        *visual.barrier = if self.active_after && self.interactive {
            RouterInputBarrier::allow_all()
        } else {
            RouterInputBarrier::deny_all()
        };
    }

    #[cfg(test)]
    fn is_prepared_inert(&self) -> bool {
        self.visual.as_ref().is_none_or(|visual| {
            *visual.visibility == Visibility::Hidden
                && *visual.pickable == Pickable::IGNORE
                && visual.barrier.denies_all()
        }) && self.lifecycle.matches(self.current)
    }

    #[cfg(test)]
    fn is_base_active(&self) -> bool {
        self.visual.as_ref().is_none_or(|visual| {
            *visual.visibility
                == if self.visible_after {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                }
        }) && self.lifecycle.matches(self.next)
    }

    #[cfg(test)]
    fn is_final(&self) -> bool {
        self.visual.as_ref().is_none_or(|visual| {
            *visual.visibility
                == if self.visible_after {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                }
                && *visual.pickable
                    == if self.active_after && self.interactive {
                        ACTIVE_PICKABLE
                    } else {
                        Pickable::IGNORE
                    }
                && if self.active_after && self.interactive {
                    visual.barrier.allows_all()
                } else {
                    visual.barrier.denies_all()
                }
        }) && self.lifecycle.matches(self.next)
    }
}

impl<'w> PreparedPresentationMutation<'w> {
    pub(crate) fn apply(self) {
        self.pointer.apply();
        for (_, mut entity) in self.entities {
            entity.apply_final();
        }
    }

    pub(crate) fn seal_commit(
        self,
        split_at: usize,
    ) -> Option<PreparedCommitPresentationMutation<'w>> {
        let valid = split_at <= self.entities.len()
            && self
                .entities
                .iter()
                .enumerate()
                .all(|(position, (index, entity))| {
                    *index == position
                        && if position < split_at {
                            matches!(
                                (entity.current, entity.next),
                                (
                                    EntityLifecycle::Active | EntityLifecycle::PreparedInert,
                                    EntityLifecycle::Leaving | EntityLifecycle::Retained
                                ) | (
                                    EntityLifecycle::Retained,
                                    EntityLifecycle::Leaving | EntityLifecycle::PreparedInert
                                ) | (EntityLifecycle::Active, EntityLifecycle::PreparedInert)
                            ) && !entity.active_after
                        } else {
                            matches!(
                                entity.current,
                                EntityLifecycle::PreparedInert | EntityLifecycle::Retained
                            ) && entity.next == EntityLifecycle::Active
                                && entity.active_after
                        }
                });
        valid.then_some(PreparedCommitPresentationMutation {
            entities: self.entities,
            pointer: self.pointer,
            split_at,
        })
    }
}

impl<'w> PreparedCommitPresentationMutation<'w> {
    fn state(&self) -> CommitPresentationState<'_, 'w> {
        let (leaving, entering) = self.entities.split_at(self.split_at);
        CommitPresentationState { leaving, entering }
    }

    pub(crate) fn apply_staged(
        mut self,
        focus: impl FnOnce(CommitPresentationState<'_, 'w>),
        registry: impl FnOnce(CommitPresentationState<'_, 'w>),
        gate: impl FnOnce(CommitPresentationState<'_, 'w>),
    ) {
        for (_, entity) in &mut self.entities[..self.split_at] {
            #[cfg(test)]
            record_test_commit_mutation(entity.entity, entity.next);
            entity.apply_final();
        }
        focus(self.state());
        for (_, entity) in &mut self.entities[self.split_at..] {
            #[cfg(test)]
            record_test_commit_mutation(entity.entity, entity.next);
            entity.apply_base_activation();
        }
        registry(self.state());
        for (_, entity) in &mut self.entities[self.split_at..] {
            entity.apply_capabilities();
        }
        // Native state is revoked in place while all prepared component and
        // resource borrows are still held, before gate publication returns.
        self.pointer.apply();
        let (leaving, entering) = self.entities.split_at(self.split_at);
        gate(CommitPresentationState { leaving, entering });
    }
}

#[cfg(test)]
impl CommitPresentationState<'_, '_> {
    pub(crate) fn leaving_is_isolated(&self) -> bool {
        self.leaving.iter().all(|(_, entity)| entity.is_final())
    }

    pub(crate) fn entering_is_prepared_inert(&self) -> bool {
        self.entering
            .iter()
            .all(|(_, entity)| entity.is_prepared_inert())
    }

    pub(crate) fn entering_base_is_active(&self) -> bool {
        self.entering
            .iter()
            .all(|(_, entity)| entity.is_base_active())
    }

    pub(crate) fn entering_capabilities_are_active(&self) -> bool {
        self.entering.iter().all(|(_, entity)| entity.is_final())
    }
}

const fn child_slot_lifecycle(lifecycle: EntityLifecycle) -> ChildSlotLifecycle {
    match lifecycle {
        EntityLifecycle::PreparedInert => ChildSlotLifecycle::RegisteredInert,
        EntityLifecycle::Active => ChildSlotLifecycle::Active,
        EntityLifecycle::Retained => ChildSlotLifecycle::Retained,
        EntityLifecycle::Leaving => ChildSlotLifecycle::Leaving,
        EntityLifecycle::Quarantined => ChildSlotLifecycle::Quarantined,
    }
}

pub(crate) fn child_slot_mutation_target(
    slot: &MountedChildSlot,
    current: EntityLifecycle,
    next: EntityLifecycle,
    active_after: bool,
) -> FrameMutationTarget {
    FrameMutationTarget {
        entity: slot.slot(),
        current,
        next,
        active_after,
        visible_before: current == EntityLifecycle::Active,
        visible_after: active_after,
        interactive: false,
        kind: PresentationTargetKind::ChildSlot,
    }
}

impl RealizedFrame {
    pub(crate) fn cleanup_host(&self) -> Entity {
        self.host_candidate.host()
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn native_focus_owner(&self) -> Option<&crate::native_focus::NativeFocusOwner> {
        self.native.as_ref()?.focus_owner.as_ref()
    }

    pub(crate) fn consume_native_focus_owner(&mut self) {
        #[cfg(feature = "bsn")]
        if let Some(native) = self.native.as_mut() {
            native.focus_owner.take();
        }
    }
    pub(crate) fn native_root(&self) -> Option<Entity> {
        #[cfg(feature = "bsn")]
        {
            self.native.as_ref().map(|native| native.root)
        }
        #[cfg(not(feature = "bsn"))]
        {
            None
        }
    }

    pub(crate) fn live_interactive_entities(&self, world: &World) -> Vec<Entity> {
        self.content_input_facts(world, &[]).1
    }

    /// Observe membership without computing authored visibility or input eligibility.
    pub(crate) fn managed_content_entities(&self, world: &World) -> Vec<Entity> {
        #[cfg(feature = "bsn")]
        if let Some(root) = self.native_root() {
            return native_frame::collect_content(world, self.frame(), root).unwrap_or_default();
        }
        let _ = world;
        self.claims().map(|claim| claim.entity()).collect()
    }

    /// Current managed content and its eligible subset. A visual ancestor is
    /// observed without extending this frame's content/ownership boundary.
    pub(crate) fn content_input_facts(
        &self,
        world: &World,
        next: &[FrameMutationTarget],
    ) -> (Vec<Entity>, Vec<Entity>) {
        #[cfg(feature = "bsn")]
        if self.native_root().is_some() {
            let managed = self.managed_content_entities(world);
            let eligible = self.input_entities_for_observed_content(world, next, &managed);
            return (managed, eligible);
        }
        let _ = (world, next);
        (Vec::new(), self.interactive_entities())
    }

    /// Derive eligibility from membership observed in this same immutable World
    /// borrow. Callers retain the membership proof and its observation lifetime.
    pub(crate) fn input_entities_for_observed_content(
        &self,
        world: &World,
        next: &[FrameMutationTarget],
        managed: &[Entity],
    ) -> Vec<Entity> {
        #[cfg(feature = "bsn")]
        if self.native_root().is_some() {
            let mut visibility = native_frame::AuthoredVisibilityObservation::new(world, next);
            return managed
                .iter()
                .copied()
                .filter(|entity| {
                    // Native widgets such as ScrollbarThumb have computed UI
                    // geometry without participating in Node/taffy layout.
                    world.get::<ComputedNode>(*entity).is_some() && visibility.is_visible(*entity)
                })
                .collect();
        }
        let _ = (world, next, managed);
        self.interactive_entities()
    }
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

    #[cfg(test)]
    pub(crate) fn replace_mount_binding_for_test(&mut self, mount: FrameMountBinding) {
        self.mount = mount;
    }

    #[cfg(test)]
    pub(crate) fn replace_host_top_mount_host_for_test(&mut self, replacement: Entity) -> bool {
        self.mount.replace_host_top_host_for_test(replacement)
    }

    #[cfg(test)]
    pub(crate) fn replace_shell_entity_for_test(&mut self, shell: Entity) {
        self.shell = shell;
    }

    #[cfg(test)]
    pub(crate) fn replace_shell_parent_for_test(&mut self, parent: Entity) {
        let shell = self.shell;
        self.entities
            .iter_mut()
            .find(|realized| realized.entity == shell)
            .expect("test parent replacement targets the stored frame shell")
            .parent = parent;
    }

    #[cfg(test)]
    pub(crate) fn replace_primary_claim_for_test(
        &mut self,
        entity: Entity,
        claim: ExactRegistryClaim,
    ) {
        self.entities
            .iter_mut()
            .find(|realized| realized.entity == entity)
            .expect("test claim replacement targets one stored frame entity")
            .claim = claim;
    }

    pub(crate) fn sealed_reconciliation_entering_shell_for(
        &self,
        existing: &Self,
    ) -> Option<&ExactRegistryClaim> {
        let existing_shell = existing.entities.iter().find_map(|entity| {
            (entity.entity == existing.shell
                && entity.claim.lifecycle() == EntityLifecycle::Active
                && matches!(
                    entity.claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                        if owner == existing.frame()
                ))
            .then_some(&entity.claim)
        })?;
        let entering_shell = self.entities.iter().find_map(|entity| {
            (entity.entity == self.shell
                && entity.claim.lifecycle() == EntityLifecycle::PreparedInert
                && matches!(
                    entity.claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                        if owner == self.frame()
                ))
            .then_some(&entity.claim)
        })?;
        (self.mount.same_physical_mount(&existing.mount)
            && self.mount.allows_existing_shell(existing_shell))
        .then_some(entering_shell)
    }

    pub(crate) fn sealed_reconciliation_leaving_sibling_shell_for<'a>(
        &self,
        leaving: &'a Self,
    ) -> Option<&'a ExactRegistryClaim> {
        let leaving_shell = leaving.entities.iter().find_map(|entity| {
            (entity.entity == leaving.shell
                && entity.claim.lifecycle() == EntityLifecycle::Active
                && matches!(
                    entity.claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                        if owner == leaving.frame()
                ))
            .then_some(&entity.claim)
        })?;
        let entering_shell_is_exact = self
            .entities
            .iter()
            .find_map(|entity| {
                (entity.entity == self.shell
                    && entity.claim.lifecycle() == EntityLifecycle::PreparedInert
                    && matches!(
                        entity.claim.owner(),
                        RegistryOwner::Committed(EntityOwner::FrameShell(owner))
                            if owner == self.frame()
                    ))
                .then_some(&entity.claim)
            })
            .is_some();
        (entering_shell_is_exact && self.mount.same_physical_mount(&leaving.mount))
            .then_some(leaving_shell)
    }

    pub(crate) fn prior_mount_children(&self) -> &[Entity] {
        &self.prior_mount_children
    }

    pub(crate) const fn shell_is_attached(&self) -> bool {
        matches!(self.mount_stage, FrameMountStage::ShellAttached)
    }

    pub(crate) fn prepare_retained_activation(
        &self,
        world: &World,
        projection: &RetainedLeafProjection,
        slot: &MountedChildSlot,
        leaving_shells: &[ExactRegistryClaim],
    ) -> Option<PreparedRetainedFrameActivation> {
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        if self.frame() != projection.frame()
            || !self.shell_is_attached()
            || !self.mount.matches_retained_scope(projection)
            || self.mount.slot != slot.slot()
            || self.mount.slot_claim.owner() != slot.slot_claim().owner()
            || self.mount.slot_claim.seal() != slot.slot_claim().seal()
        {
            return None;
        }
        #[cfg(feature = "bsn")]
        if self.native_focus_owner().is_some() {
            return None;
        }
        let FrameMountBindingKind::ChildOutlet {
            parent,
            outlet,
            anchor,
            anchor_binding,
            ..
        } = &self.mount.kind
        else {
            return None;
        };
        if parent != slot.parent()
            || outlet != slot.outlet()
            || *anchor != slot.anchor()
            || anchor_binding.anchor() != slot.anchor_binding().anchor()
            || anchor_binding.source_claim().entity() != slot.anchor_claim().entity()
            || anchor_binding.source_claim().owner() != slot.anchor_claim().owner()
            || anchor_binding.source_claim().seal() != slot.anchor_claim().seal()
        {
            return None;
        }
        let children = world
            .get::<Children>(slot.slot())?
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if children
            .iter()
            .filter(|child| **child == self.shell)
            .count()
            != 1
        {
            return None;
        }
        let mut peers = Vec::new();
        peers
            .try_reserve_exact(children.len().saturating_sub(1))
            .ok()?;
        let mut prior = Vec::new();
        prior
            .try_reserve_exact(children.len().saturating_sub(1))
            .ok()?;
        for child in children.into_iter().filter(|child| *child != self.shell) {
            if world.get::<ChildOf>(child).map(ChildOf::parent) != Some(slot.slot()) {
                return None;
            }
            let claim = registry.record(child)?.claim();
            let current_leaving =
                leaving_shells.contains(&claim) && claim.lifecycle() == EntityLifecycle::Active;
            if !current_leaving && !registry.retained_shell_is_current(world, child, slot.slot()) {
                return None;
            }
            prior.push(child);
            // Publication rebinds peers selected for retirement to Leaving.
            // Peers that remain Retained are removed from this alias list and
            // continue to use their independent retained-boundary proof.
            peers.push(claim);
        }
        let mut mount = FrameMountBinding::from_mounted_child_slot(slot);
        if let FrameMountBindingKind::ChildOutlet {
            staging_lifecycle,
            allowed_existing_shells,
            ..
        } = &mut mount.kind
        {
            *staging_lifecycle = slot.slot_claim().lifecycle();
            *allowed_existing_shells = peers;
        }
        Some(PreparedRetainedFrameActivation {
            mount,
            prior_mount_children: prior,
        })
    }

    pub(crate) fn apply_retained_activation(&mut self, prepared: PreparedRetainedFrameActivation) {
        self.mount = prepared.mount;
        self.prior_mount_children = prepared.prior_mount_children;
    }

    pub(crate) fn prepare_mount_rebase(&self, leaving: &Self) -> Option<PreparedFrameMountRebase> {
        let mut expected_prior = leaving.prior_mount_children.clone();
        expected_prior.push(leaving.shell);
        if !self.shell_is_attached()
            || !leaving.shell_is_attached()
            || !self.mount.same_physical_mount(&leaving.mount)
            || self.prior_mount_children != expected_prior
        {
            return None;
        }
        Some(PreparedFrameMountRebase {
            prior_mount_children: leaving.prior_mount_children.clone(),
        })
    }

    pub(crate) fn prepare_mount_rebase_after_cleanup(
        &self,
        world: &World,
        removed: &[Entity],
    ) -> Option<PreparedFrameMountRebase> {
        let slot = self.mount.slot();
        let children = world
            .get::<Children>(slot)?
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if !self.shell_is_attached()
            || world.get::<ChildOf>(self.shell).map(ChildOf::parent) != Some(slot)
            || children
                .iter()
                .filter(|child| **child == self.shell)
                .count()
                != 1
            || removed.contains(&self.shell)
            || removed.iter().enumerate().any(|(index, entity)| {
                removed[index + 1..].contains(entity)
                    || children.iter().filter(|child| *child == entity).count() != 1
            })
        {
            return None;
        }
        Some(PreparedFrameMountRebase {
            prior_mount_children: children
                .into_iter()
                .filter(|child| *child != self.shell && !removed.contains(child))
                .collect(),
        })
    }

    pub(crate) fn apply_mount_rebase(&mut self, prepared: PreparedFrameMountRebase) {
        self.prior_mount_children = prepared.prior_mount_children;
    }

    pub(crate) fn expected_owned_children(&self, parent: Entity) -> Vec<Entity> {
        expected_children(self, parent)
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

    #[cfg(test)]
    pub(crate) fn mark_prewrite_incomplete_for_test(&mut self) {
        self.prewrite_complete = false;
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
                visible_before: current == EntityLifecycle::Active,
                visible_after: active_after,
                interactive: entity.role == RealizedRole::SolidRect,
                kind: if self.native_root().is_some() {
                    if entity.entity == self.shell {
                        PresentationTargetKind::NativeShell
                    } else {
                        PresentationTargetKind::NativeContent
                    }
                } else {
                    if entity.role == RealizedRole::Shell {
                        PresentationTargetKind::Shell
                    } else {
                        PresentationTargetKind::Frame
                    }
                },
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

    fn apply_primary_claim_rebind(&mut self, claims: &[ExactRegistryClaim]) {
        for (entity, claim) in self.entities.iter_mut().zip(claims) {
            entity.claim = claim.clone();
        }
    }

    pub(crate) fn claims_can_rebind(
        &self,
        primary_claims: &[ExactRegistryClaim],
        all_claims: &[ExactRegistryClaim],
        expected_current: EntityLifecycle,
        expected_next: EntityLifecycle,
    ) -> bool {
        primary_claims.len() == self.entities.len()
            && self
                .entities
                .iter()
                .zip(primary_claims)
                .all(|(entity, claim)| {
                    claim_rebind_is_exact(&entity.claim, claim, expected_current, expected_next)
                })
            && self
                .mount
                .internal_claims_can_rebind(all_claims, expected_current, expected_next)
            && self
                .outlets
                .iter()
                .all(|outlet| outlet.claim_can_rebind(all_claims, expected_current, expected_next))
    }

    pub(crate) fn apply_claim_rebind(
        &mut self,
        primary_claims: &[ExactRegistryClaim],
        all_claims: &[ExactRegistryClaim],
    ) {
        self.apply_primary_claim_rebind(primary_claims);
        self.mount.apply_internal_claim_rebind(all_claims);
        if let FrameMountBindingKind::ChildOutlet {
            allowed_existing_shells,
            ..
        } = &mut self.mount.kind
        {
            allowed_existing_shells.retain(|claim| claim.lifecycle() != EntityLifecycle::Retained);
        }
        for outlet in &mut self.outlets {
            outlet.apply_claim_rebind(all_claims);
        }
    }
}

pub(crate) fn claim_rebind_is_exact(
    current: &ExactRegistryClaim,
    next: &ExactRegistryClaim,
    expected_current: EntityLifecycle,
    expected_next: EntityLifecycle,
) -> bool {
    current.entity() == next.entity()
        && current.owner() == next.owner()
        && current.seal() == next.seal()
        && current.lifecycle() == expected_current
        && next.lifecycle() == expected_next
}

fn required_claim_alias_can_rebind(
    current: &ExactRegistryClaim,
    claims: &[ExactRegistryClaim],
    expected_current: EntityLifecycle,
    expected_next: EntityLifecycle,
) -> bool {
    claims
        .iter()
        .find(|claim| claim.entity() == current.entity())
        .is_some_and(|next| claim_rebind_is_exact(current, next, expected_current, expected_next))
}

fn optional_claim_alias_can_rebind(
    current: &ExactRegistryClaim,
    claims: &[ExactRegistryClaim],
    expected_current: EntityLifecycle,
    expected_next: EntityLifecycle,
) -> bool {
    claims
        .iter()
        .find(|claim| claim.entity() == current.entity())
        .is_none_or(|next| claim_rebind_is_exact(current, next, expected_current, expected_next))
}

fn apply_claim_alias_rebind(current: &mut ExactRegistryClaim, claims: &[ExactRegistryClaim]) {
    if let Some(next) = claims
        .iter()
        .find(|claim| claim.entity() == current.entity())
    {
        *current = next.clone();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RealizationError {
    #[cfg(feature = "bsn")]
    NativeFocus(crate::native_focus::NativeFocusError),
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
    let host = host_candidate.host();
    realize_frame_with_binding_deferred(
        world,
        host_candidate,
        FrameMountBinding::host_top(top_slot, top_slot_claim, host),
        frame,
        output,
        plan,
    )
}

#[cfg(test)]
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

pub(crate) fn realize_reconciliation_child_frame_deferred(
    world: &mut World,
    mount: &crate::branch_transaction::SealedReconciliationStagingMount<'_>,
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
    let parent = mount.parent();
    let mounted = mount.slot();
    if parent.frame() != mounted.parent()
        || task_parent != *mounted.parent()
        || task_outlet != *mounted.outlet()
        || task.frame().slot() != mounted.slot_id()
    {
        return Err(DeferredRealizationFailure::before_complete_frame(
            RealizationError::InvalidMount,
        ));
    }
    realize_frame_with_binding_deferred(
        world,
        parent.host_candidate.clone(),
        FrameMountBinding::from_reconciliation_staging_mount(mount),
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
    let mut realized = Vec::with_capacity(plan.entries().len() + 2);
    let shell = allocate_registered(
        world,
        RegistryOwner::Committed(EntityOwner::FrameShell(frame.clone())),
        RealizedRole::Shell,
        mount_slot,
        true,
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
        let mut registered =
            match register_allocated(world, entity, owner, role, parent, has_children) {
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
            let anchor_binding = realized
                .iter()
                .find(|entity| entity.entity == anchor)
                .expect("realized outlet anchor must retain its exact claim")
                .claim
                .clone();
            RealizedOutlet {
                parent: frame.clone(),
                key: outlet.key().clone(),
                anchor,
                anchor_binding: OutletBinding::RegisteredOutput(anchor_binding),
            }
        })
        .collect();
    let result = RealizedFrame {
        frame,
        host_candidate,
        mount,
        prior_mount_children: mount_children_before,
        shell: realized[0].entity,
        mount_stage: FrameMountStage::ShellAttached,
        entities: realized,
        outlets,
        prewrite_complete: true,
        #[cfg(feature = "bsn")]
        native: None,
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
        mount_stage: FrameMountStage::ShellAllocated,
        entities,
        outlets: Vec::new(),
        prewrite_complete: false,
        #[cfg(feature = "bsn")]
        native: None,
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
        FrameMountBindingKind::HostTop { .. } => validate_top_slot(
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
            anchor_binding,
            expected_anchor_children,
            staging_lifecycle,
            retained_parent,
            ..
        } => {
            let (component_lifecycle, visibility) = match staging_lifecycle {
                EntityLifecycle::PreparedInert => {
                    (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden)
                }
                EntityLifecycle::Active => (ChildSlotLifecycle::Active, Visibility::Visible),
                EntityLifecycle::Retained
                    if retained_parent.as_ref().is_some_and(|projection| {
                        projection.frame() == parent
                            && world
                                .resource::<RouteEntityRegistry>()
                                .retained_parent_is_current(world, projection)
                    }) =>
                {
                    (ChildSlotLifecycle::Retained, Visibility::Hidden)
                }
                EntityLifecycle::Retained
                | EntityLifecycle::Leaving
                | EntityLifecycle::Quarantined => {
                    return Err(RealizationError::InvalidMount);
                }
            };
            validate_child_mount_slot(
                world,
                host_candidate,
                mount.slot,
                &mount.slot_claim,
                parent,
                outlet,
                *anchor,
                anchor_binding,
                expected_anchor_children,
                frame,
                allowed_manifest_additions,
                false,
                *staging_lifecycle,
                component_lifecycle,
                visibility,
            )
        }
    }
}

fn validate_committed_mount_binding(
    world: &World,
    host_candidate: &ValidatedHostCandidate,
    mount: &FrameMountBinding,
    frame: &RouteFrameVersion,
    allow_leaving_child_boundary_drift: bool,
) -> Result<(), RealizationError> {
    match &mount.kind {
        FrameMountBindingKind::HostTop { .. } => validate_top_slot(
            world,
            host_candidate,
            mount.slot,
            &mount.slot_claim,
            frame,
            &[],
        ),
        FrameMountBindingKind::ChildOutlet {
            parent,
            outlet,
            anchor,
            anchor_binding,
            expected_anchor_children,
            ..
        } => validate_child_mount_slot(
            world,
            host_candidate,
            mount.slot,
            &mount.slot_claim,
            parent,
            outlet,
            *anchor,
            anchor_binding,
            expected_anchor_children,
            frame,
            &[],
            allow_leaving_child_boundary_drift,
            EntityLifecycle::Active,
            ChildSlotLifecycle::Active,
            Visibility::Visible,
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
    anchor_binding: &OutletBinding,
    expected_anchor_children: &[Entity],
    frame: &RouteFrameVersion,
    allowed_manifest_additions: &[ComponentId],
    allow_arbitrary_manifest_additions: bool,
    expected_registry_lifecycle: EntityLifecycle,
    expected_component_lifecycle: ChildSlotLifecycle,
    expected_visibility: Visibility,
) -> Result<(), RealizationError> {
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return Err(RealizationError::InvalidMount);
    };
    let Some(slot_record) = registry.record(slot_entity) else {
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
    let expected_node = Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..Node::default()
    };
    if !frame_versions_share_router(frame, parent)
        || frame.host() != parent.host()
        || slot_record.claim() != *slot_claim
        || !owner_is_exact
        || slot_record.lifecycle() != expected_registry_lifecycle
        || !(if allow_arbitrary_manifest_additions {
            manifest_contains_expected(world, slot_entity, slot_record.manifest())
        } else {
            manifest_matches_allowing_additions(
                world,
                slot_entity,
                slot_record.manifest(),
                allowed_manifest_additions,
            )
        })
        || world.get::<RouteOwnershipSeal>(slot_entity)
            != Some(&RouteOwnershipSeal::new(slot_record.seal()))
        || world.get::<Visibility>(slot_entity) != Some(&expected_visibility)
        || world.get::<Pickable>(slot_entity) != Some(&Pickable::IGNORE)
        || !world
            .get::<RouterInputBarrier>(slot_entity)
            .is_some_and(RouterInputBarrier::denies_all)
        || world.get::<Node>(slot_entity) != Some(&expected_node)
        || world.get::<ChildSlotLifecycle>(slot_entity) != Some(&expected_component_lifecycle)
        || !world
            .get::<ChildSlotAuthority>(slot_entity)
            .is_some_and(|authority| {
                authority.parent() == parent
                    && authority.outlet() == outlet
                    && authority.slot() == frame.slot()
            })
        || world.get::<ChildOf>(slot_entity).map(ChildOf::parent) != Some(anchor)
        || anchor_binding.anchor() != anchor
        || !anchor_binding.is_current_with_manifest_policy(
            world,
            parent,
            true,
            allow_arbitrary_manifest_additions,
        )
        || !anchor_binding.children_are_current(world, slot_entity, expected_anchor_children)
        || !validated_host_candidate_is_current(world, host_candidate)
    {
        return Err(RealizationError::InvalidMount);
    }
    Ok(())
}

fn manifest_contains_expected(world: &World, entity: Entity, expected: &ComponentManifest) -> bool {
    ComponentManifest::from_world(world, entity).is_some_and(|actual| {
        expected
            .component_ids()
            .iter()
            .all(|component| actual.component_ids().contains(component))
    })
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
) -> Result<RealizedEntity, RealizationError> {
    let entity = world.spawn_empty().id();
    register_allocated(world, entity, owner, role, parent, has_children)
}

fn register_allocated(
    world: &mut World,
    entity: Entity,
    owner: RegistryOwner,
    role: RealizedRole,
    parent: Entity,
    has_children: bool,
) -> Result<RealizedEntity, RealizationError> {
    let manifest = realization_manifest(world, role, has_children);
    let claim = {
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        match registry.register(entity, owner, manifest, EntityLifecycle::PreparedInert) {
            Ok(claim) => claim,
            Err(_) => {
                world.despawn(entity);
                return Err(RealizationError::RegistryRejected);
            }
        }
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
    let (node, background) = match kind {
        RealizedContentKind::Empty => (
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
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
        solid_rect: match kind {
            RealizedContentKind::Empty => None,
            RealizedContentKind::SolidRect(rect) => Some(rect),
        },
    });
    world
        .entity_mut(realized.entity)
        .insert(FrameLifecycle(EntityLifecycle::PreparedInert));
}

fn realization_manifest(
    world: &mut World,
    role: RealizedRole,
    has_children: bool,
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
    let mut complete = explicit.clone();
    for component in explicit {
        if let Some(required) = world.get_required_components_by_id(component) {
            complete.extend(required.iter_ids());
        }
    }
    ComponentManifest::new(ComponentManifest::observed_component_ids(world, &complete))
}

#[cfg(test)]
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
        FrameMountBindingKind::HostTop { .. } => vec![
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
        registry,
        ComponentManifest::observed_component_ids(world, &expected).collect::<Vec<_>>(),
        "registry manifest drifted for {entity:?}"
    );
}

#[cfg(feature = "bsn")]
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

#[cfg(test)]
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
    validate_realized_frame_allowing_child_slots(world, frame, &[], None, &[])
}

pub(crate) fn validate_realized_frame_with_child_slots(
    world: &World,
    frame: &RealizedFrame,
    child_slots: &[(Entity, Vec<Entity>)],
) -> bool {
    validate_realized_frame_allowing_child_slots(world, frame, child_slots, None, &[])
}

pub(crate) fn validate_reconciliation_entering_frame_with_child_slots(
    world: &World,
    frame: &RealizedFrame,
    child_slots: &[(Entity, Vec<Entity>)],
    existing_mount: Option<&RealizedFrame>,
    observed_leaving_shells: &[ExactRegistryClaim],
) -> bool {
    validate_realized_frame_allowing_child_slots(
        world,
        frame,
        child_slots,
        existing_mount,
        observed_leaving_shells,
    )
}

fn validate_realized_frame_allowing_child_slots(
    world: &World,
    frame: &RealizedFrame,
    child_slots: &[(Entity, Vec<Entity>)],
    existing_mount: Option<&RealizedFrame>,
    observed_leaving_shells: &[ExactRegistryClaim],
) -> bool {
    #[cfg(feature = "bsn")]
    if frame.native_focus_owner().is_some() && !crate::native_focus::source_is_valid(world) {
        return false;
    }
    let sealed_existing_mount = existing_mount.is_some_and(|existing| {
        frame
            .sealed_reconciliation_entering_shell_for(existing)
            .is_some()
    });
    let mount_valid = if sealed_existing_mount {
        validate_committed_mount_binding(
            world,
            &frame.host_candidate,
            &frame.mount,
            frame.frame(),
            true,
        )
        .is_ok()
    } else {
        validate_mount_binding(world, &frame.host_candidate, &frame.mount, frame.frame()).is_ok()
    };
    let mount_children_valid = if sealed_existing_mount {
        mount_slot_children_are_acceptable(
            world,
            frame,
            CurrentShellManifestPolicy::Strict,
            true,
            &[],
            observed_leaving_shells,
        )
    } else {
        mount_slot_children_are_acceptable(
            world,
            frame,
            CurrentShellManifestPolicy::Strict,
            false,
            &[],
            observed_leaving_shells,
        )
    };
    let focus_present = frame_focus_is_in_subtree(world, frame);
    if !mount_valid || !mount_children_valid || focus_present {
        return false;
    }
    #[cfg(feature = "bsn")]
    if frame.native_root().is_some() {
        return native_frame::validate_native_values(world, frame, EntityLifecycle::PreparedInert);
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
            child_slots
                .iter()
                .find_map(|(anchor, expected)| {
                    (*anchor == realized.entity)
                        .then(|| exact_children_match(world, realized.entity, expected.clone()))
                })
                .unwrap_or_else(|| {
                    exact_children_match(
                        world,
                        realized.entity,
                        expected_children(frame, realized.entity),
                    )
                }),
            realized_values_match(world, realized, frame.frame()),
        ];
        checks.into_iter().all(|valid| valid)
    })
}

pub(crate) fn prepare_presentation_mutation<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
) -> Option<PreparedPresentationMutation<'w>> {
    prepare_presentation_with_pointer_revocation(world, targets, &[])
}

/// Acquire presentation and existing pointer component borrows together. Pointer
/// revocations may include a previously allowed target absent from the current
/// presentation mutation, so their selection is independent of that partition.
pub(crate) fn prepare_presentation_with_pointer_revocation<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
    pointer_targets: &[Entity],
) -> Option<PreparedPresentationMutation<'w>> {
    prepare_presentation_access(world, targets, pointer_targets, false).map(|access| access.0)
}

/// All publication receivers remain components on their exact resource
/// entities. Dropping these borrows never inserts a component or flushes World.
pub(crate) struct PublicationResources<'w> {
    pub(crate) navigation: Mut<'w, crate::navigation::RouterNavigation>,
    pub(crate) registry: Mut<'w, RouteEntityRegistry>,
    pub(crate) gate: Mut<'w, crate::input::RouterInputGate>,
    pub(crate) input_focus: Mut<'w, InputFocus>,
    pub(crate) route_focus: Mut<'w, RouteFocusState>,
}

pub(crate) fn prepare_publication_access<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
    pointer_targets: &[Entity],
) -> Option<(PreparedPresentationMutation<'w>, PublicationResources<'w>)> {
    let (presentation, resources) =
        prepare_presentation_access(world, targets, pointer_targets, true)?;
    Some((presentation, resources?))
}

fn prepare_presentation_access<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
    pointer_targets: &[Entity],
    publication: bool,
) -> Option<(
    PreparedPresentationMutation<'w>,
    Option<PublicationResources<'w>>,
)> {
    // Preserve the original presentation query's default-filter eligibility.
    // Native pointer history, unlike presentation mutation, must still be
    // reachable for entities disabled after the gesture began.
    let mut presentation_eligibility = world.query::<(
        &Visibility,
        Option<&Pickable>,
        &RouterInputBarrier,
        Option<&FrameLifecycle>,
        Option<&ChildSlotLifecycle>,
    )>();
    if targets.iter().any(|target| {
        !matches!(target.kind, PresentationTargetKind::NativeContent)
            && presentation_eligibility.get(world, target.entity).is_err()
    }) {
        return None;
    }
    let pointer_targets = pointer_targets.iter().copied().collect::<HashSet<_>>();
    let mut access_entities =
        EntityHashMap::with_capacity(targets.len() + pointer_targets.len() + 8);
    for (index, target) in targets.iter().enumerate() {
        if access_entities.insert(target.entity, Some(index)).is_some() {
            return None;
        }
    }
    let hover_entity = pointer_resource_entity::<HoverMap>(world);
    let previous_hover_entity = pointer_resource_entity::<PreviousHoverMap>(world);
    let state_entity = pointer_resource_entity::<PointerState>(world);
    let navigation_entity = pointer_resource_entity::<crate::navigation::RouterNavigation>(world);
    let registry_entity = pointer_resource_entity::<RouteEntityRegistry>(world);
    let gate_entity = pointer_resource_entity::<crate::input::RouterInputGate>(world);
    let input_focus_entity = pointer_resource_entity::<InputFocus>(world);
    let route_focus_entity = pointer_resource_entity::<RouteFocusState>(world);
    // Borrow only authorized targets and the exact native/publication receivers.
    // The union provides Bevy's uniqueness guarantee even when roles overlap.
    for entity in pointer_targets.iter().copied().chain(
        [
            hover_entity,
            previous_hover_entity,
            state_entity,
            navigation_entity,
            registry_entity,
            gate_entity,
            input_focus_entity,
            route_focus_entity,
        ]
        .into_iter()
        .flatten(),
    ) {
        access_entities.entry(entity).or_insert(None);
    }
    let mut query = query_including_disabled::<(
        Entity,
        Option<(
            &mut Visibility,
            Option<&mut Pickable>,
            &mut RouterInputBarrier,
        )>,
        (
            Option<&mut FrameLifecycle>,
            Option<&mut ChildSlotLifecycle>,
            Option<&mut Node>,
        ),
        Option<&mut PickingInteraction>,
        Option<&mut HoverMap>,
        Option<&mut PreviousHoverMap>,
        Option<&mut PointerState>,
        (
            Option<&mut crate::navigation::RouterNavigation>,
            Option<&mut RouteEntityRegistry>,
            Option<&mut crate::input::RouterInputGate>,
            Option<&mut InputFocus>,
            Option<&mut RouteFocusState>,
        ),
    )>(world);
    let mut found = vec![false; targets.len()];
    let mut entities = Vec::with_capacity(targets.len());
    let mut pointer_interactions = Vec::with_capacity(pointer_targets.len());
    let mut pointer_resources = PointerStateResources::default();
    let (mut navigation, mut registry, mut gate, mut input_focus, mut route_focus) =
        (None, None, None, None, None);
    for (
        entity,
        presentation,
        (frame_lifecycle, slot_lifecycle, node),
        interaction,
        hover,
        previous_hover,
        state,
        resources,
    ) in query.iter_many_unique_mut(world, access_entities.keys())
    {
        let (nav, reg, input_gate, focus, route) = resources;
        if publication {
            if Some(entity) == navigation_entity {
                navigation = nav;
            }
            if Some(entity) == registry_entity {
                registry = reg;
            }
            if Some(entity) == gate_entity {
                gate = input_gate;
            }
            if Some(entity) == input_focus_entity {
                input_focus = focus;
            }
            if Some(entity) == route_focus_entity {
                route_focus = route;
            }
        }
        if Some(entity) == hover_entity {
            pointer_resources.hover = hover;
        }
        if Some(entity) == previous_hover_entity {
            pointer_resources.previous_hover = previous_hover;
        }
        if Some(entity) == state_entity {
            pointer_resources.state = state;
        }
        if pointer_targets.contains(&entity)
            && let Some(interaction) = interaction
            && *interaction != PickingInteraction::None
        {
            pointer_interactions.push(interaction);
        }
        let Some(&Some(index)) = access_entities.get(&entity) else {
            continue;
        };
        let target = &targets[index];
        if matches!(target.kind, PresentationTargetKind::NativeContent) {
            let lifecycle = frame_lifecycle?;
            if lifecycle.0 != target.current || slot_lifecycle.is_some() || found[index] {
                return None;
            }
            found[index] = true;
            entities.push((
                index,
                PreparedEntityMutation {
                    #[cfg(test)]
                    entity,
                    visual: None,
                    shell_node: None,
                    lifecycle: PreparedLifecycleMutation::Frame(lifecycle),
                    current: target.current,
                    next: target.next,
                    active_after: target.active_after,
                    visible_after: target.visible_after,
                    interactive: false,
                },
            ));
            continue;
        }
        let (visibility, pickable, barrier) = presentation?;
        let expected_pickable = if target.current == EntityLifecycle::Active && target.interactive {
            ACTIVE_PICKABLE
        } else {
            Pickable::IGNORE
        };
        let Some(pickable) = pickable else {
            return None;
        };
        let pickable_valid = *pickable == expected_pickable;
        let lifecycle = match target.kind {
            PresentationTargetKind::Frame
            | PresentationTargetKind::Shell
            | PresentationTargetKind::NativeShell => match (frame_lifecycle, slot_lifecycle) {
                (Some(lifecycle), None) if lifecycle.0 == target.current => {
                    PreparedLifecycleMutation::Frame(lifecycle)
                }
                _ => return None,
            },
            PresentationTargetKind::ChildSlot => match (frame_lifecycle, slot_lifecycle) {
                (None, Some(lifecycle)) if child_slot_lifecycle(target.current) == *lifecycle => {
                    PreparedLifecycleMutation::ChildSlot(lifecycle)
                }
                _ => return None,
            },
            PresentationTargetKind::NativeContent => unreachable!(),
        };
        if found[index]
            || !pickable_valid
            || *visibility
                != if target.visible_before {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                }
            || (matches!(
                target.current,
                EntityLifecycle::PreparedInert
                    | EntityLifecycle::Retained
                    | EntityLifecycle::Leaving
            ) && !barrier.denies_all())
            || (target.current == EntityLifecycle::Active
                && if target.interactive {
                    !barrier.allows_all()
                } else {
                    !barrier.denies_all()
                })
        {
            return None;
        }
        found[index] = true;
        entities.push((
            index,
            PreparedEntityMutation {
                #[cfg(test)]
                entity,
                visual: Some(PreparedVisualMutation {
                    visibility,
                    pickable,
                    barrier,
                }),
                shell_node: if matches!(target.kind, PresentationTargetKind::NativeShell)
                    || (matches!(target.kind, PresentationTargetKind::Shell)
                        && (target.next == EntityLifecycle::Retained
                            || target.current == EntityLifecycle::Retained))
                {
                    Some(node?)
                } else {
                    None
                },
                lifecycle,
                current: target.current,
                next: target.next,
                active_after: target.active_after,
                visible_after: target.visible_after,
                interactive: target.interactive,
            },
        ));
    }
    entities.sort_by_key(|(index, _)| *index);
    let resources = if publication {
        Some(PublicationResources {
            navigation: navigation?,
            registry: registry?,
            gate: gate?,
            input_focus: input_focus?,
            route_focus: route_focus?,
        })
    } else {
        None
    };
    found.into_iter().all(|present| present).then_some((
        PreparedPresentationMutation {
            entities,
            pointer: pointer_resources.prepare(pointer_targets, pointer_interactions),
        },
        resources,
    ))
}

pub(crate) fn validate_active_frame(world: &World, frame: &RealizedFrame) -> bool {
    validate_active_frame_with_child_slots(world, frame, &[])
}

pub(crate) fn validate_active_frame_local_values(world: &World, frame: &RealizedFrame) -> bool {
    #[cfg(feature = "bsn")]
    if frame.native_root().is_some() {
        return native_frame::validate_native_values(world, frame, EntityLifecycle::Active);
    }
    let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
        return false;
    };
    frame.entities.iter().all(|realized| {
        let interactive = realized.role == RealizedRole::SolidRect;
        registry.record(realized.entity).is_some_and(|record| {
            record.claim() == realized.claim
                && record.lifecycle() == EntityLifecycle::Active
                && manifest_contains_expected(world, realized.entity, record.manifest())
        }) && world.get::<RouteOwnershipSeal>(realized.entity)
            == Some(&RouteOwnershipSeal::new(realized.claim.seal()))
            && world.get::<Visibility>(realized.entity) == Some(&Visibility::Visible)
            && world.get::<Pickable>(realized.entity)
                == Some(if interactive {
                    &ACTIVE_PICKABLE
                } else {
                    &Pickable::IGNORE
                })
            && world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(|barrier| {
                    if interactive {
                        barrier.allows_all()
                    } else {
                        barrier.denies_all()
                    }
                })
            && world.get::<FrameLifecycle>(realized.entity)
                == Some(&FrameLifecycle(EntityLifecycle::Active))
            && world.get::<ChildOf>(realized.entity).map(ChildOf::parent) == Some(realized.parent)
            && realized_values_match(world, realized, frame.frame())
    })
}

pub(crate) fn validate_active_frame_with_child_slots(
    world: &World,
    frame: &RealizedFrame,
    child_slots: &[(Entity, Vec<Entity>)],
) -> bool {
    validate_active_frame_with_child_slots_and_entering_shells(world, frame, child_slots, &[])
}

pub(crate) fn validate_active_frame_with_child_slots_and_entering_shells(
    world: &World,
    frame: &RealizedFrame,
    child_slots: &[(Entity, Vec<Entity>)],
    entering_shells: &[ExactRegistryClaim],
) -> bool {
    matches!(
        classify_committed_frame(
            world,
            frame,
            CommittedFrameRole::Preserved,
            child_slots,
            entering_shells,
        ),
        CommittedFramePreflight::Ready
    )
}

pub(crate) fn validate_leaving_frame_with_child_slots(
    world: &World,
    frame: &RealizedFrame,
    child_slots: &[(Entity, Vec<Entity>)],
) -> bool {
    matches!(
        classify_committed_frame(world, frame, CommittedFrameRole::Leaving, child_slots, &[],),
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

fn committed_frame_boundary_is_valid(
    world: &World,
    frame: &RealizedFrame,
    role: CommittedFrameRole,
    entering_shells: &[ExactRegistryClaim],
) -> bool {
    let leaving_child_boundary = matches!(role, CommittedFrameRole::Leaving)
        && matches!(frame.mount.kind, FrameMountBindingKind::ChildOutlet { .. });
    let binding_valid = validate_committed_mount_binding(
        world,
        &frame.host_candidate,
        &frame.mount,
        frame.frame(),
        leaving_child_boundary,
    )
    .is_ok();
    let mount_children_valid = mount_slot_children_are_acceptable(
        world,
        frame,
        CurrentShellManifestPolicy::ObserveAdditions,
        leaving_child_boundary,
        entering_shells,
        &[],
    );
    binding_valid && mount_children_valid
}

/// Reuse the native values/content proof only within the active read observation.
#[cfg(feature = "bsn")]
pub(crate) fn observe_active_native_frame<'w>(
    world: &'w World,
    frame: &RealizedFrame,
    entering_shells: &[ExactRegistryClaim],
) -> Option<native_frame::NativeContentObservation<'w>> {
    if !committed_frame_boundary_is_valid(
        world,
        frame,
        CommittedFrameRole::Preserved,
        entering_shells,
    ) {
        return None;
    }
    native_frame::observe_native_values(world, frame, EntityLifecycle::Active)
}

fn classify_committed_frame(
    world: &World,
    frame: &RealizedFrame,
    role: CommittedFrameRole,
    child_slots: &[(Entity, Vec<Entity>)],
    entering_shells: &[ExactRegistryClaim],
) -> CommittedFramePreflight {
    if !committed_frame_boundary_is_valid(world, frame, role, entering_shells) {
        return CommittedFramePreflight::Invalid;
    }

    #[cfg(feature = "bsn")]
    if frame.native_root().is_some() {
        return if native_frame::validate_native_values(world, frame, EntityLifecycle::Active) {
            CommittedFramePreflight::Ready
        } else {
            CommittedFramePreflight::Invalid
        };
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
            owned_children_with_foreign_descendants(world, frame, realized.entity, child_slots)
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
    child_slots: &[(Entity, Vec<Entity>)],
) -> Option<bool> {
    let expected = child_slots
        .iter()
        .find_map(|(anchor, expected)| (*anchor == parent).then(|| expected.clone()))
        .unwrap_or_else(|| expected_children(frame, parent));
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

pub(crate) fn isolate_frame_for_settlement(
    world: &mut World,
    frame: &RealizedFrame,
    requires_exact_isolation: &impl Fn(&ExactRegistryClaim) -> bool,
) -> bool {
    isolate_frame_for_settlement_guarded(world, frame, requires_exact_isolation, &|_| true)
}

pub(crate) fn isolate_frame_for_settlement_guarded(
    world: &mut World,
    frame: &RealizedFrame,
    requires_exact_isolation: &impl Fn(&ExactRegistryClaim) -> bool,
    protected_partition_is_exact: &impl Fn(&World) -> bool,
) -> bool {
    #[cfg(feature = "bsn")]
    if frame.native_root().is_some() {
        return native_frame::isolate_native_frame(
            world,
            frame,
            requires_exact_isolation,
            protected_partition_is_exact,
        );
    }
    if !protected_partition_is_exact(world)
        || !frame
            .entities
            .iter()
            .filter(|realized| requires_exact_isolation(&realized.claim))
            .all(|realized| frame_claim_is_current(world, realized))
    {
        return false;
    }
    for realized in frame
        .entities
        .iter()
        .filter(|realized| requires_exact_isolation(&realized.claim))
    {
        if !protected_partition_is_exact(world) || !frame_claim_is_current(world, realized) {
            return false;
        }
        if world.get::<Visibility>(realized.entity).is_none() {
            if !protected_partition_is_exact(world) || !frame_claim_is_current(world, realized) {
                return false;
            }
            world.entity_mut(realized.entity).insert(Visibility::Hidden);
            world.flush();
            if !protected_partition_is_exact(world)
                || !frame_claim_is_current(world, realized)
                || world.get::<Visibility>(realized.entity) != Some(&Visibility::Hidden)
            {
                return false;
            }
        }
        if world.get::<Pickable>(realized.entity).is_none() {
            if !protected_partition_is_exact(world) || !frame_claim_is_current(world, realized) {
                return false;
            }
            world.entity_mut(realized.entity).insert(Pickable::IGNORE);
            world.flush();
            if !protected_partition_is_exact(world)
                || !frame_claim_is_current(world, realized)
                || world.get::<Pickable>(realized.entity) != Some(&Pickable::IGNORE)
            {
                return false;
            }
        }
        if world.get::<RouterInputBarrier>(realized.entity).is_none() {
            if !protected_partition_is_exact(world) || !frame_claim_is_current(world, realized) {
                return false;
            }
            world
                .entity_mut(realized.entity)
                .insert(RouterInputBarrier::deny_all());
            world.flush();
            if !protected_partition_is_exact(world)
                || !frame_claim_is_current(world, realized)
                || !world
                    .get::<RouterInputBarrier>(realized.entity)
                    .is_some_and(RouterInputBarrier::denies_all)
            {
                return false;
            }
        }
        if !protected_partition_is_exact(world) || !frame_claim_is_current(world, realized) {
            return false;
        }
        {
            let Some(mut visibility) = world.get_mut::<Visibility>(realized.entity) else {
                return false;
            };
            *visibility = Visibility::Hidden;
        }
        if !protected_partition_is_exact(world)
            || !frame_claim_is_current(world, realized)
            || world.get::<Visibility>(realized.entity) != Some(&Visibility::Hidden)
        {
            return false;
        }
        {
            if !protected_partition_is_exact(world) {
                return false;
            }
            let Some(mut pickable) = world.get_mut::<Pickable>(realized.entity) else {
                return false;
            };
            *pickable = Pickable::IGNORE;
        }
        if !protected_partition_is_exact(world)
            || !frame_claim_is_current(world, realized)
            || world.get::<Pickable>(realized.entity) != Some(&Pickable::IGNORE)
        {
            return false;
        }
        {
            if !protected_partition_is_exact(world) {
                return false;
            }
            let Some(mut barrier) = world.get_mut::<RouterInputBarrier>(realized.entity) else {
                return false;
            };
            *barrier = RouterInputBarrier::deny_all();
        }
        if !protected_partition_is_exact(world)
            || !frame_claim_is_current(world, realized)
            || !world
                .get::<RouterInputBarrier>(realized.entity)
                .is_some_and(RouterInputBarrier::denies_all)
        {
            return false;
        }
    }
    protected_partition_is_exact(world)
        && frame_is_isolated_for_settlement(world, frame, requires_exact_isolation)
}

pub(crate) fn frame_is_isolated_for_settlement(
    world: &World,
    frame: &RealizedFrame,
    requires_exact_isolation: &impl Fn(&ExactRegistryClaim) -> bool,
) -> bool {
    #[cfg(feature = "bsn")]
    if frame.native_root().is_some() {
        return frame
            .entities
            .iter()
            .filter(|entity| requires_exact_isolation(&entity.claim))
            .all(|entity| {
                frame_claim_is_current(world, entity)
                    && if entity.entity == frame.shell() {
                        world
                            .get::<Node>(entity.entity)
                            .is_some_and(|node| node.display == bevy_ui::Display::None)
                    } else {
                        native_frame::content_scope_is_inert(world, entity.entity)
                    }
            });
    }
    frame
        .entities
        .iter()
        .filter(|realized| requires_exact_isolation(&realized.claim))
        .all(|realized| {
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

#[cfg(test)]
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
            let (node, background) = match realized.solid_rect {
                None => (expected_container, Some(BackgroundColor::default())),
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
                    )
                }
            };
            world.get::<Node>(realized.entity) == Some(&node)
                && world.get::<BackgroundColor>(realized.entity) == background.as_ref()
                && world.get::<FrameOutput>(realized.entity)
                    == Some(&FrameOutput {
                        frame: frame.clone(),
                        local_index: *local_index,
                        solid_rect: realized.solid_rect,
                    })
        }
    }
}

pub(crate) fn expected_children(frame: &RealizedFrame, parent: Entity) -> Vec<Entity> {
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
    mount_slot_children_are_acceptable(
        world,
        frame,
        CurrentShellManifestPolicy::Strict,
        false,
        &[],
        &[],
    )
}

#[derive(Clone, Copy)]
enum CurrentShellManifestPolicy {
    Strict,
    ObserveAdditions,
}

fn mount_slot_children_are_acceptable(
    world: &World,
    frame: &RealizedFrame,
    current_shell_manifest: CurrentShellManifestPolicy,
    allow_foreign_children: bool,
    entering_shells: &[ExactRegistryClaim],
    observed_leaving_shells: &[ExactRegistryClaim],
) -> bool {
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
            if record.lifecycle() == EntityLifecycle::Retained {
                if child == frame.shell
                    || child_frame == &frame.frame
                    || !frame_version_matches_mount(&frame.mount, mount_owner, child_frame)
                    || !registry.retained_shell_is_current(world, child, slot_entity)
                {
                    return false;
                }
                continue;
            }
            if !frame_version_matches_mount(&frame.mount, mount_owner, child_frame)
                || (child == frame.shell && child_frame != &frame.frame)
                || (child != frame.shell && child_frame == &frame.frame)
                || !(if child == frame.shell {
                    match current_shell_manifest {
                        CurrentShellManifestPolicy::Strict => {
                            record.manifest().matches(world, child)
                        }
                        CurrentShellManifestPolicy::ObserveAdditions => {
                            manifest_contains_expected(world, child, record.manifest())
                        }
                    }
                } else if frame.mount.allows_existing_shell(&record.claim())
                    || entering_shells.iter().any(|claim| *claim == record.claim())
                    || observed_leaving_shells
                        .iter()
                        .any(|claim| *claim == record.claim())
                {
                    manifest_contains_expected(world, child, record.manifest())
                } else {
                    record.manifest().matches(world, child)
                })
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
                    let visibility =
                        if crate::transition::holds_visible_claim(world, &record.claim()) {
                            Visibility::Visible
                        } else {
                            Visibility::Hidden
                        };
                    if world.get::<Visibility>(child) != Some(&visibility) {
                        return false;
                    }
                }
                EntityLifecycle::Retained => {
                    return false;
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
        if allow_foreign_children && registry.record(child).is_none() {
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
            FrameMountBindingKind::HostTop { .. },
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
pub(crate) mod tests;
