#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

use crate::driver::EcsViewDriver;
use bevy_app::App;
use bevy_ecs::{
    component::Component, lifecycle::HookContext, resource::Resource, world::DeferredWorld,
};
use bevy_ui_router_core::prelude::{RouteLocation, RouteTree, RouteViewKey};
use bevy_ui_router_view::{RouteViewRegistry, ViewDriverKey};

pub struct RouterConfig {
    routes: RouteTree,
    views: RouteViewRegistry,
    fallback: RouteViewKey,
    keep_alive_capacity: usize,
    initial: Option<RouteLocation>,
    transition_driver: Option<Box<dyn crate::transition::RouteTransitionDriver>>,
}

impl RouterConfig {
    pub fn new(routes: RouteTree, views: RouteViewRegistry, fallback: RouteViewKey) -> Self {
        Self {
            routes,
            views,
            fallback,
            keep_alive_capacity: 0,
            initial: None,
            transition_driver: None,
        }
    }

    /// Sets the maximum number of inactive opted-in page and layout instances.
    /// The default and zero disable retention.
    pub fn with_keep_alive_capacity(mut self, capacity: usize) -> Self {
        self.keep_alive_capacity = capacity;
        self
    }

    /// Navigates once when this router first becomes ready.
    ///
    /// A successfully enqueued explicit navigation consumes this default before
    /// it is submitted. Re-registering a host never replays a consumed initial.
    pub fn with_initial_location(mut self, location: impl Into<RouteLocation>) -> Self {
        self.initial = Some(location.into());
        self
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn set_transition_driver(
        &mut self,
        driver: Option<Box<dyn crate::transition::RouteTransitionDriver>>,
    ) {
        self.transition_driver = driver;
    }

    /// Adds application-owned animation around route publication. Without a
    /// driver navigation keeps its ordinary presentation and cleanup behavior.
    pub fn with_transition_driver(
        mut self,
        driver: impl crate::transition::RouteTransitionDriver,
    ) -> Self {
        self.transition_driver = Some(Box::new(driver));
        self
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
    MissingSceneRuntime,
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
            RouterInstallErrorKind::MissingSceneRuntime => "BSN requires AssetServer and Assets<ScenePatch>; install the Bevy asset and scene runtime before adding this router",
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
    drivers: Vec<(ViewDriverKey, ViewExecutor)>,
}

pub(crate) enum ViewExecutor {
    #[cfg(test)]
    Data(Box<dyn EcsViewDriver>),
    #[cfg(feature = "bsn")]
    Native(bevy_ui_router_bsn::BsnSceneFactory),
}

impl ViewDriverStore {
    pub(crate) const fn new() -> Self {
        Self {
            drivers: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn register<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError> {
        let key = driver.key();
        if self.drivers.iter().any(|(stored, _)| stored == &key) {
            return Err(ViewDriverRegistrationError::duplicate_key());
        }

        self.drivers
            .push((key.clone(), ViewExecutor::Data(Box::new(driver))));
        Ok(key)
    }

    pub(crate) fn get_mut(
        &mut self,
        _key: &ViewDriverKey,
    ) -> Option<&mut (dyn EcsViewDriver + 'static)> {
        self.drivers
            .iter_mut()
            .find_map(|(_stored, driver)| match driver {
                #[cfg(test)]
                ViewExecutor::Data(driver) if _stored == _key => Some(driver.as_mut()),
                _ => None,
            })
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn register_native(
        &mut self,
        key: ViewDriverKey,
        factory: bevy_ui_router_bsn::BsnSceneFactory,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError> {
        if self.contains(&key) {
            return Err(ViewDriverRegistrationError::duplicate_key());
        }
        self.drivers
            .push((key.clone(), ViewExecutor::Native(factory)));
        Ok(key)
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn install_native_batch(
        &mut self,
        factories: Vec<(ViewDriverKey, bevy_ui_router_bsn::BsnSceneFactory)>,
    ) {
        self.drivers.extend(
            factories
                .into_iter()
                .map(|(key, factory)| (key, ViewExecutor::Native(factory))),
        );
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn native_mut(
        &mut self,
        key: &ViewDriverKey,
    ) -> Option<&mut bevy_ui_router_bsn::BsnSceneFactory> {
        self.drivers
            .iter_mut()
            .find_map(|(stored, executor)| match executor {
                ViewExecutor::Native(factory) if stored == key => Some(factory),
                _ => None,
            })
    }

    #[cfg(feature = "bsn")]
    fn is_native(&self, key: &ViewDriverKey) -> bool {
        self.drivers
            .iter()
            .any(|(stored, executor)| stored == key && matches!(executor, ViewExecutor::Native(_)))
    }

    pub(crate) fn contains(&self, key: &ViewDriverKey) -> bool {
        self.drivers.iter().any(|(stored, _)| stored == key)
    }

    #[cfg(test)]
    pub(crate) fn take_for_test(
        &mut self,
        key: &ViewDriverKey,
    ) -> Option<(ViewDriverKey, ViewExecutor)> {
        let position = self.drivers.iter().position(|(stored, _)| stored == key)?;
        Some(self.drivers.remove(position))
    }

    #[cfg(test)]
    pub(crate) fn restore_for_test(&mut self, driver: (ViewDriverKey, ViewExecutor)) {
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
    fn add_ui_router(&mut self, config: RouterConfig) -> Result<RouterHandle, RouterInstallError>;
}

/// The independent unit-test renderer supplies real build/physical facts to
/// the normal Core/ECS pipeline without becoming a shipping registration API.
#[cfg(test)]
pub(crate) trait TestAppRouterExt {
    fn register_ui_view_driver<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError>;
}

#[cfg(test)]
impl TestAppRouterExt for App {
    fn register_ui_view_driver<D: EcsViewDriver>(
        &mut self,
        driver: D,
    ) -> Result<ViewDriverKey, ViewDriverRegistrationError> {
        crate::plugin::install_foundations(self);
        self.world_mut()
            .resource_mut::<ViewDriverStore>()
            .register(driver)
    }
}

impl AppRouterExt for App {
    fn add_ui_router(&mut self, config: RouterConfig) -> Result<RouterHandle, RouterInstallError> {
        crate::plugin::install_foundations(self);
        validate_router_config(self, &config, &[])?;
        install_validated_router(self, config)
    }
}

// Both the explicit registry API and the BSN declaration compiler use this
// validation. Additional keys belong to a fully prepared native factory batch.
pub(crate) fn validate_router_config(
    app: &App,
    config: &RouterConfig,
    additional_native: &[ViewDriverKey],
) -> Result<(), RouterInstallError> {
    let drivers = app.world().resource::<ViewDriverStore>();
    let mut binding_error = None;
    #[cfg(feature = "bsn")]
    let mut uses_native = false;
    bevy_ui_router_core::integration::for_each_route_view_binding(
        &config.routes,
        |normal, boundary, error| {
            if binding_error.is_some() {
                return;
            }
            let Some(normal) = normal else {
                binding_error = Some(RouterInstallErrorKind::MissingRouteView);
                return;
            };
            let Some(normal_driver) =
                bevy_ui_router_view::integration::driver_key(&config.views, normal)
            else {
                binding_error = Some(RouterInstallErrorKind::UnknownRouteView);
                return;
            };
            if !drivers.contains(normal_driver) && !additional_native.contains(normal_driver) {
                binding_error = Some(RouterInstallErrorKind::UnknownViewDriver);
                return;
            }
            #[cfg(feature = "bsn")]
            {
                uses_native |=
                    drivers.is_native(normal_driver) || additional_native.contains(normal_driver);
            }
            if boundary.is_some() {
                let Some(error) = error else {
                    binding_error = Some(RouterInstallErrorKind::MissingErrorView);
                    return;
                };
                let Some(error_driver) =
                    bevy_ui_router_view::integration::driver_key(&config.views, error)
                else {
                    binding_error = Some(RouterInstallErrorKind::UnknownRouteView);
                    return;
                };
                if !drivers.contains(error_driver) && !additional_native.contains(error_driver) {
                    binding_error = Some(RouterInstallErrorKind::UnknownViewDriver);
                }
                #[cfg(feature = "bsn")]
                {
                    uses_native |=
                        drivers.is_native(error_driver) || additional_native.contains(error_driver);
                }
            }
        },
    );
    if let Some(kind) = binding_error {
        return Err(RouterInstallError::new(kind));
    }
    let fallback_driver =
        bevy_ui_router_view::integration::driver_key(&config.views, &config.fallback)
            .ok_or_else(|| RouterInstallError::new(RouterInstallErrorKind::MissingFallbackView))?;
    if !drivers.contains(fallback_driver) && !additional_native.contains(fallback_driver) {
        return Err(RouterInstallError::new(
            RouterInstallErrorKind::UnknownViewDriver,
        ));
    }
    #[cfg(feature = "bsn")]
    if (uses_native
        || drivers.is_native(fallback_driver)
        || additional_native.contains(fallback_driver))
        && !crate::bsn::scene_runtime_is_present(app.world())
    {
        return Err(RouterInstallError::new(
            RouterInstallErrorKind::MissingSceneRuntime,
        ));
    }
    Ok(())
}

pub(crate) fn install_validated_router(
    app: &mut App,
    config: RouterConfig,
) -> Result<RouterHandle, RouterInstallError> {
    let RouterConfig {
        routes,
        views,
        fallback,
        keep_alive_capacity,
        initial,
        transition_driver,
    } = config;
    let handle = app
        .world_mut()
        .resource_mut::<crate::navigation::RouterNavigation>()
        .install(routes, views, fallback, keep_alive_capacity)
        .map_err(|_| RouterInstallError::new(RouterInstallErrorKind::UnknownRouteView))?;
    app.world_mut()
        .resource_mut::<crate::navigation::RouterNavigation>()
        .entries
        .iter_mut()
        .find(|entry| entry.handle == handle)
        .expect("the installed router entry is retained")
        .initial = initial;
    let projection = app
        .world()
        .resource::<crate::navigation::RouterNavigation>()
        .entries
        .iter()
        .find(|entry| entry.handle == handle)
        .expect("the installed router entry is retained")
        .engine
        .snapshot();
    app.world_mut()
        .resource_mut::<crate::snapshots::RouterSnapshots>()
        .install_router(handle, projection);
    if let Some(driver) = transition_driver {
        crate::transition::register_transition_driver(app.world_mut(), handle, driver)
            .expect("a newly installed router has no transition driver");
    }
    Ok(handle)
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
