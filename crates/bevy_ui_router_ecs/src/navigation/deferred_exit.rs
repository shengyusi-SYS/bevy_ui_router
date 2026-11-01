//! Original physical payloads held between publication and first cleanup seal.

use super::*;
use crate::{
    cleanup::{DeferredExitManifestError, PreparedLeavingAssembly},
    input::native_ui_candidates::{NativeUiInputProjection, NativeUiInputProjectionFailure},
    instance::InstancePayload,
    presentation::FrameMutationTarget,
    transition::{self, RouteTransitionId, TransitionCancelReason, TransitionSessionError},
};

/// Prepared and allocated before publication. No lifecycle capability is
/// reconstructed here; Core moves its original owner into `install` in the tail.
pub(super) struct DeferredExitParts {
    pub(super) session: RouteTransitionId,
    pub(super) disposable: RealizedBranch,
    pub(super) retaining: Vec<(r3_core::RetainedLeafProjection, InstancePayload)>,
    pub(super) retiring: Vec<(r3_core::RetainedLeafProjection, InstancePayload)>,
    pub(super) successor: Option<bevy_ui_router_core::frame::RouteFrameVersion>,
    pub(super) release_targets: Vec<FrameMutationTarget>,
    pub(super) visible_claims: Vec<ExactRegistryClaim>,
}

pub(super) struct DeferredExit {
    pub(super) owner: r3_core::DeferredExitOwner,
    pub(super) parts: DeferredExitParts,
    pub(super) released: bool,
    /// Permanent rejection retains the original owner for authority transfer;
    /// it must not become an automatic retry of the same contradictory source.
    pub(super) failure: Option<DeferredExitFailure>,
}

#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "Deferred exit failures retain their exact source and original failure chain for authority settlement"
)]
pub(super) enum DeferredExitFailure {
    HostAuthority,
    MissingResources(RequiredResourceSet),
    Session(TransitionSessionError),
    ReleaseSource,
    PresentationAccess,
    #[cfg(feature = "bsn")]
    Native(crate::presentation::native_frame::NativeCleanupPreparationError),
    Manifest(DeferredExitManifestError),
    Ledger(CleanupLedgerReserveErrorKind),
    Successor,
    CleanupAlreadyInstalled,
    Core(IntegrationError),
    AuthorityResidualRejected {
        kind: r3_core::ExitResidualErrorKind,
        prior: Option<Box<DeferredExitFailure>>,
        reason: std::sync::Arc<super::deferred_exit_authority::ExitAuthorityFailure>,
    },
    NativeUiProjection {
        projection: Arc<NativeUiInputProjectionFailure>,
        release: Box<DeferredExitFailure>,
    },
}

/// One consumed adapter failure and the exact original exits it forced to
/// release. The coordinator routes AuthorityRequired outcomes to the original
/// deferred authority holder; successful exits have entered ordinary cleanup.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "The stored adapter settlement keeps the originating projection and all per-router outcomes"
)]
pub(crate) struct NativeUiProjectionFailureSettlement {
    pub(crate) projection: Arc<NativeUiInputProjectionFailure>,
    pub(crate) outcomes: Vec<(RouterHandle, DeferredExitDrive)>,
}

pub(crate) fn settle_native_ui_projection_failure(
    world: &mut World,
) -> Option<NativeUiProjectionFailureSettlement> {
    let projection = Arc::new(
        world
            .get_resource_mut::<NativeUiInputProjection>()?
            .take_failure()?,
    );
    let exits = world
        .get_resource::<RouterNavigation>()
        .into_iter()
        .flat_map(|navigation| &navigation.entries)
        .filter_map(|entry| {
            entry
                .deferred_exit
                .as_ref()
                .map(|exit| (entry.handle, exit.parts.session.clone()))
        })
        .collect::<Vec<_>>();
    let mut outcomes = Vec::with_capacity(exits.len());
    for (handle, session) in exits {
        let exact_exit = world
            .get_resource::<RouterNavigation>()
            .and_then(|navigation| {
                navigation
                    .entries
                    .iter()
                    .find(|entry| entry.handle == handle)
            })
            .and_then(|entry| entry.deferred_exit.as_ref())
            .is_some_and(|exit| exit.parts.session == session);
        if !exact_exit {
            // A callback cannot redirect this frozen cancellation to a different
            // physical owner on the same Router.
            continue;
        }
        let stopped = match transition::current_transition(world, handle) {
            Some(current) if current != session => Err(DeferredExitFailure::ReleaseSource),
            _ => {
                transition::cancel_transition(world, handle, TransitionCancelReason::AuthorityLost)
                    .map_err(DeferredExitFailure::Session)
            }
        };
        let outcome = match stopped.and_then(|()| hide_deferred_presence(world, handle)) {
            Ok(()) => drive_deferred_exit(world, handle),
            Err(error) => retain_failure(world, handle, error),
        };
        if outcome == DeferredExitDrive::AuthorityRequired
            && let Some(exit) = world
                .resource_mut::<RouterNavigation>()
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .and_then(|entry| entry.deferred_exit.as_mut())
            && let Some(release) = exit.failure.take()
        {
            exit.failure = Some(DeferredExitFailure::NativeUiProjection {
                projection: Arc::clone(&projection),
                release: Box::new(release),
            });
        }
        outcomes.push((handle, outcome));
    }
    Some(NativeUiProjectionFailureSettlement {
        projection,
        outcomes,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeferredExitDrive {
    Absent,
    Holding,
    Retryable,
    EnteredCleanup,
    /// The original holder remains installed, with its typed failure. The
    /// caller must route it to deferred-exit authority settlement, not ordinary
    /// prepublication authority handling or a fabricated cleanup batch.
    AuthorityRequired,
}

impl DeferredExitParts {
    pub(super) fn install(self, owner: r3_core::DeferredExitOwner) -> DeferredExit {
        DeferredExit {
            owner,
            parts: self,
            released: false,
            failure: None,
        }
    }

    /// The caller has stopped writers and revoked visible presence. Keeping
    /// this preparation in the original physical holder makes capacity retry
    /// preserve the first frozen native closure instead of reacquiring it.
    pub(super) fn prepare_native_cleanup(
        &mut self,
        world: &mut World,
    ) -> Result<(), DeferredExitFailure> {
        #[cfg(feature = "bsn")]
        {
            self.disposable
                .prepare_native_cleanup(world)
                .map_err(DeferredExitFailure::Native)?;
            for (_, payload) in &mut self.retiring {
                payload
                    .prepare_native_cleanup(world)
                    .map_err(DeferredExitFailure::Native)?;
            }
        }
        #[cfg(not(feature = "bsn"))]
        let _ = world;
        Ok(())
    }
}

/// Run before ordinary lifecycle work for this Router. A newly accepted request
/// first stops the old writer and seals its original cleanup responsibility.
pub(crate) fn drive_deferred_exit(world: &mut World, handle: RouterHandle) -> DeferredExitDrive {
    let Some(entry) = world
        .get_resource::<RouterNavigation>()
        .and_then(|navigation| {
            navigation
                .entries
                .iter()
                .find(|entry| entry.handle == handle)
        })
    else {
        return DeferredExitDrive::Absent;
    };
    let Some(exit) = &entry.deferred_exit else {
        return DeferredExitDrive::Absent;
    };
    if exit.failure.is_some() {
        return DeferredExitDrive::AuthorityRequired;
    }
    let session = exit.parts.session.clone();
    let released = exit.released;
    let demand = entry.engine.exit_release_demand(&exit.owner);
    let host_exact = entry
        .host
        .as_ref()
        .is_some_and(|host| current_host_binding_is_exact(world, host));
    let missing = RequiredResourceSet::missing_from(world);
    if !missing.is_empty() {
        return retain_failure(
            world,
            handle,
            DeferredExitFailure::MissingResources(missing),
        );
    }
    if !host_exact {
        return retain_failure(world, handle, DeferredExitFailure::HostAuthority);
    }
    let demand = match demand {
        Ok(demand) => demand,
        Err(error) => return retain_failure(world, handle, DeferredExitFailure::Core(error)),
    };

    if !released {
        let current = transition::current_transition(world, handle);
        if current.as_ref().is_some_and(|current| current != &session) {
            return retain_failure(
                world,
                handle,
                DeferredExitFailure::Session(TransitionSessionError::InvalidProjection),
            );
        }
        let requested = transition::pending_transition_requests(world, handle)
            .filter(|request| request.id == session);
        let superseded = matches!(demand, r3_core::ExitReleaseDemand::QueuedRequest(_));
        if !superseded
            && requested
                .as_ref()
                .is_none_or(|request| !request.release_exit)
            && transition::transition_holds_exit(world, &session)
        {
            return DeferredExitDrive::Holding;
        }
        // Explicit completion commands promise that the application's relevant
        // writers have stopped. Supersession synchronously calls cancellation.
        let stopped = if superseded {
            transition::cancel_transition(world, handle, TransitionCancelReason::Superseded)
        } else if current.is_none() {
            Ok(())
        } else if requested.as_ref().is_some_and(|request| request.finish) {
            transition::finish_transition(world, &session)
        } else {
            transition::release_transition_exit(world, &session)
        };
        if let Err(error) = stopped {
            return retain_failure(world, handle, DeferredExitFailure::Session(error));
        }
        if let Err(error) = hide_deferred_presence(world, handle) {
            return retain_failure(world, handle, error);
        }
    }

    // Callbacks above may change host or resource facts. No observation from
    // before application code authorizes the physical cleanup preparation.
    let missing = RequiredResourceSet::missing_from(world);
    if !missing.is_empty() {
        return retain_failure(
            world,
            handle,
            DeferredExitFailure::MissingResources(missing),
        );
    }
    let host_exact = world
        .resource::<RouterNavigation>()
        .entries
        .iter()
        .find(|entry| entry.handle == handle)
        .and_then(|entry| entry.host.as_ref())
        .is_some_and(|host| current_host_binding_is_exact(world, host));
    if !host_exact {
        return retain_failure(world, handle, DeferredExitFailure::HostAuthority);
    }
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            let entry = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
                .expect("installed deferred exit retains its Router entry");
            let Some(mut exit) = entry.deferred_exit.take() else {
                return DeferredExitDrive::Absent;
            };
            if entry.cleanup_ledger.is_some() {
                exit.failure = Some(DeferredExitFailure::CleanupAlreadyInstalled);
                entry.deferred_exit = Some(exit);
                return DeferredExitDrive::AuthorityRequired;
            }
            if let Err(error) = exit.parts.prepare_native_cleanup(world) {
                let retry = native_capacity_failure(&error);
                if !retry {
                    exit.failure = Some(error);
                }
                entry.deferred_exit = Some(exit);
                return if retry {
                    DeferredExitDrive::Retryable
                } else {
                    DeferredExitDrive::AuthorityRequired
                };
            }
            seal_released_exit(world, entry, exit)
        },
    )
}

fn native_capacity_failure(error: &DeferredExitFailure) -> bool {
    #[cfg(feature = "bsn")]
    if let DeferredExitFailure::Native(error) = error {
        return error.is_retryable();
    }
    let _ = error;
    false
}

fn retain_failure(
    world: &mut World,
    handle: RouterHandle,
    failure: DeferredExitFailure,
) -> DeferredExitDrive {
    if let Some(mut navigation) = world.get_resource_mut::<RouterNavigation>()
        && let Some(exit) = navigation
            .entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .and_then(|entry| entry.deferred_exit.as_mut())
    {
        exit.failure = Some(failure);
    }
    DeferredExitDrive::AuthorityRequired
}

/// Stop/cancel the application writer before calling. The targets are the exact
/// release packet frozen before publication, never rediscovered from current UI.
pub(super) fn hide_deferred_presence(
    world: &mut World,
    handle: RouterHandle,
) -> Result<(), DeferredExitFailure> {
    let (session, targets) = {
        let entry = world
            .get_resource::<RouterNavigation>()
            .and_then(|navigation| {
                navigation
                    .entries
                    .iter()
                    .find(|entry| entry.handle == handle)
            })
            .ok_or(DeferredExitFailure::ReleaseSource)?;
        let exit = entry
            .deferred_exit
            .as_ref()
            .ok_or(DeferredExitFailure::ReleaseSource)?;
        if exit.released {
            return Ok(());
        }
        let registry = world
            .get_resource::<RouteEntityRegistry>()
            .ok_or(DeferredExitFailure::ReleaseSource)?;
        if exit.parts.visible_claims.len() != exit.parts.release_targets.len()
            || exit.parts.visible_claims.iter().any(|claim| {
                exit.parts
                    .release_targets
                    .iter()
                    .filter(|target| target.entity() == claim.entity())
                    .count()
                    != 1
                    || registry
                        .record(claim.entity())
                        .is_none_or(|record| record.claim() != *claim)
                    || !crate::cleanup::cleanup_claim_has_exact_authority(world, claim)
                    || entry.active.as_ref().is_some_and(|active| {
                        active
                            .claims()
                            .any(|current| current.entity() == claim.entity())
                    })
                    || !release_parent_is_current(world, &exit.parts, claim)
            })
        {
            return Err(DeferredExitFailure::ReleaseSource);
        }
        (
            exit.parts.session.clone(),
            exit.parts.release_targets.clone(),
        )
    };
    let presentation = crate::presentation::prepare_presentation_mutation(world, &targets)
        .ok_or(DeferredExitFailure::PresentationAccess)?;
    presentation.apply();
    world
        .resource_mut::<RouterNavigation>()
        .entries
        .iter_mut()
        .find(|entry| entry.handle == handle)
        .unwrap()
        .deferred_exit
        .as_mut()
        .unwrap()
        .released = true;
    transition::release_transition_presence(world, &session);
    Ok(())
}

fn release_parent_is_current(
    world: &World,
    parts: &DeferredExitParts,
    claim: &ExactRegistryClaim,
) -> bool {
    let frames = parts
        .disposable
        .frames()
        .iter()
        .chain(parts.retaining.iter().map(|(_, payload)| payload.frame()))
        .chain(parts.retiring.iter().map(|(_, payload)| payload.frame()));
    let frame_parent = frames
        .flat_map(RealizedFrame::cleanup_members)
        .find_map(|(current, parent)| (current == claim).then_some(parent));
    let parent = frame_parent.or_else(|| {
        parts
            .disposable
            .child_slots()
            .iter()
            .chain(
                parts
                    .retaining
                    .iter()
                    .flat_map(|(_, payload)| payload.child_slots()),
            )
            .chain(
                parts
                    .retiring
                    .iter()
                    .flat_map(|(_, payload)| payload.child_slots()),
            )
            .find_map(|slot| (slot.slot_claim() == claim).then_some(slot.anchor()))
    });
    parent.is_some_and(|parent| {
        world.get::<ChildOf>(claim.entity()).map(ChildOf::parent) == Some(parent)
            && world.get::<Children>(parent).is_some_and(|children| {
                children
                    .iter()
                    .filter(|entity| **entity == claim.entity())
                    .count()
                    == 1
            })
    })
}

fn seal_released_exit(
    world: &mut World,
    entry: &mut RouterEntry,
    mut exit: DeferredExit,
) -> DeferredExitDrive {
    let retaining = exit
        .parts
        .retaining
        .iter()
        .map(|(projection, payload)| (projection, payload))
        .collect::<Vec<_>>();
    let retiring = exit
        .parts
        .retiring
        .iter()
        .map(|(projection, payload)| (projection, payload))
        .collect::<Vec<_>>();
    let manifest = match FrozenCleanupManifest::from_deferred_exit(
        world,
        world.resource::<RouteEntityRegistry>(),
        &exit.owner,
        &exit.parts.disposable,
        &retaining,
        &retiring,
        entry.active.as_ref(),
    ) {
        Ok(manifest) => manifest,
        Err(error) => {
            let retry = error.is_retryable();
            if !retry {
                exit.failure = Some(DeferredExitFailure::Manifest(error));
            }
            entry.deferred_exit = Some(exit);
            return if retry {
                DeferredExitDrive::Retryable
            } else {
                DeferredExitDrive::AuthorityRequired
            };
        }
    };
    let rebase = match exit.parts.successor.as_ref() {
        Some(successor) => {
            let prepared = entry.active.as_ref().and_then(|active| {
                let mut matches = active
                    .frames()
                    .iter()
                    .enumerate()
                    .filter(|(_, frame)| frame.frame() == successor);
                let (index, frame) = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                manifest
                    .prepare_successor_mount_rebase(world, frame)
                    .map(|rebase| (index, rebase))
            });
            let Some(rebase) = prepared else {
                exit.failure = Some(DeferredExitFailure::Successor);
                entry.deferred_exit = Some(exit);
                return DeferredExitDrive::AuthorityRequired;
            };
            Some(rebase)
        }
        None => None,
    };
    let frames = exit
        .parts
        .disposable
        .frames()
        .iter()
        .chain(retiring.iter().map(|(_, payload)| payload.frame()))
        .collect::<Vec<_>>();
    let slots = exit
        .parts
        .disposable
        .child_slots()
        .iter()
        .chain(
            retiring
                .iter()
                .flat_map(|(_, payload)| payload.child_slots()),
        )
        .collect::<Vec<_>>();
    let Some(assembly) = PreparedLeavingAssembly::reserve(&manifest, &frames, &slots) else {
        // Source equality was established by the manifest builder; this reserve
        // only allocates the same original physical receivers.
        entry.deferred_exit = Some(exit);
        return DeferredExitDrive::Retryable;
    };
    let logical = CleanupManifest::new(
        exit.owner.cleanup_roots().to_vec(),
        manifest.digest(),
        manifest.len(),
    );
    let reserved = match CleanupLedger::reserve_deferred_exit(
        world,
        manifest,
        &exit.parts.disposable,
        &retaining,
        &retiring,
        entry.active.as_ref(),
    ) {
        Ok(reserved) => reserved,
        Err(error) => {
            let kind = error.kind();
            let retry = kind == CleanupLedgerReserveErrorKind::RetryableCapacity;
            if !retry {
                exit.failure = Some(DeferredExitFailure::Ledger(kind));
            }
            entry.deferred_exit = Some(exit);
            return if retry {
                DeferredExitDrive::Retryable
            } else {
                DeferredExitDrive::AuthorityRequired
            };
        }
    };
    // Publication already reserved these additions, but an explicit local
    // reserve protects the callback's allocation-free contract on retries.
    let Some(_capacity) =
        entry
            .retained
            .prepare_change_with_selected(&[], &[], exit.parts.retaining.len())
    else {
        entry.deferred_exit = Some(exit);
        return DeferredExitDrive::Retryable;
    };
    let mut retiring_receiver = Vec::new();
    if retiring_receiver
        .try_reserve_exact(exit.parts.retiring.len())
        .is_err()
    {
        entry.deferred_exit = Some(exit);
        return DeferredExitDrive::Retryable;
    }
    let DeferredExit {
        owner,
        parts,
        released,
        failure,
    } = exit;
    let mut parts = Some(parts);
    let cleanup_slot = &mut entry.cleanup_ledger;
    let retained_slot = &mut entry.retained;
    let active_slot = &mut entry.active;
    let result = entry
        .engine
        .begin_exit_cleanup(owner, logical, |batch, owners| {
            let parts = parts
                .take()
                .expect("the original physical exit is delivered once");
            debug_assert_eq!(parts.retaining.len(), owners.len());
            for (_, payload) in parts.retiring {
                retiring_receiver.push(payload);
            }
            let leaving = assembly.assemble(parts.disposable, retiring_receiver);
            let cleanup =
                CleanupLedger::activate_leaving_forest(reserved.bind_batch(batch), leaving);
            if let Some((index, rebase)) = rebase {
                active_slot
                    .as_mut()
                    .expect("prepared successor remains active")
                    .apply_frame_mount_rebase(index, rebase);
            }
            *cleanup_slot = Some(cleanup);
            for ((projection, payload), owner) in parts.retaining.into_iter().zip(owners) {
                retained_slot.install_published(projection, Some(owner), payload);
            }
        });
    match result {
        Ok(()) => DeferredExitDrive::EnteredCleanup,
        Err(error) => {
            let (owner, _manifest, error) = error.into_parts();
            entry.deferred_exit = Some(DeferredExit {
                owner,
                parts: parts.expect("rejected Core handoff does not consume the physical owner"),
                released,
                failure: failure.or(Some(DeferredExitFailure::Core(error))),
            });
            DeferredExitDrive::AuthorityRequired
        }
    }
}
