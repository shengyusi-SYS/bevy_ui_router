use bevy_ecs::{prelude::Entity, system::SystemParam};

use crate::active_scope::ActiveRouteScope;

/// Queries whether a target belongs to currently published active route content.
///
/// Native content added after publication is observed through its current parent
/// relationships. Retained, pending, leaving, quarantined and unrelated targets
/// return `false`. Activity is independent of authored visibility and input.
///
/// This parameter only reads Router metadata and hierarchy components, so it can
/// be used beside an application's `Query<(Entity, &mut T)>` for its own state.
///
/// Repeated or mixed queries can share work through [`crate::RouteQueries::read`].
#[derive(SystemParam)]
pub struct RouteActivity<'w, 's> {
    scope: ActiveRouteScope<'w, 's>,
}

impl RouteActivity<'_, '_> {
    /// Returns the target's activity at the time this method is called.
    ///
    /// Systems that need this update's completed publication can run after
    /// [`crate::RouterScheduleSet::PublishSnapshots`]. This query grants no
    /// lifecycle or cleanup authority.
    pub fn is_active(&self, target: Entity) -> bool {
        self.scope.resolve(target).is_some()
    }
}
