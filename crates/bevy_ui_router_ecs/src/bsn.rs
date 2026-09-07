//! Native Scene preparation for the phase C executor migration.
//!
//! This module deliberately does not register a navigation driver yet. Its
//! prepared graph must be coupled to exact allocation obligations, current
//! containment, input isolation and cleanup before native publication is enabled.

#![allow(dead_code)] // Phase C: consumed by the forthcoming native realization path.

use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use bevy_asset::{AssetServer, Assets, Handle, UntypedHandle};
use bevy_ecs::{hierarchy::ChildOf, template::SceneEntityReference, world::World};
use bevy_scene::{CachedSceneError, ResolveSceneError, ResolvedSceneRoot, Scene, ScenePatch};
use bevy_ui_router_bsn::{BsnBuildError, BsnSceneFactory};
use bevy_ui_router_core::view_input::RouteViewInput;

static NEXT_SCENE_ATTEMPT: AtomicU64 = AtomicU64::new(0);

/// A declaration is not an owner. Realization must first allocate and register
/// each exact Entity before binding these references or executing any templates.
pub(crate) struct BsnSceneDeclaration {
    pub(crate) parent: Option<usize>,
    pub(crate) references: Vec<SceneEntityReference>,
}

/// One-shot resolved payload. Keep the dependency handles with this payload
/// through apply and settlement, rather than dropping its ScenePatch after resolve.
pub(crate) struct PreparedBsnScene {
    pub(crate) root: ResolvedSceneRoot,
    pub(crate) declarations: Vec<BsnSceneDeclaration>,
    pub(crate) dependencies: Vec<UntypedHandle>,
}

/// A rejected preparation still keeps its loaded dependencies until the caller
/// settles the attempt. This is data lifetime, not a Router lifecycle capability.
#[derive(Debug)]
pub(crate) struct BsnPreparationFailure {
    pub(crate) error: BsnPreparationError,
    pub(crate) dependencies: Vec<UntypedHandle>,
}

impl From<BsnPreparationError> for BsnPreparationFailure {
    fn from(error: BsnPreparationError) -> Self {
        Self {
            error,
            dependencies: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum BsnPreparationError {
    MissingSceneRuntime,
    Build(BsnBuildError),
    Resolve(ResolveSceneError),
    CachedInclusion(CachedSceneError),
    DuplicateReference(SceneEntityReference),
    Capacity,
    AttemptExhausted,
}

impl std::fmt::Display for BsnPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSceneRuntime => formatter.write_str(
                "BSN requires AssetServer and Assets<ScenePatch> in the application World",
            ),
            Self::Build(error) => write!(formatter, "BSN factory failed: {error}"),
            Self::Resolve(error) => write!(formatter, "BSN scene resolve failed: {error}"),
            Self::CachedInclusion(error) => write!(
                formatter,
                "cached Scene inclusion is unsupported by the synchronous BSN profile: {error}",
            ),
            Self::DuplicateReference(reference) => {
                write!(
                    formatter,
                    "BSN declaration repeats an entity reference: {reference}"
                )
            }
            Self::Capacity => formatter.write_str("BSN declaration graph capacity unavailable"),
            Self::AttemptExhausted => formatter.write_str("BSN scene attempt identity exhausted"),
        }
    }
}

impl std::error::Error for BsnPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Build(error) => Some(error),
            Self::Resolve(error) => Some(error),
            Self::CachedInclusion(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) fn scene_runtime_is_present(world: &World) -> bool {
    world.contains_resource::<AssetServer>() && world.contains_resource::<Assets<ScenePatch>>()
}

pub(crate) fn prepare_bsn_view(
    world: &World,
    factory: &mut BsnSceneFactory,
    input: &RouteViewInput,
) -> Result<PreparedBsnScene, BsnPreparationFailure> {
    // Check before invoking application code as well as before official resource access.
    if !scene_runtime_is_present(world) {
        return Err(BsnPreparationError::MissingSceneRuntime.into());
    }
    let scene = factory(input).map_err(BsnPreparationError::Build)?;
    prepare_bsn_scene(world, scene)
}

fn prepare_bsn_scene(
    world: &World,
    scene: Box<dyn Scene>,
) -> Result<PreparedBsnScene, BsnPreparationFailure> {
    let assets = world
        .get_resource::<AssetServer>()
        .ok_or(BsnPreparationError::MissingSceneRuntime)?;
    let patches = world
        .get_resource::<Assets<ScenePatch>>()
        .ok_or(BsnPreparationError::MissingSceneRuntime)?;
    let mut patch = ScenePatch::load(assets, scene);
    let prepared = (|| {
        patch
            .resolve(assets, patches)
            .map_err(BsnPreparationError::Resolve)?;
        // This local patch has never shared the resolved Arc. Consume, never clone it.
        let mut root =
            Arc::try_unwrap(patch.resolved.take().expect("successful resolve sets root"))
                .unwrap_or_else(|_| {
                    unreachable!("a local ScenePatch cannot share its resolved root")
                });
        let attempt = next_scene_attempt(&NEXT_SCENE_ATTEMPT)?;
        let declarations = prepare_declarations(&mut root, attempt)?;
        Ok((root, declarations))
    })();
    match prepared {
        Ok((root, declarations)) => Ok(PreparedBsnScene {
            root,
            declarations,
            dependencies: patch.dependencies,
        }),
        Err(error) => Err(BsnPreparationFailure {
            error,
            dependencies: patch.dependencies,
        }),
    }
}

fn next_scene_attempt(counter: &AtomicU64) -> Result<u64, BsnPreparationError> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(|previous| previous + 1)
        .map_err(|_| BsnPreparationError::AttemptExhausted)
}

fn prepare_declarations(
    root: &mut ResolvedSceneRoot,
    attempt: u64,
) -> Result<Vec<BsnSceneDeclaration>, BsnPreparationError> {
    let mut declarations = Vec::new();
    let mut references = HashSet::new();
    let mut pending = Vec::new();
    pending
        .try_reserve(1)
        .map_err(|_| BsnPreparationError::Capacity)?;
    pending.push((&mut root.scene, None));
    while let Some((scene, parent)) = pending.pop() {
        // 0.19.1 has no cached getter. Inserting the ChildOf collection ensures
        // include_cached cannot succeed/mutate this scene: an existing cache
        // produces MultipleCached; otherwise the nonempty related map yields
        // LateCached. This is also the documented empty-Children leaf behavior.
        scene.get_or_insert_related_resolved_scenes::<ChildOf>();
        match scene.include_cached(Handle::default()) {
            Err(error @ CachedSceneError::MultipleCached { .. }) => {
                return Err(BsnPreparationError::CachedInclusion(error));
            }
            Err(CachedSceneError::LateCached { .. }) => {}
            Ok(()) => unreachable!("ChildOf collection precludes late cached inclusion"),
        }

        let index = declarations.len();
        if scene.entity_references.is_empty() {
            scene
                .entity_references
                .try_reserve(1)
                .map_err(|_| BsnPreparationError::Capacity)?;
            scene.entity_references.push(SceneEntityReference::new(
                (file!(), line!() as usize, column!() as usize),
                index,
                attempt,
            ));
        }
        references
            .try_reserve(scene.entity_references.len())
            .map_err(|_| BsnPreparationError::Capacity)?;
        let mut node_references = Vec::new();
        node_references
            .try_reserve_exact(scene.entity_references.len())
            .map_err(|_| BsnPreparationError::Capacity)?;
        for &reference in &scene.entity_references {
            if !references.insert(reference) {
                return Err(BsnPreparationError::DuplicateReference(reference));
            }
            node_references.push(reference);
        }
        declarations
            .try_reserve(1)
            .map_err(|_| BsnPreparationError::Capacity)?;
        declarations.push(BsnSceneDeclaration {
            parent,
            references: node_references,
        });

        let children = &mut scene
            .get_or_insert_related_resolved_scenes::<ChildOf>()
            .scenes;
        pending
            .try_reserve(children.len())
            .map_err(|_| BsnPreparationError::Capacity)?;
        pending.extend(children.iter_mut().rev().map(|child| (child, Some(index))));
    }
    Ok(declarations)
}

#[cfg(test)]
mod tests;
