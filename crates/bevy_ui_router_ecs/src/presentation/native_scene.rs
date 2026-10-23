//! Creation-time ownership for the native BSN Scene realization path.
//!
//! This unit deliberately stops before navigation publication. It consumes one
//! prepared Scene in a single exclusive call, registers every declaration before
//! application code can write it, and returns either an adopted containment
//! snapshot or exact residual evidence.

use std::{any::type_name, collections::HashSet};

use bevy_asset::UntypedHandle;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    bundle::BundleScratch,
    entity::{Entity, EntityHashMap},
    hierarchy::{ChildOf, Children},
    template::{SceneEntityReference, Template, TemplateContext},
    world::World,
};
use bevy_picking::Pickable;
use bevy_scene::{ApplySceneError, ResolvedScene, ResolvedSceneRoot};
use bevy_ui::{Display, Node, Val};
use bevy_ui_router_core::frame::RouteFrameVersion;

use super::{FrameContentTarget, FrameLifecycle, FrameShell, RealizationError};
use crate::{
    bsn::PreparedBsnScene,
    components::{RouteOwnershipSeal, RouterInputBarrier},
    registry::{
        ComponentManifest, EntityLifecycle, EntityOwner, ExactRegistryClaim, QuarantineLeaseTable,
        RegistryOwner, RegistryWriteError, RouteEntityRegistry,
        apply_prepared_exact_registry_retirement, prepare_exact_registry_retirement,
    },
};

#[derive(Debug)]
pub(crate) struct NativeSceneAdoption {
    frame: RouteFrameVersion,
    shell: ExactRegistryClaim,
    root: ExactRegistryClaim,
    /// A validation snapshot only. Dynamic input and cleanup must rebuild this
    /// closure from the live root rather than treating the vector as ownership.
    current_containment: Vec<Entity>,
    dependencies: Vec<UntypedHandle>,
}

impl NativeSceneAdoption {
    pub(crate) const fn frame(&self) -> &RouteFrameVersion {
        &self.frame
    }

    pub(crate) const fn shell_claim(&self) -> &ExactRegistryClaim {
        &self.shell
    }

    pub(crate) const fn root_claim(&self) -> &ExactRegistryClaim {
        &self.root
    }

    pub(crate) fn current_containment(&self) -> &[Entity] {
        &self.current_containment
    }

    pub(crate) fn into_dependencies(self) -> Vec<UntypedHandle> {
        self.dependencies
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        RouteFrameVersion,
        ExactRegistryClaim,
        ExactRegistryClaim,
        Vec<UntypedHandle>,
    ) {
        (self.frame, self.shell, self.root, self.dependencies)
    }
}

/// Distinguishes Scene-authored roots from the data driver's closed manifests.
#[derive(bevy_ecs::component::Component)]
pub(crate) struct NativeContentRoot;

#[derive(bevy_ecs::component::Component)]
pub(crate) struct NativeFrameShell;

#[derive(Debug)]
pub(crate) struct NativeSceneCreationFailure {
    error: NativeSceneCreationError,
    settlement: NativeSceneSettlement,
    dependencies: Vec<UntypedHandle>,
    focus_error: Option<crate::native_focus::NativeFocusError>,
}

impl NativeSceneCreationFailure {
    pub(crate) fn focus_entities(&self) -> Vec<Entity> {
        self.settlement
            .removed
            .iter()
            .map(|allocation| allocation.claim.entity())
            .chain(
                self.settlement
                    .residual
                    .iter()
                    .flat_map(|residual| residual.claims().map(ExactRegistryClaim::entity)),
            )
            .collect()
    }
    pub(crate) fn focus_error(&self) -> Option<crate::native_focus::NativeFocusError> {
        self.focus_error
    }
    pub(crate) fn set_focus_error(&mut self, error: crate::native_focus::NativeFocusError) {
        self.focus_error = Some(error);
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        NativeSceneCreationError,
        NativeSceneSettlement,
        Vec<UntypedHandle>,
    ) {
        (self.error, self.settlement, self.dependencies)
    }
}

#[derive(Debug)]
pub(crate) enum NativeSceneCreationError {
    Mount(RealizationError),
    Capacity,
    Registry(RegistryWriteError),
    PrivateIdentityDrift,
    Apply(ApplySceneError),
    Adoption(NativeSceneAdoptionError),
}

impl std::fmt::Display for NativeSceneCreationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mount(error) => write!(formatter, "BSN mount validation failed: {error:?}"),
            Self::Apply(error) => write!(formatter, "BSN scene apply failed: {error}"),
            Self::Capacity => formatter.write_str("BSN creation capacity unavailable"),
            Self::Registry(error) => {
                write!(formatter, "BSN allocation registration failed: {error:?}")
            }
            Self::PrivateIdentityDrift => {
                formatter.write_str("BSN creation identity changed during application")
            }
            Self::Adoption(error) => write!(formatter, "BSN content adoption failed: {error:?}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSceneAdoptionError {
    AllocationDrift { declaration: Option<usize> },
    RelationshipDrift { declaration: Option<usize> },
    UnexpectedChild { parent: Entity, child: Entity },
    ManifestDrift,
    RetirementDrift,
}

#[derive(Debug)]
pub(crate) struct NativeSceneSettlement {
    removed: Vec<SettledNativeSceneAllocation>,
    residual: Option<NativeSceneResidual>,
}

impl NativeSceneSettlement {
    pub(crate) fn into_residual(self) -> Option<NativeSceneResidual> {
        self.residual
    }
    pub(crate) fn removed(&self) -> &[SettledNativeSceneAllocation] {
        &self.removed
    }

    pub(crate) const fn residual(&self) -> Option<&NativeSceneResidual> {
        self.residual.as_ref()
    }

    pub(crate) const fn is_clean(&self) -> bool {
        self.residual.is_none()
    }
}

#[derive(Debug)]
pub(crate) struct SettledNativeSceneAllocation {
    role: NativeSceneAllocationRole,
    claim: ExactRegistryClaim,
    actual_parent: Option<Entity>,
    observed_manifest: Option<ComponentManifest>,
}

#[derive(Debug)]
pub(crate) struct NativeSceneResidual {
    isolation: NativeSceneResidualIsolation,
    allocations: Vec<NativeSceneResidualAllocation>,
    protected_entities: Vec<Entity>,
}

impl NativeSceneResidual {
    pub(crate) fn claims(&self) -> impl Iterator<Item = &ExactRegistryClaim> {
        self.allocations.iter().map(|allocation| &allocation.claim)
    }

    /// Re-observe the original allocations; a historical disposition is not a
    /// proof that a later continuation may release their physical owner.
    pub(crate) fn validate_inert(&self, world: &World) -> Result<(), NativeSceneIsolationFailure> {
        let NativeSceneResidualIsolation::InertUnderShell { shell } = self.isolation else {
            return Err(match self.isolation {
                NativeSceneResidualIsolation::Unisolated(reason) => reason,
                _ => unreachable!(),
            });
        };
        if !shell_is_inert(world, shell) {
            return Err(NativeSceneIsolationFailure::ShellNotInert);
        }
        for allocation in &self.allocations {
            if !allocation.residual_identity_is_exact(world)
                || current_parent(world, allocation.claim.entity()) != allocation.actual_parent
            {
                return Err(NativeSceneIsolationFailure::AllocationAuthorityDrift);
            }
            let mut current = allocation.claim.entity();
            let mut seen = HashSet::new();
            while current != shell {
                if !seen.insert(current) {
                    return Err(NativeSceneIsolationFailure::RelationshipCycle);
                }
                let Some(parent) = current_parent(world, current) else {
                    return Err(NativeSceneIsolationFailure::RelationshipDrift);
                };
                if !current_children(world, parent).contains(&current)
                    || (!self
                        .allocations
                        .iter()
                        .any(|allocation| allocation.claim.entity() == parent)
                        && !self.protected_entities.contains(&parent))
                {
                    return Err(NativeSceneIsolationFailure::RelationshipDrift);
                }
                current = parent;
            }
        }
        if !self.allocations.iter().any(|allocation| {
            allocation.claim.entity() == shell
                && allocation.role == NativeSceneAllocationRole::Shell
        }) {
            return Err(NativeSceneIsolationFailure::AllocationAuthorityDrift);
        }
        Ok(())
    }

    pub(crate) fn claim_identity_is_exact(
        &self,
        world: &World,
        claim: &ExactRegistryClaim,
    ) -> bool {
        self.allocations.iter().any(|allocation| {
            &allocation.claim == claim && allocation.residual_identity_is_exact(world)
        })
    }
    pub(crate) const fn isolation(&self) -> NativeSceneResidualIsolation {
        self.isolation
    }

    pub(crate) fn allocations(&self) -> &[NativeSceneResidualAllocation] {
        &self.allocations
    }

    pub(crate) fn protected_entities(&self) -> &[Entity] {
        &self.protected_entities
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSceneResidualIsolation {
    InertUnderShell { shell: Entity },
    Unisolated(NativeSceneIsolationFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSceneIsolationFailure {
    Focus(crate::native_focus::NativeFocusError),
    Capacity,
    AllocationAuthorityDrift,
    ProtectedClosureDrift,
    RelationshipDrift,
    RelationshipCycle,
    ShellNotInert,
    RegistryRetirementDrift,
}

#[derive(Debug)]
pub(crate) struct NativeSceneResidualAllocation {
    role: NativeSceneAllocationRole,
    claim: ExactRegistryClaim,
    actual_parent: Option<Entity>,
    observed_manifest: Option<ComponentManifest>,
    temporary_markers_released: bool,
}

impl NativeSceneResidualAllocation {
    fn residual_identity_is_exact(&self, world: &World) -> bool {
        let Some(manifest) = self.observed_manifest.as_ref() else {
            return false;
        };
        allocation_identity_fields_are_exact(
            world,
            self.role,
            &self.claim,
            self.temporary_markers_released,
        ) && ComponentManifest::from_world(world, self.claim.entity()).as_ref() == Some(manifest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeSceneAllocationRole {
    Shell,
    Declaration(usize),
}

#[derive(Debug)]
struct NativeSceneAllocation {
    role: NativeSceneAllocationRole,
    claim: ExactRegistryClaim,
    manifest: ComponentManifest,
    /// Descendant marker removal is an intended intermediate state while the
    /// exact registry claim is still held by this single creation attempt.
    temporary_markers_released: bool,
}

impl NativeSceneAllocation {
    const fn entity(&self) -> Entity {
        self.claim.entity()
    }
}

struct ReferenceSeedTemplate {
    bindings: Vec<(SceneEntityReference, Entity)>,
}

impl Template for ReferenceSeedTemplate {
    type Output = ();

    fn build_template(&self, context: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<()> {
        for (reference, entity) in self.bindings.iter().copied() {
            context.entity_references.set(reference, entity);
        }
        Ok(())
    }

    fn clone_template(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
        }
    }
}

pub(crate) fn create_native_scene(
    world: &mut World,
    frame: RouteFrameVersion,
    prepared: PreparedBsnScene,
) -> Result<NativeSceneAdoption, NativeSceneCreationFailure> {
    let PreparedBsnScene {
        attempt,
        root,
        declarations,
        dependencies,
    } = prepared;

    let mut owners = Vec::new();
    if owners.try_reserve_exact(declarations.len() + 1).is_err() {
        return Err(failure_without_allocations(
            NativeSceneCreationError::Capacity,
            dependencies,
        ));
    }
    owners.push(RegistryOwner::Committed(EntityOwner::FrameShell(
        frame.clone(),
    )));
    for declaration in 0..declarations.len() {
        owners.push(if declaration == 0 {
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame.clone()))
        } else {
            RegistryOwner::Committed(EntityOwner::FrameOutput {
                frame: frame.clone(),
                output: attempt,
                local_index: declaration,
            })
        });
    }

    if let Err(error) = world
        .resource_mut::<RouteEntityRegistry>()
        .try_reserve_registrations(&owners)
    {
        return Err(failure_without_allocations(
            NativeSceneCreationError::Registry(error),
            dependencies,
        ));
    }

    let mut allocations = Vec::new();
    if allocations.try_reserve_exact(owners.len()).is_err() {
        return Err(failure_without_allocations(
            NativeSceneCreationError::Capacity,
            dependencies,
        ));
    }
    for (index, owner) in owners.into_iter().enumerate() {
        let entity = world.spawn_empty().id();
        let empty = ComponentManifest::new([]);
        let registration = world.resource_mut::<RouteEntityRegistry>().register(
            entity,
            owner,
            empty.clone(),
            EntityLifecycle::PreparedInert,
        );
        let claim = match registration {
            Ok(claim) => claim,
            Err(error) => {
                world.despawn(entity);
                return Err(fail_and_settle(
                    world,
                    NativeSceneCreationError::Registry(error),
                    allocations,
                    dependencies,
                ));
            }
        };
        allocations.push(NativeSceneAllocation {
            role: if index == 0 {
                NativeSceneAllocationRole::Shell
            } else {
                NativeSceneAllocationRole::Declaration(index - 1)
            },
            claim,
            manifest: empty,
            temporary_markers_released: false,
        });
    }

    install_private_identity(world, &frame, &allocations);
    world.flush();
    if !synchronize_allocation_manifests(world, &mut allocations)
        || !allocations
            .iter()
            .all(|allocation| allocation_has_exact_private_identity(world, allocation))
    {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::PrivateIdentityDrift,
            allocations,
            dependencies,
        ));
    }

    let mut bindings = Vec::new();
    let reference_count = declarations.iter().try_fold(0usize, |count, declaration| {
        count.checked_add(declaration.references.len())
    });
    let Some(reference_count) = reference_count else {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Capacity,
            allocations,
            dependencies,
        ));
    };
    if bindings.try_reserve_exact(reference_count).is_err() {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Capacity,
            allocations,
            dependencies,
        ));
    }
    for (declaration, allocation) in declarations.iter().zip(&allocations[1..]) {
        bindings.extend(
            declaration
                .references
                .iter()
                .copied()
                .map(|reference| (reference, allocation.entity())),
        );
    }

    let topology = match PreparedNativeSceneTopology::new(&allocations, &declarations) {
        Ok(topology) => topology,
        Err(()) => {
            return Err(fail_and_settle(
                world,
                NativeSceneCreationError::Capacity,
                allocations,
                dependencies,
            ));
        }
    };

    let shell = allocations[0].entity();
    let mut wrapper = ResolvedScene::default();
    wrapper.push_bundle_template(ReferenceSeedTemplate { bindings });
    wrapper
        .get_or_insert_related_resolved_scenes::<ChildOf>()
        .scenes
        .push(root.scene);
    let wrapper = ResolvedSceneRoot { scene: wrapper };
    let apply_result = {
        let mut shell_entity = world.entity_mut(shell);
        wrapper.apply(&mut shell_entity, &mut BundleScratch::default())
    };
    world.flush();
    if let Err(error) = apply_result {
        let error = strip_private_wrapper_error(error);
        synchronize_surviving_manifests(world, &mut allocations);
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Apply(error),
            allocations,
            dependencies,
        ));
    }

    if !synchronize_allocation_manifests(world, &mut allocations) {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Adoption(NativeSceneAdoptionError::ManifestDrift),
            allocations,
            dependencies,
        ));
    }
    let mut scratch = NativeSceneValidationScratch::default();
    if let Err(error) =
        validate_adoption(world, &allocations, &declarations, &topology, &mut scratch)
    {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Adoption(error),
            allocations,
            dependencies,
        ));
    }

    let descendant_claims = allocations[2..]
        .iter()
        .map(|allocation| allocation.claim.clone())
        .collect::<Vec<_>>();
    let retirement = match prepare_exact_registry_retirement(
        world.resource::<RouteEntityRegistry>(),
        descendant_claims,
    ) {
        Ok(retirement) => retirement,
        Err(_) => {
            return Err(fail_and_settle(
                world,
                NativeSceneCreationError::Adoption(NativeSceneAdoptionError::RetirementDrift),
                allocations,
                dependencies,
            ));
        }
    };

    for index in 2..allocations.len() {
        allocations[index].temporary_markers_released = true;
        world
            .entity_mut(allocations[index].entity())
            .remove::<(RouteOwnershipSeal, FrameLifecycle)>();
        world.flush();
        if validate_adoption(world, &allocations, &declarations, &topology, &mut scratch).is_err() {
            return Err(fail_and_settle(
                world,
                NativeSceneCreationError::Adoption(NativeSceneAdoptionError::RetirementDrift),
                allocations,
                dependencies,
            ));
        }
    }
    // Marker hooks may add ordinary content/components. Keep the surviving
    // shell/root registry observations in sync with the live accepted Scene.
    if !synchronize_allocation_manifests(world, &mut allocations) {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Adoption(NativeSceneAdoptionError::ManifestDrift),
            allocations,
            dependencies,
        ));
    }
    apply_prepared_exact_registry_retirement(
        &mut world.resource_mut::<RouteEntityRegistry>(),
        retirement,
    );
    if allocations[2..].iter().any(|allocation| {
        world
            .resource::<RouteEntityRegistry>()
            .record(allocation.entity())
            .is_some()
    }) {
        return Err(fail_and_settle(
            world,
            NativeSceneCreationError::Adoption(NativeSceneAdoptionError::RetirementDrift),
            allocations,
            dependencies,
        ));
    }

    Ok(NativeSceneAdoption {
        frame,
        shell: allocations[0].claim.clone(),
        root: allocations[1].claim.clone(),
        current_containment: scratch.containment,
        dependencies,
    })
}

pub(super) fn failure_without_allocations(
    error: NativeSceneCreationError,
    dependencies: Vec<UntypedHandle>,
) -> NativeSceneCreationFailure {
    NativeSceneCreationFailure {
        error,
        focus_error: None,
        settlement: NativeSceneSettlement {
            removed: Vec::new(),
            residual: None,
        },
        dependencies,
    }
}

fn fail_and_settle(
    world: &mut World,
    error: NativeSceneCreationError,
    allocations: Vec<NativeSceneAllocation>,
    dependencies: Vec<UntypedHandle>,
) -> NativeSceneCreationFailure {
    NativeSceneCreationFailure {
        error,
        focus_error: None,
        settlement: settle_failed_native_scene(world, allocations),
        dependencies,
    }
}

fn install_private_identity(
    world: &mut World,
    frame: &RouteFrameVersion,
    allocations: &[NativeSceneAllocation],
) {
    for allocation in allocations {
        let common = (
            RouteOwnershipSeal::new(allocation.claim.seal()),
            FrameLifecycle(EntityLifecycle::PreparedInert),
        );
        match allocation.role {
            NativeSceneAllocationRole::Shell => {
                world.entity_mut(allocation.entity()).insert((
                    common,
                    FrameShell(frame.clone()),
                    NativeFrameShell,
                    Node {
                        display: Display::None,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..Node::default()
                    },
                    Visibility::Hidden,
                    Pickable::IGNORE,
                    RouterInputBarrier::deny_all(),
                ));
            }
            NativeSceneAllocationRole::Declaration(0) => {
                world.entity_mut(allocation.entity()).insert((
                    common,
                    FrameContentTarget(frame.clone()),
                    NativeContentRoot,
                ));
            }
            NativeSceneAllocationRole::Declaration(_) => {
                world.entity_mut(allocation.entity()).insert(common);
            }
        }
    }
}

fn allocation_has_exact_private_identity(
    world: &World,
    allocation: &NativeSceneAllocation,
) -> bool {
    allocation_identity_fields_are_exact(
        world,
        allocation.role,
        &allocation.claim,
        allocation.temporary_markers_released,
    )
}

fn allocation_identity_fields_are_exact(
    world: &World,
    role: NativeSceneAllocationRole,
    claim: &ExactRegistryClaim,
    temporary_markers_released: bool,
) -> bool {
    let record_is_exact = world
        .resource::<RouteEntityRegistry>()
        .record(claim.entity())
        .is_some_and(|record| record.claim() == *claim);
    let markers_match = if temporary_markers_released {
        matches!(role, NativeSceneAllocationRole::Declaration(index) if index > 0)
            && world.get::<RouteOwnershipSeal>(claim.entity()).is_none()
            && world.get::<FrameLifecycle>(claim.entity()).is_none()
    } else {
        world.get::<RouteOwnershipSeal>(claim.entity())
            == Some(&RouteOwnershipSeal::new(claim.seal()))
            && world.get::<FrameLifecycle>(claim.entity())
                == Some(&FrameLifecycle(EntityLifecycle::PreparedInert))
    };
    if !record_is_exact || !markers_match {
        return false;
    }
    match role {
        NativeSceneAllocationRole::Shell => {
            matches!(claim.owner(), RegistryOwner::Committed(EntityOwner::FrameShell(frame))
                if world.get::<FrameShell>(claim.entity()) == Some(&FrameShell(frame.clone())))
                && shell_is_inert(world, claim.entity())
        }
        NativeSceneAllocationRole::Declaration(0) => {
            matches!(claim.owner(), RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame))
                if world.get::<FrameContentTarget>(claim.entity())
                    == Some(&FrameContentTarget(frame.clone())))
        }
        NativeSceneAllocationRole::Declaration(index) => matches!(
            claim.owner(),
            RegistryOwner::Committed(EntityOwner::FrameOutput { local_index, .. })
                if *local_index == index
        ),
    }
}

fn shell_is_inert(world: &World, shell: Entity) -> bool {
    world
        .get::<Node>(shell)
        .is_some_and(|node| node.display == Display::None)
        && world.get::<Visibility>(shell) == Some(&Visibility::Hidden)
        && world.get::<Pickable>(shell) == Some(&Pickable::IGNORE)
        && world
            .get::<RouterInputBarrier>(shell)
            .is_some_and(RouterInputBarrier::denies_all)
}

fn synchronize_allocation_manifests(
    world: &mut World,
    allocations: &mut [NativeSceneAllocation],
) -> bool {
    let mut exchanges = Vec::new();
    if exchanges.try_reserve_exact(allocations.len()).is_err() {
        return false;
    }
    for allocation in allocations.iter() {
        if !world
            .resource::<RouteEntityRegistry>()
            .record(allocation.entity())
            .is_some_and(|record| {
                record.claim() == allocation.claim && record.manifest() == &allocation.manifest
            })
        {
            return false;
        }
        let Ok(Some(actual)) = ComponentManifest::try_from_world(world, allocation.entity()) else {
            return false;
        };
        exchanges.push((
            allocation.claim.clone(),
            allocation.manifest.clone(),
            actual,
        ));
    }
    if !world
        .resource_mut::<RouteEntityRegistry>()
        .compare_exchange_manifests_exact(&exchanges)
    {
        return false;
    }
    for (allocation, (_, _, actual)) in allocations.iter_mut().zip(exchanges) {
        allocation.manifest = actual;
    }
    true
}

fn synchronize_surviving_manifests(world: &mut World, allocations: &mut [NativeSceneAllocation]) {
    for allocation in allocations {
        let Some(record) = world
            .resource::<RouteEntityRegistry>()
            .record(allocation.entity())
        else {
            continue;
        };
        if record.claim() != allocation.claim {
            continue;
        }
        let expected = record.manifest().clone();
        let Ok(Some(actual)) = ComponentManifest::try_from_world(world, allocation.entity()) else {
            continue;
        };
        if world
            .resource_mut::<RouteEntityRegistry>()
            .compare_exchange_manifest_exact(&allocation.claim, &expected, actual.clone())
        {
            allocation.manifest = actual;
        }
    }
}

fn strip_private_wrapper_error(error: ApplySceneError) -> ApplySceneError {
    match error {
        ApplySceneError::RelatedSceneError {
            relationship_type_name,
            index: 0,
            error,
        } if relationship_type_name == type_name::<ChildOf>() => *error,
        other => other,
    }
}

/// Immutable declaration relationships for one realization. Live identity,
/// relationships and containment are still read again after every write.
struct PreparedNativeSceneTopology {
    children: Vec<Vec<Entity>>,
    declaration_indices: EntityHashMap<usize>,
}

impl PreparedNativeSceneTopology {
    fn new(
        allocations: &[NativeSceneAllocation],
        declarations: &[crate::bsn::BsnSceneDeclaration],
    ) -> Result<Self, ()> {
        let mut children = Vec::new();
        children
            .try_reserve_exact(declarations.len())
            .map_err(|_| ())?;
        children.resize_with(declarations.len(), Vec::new);
        let mut declaration_indices = EntityHashMap::new();
        declaration_indices
            .try_reserve(declarations.len())
            .map_err(|_| ())?;
        for (index, declaration) in declarations.iter().enumerate() {
            let entity = allocations[index + 1].entity();
            declaration_indices.insert(entity, index);
            if let Some(parent) = declaration.parent {
                children[parent].try_reserve(1).map_err(|_| ())?;
                children[parent].push(entity);
            }
        }
        Ok(Self {
            children,
            declaration_indices,
        })
    }
}

// Capacity is reused only within one realization; each validation clears all facts.
#[derive(Default)]
struct NativeSceneValidationScratch {
    containment: Vec<Entity>,
    stack: Vec<(Entity, Entity)>,
    visited: HashSet<Entity>,
}

fn validate_adoption(
    world: &World,
    allocations: &[NativeSceneAllocation],
    declarations: &[crate::bsn::BsnSceneDeclaration],
    topology: &PreparedNativeSceneTopology,
    scratch: &mut NativeSceneValidationScratch,
) -> Result<(), NativeSceneAdoptionError> {
    scratch.containment.clear();
    scratch.stack.clear();
    scratch.visited.clear();
    for allocation in allocations {
        if !allocation_has_exact_private_identity(world, allocation) {
            return Err(NativeSceneAdoptionError::AllocationDrift {
                declaration: match allocation.role {
                    NativeSceneAllocationRole::Shell => None,
                    NativeSceneAllocationRole::Declaration(index) => Some(index),
                },
            });
        }
    }
    validate_adoption_relationships(world, allocations, declarations, topology, scratch)
}

fn validate_adoption_relationships(
    world: &World,
    allocations: &[NativeSceneAllocation],
    declarations: &[crate::bsn::BsnSceneDeclaration],
    topology: &PreparedNativeSceneTopology,
    scratch: &mut NativeSceneValidationScratch,
) -> Result<(), NativeSceneAdoptionError> {
    if allocations.len() != declarations.len() + 1 {
        return Err(NativeSceneAdoptionError::RelationshipDrift { declaration: None });
    }
    let shell = allocations[0].entity();
    for (index, declaration) in declarations.iter().enumerate() {
        let allocation = &allocations[index + 1];
        let expected_parent = declaration
            .parent
            .map(|parent| allocations[parent + 1].entity())
            .unwrap_or(shell);
        if current_parent(world, allocation.entity()) != Some(expected_parent) {
            return Err(NativeSceneAdoptionError::RelationshipDrift {
                declaration: Some(index),
            });
        }
    }
    let expected_shell_children = [allocations[1].entity()];
    let actual_shell_children = world
        .get::<Children>(shell)
        .map_or(&[][..], |children| &children[..]);
    if actual_shell_children != expected_shell_children {
        if let Some(child) = actual_shell_children
            .iter()
            .copied()
            .find(|child| !expected_shell_children.contains(child))
        {
            return Err(NativeSceneAdoptionError::UnexpectedChild {
                parent: shell,
                child,
            });
        }
        return Err(NativeSceneAdoptionError::RelationshipDrift { declaration: None });
    }
    for parent in 0..declarations.len() {
        let entity = allocations[parent + 1].entity();
        let expected = &topology.children[parent];
        let actual = world
            .get::<Children>(entity)
            .map_or(&[][..], |children| &children[..]);
        let declared_in_actual = actual.iter().copied().filter(|child| {
            topology
                .declaration_indices
                .get(child)
                .is_some_and(|index| declarations[*index].parent == Some(parent))
        });
        if !declared_in_actual.eq(expected.iter().copied()) {
            return Err(NativeSceneAdoptionError::RelationshipDrift {
                declaration: Some(parent),
            });
        }
    }

    let root = allocations[1].entity();
    let NativeSceneValidationScratch {
        containment,
        stack,
        visited,
    } = scratch;
    stack.push((root, shell));
    while let Some((entity, parent)) = stack.pop() {
        if !visited.insert(entity) || current_parent(world, entity) != Some(parent) {
            return Err(NativeSceneAdoptionError::RelationshipDrift {
                declaration: allocations[1..].iter().find_map(|allocation| {
                    (allocation.entity() == entity).then_some(match allocation.role {
                        NativeSceneAllocationRole::Declaration(index) => index,
                        NativeSceneAllocationRole::Shell => unreachable!(),
                    })
                }),
            });
        }
        if !topology.declaration_indices.contains_key(&entity)
            && entity_has_current_router_owner(world, entity)
        {
            return Err(NativeSceneAdoptionError::UnexpectedChild {
                parent,
                child: entity,
            });
        }
        containment.push(entity);
        let children = world
            .get::<Children>(entity)
            .map_or(&[][..], |children| &children[..]);
        stack.extend(children.iter().copied().rev().map(|child| (child, entity)));
    }
    if topology
        .declaration_indices
        .keys()
        .any(|entity| !visited.contains(entity))
    {
        return Err(NativeSceneAdoptionError::RelationshipDrift { declaration: None });
    }
    Ok(())
}

fn entity_has_current_router_owner(world: &World, entity: Entity) -> bool {
    world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .is_some()
        || world
            .get_resource::<QuarantineLeaseTable>()
            .is_some_and(|table| {
                table.record(entity).is_some() || table.unresolved_record(entity).is_some()
            })
}

fn current_parent(world: &World, entity: Entity) -> Option<Entity> {
    world.get::<ChildOf>(entity).map(ChildOf::parent)
}

fn current_children(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map(|children| children.iter().copied().collect())
        .unwrap_or_default()
}

#[derive(Debug)]
struct FrozenProtectedEntity {
    entity: Entity,
    parent: Option<Entity>,
    children: Vec<Entity>,
    manifest: ComponentManifest,
}

#[derive(Clone, Debug)]
struct FrozenAllocationRelationships {
    entity: Entity,
    present: bool,
    parent: Option<Entity>,
    children: Vec<Entity>,
}

fn validate_settlement_targets(
    world: &World,
    allocations: &[NativeSceneAllocation],
    relationships: &[FrozenAllocationRelationships],
) -> Result<(), NativeSceneIsolationFailure> {
    for allocation in allocations {
        let frozen = relationships
            .iter()
            .find(|frozen| frozen.entity == allocation.entity())
            .ok_or(NativeSceneIsolationFailure::RelationshipDrift)?;
        if frozen.present != world.get_entity(allocation.entity()).is_ok()
            || current_parent(world, allocation.entity()) != frozen.parent
            || current_children(world, allocation.entity()) != frozen.children
        {
            return Err(NativeSceneIsolationFailure::RelationshipDrift);
        }
        if allocation.role == NativeSceneAllocationRole::Shell
            && frozen.present
            && !allocation.manifest.component_ids().is_empty()
            && !shell_is_inert(world, allocation.entity())
        {
            return Err(NativeSceneIsolationFailure::ShellNotInert);
        }
        if !allocation_can_be_settled(world, allocation) {
            return Err(NativeSceneIsolationFailure::AllocationAuthorityDrift);
        }
    }
    Ok(())
}

fn validate_parent_chain(world: &World, start: Entity) -> Result<(), NativeSceneIsolationFailure> {
    let mut current = start;
    let mut visited = HashSet::new();
    while world.get_entity(current).is_ok() {
        if !visited.insert(current) {
            return Err(NativeSceneIsolationFailure::RelationshipCycle);
        }
        let Some(parent) = current_parent(world, current) else {
            return Ok(());
        };
        if current_children(world, parent)
            .iter()
            .filter(|child| **child == current)
            .count()
            != 1
        {
            return Err(NativeSceneIsolationFailure::RelationshipDrift);
        }
        current = parent;
    }
    Err(NativeSceneIsolationFailure::RelationshipDrift)
}

fn settle_failed_native_scene(
    world: &mut World,
    mut allocations: Vec<NativeSceneAllocation>,
) -> NativeSceneSettlement {
    let mut removed = Vec::new();
    if allocations.is_empty() {
        return NativeSceneSettlement {
            removed,
            residual: None,
        };
    }
    let targets = allocations
        .iter()
        .map(NativeSceneAllocation::entity)
        .collect::<HashSet<_>>();
    let mut relationships = allocations
        .iter()
        .map(|allocation| FrozenAllocationRelationships {
            entity: allocation.entity(),
            present: world.get_entity(allocation.entity()).is_ok(),
            parent: current_parent(world, allocation.entity()),
            children: current_children(world, allocation.entity()),
        })
        .collect::<Vec<_>>();
    if let Err(reason) = validate_settlement_targets(world, &allocations, &relationships) {
        return settlement_with_residual(
            world,
            removed,
            allocations,
            Vec::new(),
            NativeSceneResidualIsolation::Unisolated(reason),
        );
    }
    let mut protected = match freeze_protected_entities(world, &allocations, &targets) {
        Ok(protected) => protected,
        Err(reason) => {
            return settlement_with_residual(
                world,
                removed,
                allocations,
                Vec::new(),
                NativeSceneResidualIsolation::Unisolated(reason),
            );
        }
    };

    let shell = allocations
        .iter()
        .find(|allocation| allocation.role == NativeSceneAllocationRole::Shell)
        .map(NativeSceneAllocation::entity)
        .expect("native allocation ledger always begins with its shell");
    let mut retained = HashSet::new();
    for allocation in &allocations {
        if protected
            .iter()
            .any(|frozen| frozen.entity == allocation.entity())
            || current_children(world, allocation.entity())
                .iter()
                .any(|child| !targets.contains(child))
        {
            retained.insert(allocation.entity());
        }
    }
    if !retained.is_empty() {
        retained.insert(shell);
    }

    let retained_roots = allocations
        .iter()
        .map(NativeSceneAllocation::entity)
        .filter(|entity| retained.contains(entity))
        .filter(|entity| {
            *entity != shell
                && !protected.iter().any(|frozen| frozen.entity == *entity)
                && current_parent(world, *entity).is_none_or(|parent| !retained.contains(&parent))
        })
        .collect::<Vec<_>>();
    for root in retained_roots {
        if current_parent(world, root) == Some(shell) {
            continue;
        }
        if let Err(reason) = move_exact_component_under_shell(
            world,
            root,
            shell,
            &allocations,
            &mut relationships,
            &mut protected,
        ) {
            return settlement_with_residual(
                world,
                removed,
                allocations,
                protected,
                NativeSceneResidualIsolation::Unisolated(reason),
            );
        }
    }

    let mut remaining = allocations
        .iter()
        .map(NativeSceneAllocation::entity)
        .filter(|entity| !retained.contains(entity))
        .collect::<HashSet<_>>();
    while !remaining.is_empty() {
        let next = allocations
            .iter()
            .filter(|allocation| allocation.role != NativeSceneAllocationRole::Shell)
            .chain(
                allocations
                    .iter()
                    .filter(|allocation| allocation.role == NativeSceneAllocationRole::Shell),
            )
            .map(NativeSceneAllocation::entity)
            .find(|entity| {
                remaining.contains(entity)
                    && (world.get_entity(*entity).is_err()
                        || current_children(world, *entity).is_empty())
            });
        let Some(entity) = next else {
            return settlement_with_residual(
                world,
                removed,
                allocations,
                protected,
                NativeSceneResidualIsolation::Unisolated(
                    NativeSceneIsolationFailure::RelationshipCycle,
                ),
            );
        };
        let index = allocations
            .iter()
            .position(|allocation| allocation.entity() == entity)
            .expect("remaining allocation belongs to the ledger");
        match remove_exact_leaf(
            world,
            index,
            &allocations,
            &mut relationships,
            &mut protected,
        ) {
            Ok(receipt) => {
                removed.push(receipt);
                remaining.remove(&entity);
                allocations.remove(index);
                relationships.retain(|frozen| frozen.entity != entity);
            }
            Err(reason) => {
                return settlement_with_residual(
                    world,
                    removed,
                    allocations,
                    protected,
                    NativeSceneResidualIsolation::Unisolated(reason),
                );
            }
        }
    }

    if retained.is_empty() {
        return NativeSceneSettlement {
            removed,
            residual: None,
        };
    }
    if let Err(reason) = validate_settlement_targets(world, &allocations, &relationships) {
        return settlement_with_residual(
            world,
            removed,
            allocations,
            protected,
            NativeSceneResidualIsolation::Unisolated(reason),
        );
    }
    for allocation in &allocations {
        if allocation.entity() == shell {
            continue;
        }
        if !parent_chain_reaches_shell(world, allocation.entity(), shell, &retained, &protected) {
            return settlement_with_residual(
                world,
                removed,
                allocations,
                protected,
                NativeSceneResidualIsolation::Unisolated(
                    NativeSceneIsolationFailure::RelationshipDrift,
                ),
            );
        }
    }
    if !protected_entities_match(world, &protected, None) {
        return settlement_with_residual(
            world,
            removed,
            allocations,
            protected,
            NativeSceneResidualIsolation::Unisolated(
                NativeSceneIsolationFailure::ProtectedClosureDrift,
            ),
        );
    }
    synchronize_surviving_manifests(world, &mut allocations);
    settlement_with_residual(
        world,
        removed,
        allocations,
        protected,
        NativeSceneResidualIsolation::InertUnderShell { shell },
    )
}

fn allocation_can_be_settled(world: &World, allocation: &NativeSceneAllocation) -> bool {
    let claim_is_exact = world
        .resource::<RouteEntityRegistry>()
        .record(allocation.entity())
        .is_some_and(|record| record.claim() == allocation.claim);
    if !claim_is_exact {
        return false;
    }
    if world.get_entity(allocation.entity()).is_err() {
        return true;
    }
    allocation_has_exact_private_identity(world, allocation)
        || (allocation.manifest.component_ids().is_empty()
            && ComponentManifest::from_world(world, allocation.entity())
                .is_some_and(|manifest| manifest.component_ids().is_empty()))
}

fn freeze_protected_entities(
    world: &World,
    allocations: &[NativeSceneAllocation],
    targets: &HashSet<Entity>,
) -> Result<Vec<FrozenProtectedEntity>, NativeSceneIsolationFailure> {
    let mut roots = Vec::new();
    for target in allocations.iter().map(NativeSceneAllocation::entity) {
        if world.get_entity(target).is_ok() {
            validate_parent_chain(world, target)?;
        }
        if let Some(parent) = current_parent(world, target)
            && !targets.contains(&parent)
        {
            roots.push((parent, false));
        }
        for child in current_children(world, target) {
            if current_parent(world, child) != Some(target) {
                return Err(NativeSceneIsolationFailure::RelationshipDrift);
            }
            if !targets.contains(&child) {
                roots.push((child, true));
            }
        }
    }
    let mut frozen = Vec::new();
    let mut visited = HashSet::new();
    let mut expanded = HashSet::new();
    while let Some((entity, recurse)) = roots.pop() {
        let first_visit = visited.insert(entity);
        // A boundary parent may later be reached as a full protected subtree.
        // Preserve that stronger role even when the parent was already frozen.
        if !first_visit && (!recurse || !expanded.insert(entity)) {
            continue;
        }
        if recurse {
            expanded.insert(entity);
        }
        validate_parent_chain(world, entity)?;
        let manifest = ComponentManifest::try_from_world(world, entity)
            .map_err(|_| NativeSceneIsolationFailure::Capacity)?
            .ok_or(NativeSceneIsolationFailure::ProtectedClosureDrift)?;
        let children = current_children(world, entity);
        if children
            .iter()
            .any(|child| current_parent(world, *child) != Some(entity))
        {
            return Err(NativeSceneIsolationFailure::RelationshipDrift);
        }
        roots.extend(
            children
                .iter()
                .copied()
                .filter(|child| recurse || !targets.contains(child))
                .map(|child| (child, true)),
        );
        if first_visit {
            frozen
                .try_reserve(1)
                .map_err(|_| NativeSceneIsolationFailure::Capacity)?;
            frozen.push(FrozenProtectedEntity {
                entity,
                parent: current_parent(world, entity),
                children,
                manifest,
            });
        }
    }
    Ok(frozen)
}

fn move_exact_component_under_shell(
    world: &mut World,
    root: Entity,
    shell: Entity,
    allocations: &[NativeSceneAllocation],
    relationships: &mut Vec<FrozenAllocationRelationships>,
    protected: &mut [FrozenProtectedEntity],
) -> Result<(), NativeSceneIsolationFailure> {
    validate_settlement_targets(world, allocations, relationships)?;
    if !protected_entities_match(world, protected, None) {
        return Err(NativeSceneIsolationFailure::ProtectedClosureDrift);
    }
    let allocation = allocations
        .iter()
        .find(|allocation| allocation.entity() == root)
        .ok_or(NativeSceneIsolationFailure::AllocationAuthorityDrift)?;
    let shell_allocation = allocations
        .iter()
        .find(|allocation| allocation.entity() == shell)
        .ok_or(NativeSceneIsolationFailure::AllocationAuthorityDrift)?;
    if !shell_is_inert(world, shell) {
        return Err(NativeSceneIsolationFailure::ShellNotInert);
    }
    if !allocation_has_exact_private_identity(world, allocation)
        || !allocation_has_exact_private_identity(world, shell_allocation)
    {
        return Err(NativeSceneIsolationFailure::AllocationAuthorityDrift);
    }
    if parent_chain_contains(world, shell, root) {
        return Err(NativeSceneIsolationFailure::RelationshipCycle);
    }
    let before_root = freeze_relationship_boundary(world, root)?;
    let before_shell = freeze_relationship_boundary(world, shell)?;
    let old_parent = before_root.parent;
    let before_old_parent = old_parent
        .filter(|parent| *parent != shell)
        .map(|parent| freeze_relationship_boundary(world, parent))
        .transpose()?;
    if let Some(parent) = &before_old_parent
        && !parent.children.contains(&root)
    {
        return Err(NativeSceneIsolationFailure::RelationshipDrift);
    }
    if before_shell.children.contains(&root) {
        return Err(NativeSceneIsolationFailure::RelationshipDrift);
    }

    let mut expected_shell_children = before_shell.children.clone();
    expected_shell_children.push(root);
    let mut expected_targets = relationships.clone();
    for frozen in &mut expected_targets {
        if frozen.entity == root {
            frozen.parent = Some(shell);
        }
        if frozen.entity == shell {
            frozen.children.clone_from(&expected_shell_children);
        }
        if Some(frozen.entity) == old_parent {
            frozen.children.retain(|child| *child != root);
        }
    }

    world.entity_mut(root).insert(ChildOf(shell));
    world.flush();
    validate_settlement_targets(world, allocations, &expected_targets)?;
    if let Some(before_parent) = &before_old_parent {
        let expected = before_parent
            .children
            .iter()
            .copied()
            .filter(|child| *child != root)
            .collect::<Vec<_>>();
        if current_parent(world, before_parent.entity) != before_parent.parent
            || current_children(world, before_parent.entity) != expected
        {
            return Err(NativeSceneIsolationFailure::RelationshipDrift);
        }
        if !protected_entities_match(world, protected, Some((before_parent.entity, &expected))) {
            return Err(NativeSceneIsolationFailure::ProtectedClosureDrift);
        }
        refresh_protected_snapshot(world, protected, before_parent.entity)?;
    } else if !protected_entities_match(world, protected, None) {
        return Err(NativeSceneIsolationFailure::ProtectedClosureDrift);
    }
    *relationships = expected_targets;
    Ok(())
}

fn remove_exact_leaf(
    world: &mut World,
    index: usize,
    allocations: &[NativeSceneAllocation],
    relationships: &mut Vec<FrozenAllocationRelationships>,
    protected: &mut [FrozenProtectedEntity],
) -> Result<SettledNativeSceneAllocation, NativeSceneIsolationFailure> {
    validate_settlement_targets(world, allocations, relationships)?;
    if !protected_entities_match(world, protected, None) {
        return Err(NativeSceneIsolationFailure::ProtectedClosureDrift);
    }
    let allocation = &allocations[index];
    let entity = allocation.entity();
    let parent = current_parent(world, entity);
    let observed_manifest = ComponentManifest::try_from_world(world, entity)
        .map_err(|_| NativeSceneIsolationFailure::Capacity)?;
    let retirement = prepare_exact_registry_retirement(
        world.resource::<RouteEntityRegistry>(),
        vec![allocation.claim.clone()],
    )
    .map_err(|_| NativeSceneIsolationFailure::RegistryRetirementDrift)?;
    if observed_manifest.is_none() {
        apply_prepared_exact_registry_retirement(
            &mut world.resource_mut::<RouteEntityRegistry>(),
            retirement,
        );
        return Ok(SettledNativeSceneAllocation {
            role: allocation.role,
            claim: allocation.claim.clone(),
            actual_parent: parent,
            observed_manifest,
        });
    }
    if !allocation_can_be_settled(world, allocation) || !current_children(world, entity).is_empty()
    {
        return Err(NativeSceneIsolationFailure::AllocationAuthorityDrift);
    }
    let parent_boundary = parent
        .map(|parent| freeze_relationship_boundary(world, parent))
        .transpose()?;
    if let Some(parent_boundary) = &parent_boundary
        && !parent_boundary.children.contains(&entity)
    {
        return Err(NativeSceneIsolationFailure::RelationshipDrift);
    }

    let mut expected_targets = relationships.clone();
    for frozen in &mut expected_targets {
        if frozen.entity == entity {
            frozen.present = false;
            frozen.parent = None;
            frozen.children.clear();
        }
        if Some(frozen.entity) == parent {
            frozen.children.retain(|child| *child != entity);
        }
    }
    world.despawn(entity);
    world.flush();
    validate_settlement_targets(world, allocations, &expected_targets)?;
    if let Some(parent_boundary) = &parent_boundary {
        let expected = parent_boundary
            .children
            .iter()
            .copied()
            .filter(|child| *child != entity)
            .collect::<Vec<_>>();
        if current_parent(world, parent_boundary.entity) != parent_boundary.parent
            || current_children(world, parent_boundary.entity) != expected
            || !protected_entities_match(
                world,
                protected,
                Some((parent_boundary.entity, &expected)),
            )
        {
            return Err(NativeSceneIsolationFailure::ProtectedClosureDrift);
        }
        refresh_protected_snapshot(world, protected, parent_boundary.entity)?;
    } else if !protected_entities_match(world, protected, None) {
        return Err(NativeSceneIsolationFailure::ProtectedClosureDrift);
    }
    apply_prepared_exact_registry_retirement(
        &mut world.resource_mut::<RouteEntityRegistry>(),
        retirement,
    );
    if world
        .resource::<RouteEntityRegistry>()
        .record(entity)
        .is_some()
    {
        return Err(NativeSceneIsolationFailure::RegistryRetirementDrift);
    }
    *relationships = expected_targets;
    Ok(SettledNativeSceneAllocation {
        role: allocation.role,
        claim: allocation.claim.clone(),
        actual_parent: parent,
        observed_manifest,
    })
}

#[derive(Debug)]
struct FrozenRelationshipBoundary {
    entity: Entity,
    parent: Option<Entity>,
    children: Vec<Entity>,
    manifest: ComponentManifest,
}

fn freeze_relationship_boundary(
    world: &World,
    entity: Entity,
) -> Result<FrozenRelationshipBoundary, NativeSceneIsolationFailure> {
    Ok(FrozenRelationshipBoundary {
        entity,
        parent: current_parent(world, entity),
        children: current_children(world, entity),
        manifest: ComponentManifest::try_from_world(world, entity)
            .map_err(|_| NativeSceneIsolationFailure::Capacity)?
            .ok_or(NativeSceneIsolationFailure::RelationshipDrift)?,
    })
}

fn relationship_boundary_matches(
    world: &World,
    before: &FrozenRelationshipBoundary,
    expected_parent: Option<Entity>,
    expected_children: &[Entity],
) -> bool {
    current_parent(world, before.entity) == expected_parent
        && current_children(world, before.entity) == expected_children
        && components_match_except_relationship(world, before.entity, &before.manifest)
}

fn components_match_except_relationship(
    world: &World,
    entity: Entity,
    expected: &ComponentManifest,
) -> bool {
    let Ok(entity_ref) = world.get_entity(entity) else {
        return false;
    };
    let Some(child_of) = world.components().component_id::<ChildOf>() else {
        return false;
    };
    let Some(children) = world.components().component_id::<Children>() else {
        return false;
    };
    let actual =
        ComponentManifest::observed_component_ids(world, entity_ref.archetype().components());
    let expected_count = expected
        .component_ids()
        .iter()
        .filter(|component| **component != child_of && **component != children)
        .count();
    actual
        .clone()
        .filter(|component| *component != child_of && *component != children)
        .count()
        == expected_count
        && actual
            .filter(|component| *component != child_of && *component != children)
            .all(|component| expected.component_ids().contains(&component))
}

fn protected_entities_match(
    world: &World,
    protected: &[FrozenProtectedEntity],
    allowed_children_change: Option<(Entity, &[Entity])>,
) -> bool {
    protected.iter().all(|frozen| {
        if let Some((entity, expected_children)) = allowed_children_change
            && frozen.entity == entity
        {
            let boundary = FrozenRelationshipBoundary {
                entity: frozen.entity,
                parent: frozen.parent,
                children: frozen.children.clone(),
                manifest: frozen.manifest.clone(),
            };
            return relationship_boundary_matches(
                world,
                &boundary,
                frozen.parent,
                expected_children,
            );
        }
        current_parent(world, frozen.entity) == frozen.parent
            && current_children(world, frozen.entity) == frozen.children
            && frozen
                .manifest
                .matches_without_materialization(world, frozen.entity)
    })
}

fn refresh_protected_snapshot(
    world: &World,
    protected: &mut [FrozenProtectedEntity],
    entity: Entity,
) -> Result<(), NativeSceneIsolationFailure> {
    let Some(frozen) = protected.iter_mut().find(|frozen| frozen.entity == entity) else {
        return Ok(());
    };
    frozen.parent = current_parent(world, entity);
    frozen.children = current_children(world, entity);
    frozen.manifest = ComponentManifest::try_from_world(world, entity)
        .map_err(|_| NativeSceneIsolationFailure::Capacity)?
        .ok_or(NativeSceneIsolationFailure::ProtectedClosureDrift)?;
    Ok(())
}

fn parent_chain_contains(world: &World, start: Entity, expected: Entity) -> bool {
    let mut current = start;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        if current == expected {
            return true;
        }
        let Some(parent) = current_parent(world, current) else {
            return false;
        };
        current = parent;
    }
    true
}

fn parent_chain_reaches_shell(
    world: &World,
    start: Entity,
    shell: Entity,
    retained: &HashSet<Entity>,
    protected: &[FrozenProtectedEntity],
) -> bool {
    let mut current = start;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        let Some(parent) = current_parent(world, current) else {
            return false;
        };
        if parent == shell {
            return true;
        }
        if !retained.contains(&parent) && !protected.iter().any(|frozen| frozen.entity == parent) {
            return false;
        }
        current = parent;
    }
    false
}

fn settlement_with_residual(
    world: &World,
    removed: Vec<SettledNativeSceneAllocation>,
    allocations: Vec<NativeSceneAllocation>,
    protected: Vec<FrozenProtectedEntity>,
    isolation: NativeSceneResidualIsolation,
) -> NativeSceneSettlement {
    NativeSceneSettlement {
        removed,
        residual: Some(NativeSceneResidual {
            isolation,
            allocations: allocations
                .into_iter()
                .map(|allocation| NativeSceneResidualAllocation {
                    role: allocation.role,
                    actual_parent: current_parent(world, allocation.entity()),
                    observed_manifest: ComponentManifest::from_world(world, allocation.entity()),
                    temporary_markers_released: allocation.temporary_markers_released,
                    claim: allocation.claim,
                })
                .collect(),
            protected_entities: protected.into_iter().map(|frozen| frozen.entity).collect(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy_app::{App, TaskPoolPlugin};
    use bevy_asset::AssetPlugin;
    use bevy_ecs::{
        component::Component,
        error::BevyError,
        hierarchy::ChildOf,
        lifecycle::HookContext,
        prelude::{Commands, Insert, On, Query, Remove, Resource, With},
        template::{Template, TemplateContext},
        world::DeferredWorld,
    };
    use bevy_scene::{ResolvedScene, SceneFunction, ScenePlugin};
    use bevy_ui::Node;
    use bevy_ui_router_bsn::BsnSceneFactory;
    use bevy_ui_router_core::{
        integration::{EngineAction, RouterEngine},
        prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
        view_input::RouteViewInput,
    };

    use super::*;
    use crate::{
        app_ext::{RouterHandle, RouterHostRegistration},
        bsn::prepare_bsn_view,
        content::ContentPlan,
        host::{
            HostCandidateClassification, HostCandidateRevisionState, activate_granted_top_slot,
            classify_host_candidate, stage_granted_top_slot,
        },
        registry::{
            QuarantineFacts, apply_prepared_unresolved_residual_transfer,
            prepare_unresolved_residual_transfer,
        },
    };

    #[derive(Component, Clone, Debug, Default, Eq, PartialEq)]
    struct AuthoredValue(usize);

    #[derive(Component, Clone, Debug, Eq, PartialEq)]
    struct PartialNodeWasWritten;

    #[derive(Component, Clone, Debug, Eq, PartialEq)]
    struct ProtectedValue;

    #[derive(Component, Clone, Copy)]
    struct NamedEntity(Entity);

    #[derive(Clone)]
    struct ResolveNamedEntity(SceneEntityReference);

    impl Template for ResolveNamedEntity {
        type Output = (NamedEntity,);

        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            let references = &mut context.entity_references;
            let entity = context
                .entity
                .world_scope(|world| references.get(self.0, world));
            Ok((NamedEntity(entity),))
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Clone)]
    struct CaptureClaims(Arc<Mutex<Vec<ExactRegistryClaim>>>);

    impl Template for CaptureClaims {
        type Output = ();

        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            *self.0.lock().unwrap() = context
                .entity
                .world()
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| record.claim())
                .collect();
            Ok(())
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Component, Clone)]
    #[component(on_despawn)]
    struct ChangeOtherOnDespawn {
        other: Entity,
        remove_seal: bool,
    }

    impl ChangeOtherOnDespawn {
        fn on_despawn(mut world: DeferredWorld, context: HookContext) {
            let change = world.get::<Self>(context.entity).unwrap().clone();
            if change.remove_seal {
                world
                    .commands()
                    .entity(change.other)
                    .remove::<RouteOwnershipSeal>();
            } else {
                world
                    .commands()
                    .entity(change.other)
                    .insert(AuthoredValue(81));
            }
        }
    }

    #[derive(Clone)]
    struct InstallDespawnChange {
        other: SceneEntityReference,
        remove_seal: bool,
    }

    impl Template for InstallDespawnChange {
        type Output = (ChangeOtherOnDespawn,);

        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            let references = &mut context.entity_references;
            let other = context
                .entity
                .world_scope(|world| references.get(self.other, world));
            Ok((ChangeOtherOnDespawn {
                other,
                remove_seal: self.remove_seal,
            },))
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Resource, Default)]
    struct FirstPrivateWrite(Vec<ExactRegistryClaim>);

    fn witness_first_private_write(mut world: DeferredWorld, _: HookContext) {
        if world.resource::<FirstPrivateWrite>().0.is_empty() {
            let claims = world
                .resource::<RouteEntityRegistry>()
                .records()
                .iter()
                .map(|record| record.claim())
                .collect();
            world.resource_mut::<FirstPrivateWrite>().0 = claims;
        }
    }

    #[derive(Component, Clone, Debug, Default, Eq, PartialEq)]
    #[component(on_insert)]
    struct SpawnOrdinaryChild;

    impl SpawnOrdinaryChild {
        fn on_insert(mut world: DeferredWorld, context: HookContext) {
            world
                .commands()
                .spawn((AuthoredValue(4), ChildOf(context.entity)));
        }
    }

    #[derive(Clone)]
    struct InsertNodeBundle;

    impl Template for InsertNodeBundle {
        type Output = (Node, PartialNodeWasWritten);

        fn build_template(
            &self,
            _context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            Ok((
                Node {
                    width: Val::Px(37.0),
                    ..Node::default()
                },
                PartialNodeWasWritten,
            ))
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Clone)]
    struct AttachProtected(Entity);

    #[derive(Component, Clone)]
    #[component(on_insert)]
    struct AttachProtectedOnInsert(Entity);

    impl AttachProtectedOnInsert {
        fn on_insert(mut world: DeferredWorld, context: HookContext) {
            let child = world.get::<Self>(context.entity).unwrap().0;
            world
                .commands()
                .entity(child)
                .insert(ChildOf(context.entity));
        }
    }

    impl Template for AttachProtectedOnInsert {
        type Output = Self;

        fn build_template(&self, _: &mut TemplateContext<'_, '_>) -> bevy_ecs::error::Result<Self> {
            Ok(self.clone())
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Clone)]
    struct AttachToParent(Entity);

    impl Template for AttachToParent {
        type Output = (ChildOf,);

        fn build_template(
            &self,
            _: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            Ok((ChildOf(self.0),))
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    impl Template for AttachProtected {
        type Output = ();

        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            let target = context.entity.id();
            context.entity.world_scope(|world| {
                world.entity_mut(self.0).insert(ChildOf(target));
            });
            Ok(())
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Clone)]
    struct CreateOrdinaryForwardReferenceParent {
        child: SceneEntityReference,
        observed: Arc<Mutex<Option<(Entity, Entity, Entity, Vec<ExactRegistryClaim>)>>>,
    }

    impl Template for CreateOrdinaryForwardReferenceParent {
        type Output = ();

        fn build_template(
            &self,
            context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            let root = context.entity.id();
            let references = &mut context.entity_references;
            let observed = context.entity.world_scope(|world| {
                let child = references.get(self.child, world);
                let claims = world
                    .resource::<RouteEntityRegistry>()
                    .records()
                    .iter()
                    .map(|record| record.claim())
                    .collect();
                let ordinary = world
                    .spawn((
                        ProtectedValue,
                        Node {
                            width: Val::Px(23.0),
                            ..Node::default()
                        },
                        Visibility::Visible,
                        Pickable::default(),
                        ChildOf(root),
                    ))
                    .id();
                world.entity_mut(child).insert((
                    Node {
                        width: Val::Px(19.0),
                        ..Node::default()
                    },
                    AuthoredValue(7),
                    ChildOf(ordinary),
                ));
                (root, ordinary, child, claims)
            });
            *self.observed.lock().unwrap() = Some(observed);
            Ok(())
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    #[derive(Clone)]
    struct FailBundle;

    impl Template for FailBundle {
        type Output = ();

        fn build_template(
            &self,
            _context: &mut TemplateContext<'_, '_>,
        ) -> bevy_ecs::error::Result<Self::Output> {
            Err(BevyError::error("native Scene test failure"))
        }

        fn clone_template(&self) -> Self {
            self.clone()
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ));
        app.init_resource::<RouteEntityRegistry>();
        app
    }

    fn frame_and_input() -> (RouteFrameVersion, RouteViewInput) {
        let view = RouteViewKey::new("native-creation").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(routes, view).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(mut task) = engine.action().unwrap() else {
            panic!("root push must yield a frame task");
        };
        (task.frame().clone(), task.take_input())
    }

    fn prepare(
        app: &App,
        input: &RouteViewInput,
        scene: impl FnOnce(&mut ResolvedScene) + Send + Sync + 'static,
    ) -> PreparedBsnScene {
        let scene = Arc::new(Mutex::new(Some(scene)));
        let mut factory: BsnSceneFactory = Box::new(move |_| {
            let scene = scene
                .lock()
                .unwrap()
                .take()
                .expect("the one-shot test Scene is built once");
            Ok(Box::new(SceneFunction(move |_, root| scene(root))))
        });
        prepare_bsn_view(app.world(), &mut factory, input).unwrap()
    }

    #[test]
    fn official_wrapper_adopts_preallocated_scene_and_retires_descendant_claims() {
        let mut app = app();
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, |root| {
            root.push_template(AuthoredValue(1));
            root.push_template(SpawnOrdinaryChild);
            let mut first = ResolvedScene::default();
            first.push_template(AuthoredValue(2));
            let mut grandchild = ResolvedScene::default();
            grandchild.push_template(AuthoredValue(3));
            first
                .get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(grandchild);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(first);
        });

        let adoption = create_native_scene(app.world_mut(), frame.clone(), prepared).unwrap();
        assert_eq!(adoption.frame(), &frame);
        assert_eq!(adoption.current_containment().len(), 4);
        let root = adoption.current_containment()[0];
        let find_value = |value| {
            adoption
                .current_containment()
                .iter()
                .copied()
                .find(|entity| {
                    app.world().get::<AuthoredValue>(*entity) == Some(&AuthoredValue(value))
                })
                .expect("authored content must remain in the adopted closure")
        };
        let child = find_value(2);
        let grandchild = find_value(3);
        let ordinary_child = find_value(4);
        assert_eq!(
            app.world().get::<AuthoredValue>(root),
            Some(&AuthoredValue(1))
        );
        assert_eq!(
            app.world().get::<AuthoredValue>(child),
            Some(&AuthoredValue(2))
        );
        assert_eq!(
            app.world().get::<AuthoredValue>(grandchild),
            Some(&AuthoredValue(3))
        );
        assert_eq!(
            current_parent(app.world(), root),
            Some(adoption.shell_claim().entity())
        );
        assert_eq!(current_parent(app.world(), child), Some(root));
        assert_eq!(current_parent(app.world(), grandchild), Some(child));
        assert_eq!(current_parent(app.world(), ordinary_child), Some(root));
        assert!(shell_is_inert(app.world(), adoption.shell_claim().entity()));
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(adoption.root_claim().entity())
                .is_some()
        );
        for descendant in [child, grandchild] {
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(descendant)
                    .is_none()
            );
            assert!(app.world().get::<RouteOwnershipSeal>(descendant).is_none());
            assert!(app.world().get::<FrameLifecycle>(descendant).is_none());
        }
    }

    #[test]
    fn named_references_use_exact_preallocation_visible_at_the_first_private_write() {
        let mut app = app();
        app.init_resource::<FirstPrivateWrite>();
        app.world_mut()
            .register_component_hooks::<RouteOwnershipSeal>()
            .on_add(witness_first_private_write);
        let (frame, input) = frame_and_input();
        let root_reference = SceneEntityReference::new((file!(), line!() as usize, 0), 0, 0);
        let child_reference = SceneEntityReference::new((file!(), line!() as usize, 0), 1, 0);
        let prepared = prepare(&app, &input, move |root| {
            root.entity_references.push(root_reference);
            root.push_bundle_template(ResolveNamedEntity(child_reference));
            let mut child = ResolvedScene::default();
            child.entity_references.push(child_reference);
            child.push_bundle_template(ResolveNamedEntity(root_reference));
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });
        let entity_count_before = app.world().entities().count_spawned();
        let adoption = create_native_scene(app.world_mut(), frame, prepared).unwrap();
        let root = adoption.root_claim().entity();
        let child = app.world().get::<NamedEntity>(root).unwrap().0;
        assert_eq!(app.world().get::<NamedEntity>(child).unwrap().0, root);
        assert_eq!(current_children(app.world(), root), vec![child]);
        assert_eq!(adoption.current_containment(), &[root, child]);
        assert_eq!(
            app.world().entities().count_spawned(),
            entity_count_before + 3
        );
        let first = &app.world().resource::<FirstPrivateWrite>().0;
        assert_eq!(first.len(), 3);
        assert!(first.contains(adoption.shell_claim()));
        assert!(first.contains(adoption.root_claim()));
        assert!(first.iter().any(|claim| claim.entity() == child));
        assert!(app.world().get::<RouteOwnershipSeal>(child).is_none());
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(child)
                .is_none()
        );
    }

    #[test]
    fn partial_node_failure_removes_unattached_exact_child_in_the_same_call() {
        let mut app = app();
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, |root| {
            let mut child = ResolvedScene::default();
            child.push_bundle_template(InsertNodeBundle);
            child.push_bundle_template(FailBundle);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });

        let failure = create_native_scene(app.world_mut(), frame, prepared)
            .err()
            .expect("the official child bundle failure must be returned");
        let (error, settlement, dependencies) = failure.into_parts();
        assert!(dependencies.is_empty());
        let NativeSceneCreationError::Apply(ApplySceneError::RelatedSceneError {
            relationship_type_name,
            index: 0,
            error,
        }) = error
        else {
            panic!("the original child relationship layer must remain");
        };
        assert_eq!(relationship_type_name, type_name::<ChildOf>());
        let ApplySceneError::TemplateBuildError(source) = *error else {
            panic!("the private wrapper must not add another relationship error layer");
        };
        assert!(source.to_string().contains("native Scene test failure"));
        assert!(settlement.is_clean());
        let node = app.world_mut().register_component::<Node>();
        let partial = app
            .world_mut()
            .register_component::<PartialNodeWasWritten>();
        let failed_child = settlement
            .removed()
            .iter()
            .find(|removed| removed.role == NativeSceneAllocationRole::Declaration(1))
            .expect("the failed child remains an exact creation obligation");
        assert_eq!(failed_child.actual_parent, None);
        let manifest = failed_child
            .observed_manifest
            .as_ref()
            .expect("the partial child existed before safe leaf cleanup");
        assert!(manifest.component_ids().contains(&node));
        assert!(manifest.component_ids().contains(&partial));
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .is_empty()
        );
        for removed in settlement.removed() {
            assert!(app.world().get_entity(removed.claim.entity()).is_err());
        }
    }

    #[test]
    fn protected_extra_survives_apply_failure_under_the_inert_shell_residual() {
        let mut app = app();
        let protected = app.world_mut().spawn(ProtectedValue).id();
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, move |root| {
            let mut child = ResolvedScene::default();
            child.push_bundle_template(InsertNodeBundle);
            child.push_bundle_template(AttachProtected(protected));
            child.push_bundle_template(FailBundle);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });

        let failure = create_native_scene(app.world_mut(), frame, prepared)
            .err()
            .expect("the official child bundle failure must be returned");
        let (error, settlement, _) = failure.into_parts();
        assert!(matches!(error, NativeSceneCreationError::Apply(_)));
        let residual = settlement
            .residual()
            .expect("a protected child requires an owned inert residual");
        let NativeSceneResidualIsolation::InertUnderShell { shell } = residual.isolation() else {
            panic!("the retained component must be proven below the inert shell");
        };
        assert!(shell_is_inert(app.world(), shell));
        assert_eq!(residual.protected_entities(), &[protected]);
        assert!(app.world().get::<ProtectedValue>(protected).is_some());
        let carrier = current_parent(app.world(), protected)
            .expect("the protected extra remains attached to its exact carrier");
        assert_eq!(current_parent(app.world(), carrier), Some(shell));
        assert!(
            residual
                .allocations()
                .iter()
                .any(|allocation| allocation.claim.entity() == carrier)
        );
        assert!(
            settlement
                .removed()
                .iter()
                .any(|allocation| allocation.role == NativeSceneAllocationRole::Declaration(0))
        );
    }

    #[test]
    fn shell_value_drift_during_reparent_stops_before_any_leaf_removal() {
        let mut app = app();
        let protected = app.world_mut().spawn(ProtectedValue).id();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let scene_captured = captured.clone();
        app.world_mut().add_observer(
            |event: On<Insert, ChildOf>,
             parents: Query<&ChildOf>,
             partial: Query<(), With<PartialNodeWasWritten>>,
             mut shells: Query<&mut Node, With<FrameShell>>| {
                if partial.contains(event.entity)
                    && let Ok(parent) = parents.get(event.entity)
                    && let Ok(mut shell) = shells.get_mut(parent.parent())
                {
                    shell.display = Display::Flex;
                }
            },
        );
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, move |root| {
            let mut child = ResolvedScene::default();
            child.push_bundle_template(CaptureClaims(scene_captured));
            child.push_bundle_template(InsertNodeBundle);
            child.push_bundle_template(AttachProtected(protected));
            child.push_bundle_template(FailBundle);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .extend([child, ResolvedScene::default()]);
        });
        let (error, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        assert!(matches!(error, NativeSceneCreationError::Apply(_)));
        let residual = settlement.residual().unwrap();
        assert_eq!(
            residual.isolation(),
            NativeSceneResidualIsolation::Unisolated(NativeSceneIsolationFailure::ShellNotInert)
        );
        assert!(settlement.removed().is_empty());
        let claims = captured.lock().unwrap();
        assert_eq!(claims.len(), 4);
        for claim in claims.iter() {
            assert!(app.world().get_entity(claim.entity()).is_ok());
            assert!(
                residual
                    .allocations()
                    .iter()
                    .any(|allocation| &allocation.claim == claim)
            );
        }
        assert!(app.world().get::<ProtectedValue>(protected).is_some());
    }

    #[test]
    fn despawn_hook_authority_loss_stops_before_touching_other_allocations() {
        let (app, error, settlement, claims) = run_despawn_change_failure(true);
        assert!(matches!(error, NativeSceneCreationError::Apply(_)));
        let residual = settlement.residual().unwrap();
        assert_eq!(
            residual.isolation(),
            NativeSceneResidualIsolation::Unisolated(
                NativeSceneIsolationFailure::AllocationAuthorityDrift
            )
        );
        let root = claims
            .iter()
            .find(|claim| {
                matches!(
                    claim.owner(),
                    RegistryOwner::Committed(EntityOwner::FrameContentTarget(_))
                )
            })
            .unwrap()
            .entity();
        assert!(app.world().get_entity(root).is_err());
        for claim in claims.iter().filter(|claim| claim.entity() != root) {
            assert!(app.world().get_entity(claim.entity()).is_ok());
            assert!(
                residual
                    .allocations()
                    .iter()
                    .any(|allocation| &allocation.claim == claim)
            );
        }
    }

    #[test]
    fn ordinary_component_added_by_despawn_hook_does_not_revoke_native_ownership() {
        let (app, error, settlement, claims) = run_despawn_change_failure(false);
        assert!(matches!(error, NativeSceneCreationError::Apply(_)));
        assert!(settlement.is_clean());
        assert_eq!(settlement.removed().len(), claims.len());
        for claim in claims {
            assert!(app.world().get_entity(claim.entity()).is_err());
            assert!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .is_none()
            );
        }
    }

    fn run_despawn_change_failure(
        remove_seal: bool,
    ) -> (
        App,
        NativeSceneCreationError,
        NativeSceneSettlement,
        Vec<ExactRegistryClaim>,
    ) {
        let mut app = app();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let scene_captured = captured.clone();
        let (frame, input) = frame_and_input();
        let other = SceneEntityReference::new((file!(), line!() as usize, 0), 0, 0);
        let prepared = prepare(&app, &input, move |root| {
            root.push_bundle_template(CaptureClaims(scene_captured));
            root.push_bundle_template(InstallDespawnChange { other, remove_seal });
            root.push_bundle_template(FailBundle);
            let mut later = ResolvedScene::default();
            later.entity_references.push(other);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .extend([ResolvedScene::default(), later]);
        });
        let (error, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        let claims = captured.lock().unwrap().clone();
        (app, error, settlement, claims)
    }

    #[test]
    fn external_parent_deep_sibling_drift_stops_after_the_reparent() {
        let mut app = app();
        let external = app.world_mut().spawn(AuthoredValue(20)).id();
        let sibling = app
            .world_mut()
            .spawn((AuthoredValue(21), ChildOf(external)))
            .id();
        let deep = app
            .world_mut()
            .spawn((ProtectedValue, ChildOf(sibling)))
            .id();
        let carried = app.world_mut().spawn(ProtectedValue).id();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let scene_captured = captured.clone();
        app.world_mut().add_observer(
            move |event: On<Insert, ChildOf>,
                  parents: Query<&ChildOf>,
                  partial: Query<(), With<PartialNodeWasWritten>>,
                  shells: Query<(), With<FrameShell>>,
                  mut commands: Commands| {
                if partial.contains(event.entity)
                    && let Ok(parent) = parents.get(event.entity)
                    && shells.contains(parent.parent())
                {
                    commands.entity(deep).remove::<ProtectedValue>();
                }
            },
        );
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, move |root| {
            let mut child = ResolvedScene::default();
            child.push_bundle_template(CaptureClaims(scene_captured));
            child.push_bundle_template(AttachToParent(external));
            child.push_bundle_template(InsertNodeBundle);
            child.push_bundle_template(AttachProtected(carried));
            child.push_bundle_template(FailBundle);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });
        let (_, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        assert_eq!(
            settlement.residual().unwrap().isolation(),
            NativeSceneResidualIsolation::Unisolated(
                NativeSceneIsolationFailure::ProtectedClosureDrift
            )
        );
        assert!(settlement.removed().is_empty());
        for claim in captured.lock().unwrap().iter() {
            assert!(app.world().get_entity(claim.entity()).is_ok());
        }
        assert_eq!(current_children(app.world(), external), vec![sibling]);
        assert_eq!(current_parent(app.world(), sibling), Some(external));
        assert_eq!(current_parent(app.world(), deep), Some(sibling));
        assert!(app.world().get::<ProtectedValue>(deep).is_none());
        assert!(app.world().get::<ProtectedValue>(carried).is_some());
    }

    #[test]
    fn adoption_returns_the_live_closure_after_descendant_marker_hooks() {
        let mut app = app();
        app.world_mut().add_observer(
            |event: On<Remove, RouteOwnershipSeal>,
             authored: Query<&AuthoredValue>,
             mut commands: Commands| {
                if authored.get(event.entity) == Ok(&AuthoredValue(1)) {
                    commands.spawn((AuthoredValue(2), ChildOf(event.entity)));
                }
            },
        );
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, |root| {
            let mut child = ResolvedScene::default();
            child.push_template(AuthoredValue(1));
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });
        let adoption = create_native_scene(app.world_mut(), frame, prepared).unwrap();
        assert_eq!(adoption.current_containment().len(), 3);
        let child = current_children(app.world(), adoption.root_claim().entity())[0];
        let ordinary = current_children(app.world(), child)[0];
        assert_eq!(
            adoption.current_containment(),
            &[adoption.root_claim().entity(), child, ordinary]
        );
        assert_eq!(
            app.world().get::<AuthoredValue>(ordinary),
            Some(&AuthoredValue(2))
        );
    }

    #[test]
    fn adoption_rechecks_declared_sibling_order_after_marker_hooks() {
        let mut app = app();
        app.world_mut().add_observer(
            |event: On<Remove, RouteOwnershipSeal>,
             authored: Query<(&AuthoredValue, &ChildOf)>,
             children: Query<&Children>,
             mut commands: Commands| {
                if let Ok((AuthoredValue(1), parent)) = authored.get(event.entity) {
                    let siblings = children.get(parent.parent()).unwrap();
                    commands
                        .entity(parent.parent())
                        .replace_children(&[siblings[1], siblings[0]]);
                }
            },
        );
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, |root| {
            for value in [1, 2] {
                let mut child = ResolvedScene::default();
                child.push_template(AuthoredValue(value));
                root.get_or_insert_related_resolved_scenes::<ChildOf>()
                    .scenes
                    .push(child);
            }
        });
        let (error, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        assert!(matches!(
            error,
            NativeSceneCreationError::Adoption(NativeSceneAdoptionError::RetirementDrift)
        ));
        assert!(settlement.is_clean());
        assert_eq!(settlement.removed().len(), 4);
    }

    #[test]
    fn intended_marker_release_remains_settleable_after_a_relationship_hook_failure() {
        let mut app = app();
        app.world_mut().add_observer(
            |event: On<Remove, RouteOwnershipSeal>,
             authored: Query<&AuthoredValue>,
             mut commands: Commands| {
                if authored.get(event.entity) == Ok(&AuthoredValue(1)) {
                    commands.entity(event.entity).remove::<ChildOf>();
                }
            },
        );
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, |root| {
            let mut child = ResolvedScene::default();
            child.push_template(AuthoredValue(1));
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });
        let (error, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        assert!(matches!(
            error,
            NativeSceneCreationError::Adoption(NativeSceneAdoptionError::RetirementDrift)
        ));
        assert!(settlement.is_clean());
        assert_eq!(settlement.removed().len(), 3);
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .records()
                .is_empty()
        );
    }

    #[test]
    fn adoption_rejects_an_existing_unresolved_owner_without_retiring_or_mutating_it() {
        let mut app = app();
        app.init_resource::<QuarantineLeaseTable>();
        let (old_frame, old_input) = frame_and_input();
        let old_prepared = prepare(&app, &old_input, |root| {
            root.push_template(AuthoredValue(45));
        });
        let old = create_native_scene(app.world_mut(), old_frame, old_prepared).unwrap();
        let old_root = old.root_claim().entity();
        let old_claim = old.root_claim().clone();
        let actual = ComponentManifest::from_world(app.world(), old_root).unwrap();
        app.world_mut().resource_scope(
            |world, mut registry: bevy_ecs::prelude::Mut<RouteEntityRegistry>| {
                let parent = current_parent(world, old_root);
                let mut table = world.resource_mut::<QuarantineLeaseTable>();
                let transfer = prepare_unresolved_residual_transfer(
                    &mut table,
                    &registry,
                    vec![old_claim.clone()],
                    vec![(old_root, actual)],
                    QuarantineFacts::new(None, parent, vec![], vec![]),
                )
                .unwrap();
                apply_prepared_unresolved_residual_transfer(&mut table, &mut registry, transfer);
            },
        );
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(old_root)
                .is_none()
        );
        let old_residuals = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .unresolved_residuals()
            .to_vec();
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, move |root| {
            root.push_template(AttachProtectedOnInsert(old_root));
        });
        let (error, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        assert!(
            matches!(error, NativeSceneCreationError::Adoption(NativeSceneAdoptionError::UnexpectedChild { child, .. }) if child == old_root)
        );
        let residual = settlement.residual().unwrap();
        let NativeSceneResidualIsolation::InertUnderShell { shell } = residual.isolation() else {
            panic!("the new exact carrier must retain the protected old owner under its shell");
        };
        let carrier = current_parent(app.world(), old_root).unwrap();
        assert_eq!(current_parent(app.world(), carrier), Some(shell));
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_residuals(),
            old_residuals
        );
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .unresolved_record(old_root)
                .unwrap()
                .claim(),
            old_claim
        );
        assert_eq!(
            app.world().get::<AuthoredValue>(old_root),
            Some(&AuthoredValue(45))
        );
        assert_eq!(
            app.world().get::<RouteOwnershipSeal>(old_root),
            Some(&RouteOwnershipSeal::new(old_claim.seal()))
        );
        assert!(
            settlement
                .removed()
                .iter()
                .all(|receipt| receipt.claim.entity() != old_root)
        );
    }

    #[test]
    fn adoption_rejects_a_real_quarantine_lease_without_changing_its_owner() {
        let mut app = app();
        app.init_resource::<QuarantineLeaseTable>();
        let old_shell = realize_and_quarantine_old_frame(app.world_mut());
        let leases_before = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .leases()
            .to_vec();
        let old_claim = app
            .world()
            .resource::<QuarantineLeaseTable>()
            .record(old_shell)
            .unwrap()
            .claim();
        let old_children = current_children(app.world(), old_shell);
        assert!(
            app.world()
                .resource::<RouteEntityRegistry>()
                .record(old_shell)
                .is_none()
        );
        let (frame, input) = frame_and_input();
        let prepared = prepare(&app, &input, move |root| {
            root.push_template(AttachProtectedOnInsert(old_shell));
        });
        let (error, settlement, _) = create_native_scene(app.world_mut(), frame, prepared)
            .unwrap_err()
            .into_parts();
        assert!(
            matches!(error, NativeSceneCreationError::Adoption(NativeSceneAdoptionError::UnexpectedChild { child, .. }) if child == old_shell)
        );
        let residual = settlement.residual().unwrap();
        let NativeSceneResidualIsolation::InertUnderShell { shell } = residual.isolation() else {
            panic!("the new carrier must retain the protected quarantine subtree under its shell");
        };
        assert_eq!(
            current_parent(app.world(), current_parent(app.world(), old_shell).unwrap()),
            Some(shell)
        );
        assert_eq!(current_children(app.world(), old_shell), old_children);
        assert_eq!(
            app.world().resource::<QuarantineLeaseTable>().leases(),
            leases_before
        );
        assert_eq!(
            app.world()
                .resource::<QuarantineLeaseTable>()
                .record(old_shell)
                .unwrap()
                .claim(),
            old_claim
        );
        assert!(residual.protected_entities().contains(&old_shell));
        assert!(
            settlement
                .removed()
                .iter()
                .all(|receipt| receipt.claim.entity() != old_shell)
        );
    }

    fn realize_and_quarantine_old_frame(world: &mut World) -> Entity {
        // Reuse the legacy settlement's real host/admission/realization path to
        // supply an existing lease. This does not construct a verified token or
        // use a legacy content plan to realize the native Scene under test.
        let router = RouterHandle::fixture(17, 19);
        let host = world
            .spawn((Node::default(), RouterHostRegistration::new(router)))
            .id();
        let mut revisions = HostCandidateRevisionState::default();
        let HostCandidateClassification::Unique(candidate) =
            classify_host_candidate(world, router, &mut revisions)
        else {
            panic!("the real host candidate must be unique");
        };
        let view = RouteViewKey::new("old-quarantine").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(routes, view).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        let staged = stage_granted_top_slot(world, candidate.clone(), &grant).unwrap();
        let top_slot = staged.entity();
        assert!(activate_granted_top_slot(
            world, &candidate, &grant, &staged
        ));
        let top_claim = world
            .resource::<RouteEntityRegistry>()
            .record(top_slot)
            .unwrap()
            .claim();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("real host admission must yield the old frame task");
        };
        let old = super::super::realize_frame(
            world,
            candidate,
            top_slot,
            top_claim,
            task.frame().clone(),
            task.view_request_sequence(),
            ContentPlan::new().accept(),
        )
        .unwrap();
        let shell = old.shell();
        assert!(super::super::validate_realized_frame(world, &old));
        super::super::settle_frame(world, old).unwrap();
        assert!(world.get_entity(host).is_ok());
        assert!(
            world
                .resource::<QuarantineLeaseTable>()
                .record(shell)
                .is_some()
        );
        shell
    }

    #[test]
    fn forward_reference_below_ordinary_child_is_retained_under_inert_shell_on_failure() {
        // The same ordinary parent is reached from a protected child edge and an
        // allocation's external-parent edge. Repeated real attempts also catch
        // classification that accidentally depends on randomized set iteration.
        for _ in 0..32 {
            assert_forward_reference_failure_is_isolated();
        }
    }

    fn assert_forward_reference_failure_is_isolated() {
        let mut app = app();
        let unrelated = app.world_mut().spawn(AuthoredValue(99)).id();
        let (frame, input) = frame_and_input();
        let observed = Arc::new(Mutex::new(None));
        let scene_observed = observed.clone();
        let child_reference = SceneEntityReference::new((file!(), line!() as usize, 0), 0, 0);
        let prepared = prepare(&app, &input, move |root| {
            root.push_bundle_template(InsertNodeBundle);
            root.push_bundle_template(CreateOrdinaryForwardReferenceParent {
                child: child_reference,
                observed: scene_observed,
            });
            root.push_bundle_template(FailBundle);
            let mut child = ResolvedScene::default();
            child.entity_references.push(child_reference);
            root.get_or_insert_related_resolved_scenes::<ChildOf>()
                .scenes
                .push(child);
        });

        let failure = create_native_scene(app.world_mut(), frame, prepared)
            .err()
            .expect("the later root bundle fails before official shell attachment");
        let (error, settlement, dependencies) = failure.into_parts();
        let NativeSceneCreationError::Apply(ApplySceneError::TemplateBuildError(source)) = error
        else {
            panic!("only the wrapper layer must be stripped from the original root error");
        };
        assert!(source.to_string().contains("native Scene test failure"));
        assert!(dependencies.is_empty());
        let (root, ordinary, child, claims) = observed.lock().unwrap().take().unwrap();
        assert_eq!(claims.len(), 3);
        assert!(claims.iter().any(|claim| claim.entity() == root));
        assert!(claims.iter().any(|claim| claim.entity() == child));

        let residual = settlement
            .residual()
            .expect("the ordinary child and its exact descendant need retained ownership");
        let NativeSceneResidualIsolation::InertUnderShell { shell } = residual.isolation() else {
            panic!("an acyclic allocation/ordinary/allocation chain must be isolated in this call");
        };
        for claim in claims {
            if claim.entity() == child && app.world().get_entity(child).is_err() {
                let receipt = settlement
                    .removed()
                    .iter()
                    .find(|receipt| receipt.claim == claim)
                    .expect("safe exact leaf deletion must retain its original claim receipt");
                assert_eq!(receipt.actual_parent, Some(ordinary));
                assert!(receipt.observed_manifest.is_some());
                assert!(
                    app.world()
                        .resource::<RouteEntityRegistry>()
                        .record(child)
                        .is_none()
                );
                assert!(
                    !residual
                        .allocations()
                        .iter()
                        .any(|allocation| allocation.claim == claim)
                );
                continue;
            }
            assert_eq!(
                app.world()
                    .resource::<RouteEntityRegistry>()
                    .record(claim.entity())
                    .unwrap()
                    .claim(),
                claim
            );
            assert_eq!(
                app.world().get::<RouteOwnershipSeal>(claim.entity()),
                Some(&RouteOwnershipSeal::new(claim.seal()))
            );
            assert!(
                residual
                    .allocations()
                    .iter()
                    .any(|allocation| allocation.claim == claim)
            );
        }
        assert_eq!(current_parent(app.world(), root), Some(shell));
        assert_eq!(current_parent(app.world(), ordinary), Some(root));
        assert_eq!(current_children(app.world(), shell), vec![root]);
        assert_eq!(current_children(app.world(), root), vec![ordinary]);
        if app.world().get_entity(child).is_ok() {
            assert_eq!(current_parent(app.world(), child), Some(ordinary));
            assert_eq!(current_children(app.world(), ordinary), vec![child]);
            assert_eq!(app.world().get::<Node>(child).unwrap().width, Val::Px(19.0));
            assert_eq!(
                app.world().get::<AuthoredValue>(child),
                Some(&AuthoredValue(7))
            );
        } else {
            assert!(current_children(app.world(), ordinary).is_empty());
        }
        assert_eq!(app.world().get::<Node>(root).unwrap().width, Val::Px(37.0));
        assert_eq!(
            app.world().get::<Node>(ordinary).unwrap().width,
            Val::Px(23.0)
        );
        assert_eq!(
            app.world().get::<Visibility>(ordinary),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().get::<Pickable>(ordinary),
            Some(&Pickable::default())
        );
        assert!(residual.protected_entities().contains(&ordinary));
        assert!(shell_is_inert(app.world(), shell));
        assert_eq!(
            app.world().get::<AuthoredValue>(unrelated),
            Some(&AuthoredValue(99))
        );
        assert_eq!(current_parent(app.world(), unrelated), None);
        assert!(current_children(app.world(), unrelated).is_empty());
    }
}
