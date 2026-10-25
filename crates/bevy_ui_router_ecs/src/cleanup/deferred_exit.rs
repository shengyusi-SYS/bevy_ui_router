//! First physical cleanup freeze after an original deferred exit is released.
//!
//! The Core owner fixes disposition; the payloads remain the original physical
//! owners. Neither current routing state nor a retained registry boundary can
//! reconstruct them after publication. Ordinary native content is observed only
//! after the caller has ended the visual hold and prepared native cleanup.

use bevy_ui_router_core::integration::DeferredExitOwner;

use super::*;
use crate::presentation::FrameShell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeferredExitManifestError {
    RetryableCapacity,
    InvalidPartition,
    InvalidSource(Entity),
    #[cfg(feature = "bsn")]
    NativeContentNotPrepared(Entity),
    InvalidManifest,
}

impl DeferredExitManifestError {
    pub(crate) const fn is_retryable(self) -> bool {
        matches!(self, Self::RetryableCapacity)
    }
}

impl FrozenCleanupManifest {
    /// Freeze the actual disposal closure without consuming or reconstructing
    /// the original exit owner. The caller retains every owner on rejection.
    pub(crate) fn from_deferred_exit(
        world: &World,
        registry: &RouteEntityRegistry,
        owner: &DeferredExitOwner,
        disposable: &RealizedBranch,
        retaining: &[(&RetainedLeafProjection, &InstancePayload)],
        retiring: &[(&RetainedLeafProjection, &InstancePayload)],
        active: Option<&RealizedBranch>,
    ) -> Result<Self, DeferredExitManifestError> {
        use DeferredExitManifestError::{InvalidManifest, RetryableCapacity};

        validate_exit_partition(owner, disposable, retaining, retiring, active)?;

        // Check every source independently after publication. Shared parent
        // aliases are refreshed below by exact identity; own claims may not be.
        let mut own_entities = HashSet::new();
        validate_payload_source(
            world,
            registry,
            disposable.as_payload_ref(),
            EntityLifecycle::Leaving,
            true,
            &mut own_entities,
        )?;
        for (_, payload) in retiring {
            validate_payload_source(
                world,
                registry,
                payload.as_payload_ref(),
                EntityLifecycle::Leaving,
                true,
                &mut own_entities,
            )?;
        }
        for (_, payload) in retaining {
            validate_payload_source(
                world,
                registry,
                payload.as_payload_ref(),
                EntityLifecycle::Retained,
                false,
                &mut own_entities,
            )?;
        }
        if let Some(active) = active {
            validate_payload_source(
                world,
                registry,
                active.as_payload_ref(),
                EntityLifecycle::Active,
                false,
                &mut own_entities,
            )?;
        }

        let mut frames = Vec::new();
        frames
            .try_reserve_exact(disposable.frames().len() + retiring.len())
            .map_err(|_| RetryableCapacity)?;
        frames.extend(disposable.frames());
        frames.extend(retiring.iter().map(|(_, payload)| payload.frame()));
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(
                disposable.child_slots().len()
                    + retiring
                        .iter()
                        .map(|(_, payload)| payload.child_slots().len())
                        .sum::<usize>(),
            )
            .map_err(|_| RetryableCapacity)?;
        slots.extend(disposable.child_slots());
        slots.extend(
            retiring
                .iter()
                .flat_map(|(_, payload)| payload.child_slots()),
        );
        #[cfg(feature = "bsn")]
        for frame in &frames {
            native_cleanup_was_prepared(world, frame)?;
        }
        let claims = frames
            .iter()
            .flat_map(|frame| frame.claims())
            .chain(slots.iter().map(|slot| slot.slot_claim()))
            .cloned()
            .collect::<Vec<_>>();
        let authorized_retired = retiring
            .iter()
            .map(|(projection, _)| projection.frame())
            .collect::<Vec<_>>();
        let manifest = Self::from_leaving_frame_refs(
            world,
            registry,
            &frames,
            &slots,
            &claims,
            LeavingMountProof::DeferredExit,
            &authorized_retired,
        )
        .ok_or(InvalidManifest)?;

        // The same existing classifier proves the full observed target closure;
        // a protected child, replacement claim or escaped target cannot become
        // authorized merely because it is currently under an outgoing shell.
        let receipts = vec![None; manifest.obligations.len()];
        let observed = collect_evidence(world, &manifest.obligations, &receipts);
        if !manifest.preflight(world)
            || quarantine_disposition(&observed, &manifest.obligations).is_some()
        {
            return Err(InvalidManifest);
        }
        Ok(manifest)
    }
}

fn validate_exit_partition(
    owner: &DeferredExitOwner,
    disposable: &RealizedBranch,
    retaining: &[(&RetainedLeafProjection, &InstancePayload)],
    retiring: &[(&RetainedLeafProjection, &InstancePayload)],
    active: Option<&RealizedBranch>,
) -> Result<(), DeferredExitManifestError> {
    if !instance_partition_matches(owner.retained(), retaining)
        || !instance_partition_matches(owner.retiring(), retiring)
        || owner.outgoing().len() != disposable.frames().len() + retaining.len()
        || owner.outgoing().iter().any(|frame| {
            disposable
                .frames()
                .iter()
                .filter(|payload| payload.frame() == frame)
                .count()
                + retaining
                    .iter()
                    .filter(|(_, payload)| payload.frame().frame() == frame)
                    .count()
                != 1
        })
        || owner.cleanup_roots().len() != disposable.frames().len() + retiring.len()
        || owner.cleanup_roots().iter().any(|frame| {
            disposable
                .frames()
                .iter()
                .filter(|payload| payload.frame() == frame)
                .count()
                + retiring
                    .iter()
                    .filter(|(_, payload)| payload.frame().frame() == frame)
                    .count()
                != 1
        })
        || !owner.target().iter().eq(active
            .into_iter()
            .flat_map(|branch| branch.frames().iter().map(RealizedFrame::frame)))
    {
        return Err(DeferredExitManifestError::InvalidPartition);
    }
    Ok(())
}

impl FrozenCleanupManifest {
    /// Host loss keeps the exact original disposition partition, while the
    /// existing authority classifier observes each present or absent physical
    /// obligation. No normal mount-parent exchange is inferred from a lost host.
    pub(crate) fn from_deferred_exit_authority(
        world: &World,
        registry: &RouteEntityRegistry,
        owner: &DeferredExitOwner,
        disposable: &RealizedBranch,
        retaining: &[(&RetainedLeafProjection, &InstancePayload)],
        retiring: &[(&RetainedLeafProjection, &InstancePayload)],
        active: Option<&RealizedBranch>,
    ) -> Result<Self, DeferredExitManifestError> {
        validate_exit_partition(owner, disposable, retaining, retiring, active)?;
        let frames = disposable
            .frames()
            .iter()
            .chain(retiring.iter().map(|(_, payload)| payload.frame()));
        let mut obligations = Vec::new();
        for frame in frames {
            #[cfg(feature = "bsn")]
            native_cleanup_was_prepared(world, frame)?;
            let targets = authority_lost_frame_obligations(world, registry, frame)
                .ok_or(DeferredExitManifestError::InvalidManifest)?;
            obligations
                .try_reserve(targets.len())
                .map_err(|_| DeferredExitManifestError::RetryableCapacity)?;
            obligations.extend(targets);
        }
        for slot in disposable.child_slots().iter().chain(
            retiring
                .iter()
                .flat_map(|(_, payload)| payload.child_slots()),
        ) {
            obligations
                .try_reserve(1)
                .map_err(|_| DeferredExitManifestError::RetryableCapacity)?;
            obligations.push(authority_settlement_slot_obligation(world, registry, slot));
        }
        if obligations.iter().enumerate().any(|(index, target)| {
            obligations[..index]
                .iter()
                .any(|prior| prior.entity() == target.entity())
        }) {
            return Err(DeferredExitManifestError::InvalidPartition);
        }
        order_leaf_to_root(&mut obligations).ok_or(DeferredExitManifestError::InvalidManifest)?;
        let mount_parents = Vec::new();
        let digest = frozen_manifest_digest(&obligations, &mount_parents);
        Ok(Self {
            obligations,
            mount_parents,
            boundary_drifts: Vec::new(),
            pending_boundary_loss_claims: Vec::new(),
            digest,
        })
    }
}

fn instance_partition_matches(
    expected: &[RetainedLeafProjection],
    actual: &[(&RetainedLeafProjection, &InstancePayload)],
) -> bool {
    expected.len() == actual.len()
        && expected.iter().all(|projection| {
            actual
                .iter()
                .filter(|(candidate, _)| *candidate == projection)
                .count()
                == 1
        })
        && actual.iter().all(|(projection, payload)| {
            let frame = payload.frame();
            frame.frame() == projection.frame()
                && frame.mount_binding().matches_retained_scope(projection)
                && projection.own_slots().len() == payload.child_slots().len()
                && projection.own_slots().iter().all(|binding| {
                    payload
                        .child_slots()
                        .iter()
                        .filter(|slot| {
                            slot.parent() == binding.parent()
                                && slot.outlet() == binding.outlet()
                                && slot.slot_id() == binding.slot()
                        })
                        .count()
                        == 1
                })
        })
}

fn validate_payload_source(
    world: &World,
    registry: &RouteEntityRegistry,
    payload: PayloadRef<'_>,
    lifecycle: EntityLifecycle,
    allow_absent: bool,
    own_entities: &mut HashSet<Entity>,
) -> Result<(), DeferredExitManifestError> {
    for frame in payload.frames() {
        for claim in frame.claims() {
            let owner_matches = match claim.owner() {
                RegistryOwner::Committed(EntityOwner::FrameShell(owner)) => {
                    owner == frame.frame() && claim.entity() == frame.shell()
                }
                RegistryOwner::Committed(EntityOwner::FrameContentTarget(owner)) => {
                    owner == frame.frame()
                }
                RegistryOwner::Committed(EntityOwner::FrameOutput { frame: owner, .. }) => {
                    owner == frame.frame()
                }
                _ => false,
            };
            if !owner_matches
                || claim.lifecycle() != lifecycle
                || !own_entities.insert(claim.entity())
                || !source_claim_is_current(world, registry, claim, allow_absent)
            {
                return Err(DeferredExitManifestError::InvalidSource(claim.entity()));
            }
        }
    }
    let frames = payload.frames().iter().collect::<Vec<_>>();
    for slot in payload.child_slots() {
        let claim = slot.slot_claim();
        // An active branch can also own an empty registered-inert outlet slot.
        let role_matches = claim.lifecycle() == lifecycle
            || lifecycle == EntityLifecycle::Active
                && claim.lifecycle() == EntityLifecycle::PreparedInert;
        if !role_matches
            || !leaving_slot_has_exact_parent(&frames, slot)
            || !own_entities.insert(claim.entity())
            || !source_claim_is_current(world, registry, claim, allow_absent)
        {
            return Err(DeferredExitManifestError::InvalidSource(claim.entity()));
        }
    }
    Ok(())
}

fn source_claim_is_current(
    world: &World,
    registry: &RouteEntityRegistry,
    claim: &ExactRegistryClaim,
    allow_absent: bool,
) -> bool {
    if registry
        .record(claim.entity())
        .is_none_or(|record| record.claim() != *claim)
    {
        return false;
    }
    if world.get_entity(claim.entity()).is_err() {
        return allow_absent;
    }
    cleanup_claim_has_exact_authority(world, claim)
        && match claim.owner() {
            RegistryOwner::Committed(EntityOwner::FrameShell(frame)) => world
                .get::<FrameShell>(claim.entity())
                .is_some_and(|shell| shell.matches_frame(frame)),
            RegistryOwner::Committed(EntityOwner::FrameContentTarget(frame)) => world
                .get::<FrameContentTarget>(claim.entity())
                .is_some_and(|root| root.matches_frame(frame)),
            RegistryOwner::Committed(EntityOwner::FrameOutput {
                frame, local_index, ..
            }) => world
                .get::<FrameOutput>(claim.entity())
                .is_some_and(|output| output.matches_owner(frame, *local_index)),
            RegistryOwner::Committed(
                EntityOwner::ChildSlot { .. } | EntityOwner::HostTopSlot { .. },
            ) => true,
            _ => false,
        }
}

#[cfg(feature = "bsn")]
fn native_cleanup_was_prepared(
    world: &World,
    frame: &RealizedFrame,
) -> Result<(), DeferredExitManifestError> {
    let Some(root) = frame.native_root() else {
        return Ok(());
    };
    if world.get_entity(root).is_err() {
        return Ok(());
    }
    let content = crate::presentation::native_frame::collect_content(world, frame.frame(), root)
        .map_err(|_| DeferredExitManifestError::NativeContentNotPrepared(root))?;
    let targets = crate::presentation::native_frame::contained_cleanup_targets(frame);
    if content.len().saturating_sub(1) != targets.len()
        || content.iter().copied().skip(1).any(|entity| {
            targets
                .iter()
                .filter(|(target, parent, manifest)| {
                    *target == entity
                        && world.get::<ChildOf>(entity).map(ChildOf::parent) == Some(*parent)
                        && manifest.matches(world, entity)
                })
                .count()
                != 1
        })
    {
        return Err(DeferredExitManifestError::NativeContentNotPrepared(root));
    }
    Ok(())
}

/// Only external boundaries reach this function. Internal mount parents are
/// already exact target obligations and need no separate protected settlement.
pub(super) fn deferred_frame_mount_parent_candidate(
    world: &World,
    registry: &RouteEntityRegistry,
    frame: &RealizedFrame,
) -> Option<CleanupMountParentCandidate> {
    let mut candidate = if world.get_entity(frame.shell()).is_ok() {
        attached_frame_mount_parent_candidate(world, frame)?
    } else {
        let children = observed_children(world, frame.mount_binding().slot());
        if children.contains(&frame.shell()) {
            return None;
        }
        let mut candidate = frame_mount_parent_candidate(frame, None)?;
        candidate.children_before = children.clone();
        candidate.children_after = children;
        candidate.removed_child_was_attached = false;
        candidate
    };
    let refresh = |claim: &ExactRegistryClaim| {
        let current = registry.record(claim.entity())?.claim();
        (current.owner() == claim.owner()
            && current.seal() == claim.seal()
            && matches!(
                current.lifecycle(),
                EntityLifecycle::Active
                    | EntityLifecycle::PreparedInert
                    | EntityLifecycle::Retained
            )
            && source_claim_is_current(world, registry, &current, false))
        .then_some(current)
    };
    candidate.claim = refresh(&candidate.claim)?;
    if let Some((_, Some(claim))) = &mut candidate.supplied_parent {
        *claim = refresh(claim)?;
    }
    if let CleanupParentBinding::Structural {
        upstream: Some(binding),
    } = &mut candidate.binding
    {
        *binding.source_claim_mut() = refresh(binding.source_claim())?;
    }
    Some(candidate)
}

impl CleanupLedger {
    /// Reserve a late-sealed ledger with the full related protected closure,
    /// including empty native roots and the independently published active UI.
    /// This storage acquires no lifecycle owner until the original Core handoff.
    pub(crate) fn reserve_deferred_exit(
        world: &World,
        manifest: FrozenCleanupManifest,
        disposable: &RealizedBranch,
        retaining: &[(&RetainedLeafProjection, &InstancePayload)],
        retiring: &[(&RetainedLeafProjection, &InstancePayload)],
        active: Option<&RealizedBranch>,
    ) -> Result<PreparedCleanupLedger, CleanupLedgerReserveError> {
        let targets = manifest
            .obligations
            .iter()
            .map(CleanupObligation::entity)
            .collect::<HashSet<_>>();
        let protected_claims = active
            .into_iter()
            .flat_map(RealizedBranch::claims)
            .chain(retaining.iter().flat_map(|(_, payload)| payload.claims()));
        if protected_claims
            .into_iter()
            .any(|claim| targets.contains(&claim.entity()))
        {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts,
                manifest,
            ));
        }
        let Some(mut protected) =
            freeze_retained_invalidation_protected(world, &manifest.obligations)
        else {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts,
                manifest,
            ));
        };
        let roots = active
            .into_iter()
            .flat_map(|branch| branch.frames().iter().map(RealizedFrame::shell))
            .chain(retaining.iter().map(|(_, payload)| payload.frame().shell()))
            .collect::<Vec<_>>();
        let Some(closure) = freeze_failure_cleanup_closure(world, &roots, &[]) else {
            return Err(CleanupLedgerReserveError::new(
                CleanupLedgerReserveErrorKind::UnfinalizedBoundaryFacts,
                manifest,
            ));
        };
        let registry = world.resource::<RouteEntityRegistry>();
        for entity in closure {
            if !targets.contains(&entity)
                && !protected.iter().any(|fact| fact.observed.entity == entity)
            {
                protected.push(CleanupProtectedEntity {
                    observed: FrozenFailureCleanupObservedEntity::observe(world, registry, entity),
                    display: world.get::<Node>(entity).map(|node| node.display),
                    scope: CleanupProtectedScope::RelatedClosure,
                });
            }
        }
        append_cleanup_host_sources(world, &mut protected, disposable.frames());
        for (_, payload) in retiring.iter().chain(retaining) {
            append_cleanup_host_sources(world, &mut protected, payload.frames());
        }
        if let Some(active) = active {
            append_cleanup_host_sources(world, &mut protected, active.frames());
        }
        let mut prepared = Self::reserve(manifest)?;
        prepared.related_protected = Some(protected);
        Ok(prepared)
    }
}
