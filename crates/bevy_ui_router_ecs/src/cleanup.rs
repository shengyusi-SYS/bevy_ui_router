use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
};

use bevy_ecs::{
    component::ComponentId,
    hierarchy::{ChildOf, Children},
    prelude::{Entity, World},
};
use bevy_ui_router_core::integration::{CleanupBatch, CleanupCompletion};

use crate::{
    input::{RouteFocusState, prepare_focus_mutation},
    presentation::{
        RealizedFrame, prepare_cleanup_quarantine_mutation,
        repair_missing_pickable_for_cleanup_quarantine, settle_frame,
    },
    registry::{
        ComponentManifest, ExactRegistryClaim, QuarantineFacts, QuarantineLeaseTable,
        RouteEntityRegistry, apply_prepared_quarantine_transfer,
        prepare_verified_cleanup_quarantine_transfer,
    },
};
use bevy_input_focus::InputFocus;
use bevy_picking::Pickable;

const MANIFEST_DOMAIN: u64 = 0x4543_534d_414e_4946;
const ATTEMPT_DOMAIN: u64 = 0x4543_5341_5454_454d;
const RECEIPT_DOMAIN: u64 = 0x4543_5352_4543_5054;
const COMPLETION_DOMAIN: u64 = 0x4543_5343_4f4d_504c;
static NEXT_LEDGER_DOMAIN: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn fail_next_reservation_for_test() {
    FAIL_NEXT_RESERVATION.set(true);
}

#[cfg(test)]
fn reservation_fault_injected() -> bool {
    FAIL_NEXT_RESERVATION.replace(false)
}

#[cfg(not(test))]
const fn reservation_fault_injected() -> bool {
    false
}

fn allocate_ledger_domain(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1).filter(|_| current != 0)
        })
        .ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupDisposition {
    Removed,
    RemovedDetached,
    AlreadyAbsent,
    QuarantinedProtectedParent,
    QuarantinedForeignDescendant,
    QuarantinedForeignComponent,
}

impl CleanupDisposition {
    const fn tag(self) -> u64 {
        match self {
            Self::Removed => 1,
            Self::RemovedDetached => 2,
            Self::AlreadyAbsent => 3,
            Self::QuarantinedProtectedParent => 4,
            Self::QuarantinedForeignDescendant => 5,
            Self::QuarantinedForeignComponent => 6,
        }
    }

    pub(crate) const fn public(self) -> bevy_ui_router_core::prelude::CleanupDisposition {
        use bevy_ui_router_core::prelude::CleanupDisposition as Public;
        match self {
            Self::Removed => Public::Removed,
            Self::RemovedDetached => Public::RemovedDetached,
            Self::AlreadyAbsent => Public::AlreadyAbsent,
            Self::QuarantinedProtectedParent => Public::QuarantinedProtectedParent,
            Self::QuarantinedForeignDescendant => Public::QuarantinedForeignDescendant,
            Self::QuarantinedForeignComponent => Public::QuarantinedForeignComponent,
        }
    }

    const fn is_quarantine(self) -> bool {
        matches!(
            self,
            Self::QuarantinedProtectedParent
                | Self::QuarantinedForeignDescendant
                | Self::QuarantinedForeignComponent
        )
    }
}

#[derive(Default)]
pub(crate) struct RouterCleanupObservation {
    quarantine_count: usize,
    last_disposition: Option<bevy_ui_router_core::prelude::CleanupDisposition>,
}

impl RouterCleanupObservation {
    pub(crate) fn record(&mut self, disposition: CleanupDisposition, terminal_quarantine: bool) {
        self.last_disposition = Some(disposition.public());
        if terminal_quarantine {
            self.quarantine_count = self.quarantine_count.saturating_add(1);
        }
    }

    pub(crate) fn summary(&self, foreground_pending: bool) -> bevy_ui_router_core::CleanupSummary {
        bevy_ui_router_core::integration::project_cleanup_summary(
            foreground_pending,
            self.quarantine_count,
            self.last_disposition,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupObligation {
    claim: ExactRegistryClaim,
    manifest: ComponentManifest,
    expected_parent: Entity,
    unit_root: Entity,
    authorized_parent: Entity,
    allow_missing_components: bool,
}

pub(crate) struct VerifiedCleanupQuarantineTransfer {
    claims: Vec<ExactRegistryClaim>,
    manifests: Vec<(Entity, ComponentManifest)>,
    facts: QuarantineFacts,
}

impl VerifiedCleanupQuarantineTransfer {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrozenCleanupManifest {
    obligations: Vec<CleanupObligation>,
    digest: u64,
}

impl FrozenCleanupManifest {
    pub(crate) fn empty() -> Self {
        Self {
            obligations: Vec::new(),
            digest: 0,
        }
    }

    pub(crate) fn from_frame(
        _world: &World,
        registry: &RouteEntityRegistry,
        frame: &RealizedFrame,
        projected_claims: &[ExactRegistryClaim],
    ) -> Option<Self> {
        if projected_claims.len() != frame.obligation_count() {
            return None;
        }
        let mut obligations = frame
            .cleanup_members()
            .map(|(current, expected_parent)| {
                let projected = projected_claims
                    .iter()
                    .find(|claim| claim.entity() == current.entity())?;
                let record = registry.record(current.entity())?;
                if projected.owner() != current.owner() || projected.seal() != current.seal() {
                    return None;
                }
                if record.claim() != *current {
                    return None;
                }
                Some(CleanupObligation {
                    claim: projected.clone(),
                    manifest: record.manifest().clone(),
                    expected_parent,
                    unit_root: frame.shell(),
                    authorized_parent: frame.top_slot(),
                    allow_missing_components: !frame.prewrite_complete(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        order_leaf_to_root(&mut obligations)?;
        let digest = manifest_digest(&obligations);
        Some(Self {
            obligations,
            digest,
        })
    }

    pub(crate) fn from_current_frame(
        world: &World,
        registry: &RouteEntityRegistry,
        frame: &RealizedFrame,
    ) -> Option<Self> {
        let claims = frame.claims().cloned().collect::<Vec<_>>();
        Self::from_frame(world, registry, frame, &claims)
    }

    pub(crate) const fn digest(&self) -> u64 {
        self.digest
    }

    pub(crate) fn len(&self) -> usize {
        self.obligations.len()
    }
}

fn order_leaf_to_root(obligations: &mut [CleanupObligation]) -> Option<()> {
    let entities = obligations
        .iter()
        .map(|obligation| obligation.claim.entity())
        .collect::<HashSet<_>>();
    let mut depths = Vec::with_capacity(obligations.len());
    for obligation in obligations.iter() {
        let mut depth = 0_usize;
        let mut parent = obligation.expected_parent;
        let mut visited = HashSet::new();
        while entities.contains(&parent) {
            if !visited.insert(parent) {
                return None;
            }
            depth = depth.checked_add(1)?;
            parent = obligations
                .iter()
                .find(|candidate| candidate.claim.entity() == parent)?
                .expected_parent;
        }
        depths.push((obligation.claim.entity(), depth));
    }
    obligations.sort_by_key(|obligation| {
        let depth = depths
            .iter()
            .find_map(|(entity, depth)| (*entity == obligation.claim.entity()).then_some(*depth))
            .unwrap_or_default();
        (
            std::cmp::Reverse(depth),
            obligation.claim.entity().to_bits(),
        )
    });
    Some(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CleanupEvidence {
    AlreadyAbsent,
    Exact {
        actual_parent: Option<Entity>,
        normalized_manifest: ComponentManifest,
        foreign_descendants: Vec<Entity>,
        foreign_components: Vec<ComponentId>,
        missing_components: Vec<ComponentId>,
        protected_parent: Option<Entity>,
    },
    StaleClaim,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupAttempt {
    ledger_domain: u64,
    revision: u64,
    manifest_digest: u64,
    unsettled_digest: u64,
    evidence: Vec<(usize, CleanupEvidence)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CleanupReceipt {
    ledger_domain: u64,
    attempt_revision: u64,
    obligation: usize,
    disposition: CleanupDisposition,
    binding: u64,
    observed: bool,
}

pub(crate) struct PreparedCleanupLedger {
    batch: Option<CleanupBatch>,
    obligations: Vec<CleanupObligation>,
    manifest_digest: u64,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
}

impl PreparedCleanupLedger {
    pub(crate) fn bind_batch(mut self, batch: CleanupBatch) -> Self {
        self.batch = Some(batch);
        self
    }
}

pub(crate) struct CleanupLedger {
    batch: Option<CleanupBatch>,
    leaving: Option<RealizedFrame>,
    obligations: Vec<CleanupObligation>,
    manifest_digest: u64,
    receipts: Vec<Option<CleanupReceipt>>,
    ledger_domain: u64,
    next_attempt: u64,
    issued_attempt: Option<CleanupAttempt>,
    completion_digest: Option<u64>,
    quarantine_event_observed: bool,
}

impl CleanupLedger {
    #[cfg(test)]
    pub(crate) fn prepare(
        batch: CleanupBatch,
        manifest: FrozenCleanupManifest,
    ) -> Option<PreparedCleanupLedger> {
        if !cleanup_batch_matches(&batch, manifest.digest, manifest.obligations.len()) {
            return None;
        }
        Some(Self::reserve(manifest)?.bind_batch(batch))
    }

    pub(crate) fn reserve(manifest: FrozenCleanupManifest) -> Option<PreparedCleanupLedger> {
        Self::prepare_physical(None, manifest)
    }

    fn prepare_physical(
        batch: Option<CleanupBatch>,
        manifest: FrozenCleanupManifest,
    ) -> Option<PreparedCleanupLedger> {
        if reservation_fault_injected() {
            return None;
        }
        let mut receipts = Vec::new();
        receipts
            .try_reserve_exact(manifest.obligations.len())
            .ok()?;
        receipts.resize(manifest.obligations.len(), None);
        let ledger_domain = allocate_ledger_domain(&NEXT_LEDGER_DOMAIN)?;
        Some(PreparedCleanupLedger {
            batch,
            obligations: manifest.obligations,
            manifest_digest: manifest.digest,
            receipts,
            ledger_domain,
        })
    }

    pub(crate) fn for_failed_realization(
        world: &World,
        frame: RealizedFrame,
    ) -> Result<Self, RealizedFrame> {
        let manifest = {
            let Some(registry) = world.get_resource::<RouteEntityRegistry>() else {
                return Err(frame);
            };
            let Some(manifest) = FrozenCleanupManifest::from_current_frame(world, registry, &frame)
            else {
                return Err(frame);
            };
            manifest
        };
        let Some(prepared) = Self::reserve(manifest) else {
            return Err(frame);
        };
        Ok(Self::activate(prepared, Some(frame)))
    }

    pub(crate) fn activate(
        prepared: PreparedCleanupLedger,
        leaving: Option<RealizedFrame>,
    ) -> Self {
        let completion_digest = prepared
            .obligations
            .is_empty()
            .then_some(canonical_empty_completion_digest());
        Self {
            batch: prepared.batch,
            leaving,
            obligations: prepared.obligations,
            manifest_digest: prepared.manifest_digest,
            receipts: prepared.receipts,
            ledger_domain: prepared.ledger_domain,
            next_attempt: 1,
            issued_attempt: None,
            completion_digest,
            quarantine_event_observed: false,
        }
    }

    pub(crate) fn pending_completion(&self) -> Option<CleanupCompletion> {
        let completion_digest = self.completion_digest?;
        let batch = self.batch.clone()?;
        Some(if self.obligations.is_empty() {
            batch.complete_empty()
        } else {
            batch.complete(completion_digest, self.obligations.len())
        })
    }

    pub(crate) fn batch(&self) -> Option<&CleanupBatch> {
        self.batch.as_ref()
    }

    #[cfg(feature = "test-support")]
    pub(crate) const fn leaving(&self) -> Option<&RealizedFrame> {
        self.leaving.as_ref()
    }

    pub(crate) fn attempt(&mut self, world: &World) -> Option<CleanupAttempt> {
        if self.completion_digest.is_some() {
            return None;
        }
        let revision = self.next_attempt;
        self.next_attempt = self
            .next_attempt
            .checked_add(1)
            .expect("cleanup attempt revision exhausted");
        let attempt = CleanupAttempt {
            ledger_domain: self.ledger_domain,
            revision,
            manifest_digest: self.manifest_digest,
            unsettled_digest: unsettled_digest(
                self.manifest_digest,
                &self.obligations,
                &self.receipts,
            ),
            evidence: collect_evidence(world, &self.obligations, &self.receipts),
        };
        self.issued_attempt = Some(attempt.clone());
        Some(attempt)
    }

    pub(crate) const fn is_settled(&self) -> bool {
        self.completion_digest.is_some()
    }

    pub(crate) fn settle(&mut self, world: &mut World, attempt: CleanupAttempt) -> bool {
        if self.issued_attempt.as_ref() != Some(&attempt)
            || attempt.ledger_domain != self.ledger_domain
            || attempt.manifest_digest != self.manifest_digest
            || attempt.unsettled_digest
                != unsettled_digest(self.manifest_digest, &self.obligations, &self.receipts)
        {
            return false;
        }
        if collect_evidence(world, &self.obligations, &self.receipts) != attempt.evidence {
            self.issued_attempt = None;
            return false;
        }
        if attempt
            .evidence
            .iter()
            .any(|(_, evidence)| matches!(evidence, CleanupEvidence::StaleClaim))
        {
            self.issued_attempt = None;
            return false;
        }

        if let Some(disposition) = quarantine_disposition(&attempt.evidence) {
            if !self.quarantine_residual(world, &attempt, disposition) {
                self.issued_attempt = None;
                return false;
            }
            self.leaving.take();
            self.finish_if_settled();
            return self.completion_digest.is_some();
        }

        for (index, planned) in &attempt.evidence {
            if self.receipts[*index].is_some() {
                continue;
            }
            let current = classify_obligation(world, &self.obligations, &self.receipts, *index);
            if &current != planned {
                self.issued_attempt = None;
                return false;
            }
            let disposition = match current {
                CleanupEvidence::AlreadyAbsent => {
                    world
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(&self.obligations[*index].claim);
                    CleanupDisposition::AlreadyAbsent
                }
                CleanupEvidence::Exact { actual_parent, .. } => {
                    if !world.despawn(self.obligations[*index].claim.entity()) {
                        self.issued_attempt = None;
                        return false;
                    }
                    world.flush();
                    world
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(&self.obligations[*index].claim);
                    if actual_parent == Some(self.obligations[*index].expected_parent) {
                        CleanupDisposition::Removed
                    } else {
                        CleanupDisposition::RemovedDetached
                    }
                }
                CleanupEvidence::StaleClaim => unreachable!(),
            };
            let receipt = CleanupReceipt::new(
                self.ledger_domain,
                attempt.revision,
                *index,
                disposition,
                &self.obligations[*index],
            );
            if !self.accept_receipt(&attempt, receipt) {
                self.issued_attempt = None;
                return false;
            }
            let expected_remaining = attempt
                .evidence
                .iter()
                .filter(|(remaining, _)| self.receipts[*remaining].is_none())
                .cloned()
                .collect::<Vec<_>>();
            if collect_evidence(world, &self.obligations, &self.receipts) != expected_remaining {
                self.issued_attempt = None;
                return false;
            }
        }
        self.leaving.take();
        self.finish_if_settled();
        self.completion_digest.is_some()
    }

    fn quarantine_residual(
        &mut self,
        world: &mut World,
        attempt: &CleanupAttempt,
        disposition: CleanupDisposition,
    ) -> bool {
        let mut transfer_indices = Vec::new();
        for (index, evidence) in &attempt.evidence {
            if self.receipts[*index].is_some() {
                continue;
            }
            match evidence {
                CleanupEvidence::AlreadyAbsent => {
                    world
                        .resource_mut::<RouteEntityRegistry>()
                        .remove_exact(&self.obligations[*index].claim);
                    let receipt = CleanupReceipt::new(
                        self.ledger_domain,
                        attempt.revision,
                        *index,
                        CleanupDisposition::AlreadyAbsent,
                        &self.obligations[*index],
                    );
                    if !self.accept_receipt(attempt, receipt) {
                        return false;
                    }
                }
                CleanupEvidence::Exact { .. } => transfer_indices.push(*index),
                CleanupEvidence::StaleClaim => return false,
            }
        }
        if transfer_indices.is_empty() {
            return true;
        }

        let claims = transfer_indices
            .iter()
            .map(|index| self.obligations[*index].claim.clone())
            .collect::<Vec<_>>();
        let facts = quarantine_facts(&attempt.evidence);
        let focus_closure = quarantine_focus_closure(&attempt.evidence, &claims);
        let repair_preserved_attempt =
            if repair_missing_pickable_for_cleanup_quarantine(world, &claims) {
                let refreshed = collect_evidence(world, &self.obligations, &self.receipts);
                world
                    .components()
                    .component_id::<Pickable>()
                    .is_some_and(|pickable| {
                        evidence_matches_after_pickable_repair(
                            &attempt.evidence,
                            &refreshed,
                            &transfer_indices,
                            pickable,
                        )
                    })
            } else {
                false
            };
        let transferred = repair_preserved_attempt
            && transfer_cleanup_quarantine(world, claims.clone(), facts, focus_closure);
        if !transferred {
            if self.receipts.iter().all(Option::is_none)
                && transfer_indices.len() == self.obligations.len()
            {
                let Some(frame) = self.leaving.take() else {
                    return false;
                };
                match settle_frame(world, frame) {
                    Ok(()) => {
                        for index in transfer_indices {
                            let receipt = CleanupReceipt::new(
                                self.ledger_domain,
                                attempt.revision,
                                index,
                                disposition,
                                &self.obligations[index],
                            );
                            if !self.accept_receipt(attempt, receipt) {
                                return false;
                            }
                        }
                        return true;
                    }
                    Err(frame) => {
                        self.leaving = Some(frame);
                    }
                }
            }
            return false;
        }
        for index in transfer_indices {
            let receipt = CleanupReceipt::new(
                self.ledger_domain,
                attempt.revision,
                index,
                disposition,
                &self.obligations[index],
            );
            if !self.accept_receipt(attempt, receipt) {
                return false;
            }
        }
        true
    }

    fn accept_receipt(&mut self, attempt: &CleanupAttempt, receipt: CleanupReceipt) -> bool {
        if self.issued_attempt.as_ref() != Some(attempt)
            || receipt.ledger_domain != self.ledger_domain
            || receipt.attempt_revision != attempt.revision
            || receipt.obligation >= self.obligations.len()
            || self.receipts[receipt.obligation].is_some()
            || receipt.binding
                != receipt_binding(
                    receipt.ledger_domain,
                    receipt.attempt_revision,
                    receipt.obligation,
                    receipt.disposition,
                    &self.obligations[receipt.obligation],
                )
        {
            return false;
        }
        let obligation = receipt.obligation;
        self.receipts[obligation] = Some(receipt);
        true
    }

    fn finish_if_settled(&mut self) {
        if self.receipts.iter().all(Option::is_some) {
            self.completion_digest = Some(completion_digest(&self.receipts));
            self.issued_attempt = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts.iter().flatten().count()
    }

    pub(crate) fn next_unobserved_receipt(&self) -> Option<(usize, CleanupDisposition)> {
        self.receipts
            .iter()
            .enumerate()
            .find_map(|(index, receipt)| {
                receipt
                    .as_ref()
                    .filter(|receipt| !receipt.observed)
                    .map(|receipt| (index, receipt.disposition))
            })
    }

    pub(crate) fn mark_receipt_observed(
        &mut self,
        obligation: usize,
        disposition: CleanupDisposition,
    ) -> bool {
        let Some(receipt) = self.receipts.get_mut(obligation).and_then(Option::as_mut) else {
            return false;
        };
        if receipt.observed || receipt.disposition != disposition {
            return false;
        }
        receipt.observed = true;
        true
    }

    pub(crate) fn mark_quarantine_event_observed(
        &mut self,
        disposition: CleanupDisposition,
    ) -> bool {
        if !disposition.is_quarantine() || self.quarantine_event_observed {
            return false;
        }
        self.quarantine_event_observed = true;
        true
    }

    #[cfg(test)]
    fn disposition(&self, obligation: usize) -> Option<CleanupDisposition> {
        self.receipts
            .get(obligation)
            .and_then(Option::as_ref)
            .map(|receipt| receipt.disposition)
    }
}

fn transfer_cleanup_quarantine(
    world: &mut World,
    claims: Vec<ExactRegistryClaim>,
    facts: QuarantineFacts,
    focus_closure: Vec<Entity>,
) -> bool {
    let mut manifests = Vec::with_capacity(claims.len());
    for claim in &claims {
        let Some(manifest) = ComponentManifest::from_world(world, claim.entity()) else {
            return false;
        };
        manifests.push((claim.entity(), manifest));
    }
    let transfer =
        VerifiedCleanupQuarantineTransfer::after_world_validation(claims.clone(), manifests, facts);
    world.resource_scope(
        |world, mut registry: bevy_ecs::change_detection::Mut<RouteEntityRegistry>| {
            world.resource_scope(
                |world, mut table: bevy_ecs::change_detection::Mut<QuarantineLeaseTable>| {
                    let Ok(transfer) = prepare_verified_cleanup_quarantine_transfer(
                        &mut table, &registry, transfer,
                    ) else {
                        return false;
                    };
                    world.resource_scope(
                        |world, input_focus: bevy_ecs::change_detection::Mut<InputFocus>| {
                            world.resource_scope(
                                |world,
                                 route_focus: bevy_ecs::change_detection::Mut<RouteFocusState>| {
                                    let focus = prepare_focus_mutation(
                                        input_focus,
                                        route_focus,
                                        &focus_closure,
                                    );
                                    let Some(presentation) =
                                        prepare_cleanup_quarantine_mutation(world, &claims)
                                    else {
                                        return false;
                                    };
                                    presentation.apply();
                                    focus.apply();
                                    apply_prepared_quarantine_transfer(
                                        &mut table,
                                        &mut registry,
                                        transfer,
                                    );
                                    true
                                },
                            )
                        },
                    )
                },
            )
        },
    )
}

impl CleanupReceipt {
    fn new(
        ledger_domain: u64,
        attempt_revision: u64,
        obligation: usize,
        disposition: CleanupDisposition,
        identity: &CleanupObligation,
    ) -> Self {
        Self {
            ledger_domain,
            attempt_revision,
            obligation,
            disposition,
            binding: receipt_binding(
                ledger_domain,
                attempt_revision,
                obligation,
                disposition,
                identity,
            ),
            observed: false,
        }
    }
}

fn collect_evidence(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
) -> Vec<(usize, CleanupEvidence)> {
    obligations
        .iter()
        .enumerate()
        .filter(|(index, _)| receipts[*index].is_none())
        .map(|(index, _)| {
            (
                index,
                classify_obligation(world, obligations, receipts, index),
            )
        })
        .collect()
}

fn evidence_matches_after_pickable_repair(
    expected: &[(usize, CleanupEvidence)],
    actual: &[(usize, CleanupEvidence)],
    indices: &[usize],
    pickable: ComponentId,
) -> bool {
    if actual.len() != indices.len() {
        return false;
    }
    indices.iter().all(|index| {
        let Some((_, expected)) = expected.iter().find(|(candidate, _)| candidate == index) else {
            return false;
        };
        let Some((_, actual)) = actual.iter().find(|(candidate, _)| candidate == index) else {
            return false;
        };
        let (
            CleanupEvidence::Exact {
                actual_parent: expected_parent,
                normalized_manifest: expected_manifest,
                foreign_descendants: expected_descendants,
                foreign_components: expected_foreign,
                missing_components: expected_missing,
                protected_parent: expected_protected,
            },
            CleanupEvidence::Exact {
                actual_parent,
                normalized_manifest: actual_manifest,
                foreign_descendants: actual_descendants,
                foreign_components: actual_foreign,
                missing_components: actual_missing,
                protected_parent: actual_protected,
            },
        ) = (expected, actual)
        else {
            return false;
        };
        expected_parent == actual_parent
            && expected_descendants == actual_descendants
            && expected_protected == actual_protected
            && components_without(expected_manifest.component_ids(), pickable)
                == components_without(actual_manifest.component_ids(), pickable)
            && components_without(expected_foreign, pickable)
                == components_without(actual_foreign, pickable)
            && components_without(expected_missing, pickable)
                == components_without(actual_missing, pickable)
    })
}

fn components_without(components: &[ComponentId], excluded: ComponentId) -> Vec<ComponentId> {
    components
        .iter()
        .copied()
        .filter(|component| *component != excluded)
        .collect()
}

fn classify_obligation(
    world: &World,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
    index: usize,
) -> CleanupEvidence {
    let obligation = &obligations[index];
    if world.get_entity(obligation.claim.entity()).is_err() {
        return CleanupEvidence::AlreadyAbsent;
    }
    let Some(record) = world
        .get_resource::<RouteEntityRegistry>()
        .and_then(|registry| registry.record(obligation.claim.entity()))
    else {
        return CleanupEvidence::StaleClaim;
    };
    if record.claim() != obligation.claim {
        return CleanupEvidence::StaleClaim;
    }
    let Some(actual_manifest) = ComponentManifest::from_world(world, obligation.claim.entity())
    else {
        return CleanupEvidence::AlreadyAbsent;
    };
    let relationship_components = [
        world.components().component_id::<ChildOf>(),
        world.components().component_id::<Children>(),
    ]
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>();
    let normalized_manifest = ComponentManifest::new(
        actual_manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| !relationship_components.contains(component)),
    );
    let mut foreign_components = actual_manifest
        .component_ids()
        .iter()
        .copied()
        .filter(|component| {
            !obligation.manifest.component_ids().contains(component)
                && !relationship_components.contains(component)
        })
        .collect::<Vec<_>>();
    foreign_components.sort_unstable();
    foreign_components.dedup();
    let mut missing_components = if obligation.allow_missing_components {
        Vec::new()
    } else {
        obligation
            .manifest
            .component_ids()
            .iter()
            .copied()
            .filter(|component| {
                !actual_manifest.component_ids().contains(component)
                    && !relationship_components.contains(component)
            })
            .collect::<Vec<_>>()
    };
    missing_components.sort_unstable();
    missing_components.dedup();

    let unsettled_owned = obligations
        .iter()
        .enumerate()
        .filter_map(|(candidate, obligation)| {
            receipts[candidate]
                .is_none()
                .then_some(obligation.claim.entity())
        })
        .collect::<HashSet<_>>();
    let mut foreign_descendants = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = world
        .get::<Children>(obligation.claim.entity())
        .into_iter()
        .flat_map(|children| children.iter().copied())
        .collect::<Vec<_>>();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            continue;
        }
        if !unsettled_owned.contains(&entity) {
            foreign_descendants.push(entity);
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter().copied());
        }
    }
    foreign_descendants.sort_by_key(|entity| entity.to_bits());
    foreign_descendants.dedup();

    let actual_parent = world
        .get::<ChildOf>(obligation.claim.entity())
        .map(ChildOf::parent);
    let protected_parent = actual_parent.filter(|parent| {
        *parent != obligation.expected_parent
            && *parent != obligation.authorized_parent
            && !unsettled_owned.contains(parent)
    });
    CleanupEvidence::Exact {
        actual_parent,
        normalized_manifest,
        foreign_descendants,
        foreign_components,
        missing_components,
        protected_parent,
    }
}

fn quarantine_disposition(evidence: &[(usize, CleanupEvidence)]) -> Option<CleanupDisposition> {
    if evidence.iter().any(|(_, evidence)| {
        matches!(
            evidence,
            CleanupEvidence::Exact {
                protected_parent: Some(_),
                ..
            }
        )
    }) {
        return Some(CleanupDisposition::QuarantinedProtectedParent);
    }
    if evidence.iter().any(|(_, evidence)| {
        matches!(
            evidence,
            CleanupEvidence::Exact {
                foreign_descendants,
                ..
            } if !foreign_descendants.is_empty()
        )
    }) {
        return Some(CleanupDisposition::QuarantinedForeignDescendant);
    }
    evidence
        .iter()
        .any(|(_, evidence)| {
            matches!(
                evidence,
                CleanupEvidence::Exact {
                    foreign_components,
                    missing_components,
                    ..
                } if !foreign_components.is_empty() || !missing_components.is_empty()
            )
        })
        .then_some(CleanupDisposition::QuarantinedForeignComponent)
}

fn quarantine_facts(evidence: &[(usize, CleanupEvidence)]) -> QuarantineFacts {
    let mut protected_parents: Vec<Entity> = Vec::new();
    let mut foreign_descendants: Vec<Entity> = Vec::new();
    let mut foreign_components: Vec<ComponentId> = Vec::new();
    let mut missing_components: Vec<ComponentId> = Vec::new();
    for (_, evidence) in evidence {
        if let CleanupEvidence::Exact {
            protected_parent,
            foreign_descendants: descendants,
            foreign_components: components,
            missing_components: missing,
            ..
        } = evidence
        {
            protected_parents.extend(protected_parent);
            foreign_descendants.extend(descendants);
            foreign_components.extend(components);
            missing_components.extend(missing);
        }
    }
    protected_parents.sort_by_key(|entity| entity.to_bits());
    protected_parents.dedup();
    QuarantineFacts::with_component_drift(
        None,
        protected_parents.first().copied(),
        foreign_descendants,
        foreign_components,
        missing_components,
    )
}

fn quarantine_focus_closure(
    evidence: &[(usize, CleanupEvidence)],
    claims: &[ExactRegistryClaim],
) -> Vec<Entity> {
    let mut entities = claims
        .iter()
        .map(ExactRegistryClaim::entity)
        .collect::<Vec<_>>();
    for (_, evidence) in evidence {
        if let CleanupEvidence::Exact {
            foreign_descendants,
            ..
        } = evidence
        {
            entities.extend(foreign_descendants);
        }
    }
    entities.sort_by_key(|entity| entity.to_bits());
    entities.dedup();
    entities
}

fn manifest_digest(obligations: &[CleanupObligation]) -> u64 {
    let mut digest = StructuredDigest::new(MANIFEST_DOMAIN);
    digest.push(obligations.len() as u64);
    for obligation in obligations {
        digest.obligation(obligation);
    }
    digest.finish()
}

fn unsettled_digest(
    manifest_digest: u64,
    obligations: &[CleanupObligation],
    receipts: &[Option<CleanupReceipt>],
) -> u64 {
    let mut digest = StructuredDigest::new(ATTEMPT_DOMAIN);
    digest.push(manifest_digest);
    for (index, obligation) in obligations.iter().enumerate() {
        if receipts[index].is_none() {
            digest.push(index as u64);
            digest.obligation(obligation);
        }
    }
    digest.finish()
}

fn receipt_binding(
    ledger_domain: u64,
    attempt_revision: u64,
    obligation: usize,
    disposition: CleanupDisposition,
    identity: &CleanupObligation,
) -> u64 {
    let mut digest = StructuredDigest::new(RECEIPT_DOMAIN);
    digest.push(ledger_domain);
    digest.push(attempt_revision);
    digest.push(obligation as u64);
    digest.push(disposition.tag());
    digest.obligation(identity);
    digest.finish()
}

fn completion_digest(receipts: &[Option<CleanupReceipt>]) -> u64 {
    let mut digest = StructuredDigest::new(COMPLETION_DOMAIN);
    digest.push(receipts.len() as u64);
    for receipt in receipts.iter().flatten() {
        digest.push(receipt.obligation as u64);
        digest.push(receipt.disposition.tag());
        digest.push(receipt.binding);
    }
    digest.finish()
}

fn canonical_empty_completion_digest() -> u64 {
    StructuredDigest::new(COMPLETION_DOMAIN).finish()
}

struct StructuredDigest(u64);

impl StructuredDigest {
    const fn new(domain: u64) -> Self {
        Self(0xcbf2_9ce4_8422_2325 ^ domain)
    }

    fn push(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.push(bytes.len() as u64);
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn obligation(&mut self, obligation: &CleanupObligation) {
        self.push(obligation.claim.entity().to_bits());
        self.bytes(format!("{:?}", obligation.claim.owner()).as_bytes());
        self.bytes(format!("{:?}", obligation.claim.seal()).as_bytes());
        self.push(obligation.manifest.component_ids().len() as u64);
        for component in obligation.manifest.component_ids() {
            self.push(component.index() as u64);
        }
        self.push(obligation.expected_parent.to_bits());
        self.push(obligation.unit_root.to_bits());
        self.push(obligation.authorized_parent.to_bits());
        self.push(u64::from(obligation.allow_missing_components));
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
pub(crate) fn cleanup_batch_matches(
    batch: &CleanupBatch,
    manifest_digest: u64,
    obligation_count: usize,
) -> bool {
    batch.manifest_digest() == manifest_digest && batch.obligation_count() == obligation_count
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        host::HostCandidateRevision,
        registry::{
            ComponentManifest, EntityLifecycle, ProvisionalTopSlotOwner, QuarantineLeaseTable,
            RegistryOwner, RouteEntityRegistry,
        },
    };
    use bevy_ecs::{component::Component, hierarchy::ChildOf};
    use bevy_ui_router_core::{
        integration::{CleanupManifest, EngineAction, RouterEngine},
        prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
    };

    fn published_empty_batch() -> (RouterEngine, CleanupBatch) {
        let view = RouteViewKey::new("root").unwrap();
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine =
            RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.confirm_host(grant);
        engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let EngineAction::FrameTask(task) = engine.action().unwrap() else {
            panic!("root task expected")
        };
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let batch = prepared.cleanup().clone();
        engine.publish(prepared, || {});
        (engine, batch)
    }

    #[test]
    fn cleanup_attempt_identity_is_monotonic_and_private() {
        let source = include_str!("cleanup.rs");
        assert!(source.contains("next_attempt"));
        assert!(!source.contains("\npub struct CleanupAttempt"));
        assert!(!source.contains("\npub fn complete("));
    }

    #[test]
    fn exhausted_ledger_domain_never_advances_or_reuses_authority() {
        let counter = AtomicU64::new(u64::MAX - 1);

        assert_eq!(allocate_ledger_domain(&counter), Some(u64::MAX - 1));
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        assert_eq!(allocate_ledger_domain(&counter), None);
        assert_eq!(allocate_ledger_domain(&counter), None);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn settled_completion_remains_replayable_until_core_acknowledges_it() {
        let (mut engine, batch) = published_empty_batch();
        let prepared = CleanupLedger::prepare(batch, FrozenCleanupManifest::empty()).unwrap();
        let ledger = CleanupLedger::activate(prepared, None);
        let first = ledger.pending_completion().unwrap();
        let replay = ledger.pending_completion().unwrap();
        engine.complete_cleanup(first).unwrap();
        assert!(engine.complete_cleanup(replay).is_err());
    }

    #[test]
    fn sealed_batch_must_match_the_frozen_ecs_manifest() {
        let (_engine, batch) = published_empty_batch();
        assert!(cleanup_batch_matches(&batch, 0, 0));
        assert!(!cleanup_batch_matches(&batch, 1, 0));
        assert!(!cleanup_batch_matches(&batch, 0, 1));
    }

    fn one_entity_failure_ledger(
        world: &mut World,
        entity: Entity,
        expected_parent: Entity,
    ) -> CleanupLedger {
        let claim = world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .unwrap()
            .claim();
        let manifest = world
            .resource::<RouteEntityRegistry>()
            .record(entity)
            .unwrap()
            .manifest()
            .clone();
        let obligations = vec![CleanupObligation {
            claim,
            manifest,
            expected_parent,
            unit_root: entity,
            authorized_parent: expected_parent,
            allow_missing_components: false,
        }];
        let manifest = FrozenCleanupManifest {
            digest: manifest_digest(&obligations),
            obligations,
        };
        let prepared = CleanupLedger::prepare_physical(None, manifest).unwrap();
        CleanupLedger::activate(prepared, None)
    }

    fn registered_entity(world: &mut World, parent: Entity) -> Entity {
        let entity = world.spawn(ChildOf(parent)).id();
        let manifest = ComponentManifest::from_world(world, entity).unwrap();
        let mut registry = world.resource_mut::<RouteEntityRegistry>();
        let seal = registry.issue_seal();
        registry
            .register(
                entity,
                RegistryOwner::Provisional(ProvisionalTopSlotOwner::new(
                    HostCandidateRevision::fixture(entity.to_bits().wrapping_add(1)),
                )),
                seal,
                manifest,
                EntityLifecycle::PreparedInert,
            )
            .unwrap();
        entity
    }

    #[test]
    fn exact_clean_removal_despawns_and_removes_the_registry_claim() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get_entity(entity).is_err());
        assert!(
            world
                .resource::<RouteEntityRegistry>()
                .record(entity)
                .is_none()
        );
        assert_eq!(ledger.disposition(0), Some(CleanupDisposition::Removed));
    }

    #[test]
    fn absent_exact_generation_never_touches_a_reused_index() {
        #[derive(Component)]
        struct Replacement;

        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let replacement = world.despawn_no_free(entity).unwrap();
        world.spawn_at(replacement, Replacement).unwrap();
        assert_eq!(replacement.index(), entity.index());
        assert_ne!(replacement, entity);

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get::<Replacement>(replacement).is_some());
        assert_eq!(
            ledger.disposition(0),
            Some(CleanupDisposition::AlreadyAbsent)
        );
    }

    #[test]
    fn detached_exact_entity_uses_removed_detached() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let expected_parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, expected_parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, expected_parent);
        world.entity_mut(entity).remove::<ChildOf>();

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));

        assert!(world.get_entity(entity).is_err());
        assert_eq!(
            ledger.disposition(0),
            Some(CleanupDisposition::RemovedDetached)
        );
    }

    #[test]
    fn manifest_receipt_and_completion_digests_are_domain_separated() {
        let manifest = StructuredDigest::new(MANIFEST_DOMAIN).finish();
        let attempt = StructuredDigest::new(ATTEMPT_DOMAIN).finish();
        let receipt = StructuredDigest::new(RECEIPT_DOMAIN).finish();
        let completion = StructuredDigest::new(COMPLETION_DOMAIN).finish();
        let values = [manifest, attempt, receipt, completion];
        for (index, value) in values.iter().enumerate() {
            assert!(!values[index + 1..].contains(value));
        }
    }

    #[test]
    fn old_and_foreign_attempts_are_world_zero() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let old = ledger.attempt(&world).unwrap();
        let current = ledger.attempt(&world).unwrap();
        let mut foreign = current.clone();
        foreign.ledger_domain = foreign.ledger_domain.wrapping_add(1);

        assert!(!ledger.settle(&mut world, old));
        assert!(!ledger.settle(&mut world, foreign));
        assert!(world.get_entity(entity).is_ok());
        assert_eq!(ledger.receipt_count(), 0);

        assert!(ledger.settle(&mut world, current));
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    pub(crate) fn foreign_and_replayed_receipts_are_ledger_zero() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        let attempt = ledger.attempt(&world).unwrap();
        let foreign_domain = ledger.ledger_domain.wrapping_add(1);
        let foreign = CleanupReceipt::new(
            foreign_domain,
            attempt.revision,
            0,
            CleanupDisposition::Removed,
            &ledger.obligations[0],
        );

        assert!(!ledger.accept_receipt(&attempt, foreign));
        assert_eq!(ledger.receipt_count(), 0);

        assert!(ledger.settle(&mut world, attempt.clone()));
        let replay = ledger.receipts[0].clone().unwrap();
        let receipts_before = ledger.receipts.clone();
        let registry_before = world.resource::<RouteEntityRegistry>().records().to_vec();
        let world_before = world
            .iter_entities()
            .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
            .collect::<Vec<_>>();
        assert!(!ledger.accept_receipt(&attempt, replay));
        assert_eq!(ledger.receipts, receipts_before);
        assert_eq!(ledger.receipt_count(), 1);
        assert_eq!(
            world.resource::<RouteEntityRegistry>().records(),
            registry_before
        );
        assert_eq!(
            world
                .iter_entities()
                .map(|entity| (entity.id(), entity.archetype().components().to_vec()))
                .collect::<Vec<_>>(),
            world_before
        );
    }

    #[test]
    fn cleanup_receipt_observation_is_exact_once() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);

        let attempt = ledger.attempt(&world).unwrap();
        assert!(ledger.settle(&mut world, attempt));
        let (obligation, disposition) = ledger
            .next_unobserved_receipt()
            .expect("settlement must expose one unobserved receipt");

        assert!(ledger.mark_receipt_observed(obligation, disposition));
        assert!(ledger.next_unobserved_receipt().is_none());
        assert!(!ledger.mark_receipt_observed(obligation, disposition));
    }

    #[test]
    fn partial_receipt_remains_observable_before_aggregate_completion() {
        let mut world = World::new();
        world.insert_resource(RouteEntityRegistry::new());
        world.insert_resource(QuarantineLeaseTable::new());
        let parent = world.spawn_empty().id();
        let entity = registered_entity(&mut world, parent);
        let mut ledger = one_entity_failure_ledger(&mut world, entity, parent);
        ledger.obligations.push(ledger.obligations[0].clone());
        ledger.receipts.push(None);
        ledger.manifest_digest = manifest_digest(&ledger.obligations);
        let attempt = ledger.attempt(&world).unwrap();
        let receipt = CleanupReceipt::new(
            ledger.ledger_domain,
            attempt.revision,
            0,
            CleanupDisposition::Removed,
            &ledger.obligations[0],
        );

        assert!(ledger.accept_receipt(&attempt, receipt));
        ledger.finish_if_settled();
        assert!(!ledger.is_settled());
        assert_eq!(
            ledger.next_unobserved_receipt(),
            Some((0, CleanupDisposition::Removed))
        );
        assert!(ledger.mark_receipt_observed(0, CleanupDisposition::Removed));
        assert!(ledger.next_unobserved_receipt().is_none());
    }

    #[test]
    fn router_cleanup_observation_counts_terminal_quarantine_events_not_receipts() {
        let mut observation = RouterCleanupObservation::default();
        observation.record(CleanupDisposition::Removed, false);
        observation.record(CleanupDisposition::QuarantinedProtectedParent, true);
        observation.record(CleanupDisposition::QuarantinedProtectedParent, false);

        let pending = observation.summary(true);
        assert!(pending.foreground_pending());
        assert_eq!(pending.quarantine_count(), 1);
        assert_eq!(
            pending.last_disposition(),
            Some(bevy_ui_router_core::prelude::CleanupDisposition::QuarantinedProtectedParent)
        );
    }
}
