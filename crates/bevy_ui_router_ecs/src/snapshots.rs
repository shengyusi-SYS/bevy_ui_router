#![allow(dead_code)]

use crate::app_ext::RouterHandle;
use bevy_ecs::resource::Resource;
use bevy_ui_router_core::diagnostics::DiagnosticEvent;
use bevy_ui_router_core::fingerprint::RouteBranchFingerprint;
use bevy_ui_router_core::integration::{MissingRequestClassification, RouterSnapshotProjection};
use bevy_ui_router_core::prelude::{
    ActivePresentation, CleanupSummary, DiagnosticSnapshot, HistorySnapshot, HostSnapshot,
    NavigationPhase, NavigationRequestId, NavigationTraceEvent, RequestStatus, RouteLocation,
    RouterAvailability,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRequestSnapshot {
    requested: Option<RouteLocation>,
    matched: Option<RouteBranchFingerprint>,
    status: RequestStatus,
    trace: Vec<NavigationTraceEvent>,
    diagnostics: Vec<DiagnosticEvent>,
}

impl NavigationRequestSnapshot {
    fn new(
        requested: Option<RouteLocation>,
        matched: Option<RouteBranchFingerprint>,
        status: RequestStatus,
        trace: Vec<NavigationTraceEvent>,
        diagnostics: Vec<DiagnosticEvent>,
    ) -> Self {
        Self {
            requested,
            matched,
            status,
            trace,
            diagnostics,
        }
    }

    pub fn requested(&self) -> Option<&RouteLocation> {
        self.requested.as_ref()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.matched.as_ref()
    }

    pub fn status(&self) -> &RequestStatus {
        &self.status
    }

    pub fn trace(&self) -> &[NavigationTraceEvent] {
        &self.trace
    }

    pub fn diagnostics(&self) -> &[DiagnosticEvent] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouterSnapshot {
    projection: RouterSnapshotProjection,
    cleanup: CleanupSummary,
}

impl RouterSnapshot {
    pub(crate) fn project(projection: RouterSnapshotProjection, cleanup: CleanupSummary) -> Self {
        Self {
            projection,
            cleanup,
        }
    }

    pub fn requested(&self) -> Option<&RouteLocation> {
        self.projection.requested()
    }

    pub fn matched(&self) -> Option<&RouteBranchFingerprint> {
        self.projection.matched()
    }

    pub fn active(&self) -> &ActivePresentation {
        self.projection.active()
    }

    pub fn availability(&self) -> &RouterAvailability {
        self.projection.availability()
    }

    pub fn pending_phase(&self) -> Option<&NavigationPhase> {
        self.projection.pending_phase()
    }

    pub fn history(&self) -> &HistorySnapshot {
        self.projection.history()
    }

    pub fn running_request(&self) -> Option<NavigationRequestId> {
        self.projection.running_request()
    }

    pub fn earliest_retained_sequence(&self) -> Option<u64> {
        self.projection.earliest_retained_sequence()
    }

    pub fn cleanup(&self) -> &CleanupSummary {
        &self.cleanup
    }

    pub fn host(&self) -> &HostSnapshot {
        self.projection.host()
    }

    pub fn diagnostics(&self) -> &DiagnosticSnapshot {
        self.projection.diagnostics()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestLookup<'a> {
    Found(&'a NavigationRequestSnapshot),
    NotRetained,
    Unknown,
}

struct RequestSnapshotEntry {
    router: RouterHandle,
    request: NavigationRequestId,
    snapshot: NavigationRequestSnapshot,
}

#[derive(Resource)]
pub struct RouterSnapshots {
    routers: Vec<(RouterHandle, RouterSnapshot)>,
    requests: Vec<RequestSnapshotEntry>,
}

impl RouterSnapshots {
    pub(crate) const fn new() -> Self {
        Self {
            routers: Vec::new(),
            requests: Vec::new(),
        }
    }

    pub fn get(&self, router: RouterHandle) -> Option<&RouterSnapshot> {
        self.routers
            .iter()
            .find_map(|(stored, snapshot)| (*stored == router).then_some(snapshot))
    }

    pub fn request(&self, router: RouterHandle, request: NavigationRequestId) -> RequestLookup<'_> {
        let Some(router_snapshot) = self.get(router) else {
            return RequestLookup::Unknown;
        };
        if let Some(snapshot) = self.requests.iter().find_map(|entry| {
            (entry.router == router && entry.request == request).then_some(&entry.snapshot)
        }) {
            return RequestLookup::Found(snapshot);
        }
        match router_snapshot
            .projection
            .request_retention()
            .classify(request)
        {
            MissingRequestClassification::NotRetained => RequestLookup::NotRetained,
            MissingRequestClassification::Unknown => RequestLookup::Unknown,
        }
    }

    pub(crate) fn install_router(
        &mut self,
        router: RouterHandle,
        projection: RouterSnapshotProjection,
    ) {
        if self.routers.iter().all(|(stored, _)| *stored != router) {
            self.routers.push((
                router,
                RouterSnapshot::project(
                    projection,
                    bevy_ui_router_core::integration::project_cleanup_summary(false, 0, None),
                ),
            ));
        }
    }

    pub(crate) fn publish(&mut self, entries: &mut [crate::navigation::RouterEntry]) {
        self.requests.clear();
        for entry in entries {
            entry.publish_history_admission_stamp();
            entry
                .requests
                .retain(|request| entry.engine.request(*request).is_some());
            let projected =
                RouterSnapshot::project(entry.engine.snapshot(), entry.cleanup_summary());
            if let Some((_, snapshot)) = self
                .routers
                .iter_mut()
                .find(|(stored, _)| *stored == entry.handle)
            {
                *snapshot = projected;
            } else {
                self.routers.push((entry.handle, projected));
            }
            for request in &entry.requests {
                let Some(view) = entry.engine.request(*request) else {
                    continue;
                };
                self.requests.push(RequestSnapshotEntry {
                    router: entry.handle,
                    request: *request,
                    snapshot: NavigationRequestSnapshot::new(
                        view.requested().cloned(),
                        view.matched().cloned(),
                        view.status().clone(),
                        view.trace().to_vec(),
                        view.diagnostics().to_vec(),
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::fingerprint::RouteBranchFingerprint;
    use bevy_ui_router_core::{
        integration::{CleanupManifest, EngineAction, RouterEngine},
        prelude::{
            NavigationRequestId, RequestStatus, RouteLocation, RouteNodeOptions, RoutePattern,
            RouteTree, RouteViewKey,
        },
    };

    fn confirmed_engine() -> RouterEngine {
        let view = RouteViewKey::new("root").unwrap();
        let tree = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(view),
            )
            .unwrap()
            .build()
            .unwrap();
        let mut engine =
            RouterEngine::install(tree, RouteViewKey::new("fallback").unwrap()).unwrap();
        let token = engine.request_host_admission().unwrap();
        let grant = engine.grant_host_admission(&token).unwrap();
        engine.record_top_slot_registered(&grant).unwrap();
        engine.confirm_host(grant);
        engine
    }

    fn settle_push(engine: &mut RouterEngine) -> NavigationRequestId {
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let action = engine.action().unwrap();
        let ticket = match action {
            EngineAction::FrameTask(task) => engine.report_applied(task).unwrap(),
            EngineAction::Commit(ticket) => ticket,
            EngineAction::MountRequest(_) => {
                panic!("depth-one snapshot fixture must not produce a nested mount action")
            }
            EngineAction::FailureCleanup(_) => {
                panic!("depth-one snapshot fixture must not produce nested failure cleanup")
            }
            EngineAction::BackResolution(ref handoff) => {
                let _scope = handoff.scope();
                panic!("push snapshot fixture must not produce a back resolution action")
            }
        };
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        let validated = engine.validate_publication(prepared).unwrap();
        validated.publish(|| {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        request
    }

    fn public_methods(type_name: &str) -> Vec<(String, bool)> {
        let file = syn::parse_file(include_str!("snapshots.rs")).unwrap();
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Impl(item) if item.trait_.is_none() => Some(item),
                _ => None,
            })
            .filter(|item| match item.self_ty.as_ref() {
                syn::Type::Path(path) => path.path.segments.last().unwrap().ident == type_name,
                _ => false,
            })
            .flat_map(|item| item.items.iter())
            .filter_map(|item| match item {
                syn::ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
                    Some((method.sig.ident.to_string(), method.sig.constness.is_some()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn associated_api_exposes_the_canonical_root_snapshot_surface() {
        assert_eq!(
            public_methods("NavigationRequestSnapshot"),
            vec![
                ("requested".into(), false),
                ("matched".into(), false),
                ("status".into(), false),
                ("trace".into(), false),
                ("diagnostics".into(), false),
            ]
        );
        assert_eq!(
            public_methods("RouterSnapshot"),
            vec![
                ("requested".into(), false),
                ("matched".into(), false),
                ("active".into(), false),
                ("availability".into(), false),
                ("pending_phase".into(), false),
                ("history".into(), false),
                ("running_request".into(), false),
                ("earliest_retained_sequence".into(), false),
                ("cleanup".into(), false),
                ("host".into(), false),
                ("diagnostics".into(), false),
            ]
        );
        assert_eq!(
            public_methods("RouterSnapshots"),
            vec![("get".into(), false), ("request".into(), false)]
        );
    }

    #[test]
    fn request_snapshot_exposes_only_frozen_read_only_data() {
        fn assert_traits<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
        assert_traits::<NavigationRequestSnapshot>();

        let requested = RouteLocation::new("/settings");
        let snapshot = NavigationRequestSnapshot::new(
            Some(requested.clone()),
            None,
            RequestStatus::Queued,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(snapshot.requested(), Some(&requested));
        assert_eq!(snapshot.matched(), None);
        assert_eq!(snapshot.status(), &RequestStatus::Queued);
        assert!(snapshot.trace().is_empty());
        assert!(snapshot.diagnostics().is_empty());
    }

    #[test]
    fn router_snapshot_has_final_value_traits() {
        fn assert_traits<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
        assert_traits::<RouterSnapshot>();
    }

    #[test]
    fn snapshot_resource_starts_empty_and_exposes_frozen_lookup_signatures() {
        fn assert_resource<T: bevy_ecs::resource::Resource>() {}
        fn assert_component<T: bevy_ecs::component::Component>() {}
        fn assert_lookup_traits<'a, T: Clone + std::fmt::Debug + Eq + PartialEq>() {}

        assert_resource::<RouterSnapshots>();
        assert_component::<RouterSnapshots>();
        assert_lookup_traits::<RequestLookup<'static>>();

        let snapshots = RouterSnapshots::new();
        let router = crate::app_ext::RouterHandle::fixture(3, 5);
        assert!(snapshots.get(router).is_none());

        let _: for<'a> fn(
            &'a RouterSnapshots,
            crate::app_ext::RouterHandle,
            NavigationRequestId,
        ) -> RequestLookup<'a> = RouterSnapshots::request;
        let _: Option<&RouteBranchFingerprint> = None;
    }

    #[test]
    fn frozen_request_lookup_distinguishes_retained_evicted_and_unknown_ids() {
        let mut engine = confirmed_engine();
        let requests = (0..66)
            .map(|_| settle_push(&mut engine))
            .collect::<Vec<_>>();
        let frozen = engine.snapshot();
        let newest = *requests.last().unwrap();
        let newest_view = engine.request(newest).unwrap();
        let router = crate::app_ext::RouterHandle::fixture(3, 5);
        let mut snapshots = RouterSnapshots::new();
        snapshots.install_router(router, frozen);
        snapshots.requests.push(RequestSnapshotEntry {
            router,
            request: newest,
            snapshot: NavigationRequestSnapshot::new(
                newest_view.requested().cloned(),
                newest_view.matched().cloned(),
                newest_view.status().clone(),
                newest_view.trace().to_vec(),
                newest_view.diagnostics().to_vec(),
            ),
        });

        assert!(matches!(
            snapshots.request(router, newest),
            RequestLookup::Found(_)
        ));
        assert_eq!(
            snapshots.request(router, requests[0]),
            RequestLookup::NotRetained
        );

        let future = engine.push(RouteLocation::new("/")).unwrap();
        assert_eq!(snapshots.request(router, future), RequestLookup::Unknown);

        let mut foreign = confirmed_engine();
        let foreign = foreign.push(RouteLocation::new("/")).unwrap();
        assert_eq!(snapshots.request(router, foreign), RequestLookup::Unknown);
        assert_eq!(
            snapshots.request(crate::app_ext::RouterHandle::fixture(3, 6), newest),
            RequestLookup::Unknown
        );
    }
}
