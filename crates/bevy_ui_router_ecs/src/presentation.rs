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
use bevy_picking::{
    Pickable,
    events::PointerState,
    hover::{HoverMap, PickingInteraction, PreviousHoverMap},
};
use bevy_ui::{
    BackgroundColor, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, Node, ScrollPosition, Val,
};
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
    host::{ValidatedHostCandidate, validated_host_candidate_is_current},
    input::{
        PointerStateResources, PreparedPointerMutation, RouteFocusState, pointer_query,
        pointer_resource_entity,
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
        require_outlet: bool,
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
                    && (!require_outlet || content.iter().filter(|entity| world.get::<bevy_ui_router_bsn::DefaultOutlet>(**entity).is_some()).count() == 1))
                && (!require_outlet || world.get::<Node>(*anchor).is_some()
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
struct FrameShell(RouteFrameVersion);

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

    pub(crate) fn is_current(&self, world: &World) -> bool {
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
            EntityLifecycle::Retained | EntityLifecycle::Leaving | EntityLifecycle::Quarantined => {
                return false;
            }
        };
        if validate_child_mount_slot(
            world,
            &self.host_candidate,
            self.mount.slot,
            &slot_claim,
            parent,
            outlet,
            *anchor,
            anchor_binding,
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
                        && world.get::<Node>(entity.entity)
                            == Some(&Node {
                                display: bevy_ui::Display::None,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..Node::default()
                            })
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
    pub(crate) fn matches_frame(&self, frame: &RealizedFrame) -> bool {
        self.mount == frame.mount && self.prior_mount_children == frame.prior_mount_children
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
    interactive: bool,
    kind: PresentationTargetKind,
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
    scroll: Vec<(Entity, Mut<'w, ScrollPosition>, Option<ScrollPosition>)>,
}

pub(crate) struct PreparedCommitPresentationMutation<'w> {
    entities: Vec<(usize, PreparedEntityMutation<'w>)>,
    pointer: PreparedPointerMutation<'w>,
    scroll: Vec<(Entity, Mut<'w, ScrollPosition>, Option<ScrollPosition>)>,
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
            *visual.visibility = if self.active_after {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
        if let Some(node) = &mut self.shell_node {
            node.display = if self.active_after {
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
                == if self.active_after {
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
                == if self.active_after {
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
            scroll: self.scroll,
            split_at,
        })
    }
}

impl<'w> PreparedCommitPresentationMutation<'w> {
    pub(crate) fn with_scroll(mut self, writes: &[crate::scroll::ScrollWrite]) -> Self {
        for (entity, _, position) in &mut self.scroll {
            *position = writes
                .iter()
                .find(|write| write.entity == *entity)
                .map(|write| write.position.clone());
        }
        self
    }

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
        for (_, position, value) in &mut self.scroll {
            if let Some(value) = value
                && position.0 != value.0
            {
                position.0 = value.0;
            }
        }
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
        interactive: false,
        kind: PresentationTargetKind::ChildSlot,
    }
}

impl RealizedFrame {
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

    /// Current managed content and its eligible subset. A visual ancestor is
    /// observed without extending this frame's content/ownership boundary.
    pub(crate) fn content_input_facts(
        &self,
        world: &World,
        next: &[FrameMutationTarget],
    ) -> (Vec<Entity>, Vec<Entity>) {
        #[cfg(feature = "bsn")]
        if let Some(root) = self.native_root() {
            let managed =
                native_frame::collect_content(world, self.frame(), root).unwrap_or_default();
            let eligible = managed
                .iter()
                .copied()
                .filter(|entity| {
                    world.get::<Node>(*entity).is_some()
                        && native_frame::authored_target_is_visible(world, *entity, next)
                })
                .collect();
            return (managed, eligible);
        }
        let _ = (world, next);
        (Vec::new(), self.interactive_entities())
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
            // Other retained peers keep their independently re-observed proof.
            if current_leaving {
                peers.push(claim);
            }
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
            ..
        } => {
            let (component_lifecycle, visibility) = match staging_lifecycle {
                EntityLifecycle::PreparedInert => {
                    (ChildSlotLifecycle::RegisteredInert, Visibility::Hidden)
                }
                EntityLifecycle::Active => (ChildSlotLifecycle::Active, Visibility::Visible),
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
    prepare_presentation_access(world, targets, pointer_targets, &[], false).map(|access| access.0)
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

#[cfg(test)]
pub(crate) fn prepare_publication_access<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
    pointer_targets: &[Entity],
) -> Option<(PreparedPresentationMutation<'w>, PublicationResources<'w>)> {
    let (presentation, resources) =
        prepare_presentation_access(world, targets, pointer_targets, &[], true)?;
    Some((presentation, resources?))
}

pub(crate) fn prepare_publication_access_with_scroll<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
    pointer_targets: &[Entity],
    scroll_targets: &[Entity],
) -> Option<(PreparedPresentationMutation<'w>, PublicationResources<'w>)> {
    let (presentation, resources) =
        prepare_presentation_access(world, targets, pointer_targets, scroll_targets, true)?;
    Some((presentation, resources?))
}

fn prepare_presentation_access<'w>(
    world: &'w mut World,
    targets: &[FrameMutationTarget],
    pointer_targets: &[Entity],
    scroll_targets: &[Entity],
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
    let hover_entity = pointer_resource_entity::<HoverMap>(world);
    let previous_hover_entity = pointer_resource_entity::<PreviousHoverMap>(world);
    let state_entity = pointer_resource_entity::<PointerState>(world);
    let navigation_entity = pointer_resource_entity::<crate::navigation::RouterNavigation>(world);
    let registry_entity = pointer_resource_entity::<RouteEntityRegistry>(world);
    let gate_entity = pointer_resource_entity::<crate::input::RouterInputGate>(world);
    let input_focus_entity = pointer_resource_entity::<InputFocus>(world);
    let route_focus_entity = pointer_resource_entity::<RouteFocusState>(world);
    let mut query = pointer_query::<(
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
        Option<&mut ScrollPosition>,
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
    let mut scroll = Vec::with_capacity(scroll_targets.len());
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
        scroll_position,
        resources,
    ) in query.iter_mut(world)
    {
        if scroll_targets.contains(&entity)
            && let Some(position) = scroll_position
        {
            scroll.push((entity, position, None));
        }
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
        let Some((index, target)) = targets
            .iter()
            .enumerate()
            .find(|(_, target)| target.entity == entity)
        else {
            continue;
        };
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
            || (matches!(
                target.current,
                EntityLifecycle::PreparedInert
                    | EntityLifecycle::Retained
                    | EntityLifecycle::Leaving
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
            scroll,
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

fn classify_committed_frame(
    world: &World,
    frame: &RealizedFrame,
    role: CommittedFrameRole,
    child_slots: &[(Entity, Vec<Entity>)],
    entering_shells: &[ExactRegistryClaim],
) -> CommittedFramePreflight {
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
    if binding_valid && mount_children_valid {
        // Continue with the role-sensitive closure checks below.
    } else {
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
                    if world.get::<Visibility>(child) != Some(&Visibility::Hidden) {
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
pub(crate) mod tests {
    use super::*;
    use crate::{
        app_ext::{
            AppRouterExt, RouterConfig, RouterHandle, RouterHostRegistration, TestAppRouterExt,
        },
        content::{ContentColor, ContentPlan, FrameContentWriter, SolidRect},
        driver::{EcsViewDriver, EcsViewReportSession, ViewBuildReport},
        host::{HostCandidateClassification, HostCandidateRevisionState, classify_host_candidate},
        navigation::{
            RouterNavigation, collect_navigation_system, commit_presentation_system,
            drive_navigation_system, observe_authority_system, settle_cleanup_system,
        },
        plugin::BevyUiRouterPlugin,
        registry::QuarantineLeaseTable,
    };
    use bevy_app::{App, PostUpdate, PreUpdate};
    use bevy_camera::visibility::InheritedVisibility;
    use bevy_ecs::{
        component::Component,
        event::EntityEvent,
        lifecycle::{HookContext, Insert},
        observer::On,
        prelude::{Commands, Resource},
        query::With,
    };
    use bevy_internal::{
        DefaultPlugins,
        camera::{Camera2d, NormalizedRenderTarget},
        math::Vec2,
        picking::{
            backend::PointerHits,
            hover::HoverMap,
            pointer::{Location, PointerId, PointerLocation},
        },
        window::{PrimaryWindow, Window, WindowRef},
    };
    use bevy_picking::events::{Over, Pointer};
    use bevy_ui::{ComputedNode, UiGlobalTransform};
    use bevy_ui_router_core::{
        integration::{EngineAction, RootFrameTask, RouterEngine},
        prelude::{
            RouteLocation, RouteNodeOptions, RoutePattern, RouteSlotKey, RouteTree, RouteViewKey,
        },
    };
    use bevy_ui_router_view::{
        RouteViewDescriptor, RouteViewRegistry, ViewBuildEffect, ViewDriverKey,
    };

    #[derive(Resource, Default)]
    struct RealizationStageAudit(Vec<bool>);

    #[derive(Resource)]
    struct PersistentPickableRemoval;

    #[derive(Component)]
    struct ForeignRealizationComponent;

    #[derive(Component)]
    struct ProtectedIsolationMarker;

    struct NestedPickingDriver {
        key: ViewDriverKey,
    }

    impl EcsViewDriver for NestedPickingDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            effect: &ViewBuildEffect,
            content: &mut FrameContentWriter<'_>,
            report: EcsViewReportSession<'_>,
        ) -> ViewBuildReport {
            content
                .spawn_solid_rect(
                    SolidRect::new(24, 16, ContentColor::srgba8(1, 2, 3, 255)).unwrap(),
                )
                .unwrap();
            if effect.view() == &RouteViewKey::new("layout").unwrap() {
                let anchor = content.spawn_empty().unwrap();
                content
                    .declare_outlet(anchor, RouteSlotKey::default())
                    .unwrap();
            }
            report.applied()
        }
    }

    #[derive(Resource, Default)]
    struct NestedTargetDeliveries(std::collections::BTreeMap<Entity, usize>);

    fn install_nested_pointer_observer(app: &mut App) {
        app.world_mut().add_observer(
            |event: On<Pointer<Over>>,
             mut deliveries: bevy_ecs::prelude::ResMut<NestedTargetDeliveries>| {
                *deliveries.0.entry(event.event_target()).or_default() += 1;
            },
        );
    }

    struct ActiveNestedTopology {
        nodes: Vec<Entity>,
        parent_primitive: Entity,
        child_primitive: Entity,
    }

    fn active_nested_topology(world: &World, host: Entity) -> ActiveNestedTopology {
        let registry = world.resource::<RouteEntityRegistry>();
        let top_slot = registry
            .records()
            .iter()
            .find_map(|record| {
                matches!(
                    record.owner(),
                    RegistryOwner::Committed(EntityOwner::HostTopSlot { .. })
                )
                .then_some(record.entity())
            })
            .expect("active router must retain its top slot");
        let child_slot = registry
            .records()
            .iter()
            .find_map(|record| {
                (record.lifecycle() == EntityLifecycle::Active
                    && matches!(
                        record.owner(),
                        RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                    ))
                .then_some(record.entity())
            })
            .expect("active nested branch must retain its child slot");

        let parent_shell = world
            .iter_entities()
            .find_map(|entity| {
                (entity.get::<FrameLifecycle>() == Some(&FrameLifecycle(EntityLifecycle::Active))
                    && entity.get::<FrameShell>().is_some()
                    && entity.get::<ChildOf>().map(ChildOf::parent) == Some(top_slot))
                .then_some(entity.id())
            })
            .expect("active parent shell must be mounted under the top slot");
        let parent_frame = world.get::<FrameShell>(parent_shell).unwrap().0.clone();
        let parent_content = world
            .iter_entities()
            .find_map(|entity| {
                entity
                    .get::<FrameContentTarget>()
                    .is_some_and(|target| target.matches_frame(&parent_frame))
                    .then_some(entity.id())
            })
            .expect("active parent content target must exist");
        let outlet_anchor = world
            .get::<ChildOf>(child_slot)
            .map(ChildOf::parent)
            .expect("active child slot must retain its outlet anchor");
        let child_shell = world
            .iter_entities()
            .find_map(|entity| {
                (entity.get::<FrameLifecycle>() == Some(&FrameLifecycle(EntityLifecycle::Active))
                    && entity.get::<FrameShell>().is_some()
                    && entity.get::<ChildOf>().map(ChildOf::parent) == Some(child_slot))
                .then_some(entity.id())
            })
            .expect("active child shell must be mounted under the child slot");
        let child_frame = world.get::<FrameShell>(child_shell).unwrap().0.clone();
        let child_content = world
            .iter_entities()
            .find_map(|entity| {
                entity
                    .get::<FrameContentTarget>()
                    .is_some_and(|target| target.matches_frame(&child_frame))
                    .then_some(entity.id())
            })
            .expect("active child content target must exist");
        let primitive_for = |frame: &RouteFrameVersion| {
            world
                .iter_entities()
                .find_map(|entity| {
                    entity
                        .get::<FrameOutput>()
                        .is_some_and(|output| output.frame == *frame && output.solid_rect.is_some())
                        .then_some(entity.id())
                })
                .expect("each active nested frame must publish one primitive")
        };

        assert_eq!(
            world.get::<ChildOf>(top_slot).map(ChildOf::parent),
            Some(host)
        );
        assert_eq!(
            world.get::<ChildOf>(parent_content).map(ChildOf::parent),
            Some(parent_shell)
        );
        assert_eq!(
            world.get::<ChildOf>(outlet_anchor).map(ChildOf::parent),
            Some(parent_content)
        );
        assert_eq!(
            world.get::<ChildOf>(child_content).map(ChildOf::parent),
            Some(child_shell)
        );

        ActiveNestedTopology {
            nodes: vec![
                host,
                top_slot,
                parent_shell,
                parent_content,
                outlet_anchor,
                child_slot,
                child_shell,
                child_content,
            ],
            parent_primitive: primitive_for(&parent_frame),
            child_primitive: primitive_for(&child_frame),
        }
    }

    fn interactive_outputs(world: &World, lifecycle: EntityLifecycle) -> Vec<Entity> {
        let mut outputs = world
            .iter_entities()
            .filter_map(|entity| {
                (entity
                    .get::<FrameOutput>()
                    .is_some_and(|output| output.solid_rect.is_some())
                    && entity.get::<FrameLifecycle>() == Some(&FrameLifecycle(lifecycle)))
                .then_some(entity.id())
            })
            .collect::<Vec<_>>();
        outputs.sort_by_key(|entity| entity.to_bits());
        outputs
    }

    #[derive(Debug)]
    struct NestedDriveSample {
        prepared_outputs: usize,
        committed_slots: usize,
        provisional_slots: usize,
        pending: Option<(usize, usize, bool)>,
    }

    fn drive_nested_until_prepared(app: &mut App) {
        let mut trace = Vec::new();
        for _ in 0..8 {
            drive_navigation_system(app.world_mut());
            let registry = app.world().resource::<RouteEntityRegistry>();
            let sample = NestedDriveSample {
                prepared_outputs: interactive_outputs(app.world(), EntityLifecycle::PreparedInert)
                    .len(),
                committed_slots: registry
                    .records()
                    .iter()
                    .filter(|record| {
                        record.lifecycle() == EntityLifecycle::PreparedInert
                            && matches!(
                                record.owner(),
                                RegistryOwner::Committed(EntityOwner::ChildSlot { .. })
                            )
                    })
                    .count(),
                provisional_slots: registry
                    .records()
                    .iter()
                    .filter(|record| {
                        matches!(
                            record.owner(),
                            RegistryOwner::Provisional(
                                crate::registry::ProvisionalOwner::ChildSlot { .. }
                            )
                        )
                    })
                    .count(),
                pending: app
                    .world()
                    .resource::<RouterNavigation>()
                    .pending_shape_for_test(),
            };
            let ready = sample.pending.is_some_and(|(frames, slots, final_task)| {
                final_task
                    && sample.prepared_outputs == frames
                    && sample.committed_slots == slots
                    && sample.provisional_slots == 0
            });
            trace.push(sample);
            if ready {
                return;
            }
        }
        panic!("Nested branch did not reach the prepared commit gate: {trace:?}");
    }

    fn move_mouse_to(app: &mut App, primary_window: Entity, target: Entity) {
        let center = *app.world().get::<UiGlobalTransform>(target).unwrap() * Vec2::ZERO;
        let pointer = app
            .world_mut()
            .query::<(Entity, &PointerId)>()
            .iter(app.world())
            .find_map(|(entity, id)| (*id == PointerId::Mouse).then_some(entity))
            .expect("DefaultPlugins must install a mouse pointer");
        *app.world_mut().get_mut::<PointerLocation>(pointer).unwrap() =
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(primary_window)).unwrap(),
                ),
                position: center,
            });
        app.world_mut().run_schedule(PreUpdate);
    }

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
            &mut world, &candidate, &grant, &staged,
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
    fn guarded_settlement_stops_writes_after_protected_partition_drift() {
        let (mut world, _host, _top_slot, frame) = realized_empty_fixture();
        let target = frame.shell();
        let protected = world.spawn(ProtectedIsolationMarker).id();
        world.add_observer(
            move |event: On<Insert, Visibility>, mut commands: Commands| {
                if event.entity == target {
                    commands
                        .entity(protected)
                        .remove::<ProtectedIsolationMarker>();
                }
            },
        );
        world.entity_mut(target).remove::<Visibility>();
        world.entity_mut(target).remove::<Pickable>();
        world.entity_mut(target).remove::<RouterInputBarrier>();

        let isolated = isolate_frame_for_settlement_guarded(
            &mut world,
            &frame,
            &|claim| claim.entity() == target,
            &|world| world.get::<ProtectedIsolationMarker>(protected).is_some(),
        );

        assert!(!isolated);
        assert!(world.get::<ProtectedIsolationMarker>(protected).is_none());
        assert_eq!(world.get::<Visibility>(target), Some(&Visibility::Hidden));
        assert!(world.get::<Pickable>(target).is_none());
        assert!(world.get::<RouterInputBarrier>(target).is_none());
    }

    fn activate_realized_frame_for_committed_preflight(
        world: &mut World,
        frame: &mut RealizedFrame,
    ) {
        let transitions = frame.lifecycle_transitions(EntityLifecycle::Active);
        let registry_mutation = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .expect("the exact realized-frame claims must prepare one active transition");
        let resulting_claims = registry_mutation.resulting_claims().to_vec();
        assert!(frame.claims_can_rebind(
            &resulting_claims,
            &resulting_claims,
            EntityLifecycle::PreparedInert,
            EntityLifecycle::Active,
        ));
        let presentation = prepare_presentation_mutation(
            world,
            &frame.mutation_targets(
                EntityLifecycle::PreparedInert,
                EntityLifecycle::Active,
                true,
            ),
        )
        .expect("the realized frame must prepare one exact presentation transition");

        presentation.apply();
        registry_mutation.apply(&mut world.resource_mut::<RouteEntityRegistry>());
        frame.apply_claim_rebind(&resulting_claims, &resulting_claims);
        assert!(validate_active_frame(world, frame));
    }

    fn insert_single_foreign_shell_component(
        world: &mut World,
        frame: &RealizedFrame,
        expected_lifecycle: EntityLifecycle,
    ) {
        let shell = frame.shell();
        let realized_shell = frame
            .entities
            .iter()
            .find(|realized| realized.entity == shell)
            .expect("the realized frame must retain its shell fact");
        let expected_claim = realized_shell.claim.clone();
        let expected_parent = realized_shell.parent;
        let expected_manifest = world
            .resource::<RouteEntityRegistry>()
            .record(shell)
            .expect("the realized shell must remain registered")
            .manifest()
            .clone();

        world.entity_mut(shell).insert(ForeignRealizationComponent);

        let actual_manifest = ComponentManifest::from_world(world, shell)
            .expect("the realized shell must remain observable");
        let foreign_component = world
            .components()
            .component_id::<ForeignRealizationComponent>()
            .expect("insertion must register the test-private foreign component");
        let additions = actual_manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| !expected_manifest.component_ids().contains(component))
            .collect::<Vec<_>>();
        let record = world
            .resource::<RouteEntityRegistry>()
            .record(shell)
            .expect("the realized shell must retain its registry record");

        assert_eq!(additions, vec![foreign_component]);
        assert_eq!(record.claim(), expected_claim);
        assert_eq!(record.lifecycle(), expected_lifecycle);
        assert_eq!(record.manifest(), &expected_manifest);
        assert_eq!(
            world.get::<RouteOwnershipSeal>(shell),
            Some(&RouteOwnershipSeal::new(expected_claim.seal()))
        );
        assert_eq!(
            world.get::<FrameLifecycle>(shell),
            Some(&FrameLifecycle(expected_lifecycle))
        );
        assert_eq!(
            world.get::<ChildOf>(shell).map(ChildOf::parent),
            Some(expected_parent)
        );
        assert!(
            expected_manifest
                .component_ids()
                .iter()
                .all(|component| actual_manifest.component_ids().contains(component)),
            "the foreign addition must not remove any registry-required shell component"
        );
    }

    #[test]
    fn committed_current_shell_foreign_component_is_preserved_ready() {
        let (mut world, _host, _top_slot, mut frame) = realized_empty_fixture();
        activate_realized_frame_for_committed_preflight(&mut world, &mut frame);
        insert_single_foreign_shell_component(&mut world, &frame, EntityLifecycle::Active);

        assert!(
            matches!(
                classify_committed_frame(&world, &frame, CommittedFrameRole::Preserved, &[], &[],),
                CommittedFramePreflight::Ready
            ),
            "RED: a preserved committed shell with required manifest coverage and one protected foreign component must remain Ready"
        );
    }

    #[test]
    fn committed_current_shell_foreign_component_requires_leaving_quarantine() {
        let (mut world, _host, _top_slot, mut frame) = realized_empty_fixture();
        activate_realized_frame_for_committed_preflight(&mut world, &mut frame);
        insert_single_foreign_shell_component(&mut world, &frame, EntityLifecycle::Active);

        assert!(
            matches!(
                classify_committed_frame(&world, &frame, CommittedFrameRole::Leaving, &[], &[],),
                CommittedFramePreflight::ReadyWithQuarantine
            ),
            "RED: a leaving committed shell with one protected foreign component must be ReadyWithQuarantine"
        );
    }

    #[test]
    fn committed_current_shell_missing_required_component_is_invalid() {
        let (mut world, _host, _top_slot, mut frame) = realized_empty_fixture();
        activate_realized_frame_for_committed_preflight(&mut world, &mut frame);
        let shell = frame.shell();
        let pickable = world
            .components()
            .component_id::<Pickable>()
            .expect("the committed shell manifest must register Pickable");
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(shell)
                .expect("the committed shell must remain registered")
                .manifest()
                .component_ids()
                .contains(&pickable)
        );

        world.entity_mut(shell).remove::<Pickable>();

        assert!(world.get::<Pickable>(shell).is_none());
        assert!(matches!(
            classify_committed_frame(&world, &frame, CommittedFrameRole::Preserved, &[], &[],),
            CommittedFramePreflight::Invalid
        ));
        assert!(matches!(
            classify_committed_frame(&world, &frame, CommittedFrameRole::Leaving, &[], &[],),
            CommittedFramePreflight::Invalid
        ));
    }

    #[test]
    fn prepared_current_shell_foreign_component_remains_invalid() {
        let (mut world, _host, _top_slot, frame) = realized_empty_fixture();
        insert_single_foreign_shell_component(&mut world, &frame, EntityLifecycle::PreparedInert);

        assert!(
            !validate_realized_frame(&world, &frame),
            "prepared and entering shell validation must retain strict manifest equality"
        );
    }

    fn presentation_commit_fixture(
        world: &mut World,
    ) -> (Entity, Entity, Entity, Vec<FrameMutationTarget>) {
        let leaving = world
            .spawn((
                Visibility::Visible,
                ACTIVE_PICKABLE,
                RouterInputBarrier::allow_all(),
                FrameLifecycle(EntityLifecycle::Active),
            ))
            .id();
        let entering = world
            .spawn((
                Visibility::Hidden,
                Pickable::IGNORE,
                RouterInputBarrier::deny_all(),
                FrameLifecycle(EntityLifecycle::PreparedInert),
            ))
            .id();
        let child_slot = world
            .spawn((
                Visibility::Hidden,
                Pickable::IGNORE,
                RouterInputBarrier::deny_all(),
                ChildSlotLifecycle::RegisteredInert,
            ))
            .id();
        let targets = vec![
            FrameMutationTarget {
                entity: leaving,
                current: EntityLifecycle::Active,
                next: EntityLifecycle::Leaving,
                active_after: false,
                interactive: true,
                kind: PresentationTargetKind::Frame,
            },
            FrameMutationTarget {
                entity: entering,
                current: EntityLifecycle::PreparedInert,
                next: EntityLifecycle::Active,
                active_after: true,
                interactive: true,
                kind: PresentationTargetKind::Frame,
            },
            FrameMutationTarget {
                entity: child_slot,
                current: EntityLifecycle::PreparedInert,
                next: EntityLifecycle::Active,
                active_after: true,
                interactive: false,
                kind: PresentationTargetKind::ChildSlot,
            },
        ];
        (leaving, entering, child_slot, targets)
    }

    #[test]
    fn staged_commit_exposes_each_infallible_closure_boundary() {
        #[derive(Debug, Eq, PartialEq)]
        enum ClosureBoundary {
            Focus,
            Registry,
            Gate,
        }

        let mut world = World::new();
        let (leaving, entering, child_slot, targets) = presentation_commit_fixture(&mut world);
        let prepared = prepare_presentation_mutation(&mut world, &targets).unwrap();
        let commit = prepared.seal_commit(1).unwrap();
        let trace = std::cell::RefCell::new(Vec::new());

        commit.apply_staged(
            |state| {
                trace.borrow_mut().push((
                    ClosureBoundary::Focus,
                    state.leaving_is_isolated(),
                    state.entering_is_prepared_inert(),
                    state.entering_base_is_active(),
                    state.entering_capabilities_are_active(),
                ));
            },
            |state| {
                trace.borrow_mut().push((
                    ClosureBoundary::Registry,
                    state.leaving_is_isolated(),
                    state.entering_is_prepared_inert(),
                    state.entering_base_is_active(),
                    state.entering_capabilities_are_active(),
                ));
            },
            |state| {
                trace.borrow_mut().push((
                    ClosureBoundary::Gate,
                    state.leaving_is_isolated(),
                    state.entering_is_prepared_inert(),
                    state.entering_base_is_active(),
                    state.entering_capabilities_are_active(),
                ));
            },
        );

        assert_eq!(
            trace.into_inner(),
            vec![
                (ClosureBoundary::Focus, true, true, false, false),
                (ClosureBoundary::Registry, true, false, true, false),
                (ClosureBoundary::Gate, true, false, true, true),
            ]
        );
        assert_eq!(world.get::<Visibility>(leaving), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Pickable>(leaving), Some(&Pickable::IGNORE));
        assert!(
            world
                .get::<RouterInputBarrier>(leaving)
                .is_some_and(RouterInputBarrier::denies_all)
        );
        assert_eq!(
            world.get::<FrameLifecycle>(leaving),
            Some(&FrameLifecycle(EntityLifecycle::Leaving))
        );
        assert_eq!(
            world.get::<Visibility>(entering),
            Some(&Visibility::Visible)
        );
        assert_eq!(world.get::<Pickable>(entering), Some(&ACTIVE_PICKABLE));
        assert!(
            world
                .get::<RouterInputBarrier>(entering)
                .is_some_and(RouterInputBarrier::allows_all)
        );
        assert_eq!(
            world.get::<FrameLifecycle>(entering),
            Some(&FrameLifecycle(EntityLifecycle::Active))
        );
        assert_eq!(
            world.get::<ChildSlotLifecycle>(child_slot),
            Some(&ChildSlotLifecycle::Active)
        );
        assert_eq!(
            world.get::<Visibility>(child_slot),
            Some(&Visibility::Visible)
        );
        assert_eq!(world.get::<Pickable>(child_slot), Some(&Pickable::IGNORE));
        assert!(
            world
                .get::<RouterInputBarrier>(child_slot)
                .is_some_and(RouterInputBarrier::denies_all)
        );
    }

    #[test]
    fn pointer_preparation_keeps_disabled_presentation_rejected() {
        let mut world = World::new();
        let (leaving, entering, _, targets) = presentation_commit_fixture(&mut world);
        crate::input::tests::seed_pointer_history(&mut world, &[leaving]);
        let before = world.resource::<PointerState>().clone();
        world
            .entity_mut(entering)
            .insert(bevy_ecs::entity_disabling::Disabled);
        assert!(
            prepare_presentation_with_pointer_revocation(&mut world, &targets, &[leaving])
                .is_none()
        );
        crate::input::tests::assert_pointer_target_preserved(&world, leaving, &before);
        assert_eq!(world.get::<Visibility>(entering), Some(&Visibility::Hidden));
    }

    #[test]
    fn commit_split_is_rejected_before_any_mutation() {
        let mut world = World::new();
        let (leaving, entering, child_slot, targets) = presentation_commit_fixture(&mut world);
        let prepared = prepare_presentation_mutation(&mut world, &targets).unwrap();

        assert!(prepared.seal_commit(0).is_none());
        assert_eq!(world.get::<Visibility>(leaving), Some(&Visibility::Visible));
        assert_eq!(world.get::<Visibility>(entering), Some(&Visibility::Hidden));
        assert_eq!(
            world.get::<Visibility>(child_slot),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn reconciliation_nested_default_plugins_reaches_layout_hover_and_target_delivery() {
        let mut app = App::new();
        app.add_plugins(DefaultPlugins);
        app.add_plugins(BevyUiRouterPlugin::default());
        app.world_mut()
            .insert_resource(NestedTargetDeliveries::default());
        install_nested_pointer_observer(&mut app);

        let primary_windows = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        assert_eq!(primary_windows.len(), 1);
        let primary_window = primary_windows[0];
        app.world_mut()
            .get_mut::<Window>(primary_window)
            .unwrap()
            .resolution
            .set(320.0, 180.0);
        app.world_mut().spawn(Camera2d);

        let driver = ViewDriverKey::new("nested-picking-driver").unwrap();
        app.register_ui_view_driver(NestedPickingDriver {
            key: driver.clone(),
        })
        .unwrap();
        let layout = RouteViewKey::new("layout").unwrap();
        let item = RouteViewKey::new("item").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let mut views = RouteViewRegistry::new();
        for view in [&layout, &item, &fallback] {
            views
                .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
                .unwrap();
        }
        let router = app
            .add_ui_router(RouterConfig::new(
                bevy_ui_router_core::test_support::nested_items_view_route_tree(),
                views,
                fallback,
            ))
            .unwrap();
        let host = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..Node::default()
                },
                RouterHostRegistration::new(router),
            ))
            .id();

        app.update();
        observe_authority_system(app.world_mut());
        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_nested_until_prepared(&mut app);
        assert_eq!(
            interactive_outputs(app.world(), EntityLifecycle::PreparedInert).len(),
            2
        );
        commit_presentation_system(app.world_mut());
        assert!(
            app.world().resource::<RouterNavigation>().entries[0]
                .cleanup_ledger
                .is_some(),
            "suffix replacement must reserve one leaving cleanup ledger"
        );
        settle_cleanup_system(app.world_mut());
        app.world_mut().run_schedule(PostUpdate);

        let topology = active_nested_topology(app.world(), host);
        assert_eq!(
            app.world()
                .get::<ChildOf>(topology.parent_primitive)
                .map(ChildOf::parent),
            Some(topology.nodes[3])
        );
        assert_eq!(
            app.world()
                .get::<ChildOf>(topology.child_primitive)
                .map(ChildOf::parent),
            Some(topology.nodes[7])
        );
        for entity in topology
            .nodes
            .iter()
            .copied()
            .chain([topology.parent_primitive, topology.child_primitive])
        {
            assert!(app.world().get::<Node>(entity).is_some());
            assert!(
                app.world().get::<ComputedNode>(entity).is_some(),
                "nested Node {entity:?} is absent from the real layout pipeline"
            );
            assert!(
                app.world()
                    .get::<InheritedVisibility>(entity)
                    .is_some_and(|visibility| visibility.get()),
                "nested Node {entity:?} is not effectively visible"
            );
        }
        for primitive in [topology.parent_primitive, topology.child_primitive] {
            assert!(
                app.world()
                    .get::<ComputedNode>(primitive)
                    .is_some_and(|computed| computed.size().cmpgt(Vec2::ZERO).all()),
                "nested primitive {primitive:?} retained zero layout"
            );
        }

        move_mouse_to(&mut app, primary_window, topology.child_primitive);
        let active_before = interactive_outputs(app.world(), EntityLifecycle::Active);
        assert_eq!(active_before.len(), 2);
        let hovered_before = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(hovered_before.contains_key(&topology.child_primitive));
        assert!(hovered_before.keys().all(|entity| {
            app.world().get::<FrameOutput>(*entity).is_none() || active_before.contains(entity)
        }));
        assert!(
            app.world()
                .resource::<bevy_ecs::message::Messages<PointerHits>>()
                .iter_current_update_messages()
                .flat_map(|hits| hits.picks.iter().map(|(entity, _)| entity))
                .any(|entity| active_before.contains(entity)),
            "locked UI backend did not produce a router-owned PointerHits candidate"
        );
        assert_eq!(
            app.world()
                .resource::<NestedTargetDeliveries>()
                .0
                .get(&topology.child_primitive),
            Some(&1)
        );

        app.world_mut()
            .resource_mut::<RouterNavigation>()
            .push(
                router,
                RouteLocation::new("/items/2").with_fragment("pending"),
            )
            .unwrap();
        collect_navigation_system(app.world_mut());
        drive_nested_until_prepared(&mut app);
        let pending = interactive_outputs(app.world(), EntityLifecycle::PreparedInert);
        assert_eq!(pending.len(), 1);
        app.world_mut().run_schedule(PostUpdate);
        move_mouse_to(&mut app, primary_window, topology.child_primitive);
        let pending_hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(pending_hover.keys().all(|entity| !pending.contains(entity)));
        assert!(pending.iter().all(|entity| {
            app.world()
                .resource::<NestedTargetDeliveries>()
                .0
                .get(entity)
                .is_none()
        }));

        let old_deliveries = active_before
            .iter()
            .map(|entity| {
                (
                    *entity,
                    app.world()
                        .resource::<NestedTargetDeliveries>()
                        .0
                        .get(entity)
                        .copied()
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        commit_presentation_system(app.world_mut());
        assert_eq!(
            interactive_outputs(app.world(), EntityLifecycle::Leaving),
            vec![topology.child_primitive]
        );
        let mut expected_active = vec![topology.parent_primitive];
        expected_active.extend(&pending);
        expected_active.sort_by_key(|entity| entity.to_bits());
        assert_eq!(
            interactive_outputs(app.world(), EntityLifecycle::Active),
            expected_active
        );
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut().run_schedule(PreUpdate);
        let committed_hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(committed_hover.keys().all(|entity| {
            app.world().get::<FrameOutput>(*entity).is_none() || expected_active.contains(entity)
        }));
        for (entity, deliveries) in &old_deliveries {
            assert_eq!(
                app.world()
                    .resource::<NestedTargetDeliveries>()
                    .0
                    .get(entity)
                    .copied()
                    .unwrap_or_default(),
                *deliveries,
                "leaving content received a final target event"
            );
        }

        settle_cleanup_system(app.world_mut());
        app.world_mut().run_schedule(PostUpdate);
        let replacement = active_nested_topology(app.world(), host);
        move_mouse_to(&mut app, primary_window, replacement.child_primitive);
        let final_hover = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .cloned()
            .unwrap_or_default();
        assert!(
            app.world()
                .resource::<crate::input::RouterInputGate>()
                .allows_pointer(replacement.child_primitive),
            "committed replacement must retain its exact pointer capability"
        );
        assert!(final_hover.contains_key(&replacement.child_primitive));
        assert!(final_hover.keys().all(|entity| {
            app.world().get::<FrameOutput>(*entity).is_none() || expected_active.contains(entity)
        }));
        assert_eq!(
            app.world()
                .resource::<NestedTargetDeliveries>()
                .0
                .get(&replacement.child_primitive),
            Some(&1)
        );
        for (entity, deliveries) in old_deliveries {
            assert_eq!(
                app.world()
                    .resource::<NestedTargetDeliveries>()
                    .0
                    .get(&entity)
                    .copied()
                    .unwrap_or_default(),
                deliveries
            );
            if entity == topology.parent_primitive {
                assert!(app.world().get_entity(entity).is_ok());
            } else {
                assert_eq!(entity, topology.child_primitive);
                assert!(app.world().get_entity(entity).is_err());
            }
        }
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Camera2d>>()
                .iter(app.world())
                .count(),
            1
        );
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
                anchor_binding: OutletBinding::RegisteredOutput(first_claim),
                expected_anchor_children: Vec::new(),
                staging_lifecycle: EntityLifecycle::PreparedInert,
                allowed_existing_shells: Vec::new(),
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
    fn invalid_outlet_alias_rebind_is_atomic() {
        let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        let mut content = ContentPlan::new();
        let anchor = FrameContentWriter::new(&mut content).spawn_empty().unwrap();
        FrameContentWriter::new(&mut content)
            .declare_outlet(anchor, RouteSlotKey::default())
            .unwrap();
        let mut frame = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap();
        let current_anchor = frame.outlets[0].anchor_claim().clone();
        let anchor_manifest = world
            .resource::<RouteEntityRegistry>()
            .record(current_anchor.entity())
            .unwrap()
            .manifest()
            .clone();
        let mut foreign_registry = RouteEntityRegistry::new();
        *frame.outlets[0].anchor_binding.source_claim_mut() = foreign_registry
            .register(
                current_anchor.entity(),
                current_anchor.owner().clone(),
                anchor_manifest,
                current_anchor.lifecycle(),
            )
            .unwrap();

        let transitions = frame.lifecycle_transitions(EntityLifecycle::Active);
        let resulting = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec();
        let before = frame.claims().cloned().collect::<Vec<_>>();
        let branch = crate::branch_transaction::RealizedBranch::from_single_frame(frame);

        assert!(
            branch
                .prepare_claim_rebind(
                    &resulting,
                    EntityLifecycle::PreparedInert,
                    EntityLifecycle::Active,
                )
                .is_none()
        );
        assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn missing_outlet_alias_rebind_is_atomic() {
        let (mut world, _host, candidate, top_slot, top_slot_claim, task) = pending_frame_fixture();
        let mut content = ContentPlan::new();
        let anchor = FrameContentWriter::new(&mut content).spawn_empty().unwrap();
        FrameContentWriter::new(&mut content)
            .declare_outlet(anchor, RouteSlotKey::default())
            .unwrap();
        let mut frame = realize_frame(
            &mut world,
            candidate,
            top_slot,
            top_slot_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            content.accept(),
        )
        .unwrap();
        let current_anchor = frame.outlets[0].anchor_claim().clone();
        let foreign_entity = world.spawn_empty().id();
        let anchor_manifest = world
            .resource::<RouteEntityRegistry>()
            .record(current_anchor.entity())
            .unwrap()
            .manifest()
            .clone();
        let mut foreign_registry = RouteEntityRegistry::new();
        *frame.outlets[0].anchor_binding.source_claim_mut() = foreign_registry
            .register(
                foreign_entity,
                current_anchor.owner().clone(),
                anchor_manifest,
                current_anchor.lifecycle(),
            )
            .unwrap();

        let transitions = frame.lifecycle_transitions(EntityLifecycle::Active);
        let resulting = world
            .resource::<RouteEntityRegistry>()
            .prepare_lifecycle_mutation(&transitions)
            .unwrap()
            .resulting_claims()
            .to_vec();
        let before = frame.claims().cloned().collect::<Vec<_>>();
        let branch = crate::branch_transaction::RealizedBranch::from_single_frame(frame);

        assert!(
            branch
                .prepare_claim_rebind(
                    &resulting,
                    EntityLifecycle::PreparedInert,
                    EntityLifecycle::Active,
                )
                .is_none()
        );
        assert_eq!(branch.claims().cloned().collect::<Vec<_>>(), before);
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
            registry
                .register(
                    top_slot,
                    owner,
                    manifest.clone(),
                    EntityLifecycle::PreparedInert,
                )
                .unwrap()
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
            &mut world, &candidate, &grant, &staged,
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
            &mut world, &candidate, &grant, &staged,
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
            registry
                .register(
                    blocker,
                    RegistryOwner::Committed(EntityOwner::FrameContentTarget(task.frame().clone())),
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
        assert!(world.get_entity(residual.shell()).is_ok());
        assert!(world.get::<ChildOf>(residual.shell()).is_none());

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
            &mut world, &candidate, &grant, &staged,
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
