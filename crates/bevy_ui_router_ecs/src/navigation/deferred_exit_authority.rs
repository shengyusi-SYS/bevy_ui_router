//! Host loss while the original outgoing disposition has not yet been sealed.

use super::deferred_exit::{DeferredExit, DeferredExitFailure, DeferredExitParts};
use super::post_commit::{
    FrozenRetiredHostEdge, PostCommitAcceptedInertEvidence, PreparedPostCommitPhysicalOwner,
    RetiredHostEdgeFreezePermanentEvidence,
};
use super::*;
use crate::cleanup::{DeferredExitManifestError, PreparedLeavingAssembly};
use crate::transition::{self, TransitionCancelReason};

#[derive(Debug)]
pub(super) enum ExitAuthorityFailure {
    RequiredRegistryMissing,
    EntryOwnerMissing,
    RetainedInventory,
    Native(DeferredExitFailure),
    Manifest(DeferredExitManifestError),
    Ledger(CleanupLedgerReserveErrorKind),
    Original(PostCommitOriginalFreezeFailure),
    Authority(PostCommitAuthorityFreezePermanentEvidence),
    HostEdge(RetiredHostEdgeFreezePermanentEvidence),
    Isolation(authority_isolation::PostCommitInputIsolationFailure),
    Core(r3_core::ExitAuthorityBeginErrorKind),
    Residual(r3_core::ExitResidualErrorKind),
}

/// The original Core AuthorityAccepted source and every physical owner are
/// installed together before retained-plan preparation may run.
pub(super) struct AcceptedExit {
    source: PostCommitAuthorityPreparationSource,
    physical: PreparedPostCommitPhysicalOwner,
    retained: Vec<r3_core::RetainedLeafOwner>,
}

pub(super) struct ExitResidual {
    core: Option<r3_core::DeferredExitResidual>,
    parts: DeferredExitParts,
    _original_failure: Option<DeferredExitFailure>,
    _cause: PostCommitAuthorityCause,
    _failure: ExitAuthorityFailure,
}

#[derive(bevy_ecs::resource::Resource, Default)]
struct RetiredExitResiduals {
    owners: Vec<AuthorityProtocolResidualOwner>,
}

struct PreparedExitAuthority {
    normal: PreparedCleanupLedger,
    assembly: PreparedLeavingAssembly,
    original_freeze: FrozenOriginalCleanupLedgerTransfer,
    authority: PreparedPostCommitAuthorityLedger,
    edge: FrozenRetiredHostEdge,
    isolation: authority_isolation::PreparedPostCommitInputIsolation,
    boundaries: Option<crate::registry::PreparedRetainedBoundaries>,
    retained_change: crate::retained::PreparedRetainedChange,
    retained_payloads: Vec<crate::instance::InstancePayload>,
    retiring_payloads: Vec<crate::instance::InstancePayload>,
    manifest: CleanupManifest,
}

enum Preparation {
    Ready(Box<PreparedExitAuthority>),
    Retryable,
    Residual(ExitAuthorityFailure),
}

pub(crate) fn drive_deferred_exit_authority(world: &mut World, handle: RouterHandle) {
    // Application code runs with the installed Router visible. No observation
    // made before cancellation authorizes the following physical preparation.
    let _ = transition::cancel_transition(world, handle, TransitionCancelReason::AuthorityLost);
    if super::deferred_exit::hide_deferred_presence(world, handle).is_err() {
        // Despawning the host can remove the exact release targets before the
        // ordinary presentation mutation runs. Their absence is sufficient to
        // retire this visual hint; it is not a cleanup receipt.
        let absent = world
            .get_resource::<RouterNavigation>()
            .and_then(|navigation| {
                navigation
                    .entries
                    .iter()
                    .find(|entry| entry.handle == handle)
            })
            .and_then(|entry| entry.deferred_exit.as_ref())
            .filter(|exit| {
                exit.parts
                    .visible_claims
                    .iter()
                    .all(|claim| world.get_entity(claim.entity()).is_err())
            })
            .map(|exit| exit.parts.session.clone());
        if let Some(session) = absent {
            transition::release_transition_presence(world, &session);
        }
    }
    world.resource_scope(
        |world, mut navigation: bevy_ecs::change_detection::Mut<RouterNavigation>| {
            let Some(entry) = navigation
                .entries
                .iter_mut()
                .find(|entry| entry.handle == handle)
            else {
                return;
            };
            let Some(mut exit) = entry.deferred_exit.take() else {
                archive_absent_exit_residual(world, entry);
                return;
            };
            if matches!(
                exit.failure,
                Some(DeferredExitFailure::AuthorityResidualRejected { .. })
            ) {
                entry.deferred_exit = Some(exit);
                return;
            }
            let cause = entry
                .host
                .as_ref()
                .and_then(|host| post_commit::post_commit_authority_loss_cause(world, host))
                .unwrap_or(PostCommitAuthorityCause::RequiredResourcesLost);
            match prepare(world, entry, &mut exit) {
                Preparation::Ready(prepared) => begin(world, entry, exit, cause, *prepared),
                Preparation::Retryable => entry.deferred_exit = Some(exit),
                Preparation::Residual(failure) => {
                    retire_to_residual(world, entry, exit, cause, failure)
                }
            }
            archive_absent_exit_residual(world, entry);
        },
    );
}

fn prepare(world: &mut World, entry: &mut RouterEntry, exit: &mut DeferredExit) -> Preparation {
    if world.get_resource::<RouteEntityRegistry>().is_none() {
        return Preparation::Residual(ExitAuthorityFailure::RequiredRegistryMissing);
    }
    if entry.active.is_none()
        || entry.host.is_none()
        || entry.cleanup_ledger.is_some()
        || entry.post_commit_compound.is_some()
    {
        return Preparation::Residual(ExitAuthorityFailure::EntryOwnerMissing);
    }
    let Some(inventory) = entry.retained.inventory() else {
        return Preparation::Residual(ExitAuthorityFailure::RetainedInventory);
    };
    let projections = inventory
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    let Some(change) = entry.retained.prepare_all(&projections) else {
        return Preparation::Retryable;
    };
    if let Err(error) = exit.parts.prepare_native_cleanup(world) {
        #[cfg(feature = "bsn")]
        if matches!(&error, DeferredExitFailure::Native(native) if native.is_retryable()) {
            return Preparation::Retryable;
        }
        return Preparation::Residual(ExitAuthorityFailure::Native(error));
    }
    #[cfg(feature = "bsn")]
    {
        let native = (|| {
            entry
                .active
                .as_mut()
                .unwrap()
                .prepare_native_cleanup(world)?;
            for (_, payload) in &mut exit.parts.retaining {
                payload.prepare_native_cleanup(world)?;
            }
            for (_, payload) in change.selected_mut(&mut entry.retained) {
                payload.prepare_native_cleanup(world)?;
            }
            Ok::<(), crate::presentation::native_frame::NativeCleanupPreparationError>(())
        })();
        if let Err(error) = native {
            if error.is_retryable() {
                return Preparation::Retryable;
            }
            return Preparation::Residual(ExitAuthorityFailure::Native(
                DeferredExitFailure::Native(error),
            ));
        }
    }
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
    let registry = world.resource::<RouteEntityRegistry>();
    let manifest = match FrozenCleanupManifest::from_deferred_exit_authority(
        world,
        registry,
        &exit.owner,
        &exit.parts.disposable,
        &retaining,
        &retiring,
        entry.active.as_ref(),
    ) {
        Ok(manifest) => manifest,
        Err(error) if error.is_retryable() => return Preparation::Retryable,
        Err(error) => return Preparation::Residual(ExitAuthorityFailure::Manifest(error)),
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
        return Preparation::Retryable;
    };
    let logical = CleanupManifest::new(
        exit.owner.cleanup_roots().to_vec(),
        manifest.digest(),
        manifest.len(),
    );
    let normal = match CleanupLedger::reserve(manifest) {
        Ok(normal) => normal,
        Err(error) if error.kind() == CleanupLedgerReserveErrorKind::RetryableCapacity => {
            return Preparation::Retryable;
        }
        Err(error) => return Preparation::Residual(ExitAuthorityFailure::Ledger(error.kind())),
    };
    let original_freeze = match normal.freeze_unbound_post_commit_source(world) {
        Ok(freeze) => freeze,
        Err(PostCommitOriginalFreezeFailure::RetryableCapacity) => return Preparation::Retryable,
        Err(error) => return Preparation::Residual(ExitAuthorityFailure::Original(error)),
    };
    let host = entry.host.as_ref().unwrap();
    let edge = match FrozenRetiredHostEdge::freeze(world, host) {
        Ok(edge) => edge,
        Err(error) => return Preparation::Residual(ExitAuthorityFailure::HostEdge(error)),
    };
    let mut authority_retained = change.selected(&entry.retained).collect::<Vec<_>>();
    authority_retained.extend(retaining);
    let authority = match PreparedPostCommitAuthorityLedger::freeze_and_reserve(
        world,
        entry.active.as_ref().unwrap(),
        host,
        &original_freeze,
        &authority_retained,
    ) {
        Ok(authority) => authority,
        Err(PostCommitAuthorityFreezeFailure::RetryableCapacity) => return Preparation::Retryable,
        Err(PostCommitAuthorityFreezeFailure::Permanent(error)) => {
            return Preparation::Residual(ExitAuthorityFailure::Authority(error));
        }
    };
    let authority_projections = authority_retained
        .iter()
        .map(|(projection, _)| (*projection).clone())
        .collect::<Vec<_>>();
    let boundaries = if authority_projections.is_empty() {
        None
    } else {
        match registry.prepare_retained_boundaries(&[], &authority_projections) {
            Ok(boundaries) => Some(boundaries),
            Err(_) => return Preparation::Residual(ExitAuthorityFailure::RetainedInventory),
        }
    };
    let isolation = match prepare_post_commit_input_isolation(
        world,
        entry.active.as_ref().unwrap(),
        &original_freeze,
        &authority,
    ) {
        Ok(isolation) => isolation,
        Err(authority_isolation::PostCommitInputIsolationFailure::RetryableCapacity) => {
            return Preparation::Retryable;
        }
        Err(error) => return Preparation::Residual(ExitAuthorityFailure::Isolation(error)),
    };
    let mut retained_payloads = Vec::new();
    let mut retiring_payloads = Vec::new();
    if retained_payloads
        .try_reserve_exact(authority_retained.len())
        .is_err()
        || retiring_payloads.try_reserve_exact(retiring.len()).is_err()
    {
        return Preparation::Retryable;
    }
    Preparation::Ready(Box::new(PreparedExitAuthority {
        normal,
        assembly,
        original_freeze,
        authority,
        edge,
        isolation,
        boundaries,
        retained_change: change,
        retained_payloads,
        retiring_payloads,
        manifest: logical,
    }))
}

fn begin(
    world: &mut World,
    entry: &mut RouterEntry,
    exit: DeferredExit,
    cause: PostCommitAuthorityCause,
    mut prepared: PreparedExitAuthority,
) {
    let live = prepared.retained_change.take_owners(&mut entry.retained);
    let (host, lease) = entry
        .host
        .take()
        .expect("prepared exit has its original host")
        .into_retired();
    let active = entry
        .active
        .take()
        .expect("prepared exit has its published active payload");
    let DeferredExit {
        owner,
        parts,
        released,
        failure,
    } = exit;
    let mut originals = Some((parts, active, host));
    let mut holder = Box::new(PostCommitCompoundHolder::DeferredExitAccepted(Box::new(
        None,
    )));
    let destination = &mut entry.post_commit_compound;
    let cache = &mut entry.retained;
    let result = entry.engine.begin_exit_authority_transfer(
        owner,
        lease,
        prepared.manifest,
        cause,
        live,
        |batch, source, owners| {
            let (parts, active, host) = originals
                .take()
                .expect("the original physical exit moves once");
            for (_, payload) in parts.retiring {
                prepared.retiring_payloads.push(payload);
            }
            let leaving = prepared
                .assembly
                .assemble(parts.disposable, prepared.retiring_payloads);
            let original =
                CleanupLedger::activate_leaving_forest(prepared.normal.bind_batch(batch), leaving);
            prepared
                .retained_payloads
                .extend(prepared.retained_change.into_payloads(cache));
            prepared
                .retained_payloads
                .extend(parts.retaining.into_iter().map(|(_, payload)| payload));
            let physical = PreparedPostCommitPhysicalOwner {
                original,
                original_freeze: prepared.original_freeze,
                authority: prepared.authority,
                active,
                retained: prepared.retained_payloads,
                retained_boundaries: prepared.boundaries,
                host,
                edge: prepared.edge,
                input_isolation: Some(prepared.isolation),
            };
            let PostCommitCompoundHolder::DeferredExitAccepted(slot) = holder.as_mut() else {
                unreachable!("the complete accepted-exit holder was allocated before the tail")
            };
            **slot = Some(AcceptedExit {
                source,
                physical,
                retained: owners,
            });
            *destination = Some(holder);
        },
    );
    if let Err(error) = result {
        let (owner, lease, _manifest, owners, kind) = error.into_parts();
        assert!(
            entry.retained.restore_owners(owners).is_ok(),
            "rejected exit transfer returns every original cache owner"
        );
        let (parts, active, host) =
            originals.expect("rejection retains all original physical payloads");
        entry.host = Some(host.restore(lease));
        entry.active = Some(active);
        retire_to_residual(
            world,
            entry,
            DeferredExit {
                owner,
                parts,
                released,
                failure,
            },
            cause,
            ExitAuthorityFailure::Core(kind),
        );
    }
}

pub(super) fn resume_accepted(
    world: &mut World,
    entry: &mut RouterEntry,
    accepted: AcceptedExit,
) -> Box<PostCommitCompoundHolder> {
    let AcceptedExit {
        source,
        physical,
        retained,
    } = accepted;
    let plan = entry
        .engine
        .prepare_retained_post_commit_authority_plan(&source, &retained.iter().collect::<Vec<_>>());
    let plan = match plan {
        Ok(plan) => plan,
        Err(error) => {
            // Retain the complete source, including unattached original owners.
            return Box::new(PostCommitCompoundHolder::DeferredExitRejected {
                owner: Box::new(AcceptedExit {
                    source,
                    physical,
                    retained,
                }),
                error,
            });
        }
    };
    let manifest = CleanupManifest::new(
        plan.logical_roots().to_vec(),
        physical.authority.manifest_digest(),
        physical.authority.obligation_count(),
    );
    match entry
        .engine
        .prepare_post_commit_authority_transfer_with_retention(source, plan, manifest, retained)
    {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, .. } => {
            post_commit::resume_prepared_post_commit_compound(world, entry, owner, physical)
        }
        PostCommitAuthorityPreparationOutcome::Mismatch { source, evidence } => {
            Box::new(PostCommitCompoundHolder::AcceptedInert {
                source,
                physical: Box::new(physical),
                evidence: Box::new(PostCommitAcceptedInertEvidence::Receiver(evidence)),
            })
        }
        PostCommitAuthorityPreparationOutcome::Retryable { source, evidence } => {
            Box::new(PostCommitCompoundHolder::AcceptedInert {
                source,
                physical: Box::new(physical),
                evidence: Box::new(PostCommitAcceptedInertEvidence::Retryable(evidence)),
            })
        }
        PostCommitAuthorityPreparationOutcome::Permanent { source, evidence } => {
            Box::new(PostCommitCompoundHolder::AcceptedInert {
                source,
                physical: Box::new(physical),
                evidence: Box::new(PostCommitAcceptedInertEvidence::Permanent(evidence)),
            })
        }
    }
}

fn retire_to_residual(
    world: &mut World,
    entry: &mut RouterEntry,
    exit: DeferredExit,
    cause: PostCommitAuthorityCause,
    failure: ExitAuthorityFailure,
) {
    let preparation = prepare_authority_isolation_targets(
        world,
        exit.parts
            .disposable
            .claims()
            .chain(
                exit.parts
                    .retaining
                    .iter()
                    .flat_map(|(_, payload)| payload.claims()),
            )
            .chain(
                exit.parts
                    .retiring
                    .iter()
                    .flat_map(|(_, payload)| payload.claims()),
            )
            .chain(entry.active.iter().flat_map(RealizedBranch::claims))
            .chain(
                entry
                    .retained
                    .payloads()
                    .flat_map(|payload| payload.claims()),
            )
            .chain(entry.host.iter().map(|host| &host.top_slot_claim)),
        exit.parts
            .visible_claims
            .iter()
            .map(ExactRegistryClaim::entity)
            .chain(
                entry
                    .active
                    .iter()
                    .flat_map(|active| active.live_interactive_entities(world)),
            )
            .chain(
                entry
                    .retained
                    .payloads()
                    .flat_map(|payload| payload.live_interactive_entities(world)),
            ),
        0,
    );
    let (isolation, contradiction) = match preparation {
        AuthorityIsolationPreparation::Ready(isolation) => (isolation, None),
        AuthorityIsolationPreparation::PermanentContradiction {
            contradiction,
            fail_closed,
        } => (fail_closed, Some(contradiction)),
        AuthorityIsolationPreparation::RetryableCapacity => {
            entry.deferred_exit = Some(exit);
            return;
        }
    };
    let Some(inventory) = entry.retained.inventory() else {
        entry.deferred_exit = Some(exit);
        return;
    };
    let projections = inventory
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    let Some(mut change) = entry.retained.prepare_all(&projections) else {
        entry.deferred_exit = Some(exit);
        return;
    };
    let Some(host) = entry.host.take() else {
        entry.deferred_exit = Some(exit);
        return;
    };
    let (host, lease) = host.into_retired();
    let owners = change.take_owners(&mut entry.retained);
    let DeferredExit {
        owner,
        parts,
        released,
        failure: original_failure,
    } = exit;
    let mut original = Some(AuthorityProtocolResidualOwner {
        back_target: None,
        kind: AuthorityProtocolResidualKind::DeferredExit(Box::new(ExitResidual {
            core: None,
            parts,
            _original_failure: original_failure,
            _cause: cause,
            _failure: failure,
        })),
        isolation_contradiction: contradiction,
        active: entry.active.take(),
        pending: None,
        failed: None,
        failure_cleanup_completion: None,
        failure_cleanup_ledger: None,
        cleanup_settlement_ledger: None,
        failure_cleanup_permanent_authority: None,
        retained: None,
        retained_cleanup: None,
        host: Some(host),
    });
    let destination = &mut entry.authority_protocol_residual;
    let retained = &mut entry.retained;
    // Pointer isolation performs queries and allocates its access set. Complete
    // it before the Core terminal tail; rejection keeps the original owner inert.
    isolation.apply(world);
    let AuthorityProtocolResidualKind::DeferredExit(exit) = &original.as_ref().unwrap().kind else {
        unreachable!("prepared residual keeps its original visual scope")
    };
    release_inert_presence(world, &exit.parts);
    let result =
        entry
            .engine
            .retire_exit_authority_to_residual(owner, lease, owners, cause, |core| {
                let mut residual = original
                    .take()
                    .expect("the complete physical residual was allocated before the tail");
                let AuthorityProtocolResidualKind::DeferredExit(exit) = &mut residual.kind else {
                    unreachable!("the original exit retains its typed residual receiver")
                };
                exit.core = Some(core);
                residual.retained = Some(std::mem::take(retained));
                *destination = Some(residual);
            });
    if let Err(error) = result {
        let (owner, lease, owners, kind) = error.into_parts();
        assert!(
            entry.retained.restore_owners(owners).is_ok(),
            "rejected residual preserves original cache owners"
        );
        let mut residual = original.expect("rejected residual retains physical scope");
        entry.host = Some(residual.host.take().unwrap().restore(lease));
        entry.active = residual.active.take();
        let AuthorityProtocolResidualKind::DeferredExit(exit) = residual.kind else {
            unreachable!("rejection preserves the original typed exit receiver")
        };
        entry.deferred_exit = Some(DeferredExit {
            owner,
            parts: exit.parts,
            released,
            failure: Some(DeferredExitFailure::AuthorityResidualRejected {
                kind,
                prior: exit._original_failure.map(Box::new),
                reason: std::sync::Arc::new(exit._failure),
            }),
        });
    }
}

fn release_inert_presence(world: &mut World, parts: &DeferredExitParts) {
    let inert = parts.visible_claims.iter().all(|claim| {
        if world.get_entity(claim.entity()).is_err() {
            return true;
        }
        let exact = cleanup_claim_has_exact_authority(world, claim)
            && world
                .get_resource::<RouteEntityRegistry>()
                .is_none_or(|registry| {
                    registry
                        .record(claim.entity())
                        .is_some_and(|record| record.claim() == *claim)
                });
        exact
            && (world.get::<bevy_camera::visibility::Visibility>(claim.entity())
                == Some(&bevy_camera::visibility::Visibility::Hidden)
                || world
                    .get::<bevy_ui::Node>(claim.entity())
                    .is_some_and(|node| node.display == bevy_ui::Display::None))
    });
    if inert {
        transition::release_transition_presence(world, &parts.session);
    }
}

/// Only an observed absent original scope leaves the foreground blocker. Its
/// original capabilities remain in a private archive, never reused as receipts.
pub(super) fn archive_absent_exit_residual(world: &mut World, entry: &mut RouterEntry) {
    let Some(residual) = entry.authority_protocol_residual.as_ref() else {
        return;
    };
    let AuthorityProtocolResidualKind::DeferredExit(exit) = &residual.kind else {
        return;
    };
    let claims = exit
        .parts
        .disposable
        .claims()
        .chain(
            exit.parts
                .retaining
                .iter()
                .flat_map(|(_, payload)| payload.claims()),
        )
        .chain(
            exit.parts
                .retiring
                .iter()
                .flat_map(|(_, payload)| payload.claims()),
        )
        .chain(residual.active.iter().flat_map(RealizedBranch::claims))
        .chain(
            residual
                .retained
                .iter()
                .flat_map(|retained| retained.payloads())
                .flat_map(|payload| payload.claims()),
        )
        .chain(residual.host.iter().map(|host| &host.top_slot_claim));
    if claims
        .into_iter()
        .any(|claim| world.get_entity(claim.entity()).is_ok())
    {
        return;
    }
    #[cfg(feature = "bsn")]
    {
        let frames = exit
            .parts
            .disposable
            .frames()
            .iter()
            .chain(
                exit.parts
                    .retaining
                    .iter()
                    .map(|(_, payload)| payload.frame()),
            )
            .chain(
                exit.parts
                    .retiring
                    .iter()
                    .map(|(_, payload)| payload.frame()),
            )
            .chain(residual.active.iter().flat_map(|active| active.frames()))
            .chain(
                residual
                    .retained
                    .iter()
                    .flat_map(|retained| retained.payloads())
                    .map(|payload| payload.frame()),
            );
        if frames
            .flat_map(crate::presentation::native_frame::contained_cleanup_targets)
            .any(|(entity, _, _)| world.get_entity(entity).is_ok())
        {
            return;
        }
    }
    let session = exit.parts.session.clone();
    transition::release_transition_presence(world, &session);
    world.init_resource::<RetiredExitResiduals>();
    let mut archive = world.resource_mut::<RetiredExitResiduals>();
    if archive.owners.try_reserve(1).is_err() {
        return;
    }
    archive
        .owners
        .push(entry.authority_protocol_residual.take().unwrap());
}
