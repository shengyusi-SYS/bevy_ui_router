//! Shared read facts for one active observation. Nothing survives its World borrow.

use std::cell::RefCell;

use bevy_ecs::{
    entity::EntityHashMap,
    prelude::{Entity, World},
};
#[cfg(feature = "bsn")]
use bevy_ui_router_core::frame::RouteFrameVersion;

#[cfg(feature = "bsn")]
use crate::{
    frame_local::FrameLocalError,
    presentation::native_frame::NativeContentObservation,
    registry::{ExactRegistryClaim, RouteEntityRecord},
};
use crate::{presentation::RetainedBoundaryProof, registry::RouteEntityRegistry};

pub(crate) struct ReadObservation<'w> {
    world: &'w World,
    registry: Option<&'w RouteEntityRegistry>,
    #[cfg(feature = "bsn")]
    records: EntityHashMap<&'w RouteEntityRecord>,
    retained: RefCell<EntityHashMap<Vec<RetainedPreservation<'w>>>>,
    #[cfg(feature = "bsn")]
    native: RefCell<EntityHashMap<Vec<CachedNativeContent<'w>>>>,
}

struct RetainedPreservation<'w> {
    boundary: &'w RetainedBoundaryProof,
    accepted: bool,
}

#[cfg(feature = "bsn")]
#[derive(Clone)]
pub(crate) struct NativeContentFacts<'w> {
    pub(crate) content: std::rc::Rc<NativeContentObservation<'w>>,
    pub(crate) default_outlets: usize,
}

#[cfg(feature = "bsn")]
struct CachedNativeContent<'w> {
    frame: RouteFrameVersion,
    root_claim: Option<ExactRegistryClaim>,
    result: Result<NativeContentFacts<'w>, FrameLocalError>,
}

impl<'w> ReadObservation<'w> {
    pub(crate) fn new(world: &'w World) -> Self {
        let registry = world.get_resource::<RouteEntityRegistry>();
        #[cfg(feature = "bsn")]
        let mut records = EntityHashMap::default();
        #[cfg(feature = "bsn")]
        if let Some(registry) = registry {
            for record in registry.records() {
                // Preserve record()'s first-record semantics even for invalid inventories.
                records.entry(record.entity()).or_insert(record);
            }
        }
        Self {
            world,
            registry,
            #[cfg(feature = "bsn")]
            records,
            retained: RefCell::default(),
            #[cfg(feature = "bsn")]
            native: RefCell::default(),
        }
    }

    pub(crate) fn world(&self) -> &'w World {
        self.world
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn record(&self, entity: Entity) -> Option<&'w RouteEntityRecord> {
        self.records.get(&entity).copied()
    }

    #[cfg(feature = "bsn")]
    pub(crate) fn native_content(
        &self,
        frame: &RouteFrameVersion,
        root: Entity,
    ) -> Result<NativeContentFacts<'w>, FrameLocalError> {
        let root_claim = self.record(root).map(RouteEntityRecord::claim);
        if let Some(result) = self.native.borrow().get(&root).and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.frame == *frame && entry.root_claim == root_claim)
                .map(|entry| entry.result.clone())
        }) {
            return result;
        }
        // No cache borrow crosses the proof; future nested observations may share this context.
        let result = crate::presentation::native_frame::observe_content_with_records(
            self.world,
            frame,
            root,
            |entity| self.record(entity),
        )
        .map(|content| {
            let default_outlets = content
                .managed
                .iter()
                .filter(|entity| {
                    self.world
                        .get::<bevy_ui_router_bsn::DefaultOutlet>(**entity)
                        .is_some()
                })
                .count();
            NativeContentFacts {
                content: std::rc::Rc::new(content),
                default_outlets,
            }
        });
        self.native
            .borrow_mut()
            .entry(root)
            .or_default()
            .push(CachedNativeContent {
                frame: frame.clone(),
                root_claim,
                result: result.clone(),
            });
        result
    }

    pub(crate) fn retained_shell_is_current(&self, shell: Entity, slot: Entity) -> bool {
        let Some(boundary) = self
            .registry
            .and_then(|registry| registry.retained_boundary_for_shell(shell, slot))
        else {
            return false;
        };
        if let Some(accepted) = self.retained.borrow().get(&shell).and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.boundary == boundary)
                .map(|entry| entry.accepted)
        }) {
            return accepted;
        }
        // This memoizes preservation only, never payload activation or cleanup authority.
        let accepted = boundary.preservation_is_current_with_observation(self.world, Some(self));
        self.retained
            .borrow_mut()
            .entry(shell)
            .or_default()
            .push(RetainedPreservation { boundary, accepted });
        accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        input::{RouteFocusState, RouterInputGate, try_prepare_input_gate_isolation},
        navigation::RouterNavigation,
        plugin::BevyUiRouterPlugin,
    };
    use bevy_app::App;
    use bevy_ecs::hierarchy::ChildOf;
    use bevy_input_focus::{FocusCause, InputFocus};
    use bevy_ui_router_core::prelude::{RequestStatus, RouteLocation};

    fn assert_preservation(world: &World, shell: Entity, slot: Entity, accepted: bool) {
        assert_eq!(
            world
                .resource::<RouteEntityRegistry>()
                .retained_shell_is_current(world, shell, slot),
            accepted
        );
        let observation = ReadObservation::new(world);
        assert_eq!(observation.retained_shell_is_current(shell, slot), accepted);
        assert_eq!(observation.retained_shell_is_current(shell, slot), accepted);
        assert!(!observation.retained_shell_is_current(shell, shell));
    }

    #[test]
    fn retained_preservation_observation_rechecks_relationship_gate_and_focus_after_writes() {
        let mut app = App::new();
        app.add_plugins(BevyUiRouterPlugin::default());
        let router = crate::navigation::tests::install_retained_slice_router(&mut app, 2);
        app.update();
        let mut saved_gate = None;
        let mut original = None;
        for path in ["/items/1", "/items/2"] {
            let request = app
                .world_mut()
                .resource_mut::<RouterNavigation>()
                .push(router, RouteLocation::new(path))
                .unwrap();
            for _ in 0..8 {
                app.update();
                if app.world().resource::<RouterNavigation>().entries[0]
                    .engine
                    .request(request)
                    .is_some_and(|request| matches!(request.status(), RequestStatus::Succeeded))
                {
                    break;
                }
            }
            let entry = &app.world().resource::<RouterNavigation>().entries[0];
            assert!(
                entry
                    .engine
                    .request(request)
                    .is_some_and(|request| matches!(request.status(), RequestStatus::Succeeded)),
                "fixture navigation failed for {path}: {:?}",
                entry
                    .engine
                    .request(request)
                    .map(|request| request.status().clone())
            );
            if original.is_none() {
                let active = entry.active.as_ref().unwrap();
                let leaf = active.frames().last().unwrap();
                original = Some((
                    leaf.shell(),
                    active.child_slots()[0].slot(),
                    leaf.interactive_entities()[0],
                ));
                // Keep a real original permission projection as the stale-input fact.
                saved_gate = Some(
                    try_prepare_input_gate_isolation(
                        app.world().resource::<RouterInputGate>(),
                        &[],
                    )
                    .unwrap(),
                );
            }
        }
        let (shell, slot, target) = original.unwrap();
        assert_preservation(app.world(), shell, slot, true);

        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(target, FocusCause::Navigated);
        assert_preservation(app.world(), shell, slot, false);
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(Some(target));
        assert_preservation(app.world(), shell, slot, false);
        app.world_mut()
            .resource_mut::<RouteFocusState>()
            .set_focused(None);
        assert_preservation(app.world(), shell, slot, true);

        let current_gate = app
            .world_mut()
            .remove_resource::<RouterInputGate>()
            .unwrap();
        let mut stale_gate = RouterInputGate::new();
        saved_gate.unwrap().apply(&mut stale_gate);
        app.world_mut().insert_resource(stale_gate);
        assert_preservation(app.world(), shell, slot, false);
        app.world_mut().insert_resource(current_gate);
        assert_preservation(app.world(), shell, slot, true);

        let application = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(shell)
            .insert(ChildOf(application));
        assert_preservation(app.world(), shell, slot, false);
        app.world_mut().entity_mut(shell).insert(ChildOf(slot));
        assert_preservation(app.world(), shell, slot, true);
    }
}
