use bevy_ecs::{prelude::Entity, system::SystemParam};
use bevy_ui_router_core::view_input::RouteInputRef;

use crate::active_scope::ActiveRouteScope;

/// Read-only query for inputs of currently published active route content.
///
/// Resolves the nearest exact owning frame, including dynamically added native
/// descendants. Inactive or invalid boundaries return `None`, never inputs from
/// an active ancestor. This reads only Router metadata and hierarchy, and may be
/// used alongside an application's mutable component query.
#[derive(SystemParam)]
pub struct RouteInputs<'w, 's> {
    scope: ActiveRouteScope<'w, 's>,
}

impl RouteInputs<'_, '_> {
    /// Borrows route-local params and the complete published presentation location.
    ///
    /// Systems can run after [`crate::RouterScheduleSet::PublishSnapshots`] to see
    /// this update's publication, without waiting for request terminal. Error
    /// presentations use the failed request location, not successful history.
    /// The reference cannot outlive this parameter borrow or World mutation.
    pub fn current<'a>(&'a self, target: Entity) -> Option<RouteInputRef<'a>> {
        let (entry, frame) = self.scope.resolve(target)?;
        entry.engine.current_input(frame.frame())
    }
}
