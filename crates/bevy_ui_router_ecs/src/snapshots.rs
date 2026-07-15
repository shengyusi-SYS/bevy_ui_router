#![allow(dead_code)]

use crate::app_ext::RouterHandle;
use bevy_ecs::resource::Resource;
use bevy_ui_router_core::diagnostics::DiagnosticEvent;
use bevy_ui_router_core::fingerprint::RouteBranchFingerprint;
use bevy_ui_router_core::prelude::{
    NavigationRequestId, NavigationTraceEvent, RequestStatus, RouteLocation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRequestSnapshot {
    requested: RouteLocation,
    matched: Option<RouteBranchFingerprint>,
    status: RequestStatus,
    trace: Vec<NavigationTraceEvent>,
    diagnostics: Vec<DiagnosticEvent>,
}

impl NavigationRequestSnapshot {
    fn new(
        requested: RouteLocation,
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

    pub fn requested(&self) -> &RouteLocation {
        &self.requested
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
    _private: (),
}

impl RouterSnapshot {
    const fn new() -> Self {
        Self { _private: () }
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
        self.requests
            .iter()
            .find_map(|entry| {
                (entry.router == router && entry.request == request).then_some(&entry.snapshot)
            })
            .map_or(RequestLookup::Unknown, RequestLookup::Found)
    }

    pub(crate) fn ensure_router(&mut self, router: RouterHandle) {
        if self.routers.iter().all(|(stored, _)| *stored != router) {
            self.routers.push((router, RouterSnapshot::new()));
        }
    }

    pub(crate) fn publish(&mut self, entries: &[crate::navigation::RouterEntry]) {
        self.requests.clear();
        for entry in entries {
            self.ensure_router(entry.handle);
            for request in &entry.requests {
                let Some(view) = entry.engine.request(*request) else {
                    continue;
                };
                self.requests.push(RequestSnapshotEntry {
                    router: entry.handle,
                    request: *request,
                    snapshot: NavigationRequestSnapshot::new(
                        view.requested().clone(),
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
    use bevy_ui_router_core::prelude::{NavigationRequestId, RequestStatus, RouteLocation};

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
    fn associated_api_exposes_only_the_task_seven_snapshot_surface() {
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
        assert!(public_methods("RouterSnapshot").is_empty());
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
            requested.clone(),
            None,
            RequestStatus::Queued,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(snapshot.requested(), &requested);
        assert_eq!(snapshot.matched(), None);
        assert_eq!(snapshot.status(), &RequestStatus::Queued);
        assert!(snapshot.trace().is_empty());
        assert!(snapshot.diagnostics().is_empty());
    }

    #[test]
    fn router_snapshot_is_an_opaque_final_trait_foundation() {
        fn assert_traits<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
        assert_traits::<RouterSnapshot>();
        let _ = RouterSnapshot::new();
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
}
