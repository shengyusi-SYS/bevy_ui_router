//! Post-publication compound cleanup progression and retired host settlement.

use super::authority_isolation::{
    FrozenPostCommitInputIsolation, PostCommitInputIsolationFailure,
    PreparedPostCommitInputIsolation,
};
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FrozenRetiredHostEdge {
    host: Entity,
    top_slot: Entity,
    top_slot_claim: ExactRegistryClaim,
    registration_incarnation: u64,
    host_present: bool,
    host_non_children_manifest: Option<ComponentManifest>,
    host_parent: Option<Entity>,
    host_ancestry: Vec<Entity>,
    children_before: Vec<Entity>,
    children_after: Vec<Entity>,
    top_parent_before: Option<Entity>,
    obligation_binding: u64,
    physical_source_binding: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetiredHostEdgePhysicalTerminal {
    Detached,
    HostAbsent,
    EdgeAlreadyAbsent,
}

struct RetiredHostEdgePhysicalProof {
    terminal: RetiredHostEdgePhysicalTerminal,
    binding: u64,
}

enum PostCommitAnyPhysicalProof {
    Cleanup(PostCommitPhysicalSettlementProof),
    HostEdge(RetiredHostEdgePhysicalProof),
}

pub(super) struct PreparedPostCommitPhysicalOwner {
    pub(super) original: CleanupLedger,
    pub(super) original_freeze: FrozenOriginalCleanupLedgerTransfer,
    pub(super) authority: PreparedPostCommitAuthorityLedger,
    pub(super) active: RealizedBranch,
    pub(super) retained: Vec<crate::instance::InstancePayload>,
    pub(super) retained_boundaries: Option<crate::registry::PreparedRetainedBoundaries>,
    pub(super) host: RetiredHostBinding,
    pub(super) edge: FrozenRetiredHostEdge,
    pub(super) input_isolation: Option<PreparedPostCommitInputIsolation>,
}

pub(super) struct PostCommitCompoundPhysicalOwner {
    original: CleanupLedger,
    original_freeze: FrozenOriginalCleanupLedgerTransfer,
    authority: CleanupLedger,
    partition: FrozenPostCommitAuthorityPartition,
    active: RealizedBranch,
    retained: Vec<crate::instance::InstancePayload>,
    retained_boundaries: Option<crate::registry::PreparedRetainedBoundaries>,
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    input_isolation: FrozenPostCommitInputIsolation,
    original_cursor: usize,
    authority_non_top_cursor: usize,
    settled_authority: Vec<Entity>,
    edge_terminal: Option<RetiredHostEdgePhysicalTerminal>,
    top_settled: bool,
}

pub(super) struct ActivePostCommitCompoundOwner {
    core: PostCommitCompoundBatchOwner,
    physical: PostCommitCompoundPhysicalOwner,
}

pub(super) struct PostCommitSettlementRecordInert {
    attempt: bevy_ui_router_core::integration::TrustedPostCommitSettlementAttempt,
    proof: PostCommitAnyPhysicalProof,
    physical: PostCommitCompoundPhysicalOwner,
    evidence: PostCommitSettlementTraceMismatchEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RetiredHostEdgeFreezePermanentEvidence {
    InvalidBinding,
    InvalidRelationship,
    RelationshipCycle,
}

pub(super) enum PostCommitPreAcceptInertEvidence {
    ReceiptObservationMismatch,
    Original(PostCommitOriginalFreezePermanentEvidence),
    EntryOwnerMissing,
    RetainedInventory,
    Isolation(PostCommitInputIsolationFailure),
    #[cfg(feature = "bsn")]
    NativeCleanup(crate::presentation::native_frame::NativeCleanupPreparationError),
    RetiredHostEdge(RetiredHostEdgeFreezePermanentEvidence),
    Authority(PostCommitAuthorityFreezePermanentEvidence),
    MissingNormalBatch,
    CoreReceiver {
        expected: PostCommitCleanupReceiverView,
        observed: PostCommitCleanupObservedView,
    },
}

pub(super) struct PostCommitPreAcceptPhysicalOwner {
    original: CleanupLedger,
    original_freeze: Option<FrozenOriginalCleanupLedgerTransfer>,
    authority: Option<PreparedPostCommitAuthorityLedger>,
    edge: Option<FrozenRetiredHostEdge>,
}

pub(super) enum PostCommitCompoundHolder {
    DeferredExitAccepted(Box<Option<super::deferred_exit_authority::AcceptedExit>>),
    DeferredExitRejected {
        owner: Box<super::deferred_exit_authority::AcceptedExit>,
        error: IntegrationError,
    },
    PreAcceptInert {
        physical: PostCommitPreAcceptPhysicalOwner,
        evidence: PostCommitPreAcceptInertEvidence,
    },
    AcceptedInert {
        source: PostCommitAuthorityPreparationSource,
        physical: Box<PreparedPostCommitPhysicalOwner>,
        evidence: Box<PostCommitAcceptedInertEvidence>,
    },
    PreparedInert {
        owner: PreparedPostCommitAuthorityTransfer,
        physical: PreparedPostCommitPhysicalOwner,
        evidence: PostCommitPlanBindFailure,
    },
    BoundInert {
        owner: PreparedPostCommitAuthorityTransfer,
        physical: Box<PostCommitCompoundPhysicalOwner>,
        evidence: Box<PostCommitBoundInertEvidence>,
    },
    Active(ActivePostCommitCompoundOwner),
    PermitMismatchInert {
        core: PostCommitCompoundBatchOwner,
        physical: PostCommitCompoundPhysicalOwner,
        evidence: PostCommitSettlementTraceMismatchEvidence,
    },
    PermitInert {
        attempt: bevy_ui_router_core::integration::TrustedPostCommitSettlementAttempt,
        physical: PostCommitCompoundPhysicalOwner,
        failure: PostCommitAuthoritySettlementFailure,
    },
    SettlementInert(PostCommitSettlementRecordInert),
    CompletionPreparationInert {
        core: PostCommitCompoundBatchOwner,
        physical: PostCommitCompoundPhysicalOwner,
        inputs: PostCommitCompoundCompletionInputs,
        evidence: PostCommitCompoundMismatchEvidence,
    },
    CompletionInert {
        owner: PostCommitCompoundCompletionOwner,
        physical: PostCommitCompoundPhysicalOwner,
        evidence: PostCommitCompoundMismatchEvidence,
    },
    Inert(ActivePostCommitCompoundOwner),
}

pub(super) struct SettledRetiredHostEvidence {
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    edge_terminal: RetiredHostEdgePhysicalTerminal,
    terminal: bevy_ui_router_core::integration::PostCommitTerminalEvidence,
    retired_host: RetiredHostAuthorityTerminalEvidence,
}

struct RetiredHostPhysicalTerminal {
    host: RetiredHostBinding,
    edge: FrozenRetiredHostEdge,
    edge_terminal: RetiredHostEdgePhysicalTerminal,
}

pub(super) fn settled_retired_host_allows_candidate(
    world: &World,
    evidence: &SettledRetiredHostEvidence,
    candidate: &PreparedEcsHostCandidateSource,
) -> bool {
    if candidate.candidate.host() != evidence.host.host {
        return true;
    }
    let allowed = candidate.registration_incarnation != evidence.host.registration_incarnation
        && evidence.edge.host == evidence.host.host
        && evidence.edge.top_slot == evidence.host.top_slot
        && evidence.edge.top_slot_claim == evidence.host.top_slot_claim
        && evidence.edge.registration_incarnation == evidence.host.registration_incarnation
        && matches!(
            evidence.edge_terminal,
            RetiredHostEdgePhysicalTerminal::Detached
                | RetiredHostEdgePhysicalTerminal::HostAbsent
                | RetiredHostEdgePhysicalTerminal::EdgeAlreadyAbsent
        )
        && (world.get_entity(evidence.host.top_slot).is_err()
            || world
                .get::<ChildOf>(evidence.host.top_slot)
                .is_none_or(|parent| parent.parent() != evidence.host.host));
    allowed
}

const POST_COMMIT_L_OBLIGATION_KEY: u64 = 0x5232_4c4f_424c;
const POST_COMMIT_L_RECEIPT_KEY: u64 = 0x5232_4c52_4350;
const POST_COMMIT_L_SOURCE_KEY: u64 = 0x5232_4c53_5243;
const POST_COMMIT_A_OBLIGATION_KEY: u64 = 0x5232_414f_424c;
const POST_COMMIT_A_SOURCE_KEY: u64 = 0x5232_4153_5243;
const POST_COMMIT_H_OBLIGATION_KEY: u64 = 0x5232_484f_424c;
const POST_COMMIT_H_SOURCE_KEY: u64 = 0x5232_4853_5243;

impl FrozenRetiredHostEdge {
    pub(super) fn freeze(
        world: &World,
        host: &HostBinding,
    ) -> Result<Self, RetiredHostEdgeFreezePermanentEvidence> {
        if host.top_slot_claim.entity() != host.top_slot
            || host.candidate.host() != host.host
            || !matches!(
                host.top_slot_claim.owner(),
                RegistryOwner::Committed(EntityOwner::HostTopSlot { host: version, slot })
                    if version == &host.version && *slot == host.slot_id
            )
        {
            return Err(RetiredHostEdgeFreezePermanentEvidence::InvalidBinding);
        }
        let host_present = world.get_entity(host.host).is_ok();
        let children_component = world.components().component_id::<Children>();
        let host_non_children_manifest =
            ComponentManifest::from_world(world, host.host).map(|manifest| {
                ComponentManifest::new(
                    manifest
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != children_component),
                )
            });
        let children_before = world
            .get::<Children>(host.host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let top_parent_before = world.get::<ChildOf>(host.top_slot).map(ChildOf::parent);
        let membership = children_before
            .iter()
            .filter(|child| **child == host.top_slot)
            .count();
        if host_present
            && ((top_parent_before == Some(host.host) && membership != 1)
                || (top_parent_before != Some(host.host) && membership != 0))
        {
            return Err(RetiredHostEdgeFreezePermanentEvidence::InvalidRelationship);
        }
        let mut children_after = children_before.clone();
        children_after.retain(|child| *child != host.top_slot);
        let host_parent = world.get::<ChildOf>(host.host).map(ChildOf::parent);
        let mut host_ancestry = Vec::new();
        let mut cursor = host_parent;
        while let Some(entity) = cursor {
            if entity == host.host || host_ancestry.contains(&entity) {
                return Err(RetiredHostEdgeFreezePermanentEvidence::RelationshipCycle);
            }
            host_ancestry.push(entity);
            cursor = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        let obligation_binding = host.host.to_bits().rotate_left(11)
            ^ host.top_slot.to_bits().rotate_left(29)
            ^ host.registration_incarnation;
        let physical_source_binding = obligation_binding
            ^ (children_before.len() as u64).rotate_left(17)
            ^ top_parent_before.map_or(0, |entity| entity.to_bits());
        Ok(Self {
            host: host.host,
            top_slot: host.top_slot,
            top_slot_claim: host.top_slot_claim.clone(),
            registration_incarnation: host.registration_incarnation,
            host_present,
            host_non_children_manifest,
            host_parent,
            host_ancestry,
            children_before,
            children_after,
            top_parent_before,
            obligation_binding,
            physical_source_binding,
        })
    }

    pub(super) fn host_projection_is_current(&self, world: &World, detached: bool) -> bool {
        if !self.host_present {
            return world.get_entity(self.host).is_err();
        }
        if world.get_entity(self.host).is_err()
            || world.get::<ChildOf>(self.host).map(ChildOf::parent) != self.host_parent
        {
            return false;
        }
        let children_component = world.components().component_id::<Children>();
        let observed_host_manifest =
            ComponentManifest::from_world(world, self.host).map(|manifest| {
                ComponentManifest::new(
                    manifest
                        .component_ids()
                        .iter()
                        .copied()
                        .filter(|component| Some(*component) != children_component),
                )
            });
        if observed_host_manifest != self.host_non_children_manifest {
            return false;
        }
        let mut ancestry = Vec::new();
        let mut cursor = self.host_parent;
        while let Some(entity) = cursor {
            if entity == self.host || ancestry.contains(&entity) {
                return false;
            }
            ancestry.push(entity);
            cursor = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        if ancestry != self.host_ancestry {
            return false;
        }
        let expected_children = if detached {
            &self.children_after
        } else {
            &self.children_before
        };
        let observed_children = world
            .get::<Children>(self.host)
            .map(|children| children.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let observed_parent = world.get::<ChildOf>(self.top_slot).map(ChildOf::parent);
        observed_children == *expected_children
            && if detached {
                observed_parent != Some(self.host)
            } else {
                observed_parent == self.top_parent_before
            }
    }
}

fn post_commit_cleanup_plan_entry(
    target: &crate::cleanup::FrozenPostCommitAuthorityTarget,
) -> PostCommitPlannedCleanupProjection {
    PostCommitPlannedCleanupProjection::from_trusted_ecs_freeze(
        PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
            POST_COMMIT_A_OBLIGATION_KEY,
            vec![
                target.index() as u64,
                target.entity().to_bits(),
                target.obligation_binding(),
            ],
        ),
        PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
            POST_COMMIT_A_SOURCE_KEY,
            vec![
                target.index() as u64,
                target.entity().to_bits(),
                target.physical_source_binding(),
            ],
        ),
        target.disposition().public(),
    )
}

fn bind_post_commit_settlement_plan(
    original: &FrozenOriginalCleanupLedgerTransfer,
    authority: &FrozenPostCommitAuthorityPartition,
    edge: &FrozenRetiredHostEdge,
    builder: bevy_ui_router_core::integration::PostCommitSettlementPlanBuilder,
) -> Result<
    bevy_ui_router_core::integration::PostCommitSettlementPlan,
    PostCommitSettlementPlanMismatchEvidence,
> {
    let settled_original = original
        .indices()
        .iter()
        .filter_map(|entry| {
            let receipt = entry.settled_receipt()?;
            Some(
                PostCommitSettledOriginalProjection::from_trusted_ecs_freeze(
                    PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                        POST_COMMIT_L_OBLIGATION_KEY,
                        vec![
                            original.ledger_domain(),
                            entry.index() as u64,
                            entry.obligation_binding(),
                        ],
                    ),
                    PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                        POST_COMMIT_L_RECEIPT_KEY,
                        vec![
                            original.ledger_domain(),
                            receipt.obligation() as u64,
                            receipt.attempt_revision(),
                            receipt.binding(),
                        ],
                    ),
                    receipt.disposition().public(),
                ),
            )
        })
        .collect::<Vec<_>>();
    let residual_original = original
        .indices()
        .iter()
        .filter(|entry| entry.settled_receipt().is_none())
        .map(|entry| {
            PostCommitPlannedCleanupProjection::from_trusted_ecs_freeze(
                PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                    POST_COMMIT_L_OBLIGATION_KEY,
                    vec![
                        original.ledger_domain(),
                        entry.index() as u64,
                        entry.obligation_binding(),
                    ],
                ),
                PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
                    POST_COMMIT_L_SOURCE_KEY,
                    vec![
                        original.ledger_domain(),
                        entry.index() as u64,
                        entry.physical_source_binding(),
                    ],
                ),
                entry.disposition().public(),
            )
        })
        .collect::<Vec<_>>();
    let authority_non_top = authority
        .non_top_targets()
        .map(post_commit_cleanup_plan_entry)
        .collect::<Vec<_>>();
    let authority_top_target = authority
        .top_target()
        .expect("the frozen authority partition owns exactly one top slot");
    let authority_top = post_commit_cleanup_plan_entry(authority_top_target);
    let host_obligation = PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
        POST_COMMIT_H_OBLIGATION_KEY,
        vec![
            edge.host.to_bits(),
            edge.top_slot.to_bits(),
            edge.registration_incarnation,
            edge.obligation_binding,
        ],
    );
    let host_edge = PostCommitPlannedHostEdgeProjection::from_trusted_ecs_freeze(
        host_obligation.clone(),
        PostCommitSettlementKeyProjection::from_trusted_ecs_freeze(
            POST_COMMIT_H_SOURCE_KEY,
            vec![
                edge.host.to_bits(),
                edge.top_slot.to_bits(),
                edge.physical_source_binding,
            ],
        ),
    );
    let prior_trace_start = builder
        .prior_trace_len()
        .checked_sub(settled_original.len())
        .unwrap_or(usize::MAX);
    builder.bind_trusted_ecs(PostCommitSettlementPlanDescriptor::from_trusted_ecs_freeze(
        prior_trace_start,
        settled_original,
        residual_original,
        authority_non_top,
        authority_top,
        host_obligation,
        host_edge,
    ))
}

pub(super) enum PostCommitPlanBindFailure {
    Plan(PostCommitSettlementPlanMismatchEvidence),
    Authority(crate::cleanup::PreparedAuthorityLedgerBindingMismatch),
    Isolation(PostCommitInputIsolationFailure),
}

pub(super) enum PostCommitAcceptedInertEvidence {
    Receiver(PostCommitReceiverMismatchView),
    Retained(IntegrationError),
    Retryable(PostCommitAuthorityBeginRetryEvidence),
    Permanent(PostCommitAuthorityBeginPermanentEvidence),
}

impl PostCommitAcceptedInertEvidence {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

pub(super) enum PostCommitBoundInertEvidence {
    Binding(PostCommitPlanBindFailure),
    Receiver(PostCommitReceiverMismatchView),
    Retryable(PostCommitAuthorityBeginRetryEvidence),
    Permanent(PostCommitAuthorityBeginPermanentEvidence),
}

impl PostCommitBoundInertEvidence {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

pub(super) fn post_commit_authority_loss_cause(
    world: &World,
    host: &HostBinding,
) -> Option<PostCommitAuthorityCause> {
    if current_host_binding_is_exact(world, host) {
        return None;
    }
    Some(if world.get_resource::<RouteEntityRegistry>().is_none() {
        PostCommitAuthorityCause::RequiredResourcesLost
    } else if world.get_entity(host.host).is_err() {
        PostCommitAuthorityCause::HostEntityMissing
    } else if world
        .get::<RouterHostRegistration>(host.host)
        .is_none_or(|registration| registration.incarnation() != host.registration_incarnation)
    {
        PostCommitAuthorityCause::HostRegistrationLost
    } else {
        return None;
    })
}

pub(super) fn begin_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    cause: PostCommitAuthorityCause,
) -> bool {
    if entry.post_commit_compound.is_some() {
        return true;
    }
    let Some(mut original) = entry.cleanup_ledger.take() else {
        return false;
    };
    if matches!(
        original.preflight_post_commit_transfer_source_kind(),
        Err(PostCommitOriginalFreezeFailure::IneligibleSource)
    ) {
        entry.cleanup_ledger = Some(original);
        return false;
    }
    if original.post_commit_transfer_requires_ordinary_settlement(world) {
        let Some(attempt) = original.attempt(world) else {
            entry.cleanup_ledger = Some(original);
            return true;
        };
        if !original.settle(world, attempt) {
            entry.cleanup_ledger = Some(original);
            return true;
        }
    }
    if !entry.observe_cleanup_receipts(&mut original) {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: None,
                    authority: None,
                    edge: None,
                },
                evidence: PostCommitPreAcceptInertEvidence::ReceiptObservationMismatch,
            }
            .into(),
        );
        return true;
    }
    let original_freeze = match original.freeze_for_post_commit_transfer(world) {
        Ok(freeze) => freeze,
        Err(PostCommitOriginalFreezeFailure::IneligibleSource) => {
            entry.cleanup_ledger = Some(original);
            return false;
        }
        Err(PostCommitOriginalFreezeFailure::OrdinarySettlementRequired) => {
            entry.cleanup_ledger = Some(original);
            return true;
        }
        Err(PostCommitOriginalFreezeFailure::RetryableCapacity) => {
            entry.cleanup_ledger = Some(original);
            return true;
        }
        Err(PostCommitOriginalFreezeFailure::Permanent(evidence)) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: None,
                        authority: None,
                        edge: None,
                    },
                    evidence: PostCommitPreAcceptInertEvidence::Original(evidence),
                }
                .into(),
            );
            return true;
        }
    };
    if entry.active.is_none() || entry.host.is_none() {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: Some(original_freeze),
                    authority: None,
                    edge: None,
                },
                evidence: PostCommitPreAcceptInertEvidence::EntryOwnerMissing,
            }
            .into(),
        );
        return true;
    }
    let Some(inventory) = entry.retained.inventory() else {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: Some(original_freeze),
                    authority: None,
                    edge: None,
                },
                evidence: PostCommitPreAcceptInertEvidence::RetainedInventory,
            }
            .into(),
        );
        return true;
    };
    let retiring = inventory
        .iter()
        .map(|owner| owner.projection().clone())
        .collect::<Vec<_>>();
    let mut retained_change = if retiring.is_empty() {
        None
    } else {
        let Some(change) = entry.retained.prepare_all(&retiring) else {
            entry.cleanup_ledger = Some(original);
            return true;
        };
        Some(change)
    };
    #[cfg(feature = "bsn")]
    let native_preparation = (|| {
        entry
            .active
            .as_mut()
            .unwrap()
            .prepare_native_cleanup(world)?;
        if let Some(change) = &retained_change {
            for (_, branch) in change.selected_mut(&mut entry.retained) {
                branch.prepare_native_cleanup(world)?;
            }
        }
        Ok::<(), crate::presentation::native_frame::NativeCleanupPreparationError>(())
    })();
    #[cfg(feature = "bsn")]
    if let Err(error) = native_preparation {
        if error.is_retryable() {
            entry.cleanup_ledger = Some(original);
        } else {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: None,
                        edge: None,
                    },
                    evidence: PostCommitPreAcceptInertEvidence::NativeCleanup(error),
                }
                .into(),
            );
        }
        return true;
    }
    let active = entry.active.as_ref().unwrap();
    let host = entry.host.as_ref().unwrap();
    let edge = match FrozenRetiredHostEdge::freeze(world, host) {
        Ok(edge) => edge,
        Err(evidence) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: None,
                        edge: None,
                    },
                    evidence: PostCommitPreAcceptInertEvidence::RetiredHostEdge(evidence),
                }
                .into(),
            );
            return true;
        }
    };
    let authority = match PreparedPostCommitAuthorityLedger::freeze_and_reserve(
        world,
        active,
        host,
        &original_freeze,
        &retained_change
            .as_ref()
            .into_iter()
            .flat_map(|change| change.selected(&entry.retained))
            .collect::<Vec<_>>(),
    ) {
        Ok(authority) => authority,
        Err(PostCommitAuthorityFreezeFailure::RetryableCapacity) => {
            entry.cleanup_ledger = Some(original);
            return true;
        }
        Err(PostCommitAuthorityFreezeFailure::Permanent(evidence)) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: None,
                        edge: Some(edge),
                    },
                    evidence: PostCommitPreAcceptInertEvidence::Authority(evidence),
                }
                .into(),
            );
            return true;
        }
    };
    let retained_boundaries = if retiring.is_empty() {
        None
    } else {
        let Ok(boundaries) = world
            .resource::<RouteEntityRegistry>()
            .prepare_retained_boundaries(&[], &retiring)
        else {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: Some(authority),
                        edge: Some(edge),
                    },
                    evidence: PostCommitPreAcceptInertEvidence::RetainedInventory,
                }
                .into(),
            );
            return true;
        };
        Some(boundaries)
    };
    let input_isolation =
        match prepare_post_commit_input_isolation(world, active, &original_freeze, &authority) {
            Ok(isolation) => Some(isolation),
            Err(PostCommitInputIsolationFailure::RetryableCapacity) => {
                entry.cleanup_ledger = Some(original);
                return true;
            }
            Err(evidence) => {
                entry.post_commit_compound = Some(
                    PostCommitCompoundHolder::PreAcceptInert {
                        physical: PostCommitPreAcceptPhysicalOwner {
                            original,
                            original_freeze: Some(original_freeze),
                            authority: Some(authority),
                            edge: Some(edge),
                        },
                        evidence: PostCommitPreAcceptInertEvidence::Isolation(evidence),
                    }
                    .into(),
                );
                return true;
            }
        };
    let Some(normal_batch) = original.batch() else {
        entry.post_commit_compound = Some(
            PostCommitCompoundHolder::PreAcceptInert {
                physical: PostCommitPreAcceptPhysicalOwner {
                    original,
                    original_freeze: Some(original_freeze),
                    authority: Some(authority),
                    edge: Some(edge),
                },
                evidence: PostCommitPreAcceptInertEvidence::MissingNormalBatch,
            }
            .into(),
        );
        return true;
    };
    let transfer = match entry
        .engine
        .accept_post_commit_authority_transfer(normal_batch, cause)
    {
        PostCommitAuthorityAcceptanceOutcome::Accepted(transfer) => transfer,
        PostCommitAuthorityAcceptanceOutcome::Mismatch { expected, observed } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PreAcceptInert {
                    physical: PostCommitPreAcceptPhysicalOwner {
                        original,
                        original_freeze: Some(original_freeze),
                        authority: Some(authority),
                        edge: Some(edge),
                    },
                    evidence: PostCommitPreAcceptInertEvidence::CoreReceiver { expected, observed },
                }
                .into(),
            );
            return true;
        }
    };

    let active = entry
        .active
        .take()
        .expect("the accepted post-commit receiver retains active physical ownership");
    let (host, lease) = entry
        .host
        .take()
        .expect("the accepted post-commit receiver retains host ownership")
        .into_retired();
    let mut physical = PreparedPostCommitPhysicalOwner {
        original,
        original_freeze,
        authority,
        active,
        retained: Vec::new(),
        retained_boundaries,
        host,
        edge,
        input_isolation,
    };
    let source = PostCommitAuthorityPreparationSource::new(transfer, lease);
    let preparation = if let Some(change) = retained_change.as_mut() {
        let plan = entry
            .retained
            .inventory()
            .ok_or(IntegrationError::Stale)
            .and_then(|inventory| {
                entry
                    .engine
                    .prepare_retained_post_commit_authority_plan(&source, &inventory)
            });
        let plan = match plan {
            Ok(plan)
                if plan.retiring().len() == retiring.len()
                    && plan
                        .retiring()
                        .iter()
                        .all(|projection| retiring.contains(projection)) =>
            {
                plan
            }
            result => {
                let error = result.err().unwrap_or(IntegrationError::InvalidManifest);
                entry.post_commit_compound =
                    Some(Box::new(PostCommitCompoundHolder::AcceptedInert {
                        source,
                        physical: Box::new(physical),
                        evidence: Box::new(PostCommitAcceptedInertEvidence::Retained(error)),
                    }));
                return true;
            }
        };
        let manifest = CleanupManifest::new(
            plan.logical_roots().to_vec(),
            physical.authority.manifest_digest(),
            physical.authority.obligation_count(),
        );
        let owners = change.take_owners(&mut entry.retained);
        let preparation = entry
            .engine
            .prepare_post_commit_authority_transfer_with_retention(source, plan, manifest, owners);
        // The returned Source or Prepared transfer owns the original
        // capabilities; its matching physical payloads follow every outcome.
        physical.retained = retained_change
            .take()
            .unwrap()
            .into_payloads(&mut entry.retained);
        preparation
    } else {
        let logical_roots = physical
            .active
            .frames()
            .iter()
            .rev()
            .map(|frame| frame.frame().clone())
            .collect::<Vec<_>>();
        entry.engine.prepare_post_commit_authority_transfer(
            source,
            CleanupManifest::new(
                logical_roots,
                physical.authority.manifest_digest(),
                physical.authority.obligation_count(),
            ),
        )
    };
    entry.post_commit_compound = Some(match preparation {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, .. } => {
            resume_prepared_post_commit_compound(world, entry, owner, physical)
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
    });
    true
}

fn resume_accepted_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    source: PostCommitAuthorityPreparationSource,
    physical: PreparedPostCommitPhysicalOwner,
) -> Box<PostCommitCompoundHolder> {
    let logical_roots = source
        .retained_logical_roots()
        .map(<[_]>::to_vec)
        .unwrap_or_else(|| {
            physical
                .active
                .frames()
                .iter()
                .rev()
                .map(|frame| frame.frame().clone())
                .collect::<Vec<_>>()
        });
    match entry.engine.prepare_post_commit_authority_transfer(
        source,
        CleanupManifest::new(
            logical_roots,
            physical.authority.manifest_digest(),
            physical.authority.obligation_count(),
        ),
    ) {
        PostCommitAuthorityPreparationOutcome::Prepared { owner, .. } => {
            resume_prepared_post_commit_compound(world, entry, owner, physical)
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

type PostCommitBoundPhysical<'w> = (
    Box<PostCommitCompoundPhysicalOwner>,
    bevy_ecs::change_detection::Mut<'w, RouteEntityRegistry>,
);

fn bind_prepared_post_commit_physical<'w>(
    world: &'w mut World,
    mut physical: Box<PreparedPostCommitPhysicalOwner>,
    observed_batch: &r3_core::PostCommitCleanupBatchView,
    builder: r3_core::PostCommitSettlementPlanBuilder,
) -> Result<
    (
        PostCommitBoundPhysical<'w>,
        r3_core::PostCommitSettlementPlan,
    ),
    (
        Box<PreparedPostCommitPhysicalOwner>,
        PostCommitPlanBindFailure,
    ),
> {
    let plan = match bind_post_commit_settlement_plan(
        &physical.original_freeze,
        physical.authority.partition(),
        &physical.edge,
        builder,
    ) {
        Ok(plan) => plan,
        Err(evidence) => {
            return Err((physical, PostCommitPlanBindFailure::Plan(evidence)));
        }
    };
    if !physical.authority.batch_matches(observed_batch) {
        return Err((
            physical,
            PostCommitPlanBindFailure::Authority(
                crate::cleanup::PreparedAuthorityLedgerBindingMismatch::WrongBatch,
            ),
        ));
    }
    let Some(isolation) = physical.input_isolation.take() else {
        return Err((
            physical,
            PostCommitPlanBindFailure::Isolation(PostCommitInputIsolationFailure::RefreezeRejected),
        ));
    };
    let Some(original_removed) = physical
        .original_freeze
        .removed_entities_for_authority_refreeze(world, &physical.original, None)
    else {
        return Err((
            physical,
            PostCommitPlanBindFailure::Isolation(PostCommitInputIsolationFailure::RefreezeRejected),
        ));
    };
    let input_isolation = match isolation.apply_guarded(world, &|world| {
        physical
            .original_freeze
            .refreeze(world, &physical.original, None)
            && physical
                .authority
                .partition()
                .refreeze_allowing_input_isolation(world, &original_removed)
            && physical.edge.host_projection_is_current(world, false)
    }) {
        Ok(isolation) => isolation,
        Err(evidence) => {
            return Err((physical, PostCommitPlanBindFailure::Isolation(evidence)));
        }
    };
    if !physical.authority.commit_input_isolation(
        world,
        &physical.original_freeze,
        &physical.original,
    ) {
        return Err((
            physical,
            PostCommitPlanBindFailure::Isolation(PostCommitInputIsolationFailure::RefreezeRejected),
        ));
    }
    let PreparedPostCommitPhysicalOwner {
        original,
        original_freeze,
        authority,
        active,
        retained,
        retained_boundaries,
        host,
        edge,
        input_isolation: _,
    } = *physical;
    let authority = authority.bind_verified_batch(observed_batch.clone());
    let (authority, partition) = authority.into_parts();
    let physical = Box::new(PostCommitCompoundPhysicalOwner {
        original,
        original_freeze,
        authority,
        partition,
        active,
        retained,
        retained_boundaries,
        host,
        edge,
        input_isolation,
        original_cursor: 0,
        authority_non_top_cursor: 0,
        settled_authority: Vec::new(),
        edge_terminal: None,
        top_settled: false,
    });
    // Borrow only after all World writes and their partition checks. This
    // temporary receiver travels to Core's infallible installation callback.
    let registry = world.resource_mut::<RouteEntityRegistry>();
    Ok(((physical, registry), plan))
}

pub(super) fn resume_prepared_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    owner: PreparedPostCommitAuthorityTransfer,
    mut physical: PreparedPostCommitPhysicalOwner,
) -> Box<PostCommitCompoundHolder> {
    if physical.input_isolation.is_none() {
        physical.input_isolation = match prepare_post_commit_input_isolation(
            world,
            &physical.active,
            &physical.original_freeze,
            &physical.authority,
        ) {
            Ok(isolation) => Some(isolation),
            Err(evidence) => {
                return Box::new(PostCommitCompoundHolder::PreparedInert {
                    owner,
                    physical,
                    evidence: PostCommitPlanBindFailure::Isolation(evidence),
                });
            }
        };
    }

    let mut installed = None;
    let outcome = entry.engine.begin_post_commit_authority_transfer(
        owner,
        Box::new(physical),
        move |physical, observed_batch, builder| {
            bind_prepared_post_commit_physical(world, physical, observed_batch, builder)
        },
        |(mut physical, mut registry), core| {
            if let Some(boundaries) = physical.retained_boundaries.take() {
                boundaries.apply(&mut registry);
            }
            drop(registry);
            installed = Some((physical, core));
        },
    );
    post_commit_begin_outcome_holder(release_post_commit_bound_registry(outcome), installed)
}

fn bind_bound_post_commit_physical<'w>(
    world: &'w mut World,
    physical: Box<PostCommitCompoundPhysicalOwner>,
    observed_batch: &r3_core::PostCommitCleanupBatchView,
    builder: r3_core::PostCommitSettlementPlanBuilder,
) -> Result<
    (
        PostCommitBoundPhysical<'w>,
        r3_core::PostCommitSettlementPlan,
    ),
    (
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitPlanBindFailure,
    ),
> {
    let plan = match bind_post_commit_settlement_plan(
        &physical.original_freeze,
        &physical.partition,
        &physical.edge,
        builder,
    ) {
        Ok(plan) => plan,
        Err(evidence) => return Err((physical, PostCommitPlanBindFailure::Plan(evidence))),
    };
    if !physical.authority.post_commit_batch_matches(observed_batch) {
        return Err((
            physical,
            PostCommitPlanBindFailure::Authority(
                crate::cleanup::PreparedAuthorityLedgerBindingMismatch::WrongBatch,
            ),
        ));
    }
    let Some(registry) = world.get_resource_mut::<RouteEntityRegistry>() else {
        return Err((
            physical,
            PostCommitPlanBindFailure::Isolation(PostCommitInputIsolationFailure::RefreezeRejected),
        ));
    };
    Ok(((physical, registry), plan))
}

fn resume_bound_post_commit_compound(
    world: &mut World,
    entry: &mut RouterEntry,
    owner: PreparedPostCommitAuthorityTransfer,
    physical: PostCommitCompoundPhysicalOwner,
) -> Box<PostCommitCompoundHolder> {
    let mut installed = None;
    let outcome = entry.engine.begin_post_commit_authority_transfer(
        owner,
        Box::new(physical),
        move |physical, observed_batch, builder| {
            bind_bound_post_commit_physical(world, physical, observed_batch, builder)
        },
        |(mut physical, mut registry), core| {
            if let Some(boundaries) = physical.retained_boundaries.take() {
                boundaries.apply(&mut registry);
            }
            drop(registry);
            installed = Some((physical, core));
        },
    );
    post_commit_bound_retry_outcome_holder(release_post_commit_bound_registry(outcome), installed)
}

fn release_post_commit_bound_registry<Unbound, BindError>(
    outcome: PostCommitAuthorityBeginOutcome<Unbound, PostCommitBoundPhysical<'_>, BindError>,
) -> PostCommitAuthorityBeginOutcome<Unbound, Box<PostCommitCompoundPhysicalOwner>, BindError> {
    match outcome {
        PostCommitAuthorityBeginOutcome::Begun => PostCommitAuthorityBeginOutcome::Begun,
        PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        } => PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        },
        PostCommitAuthorityBeginOutcome::ReceiverMismatch {
            owner,
            bound: (physical, registry),
            evidence,
        } => {
            drop(registry);
            PostCommitAuthorityBeginOutcome::ReceiverMismatch {
                owner,
                bound: physical,
                evidence,
            }
        }
        PostCommitAuthorityBeginOutcome::Retryable {
            owner,
            bound: (physical, registry),
            evidence,
        } => {
            drop(registry);
            PostCommitAuthorityBeginOutcome::Retryable {
                owner,
                bound: physical,
                evidence,
            }
        }
        PostCommitAuthorityBeginOutcome::Permanent {
            owner,
            bound: (physical, registry),
            evidence,
        } => {
            drop(registry);
            PostCommitAuthorityBeginOutcome::Permanent {
                owner,
                bound: physical,
                evidence,
            }
        }
    }
}

fn post_commit_begin_outcome_holder(
    outcome: PostCommitAuthorityBeginOutcome<
        Box<PreparedPostCommitPhysicalOwner>,
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitPlanBindFailure,
    >,
    installed: Option<(
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitCompoundBatchOwner,
    )>,
) -> Box<PostCommitCompoundHolder> {
    match outcome {
        PostCommitAuthorityBeginOutcome::Begun => {
            let (physical, core) =
                installed.expect("the infallible install callback publishes the compound owner");
            Box::new(PostCommitCompoundHolder::Active(
                ActivePostCommitCompoundOwner {
                    core,
                    physical: *physical,
                },
            ))
        }
        PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::PreparedInert {
            owner,
            physical: *unbound,
            evidence,
        }),
        PostCommitAuthorityBeginOutcome::ReceiverMismatch {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Receiver(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Retryable {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Retryable(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Permanent {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Permanent(evidence)),
        }),
    }
}

fn post_commit_bound_retry_outcome_holder(
    outcome: PostCommitAuthorityBeginOutcome<
        Box<PostCommitCompoundPhysicalOwner>,
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitPlanBindFailure,
    >,
    installed: Option<(
        Box<PostCommitCompoundPhysicalOwner>,
        PostCommitCompoundBatchOwner,
    )>,
) -> Box<PostCommitCompoundHolder> {
    match outcome {
        PostCommitAuthorityBeginOutcome::Begun => {
            let (physical, core) =
                installed.expect("the infallible install callback publishes the compound owner");
            Box::new(PostCommitCompoundHolder::Active(
                ActivePostCommitCompoundOwner {
                    core,
                    physical: *physical,
                },
            ))
        }
        PostCommitAuthorityBeginOutcome::BindingMismatch {
            owner,
            unbound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: unbound,
            evidence: Box::new(PostCommitBoundInertEvidence::Binding(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::ReceiverMismatch {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Receiver(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Retryable {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Retryable(evidence)),
        }),
        PostCommitAuthorityBeginOutcome::Permanent {
            owner,
            bound,
            evidence,
        } => Box::new(PostCommitCompoundHolder::BoundInert {
            owner,
            physical: bound,
            evidence: Box::new(PostCommitBoundInertEvidence::Permanent(evidence)),
        }),
    }
}

fn resume_retryable_post_commit_holder(
    world: &mut World,
    entry: &mut RouterEntry,
    holder: Box<PostCommitCompoundHolder>,
) -> Box<PostCommitCompoundHolder> {
    let retryable = match holder.as_ref() {
        PostCommitCompoundHolder::DeferredExitAccepted(_) => true,
        PostCommitCompoundHolder::AcceptedInert { evidence, .. } => evidence.is_retryable(),
        PostCommitCompoundHolder::PreparedInert {
            evidence:
                PostCommitPlanBindFailure::Isolation(PostCommitInputIsolationFailure::RetryableCapacity),
            ..
        } => true,
        PostCommitCompoundHolder::BoundInert { evidence, .. } => evidence.is_retryable(),
        _ => false,
    };
    if !retryable {
        return holder;
    }
    match *holder {
        PostCommitCompoundHolder::DeferredExitAccepted(mut accepted) => {
            super::deferred_exit_authority::resume_accepted(
                world,
                entry,
                accepted
                    .take()
                    .expect("accepted exit installs every original owner in its tail"),
            )
        }
        PostCommitCompoundHolder::AcceptedInert {
            source, physical, ..
        } => resume_accepted_post_commit_compound(world, entry, source, *physical),
        PostCommitCompoundHolder::PreparedInert {
            owner, physical, ..
        } => resume_prepared_post_commit_compound(world, entry, owner, physical),
        PostCommitCompoundHolder::BoundInert {
            owner, physical, ..
        } => resume_bound_post_commit_compound(world, entry, owner, *physical),
        _ => unreachable!("only a retryable post-accept holder is resumed"),
    }
}

fn settle_post_commit_retired_host_edge(
    world: &mut World,
    physical: &PostCommitCompoundPhysicalOwner,
) -> Result<RetiredHostEdgePhysicalProof, PostCommitAuthoritySettlementFailure> {
    let edge = &physical.edge;
    let original_removed = physical
        .original_freeze
        .removed_entities_for_authority_refreeze(world, &physical.original, None)
        .ok_or(PostCommitAuthoritySettlementFailure::PrewriteDrift)?;
    if physical.edge_terminal.is_some()
        || !physical.input_isolation.is_current(world)
        || !physical
            .original_freeze
            .terminals_are_current(world, &physical.original)
        || physical.partition.top_target().is_none_or(|top| {
            top.entity() != edge.top_slot || top.registered_claim() != Some(&edge.top_slot_claim)
        })
        || !physical.partition.refreeze(
            world,
            &physical.settled_authority,
            None,
            false,
            &original_removed,
        )
    {
        return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
    }
    let terminal = if !edge.host_present {
        if !edge.host_projection_is_current(world, false) {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        RetiredHostEdgePhysicalTerminal::HostAbsent
    } else if edge.top_parent_before != Some(edge.host) {
        if edge.children_before != edge.children_after
            || !edge.host_projection_is_current(world, false)
        {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        RetiredHostEdgePhysicalTerminal::EdgeAlreadyAbsent
    } else {
        if !edge.host_projection_is_current(world, false)
            || world.get_entity(edge.top_slot).is_err()
        {
            return Err(PostCommitAuthoritySettlementFailure::PrewriteDrift);
        }
        world.entity_mut(edge.top_slot).remove::<ChildOf>();
        world.flush();
        if !edge.host_projection_is_current(world, true)
            || !physical.input_isolation.is_current(world)
            || !physical
                .original_freeze
                .terminals_are_current(world, &physical.original)
            || !physical.partition.refreeze(
                world,
                &physical.settled_authority,
                None,
                true,
                &original_removed,
            )
        {
            return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
        }
        RetiredHostEdgePhysicalTerminal::Detached
    };
    let host_edge_detached = terminal == RetiredHostEdgePhysicalTerminal::Detached;
    if !physical
        .original_freeze
        .terminals_are_current(world, &physical.original)
        || !physical.input_isolation.is_current(world)
        || !physical.partition.refreeze(
            world,
            &physical.settled_authority,
            None,
            host_edge_detached,
            &original_removed,
        )
    {
        return Err(PostCommitAuthoritySettlementFailure::PostwriteDrift);
    }
    Ok(RetiredHostEdgePhysicalProof {
        terminal,
        binding: edge.physical_source_binding
            ^ match terminal {
                RetiredHostEdgePhysicalTerminal::Detached => 1,
                RetiredHostEdgePhysicalTerminal::HostAbsent => 2,
                RetiredHostEdgePhysicalTerminal::EdgeAlreadyAbsent => 3,
            },
    })
}

#[inline(never)]
fn drive_post_commit_original_residual(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: ActivePostCommitCompoundOwner,
) {
    if !active.physical.input_isolation.is_current(world) {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    while active.physical.original_cursor < active.physical.original_freeze.indices().len()
        && active.physical.original_freeze.indices()[active.physical.original_cursor]
            .settled_receipt()
            .is_some()
    {
        active.physical.original_cursor += 1;
    }
    let Some(frozen) = active
        .physical
        .original_freeze
        .indices()
        .get(active.physical.original_cursor)
    else {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    };
    let obligation = frozen.index();
    let attempt = match entry
        .engine
        .prepare_next_post_commit_settlement_trusted(active.core)
    {
        TrustedPostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        TrustedPostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitMismatchInert {
                    core: owner,
                    physical: active.physical,
                    evidence,
                }
                .into(),
            );
            return;
        }
    };
    let proof = match active.physical.original.post_commit_original_receipt_proof(
        world,
        &active.physical.original_freeze,
        obligation,
    ) {
        Some(proof) => proof,
        None => match active.physical.original.settle_post_commit_original_target(
            world,
            &active.physical.original_freeze,
            obligation,
            &active.physical.partition,
            &active.physical.settled_authority,
            &|world| active.physical.input_isolation.is_current(world),
        ) {
            Ok(proof) => proof,
            Err(failure) => {
                entry.post_commit_compound = Some(
                    PostCommitCompoundHolder::PermitInert {
                        attempt,
                        physical: active.physical,
                        failure,
                    }
                    .into(),
                );
                return;
            }
        },
    };
    match entry
        .engine
        .record_next_post_commit_settlement_trusted(attempt, proof)
    {
        TrustedPostCommitSettlementRecordOutcome::Recorded { owner, .. } => {
            active.core = owner;
            active.physical.original_cursor += 1;
            entry.post_commit_compound = Some(PostCommitCompoundHolder::Active(active).into());
        }
        TrustedPostCommitSettlementRecordOutcome::Mismatch {
            attempt,
            physical: proof,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::SettlementInert(PostCommitSettlementRecordInert {
                    attempt,
                    proof: PostCommitAnyPhysicalProof::Cleanup(proof),
                    physical: active.physical,
                    evidence,
                })
                .into(),
            );
        }
    }
}

#[inline(never)]
fn drive_post_commit_authority_target(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: ActivePostCommitCompoundOwner,
    top: bool,
) {
    if !active
        .physical
        .original_freeze
        .terminals_are_current(world, &active.physical.original)
        || !active.physical.input_isolation.is_current(world)
    {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    let target = if top {
        active.physical.partition.top_target()
    } else {
        active
            .physical
            .partition
            .non_top_targets()
            .nth(active.physical.authority_non_top_cursor)
    };
    let Some(target) = target else {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    };
    let target_index = target.index();
    let target_entity = target.entity();
    let host_edge_detached = matches!(
        active.physical.edge_terminal,
        Some(RetiredHostEdgePhysicalTerminal::Detached)
    );
    let attempt = match entry
        .engine
        .prepare_next_post_commit_settlement_trusted(active.core)
    {
        TrustedPostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        TrustedPostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitMismatchInert {
                    core: owner,
                    physical: active.physical,
                    evidence,
                }
                .into(),
            );
            return;
        }
    };
    let proof = active
        .physical
        .authority
        .settle_post_commit_authority_target(
            world,
            &mut active.physical.partition,
            &active.physical.original,
            &active.physical.original_freeze,
            target_index,
            &active.physical.settled_authority,
            host_edge_detached,
            &|world| active.physical.input_isolation.is_current(world),
        );
    let proof = match proof {
        Ok(proof) => proof,
        Err(failure) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitInert {
                    attempt,
                    physical: active.physical,
                    failure,
                }
                .into(),
            );
            return;
        }
    };
    match entry
        .engine
        .record_next_post_commit_settlement_trusted(attempt, proof)
    {
        TrustedPostCommitSettlementRecordOutcome::Recorded { owner, .. } => {
            active.core = owner;
            if top {
                active.physical.top_settled = true;
            } else {
                active.physical.authority_non_top_cursor += 1;
            }
            active.physical.settled_authority.push(target_entity);
            entry.post_commit_compound = Some(PostCommitCompoundHolder::Active(active).into());
        }
        TrustedPostCommitSettlementRecordOutcome::Mismatch {
            attempt,
            physical: proof,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::SettlementInert(PostCommitSettlementRecordInert {
                    attempt,
                    proof: PostCommitAnyPhysicalProof::Cleanup(proof),
                    physical: active.physical,
                    evidence,
                })
                .into(),
            );
        }
    }
}

#[inline(never)]
fn drive_post_commit_retired_host_edge(
    world: &mut World,
    entry: &mut RouterEntry,
    mut active: ActivePostCommitCompoundOwner,
) {
    if !active
        .physical
        .original_freeze
        .terminals_are_current(world, &active.physical.original)
        || !active.physical.input_isolation.is_current(world)
    {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    let attempt = match entry
        .engine
        .prepare_next_post_commit_settlement_trusted(active.core)
    {
        TrustedPostCommitSettlementPermitOutcome::Prepared(attempt) => attempt,
        TrustedPostCommitSettlementPermitOutcome::Mismatch { owner, evidence } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitMismatchInert {
                    core: owner,
                    physical: active.physical,
                    evidence,
                }
                .into(),
            );
            return;
        }
    };
    let proof = match settle_post_commit_retired_host_edge(world, &active.physical) {
        Ok(proof) => proof,
        Err(failure) => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::PermitInert {
                    attempt,
                    physical: active.physical,
                    failure,
                }
                .into(),
            );
            return;
        }
    };
    active.physical.edge_terminal = Some(proof.terminal);
    match entry
        .engine
        .record_next_post_commit_settlement_trusted(attempt, proof)
    {
        TrustedPostCommitSettlementRecordOutcome::Recorded { owner, .. } => {
            active.core = owner;
            entry.post_commit_compound = Some(PostCommitCompoundHolder::Active(active).into());
        }
        TrustedPostCommitSettlementRecordOutcome::Mismatch {
            attempt,
            physical: proof,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::SettlementInert(PostCommitSettlementRecordInert {
                    attempt,
                    proof: PostCommitAnyPhysicalProof::HostEdge(proof),
                    physical: active.physical,
                    evidence,
                })
                .into(),
            );
        }
    }
}

#[inline(never)]
fn drive_post_commit_completion(
    world: &World,
    entry: &mut RouterEntry,
    active: ActivePostCommitCompoundOwner,
) {
    if !active.physical.original.is_settled()
        || !active
            .physical
            .original_freeze
            .terminals_are_current(world, &active.physical.original)
        || !active.physical.input_isolation.is_current(world)
        || !active.physical.authority.authority_completion_is_exact()
        || !active.physical.top_settled
        || active.physical.edge_terminal.is_none()
    {
        entry.post_commit_compound = Some(PostCommitCompoundHolder::Inert(active).into());
        return;
    }
    let completion = active
        .physical
        .original
        .pending_completion()
        .expect("the exact original ledger retains its Core completion");
    match entry
        .engine
        .prepare_post_commit_compound_completion_trusted(active.core, completion, active.physical)
    {
        TrustedPostCommitCompoundCompletionPreparationOutcome::Prepared { owner, physical } => {
            match entry
                .engine
                .complete_post_commit_authority_compound_trusted(
                    owner,
                    physical,
                    |physical, _expected: RetiredHostAuthorityExpectedView| {
                        let PostCommitCompoundPhysicalOwner {
                            original,
                            authority,
                            active,
                            retained,
                            host,
                            edge,
                            edge_terminal,
                            ..
                        } = physical;
                        drop((original, authority, active, retained));
                        RetiredHostPhysicalTerminal {
                            host,
                            edge,
                            edge_terminal: edge_terminal
                                .expect("joint completion consumes the exact H terminal fact"),
                        }
                    },
                ) {
                TrustedPostCommitCompoundCompletionOutcome::Terminal {
                    terminal,
                    physical,
                    retired_host,
                } => {
                    entry.settled_retired_host_evidence = Some(SettledRetiredHostEvidence {
                        host: physical.host,
                        edge: physical.edge,
                        edge_terminal: physical.edge_terminal,
                        terminal,
                        retired_host,
                    });
                    entry.post_commit_compound = None;
                }
                TrustedPostCommitCompoundCompletionOutcome::Mismatch {
                    owner,
                    physical,
                    evidence,
                } => {
                    entry.post_commit_compound = Some(
                        PostCommitCompoundHolder::CompletionInert {
                            owner,
                            physical,
                            evidence,
                        }
                        .into(),
                    );
                }
            }
        }
        TrustedPostCommitCompoundCompletionPreparationOutcome::Mismatch {
            batch,
            physical,
            inputs,
            evidence,
        } => {
            entry.post_commit_compound = Some(
                PostCommitCompoundHolder::CompletionPreparationInert {
                    core: batch,
                    physical,
                    inputs,
                    evidence,
                }
                .into(),
            );
        }
    }
}

pub(super) fn drive_post_commit_compound(world: &mut World, entry: &mut RouterEntry) -> bool {
    if entry.post_commit_compound.is_none() {
        if entry.cleanup_ledger.is_none() {
            return false;
        }
        let Some(host) = entry.host.as_ref() else {
            return false;
        };
        let Some(cause) = post_commit_authority_loss_cause(world, host) else {
            return false;
        };
        if !begin_post_commit_compound(world, entry, cause) {
            return false;
        }
    }

    let budget = entry
        .post_commit_compound
        .as_ref()
        .and_then(|holder| match holder.as_ref() {
            PostCommitCompoundHolder::Active(active) => Some(
                active.physical.original_freeze.indices().len()
                    + active.physical.partition.targets().len()
                    + 4,
            ),
            _ => None,
        })
        .unwrap_or(1);
    for _ in 0..budget {
        let Some(holder) = entry.post_commit_compound.take() else {
            return true;
        };
        let holder = resume_retryable_post_commit_holder(world, entry, holder);
        let PostCommitCompoundHolder::Active(active) = *holder else {
            entry.post_commit_compound = Some(holder);
            return true;
        };
        match active.core.settlement().progress() {
            PostCommitSettlementTraceProgress::OriginalResidual => {
                drive_post_commit_original_residual(world, entry, active);
            }
            PostCommitSettlementTraceProgress::AuthorityNonTop => {
                drive_post_commit_authority_target(world, entry, active, false);
            }
            PostCommitSettlementTraceProgress::RetiredHostEdge => {
                drive_post_commit_retired_host_edge(world, entry, active);
            }
            PostCommitSettlementTraceProgress::AuthorityTop => {
                drive_post_commit_authority_target(world, entry, active, true);
            }
            PostCommitSettlementTraceProgress::Complete => {
                drive_post_commit_completion(world, entry, active);
            }
        }
    }
    true
}
