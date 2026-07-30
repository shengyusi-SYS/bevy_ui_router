use std::sync::{Arc, Mutex};

use crate::{
    meta::ErrorBoundaryKey,
    pattern::RoutePattern,
    route_tree::{RouteNodeOptions, RouteTree},
    view_key::RouteViewKey,
};

type BackCapacityOrderJournal = Arc<Mutex<Vec<(u64, BackCapacityOrderEvent)>>>;

#[derive(Clone)]
pub struct BackCapacityOrderObserver {
    journal: BackCapacityOrderJournal,
}

pub struct BackCapacityOrderSnapshot {
    events: Vec<(u64, BackCapacityOrderEvent)>,
}

#[doc(hidden)]
pub struct BackCapacityIssuerProducer {
    journal: BackCapacityOrderJournal,
}

#[doc(hidden)]
pub struct BackCapacityCoreProducer {
    journal: BackCapacityOrderJournal,
}

#[doc(hidden)]
pub struct BackCapacityFacadeProducer {
    journal: BackCapacityOrderJournal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackCapacityIssuerEvent {
    Issued,
    Retired,
    TransferredNoTarget,
    BoundCanonicalZero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackCapacityOrderEvent {
    EcsIssuer(BackCapacityIssuerEvent),
    CoreAllocatorAttempt,
    CoreAllocatorFailure,
    FacadeTypedReturn,
}

impl BackCapacityOrderObserver {
    pub fn snapshot(&self) -> BackCapacityOrderSnapshot {
        let events = self
            .journal
            .lock()
            .expect("back capacity order journal lock must not be poisoned")
            .clone();
        BackCapacityOrderSnapshot { events }
    }

    pub fn observes_same_journal(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.journal, &other.journal)
    }
}

impl BackCapacityOrderSnapshot {
    pub fn events(&self) -> &[(u64, BackCapacityOrderEvent)] {
        &self.events
    }
}

fn record_back_capacity_order_event(
    journal: &BackCapacityOrderJournal,
    event: BackCapacityOrderEvent,
) {
    let mut journal = journal
        .lock()
        .expect("back capacity order journal lock must not be poisoned");
    let sequence = u64::try_from(journal.len()).expect("back capacity order sequence exhausted");
    journal.push((sequence, event));
}

#[doc(hidden)]
pub fn issue_back_capacity_order_channels() -> (
    BackCapacityOrderObserver,
    BackCapacityIssuerProducer,
    BackCapacityCoreProducer,
    BackCapacityFacadeProducer,
) {
    let journal = Arc::new(Mutex::new(Vec::new()));
    (
        BackCapacityOrderObserver {
            journal: journal.clone(),
        },
        BackCapacityIssuerProducer {
            journal: journal.clone(),
        },
        BackCapacityCoreProducer {
            journal: journal.clone(),
        },
        BackCapacityFacadeProducer { journal },
    )
}

impl BackCapacityIssuerProducer {
    pub fn record(&mut self, event: BackCapacityIssuerEvent) {
        record_back_capacity_order_event(&self.journal, BackCapacityOrderEvent::EcsIssuer(event));
    }
}

impl BackCapacityCoreProducer {
    pub fn allocator_attempt(&mut self) {
        record_back_capacity_order_event(
            &self.journal,
            BackCapacityOrderEvent::CoreAllocatorAttempt,
        );
    }

    pub fn allocator_failure(&mut self) {
        record_back_capacity_order_event(
            &self.journal,
            BackCapacityOrderEvent::CoreAllocatorFailure,
        );
    }
}

impl BackCapacityFacadeProducer {
    pub fn typed_return(&mut self) {
        record_back_capacity_order_event(&self.journal, BackCapacityOrderEvent::FacadeTypedReturn);
    }
}

pub fn options_with_error_boundary(
    options: RouteNodeOptions,
    boundary: ErrorBoundaryKey,
) -> RouteNodeOptions {
    options.with_error_boundary(boundary)
}

pub fn options_with_error_view(
    options: RouteNodeOptions,
    boundary: ErrorBoundaryKey,
    view: RouteViewKey,
) -> RouteNodeOptions {
    options.with_error_view(boundary, view)
}

pub fn nested_items_route_tree() -> RouteTree {
    let mut builder = RouteTree::builder()
        .root(RoutePattern::path("/").unwrap())
        .unwrap();
    let root = builder.root_id().unwrap();
    let items = builder
        .route(root, RoutePattern::path("items").unwrap())
        .unwrap();
    builder
        .route(items, RoutePattern::path(":id").unwrap())
        .unwrap();
    builder.build().unwrap()
}

pub fn nested_items_view_route_tree() -> RouteTree {
    let mut builder = RouteTree::builder()
        .root_with_options(
            RoutePattern::path("/").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("layout").unwrap()),
        )
        .unwrap();
    let root = builder.root_id().unwrap();
    builder
        .route_with_options(
            root,
            RoutePattern::path("items/:id").unwrap(),
            RouteNodeOptions::new().with_view(RouteViewKey::new("item").unwrap()),
        )
        .unwrap();
    builder.build().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_items_view_fixture_has_two_stable_view_bound_levels() {
        let tree = nested_items_view_route_tree();
        let nodes = tree.nodes_in_declaration_order();

        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].options().view_binding(),
            Some(&RouteViewKey::new("layout").unwrap())
        );
        assert_eq!(
            nodes[1].options().view_binding(),
            Some(&RouteViewKey::new("item").unwrap())
        );
    }
}
