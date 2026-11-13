#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use crate::app_ext::RouterHandle;
use bevy_ecs::resource::Resource;
use bevy_ui_router_core::diagnostics::DiagnosticEvent;
use bevy_ui_router_core::fingerprint::RouteBranchFingerprint;
use bevy_ui_router_core::integration::{
    MissingRequestClassification, NavigationRequestView, RouterSnapshotProjection,
};
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

// Coherence metadata belongs to the request-keyed store, not the public value.
// In particular it must not affect Snapshot equality or resource change ticks.
struct StoredRequest {
    snapshot: NavigationRequestSnapshot,
    trace_closed: bool,
}

impl StoredRequest {
    fn from_view(view: &NavigationRequestView<'_>) -> Self {
        Self {
            snapshot: NavigationRequestSnapshot::new(
                view.requested().cloned(),
                view.matched().cloned(),
                view.status().clone(),
                view.trace().to_vec(),
                view.diagnostics().to_vec(),
            ),
            trace_closed: view.trace_closed(),
        }
    }

    fn trace_matches(&self, view: &NavigationRequestView<'_>) -> bool {
        // Callers select the exact (RouterHandle, NavigationRequestId) first.
        // Core never reopens or replaces a closed trace for that request.
        (self.trace_closed && view.trace_closed()) || self.snapshot.trace == view.trace()
    }

    fn matches(&self, view: &NavigationRequestView<'_>) -> bool {
        self.snapshot.requested.as_ref() == view.requested()
            && self.snapshot.matched.as_ref() == view.matched()
            && &self.snapshot.status == view.status()
            && self.trace_matches(view)
            && self.snapshot.diagnostics == view.diagnostics()
    }

    fn refresh(&mut self, view: &NavigationRequestView<'_>) -> bool {
        let mut changed = false;
        if self.snapshot.requested.as_ref() != view.requested() {
            self.snapshot.requested = view.requested().cloned();
            changed = true;
        }
        if self.snapshot.matched.as_ref() != view.matched() {
            self.snapshot.matched = view.matched().cloned();
            changed = true;
        }
        if &self.snapshot.status != view.status() {
            self.snapshot.status = view.status().clone();
            changed = true;
        }
        if !self.trace_matches(view) {
            self.snapshot.trace = view.trace().to_vec();
            changed = true;
        }
        if self.snapshot.diagnostics != view.diagnostics() {
            self.snapshot.diagnostics = view.diagnostics().to_vec();
            changed = true;
        }
        self.trace_closed = view.trace_closed();
        changed
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

#[derive(Resource)]
pub struct RouterSnapshots {
    routers: Vec<(RouterHandle, RouterSnapshot)>,
    requests: HashMap<(RouterHandle, NavigationRequestId), StoredRequest>,
}

impl RouterSnapshots {
    pub(crate) fn new() -> Self {
        Self {
            routers: Vec::new(),
            requests: HashMap::new(),
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
        if let Some(snapshot) = self.requests.get(&(router, request)) {
            return RequestLookup::Found(&snapshot.snapshot);
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

    fn matches(&self, entries: &[crate::navigation::RouterEntry]) -> bool {
        let mut request_count = 0;
        for entry in entries {
            let Some(snapshot) = self.get(entry.handle) else {
                return false;
            };
            if !entry.engine.snapshot_matches(&snapshot.projection)
                || entry.cleanup_summary() != snapshot.cleanup
            {
                return false;
            }
            let views = entry.engine.requests();
            if entry.requests.len() != views.len()
                || entry
                    .requests
                    .iter()
                    .any(|request| !self.requests.contains_key(&(entry.handle, *request)))
            {
                return false;
            }
            for view in views {
                request_count += 1;
                if !self
                    .requests
                    .get(&(entry.handle, view.id()))
                    .is_some_and(|snapshot| snapshot.matches(&view))
                {
                    return false;
                }
            }
        }
        request_count == self.requests.len()
    }

    /// Returns whether any publicly readable snapshot value changed.
    pub(crate) fn publish(&mut self, entries: &mut [crate::navigation::RouterEntry]) -> bool {
        for entry in &mut *entries {
            entry.publish_history_admission_stamp();
        }
        if self.matches(entries) {
            return false;
        }
        let mut changed = false;
        let mut retained = HashSet::new();
        for entry in entries {
            // A shallow borrowed index replaces two scans of all Core records
            // for each request. It is only needed when published values differ.
            let views = entry
                .engine
                .requests()
                .map(|view| (view.id(), view))
                .collect::<HashMap<_, _>>();
            entry.requests.retain(|request| views.contains_key(request));
            let cleanup = entry.cleanup_summary();
            if let Some((_, snapshot)) = self
                .routers
                .iter_mut()
                .find(|(stored, _)| *stored == entry.handle)
            {
                if cleanup != snapshot.cleanup
                    || !entry.engine.snapshot_matches(&snapshot.projection)
                {
                    *snapshot = RouterSnapshot::project(entry.engine.snapshot(), cleanup);
                    changed = true;
                }
            } else {
                self.routers.push((
                    entry.handle,
                    RouterSnapshot::project(entry.engine.snapshot(), cleanup),
                ));
                changed = true;
            }
            for request in &entry.requests {
                let view = &views[request];
                let key = (entry.handle, *request);
                retained.insert(key);
                if let Some(stored) = self.requests.get_mut(&key) {
                    changed |= stored.refresh(view);
                } else {
                    self.requests.insert(key, StoredRequest::from_view(view));
                    changed = true;
                }
            }
        }
        let previous_count = self.requests.len();
        self.requests.retain(|key, _| retained.contains(key));
        changed || previous_count != self.requests.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use bevy_ecs::{
        change_detection::DetectChanges,
        prelude::{Res, ResMut},
        schedule::IntoScheduleConfigs,
    };
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

    fn snapshot_app() -> (App, RouterHandle) {
        let mut app = App::new();
        let router = crate::navigation::tests::install_retained_slice_router(&mut app, 0);
        app.update();
        (app, router)
    }

    fn settle_app_request(app: &mut App, router: RouterHandle, request: NavigationRequestId) {
        for _ in 0..32 {
            app.update();
            if matches!(
                app.world().resource::<RouterSnapshots>().request(router, request),
                RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Succeeded
            ) {
                return;
            }
        }
        panic!("the public snapshot did not reach a successful terminal request");
    }

    #[derive(Resource, Default)]
    struct SnapshotChanges(Vec<bool>);

    fn observe_snapshot_changes(
        snapshots: Res<RouterSnapshots>,
        mut changes: ResMut<SnapshotChanges>,
    ) {
        changes.0.push(snapshots.is_changed());
    }

    #[test]
    fn idle_publication_keeps_snapshot_storage_and_change_detection_stable() {
        let (mut app, router) = snapshot_app();
        app.init_resource::<SnapshotChanges>().add_systems(
            Update,
            observe_snapshot_changes.after(crate::RouterScheduleSet::PublishSnapshots),
        );
        let request = app
            .world_mut()
            .resource_mut::<crate::RouterNavigation>()
            .push(router, RouteLocation::new("/items/1"))
            .unwrap();
        settle_app_request(&mut app, router, request);
        assert!(
            app.world()
                .resource::<SnapshotChanges>()
                .0
                .iter()
                .any(|changed| *changed)
        );
        let (trace, history) = {
            let snapshots = app.world().resource::<RouterSnapshots>();
            let RequestLookup::Found(request) = snapshots.request(router, request) else {
                panic!("the successful request is retained");
            };
            assert!(!request.trace().is_empty());
            (
                request.trace().as_ptr(),
                snapshots.get(router).unwrap().history().entries().as_ptr(),
            )
        };
        app.world_mut().resource_mut::<SnapshotChanges>().0.clear();
        app.update();
        app.update();
        assert_eq!(app.world().resource::<SnapshotChanges>().0, [false, false]);
        let snapshots = app.world().resource::<RouterSnapshots>();
        let RequestLookup::Found(retained) = snapshots.request(router, request) else {
            panic!("the successful request remains retained");
        };
        assert_eq!(retained.trace().as_ptr(), trace);
        assert_eq!(
            snapshots.get(router).unwrap().history().entries().as_ptr(),
            history
        );
    }

    #[test]
    fn terminal_request_comparison_includes_late_report_diagnostics() {
        let mut engine = confirmed_engine();
        let request = engine.push(RouteLocation::new("/")).unwrap();
        engine.collect();
        let Some(EngineAction::FrameTask(task)) = engine.action() else {
            panic!("the real initial request issues a frame task");
        };
        let duplicate = task.testing_duplicate();
        let ticket = engine.report_applied(task).unwrap();
        let prepared = engine
            .prepare_commit(&ticket, CleanupManifest::empty())
            .unwrap();
        let cleanup = prepared.cleanup().clone();
        engine
            .validate_publication(prepared)
            .unwrap()
            .publish(|| {});
        engine.complete_cleanup(cleanup.complete_empty()).unwrap();
        let view = engine.request(request).unwrap();
        let mut frozen = StoredRequest::from_view(&view);
        let trace = frozen.snapshot.trace().as_ptr();
        assert!(frozen.matches(&view));
        assert!(engine.validate_report_identity(&duplicate).is_err());
        let observed = engine.requests().find(|view| view.id() == request).unwrap();
        assert_eq!(observed.status(), &RequestStatus::Succeeded);
        assert_eq!(observed.trace(), frozen.snapshot.trace());
        assert_eq!(
            observed.diagnostics().len(),
            frozen.snapshot.diagnostics().len() + 1
        );
        assert!(!frozen.matches(&observed));
        assert!(frozen.refresh(&observed));
        assert_eq!(frozen.snapshot.trace().as_ptr(), trace);
        assert_eq!(frozen.snapshot.diagnostics(), observed.diagnostics());
        assert!(frozen.matches(&observed));
        assert!(!frozen.refresh(&observed));
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
        snapshots
            .requests
            .insert((router, newest), StoredRequest::from_view(&newest_view));

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

    #[test]
    fn publication_retains_queued_requests_and_prunes_evicted_terminal_records() {
        let (mut app, router) = snapshot_app();
        let mut requests = Vec::new();
        for _ in 0..66 {
            let request = app
                .world_mut()
                .resource_mut::<crate::RouterNavigation>()
                .push(router, RouteLocation::new("/items/1"))
                .unwrap();
            requests.push(request);
            settle_app_request(&mut app, router, request);
        }
        let snapshots = app.world().resource::<RouterSnapshots>();
        assert_eq!(
            snapshots.request(router, requests[0]),
            RequestLookup::NotRetained
        );
        assert!(matches!(
            snapshots.request(router, requests[2]),
            RequestLookup::Found(_)
        ));
        assert_eq!(snapshots.get(router).unwrap().history().len(), 66);
        assert!(snapshots.get(router).unwrap().history().can_back());

        let queued = app
            .world_mut()
            .resource_mut::<crate::RouterNavigation>()
            .push(router, RouteLocation::new("/items/2"))
            .unwrap();
        {
            use bevy_ecs::system::RunSystemOnce;
            app.world_mut()
                .run_system_once(crate::navigation::publish_snapshots_system)
                .unwrap();
        }
        assert!(matches!(
            app.world().resource::<RouterSnapshots>().request(router, queued),
            RequestLookup::Found(snapshot) if snapshot.status() == &RequestStatus::Queued
        ));
        settle_app_request(&mut app, router, queued);
        app.update();
        app.update();
        let back = app
            .world_mut()
            .resource_mut::<crate::RouterNavigation>()
            .back(router)
            .unwrap();
        settle_app_request(&mut app, router, back);
        assert_eq!(
            app.world()
                .resource::<RouterSnapshots>()
                .get(router)
                .unwrap()
                .history()
                .cursor(),
            Some(65)
        );
    }
}
