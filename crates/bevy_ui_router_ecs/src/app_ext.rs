#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

use crate::driver::EcsViewDriver;
use bevy_app::App;
use bevy_ecs::{
    component::Component, lifecycle::HookContext, resource::Resource, world::DeferredWorld,
};
use bevy_ui_router_core::prelude::{RouteTree, RouteViewKey};
use bevy_ui_router_view::{RouteViewRegistry, ViewDriverKey};

pub struct RouterConfig {
    routes: RouteTree,
    views: RouteViewRegistry,
    fallback: RouteViewKey,
}

impl RouterConfig {
    pub fn new(routes: RouteTree, views: RouteViewRegistry, fallback: RouteViewKey) -> Self {
        Self {
            routes,
            views,
            fallback,
        }
    }

    fn into_parts(self) -> (RouteTree, RouteViewRegistry, RouteViewKey) {
        (self.routes, self.views, self.fallback)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RouterHandle {
    domain: u64,
    router: u64,
}

impl RouterHandle {
    pub(crate) const fn from_parts(domain: u64, router: u64) -> Self {
        Self { domain, router }
    }

    pub(crate) const fn is_same_domain_as(self, other: Self) -> bool {
        self.domain == other.domain
    }

    #[cfg(test)]
    pub(crate) const fn fixture(domain: u64, router: u64) -> Self {
        Self { domain, router }
    }
}

static NEXT_HOST_REGISTRATION_INCARNATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostRegistrationIncarnationExhaustionEvidence {
    pub(crate) current: u64,
    pub(crate) attempted: u64,
}

fn next_host_registration_incarnation(
    counter: &AtomicU64,
) -> Result<u64, HostRegistrationIncarnationExhaustionEvidence> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(|previous| previous + 1)
        .map_err(|current| HostRegistrationIncarnationExhaustionEvidence {
            current,
            attempted: current,
        })
}

#[derive(Component)]
#[component(on_insert)]
pub struct RouterHostRegistration {
    router: RouterHandle,
    incarnation: u64,
    incarnation_exhaustion: Option<HostRegistrationIncarnationExhaustionEvidence>,
}

impl RouterHostRegistration {
    pub fn new(router: RouterHandle) -> Self {
        Self {
            router,
            incarnation: 0,
            incarnation_exhaustion: None,
        }
    }

    fn on_insert(mut world: DeferredWorld, context: HookContext) {
        let mut registration = world
            .get_mut::<Self>(context.entity)
            .expect("inserted host registration should exist");
        match next_host_registration_incarnation(&NEXT_HOST_REGISTRATION_INCARNATION) {
            Ok(incarnation) => {
                registration.incarnation = incarnation;
                registration.incarnation_exhaustion = None;
            }
            Err(evidence) => {
                registration.incarnation = 0;
                registration.incarnation_exhaustion = Some(evidence);
            }
        }
    }

    pub(crate) const fn router(&self) -> RouterHandle {
        self.router
    }

    #[cfg(test)]
    pub(crate) const fn testing_identity(&self) -> (RouterHandle, u64) {
        (self.router, self.incarnation)
    }

    pub(crate) const fn incarnation(&self) -> u64 {
        self.incarnation
    }

    pub(crate) const fn incarnation_exhaustion(
        &self,
    ) -> Option<HostRegistrationIncarnationExhaustionEvidence> {
        self.incarnation_exhaustion
    }

    #[cfg(test)]
    pub(crate) fn testing_mark_incarnation_exhausted(
        &mut self,
        evidence: HostRegistrationIncarnationExhaustionEvidence,
    ) {
        self.incarnation = 0;
        self.incarnation_exhaustion = Some(evidence);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterInstallErrorKind {
    MissingRouteView,
    MissingErrorView,
    UnknownRouteView,
    UnknownViewDriver,
    MissingFallbackView,
}

#[derive(Debug)]
pub struct RouterInstallError {
    kind: RouterInstallErrorKind,
}

impl RouterInstallError {
    const fn new(kind: RouterInstallErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> RouterInstallErrorKind {
        self.kind
    }
}

impl std::fmt::Display for RouterInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            RouterInstallErrorKind::MissingRouteView => "route is missing a view binding",
            RouterInstallErrorKind::MissingErrorView => "route is missing an error view binding",
            RouterInstallErrorKind::UnknownRouteView => "route view is not registered",
            RouterInstallErrorKind::UnknownViewDriver => "view driver is not registered",
            RouterInstallErrorKind::MissingFallbackView => "fallback view is not registered",
        })
    }
}

impl std::error::Error for RouterInstallError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewDriverRegistrationErrorKind {
    DuplicateKey,
}

#[derive(Debug)]
pub struct ViewDriverRegistrationError {
    kind: ViewDriverRegistrationErrorKind,
}

impl ViewDriverRegistrationError {
    const fn duplicate_key() -> Self {
        Self {
            kind: ViewDriverRegistrationErrorKind::DuplicateKey,
        }
    }

    pub fn kind(&self) -> ViewDriverRegistrationErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ViewDriverRegistrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("view driver key is already registered")
    }
}

impl std::error::Error for ViewDriverRegistrationError {}

#[derive(Resource)]
pub(crate) struct ViewDriverStore {
    drivers: Vec<(ViewDriverKey, Box<dyn EcsViewDriver>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ViewDriverStoreProjection {
    keys: Vec<ViewDriverKey>,
}

impl ViewDriverStore {
    pub(crate) const fn new() -> Self {
        Self {
            drivers: Vec::new(),
        }
    }

    pub(crate) fn register<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError> {
        let key = driver.key();
        if self.drivers.iter().any(|(stored, _)| stored == &key) {
            return Err(ViewDriverRegistrationError::duplicate_key());
        }

        self.drivers.push((key.clone(), Box::new(driver)));
        Ok(key)
    }

    pub(crate) fn get_mut(
        &mut self,
        key: &ViewDriverKey,
    ) -> Option<&mut (dyn EcsViewDriver + 'static)> {
        self.drivers
            .iter_mut()
            .find_map(|(stored, driver)| (stored == key).then_some(driver.as_mut()))
    }

    pub(crate) fn contains(&self, key: &ViewDriverKey) -> bool {
        self.drivers.iter().any(|(stored, _)| stored == key)
    }

    pub(crate) fn exact_projection(&self) -> ViewDriverStoreProjection {
        ViewDriverStoreProjection {
            keys: self.drivers.iter().map(|(key, _)| key.clone()).collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn take_for_test(
        &mut self,
        key: &ViewDriverKey,
    ) -> Option<(ViewDriverKey, Box<dyn EcsViewDriver>)> {
        let position = self.drivers.iter().position(|(stored, _)| stored == key)?;
        Some(self.drivers.remove(position))
    }

    #[cfg(test)]
    pub(crate) fn restore_for_test(&mut self, driver: (ViewDriverKey, Box<dyn EcsViewDriver>)) {
        assert!(
            !self.contains(&driver.0),
            "the removed driver key must remain absent until exact restoration"
        );
        self.drivers.push(driver);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.drivers.len()
    }

    #[cfg(test)]
    fn keys(&self) -> Vec<&ViewDriverKey> {
        self.drivers.iter().map(|(key, _)| key).collect()
    }
}

pub trait AppRouterExt {
    fn register_ui_view_driver<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError>;

    fn add_ui_router(&mut self, config: RouterConfig) -> Result<RouterHandle, RouterInstallError>;
}

impl AppRouterExt for App {
    fn register_ui_view_driver<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError> {
        crate::plugin::install_foundations(self);
        self.world_mut()
            .resource_mut::<ViewDriverStore>()
            .register(driver)
    }

    fn add_ui_router(&mut self, config: RouterConfig) -> Result<RouterHandle, RouterInstallError> {
        crate::plugin::install_foundations(self);
        let (routes, views, fallback) = config.into_parts();
        let drivers = self.world().resource::<ViewDriverStore>();
        let mut binding_error = None;
        bevy_ui_router_core::integration::for_each_route_view_binding(
            &routes,
            |normal, boundary, error| {
                if binding_error.is_some() {
                    return;
                }
                let Some(normal) = normal else {
                    binding_error = Some(RouterInstallErrorKind::MissingRouteView);
                    return;
                };
                let Some(normal_driver) =
                    bevy_ui_router_view::integration::driver_key(&views, normal)
                else {
                    binding_error = Some(RouterInstallErrorKind::UnknownRouteView);
                    return;
                };
                if !drivers.contains(normal_driver) {
                    binding_error = Some(RouterInstallErrorKind::UnknownViewDriver);
                    return;
                }
                if boundary.is_some() {
                    let Some(error) = error else {
                        binding_error = Some(RouterInstallErrorKind::MissingErrorView);
                        return;
                    };
                    let Some(error_driver) =
                        bevy_ui_router_view::integration::driver_key(&views, error)
                    else {
                        binding_error = Some(RouterInstallErrorKind::UnknownRouteView);
                        return;
                    };
                    if !drivers.contains(error_driver) {
                        binding_error = Some(RouterInstallErrorKind::UnknownViewDriver);
                    }
                }
            },
        );
        if let Some(kind) = binding_error {
            return Err(RouterInstallError::new(kind));
        }
        let fallback_driver = bevy_ui_router_view::integration::driver_key(&views, &fallback)
            .ok_or_else(|| RouterInstallError::new(RouterInstallErrorKind::MissingFallbackView))?;
        if !drivers.contains(fallback_driver) {
            return Err(RouterInstallError::new(
                RouterInstallErrorKind::UnknownViewDriver,
            ));
        }
        let handle = self
            .world_mut()
            .resource_mut::<crate::navigation::RouterNavigation>()
            .install(routes, views, fallback)
            .map_err(|_| RouterInstallError::new(RouterInstallErrorKind::UnknownRouteView))?;
        let projection = self
            .world()
            .resource::<crate::navigation::RouterNavigation>()
            .entries
            .iter()
            .find(|entry| entry.handle == handle)
            .expect("the installed router entry is retained")
            .engine
            .snapshot();
        self.world_mut()
            .resource_mut::<crate::snapshots::RouterSnapshots>()
            .install_router(handle, projection);
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ui_router_core::prelude::{
        ErrorBoundaryKey, RouteNodeOptions, RoutePattern, RouteTree, RouteViewKey,
    };
    use bevy_ui_router_core::test_support::{options_with_error_boundary, options_with_error_view};
    use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewDriverKey};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn assert_error_traits<
        T: std::fmt::Debug + std::fmt::Display + std::error::Error + Send + Sync + 'static,
    >() {
    }

    fn public_methods(type_name: &str) -> Vec<(String, bool)> {
        let file = syn::parse_file(include_str!("app_ext.rs")).unwrap();
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
    fn associated_api_is_closed_and_uses_the_exact_golden_function_shape() {
        assert_eq!(public_methods("RouterConfig"), vec![("new".into(), false)]);
        assert!(public_methods("RouterHandle").is_empty());
        assert_eq!(
            public_methods("RouterHostRegistration"),
            vec![("new".into(), false)]
        );
        assert_eq!(
            public_methods("RouterInstallError"),
            vec![("kind".into(), false)]
        );
        assert_eq!(
            public_methods("ViewDriverRegistrationError"),
            vec![("kind".into(), false)]
        );
    }

    #[test]
    fn config_stores_exact_inert_install_data() {
        let routes = RouteTree::builder()
            .root(RoutePattern::path("/").unwrap())
            .unwrap()
            .build()
            .unwrap();
        let root = routes.root();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let config = RouterConfig::new(routes, RouteViewRegistry::new(), fallback.clone());

        assert_eq!(config.routes.root(), root);
        let _ = &config.views;
        assert_eq!(config.fallback, fallback);
    }

    #[test]
    fn handle_is_opaque_and_host_registration_keeps_it_privately() {
        fn assert_handle_traits<T: Clone + Copy + std::fmt::Debug + Eq + std::hash::Hash>() {}
        fn assert_component<T: bevy_ecs::component::Component>() {}

        assert_handle_traits::<RouterHandle>();
        assert_component::<RouterHostRegistration>();

        let router = RouterHandle::fixture(11, 29);
        let registration = RouterHostRegistration::new(router);
        assert_eq!(registration.router(), router);
    }

    #[test]
    fn host_registration_incarnation_exhaustion_is_bounded_and_typed() {
        let counter = AtomicU64::new(u64::MAX);

        let outcome = next_host_registration_incarnation(&counter);

        assert_eq!(
            outcome,
            Err(HostRegistrationIncarnationExhaustionEvidence {
                current: u64::MAX,
                attempted: u64::MAX,
            })
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn install_and_driver_errors_have_the_frozen_kinds_and_error_traits() {
        assert_error_traits::<RouterInstallError>();
        assert_error_traits::<ViewDriverRegistrationError>();

        for kind in [
            RouterInstallErrorKind::MissingRouteView,
            RouterInstallErrorKind::MissingErrorView,
            RouterInstallErrorKind::UnknownRouteView,
            RouterInstallErrorKind::UnknownViewDriver,
            RouterInstallErrorKind::MissingFallbackView,
        ] {
            assert_eq!(RouterInstallError::new(kind).kind(), kind);
        }

        let duplicate = ViewDriverRegistrationError::duplicate_key();
        assert_eq!(
            duplicate.kind(),
            ViewDriverRegistrationErrorKind::DuplicateKey
        );
    }

    #[test]
    fn duplicate_driver_registration_is_zero_write_and_keeps_the_first_driver() {
        let first_drops = Arc::new(AtomicUsize::new(0));
        let replacement_drops = Arc::new(AtomicUsize::new(0));
        let key = ViewDriverKey::new("immediate").unwrap();
        let mut store = ViewDriverStore::new();

        assert_eq!(
            store
                .register(DropDriver::new(key.clone(), Arc::clone(&first_drops)))
                .unwrap(),
            key
        );
        let before = store.len();
        let error = store
            .register(DropDriver::new(key.clone(), Arc::clone(&replacement_drops)))
            .unwrap_err();

        assert_eq!(error.kind(), ViewDriverRegistrationErrorKind::DuplicateKey);
        assert_eq!(store.len(), before);
        assert_eq!(store.keys(), vec![&key]);
        assert_eq!(first_drops.load(Ordering::SeqCst), 0);
        assert_eq!(replacement_drops.load(Ordering::SeqCst), 1);
    }

    fn app_with_driver(key: &ViewDriverKey) -> App {
        let mut app = App::new();
        app.register_ui_view_driver(DropDriver::new(key.clone(), Arc::new(AtomicUsize::new(0))))
            .unwrap();
        app
    }

    fn registry_with(view: RouteViewKey, driver: ViewDriverKey) -> RouteViewRegistry {
        let mut views = RouteViewRegistry::new();
        views
            .insert(view, RouteViewDescriptor::new(driver))
            .unwrap();
        views
    }

    #[test]
    fn install_validates_normal_before_error_binding_on_the_same_node() {
        let driver = ViewDriverKey::new("install-driver").unwrap();
        let fallback = RouteViewKey::new("fallback").unwrap();
        let boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                options_with_error_boundary(RouteNodeOptions::new(), boundary),
            )
            .unwrap()
            .build()
            .unwrap();
        let views = registry_with(fallback.clone(), driver.clone());
        let mut app = app_with_driver(&driver);

        let error = app
            .add_ui_router(RouterConfig::new(routes, views, fallback))
            .unwrap_err();

        assert_eq!(error.kind(), RouterInstallErrorKind::MissingRouteView);
    }

    #[test]
    fn install_rejects_a_declared_boundary_without_an_error_view() {
        let driver = ViewDriverKey::new("install-driver").unwrap();
        let normal = RouteViewKey::new("normal").unwrap();
        let boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                options_with_error_boundary(
                    RouteNodeOptions::new().with_view(normal.clone()),
                    boundary,
                ),
            )
            .unwrap()
            .build()
            .unwrap();
        let views = registry_with(normal.clone(), driver.clone());
        let mut app = app_with_driver(&driver);

        let error = app
            .add_ui_router(RouterConfig::new(routes, views, normal))
            .unwrap_err();

        assert_eq!(error.kind(), RouterInstallErrorKind::MissingErrorView);
    }

    #[test]
    fn install_rejects_unknown_error_views_and_unregistered_drivers() {
        let registered_driver = ViewDriverKey::new("registered-driver").unwrap();
        let missing_driver = ViewDriverKey::new("missing-driver").unwrap();
        let normal = RouteViewKey::new("normal").unwrap();
        let error_view = RouteViewKey::new("error").unwrap();
        let boundary = ErrorBoundaryKey::named("root-boundary").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                options_with_error_view(
                    RouteNodeOptions::new().with_view(normal.clone()),
                    boundary,
                    error_view,
                ),
            )
            .unwrap()
            .build()
            .unwrap();
        let views = registry_with(normal.clone(), registered_driver.clone());
        let mut app = app_with_driver(&registered_driver);
        let error = app
            .add_ui_router(RouterConfig::new(routes, views, normal.clone()))
            .unwrap_err();
        assert_eq!(error.kind(), RouterInstallErrorKind::UnknownRouteView);

        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(normal.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let views = registry_with(normal.clone(), missing_driver);
        let error = app
            .add_ui_router(RouterConfig::new(routes, views, normal))
            .unwrap_err();
        assert_eq!(error.kind(), RouterInstallErrorKind::UnknownViewDriver);
    }

    #[test]
    fn install_validates_the_mandatory_fallback_after_route_bindings() {
        let driver = ViewDriverKey::new("install-driver").unwrap();
        let normal = RouteViewKey::new("normal").unwrap();
        let fallback = RouteViewKey::new("missing-fallback").unwrap();
        let routes = RouteTree::builder()
            .root_with_options(
                RoutePattern::path("/").unwrap(),
                RouteNodeOptions::new().with_view(normal.clone()),
            )
            .unwrap()
            .build()
            .unwrap();
        let views = registry_with(normal, driver.clone());
        let mut app = app_with_driver(&driver);

        let error = app
            .add_ui_router(RouterConfig::new(routes, views, fallback))
            .unwrap_err();

        assert_eq!(error.kind(), RouterInstallErrorKind::MissingFallbackView);
    }

    struct DropDriver {
        key: ViewDriverKey,
        drops: Arc<AtomicUsize>,
    }

    impl DropDriver {
        fn new(key: ViewDriverKey, drops: Arc<AtomicUsize>) -> Self {
            Self { key, drops }
        }
    }

    impl Drop for DropDriver {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl crate::driver::EcsViewDriver for DropDriver {
        fn key(&self) -> ViewDriverKey {
            self.key.clone()
        }

        fn apply(
            &mut self,
            _effect: &bevy_ui_router_view::ViewBuildEffect,
            _content: &mut crate::content::FrameContentWriter<'_>,
            _report: crate::driver::EcsViewReportSession<'_>,
        ) -> crate::driver::ViewBuildReport {
            unreachable!("the disconnected driver store never executes drivers")
        }
    }
}
