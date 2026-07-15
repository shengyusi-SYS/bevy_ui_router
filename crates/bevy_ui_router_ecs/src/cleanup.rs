use bevy_ecs::prelude::World;
use bevy_ui_router_core::integration::{CleanupBatch, CleanupCompletion};

use crate::{
    presentation::{RealizedFrame, settle_frame},
    registry::{ExactRegistryClaim, RouteEntityRegistry},
};

#[derive(Clone)]
struct CleanupObligation {
    claim: ExactRegistryClaim,
}

pub(crate) struct CleanupAttempt {
    sequence: u64,
    obligations: Vec<CleanupObligation>,
}

pub(crate) struct CleanupLedger {
    batch: CleanupBatch,
    leaving: Option<RealizedFrame>,
    obligations: Vec<CleanupObligation>,
    completion_digest: u64,
    next_attempt: u64,
    settled: bool,
}

impl CleanupLedger {
    pub(crate) fn new(
        batch: CleanupBatch,
        leaving: Option<RealizedFrame>,
        completion_digest: u64,
    ) -> Self {
        let settled = leaving.is_none();
        let obligations: Vec<CleanupObligation> = leaving
            .as_ref()
            .map(|frame| {
                frame
                    .claims()
                    .cloned()
                    .map(|claim| CleanupObligation { claim })
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            cleanup_batch_matches(&batch, completion_digest, obligations.len()),
            "sealed cleanup batch must match the frozen ECS manifest"
        );
        Self {
            batch,
            leaving,
            obligations,
            completion_digest,
            next_attempt: 1,
            settled,
        }
    }

    pub(crate) fn pending_completion(&self) -> Option<CleanupCompletion> {
        if !self.settled {
            return None;
        }
        let batch = self.batch.clone();
        Some(if self.obligations.is_empty() {
            batch.complete_empty()
        } else {
            batch.complete(self.completion_digest, self.obligations.len())
        })
    }

    #[cfg(feature = "test-support")]
    pub(crate) const fn leaving(&self) -> Option<&RealizedFrame> {
        self.leaving.as_ref()
    }

    pub(crate) fn attempt(&mut self, world: &World) -> Option<CleanupAttempt> {
        if self.settled {
            return None;
        }
        let registry = world.get_resource::<RouteEntityRegistry>()?;
        if self.obligations.iter().any(|obligation| {
            registry
                .record(obligation.claim.entity())
                .is_none_or(|record| record.claim() != obligation.claim)
        }) {
            return None;
        }
        let sequence = self.next_attempt;
        self.next_attempt = self
            .next_attempt
            .checked_add(1)
            .expect("cleanup attempt sequence exhausted");
        Some(CleanupAttempt {
            sequence,
            obligations: self.obligations.clone(),
        })
    }

    pub(crate) fn settle(&mut self, world: &mut World, attempt: CleanupAttempt) -> bool {
        if attempt.sequence + 1 != self.next_attempt
            || attempt.obligations.len() != self.obligations.len()
            || attempt
                .obligations
                .iter()
                .zip(&self.obligations)
                .any(|(actual, expected)| actual.claim != expected.claim)
        {
            return false;
        }
        if let Some(leaving) = self.leaving.take() {
            if let Err(leaving) = settle_frame(world, leaving) {
                self.leaving = Some(leaving);
                return false;
            }
        }
        self.settled = true;
        true
    }
}

pub(crate) fn cleanup_batch_matches(
    batch: &CleanupBatch,
    manifest_digest: u64,
    obligation_count: usize,
) -> bool {
    batch.manifest_digest() == manifest_digest && batch.obligation_count() == obligation_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::{
        integration::{CleanupManifest, EngineAction, RouterEngine},
        prelude::{RouteLocation, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey},
    };

    fn published_empty_batch() -> (RouterEngine, CleanupBatch) {
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(RouteViewKey::new("root").unwrap()),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine = RouterEngine::install(tree).unwrap();
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
    fn settled_completion_remains_replayable_until_core_acknowledges_it() {
        let (mut engine, batch) = published_empty_batch();
        let ledger = CleanupLedger::new(batch, None, 0);

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
}
