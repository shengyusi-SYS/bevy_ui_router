//! Lower BSN declarations, then install through the shared router entry point.

use bevy_app::App;
use bevy_ui_router_bsn::BsnSceneFactory;
use bevy_ui_router_core::prelude::{
    ErrorBoundaryKey, RouteError, RouteId, RouteNodeOptions, RoutePattern, RouteTree,
    RouteTreeBuilder, RouteViewKey,
};
use bevy_ui_router_view::{RouteViewDescriptor, RouteViewRegistry, ViewDriverKey};

use super::declaration::{BsnRoute, BsnRouter};
use crate::app_ext::{
    RouterConfig, RouterHandle, RouterInstallError, ViewDriverStore, install_validated_router,
    validate_router_config,
};

/// An invalid declaration or a failure to install its existing routing machinery.
#[derive(Debug)]
pub enum BsnRouterError {
    MissingFallback,
    Route {
        declaration: String,
        source: RouteError,
    },
    Install(RouterInstallError),
    DriverKeySpaceExhausted,
}

impl std::fmt::Display for BsnRouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFallback => f.write_str("BSN router requires a fallback factory"),
            Self::Route {
                declaration,
                source,
            } => {
                write!(f, "invalid route at {declaration}: {source}")
            }
            Self::Install(source) => std::fmt::Display::fmt(source, f),
            Self::DriverKeySpaceExhausted => f.write_str("BSN driver key space is exhausted"),
        }
    }
}

impl std::error::Error for BsnRouterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Route { source, .. } => Some(source),
            Self::Install(source) => Some(source),
            _ => None,
        }
    }
}

struct Compilation {
    views: RouteViewRegistry,
    factories: Vec<(ViewDriverKey, BsnSceneFactory)>,
    namespace: usize,
}

impl Compilation {
    fn bind(&mut self, factory: BsnSceneFactory) -> RouteViewKey {
        let ordinal = self.factories.len();
        let view = RouteViewKey::new(format!("bsn.view.{ordinal}"))
            .expect("generated view names are nonempty");
        let driver = driver_key(self.namespace, ordinal);
        self.views
            .insert(view.clone(), RouteViewDescriptor::new(driver.clone()))
            .expect("each generated view name has a distinct ordinal");
        self.factories.push((driver, factory));
        view
    }

    fn lower(
        &mut self,
        route: BsnRoute,
        declaration: &str,
    ) -> Result<(RoutePattern, RouteNodeOptions, Vec<BsnRoute>), BsnRouterError> {
        let pattern = route.pattern.map_err(|source| BsnRouterError::Route {
            declaration: declaration.to_owned(),
            source,
        })?;
        let mut options = route.options.with_view(self.bind(route.factory));
        if let Some(factory) = route.error_factory {
            let boundary = ErrorBoundaryKey::named(format!("bsn.error.{}", self.factories.len()))
                .expect("generated error boundaries are nonempty");
            options = options.with_error_view(boundary, self.bind(factory));
        }
        Ok((pattern, options, route.children))
    }

    fn children(
        &mut self,
        tree: &mut RouteTreeBuilder,
        parent: RouteId,
        children: Vec<BsnRoute>,
        declaration: &str,
    ) -> Result<(), BsnRouterError> {
        for (ordinal, child) in children.into_iter().enumerate() {
            let declaration = format!("{declaration}/children[{ordinal}] ({})", child.label);
            let (pattern, options, grandchildren) = self.lower(child, &declaration)?;
            let route = tree
                .route_with_options(parent, pattern, options)
                .map_err(|source| BsnRouterError::Route {
                    declaration: declaration.clone(),
                    source,
                })?;
            self.children(tree, route, grandchildren, &declaration)?;
        }
        Ok(())
    }
}

fn driver_key(namespace: usize, ordinal: usize) -> ViewDriverKey {
    ViewDriverKey::new(format!("bevy_ui_router.bsn.{namespace}.{ordinal}"))
        .expect("generated driver names are nonempty")
}

fn count_factories(route: &BsnRoute) -> usize {
    1 + usize::from(route.error_factory.is_some())
        + route.children.iter().map(count_factories).sum::<usize>()
}

pub(super) fn install(
    app: &mut App,
    declaration: BsnRouter,
) -> Result<RouterHandle, BsnRouterError> {
    let BsnRouter {
        root,
        fallback,
        keep_alive_capacity,
        initial,
        transition_driver,
    } = declaration;
    let fallback = fallback.ok_or(BsnRouterError::MissingFallback)?;
    // Driver keys share the public registration namespace. Search actual keys,
    // including manually registered ones, instead of reserving a magic prefix.
    let count = count_factories(&root) + 1;
    let existing = app.world().get_resource::<ViewDriverStore>();
    let mut namespace = 0;
    while existing.is_some_and(|drivers| {
        (0..count).any(|ordinal| drivers.contains(&driver_key(namespace, ordinal)))
    }) {
        namespace = namespace
            .checked_add(1)
            .ok_or(BsnRouterError::DriverKeySpaceExhausted)?;
    }
    let mut compiled = Compilation {
        views: RouteViewRegistry::new(),
        factories: Vec::new(),
        namespace,
    };
    let declaration = format!("root ({})", root.label);
    let (pattern, options, children) = compiled.lower(root, &declaration)?;
    let mut routes = RouteTree::builder()
        .root_with_options(pattern, options)
        .map_err(|source| BsnRouterError::Route {
            declaration: declaration.clone(),
            source,
        })?;
    let root_id = routes.root_id().expect("the root was just declared");
    compiled.children(&mut routes, root_id, children, &declaration)?;
    let routes = routes.build().map_err(|source| BsnRouterError::Route {
        declaration,
        source,
    })?;
    let fallback = compiled.bind(fallback);
    let mut config = RouterConfig::new(routes, compiled.views, fallback)
        .with_keep_alive_capacity(keep_alive_capacity);
    if let Some(initial) = initial {
        config = config.with_initial_location(initial);
    }
    config.set_transition_driver(transition_driver);
    crate::plugin::install_foundations(app);
    crate::native_focus::install(app);
    let keys = compiled
        .factories
        .iter()
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    validate_router_config(app, &config, &keys).map_err(BsnRouterError::Install)?;
    // All fallible installation finishes before these factories enter the store.
    // No system can observe the AwaitingHost router between these synchronous steps.
    let router = install_validated_router(app, config).map_err(BsnRouterError::Install)?;
    app.world_mut()
        .resource_mut::<ViewDriverStore>()
        .install_native_batch(compiled.factories);
    Ok(router)
}

#[cfg(test)]
mod tests;
